// SPDX-License-Identifier: GPL-3.0-only

//! Windows 11's Start menu, in place of COSMIC's launcher (the Super key) and
//! its app library, both in one, as the 2025 one is: one page, the search on
//! top, already focused; the pinned apps, the open windows, the recommended
//! (apps just installed or used, files just changed) and all the apps, by
//! category, in a grid or a list; the user and the power button at the foot.
//! It shows at once, without rising from the panel.
//!
//! The menu is the program run with `--menu`, a layer surface over the
//! screen, not an applet's popup: so the Super key, a shortcut, can open
//! it, and it gets the keyboard. The first run stays, hidden between uses;
//! the next ones ask it, over D-Bus, to show or hide, and leave. Run as the
//! panel runs applets, without it, it is the button, which does the same.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, LazyLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use cosmic::{
    Element, Task, app,
    cctk::sctk::reexports::calloop,
    cosmic_config::{Config, ConfigGet, ConfigSet},
    desktop::{IconSourceExt, fde},
    iced::{
        self, Alignment, Background, Color, Length, Limits, Subscription,
        event::{self, wayland},
        futures::{self, SinkExt, channel::mpsc},
        keyboard::{self, Key, key::Named},
        platform_specific::{
            runtime::wayland::layer_surface::SctkLayerSurfaceSettings,
            shell::commands::layer_surface::{
                Anchor, KeyboardInteractivity, Layer, destroy_layer_surface, get_layer_surface,
            },
        },
        stream,
        widget::text::Wrapping,
        window,
    },
    theme,
    widget::{self, button, icon, text_input},
};

use crate::texts::texts;
use crate::uninstall::{self, Plan};

const PINNED_AREA: &str = "pinned-area";
const MENU_BODY: &str = "menu-body";

mod anchor;
mod application;
mod capture;
mod desktop_entry;
mod drag;
mod drag_glue;
mod drop;
mod events;
mod group_ui;
mod groups;
mod icon_cache;
mod icon_raster;
mod trim;
mod launch;
mod load;
mod load_icons;
mod metrics;
mod open_glue;
mod open_groups;
mod open_strip;
mod pages;
mod pointer_stats;
mod popup;
mod preview;
mod preview_card;
mod preview_ui;
mod recent;
mod recent_files;
mod resize;
mod resize_glue;
mod search;
mod service;
mod start_button;
mod style;
mod tiles;
mod toplevels;
mod uninstall_flow;
mod uninstall_ui;
pub use launch::{self_exe, spawn_clean};
pub use load::migrate;
pub use service::{toggle_running, tools_running};
pub use start_button::StartButton;
pub use style::{Palette, flat_class, symbolic};
pub use toplevels::show_desktop;
use drag_glue::{drop_before, dragged, draggable, tile_overlay, unpin_bar_at};
use launch::spawn;
use load::{app_for_id, avatar, config, find_app, fold, load_groups, load_pinned, locales, panel_on_top, reload_apps, same_entry, save_groups, save_pinned, user_name};
use metrics::Metrics;
use pages::Pick;
use popup::focus_id;
use recent_files::{ago, file_times, load_recents, now, put_first};
use service::dbus_service;
use style::{folder_class, named_icon, palette, pill_class, px};
use tiles::{ellipsize, icons_of};
use toplevels::windows_subscription;
use uninstall_ui::{Dialog, Event as DialogEvent, Phase};

pub const APP_ID: &str = "io.github.Aenes0.CosmicStart.Menu";

/// Room around the menu for its shadow.
const MARGIN: f32 = 16.0;
/// The least the menu is, on a surface too short for it.
const MIN_HEIGHT: f32 = 300.0;
const FOOTER: f32 = 64.0;

/// The strip's first window after the windows changed: the active one
/// shown if it is `reveal`ed, else the offset kept within the new total.
/// The active window is also revealed when the capacity changed with the
/// total (10 to 11 windows brings the arrows, and one place less).
fn adjust_open(prev_cap: usize, cap: usize, total: usize, active: Option<usize>, changed: bool, offset: usize) -> usize {
    let moved = changed || prev_cap != cap;
    match active.filter(|_| moved) {
        Some(index) => open_strip::reveal(total, cap, offset, index),
        None => open_strip::clamp(total, cap, offset),
    }
}

/// The id of an item in a scope: the same app shows in several sections.
fn scoped(scope: &str, key: &str) -> widget::Id {
    widget::Id::new(format!("{scope}:{key}"))
}


static SEARCH: LazyLock<widget::Id> = LazyLock::new(|| widget::Id::new("start-search"));
static HOME: LazyLock<widget::Id> = LazyLock::new(|| widget::Id::new("start-home"));
/// Whether the menu shows, for the windows' thread.
static WINDOWS_SHOWN: AtomicBool = AtomicBool::new(false);
/// Whether the window preview shows, for the capture thread.
static PREVIEW_SHOWN: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug)]
pub struct AppEntry {
    id: String,
    name: String,
    /// `id` and `wm_class` in lower case, for `find_app`, which runs in each view.
    id_lower: String,
    wm_class_lower: Option<String>,
    exec: Option<String>,
    terminal: bool,
    icon: icon::Handle,
    /// Pre-rendered icons for the CPU renderer; empty with wgpu.
    rasters: AppRasters,
    /// The name, the generic name, the keywords…, folded, to search in.
    haystack: String,
    folded_name: String,
    actions: Vec<(String, String)>,
    category: usize,
    /// When its desktop entry appeared, in seconds since the epoch.
    added: i64,
    /// For the uninstall, all read in `load_apps`: the view does no I/O.
    path: PathBuf,
    flatpak_ref: Option<String>,
    flatpak_part_of: Option<String>,
    shadows_other_data_dir: bool,
    origin: uninstall::Origin,
}

/// The icon pre-rasterized at the two sizes the screens draw it (41 and 28 px).
#[derive(Clone, Debug, Default)]
pub struct AppRasters {
    big: Option<icon::Handle>,
    small: Option<icon::Handle>,
}

impl AppEntry {
    /// The icon to draw at `size` px: the raster when there is one for it, else the original.
    fn icon_at(&self, size: u16) -> icon::Handle {
        let raster = if size == px(36.0) {
            &self.rasters.big
        } else if size == px(24.0) {
            &self.rasters.small
        } else {
            &None
        };
        raster.clone().unwrap_or_else(|| self.icon.clone())
    }

    fn facts(&self) -> uninstall::DesktopFacts {
        uninstall::DesktopFacts {
            appid: self.id.clone(),
            path: self.path.clone(),
            exec: self.exec.clone(),
            flatpak_ref: self.flatpak_ref.clone(),
            flatpak_part_of: self.flatpak_part_of.clone(),
            shadows_other_data_dir: self.shadows_other_data_dir,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    id: String,
    title: String,
    app_id: String,
    active: bool,
    minimized: bool,
}

#[derive(Clone, Debug)]
pub struct RecentFile {
    path: String,
    name: String,
    icons: Vec<String>,
    when: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Home,
    Recommended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AllView {
    Category,
    Grid,
    List,
}

/// Whether the targets answered belong to the arm in force.
fn fresh(generation: u64, current: u64) -> bool {
    generation == current
}

/// A pinned entry: an app, or a group with its apps.
enum PinnedItem<'a> {
    App(&'a AppEntry),
    Group(groups::GroupRef, Vec<&'a AppEntry>),
}

/// What a click opened a little menu for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    App(String),
    Window(String),
    /// The icon of a group of open windows, by its key.
    OpenGroup(String),
    File(String),
    Group(groups::GroupRef),
    /// The context menu of a group.
    GroupMenu(groups::GroupRef),
    User,
    Power,
    Uninstall(String),
}

#[derive(Clone, Debug)]
pub enum Message {
    Toggle,
    /// Win+X: the tools' menu, or away if it shows.
    Tools,
    Tool(crate::tools::Tool),
    Hide,
    Unfocused,
    Escape,
    Focus,
    Focused,
    Resized(window::Id, iced::Size),
    Apps(u64, Arc<Vec<AppEntry>>),
    Trim,
    /// A frame of the open/close animation.
    AnimTick,
    /// The closing animation's backstop: the surface goes now.
    HideNow,
    Recents(Vec<RecentFile>),
    Windows(Vec<Window>),
    WindowsReady(calloop::channel::Sender<Command>),
    /// The thumbnail of a window, or `None` when it could not be taken.
    Thumb(String, Option<cosmic::widget::image::Handle>),
    Query(String),
    TypeAhead(String),
    Move(i32),
    Submit,
    Page(Page),
    ExpandPinned(bool),
    NextView,
    Launch(String),
    LaunchAction(String, String),
    Activate(String),
    CloseWindow(String),
    /// A click on the icon of a group of open windows (its key).
    OpenGroupClick(String),
    CloseWindows(Vec<String>),
    OpenFile(String),
    OpenLocation(String),
    Menu(Option<Target>),
    DragArm(drag::Source, iced::Point),
    DragMove(window::Id, iced::Point),
    DragRelease(window::Id),
    ResizeStart(resize::Edge),
    ResizeMove(window::Id, iced::Point),
    ResizeEnd(window::Id),
    ResizeReset,
    /// The measured targets of the arm that has this generation.
    DragTargets(u64, Vec<drag::Target>),
    /// A pause over a center may have lasted enough.
    DragTick,
    /// The draft of a user group's name.
    GroupName(u32, String),
    /// The name is confirmed (Enter, or the popup closes).
    GroupNameDone,
    RenameGroup(u32),
    DeleteGroup(u32),
    OpenPage(bool),
    MenuFor(Target, widget::Id),
    MenuAt(Target, Option<iced::Rectangle>),
    TogglePin(String),
    AskUninstall(String),
    UninstallPlan(String, Result<Plan, String>),
    ToggleDeleteData(bool),
    ConfirmUninstall,
    UninstallDone(Result<(), String>),
    Spawn(&'static str, Vec<String>),
    Nothing,
    /// The window card the pointer is on in the preview, if any.
    PreviewHot(Option<String>),
    Preview(preview::Event),
    PreviewMouse(window::Id, preview::Event),
    PreviewRefresh,
}

#[derive(Clone, Debug)]
pub enum Command {
    /// Send the windows as they are now: the menu shows.
    Refresh,
    Activate(String),
    Close(String),
    /// Take the thumbnails of these windows.
    Capture(Vec<String>),
}

pub struct StartMenu {
    core: app::Core,
    surface: Option<window::Id>,
    shown_at: Instant,
    /// The height the surface got: the screen, less the panel.
    room: f32,
    /// The surface's width: with `room`, the whole surface's size.
    room_width: f32,
    /// The measures the layout follows, from the menu's size.
    metrics: Metrics,
    /// Where the item that opened the little menu is, if it was found.
    menu_anchor: Option<iced::Rectangle>,
    /// The first open window the strip shows.
    open_offset: usize,
    /// The key of the group that was active when the windows last came.
    open_active: Option<String>,
    /// The open windows by app, in the order they first appeared.
    open_groups: Vec<open_groups::OpenGroup>,
    /// Dragging an app to pin it or to move a pinned one.
    drag: drag::Drag,
    /// Set when armed: the first move gives the point the threshold counts from.
    drag_fresh: bool,
    drag_targets: Vec<drag::Target>,
    drag_gen: u64,
    /// The user group whose name is being edited, with the draft.
    group_edit: Option<(u32, String)>,
    hidden_at: Instant,
    /// The open/close animation: 0 = away, 1 = in place.
    anim: crate::anim::Spring,
    anim_last: Option<Instant>,
    /// The menu is sliding out; its surface goes when it is away.
    closing: bool,
    /// The application folders as they were when the apps were read.
    apps_signature: Option<u64>,
    apps: Arc<Vec<AppEntry>>,
    recents: Vec<RecentFile>,
    /// Icons of the windows with no app, by app id; filled on update.
    window_icons: icon_cache::IconCache,
    /// Icons of the recent files, by path; filled on update.
    file_icons: icon_cache::IconCache,
    /// For a window whose icon is not (yet) in the cache.
    generic_icon: icon::Handle,
    /// For a file whose icon is not (yet) in the cache.
    generic_file_icon: icon::Handle,
    /// The apps opened from here, the last first, with when.
    launched: Vec<(String, i64)>,
    windows: Vec<Window>,
    windows_tx: Option<calloop::channel::Sender<Command>>,
    /// The thumbnails of the windows, by id; gone when the menu hides.
    thumbs: HashMap<String, cosmic::widget::image::Handle>,
    /// The id of the window card the pointer is on in the preview.
    preview_hot: Option<String>,
    /// The window preview's hover state.
    hover: preview::Hover,
    /// Windows whose thumbnail was asked and has not come.
    capturing: HashSet<String>,
    /// COSMIC_START_PREVIEW=windows:<app>: open that app's preview once.
    preview_pending: Option<String>,
    pinned: Vec<String>,
    groups: groups::Groups,
    pinned_expanded: bool,
    all_view: AllView,
    query: String,
    selected: usize,
    page: Page,
    menu: Option<Target>,
    /// The uninstall dialog: its phase and whether to delete the data.
    uninstall: Option<Dialog>,
    /// The app just uninstalled, to forget once the apps are read again.
    last_uninstalled: Option<String>,
    /// COSMIC_START_PREVIEW=uninstall:<id>: open that app's uninstall card
    /// once the apps are read.
    preview_uninstall: Option<String>,
    user: String,
    avatar: Option<PathBuf>,
    on_top: bool,
    /// The size asked for (never the effective one); `None` is the default.
    menu_pref: Option<iced::Size>,
    /// `menu_pref` when a resize began: Escape restores it.
    pref_before: Option<iced::Size>,
    resize: resize::Resize,
    /// False when the size came from the env: not saved.
    persist_size: bool,
    /// Showing the tools (Win+X) instead of the menu.
    tools: bool,
    power_open: bool,
}

impl StartMenu {
    /// Closes the little menu and forgets where it was anchored.
    fn close_menu(&mut self) {
        self.commit_name();
        self.menu = None;
        self.menu_anchor = None;
    }

    fn show(&mut self) -> app::Task<Message> {
        if self.surface.is_some() {
            return Task::none();
        }
        let id = window::Id::unique();
        self.surface = Some(id);
        self.shown_at = Instant::now();
        self.closing = false;
        self.anim_last = None;
        self.anim = crate::anim::Spring::new(if crate::anim::motion_scale() > 0.0 { 0.0 } else { 1.0 });
        self.query.clear();
        self.selected = 0;
        self.page = Page::Home;
        self.cancel_drag();
        self.resize_cancel();
        // An uninstall's outcome is not lost to a closed menu.
        if !self.uninstall.as_ref().is_some_and(Dialog::keep_on_show) {
            self.close_menu();
            self.uninstall = None;
        }
        self.pinned_expanded = false;
        self.reset_open();
        self.tools = false;
        self.power_open = false;
        self.on_top = panel_on_top();
        // Over the whole screen, transparent but for the menu: a click
        // anywhere else, on the wallpaper too, lands on it and closes it, as
        // on Windows. A surface the menu's size alone costs as much to
        // open (its making, not its pixels, is what costs), and a click on
        // the wallpaper wouldn't close it.
        let surface = get_layer_surface(SctkLayerSurfaceSettings {
            id,
            layer: Layer::Top,
            keyboard_interactivity: KeyboardInteractivity::Exclusive,
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            namespace: "central-start".into(),
            size: Some((None, None)),
            exclusive_zone: 0,
            size_limits: Limits::NONE.min_width(1.0).min_height(1.0),
            ..Default::default()
        });
        WINDOWS_SHOWN.store(true, Ordering::Relaxed);
        if let Some(tx) = &self.windows_tx {
            _ = tx.send(Command::Refresh);
        }
        // Apps come and go, but seldom: read them again only when a folder
        // of them changed. Files are quick to look at, each time.
        let apps = reload_apps(self.apps_signature);
        let recents = Task::perform(
            async { tokio::task::spawn_blocking(load_recents).await.unwrap_or_default() },
            |recents| cosmic::action::app(Message::Recents(recents)),
        );
        let focus = Task::perform(tokio::time::sleep(Duration::from_millis(60)), |()| {
            cosmic::action::app(Message::Focus)
        });
        // The size does not wait for an event: ask it.
        let size = window::size(id).map(move |size| cosmic::Action::App(Message::Resized(id, size)));
        Task::batch([surface, size, apps, recents, focus])
    }

    /// Closes the menu: it slides out first, then `finish_hide` takes the surface.
    fn hide(&mut self) -> app::Task<Message> {
        if self.surface.is_none() || self.closing {
            return Task::none();
        }
        if crate::anim::motion_scale() <= 0.0 {
            return self.finish_hide();
        }
        self.closing = true;
        self.anim_last = None;
        // The ticks end it; this is for the day they stop coming.
        Task::perform(tokio::time::sleep(Duration::from_millis(500)), |()| {
            cosmic::action::app(Message::HideNow)
        })
    }

    fn finish_hide(&mut self) -> app::Task<Message> {
        self.closing = false;
        let Some(id) = self.surface.take() else {
            return Task::none();
        };
        self.hidden_at = Instant::now();
        self.commit_name();
        self.cancel_drag();
        self.resize_cancel();
        self.thumbs.clear();
        // Through the state machine: the generations never go back to 0.
        self.preview_close();
        self.capturing.clear();
        self.preview_hot = None;
        WINDOWS_SHOWN.store(false, Ordering::Relaxed);
        // What drawing it took goes back to the system, once it is gone.
        let trim = Task::perform(tokio::time::sleep(Duration::from_millis(500)), |()| {
            cosmic::action::app(Message::Trim)
        });
        Task::batch([destroy_layer_surface(id), trim])
    }

    /// The menu's size: the one asked for, within what the screen allows.
    fn menu_size(&self) -> iced::Size {
        resize::effective(self.menu_pref, resize::limits(self.room_width, self.room))
    }

    /// Recomputes the metrics for the current menu size.
    fn refresh_metrics(&mut self) {
        let size = self.menu_size();
        self.metrics = metrics::metrics(size.width, size.height);
    }

    /// Where the menu sits on the surface.
    fn menu_rect(&self) -> iced::Rectangle {
        let surface = iced::Size::new(self.room_width, self.room);
        popup::menu_rect(surface, self.on_top, self.menu_size())
    }

    fn app_for_window(&self, window: &Window) -> Option<&AppEntry> {
        app_for_id(&self.apps, &window.app_id)
    }

    fn window_icon(&self, window: &Window) -> icon::Handle {
        match self.app_for_window(window) {
            Some(app) => app.icon.clone(),
            None => self.window_icons.get(&window.app_id).unwrap_or(&self.generic_icon).clone(),
        }
    }

    fn file_icon_of(&self, file: &RecentFile) -> icon::Handle {
        self.file_icons.get(&file.path).unwrap_or(&self.generic_file_icon).clone()
    }

    /// Resolves the icons of the windows no app claims (not on the view).
    fn refill_window_icons(&mut self) {
        let wanted = self
            .windows
            .iter()
            .filter(|w| !w.app_id.is_empty() && app_for_id(&self.apps, &w.app_id).is_none())
            .map(|w| (w.app_id.as_str(), vec![w.app_id.as_str(), "application-default", "application-x-executable"]));
        self.window_icons.refill(wanted, named_icon);
    }

    /// Resolves the icons of the recent files (not on the view).
    fn refill_file_icons(&mut self) {
        let wanted = self.recents.iter().map(|f| (f.path.as_str(), f.icons.iter().map(String::as_str).collect()));
        self.file_icons.refill(wanted, named_icon);
    }

    /// The pinned apps and groups that exist, in their order, with their index in `self.pinned`.
    fn pinned_items(&self) -> Vec<(usize, PinnedItem<'_>)> {
        self.pinned
            .iter()
            .enumerate()
            .filter_map(|(i, id)| {
                if let Some(g) = groups::GroupRef::from_pin(id) {
                    let members = self.group_apps(g);
                    return (self.groups.exists(g) && !members.is_empty())
                        .then(|| (i, PinnedItem::Group(g, members)));
                }
                find_app(&self.apps, id).map(|app| (i, PinnedItem::App(app)))
            })
            .collect()
    }

    /// One `AppKey` per app, at the same index as `self.apps`.
    fn app_keys(&self) -> Vec<groups::AppKey<'_>> {
        self.apps.iter().map(|a| groups::AppKey { id: &a.id, category: a.category }).collect()
    }

    fn group_name(&self, g: groups::GroupRef) -> String {
        match g {
            groups::GroupRef::Category(n) => texts().categories.get(n).map(|n| (*n).to_owned()).unwrap_or_default(),
            groups::GroupRef::User(id) => self.groups.name(id).map(str::to_owned).unwrap_or_default(),
            groups::GroupRef::Loose => texts().ungrouped.to_owned(),
        }
    }

    /// The indices in `self.apps` of the apps of a group, in its order.
    fn group_indices(&self, g: groups::GroupRef) -> Vec<usize> {
        self.groups.members(g, &self.app_keys())
    }

    /// The apps of a group, in its order.
    fn group_apps(&self, g: groups::GroupRef) -> Vec<&AppEntry> {
        self.group_indices(g).into_iter().filter_map(|i| self.apps.get(i)).collect()
    }

    /// The index in `self.apps` of the app a pinned entry names (maybe loosely).
    fn app_index(&self, entry: &str) -> Option<usize> {
        let app = find_app(&self.apps, entry)?;
        self.apps.iter().position(|a| a.id == app.id)
    }

    /// Whether a drop on the center of this app would make a group with it.
    fn on_app(&self, app: &AppEntry) -> bool {
        matches!(self.drag_over(), Some(drag::Slot::OnApp(i)) if self.apps.get(i).is_some_and(|a| a.id == app.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_ids_differ_by_scope() {
        assert_ne!(scoped("pin", "0"), scoped("all", "0"));
        assert_eq!(scoped("all", "a"), scoped("all", "a"));
    }

    #[test]
    fn an_old_generation_is_rejected() {
        assert!(fresh(3, 3));
        assert!(!fresh(2, 3));
    }

    #[test]
    fn the_categories_and_their_keys_match_the_texts() {
        assert_eq!(groups::CATEGORIES, texts().categories.len());
        assert_eq!(groups::CATEGORY_KEYS.len(), groups::CATEGORIES);
    }

    /// `adjust_open` with the capacities of the default size.
    fn adjust(prev_total: usize, total: usize, active: Option<usize>, changed: bool, offset: usize) -> usize {
        let m = metrics::metrics(874.0, 920.0);
        adjust_open(m.open_capacity(prev_total), m.open_capacity(total), total, active, changed, offset)
    }

    #[test]
    fn the_strip_follows_the_active_window_and_never_leaves_a_gap() {
        // 12 windows, 9 a page: the active one at 11 pulls the strip to the end.
        assert_eq!(adjust(12, 12, Some(11), true, 0), 3);
        // Already shown: the offset stays.
        assert_eq!(adjust(12, 12, Some(4), true, 3), 3);
        // Windows closed: the offset comes back in range, none left, no arrows.
        assert_eq!(adjust(12, 12, None, false, 3), 3);
        assert_eq!(adjust(12, 9, None, false, 3), 0);
        assert_eq!(adjust(5, 0, None, false, 5), 0);
        // 10 to 11 windows: capacity falls to 9, the active one (the 10th) stays in view.
        assert_eq!(adjust(10, 11, Some(9), false, 0), 1);
        // Same capacity, same active: nothing moves.
        assert_eq!(adjust(11, 12, Some(9), false, 3), 3);
    }
}
