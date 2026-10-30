//! The window preview wired to the menu: state, effects, mouse, capture.

use super::preview::{self, Effect, Event};
use super::*;

/// What keeps the preview from opening (or from staying open).
#[derive(Clone, Copy, Debug, Default)]
struct Blockers {
    hidden: bool,
    menu: bool,
    dragging: bool,
    searching: bool,
    other_page: bool,
    tools: bool,
    uninstall: bool,
    resizing: bool,
}

fn blocked(b: Blockers) -> bool {
    b.hidden || b.menu || b.dragging || b.searching || b.other_page || b.tools || b.uninstall || b.resizing
}

/// The surface less the margin on the four sides; never negative.
fn bounds_inside(room_width: f32, room: f32) -> iced::Rectangle {
    let w = (room_width - 2.0 * MARGIN).max(0.0);
    let h = (room - 2.0 * MARGIN).max(0.0);
    iced::Rectangle::new(iced::Point::new(MARGIN, MARGIN), iced::Size::new(w, h))
}

/// The windows to ask a picture of: not already asked, and not minimized
/// with a picture (it cannot change).
fn thumbs_to_request(group: &[String], capturing: &HashSet<String>, minimized_with_photo: &HashSet<String>) -> Vec<String> {
    group
        .iter()
        .filter(|id| !capturing.contains(*id) && !minimized_with_photo.contains(*id))
        .cloned()
        .collect()
}

/// A picture is kept only while the preview shows, for a window of the shown group.
fn should_keep(shown: bool, group_ids: &[String], id: &str) -> bool {
    shown && group_ids.iter().any(|g| g == id)
}

/// Closes the preview through the state machine (never by a fresh `Hover`:
/// the generations only grow, so an old timer or measure can never match).
fn reset_hover(h: &mut preview::Hover) -> Vec<Effect> {
    let ctx = preview::Ctx { blocked: true, bounds: iced::Rectangle::default(), count: |_: &str| 0 };
    preview::step(h, Event::Close, &ctx)
}

/// `COSMIC_START_PREVIEW=windows:<app>`: the app, lowercase.
fn env_target(var: Option<&str>) -> Option<String> {
    let app = var?.strip_prefix("windows:")?.trim();
    (!app.is_empty()).then(|| app.to_lowercase())
}

/// The key of the first group with a window of the app (`(key, app_id)` pairs).
fn group_for_app(windows: &[(String, String)], app: &str) -> Option<String> {
    windows.iter().find(|(_, app_id)| app_id.eq_ignore_ascii_case(app)).map(|(key, _)| key.clone())
}

/// The mouse events the preview follows, for the window they came from.
fn mouse_event(event: iced::Event, window: window::Id) -> Option<Message> {
    let ev = match event {
        iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
            pointer_stats::note(pointer_stats::Gesture::Preview, position);
            Event::Pointer(position)
        }
        iced::Event::Mouse(iced::mouse::Event::CursorLeft) => Event::PointerGone,
        iced::Event::Mouse(iced::mouse::Event::WheelScrolled { .. }) => Event::Wheel,
        _ => return None,
    };
    Some(Message::PreviewMouse(window, ev))
}

fn listen_mouse(event: iced::Event, _: event::Status, window: window::Id) -> Option<Message> {
    mouse_event(event, window)
}

impl StartMenu {
    fn preview_bounds(&self) -> iced::Rectangle {
        bounds_inside(self.room_width, self.room)
    }

    fn preview_blocked(&self) -> bool {
        blocked(Blockers {
            hidden: self.surface.is_none(),
            menu: self.menu.is_some(),
            dragging: self.drag.is_active(),
            searching: !self.query.trim().is_empty(),
            other_page: self.page != Page::Home,
            tools: self.tools,
            uninstall: self.uninstall.is_some(),
            resizing: !self.resize.is_idle(),
        })
    }

    /// Runs one event through the state machine; the effects are left to the caller.
    fn step_hover(&mut self, ev: Event) -> Vec<Effect> {
        let mut hover = std::mem::take(&mut self.hover);
        let ctx = preview::Ctx {
            blocked: self.preview_blocked(),
            bounds: self.preview_bounds(),
            count: |key: &str| self.group_of(key).map_or(0, |g| g.windows.len()),
        };
        let effects = preview::step(&mut hover, ev, &ctx);
        self.hover = hover;
        effects
    }

    pub(super) fn preview(&mut self, ev: Event) -> app::Task<Message> {
        let effects = self.step_hover(ev);
        tracing::trace!(
            generation = self.hover.generation(),
            measure = self.hover.measure(),
            phase = ?self.hover.phase(),
            ?effects,
            "preview"
        );
        PREVIEW_SHOWN.store(self.hover.is_shown(), Ordering::Relaxed);
        Task::batch(effects.into_iter().map(|fx| self.run_effect(fx)))
    }

    /// Whether the preview is waiting or showing.
    pub(super) fn preview_active(&self) -> bool {
        *self.hover.phase() != preview::Phase::Idle
    }

    /// Closes the preview now, with no task to wait for.
    pub(super) fn preview_close(&mut self) {
        if reset_hover(&mut self.hover).contains(&Effect::StopCapture) {
            self.thumbs.clear();
            self.capturing.clear();
        }
        self.preview_hot = None;
        PREVIEW_SHOWN.store(false, Ordering::Relaxed);
    }

    fn run_effect(&mut self, fx: Effect) -> app::Task<Message> {
        match fx {
            Effect::Wait(delay, generation) => Task::perform(tokio::time::sleep(delay), move |()| {
                cosmic::action::app(Message::Preview(Event::Timer(generation)))
            }),
            Effect::Measure(key, generation) => anchor::find(scoped("open", &key))
                .map(move |r| cosmic::Action::App(Message::Preview(Event::Anchor(generation, r)))),
            Effect::StartCapture(_) => {
                self.request_thumbs();
                Task::none()
            }
            Effect::StopCapture => {
                self.thumbs.clear();
                self.capturing.clear();
                Task::none()
            }
        }
    }

    /// The ids of the windows of the group the preview shows.
    fn shown_ids(&self) -> Vec<String> {
        if !self.hover.is_shown() {
            return Vec::new();
        }
        let Some(group) = self.hover.key().and_then(|k| self.group_of(k)) else {
            return Vec::new();
        };
        group.windows.iter().filter_map(|&i| self.windows.get(i)).map(|w| w.id.clone()).collect()
    }

    pub(super) fn request_thumbs(&mut self) {
        let Some(tx) = &self.windows_tx else {
            return;
        };
        let held: HashSet<String> = self
            .windows
            .iter()
            .filter(|w| w.minimized && self.thumbs.contains_key(&w.id))
            .map(|w| w.id.clone())
            .collect();
        let ids = thumbs_to_request(&self.shown_ids(), &self.capturing, &held);
        if ids.is_empty() {
            return;
        }
        self.capturing.extend(ids.iter().cloned());
        if let Err(err) = tx.send(Command::Capture(ids)) {
            // Never asked: the ids must not stay "in flight".
            if let Command::Capture(lost) = err.0 {
                for id in lost {
                    self.capturing.remove(&id);
                }
            }
        }
    }

    pub(super) fn preview_thumb(&mut self, id: String, handle: Option<cosmic::widget::image::Handle>) {
        self.capturing.remove(&id);
        if should_keep(self.hover.is_shown(), &self.shown_ids(), &id) {
            toplevels::store_thumb(&mut self.thumbs, id, handle);
        }
    }

    /// Forgets the pictures and requests of windows that are gone.
    pub(super) fn prune_thumbs(&mut self) {
        let windows = &self.windows;
        self.thumbs.retain(|id, _| windows.iter().any(|w| w.id == *id));
        self.capturing.retain(|id| windows.iter().any(|w| w.id == *id));
    }

    pub(super) fn preview_subscription(&self) -> Subscription<Message> {
        if *self.hover.phase() == preview::Phase::Idle {
            return Subscription::none();
        }
        let refresh = if self.hover.is_shown() {
            cosmic::iced::time::every(preview::REFRESH).map(|_| Message::PreviewRefresh)
        } else {
            Subscription::none()
        };
        Subscription::batch([event::listen_with(listen_mouse), refresh])
    }

    pub(super) fn preview_layer(&self, p: Palette) -> Option<Element<'_, Message>> {
        if !self.hover.is_shown() {
            return None;
        }
        self.preview_card(self.hover.key()?, self.hover.anchor()?, self.preview_bounds(), p)
    }

    /// COSMIC_START_PREVIEW=windows:<app>: opens that app's preview, once,
    /// when its windows are there.
    pub(super) fn preview_env(&mut self) -> app::Task<Message> {
        let Some(app) = self.preview_pending.clone() else {
            return Task::none();
        };
        let pairs: Vec<(String, String)> =
            self.windows.iter().map(|w| (self.window_key(w), w.app_id.clone())).collect();
        let Some(key) = group_for_app(&pairs, &app) else {
            return Task::none();
        };
        self.preview_pending = None;
        self.preview(Event::ClickGroup(key))
    }

    /// The `COSMIC_START_PREVIEW` request to preview an app's windows.
    pub(super) fn preview_request() -> Option<String> {
        env_target(std::env::var("COSMIC_START_PREVIEW").ok().as_deref())
    }
}

#[cfg(test)]
mod tests;
