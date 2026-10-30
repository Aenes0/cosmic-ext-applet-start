use super::*;

/// The menu's rectangle on a surface of `surface`'s size.
pub(super) fn menu_rect(surface: iced::Size, on_top: bool, size: iced::Size) -> iced::Rectangle {
    let x = ((surface.width - size.width) / 2.0).max(0.0);
    let y = if on_top { MARGIN } else { (surface.height - MARGIN - size.height).max(0.0) };
    iced::Rectangle::new(iced::Point::new(x, y), size)
}

/// Confirms the draft being edited, if any, and opens the name of group `next` for editing.
fn switch_edit(
    groups: &groups::Groups,
    current: Option<(u32, String)>,
    next: u32,
) -> (groups::Groups, Option<(u32, String)>) {
    let confirmed = match current {
        Some((id, draft)) => groups.rename(id, &draft),
        None => groups.clone(),
    };
    let edit = confirmed.name(next).map(|n| (next, n.to_owned()));
    (confirmed, edit)
}

/// What takes the focus: the search, or the title of a group while its name is edited.
pub(super) fn focus_id(editing: bool) -> widget::Id {
    if editing { widget::Id::new(group_ui::GROUP_NAME) } else { SEARCH.clone() }
}

/// What an entry of a group's context menu does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupAction {
    Open,
    Rename,
    Pin,
    Unpin,
    Delete,
}

/// The entries of a group's context menu: only the user's can be renamed or deleted,
/// and the apps of no group cannot be pinned.
fn group_menu_items(g: groups::GroupRef, pinned: bool) -> Vec<GroupAction> {
    let user = group_ui::editable(g).is_some();
    let mut items = vec![GroupAction::Open];
    if user {
        items.push(GroupAction::Rename);
    }
    if g.pinnable() {
        items.push(if pinned { GroupAction::Unpin } else { GroupAction::Pin });
    }
    if user {
        items.push(GroupAction::Delete);
    }
    items
}

impl StartMenu {
    /// Opens a user group's popup with its name ready to be typed over.
    pub(super) fn edit_name(&mut self, id: u32) -> app::Task<Message> {
        let (groups, edit) = switch_edit(&self.groups, self.group_edit.take(), id);
        if groups != self.groups {
            self.groups = groups;
            save_groups(&self.groups);
        }
        let Some(edit) = edit else {
            return Task::none();
        };
        self.menu = Some(Target::Group(groups::GroupRef::User(id)));
        self.menu_anchor = None;
        self.group_edit = Some(edit);
        let field = widget::Id::new(group_ui::GROUP_NAME);
        Task::batch([text_input::focus(field.clone()), text_input::select_all(field)])
    }

    /// Deletes a user group: its apps become ungrouped, its pin goes.
    pub(super) fn delete_group(&mut self, id: u32) {
        if self.group_edit.as_ref().is_some_and(|(e, _)| *e == id) {
            self.group_edit = None;
        }
        let (pinned, groups) = drop::delete_group(&self.pinned, &self.groups, id);
        self.pinned = pinned;
        self.groups = groups;
        save_pinned(&self.pinned);
        save_groups(&self.groups);
        self.close_menu();
    }

    /// Confirms the name being edited, if any, and saves it when it changed.
    pub(super) fn commit_name(&mut self) {
        let Some((id, draft)) = self.group_edit.take() else {
            return;
        };
        let renamed = self.groups.rename(id, &draft);
        if renamed != self.groups {
            self.groups = renamed;
            save_groups(&self.groups);
        }
    }

    /// The little menus: an app's, a window's, a file's (right click), a
    /// category's apps, the user's and the power button's.
    fn popup_card(&self, target: &Target, p: Palette) -> Option<PopupCard<'_>> {
        let s = texts();
        let m = &self.metrics;
        let t = texts();
        let item = |icon_name: Option<&'static str>, label: String, message: Message| -> Element<'_, Message> {
            let mut row = widget::row::with_capacity(2).spacing(12).align_y(Alignment::Center);
            if let Some(name) = icon_name {
                row = row.push(widget::icon::icon(symbolic(name)).size(16));
            }
            button::custom(row.push(widget::text::body(label)))
                .padding([8, 12])
                .width(Length::Fill)
                .class(flat_class(p, false))
                .on_press(message)
                .into()
        };
        let mut items: Vec<Element<'_, Message>> = Vec::new();
        let mut title = None;
        let mut width = m.context_width;
        // Where it shows: by the button that opened it, or in the middle.
        let mut place = (Alignment::Center, Alignment::Center);
        match target {
            Target::App(id) => {
                let Some(app) = self.apps.iter().find(|a| a.id == *id) else {
                    return None;
                };
                title = Some(app.name.clone());
                items.push(item(Some("media-playback-start-symbolic"), t.open_app.into(), Message::Launch(app.id.clone())));
                for (name, exec) in &app.actions {
                    items.push(item(None, name.clone(), Message::LaunchAction(app.id.clone(), exec.clone())));
                }
                let pinned = self.pinned.iter().any(|p| *p == app.id);
                items.push(item(
                    Some(if pinned { "view-pin-symbolic" } else { "pin-symbolic" }),
                    if pinned { t.unpin_start } else { t.pin_start }.into(),
                    Message::TogglePin(app.id.clone()),
                ));
                if uninstall_ui::uninstall_visible(app) {
                    items.push(item(Some("user-trash-symbolic"), t.uninstall.into(), Message::AskUninstall(app.id.clone())));
                }
            }
            Target::Uninstall(id) => {
                let (Some(app), Some(dialog)) = (self.apps.iter().find(|a| a.id == *id), &self.uninstall) else {
                    return None;
                };
                // The card shows the app's name in its own header.
                width = m.uninstall_width;
                items.extend(uninstall_ui::card_body(app, dialog, m.uninstall_max_height, p));
            }
            Target::Window(id) => {
                let Some(window) = self.windows.iter().find(|w| w.id == *id) else {
                    return None;
                };
                title = Some(window.title.clone());
                items.push(item(Some("window-restore-symbolic"), t.switch_to.into(), Message::Activate(window.id.clone())));
                if let Some(app) = self.app_for_window(window) {
                    items.push(item(Some("window-new-symbolic"), t.new_window.into(), Message::Launch(app.id.clone())));
                }
                items.push(item(Some("window-close-symbolic"), t.close_window.into(), Message::CloseWindow(window.id.clone())));
            }
            Target::OpenGroup(key) => {
                let (name, lines) = self.group_menu(key)?;
                title = Some(name);
                items.extend(lines.into_iter().map(|(icon_name, label, message)| item(Some(icon_name), label, message)));
            }
            Target::File(path) => {
                title = Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned());
                items.push(item(Some("document-open-symbolic"), t.open_app.into(), Message::OpenFile(path.clone())));
                items.push(item(Some("folder-open-symbolic"), t.open_location.into(), Message::OpenLocation(path.clone())));
            }
            Target::Group(g) => {
                if !self.groups.exists(*g) {
                    return None;
                }
                let dragged_id = match dragged(&self.drag) {
                    Some(drag::Source::Member(_, id)) => Some(id.as_str()),
                    _ => None,
                };
                width = m.group_popup_width;
                let ring = match self.drag_over() {
                    Some(drag::Slot::OnApp(i)) => self.apps.get(i).map(|a| a.id.as_str()),
                    _ => None,
                };
                let title = match (&self.group_edit, group_ui::editable(*g)) {
                    (Some((id, draft)), Some(user)) if *id == user => draft.clone(),
                    _ => self.group_name(*g),
                };
                items.push(group_ui::popup_body(
                    title,
                    *g,
                    &self.group_apps(*g),
                    self.drag_over(),
                    dragged_id,
                    ring,
                    self.drag.is_active(),
                    p,
                    m,
                ));
            }
            Target::GroupMenu(g) => {
                if !self.groups.exists(*g) {
                    return None;
                }
                title = Some(self.group_name(*g));
                let key = g.pin();
                let pinned = key.as_ref().is_some_and(|k| self.pinned.contains(k));
                let user = group_ui::editable(*g);
                for action in group_menu_items(*g, pinned) {
                    items.push(match (action, &key, user) {
                        (GroupAction::Open, _, _) => item(
                            Some("folder-open-symbolic"),
                            t.open_app.into(),
                            Message::Menu(Some(Target::Group(*g))),
                        ),
                        (GroupAction::Rename, _, Some(id)) => {
                            item(Some("document-edit-symbolic"), t.rename.into(), Message::RenameGroup(id))
                        }
                        (GroupAction::Delete, _, Some(id)) => {
                            item(Some("user-trash-symbolic"), t.delete_group.into(), Message::DeleteGroup(id))
                        }
                        (GroupAction::Pin | GroupAction::Unpin, Some(key), _) => item(
                            Some(if pinned { "view-pin-symbolic" } else { "pin-symbolic" }),
                            if pinned { t.unpin_start } else { t.pin_start }.into(),
                            Message::TogglePin(key.clone()),
                        ),
                        _ => continue,
                    });
                }
            }
            Target::User => {
                place = (Alignment::Start, Alignment::End);
                items.push(item(Some("system-users-symbolic"), t.account.into(), Message::Spawn("cosmic-settings", vec!["users".into()])));
                items.push(item(Some("system-lock-screen-symbolic"), s.lock.into(), Message::Spawn("loginctl", vec!["lock-session".into()])));
                items.push(item(Some("system-log-out-symbolic"), s.log_out.into(), Message::Spawn("cosmic-osd", vec!["log-out".into()])));
            }
            Target::Power => {
                place = (Alignment::End, Alignment::End);
                width = m.power_width;
                items.push(item(Some("system-lock-screen-symbolic"), s.lock.into(), Message::Spawn("loginctl", vec!["lock-session".into()])));
                items.push(item(Some("system-log-out-symbolic"), s.log_out.into(), Message::Spawn("cosmic-osd", vec!["log-out".into()])));
                items.push(item(Some("system-suspend-symbolic"), s.suspend.into(), Message::Spawn("systemctl", vec!["suspend".into()])));
                items.push(item(Some("system-shutdown-symbolic"), s.power_off.into(), Message::Spawn("cosmic-osd", vec!["shutdown".into()])));
                items.push(item(Some("system-reboot-symbolic"), s.restart.into(), Message::Spawn("cosmic-osd", vec!["restart".into()])));
            }
        }
        let (count, has_title) = (items.len(), title.is_some());
        let mut column = widget::column::with_capacity(items.len() + 1).spacing(2).padding(4);
        if let Some(title) = title {
            column = column.push(
                widget::container(widget::text::heading(title).wrapping(Wrapping::None))
                    .padding([8, 12])
                    .clip(true),
            );
        }
        for item in items {
            column = column.push(item);
        }
        let is_group = matches!(target, Target::Group(_));
        let radius = if is_group { group_ui::POPUP_RADIUS } else { 8.0 };
        let card = widget::container(column).width(Length::Fixed(width));
        let card = if is_group { card.id(group_ui::popup_card_id()) } else { card };
        let card = card.class(theme::Container::custom(move |_| widget::container::Style {
                background: Some(Background::Color(p.popup)),
                text_color: Some(p.text),
                icon_color: Some(p.text),
                border: iced::Border {
                    radius: radius.into(),
                    width: 1.0,
                    color: p.border,
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
                ..Default::default()
            }));
        Some(PopupCard { card: card.into(), count, has_title, width, place })
    }

    /// The little menu: the dimmed or clear backdrop, closing it on a click,
    /// and, unless it is drawn at its item (`anchored`), the card.
    pub(super) fn popup_menu(&self, target: &Target, p: Palette, anchored: bool) -> Element<'_, Message> {
        // A folder dims the menu behind it, as on Windows; the others don't.
        let dim = matches!(target, Target::Group(_));
        let backdrop = widget::mouse_area(
            widget::container(widget::space::horizontal())
                .width(Length::Fill)
                .height(Length::Fill)
                .class(theme::Container::custom(move |_| widget::container::Style {
                    background: dim.then(|| Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.45))),
                    border: iced::Border {
                        radius: 8.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })),
        )
        .on_press(Message::Menu(None))
        .on_right_press(Message::Menu(None));
        if anchored {
            return backdrop.into();
        }
        let Some(PopupCard { card, place: (x, y), .. }) = self.popup_card(target, p) else {
            return widget::text("").into();
        };
        let bottom = if y == Alignment::End { FOOTER - 4.0 } else { 0.0 };
        let side = if x == Alignment::Center { 0.0 } else { self.metrics.popup_side };
        iced::widget::stack(vec![
            backdrop.into(),
            widget::container(widget::mouse_area(card).on_press(Message::Nothing))
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(iced::Padding { top: 0.0, right: side, bottom, left: side })
                .align_x(x)
                .align_y(y)
                .into(),
        ])
        .into()
    }

    /// Where an app's, window's or file's menu is drawn at its item: the
    /// anchor, if the surface's size is known.
    pub(super) fn anchored(&self) -> Option<(&Target, iced::Rectangle)> {
        let target = self.menu.as_ref()?;
        let anchor = self.menu_anchor?;
        let known = self.room_width > 0.0 && self.room > 0.0;
        (known && matches!(target, Target::App(_) | Target::Window(_) | Target::OpenGroup(_) | Target::File(_) | Target::GroupMenu(_))).then_some((target, anchor))
    }

    /// The card of an anchored menu, set on the surface by the item.
    pub(super) fn anchored_popup(&self, target: &Target, anchor: iced::Rectangle, p: Palette) -> Element<'_, Message> {
        let Some(card) = self.popup_card(target, p) else {
            return widget::text("").into();
        };
        let menu = self.menu_rect();
        let size = iced::Size::new(card.width, anchor::popup_height(card.count, card.has_title));
        let at = anchor::popup_origin(anchor, size, menu);
        iced::widget::pin(widget::mouse_area(card.card).on_press(Message::Nothing))
            .x(at.x)
            .y(at.y)
            .into()
    }
}

/// A little menu's card, and what is needed to place it.
struct PopupCard<'a> {
    card: Element<'a, Message>,
    count: usize,
    has_title: bool,
    width: f32,
    place: (Alignment, Alignment),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_sits_centered_at_the_bottom_or_the_top() {
        let surface = iced::Size::new(1920.0, 1080.0);
        let bottom = menu_rect(surface, false, iced::Size::new(874.0, 900.0));
        assert_eq!((bottom.x, bottom.y), ((1920.0 - 874.0) / 2.0, 1080.0 - MARGIN - 900.0));
        assert_eq!((bottom.width, bottom.height), (874.0, 900.0));
        assert_eq!(menu_rect(surface, true, iced::Size::new(874.0, 900.0)).y, MARGIN);
    }

    #[test]
    fn the_group_menu_items_depend_on_the_group() {
        use GroupAction::*;
        let user = groups::GroupRef::User(3);
        assert_eq!(group_menu_items(user, false), vec![Open, Rename, Pin, Delete]);
        assert_eq!(group_menu_items(user, true), vec![Open, Rename, Unpin, Delete]);
        assert_eq!(group_menu_items(groups::GroupRef::Category(1), false), vec![Open, Pin]);
        assert_eq!(group_menu_items(groups::GroupRef::Loose, true), vec![Open]);
    }

    #[test]
    fn opening_another_name_confirms_the_draft() {
        let g = groups::Groups::from_stored(vec![(1, "A".into()), (2, "B".into())], vec![], vec![]);
        let (out, edit) = switch_edit(&g, Some((1, "Novo".into())), 2);
        assert_eq!(out.name(1), Some("Novo"));
        assert_eq!(edit, Some((2, "B".to_owned())));
        let (same, none) = switch_edit(&g, None, 9);
        assert_eq!(same, g);
        assert_eq!(none, None);
    }

    #[test]
    fn focus_goes_to_the_title_while_it_is_edited() {
        assert_eq!(focus_id(false), SEARCH.clone());
        assert_eq!(focus_id(true), widget::Id::new(group_ui::GROUP_NAME));
    }
}
