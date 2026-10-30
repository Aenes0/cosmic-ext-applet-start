use super::*;

/// A search result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Hit {
    App(usize),
    Window(String),
    File(usize),
}

impl StartMenu {
    pub(super) fn open_hit(&mut self, hit: Hit) -> app::Task<Message> {
        match hit {
            Hit::App(i) => self.launch(i, None),
            Hit::Window(id) => self.activate(id),
            Hit::File(i) => {
                let path = self.recents[i].path.clone();
                cosmic::Application::update(self, Message::OpenFile(path))
            }
        }
    }

    pub(super) fn hits(&self) -> Vec<Hit> {
        let query = fold(self.query.trim());
        if query.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(u8, &str, Hit)> = self
            .apps
            .iter()
            .enumerate()
            .filter_map(|(i, app)| {
                let score = if app.folded_name.starts_with(&query) {
                    0
                } else if app.folded_name.split([' ', '-', '_', '.']).any(|w| w.starts_with(&query)) {
                    1
                } else if app.folded_name.contains(&query) {
                    2
                } else if app.haystack.contains(&query) {
                    3
                } else {
                    return None;
                };
                Some((score, app.name.as_str(), Hit::App(i)))
            })
            .collect();
        // The closest first: "Configurações" before "Configurações do NVIDIA".
        scored.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.chars().count().cmp(&b.1.chars().count()))
                .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
        });
        let mut hits: Vec<Hit> = scored.into_iter().map(|(_, _, hit)| hit).take(10).collect();
        hits.extend(
            self.windows
                .iter()
                .filter(|w| fold(&w.title).contains(&query) || fold(&w.app_id).contains(&query))
                .map(|w| Hit::Window(w.id.clone())),
        );
        hits.extend(
            self.recents
                .iter()
                .enumerate()
                .filter(|(_, f)| fold(&f.name).contains(&query))
                .map(|(i, _)| Hit::File(i))
                .take(8),
        );
        hits
    }

    pub(super) fn search_bar(&self, p: Palette) -> Element<'_, Message> {
        let style = move |focused: bool| widget::text_input::Appearance {
            background: Background::Color(p.field),
            border_radius: 18.0.into(),
            border_offset: None,
            border_width: 1.0,
            border_color: if focused { p.accent } else { p.border },
            label_color: p.text,
            placeholder_color: p.dim,
            selected_text_color: Color::WHITE,
            icon_color: Some(p.dim),
            text_color: Some(p.text),
            selected_fill: p.accent,
        };
        widget::search_input(texts().start_search, &self.query)
            .id(SEARCH.clone())
            .size(14)
            .padding([8, 12])
            .on_input(Message::Query)
            .on_submit(|_| Message::Submit)
            .width(Length::Fill)
            .style(theme::TextInput::Custom {
                active: Box::new(move |_| style(false)),
                error: Box::new(move |_| style(false)),
                hovered: Box::new(move |_| style(false)),
                focused: Box::new(move |_| style(true)),
                disabled: Box::new(move |_| style(false)),
            })
            .into()
    }

    /// Results at the left, the chosen one at the right with what can be
    /// done with it, as Windows' search has them. On a narrow menu, only
    /// the list.
    pub(super) fn results(&self, p: Palette) -> Element<'_, Message> {
        let t = texts();
        let hits = self.hits();
        let Some(chosen) = hits.get(self.selected.min(hits.len().saturating_sub(1))) else {
            return widget::container(widget::text::body(t.no_results).class(theme::Text::Color(p.dim)))
                .padding([40, 0])
                .center_x(Length::Fill)
                .into();
        };
        let mut list = widget::column::with_capacity(hits.len() + 4).spacing(2);
        let mut section = "";
        for (i, hit) in hits.iter().enumerate() {
            let Some((heading, row)) = self.hit_row(i, hit, p) else { continue };
            let heading = if i == 0 { t.best_match } else { heading };
            if heading != section {
                section = heading;
                list = list.push(
                    widget::container(widget::text::heading(heading)).padding([if i == 0 { 0 } else { 10 }, 10, 4, 10]),
                );
            }
            list = list.push(row);
        }
        let pad = [4.0, self.metrics.side, 20.0, self.metrics.side];
        if !self.metrics.detail_panel {
            return widget::container(widget::scrollable(list).width(Length::Fill).height(Length::Fill))
                .padding(pad)
                .into();
        }
        widget::row::with_children(vec![
            widget::scrollable(list).width(Length::FillPortion(6)).height(Length::Fill).into(),
            self.detail_panel(chosen, p),
        ])
        .spacing(16)
        .padding(pad)
        .into()
    }

    /// One line of the results, with the right click of its context menu.
    fn hit_row(&self, i: usize, hit: &Hit, p: Palette) -> Option<(&'static str, Element<'_, Message>)> {
        let t = texts();
        let (heading, icon, title, message, target, key) = match hit {
            Hit::App(index) => {
                let app = &self.apps[*index];
                (t.apps, app.icon.clone(), app.name.clone(), Message::Launch(app.id.clone()), Target::App(app.id.clone()), app.id.clone())
            }
            Hit::Window(id) => {
                let window = self.windows.iter().find(|w| w.id == *id)?;
                (t.open_windows, self.window_icon(window), window.title.clone(), Message::Activate(id.clone()), Target::Window(id.clone()), id.clone())
            }
            Hit::File(index) => {
                let file = &self.recents[*index];
                (t.documents, self.file_icon_of(file), file.name.clone(), Message::OpenFile(file.path.clone()), Target::File(file.path.clone()), file.path.clone())
            }
        };
        let size = px(if i == 0 { 32.0 } else { 20.0 });
        let content = widget::row::with_children(vec![
            widget::icon(icon).size(size).into(),
            widget::text::body(title).wrapping(Wrapping::None).width(Length::Fill).into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center);
        let id = scoped("hit", &key);
        let button = button::custom(widget::container(content).clip(true))
            .padding(if i == 0 { [10, 10] } else { [7, 10] })
            .width(Length::Fill)
            .class(flat_class(p, i == self.selected))
            .on_press(message);
        let row = widget::mouse_area(button).on_right_press(Message::MenuFor(target, id.clone()));
        Some((heading, widget::container(row).width(Length::Fill).id(id).into()))
    }

    /// The chosen one, large, and its actions.
    fn detail_panel(&self, chosen: &Hit, p: Palette) -> Element<'_, Message> {
        let t = texts();
        let (icon, title, kind, actions): (icon::Handle, String, String, Vec<(String, Message)>) = match chosen {
            Hit::App(index) => {
                let app = &self.apps[*index];
                let pinned = self.pinned.iter().any(|id| *id == app.id);
                let mut actions = vec![(t.open_app.to_owned(), Message::Launch(app.id.clone()))];
                actions.extend(app.actions.iter().map(|(name, exec)| {
                    (name.clone(), Message::LaunchAction(app.id.clone(), exec.clone()))
                }));
                actions.push((
                    if pinned { t.unpin_start } else { t.pin_start }.to_owned(),
                    Message::TogglePin(app.id.clone()),
                ));
                if uninstall_ui::uninstall_visible(app) {
                    actions.push((t.uninstall.to_owned(), Message::AskUninstall(app.id.clone())));
                }
                (app.icon.clone(), app.name.clone(), t.app_kind.to_owned(), actions)
            }
            Hit::Window(id) => {
                let window = self.windows.iter().find(|w| w.id == *id);
                (
                    window.map(|w| self.window_icon(w)).unwrap_or_else(|| self.generic_icon.clone()),
                    window.map(|w| w.title.clone()).unwrap_or_default(),
                    t.window_kind.to_owned(),
                    vec![
                        (t.switch_to.to_owned(), Message::Activate(id.clone())),
                        (t.close_window.to_owned(), Message::CloseWindow(id.clone())),
                    ],
                )
            }
            Hit::File(index) => {
                let file = &self.recents[*index];
                (
                    self.file_icon_of(file),
                    file.name.clone(),
                    format!("{} · {}", t.file_kind, ago(file.when)),
                    vec![
                        (t.open_app.to_owned(), Message::OpenFile(file.path.clone())),
                        (t.open_location.to_owned(), Message::OpenLocation(file.path.clone())),
                    ],
                )
            }
        };
        let mut detail = widget::column::with_capacity(actions.len() + 4)
            .push(widget::icon(icon).size(px(64.0)))
            .push(
                widget::text::title4(title)
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
            )
            .push(widget::text::caption(kind).class(theme::Text::Color(p.dim)))
            .push(widget::space::vertical().height(Length::Fixed(8.0)))
            .spacing(6)
            .padding([24, 12])
            .align_x(Alignment::Center);
        for (label, message) in actions {
            let row = widget::row::with_children(vec![
                widget::icon::from_name("go-next-symbolic").size(12).into(),
                widget::text::body(label).into(),
            ])
            .spacing(12)
            .align_y(Alignment::Center);
            detail = detail.push(
                button::custom(row)
                    .padding([8, 12])
                    .width(Length::Fill)
                    .class(flat_class(p, false))
                    .on_press(message),
            );
        }
        widget::container(detail)
            .width(Length::FillPortion(5))
            .height(Length::Fill)
            .class(theme::Container::custom(move |_| widget::container::Style {
                background: Some(Background::Color(p.card)),
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: p.border,
                },
                ..Default::default()
            }))
            .into()
    }
}
