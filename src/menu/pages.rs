use super::*;

/// The label of the view button: short on a narrow menu.
fn view_label(view_as: &str, view_name: &str, compact: bool) -> String {
    if compact {
        view_name.to_owned()
    } else {
        format!("{view_as}: {view_name}")
    }
}

/// One of the recommended: an app, or a file, with what to say under it.
pub(super) enum Pick<'a> {
    /// With the subtitle and whether it is new.
    App(&'a AppEntry, String, bool),
    File(&'a RecentFile),
}

impl StartMenu {
    /// As Windows recommends: the apps just installed or opened, the files
    /// just changed, the latest first.
    fn picks(&self) -> Vec<(i64, Pick<'_>)> {
        let t = texts();
        let added: Vec<i64> = self.apps.iter().map(|a| a.added).collect();
        let mut seen = std::collections::HashSet::new();
        // The four latest installed or updated come first, new or not.
        let head: Vec<Pick<'_>> = recent::newest_apps(&added, now(), 4)
            .into_iter()
            .map(|(i, new)| {
                let app = &self.apps[i];
                seen.insert(app.id.as_str());
                Pick::App(app, format!("{} · {}", t.installed, ago(app.added)), new)
            })
            .collect();
        let mut rest: Vec<(i64, Pick<'_>)> = Vec::new();
        for (id, when) in &self.launched {
            if let Some(app) = self.apps.iter().find(|a| a.id == *id)
                && seen.insert(app.id.as_str())
            {
                rest.push((*when, Pick::App(app, ago(*when), false)));
            }
        }
        rest.extend(self.recents.iter().map(|f| (f.when, Pick::File(f))));
        put_first(head, rest)
    }

    pub(super) fn home(&self, p: Palette) -> Element<'_, Message> {
        let t = texts();
        let m = &self.metrics;
        let mut sections: Vec<Element<'_, Message>> = Vec::new();

        // The open windows, as a strip of icons above the pinned.
        if !self.windows.is_empty() {
            sections.push(self.open_strip(p));
            sections.push(widget::space::vertical().height(Length::Fixed(12.0)).into());
        }

        // Pinned: two rows, all of them on "Show all".
        let pinned = self.pinned_items();
        let more = pinned.len() > m.pinned_shown;
        sections.push(Self::header(
            t.pinned,
            more.then(|| {
                if self.pinned_expanded {
                    (t.show_less.into(), "go-up-symbolic", Message::ExpandPinned(false))
                } else {
                    (t.show_all.into(), "go-down-symbolic", Message::ExpandPinned(true))
                }
            }),
            p,
        ));
        if pinned.is_empty() {
            sections.push(
                widget::container(
                    widget::container(widget::text::body(t.pin_hint).class(theme::Text::Color(p.dim)))
                        .padding([16, 0])
                        .center_x(Length::Fill),
                )
                .width(Length::Fill)
                .id(widget::Id::new(PINNED_AREA))
                .into(),
            );
        } else {
            let shown = if self.pinned_expanded { pinned.len() } else { m.pinned_shown };
            let pinned: Vec<(usize, PinnedItem<'_>)> = pinned.into_iter().take(shown).collect();
            sections.push(
                widget::container(self.pinned_grid(&pinned, p))
                    .width(Length::Fill)
                    .id(widget::Id::new(PINNED_AREA))
                    .class(self.area_class(p))
                    .into(),
            );
        }

        // Recommended.
        let picks = self.picks();
        sections.push(widget::space::vertical().height(Length::Fixed(12.0)).into());
        sections.push(Self::header(
            t.recommended,
            (picks.len() > m.card_cols * m.card_rows).then(|| {
                (t.show_all.into(), "go-next-symbolic", Message::Page(Page::Recommended))
            }),
            p,
        ));
        if picks.is_empty() {
            sections.push(
                widget::container(widget::text::body(t.no_recommended).class(theme::Text::Color(p.dim)))
                    .padding([16, 0])
                    .center_x(Length::Fill)
                    .into(),
            );
        } else {
            let cards = picks
                .into_iter()
                .take(m.card_cols * m.card_rows)
                .map(|(_, pick)| self.pick_card(pick, p))
                .collect();
            sections.push(Self::card_grid(cards, self.metrics.card_cols));
        }

        // All, by category, in a grid or in a list.
        sections.push(widget::space::vertical().height(Length::Fixed(12.0)).into());
        let view_name = t.views[self.all_view as usize];
        sections.push(Self::header(
            t.all,
            Some((view_label(t.view_as, view_name, m.compact_headers), "go-down-symbolic", Message::NextView)),
            p,
        ));
        sections.push(match self.all_view {
            AllView::Category => self.categories(p),
            AllView::Grid => self.all_grid(p),
            AllView::List => self.all_list(p),
        });

        widget::scrollable(
            widget::column::with_children(sections)
                .spacing(8)
                .padding([4.0, m.side, 24.0, m.side]),
        )
        .id(HOME.clone())
        .height(Length::Fill)
        .into()
    }

    pub(super) fn recommended_page(&self, p: Palette) -> Element<'_, Message> {
        let m = &self.metrics;
        let cards = self
            .picks()
            .into_iter()
            .take(30)
            .map(|(_, pick)| self.pick_card(pick, p))
            .collect();
        widget::column::with_children(vec![
            widget::container(Self::back_header(texts().recommended, p))
                .padding([4.0, m.side, 8.0, m.side])
                .into(),
            widget::scrollable(widget::container(Self::card_grid(cards, self.metrics.card_cols)).padding([0.0, m.side, 24.0, m.side]))
                .height(Length::Fill)
                .into(),
        ])
        .into()
    }

    /// The apps by group, as folders: the icons in a card, the name
    /// under it; a click opens it.
    fn categories(&self, p: Palette) -> Element<'_, Message> {
        let m = &self.metrics;
        let over = self.drag_over();
        let folders: Vec<Element<'_, Message>> = self
            .groups
            .visible(&self.app_keys())
            .into_iter()
            .map(|(g, _)| {
                let icons = icons_of(&self.group_apps(g));
                let ring = over == Some(drag::Slot::Into(g));
                group_ui::folder_card(g, self.group_name(g), &icons, ring, self.drag.is_active(), p)
            })
            .collect();
        let mut folders = folders.into_iter().peekable();
        let mut grid = widget::column::with_capacity(3).spacing(16);
        while folders.peek().is_some() {
            let mut row = widget::row::with_capacity(m.category_cols).spacing(16);
            for _ in 0..m.category_cols {
                row = row.push(match folders.next() {
                    Some(folder) => folder,
                    None => widget::space::horizontal().width(Length::Fill).into(),
                });
            }
            grid = grid.push(row);
        }
        grid.into()
    }

    fn letter(app: &AppEntry) -> char {
        app.folded_name
            .chars()
            .next()
            .map_or('#', |c| if c.is_alphabetic() { c.to_ascii_uppercase() } else { '#' })
    }

    fn all_grid(&self, p: Palette) -> Element<'_, Message> {
        let m = &self.metrics;
        let mut column = widget::column::with_capacity(60).spacing(4);
        let apps: Vec<&AppEntry> = self.apps.iter().collect();
        for group in apps.chunk_by(|a, b| Self::letter(a) == Self::letter(b)) {
            column = column.push(
                widget::container(widget::text::heading(Self::letter(group[0]).to_string()))
                    .padding([8, 8, 0, 8]),
            );
            column = column.push(self.tile_grid(group, m.grid_cols, true, p));
        }
        column.into()
    }

    fn all_list(&self, p: Palette) -> Element<'_, Message> {
        let mut column = widget::column::with_capacity(self.apps.len() + 30).spacing(2);
        let mut letter = None;
        for app in self.apps.iter() {
            let first = Self::letter(app);
            if letter != Some(first) {
                letter = Some(first);
                column = column.push(
                    widget::container(widget::text::heading(first.to_string())).padding([10, 8, 4, 8]),
                );
            }
            column = column.push(self.app_row(app, false, p));
        }
        column.into()
    }

    fn app_row<'a>(&'a self, app: &'a AppEntry, selected: bool, p: Palette) -> Element<'a, Message> {
        let content = widget::row::with_children(vec![
            widget::icon(app.icon_at(px(24.0))).size(px(24.0)).into(),
            widget::text::body(app.name.as_str()).width(Length::Fill).into(),
        ])
        .spacing(14)
        .align_y(Alignment::Center);
        let id = scoped("all", &app.id);
        let row = widget::mouse_area(
            draggable(
                button::custom(content).padding([7, 10]).width(Length::Fill).class(flat_class(p, selected)),
                Some(drag::Source::App(app.id.clone())),
                Message::Launch(app.id.clone()),
                self.drag.is_active(),
            ),
        )
        .on_right_press(Message::MenuFor(Target::App(app.id.clone()), id.clone()));
        let row = iced::widget::stack(vec![row.into(), group_ui::drop_ring(self.on_app(app), 8.0, p)]);
        widget::container(row).width(Length::Fill).id(id).into()
    }

    pub(super) fn footer(&self, p: Palette) -> Element<'_, Message> {
        let m = &self.metrics;
        let s = texts();
        // The photo, if there is one; else the initial, on the accent.
        let avatar: Element<'_, Message> = match &self.avatar {
            Some(path) => widget::icon(icon::from_path(path.clone())).size(32).into(),
            None => widget::container(
                widget::text::body(
                    self.user.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default(),
                )
                .class(theme::Text::Color(Color::WHITE)),
            )
            .center(Length::Fixed(32.0))
            .class(theme::Container::custom(move |_| widget::container::Style {
                background: Some(Background::Color(p.accent)),
                border: iced::Border {
                    radius: 16.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }))
            .into(),
        };
        let user = widget::row::with_children(vec![avatar, widget::text::body(ellipsize(&self.user, m.user_chars)).into()])
            .spacing(12)
            .align_y(Alignment::Center);
        let round = |name: &'static str, message: Message| -> Element<'_, Message> {
            button::custom(widget::icon::icon(symbolic(name)).size(20))
                .padding(10)
                .class(flat_class(p, false))
                .on_press(message)
                .into()
        };
        let row = widget::row::with_children(vec![
            button::custom(user)
                .padding([6, 12, 6, 6])
                .class(flat_class(p, false))
                .on_press(Message::Menu(Some(Target::User)))
                .into(),
            widget::space::horizontal().into(),
            widget::tooltip(
                round("emblem-system-symbolic", Message::Spawn("cosmic-settings", Vec::new())),
                widget::text::body(s.settings),
                widget::tooltip::Position::Top,
            )
            .into(),
            widget::tooltip(
                round("system-shutdown-symbolic", Message::Menu(Some(Target::Power))),
                widget::text::body(s.power_off),
                widget::tooltip::Position::Top,
            )
            .into(),
        ])
        .spacing(4)
        .align_y(Alignment::Center)
        .padding([0.0, m.footer_pad]);
        widget::container(row)
            .width(Length::Fill)
            .height(Length::Fixed(FOOTER))
            .align_y(Alignment::Center)
            .class(theme::Container::custom(move |_| widget::container::Style {
                background: Some(Background::Color(p.footer)),
                border: iced::Border {
                    radius: [0.0, 0.0, 8.0, 8.0].into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..Default::default()
            }))
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::view_label;

    #[test]
    fn view_label_full_and_compact() {
        assert_eq!(view_label("Ver como", "Grade", false), "Ver como: Grade");
        assert_eq!(view_label("Ver como", "Grade", true), "Grade");
    }
}
