use super::*;

/// The icons of the apps, for a folder to draw.
pub(super) fn icons_of(apps: &[&AppEntry]) -> Vec<widget::icon::Handle> {
    apps.iter().map(|a| a.icon.clone()).collect()
}

impl StartMenu {
    // The parts.

    /// A section's title, with its button at the right ("Show all ⌄").
    pub(super) fn header<'a>(
        title: &'a str,
        action: Option<(String, &'static str, Message)>,
        p: Palette,
    ) -> Element<'a, Message> {
        let mut row = widget::row::with_capacity(3)
            .align_y(Alignment::Center)
            .push(widget::text::heading(title))
            .push(widget::space::horizontal());
        if let Some((label, arrow, message)) = action {
            let inner = widget::row::with_children(vec![
                widget::text::caption(label).into(),
                widget::icon::from_name(arrow).size(12).into(),
            ])
            .spacing(6)
            .align_y(Alignment::Center);
            row = row.push(
                button::custom(inner)
                    .padding([4, 10])
                    .class(pill_class(p))
                    .on_press(message),
            );
        }
        widget::container(row).height(Length::Fixed(28.0)).into()
    }

    pub(super) fn back_header<'a>(title: &'a str, p: Palette) -> Element<'a, Message> {
        Self::header(
            title,
            Some((texts().back.into(), "go-previous-symbolic", Message::Page(Page::Home))),
            p,
        )
    }

    /// An app as the grid has it: the icon, and the name under it.
    fn app_tile<'a>(&'a self, app: &'a AppEntry, draggable_tile: bool, p: Palette) -> Element<'a, Message> {
        let content = widget::column::with_children(vec![
            widget::icon(app.icon_at(px(36.0))).size(px(36.0)).into(),
            widget::text::caption(ellipsize(&app.name, self.metrics.tile_chars))
                .width(Length::Fill)
                .align_x(Alignment::Center)
                .wrapping(Wrapping::None)
                .into(),
        ])
        .spacing(8)
        .align_x(Alignment::Center)
        .width(Length::Fill);
        let id = scoped("all", &app.id);
        let tile = widget::mouse_area(
            draggable(
                button::custom(content).padding([14, 0, 8, 0]).width(Length::Fill).class(flat_class(p, false)),
                draggable_tile.then(|| drag::Source::App(app.id.clone())),
                Message::Launch(app.id.clone()),
                self.drag.is_active(),
            ),
        )
        .on_right_press(Message::MenuFor(Target::App(app.id.clone()), id.clone()));
        let dim = draggable_tile && matches!(dragged(&self.drag), Some(drag::Source::App(id)) if *id == app.id);
        let tile = iced::widget::stack(vec![
            tile.into(),
            tile_overlay(dim, false, p),
            group_ui::drop_ring(self.on_app(app), 8.0, p),
        ]);
        widget::container(tile).width(Length::Fill).id(id).into()
    }

    /// A pinned app: the icon alone, its name as a tip.
    fn pinned_tile<'a>(&'a self, index: usize, app: &'a AppEntry, over: Option<drag::Slot>, p: Palette) -> Element<'a, Message> {
        let id = scoped("pin", &index.to_string());
        let icon = widget::container(widget::icon(app.icon_at(px(36.0))).size(px(36.0)))
            .width(Length::Fill)
            .center_x(Length::Fill);
        let tile = widget::mouse_area(
            draggable(
                button::custom(icon).padding([14, 0]).width(Length::Fill).class(flat_class(p, false)),
                Some(drag::Source::Pinned(index)),
                Message::Launch(app.id.clone()),
                self.drag.is_active(),
            ),
        )
        .on_right_press(Message::MenuFor(Target::App(app.id.clone()), id.clone()));
        let dim = dragged(&self.drag) == Some(&drag::Source::Pinned(index));
        let tile = iced::widget::stack(vec![
            tile.into(),
            tile_overlay(dim, drop_before(over, index), p),
            group_ui::drop_ring(self.on_app(app), 8.0, p),
        ]);
        let tip = widget::tooltip(tile, widget::text::caption(app.name.as_str()), widget::tooltip::Position::Bottom)
            .delay(Duration::from_millis(500));
        widget::container(tip).width(Length::Fill).id(id).into()
    }

    /// A pinned group: its folder as a tile, the name as a tip.
    fn pinned_group<'a>(&'a self, index: usize, g: groups::GroupRef, members: &[&'a AppEntry], p: Palette) -> Element<'a, Message> {
        let over = self.drag_over();
        group_ui::pinned_group_tile(
            index,
            g,
            self.group_name(g),
            &icons_of(members),
            drop_before(over, index),
            over == Some(drag::Slot::Into(g)),
            dragged(&self.drag) == Some(&drag::Source::Pinned(index)),
            self.drag.is_active(),
            p,
        )
    }

    pub(super) fn pinned_grid<'a>(&'a self, items: &[(usize, PinnedItem<'a>)], p: Palette) -> Element<'a, Message> {
        let over = self.drag_over();
        let m = &self.metrics;
        let mut grid = widget::column::with_capacity(items.len() / m.pinned_cols + 1).spacing(12);
        for chunk in items.chunks(m.pinned_cols) {
            let mut row = widget::row::with_capacity(m.pinned_cols).spacing(12);
            for (index, item) in chunk {
                row = row.push(match item {
                    PinnedItem::App(app) => self.pinned_tile(*index, app, over, p),
                    PinnedItem::Group(g, members) => self.pinned_group(*index, *g, members, p),
                });
            }
            for _ in chunk.len()..m.pinned_cols {
                row = row.push(widget::space::horizontal().width(Length::Fill));
            }
            grid = grid.push(row);
        }
        grid.into()
    }

    /// An open app: the icon alone, with the count of its windows if several.
    fn group_icon<'a>(&'a self, group: &'a open_groups::OpenGroup, p: Palette) -> Element<'a, Message> {
        let id = scoped("open", &group.key);
        let Some(first) = group.windows.first().and_then(|&i| self.windows.get(i)) else {
            return widget::space::horizontal().width(Length::Fixed(open_strip::CELL)).into();
        };
        let active = group.is_active(|i| self.windows.get(i).is_some_and(|w| w.active));
        let icon = widget::container(widget::icon(self.window_icon(first)).size(px(36.0)))
            .width(Length::Fill)
            .center_x(Length::Fill);
        let button = button::custom(icon).padding([10, 0]).width(Length::Fill).class(flat_class(p, active));
        let tile = widget::mouse_area(draggable(
            button,
            self.app_for_window(first).map(|a| drag::Source::App(a.id.clone())),
            Message::OpenGroupClick(group.key.clone()),
            self.drag.is_active(),
        ))
        .on_enter(Message::Preview(preview::Event::EnterIcon(group.key.clone())))
        .on_exit(Message::Preview(preview::Event::LeaveIcon(group.key.clone())))
        .on_right_press(Message::MenuFor(Target::OpenGroup(group.key.clone()), id.clone()));
        let mut layers = vec![tile.into()];
        if let Some(label) = open_groups::count_label(group.len()) {
            let corner = widget::container(Self::count_badge(label, p))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::End)
                .align_y(Alignment::End)
                .padding([6, 6]);
            layers.push(corner.into());
        }
        widget::container(iced::widget::stack(layers)).width(Length::Fixed(open_strip::CELL)).id(id).into()
    }

    /// The count of windows: a small pill in the accent.
    fn count_badge<'a>(label: String, p: Palette) -> Element<'a, Message> {
        widget::container(widget::text::caption(label).size(11).class(theme::Text::Color(Color::WHITE)))
            .padding([0, 5])
            .class(theme::Container::custom(move |_| widget::container::Style {
                background: Some(Background::Color(p.accent)),
                border: iced::Border { radius: 999.0.into(), ..Default::default() },
                ..Default::default()
            }))
            .into()
    }

    /// An arrow of the open strip; off at the end, not gone.
    fn open_arrow<'a>(forward: bool, enabled: bool, p: Palette) -> Element<'a, Message> {
        let name = if forward { "go-next-symbolic" } else { "go-previous-symbolic" };
        button::custom(widget::container(widget::icon::icon(symbolic(name)).size(16)).center_x(Length::Fill))
            .padding([10, 0])
            .width(Length::Fixed(open_strip::ARROW))
            .class(flat_class(p, false))
            .on_press_maybe(enabled.then_some(Message::OpenPage(forward)))
            .into()
    }

    /// The open windows: a rounded gray band, a title and one row of icons,
    /// with arrows to the others if they do not all fit.
    pub(super) fn open_strip(&self, p: Palette) -> Element<'_, Message> {
        let total = self.open_groups.len();
        let cap = self.metrics.open_capacity(total);
        let offset = open_strip::clamp(total, cap, self.open_offset);
        let mut grid = widget::row::with_capacity(cap + 2).spacing(open_strip::GAP).align_y(Alignment::Center);
        let arrows = total > cap;
        if arrows {
            grid = grid.push(Self::open_arrow(false, open_strip::can_prev(offset), p));
        }
        for group in &self.open_groups[open_strip::visible_range(total, cap, offset)] {
            grid = grid.push(self.group_icon(group, p));
        }
        if arrows {
            grid = grid.push(Self::open_arrow(true, open_strip::can_next(total, cap, offset), p));
        }
        let content = widget::column::with_capacity(2)
            .push(widget::text::heading(texts().open_windows))
            .push(grid)
            .spacing(10);
        widget::container(content)
            .padding(12)
            .width(Length::Fill)
            .class(theme::Container::custom(move |_| widget::container::Style {
                background: Some(Background::Color(p.card)),
                border: iced::Border {
                    radius: 8.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }))
            .into()
    }

    pub(super) fn tile_grid<'a>(&'a self, apps: &[&'a AppEntry], columns: usize, draggable_tiles: bool, p: Palette) -> Element<'a, Message> {
        let mut grid = widget::column::with_capacity(apps.len() / columns + 1).spacing(12);
        for chunk in apps.chunks(columns) {
            let mut row = widget::row::with_capacity(columns).spacing(12);
            for app in chunk {
                row = row.push(self.app_tile(app, draggable_tiles, p));
            }
            for _ in chunk.len()..columns {
                row = row.push(widget::space::horizontal().width(Length::Fill));
            }
            grid = grid.push(row);
        }
        grid.into()
    }

    /// A row of the recommended: icon, a title, a line under, a pill if given.
    fn card<'a>(
        icon: icon::Handle,
        title: String,
        subtitle: String,
        selected: bool,
        on_press: Message,
        on_right: Message,
        id: widget::Id,
        source: Option<drag::Source>,
        dragging: bool,
        badge: Option<&'static str>,
        chars: (usize, usize),
        p: Palette,
    ) -> Element<'a, Message> {
        let text = widget::column::with_children(vec![
            widget::text::body(ellipsize(&title, chars.0)).size(13).wrapping(Wrapping::None).into(),
            widget::text::caption(ellipsize(&subtitle, chars.1))
                .class(theme::Text::Color(p.dim))
                .wrapping(Wrapping::None)
                .into(),
        ])
        .spacing(1);
        let mut content = widget::row::with_children(vec![
            widget::icon(icon).size(px(36.0)).into(),
            widget::container(text).width(Length::Fill).clip(true).into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center);
        if let Some(badge) = badge {
            content = content.push(Self::badge(badge, p));
        }
        let card = widget::mouse_area(
            draggable(
                button::custom(content).padding([8, 10]).width(Length::Fill).class(flat_class(p, selected)),
                source,
                on_press,
                dragging,
            ),
        )
        .on_right_press(on_right);
        widget::container(card).width(Length::Fill).id(id).into()
    }

    /// The "New" pill: the accent, white text on it.
    fn badge<'a>(label: &'static str, p: Palette) -> Element<'a, Message> {
        widget::container(widget::text::caption(label).size(11).class(theme::Text::Color(Color::WHITE)))
            .padding([1, 8])
            .class(theme::Container::custom(move |_| widget::container::Style {
                background: Some(Background::Color(p.accent)),
                border: iced::Border { radius: 999.0.into(), ..Default::default() },
                ..Default::default()
            }))
            .into()
    }

    pub(super) fn card_grid<'a>(cards: Vec<Element<'a, Message>>, cols: usize) -> Element<'a, Message> {
        let mut cards = cards.into_iter().peekable();
        let mut grid = widget::column::with_capacity(4).spacing(8);
        while cards.peek().is_some() {
            let mut row = widget::row::with_capacity(cols).spacing(16);
            for _ in 0..cols {
                row = row.push(match cards.next() {
                    Some(card) => card,
                    None => widget::space::horizontal().width(Length::Fill).into(),
                });
            }
            grid = grid.push(row);
        }
        grid.into()
    }

    pub(super) fn pick_card<'a>(&'a self, pick: Pick<'a>, p: Palette) -> Element<'a, Message> {
        let chars = (self.metrics.card_title_chars, self.metrics.card_sub_chars);
        match pick {
            Pick::App(app, subtitle, new) => Self::card(
                app.icon_at(px(36.0)),
                app.name.clone(),
                subtitle,
                false,
                Message::Launch(app.id.clone()),
                Message::MenuFor(Target::App(app.id.clone()), scoped("rec", &app.id)),
                scoped("rec", &app.id),
                Some(drag::Source::App(app.id.clone())),
                self.drag.is_active(),
                new.then_some(texts().new_badge),
                chars,
                p,
            ),
            Pick::File(file) => Self::card(
                self.file_icon_of(file),
                file.name.clone(),
                ago(file.when),
                false,
                Message::OpenFile(file.path.clone()),
                Message::MenuFor(Target::File(file.path.clone()), scoped("rec", &file.path)),
                scoped("rec", &file.path),
                None,
                false,
                None,
                chars,
                p,
            ),
        }
    }
}

/// Cut to `max` characters with "…", as Windows does names too long.
pub(super) fn ellipsize(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let cut: String = text.chars().take(max - 1).collect();
        format!("{}…", cut.trim_end())
    }
}
