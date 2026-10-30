use super::events::{accepts_size, window_event};
use super::preview::Event;
use super::*;

impl cosmic::Application for StartMenu {
    type Message = Message;
    // One worker thread, not one per core: there is little to do.
    type Executor = cosmic::SingleThreadExecutor;
    /// Whether to open on the tools.
    type Flags = bool;
    const APP_ID: &'static str = APP_ID;

    fn init(mut core: app::Core, tools: bool) -> (Self, app::Task<Message>) {
        // No frosted glass: libcosmic would blur what is behind the whole
        // surface, the screen all over, not just the menu.
        core.set_auto_blur(Default::default());
        pointer_stats::init_from_env();
        let (menu_pref, persist_size) = resize::initial_pref(
            std::env::var("COSMIC_START_MENU_SIZE").ok().as_deref(),
            config().and_then(|c| c.get::<(u32, u32)>("menu_size").ok()),
        );
        let first = resize::effective(menu_pref, resize::limits(0.0, 0.0));
        let mut menu = Self {
            core,
            surface: None,
            shown_at: Instant::now(),
            room: 0.0,
            room_width: 0.0,
            metrics: metrics::metrics(first.width, first.height),
            menu_anchor: None,
            open_offset: 0,
            open_active: None,
            open_groups: Vec::new(),
            drag: drag::Drag::Idle,
            drag_fresh: false,
            drag_targets: Vec::new(),
            drag_gen: 0,
            group_edit: None,
            hidden_at: Instant::now() - Duration::from_secs(1),
            anim: crate::anim::Spring::new(0.0),
            anim_last: None,
            closing: false,
            apps_signature: None,
            apps: Arc::default(),
            recents: Vec::new(),
            window_icons: Default::default(),
            file_icons: Default::default(),
            generic_icon: named_icon(&["application-default", "application-x-executable"]),
            generic_file_icon: named_icon(&["text-x-generic", "application-octet-stream"]),
            launched: config().and_then(|c| c.get("launched").ok()).unwrap_or_default(),
            windows: Vec::new(),
            windows_tx: None,
            thumbs: HashMap::new(),
            preview_hot: None,
            hover: preview::Hover::default(),
            capturing: HashSet::new(),
            preview_pending: Self::preview_request(),
            pinned: load_pinned(),
            groups: load_groups(),
            pinned_expanded: false,
            all_view: match config().and_then(|c| c.get::<u8>("all_view").ok()) {
                Some(1) => AllView::Grid,
                Some(2) => AllView::List,
                _ => AllView::Category,
            },
            query: String::new(),
            selected: 0,
            page: Page::Home,
            menu: None,
            uninstall: None,
            last_uninstalled: None,
            preview_uninstall: None,
            user: user_name(),
            avatar: avatar(),
            on_top: panel_on_top(),
            menu_pref,
            pref_before: None,
            resize: resize::Resize::Idle,
            persist_size,
            tools: false,
            power_open: false,
        };
        let show = menu.show();
        menu.tools = tools;
        // COSMIC_START_PREVIEW=recommended, power, user, tools-power or a category's
        // number opens there; any other text searches it: to see them
        // without clicking.
        match std::env::var("COSMIC_START_PREVIEW").as_deref() {
            Ok("recommended") => menu.page = Page::Recommended,
            Ok("tools-power") => {
                menu.tools = true;
                menu.power_open = true;
            }
            Ok("power") => menu.menu = Some(Target::Power),
            Ok("user") => menu.menu = Some(Target::User),
            Ok("grid") => menu.all_view = AllView::Grid,
            Ok("list") => menu.all_view = AllView::List,
            Ok(windows) if windows.starts_with("windows:") => {}
            Ok("end") => {
                let end = Task::perform(tokio::time::sleep(Duration::from_millis(400)), |()| {
                    cosmic::action::app(Message::Nothing)
                })
                .chain(cosmic::iced::runtime::widget::operation::snap_to_end(HOME.clone()));
                return (menu, Task::batch([show, end]));
            }
            Ok(query) if query.starts_with("uninstall:") => {
                menu.preview_uninstall = query.strip_prefix("uninstall:").map(str::to_owned);
            }
            Ok(query) if query.starts_with("group:") => {
                menu.menu = query.strip_prefix("group:").and_then(groups::GroupRef::parse).map(Target::Group);
            }
            Ok(query) => match query.parse::<usize>() {
                Ok(category) if category < groups::CATEGORIES => {
                    menu.menu = Some(Target::Group(groups::GroupRef::Category(category)));
                }
                Ok(_) => {}
                Err(_) => menu.query = query.to_owned(),
            },
            Err(_) => {}
        }
        (menu, show)
    }

    fn core(&self) -> &app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut app::Core {
        &mut self.core
    }

    fn style(&self) -> Option<iced::theme::Style> {
        Some(cosmic::applet::style())
    }

    fn update(&mut self, message: Message) -> app::Task<Message> {
        match message {
            Message::Toggle => {
                if self.surface.is_some() {
                    // Pressed again while sliding out: it comes back.
                    if self.closing {
                        self.closing = false;
                        self.anim_last = None;
                        return Task::none();
                    }
                    return self.hide();
                }
                // The click on the Start button that took the keyboard from
                // the menu, and closed it, meant to close it.
                if self.hidden_at.elapsed() < Duration::from_millis(300) {
                    return Task::none();
                }
                return self.show();
            }
            Message::Hide => return self.hide(),
            Message::Tools => {
                self.preview_close();
                self.cancel_drag();
                if self.surface.is_some() && self.tools {
                    return self.hide();
                }
                let show = self.show();
                self.tools = true;
                return show;
            }
            Message::Tool(tool) => {
                if matches!(tool, crate::tools::Tool::PowerMenu) {
                    self.power_open = !self.power_open;
                    return Task::none();
                }
                // Search is this menu itself.
                if matches!(tool, crate::tools::Tool::Search) {
                    self.tools = false;
                    return text_input::focus(SEARCH.clone());
                }
                tool.run();
                return self.hide();
            }
            // Losing the keyboard as it opens is not someone leaving it.
            Message::Unfocused => {
                // The polkit dialog takes the keyboard: it is not leaving.
                if self.uninstall_running() {
                    return Task::none();
                }
                if self.shown_at.elapsed() > Duration::from_millis(400) {
                    return self.hide();
                }
            }
            Message::Escape => {
                if self.uninstall_running() {
                    return Task::none();
                }
                if !self.resize.is_idle() {
                    self.resize_cancel();
                    return Task::none();
                }
                if !self.drag.is_idle() {
                    self.cancel_drag();
                    return Task::none();
                }
                if self.preview_active() {
                    self.preview_close();
                    return Task::none();
                }
                // As on Windows: a menu open closes first, then the search
                // empties, then the page goes back, then it all goes.
                if self.tools {
                    return self.hide();
                } else if self.menu.is_some() {
                    self.close_menu();
                } else if !self.query.is_empty() {
                    self.query.clear();
                    self.selected = 0;
                } else if self.page != Page::Home {
                    self.page = Page::Home;
                } else {
                    return self.hide();
                }
            }
            Message::Focus => return text_input::focus(SEARCH.clone()),
            Message::Focused => {
                if self.surface.is_some() {
                    return text_input::focus(focus_id(self.group_edit.is_some()));
                }
            }
            Message::GroupName(id, text) => self.group_edit = Some((id, text)),
            Message::GroupNameDone => self.commit_name(),
            Message::RenameGroup(id) => return self.edit_name(id),
            Message::DeleteGroup(id) => self.delete_group(id),
            Message::Resized(id, size) => {
                if accepts_size(self.surface, id) {
                    tracing::debug!(?size, "surface size");
                    self.room = size.height;
                    self.room_width = size.width;
                    self.refresh_metrics();
                    return self.preview(Event::Relayout);
                }
            }
            Message::AnimTick => {
                let now = Instant::now();
                let dt = self.anim_last.map_or(0.0, |t| (now - t).as_secs_f32());
                self.anim_last = Some(now);
                if self.closing {
                    self.anim.step(0.0, dt, crate::anim::CLOSE.0, crate::anim::CLOSE.1);
                    if self.anim.pos < 0.03 {
                        return self.finish_hide();
                    }
                } else {
                    self.anim.step(1.0, dt, crate::anim::OPEN.0, crate::anim::OPEN.1);
                    if self.anim.settled(1.0) {
                        self.anim = crate::anim::Spring::new(1.0);
                    }
                }
            }
            Message::HideNow => {
                if self.closing {
                    return self.finish_hide();
                }
            }
            Message::Trim => {
                if self.surface.is_none() {
                    // SAFETY: only hands freed memory back to the system.
                    unsafe { libc::malloc_trim(0) };
                    super::trim::schedule();
                }
            }
            Message::Apps(signature, apps) => {
                self.apps_signature = Some(signature);
                if !self.drag.is_idle() {
                    // The targets point at the old list.
                    self.cancel_drag();
                }
                self.apps = apps;
                // The theme or the apps changed: resolve anew.
                self.window_icons.clear();
                self.file_icons.clear();
                self.refill_window_icons();
                self.refill_file_icons();
                self.regroup_same_windows();
                self.selected = 0;
                self.forget_uninstalled();
                // The icons may have moved, or the key of a group changed.
                let relayout = self.preview(Event::Relayout);
                if let Some(id) = self.preview_uninstall.take() {
                    return Task::batch([self.ask_uninstall(id), relayout]);
                }
                return relayout;
            }
            Message::Recents(recents) => {
                self.recents = recents;
                self.refill_file_icons();
                return self.remeasure();
            }
            Message::Windows(windows) => {
                self.set_windows(windows);
                self.refill_window_icons();
                self.prune_thumbs();
                return Task::batch([self.remeasure(), self.preview(Event::Relayout), self.preview_env()]);
            }
            Message::OpenPage(forward) => {
                self.preview_close();
                self.open_page(forward);
            }
            Message::WindowsReady(tx) => self.windows_tx = Some(tx),
            Message::Thumb(id, handle) => self.preview_thumb(id, handle),
            Message::Preview(ev) => return self.preview(ev),
            Message::PreviewMouse(id, ev) if accepts_size(self.surface, id) => return self.preview(ev),
            Message::PreviewMouse(..) => {}
            Message::PreviewRefresh => self.request_thumbs(),
            Message::Query(query) => {
                self.preview_close();
                self.cancel_drag();
                self.query = query;
                self.selected = 0;
                if !self.uninstall_running() {
                    self.close_menu();
                }
            }
            Message::TypeAhead(text) => {
                if self.tools {
                    return Task::none();
                }
                self.preview_close();
                self.cancel_drag();
                self.query.push_str(&text);
                self.selected = 0;
                return text_input::focus(SEARCH.clone());
            }
            Message::Move(step) => {
                let count = self.hits().len();
                if count > 0 {
                    self.selected =
                        (self.selected as i32 + step).rem_euclid(count as i32) as usize;
                }
            }
            Message::Submit => {
                if let Some(hit) = self.hits().get(self.selected).cloned() {
                    return self.open_hit(hit);
                }
            }
            Message::Page(page) => {
                self.preview_close();
                self.page = page;
                self.close_menu();
            }
            Message::ExpandPinned(expanded) => self.pinned_expanded = expanded,
            Message::NextView => {
                self.all_view = match self.all_view {
                    AllView::Category => AllView::Grid,
                    AllView::Grid => AllView::List,
                    AllView::List => AllView::Category,
                };
                if let Some(config) = config() {
                    _ = config.set("all_view", self.all_view as u8);
                }
            }
            Message::Launch(id) => {
                if let Some(i) = self.apps.iter().position(|a| a.id == id) {
                    return self.launch(i, None);
                }
            }
            Message::LaunchAction(id, exec) => {
                if let Some(i) = self.apps.iter().position(|a| a.id == id) {
                    return self.launch(i, Some(exec));
                }
            }
            Message::Activate(id) => return self.activate(id),
            Message::OpenGroupClick(key) => return self.group_click(&key),
            Message::CloseWindows(ids) => self.close_windows(ids),
            Message::CloseWindow(id) => {
                self.close_menu();
                if let Some(tx) = &self.windows_tx {
                    _ = tx.send(Command::Close(id));
                }
            }
            Message::OpenFile(path) => {
                spawn("xdg-open", &[&path]);
                return self.hide();
            }
            Message::OpenLocation(path) => {
                let folder = Path::new(&path).parent().map(|p| p.to_string_lossy().into_owned());
                spawn("xdg-open", &[&folder.unwrap_or(path)]);
                return self.hide();
            }
            Message::Menu(target) => {
                self.preview_close();
                if self.uninstall_running() {
                    return Task::none();
                }
                self.commit_name();
                if !matches!(target, Some(Target::Uninstall(_))) {
                    self.uninstall = None;
                }
                self.menu = target;
                self.menu_anchor = None;
            }
            Message::DragArm(..) if !self.resize.is_idle() => {}
            Message::DragArm(source, point) => return self.arm_drag(source, point),
            Message::ResizeStart(edge) => {
                self.preview_close();
                self.resize_start(edge);
            }
            Message::ResizeMove(id, point) if accepts_size(self.surface, id) => {
                self.resize_moved(point);
                return self.preview(Event::Relayout);
            }
            Message::ResizeEnd(id) if accepts_size(self.surface, id) => self.resize_end(),
            Message::ResizeMove(..) | Message::ResizeEnd(_) => {}
            Message::ResizeReset => self.resize_reset(),
            Message::DragMove(id, point) if accepts_size(self.surface, id) => self.drag_moved(point),
            Message::DragRelease(id) if accepts_size(self.surface, id) => return self.drag_released(),
            Message::DragMove(..) | Message::DragRelease(_) => {}
            Message::DragTargets(generation, targets) => {
                if fresh(generation, self.drag_gen) && !self.drag.is_idle() {
                    self.drag_targets = targets;
                }
            }
            Message::DragTick => self.drag_tick(),
            Message::MenuFor(target, id) => {
                self.preview_close();
                if self.uninstall_running() {
                    return Task::none();
                }
                return anchor::find(id).map(move |r| cosmic::Action::App(Message::MenuAt(target.clone(), r)));
            }
            Message::MenuAt(target, anchor) => {
                self.preview_close();
                if self.uninstall_running() {
                    return Task::none();
                }
                self.commit_name();
                self.uninstall = None;
                self.menu = Some(target);
                tracing::debug!(?anchor, room_width = self.room_width, room = self.room, "menu anchor");
                self.menu_anchor = anchor;
            }
            Message::AskUninstall(id) => {
                self.preview_close();
                return self.ask_uninstall(id);
            }
            Message::UninstallPlan(id, result) => self.planned(&id, result),
            Message::ToggleDeleteData(on) => {
                if let Some(dialog) = &mut self.uninstall {
                    dialog.delete_data = on;
                }
            }
            Message::ConfirmUninstall => return self.confirm_uninstall(),
            Message::UninstallDone(result) => return self.uninstalled(result),
            Message::TogglePin(id) => {
                if let Some(i) = self.pinned.iter().position(|p| *p == id) {
                    self.pinned.remove(i);
                } else {
                    self.pinned.push(id);
                }
                save_pinned(&self.pinned);
                self.close_menu();
            }
            Message::Spawn(program, args) => {
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                spawn(program, &args);
                // Settings runs once: a second run only tells the first,
                // which the compositor doesn't bring forward. So we do.
                if program == "cosmic-settings"
                    && let Some(window) = self
                        .windows
                        .iter()
                        .find(|w| w.app_id.eq_ignore_ascii_case("com.system76.CosmicSettings"))
                {
                    return self.activate(window.id.clone());
                }
                return self.hide();
            }
            Message::Nothing => {}
            Message::PreviewHot(id) => self.preview_hot = id,
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            dbus_service(),
            windows_subscription(),
            self.resize_subscription(),
            if self.drag.is_idle() {
                Subscription::none()
            } else {
                // Sees the events the buttons took too, in window coordinates.
                event::listen_with(|event, _, window| match event {
                    iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                        pointer_stats::note(pointer_stats::Gesture::Drag, position);
                        Some(Message::DragMove(window, position))
                    }
                    iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                        Some(Message::DragRelease(window))
                    }
                    _ => None,
                })
            },
            if self.drag.dwelling() {
                cosmic::iced::time::every(Duration::from_millis(100)).map(|_| Message::DragTick)
            } else {
                Subscription::none()
            },
            if self.surface.is_some() && (self.closing || !self.anim.settled(1.0)) {
                cosmic::iced::time::every(Duration::from_millis(8)).map(|_| Message::AnimTick)
            } else {
                Subscription::none()
            },
            event::listen_with(window_event),
            self.preview_subscription(),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        widget::text("").into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        if self.surface != Some(id) {
            return widget::text("").into();
        }
        let p = palette(&cosmic::theme::active());
        if self.tools {
            // Where the menu's left edge would be: by the Start button,
            // as near as the menu knows.
            let tools = widget::mouse_area(crate::tools::view(self.power_open, Message::Tool, p))
                .on_press(Message::Nothing);
            let column = widget::container(tools)
                .width(Length::Fixed(self.metrics.tools_width))
                .align_x(Alignment::Start);
            let placed = widget::container(column)
                .padding(MARGIN)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(if self.on_top { Alignment::Start } else { Alignment::End });
            let backdrop = widget::mouse_area(
                widget::container(widget::space::horizontal())
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .on_press(Message::Hide)
            .on_right_press(Message::Hide);
            return iced::widget::stack(vec![backdrop.into(), placed.into()]).into();
        }
        let m = &self.metrics;
        let height = m.height;

        let body: Element<'_, Message> = if !self.query.trim().is_empty() {
            self.results(p)
        } else if self.page == Page::Recommended {
            self.recommended_page(p)
        } else {
            self.home(p)
        };
        let content = widget::column::with_children(vec![
            widget::container(self.search_bar(p))
                .padding([28.0, m.side, 16.0, m.side])
                .into(),
            widget::container(body).height(Length::Fill).into(),
            self.footer(p),
        ])
        .width(Length::Fixed(m.width))
        .height(Length::Fill);
        let mut layers: Vec<Element<'_, Message>> = vec![content.into()];
        if let Some(target) = &self.menu {
            layers.push(self.popup_menu(target, p, self.anchored().is_some()));
        }
        let glass = crate::anim::glass_alpha(self.anim.pos);
        let menu = widget::container(iced::widget::stack(layers))
            .id(widget::Id::new(MENU_BODY))
            .width(Length::Fixed(m.width))
            .height(Length::Fixed(height))
            .class(theme::Container::custom(move |_| widget::container::Style {
                background: Some(Background::Color(Color { a: p.bg.a * glass, ..p.bg })),
                text_color: Some(p.text),
                icon_color: Some(p.text),
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: p.border,
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.45 * glass),
                    offset: iced::Vector::new(0.0, 6.0),
                    blur_radius: MARGIN,
                },
                ..Default::default()
            }));
        // Clicks on the menu stay there; anywhere else, they close it.
        let menu = widget::mouse_area(menu).on_press(Message::Nothing);

        let backdrop = widget::mouse_area(
            widget::container(widget::space::horizontal())
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(Message::Hide)
        .on_right_press(Message::Hide);
        // Once the surface's size is known the menu is set at its place plus
        // the slide; until then, where the layout puts it.
        let placed: Element<'_, Message> = if self.room_width > 0.0 && self.room > 0.0 {
            let at = self.menu_rect();
            let slide = crate::anim::slide_offset(self.anim.pos) * if self.on_top { -1.0 } else { 1.0 };
            iced::widget::pin(menu).x(at.x).y(at.y + slide).into()
        } else {
            widget::container(menu)
                .padding(MARGIN)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(if self.on_top { Alignment::Start } else { Alignment::End })
                .into()
        };
        let backdrop = if self.drag.is_active() {
            backdrop.interaction(iced::mouse::Interaction::Grabbing)
        } else {
            backdrop
        };
        let mut root: Vec<Element<'_, Message>> = vec![backdrop.into(), placed.into(), self.resize_handles(id)];
        root.extend(self.preview_layer(p));
        if let Some((target, anchor)) = self.anchored() {
            root.push(self.anchored_popup(target, anchor, p));
        }
        if let Some((rect, hot)) = unpin_bar_at(&self.drag, &self.drag_targets) {
            root.push(
                iced::widget::pin(group_ui::unpin_bar(texts().unpin_bar, rect.width, hot, p))
                    .x(rect.x)
                    .y(rect.y)
                    .into(),
            );
        }
        root.extend(self.ghost());
        root.extend(self.resize_cover(id));
        iced::widget::stack(root).into()
    }
}
