//! The glue between the menu and the pure resize state machine.

use super::*;

impl StartMenu {
    /// Saves the size asked for (never the clamped one), unless it came from the env.
    fn save_pref(&self) {
        let (width, height) = resize::stored_pair(self.menu_pref);
        let Some(config) = config() else { return };
        if let Err(err) = config.set("menu_size", (width, height)) {
            tracing::warn!("could not save the menu size: {err}");
        }
    }
}

impl StartMenu {
    /// Starts a resize at an edge; the first move gives the point it counts from.
    pub(super) fn resize_start(&mut self, edge: resize::Edge) {
        if self.drag.is_active() || self.uninstall_running() || !self.resize.is_idle() {
            return;
        }
        self.commit_name();
        self.close_menu();
        self.cancel_drag();
        self.pref_before = self.menu_pref;
        self.resize = resize::Resize::arm(edge, self.menu_size());
    }

    pub(super) fn resize_moved(&mut self, point: iced::Point) {
        let limits = resize::limits(self.room_width, self.room);
        let edge = self.resize.edge();
        let (next, size) = self.resize.moved(point, self.on_top, limits);
        self.resize = next;
        let (Some(edge), Some(size)) = (edge, size) else { return };
        let asked = Some(resize::requested(edge, self.pref_before, size));
        if size != self.menu_size() || asked != self.menu_pref {
            self.menu_pref = asked;
            self.refresh_size();
        }
    }

    /// New metrics for the new size, and the open-windows strip kept in view.
    fn refresh_size(&mut self) {
        let total = self.open_groups.len();
        let prev_cap = self.metrics.open_capacity(total);
        self.refresh_metrics();
        let cap = self.metrics.open_capacity(total);
        let active = self.active_group_index();
        self.open_offset = adjust_open(prev_cap, cap, total, active, false, self.open_offset);
    }

    /// Ends the gesture; a second call (press and release in one batch) does nothing.
    pub(super) fn resize_end(&mut self) {
        let (next, was) = self.resize.finish();
        self.resize = next;
        if was && resize::should_save(self.pref_before, self.menu_pref, self.persist_size) {
            self.save_pref();
        }
    }

    /// Escape or hiding: back to the size before the gesture.
    pub(super) fn resize_cancel(&mut self) {
        if self.resize.is_idle() {
            return;
        }
        self.menu_pref = self.pref_before;
        self.resize = resize::Resize::Idle;
        self.refresh_size();
    }

    /// Double click on an edge: back to the default size.
    pub(super) fn resize_reset(&mut self) {
        let before = self.menu_pref;
        self.menu_pref = None;
        self.resize = resize::Resize::Idle;
        self.refresh_size();
        // ConfigSet cannot delete: the default maximum is stored instead.
        if resize::should_save(before, None, self.persist_size) {
            self.save_pref();
        }
    }

    pub(super) fn resize_subscription(&self) -> Subscription<Message> {
        if self.resize.is_idle() {
            return Subscription::none();
        }
        // Sees the events the handles took too, in window coordinates.
        event::listen_with(|event, _, window| match event {
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                pointer_stats::note(pointer_stats::Gesture::Resize, position);
                Some(Message::ResizeMove(window, position))
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                Some(Message::ResizeEnd(window))
            }
            _ => None,
        })
    }
}

impl StartMenu {
    /// The five handles, always a stack of the same shape (empty before the
    /// surface has a size) so the widget tree keeps its state.
    pub(super) fn resize_handles(&self, id: window::Id) -> Element<'_, Message> {
        let mut layers: Vec<Element<'_, Message>> = Vec::new();
        if self.room_width > 0.0 && self.room > 0.0 {
            for (edge, r) in resize::handles(self.menu_rect(), self.on_top) {
                layers.push(self.handle(edge, r, id));
            }
        }
        iced::widget::stack(layers).into()
    }

    fn handle(&self, edge: resize::Edge, r: iced::Rectangle, id: window::Id) -> Element<'_, Message> {
        let body = widget::container(widget::space::horizontal())
            .width(Length::Fixed(r.width))
            .height(Length::Fixed(r.height));
        let mut area = widget::mouse_area(body).on_release(Message::ResizeEnd(id));
        // During an app drag the layer stays, but inert.
        if self.drag.is_idle() {
            area = area
                .interaction(resize::interaction(edge, self.on_top))
                .on_press(Message::ResizeStart(edge))
                .on_double_press(Message::ResizeReset);
        }
        iced::widget::pin(area).x(r.x).y(r.y).into()
    }

    /// While resizing, the whole surface keeps the resize cursor.
    pub(super) fn resize_cover(&self, id: window::Id) -> Option<Element<'_, Message>> {
        let edge = self.resize.edge()?;
        let body = widget::container(widget::space::horizontal())
            .width(Length::Fill)
            .height(Length::Fill);
        Some(
            widget::mouse_area(body)
                .interaction(resize::interaction(edge, self.on_top))
                .on_release(Message::ResizeEnd(id))
                .into(),
        )
    }
}
