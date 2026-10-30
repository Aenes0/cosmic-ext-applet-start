//! The open-windows strip, by app: the glue between the windows and their groups.

use super::open_groups::{OpenGroup, active_group, group, group_key, switch_target};
use super::*;

/// An entry of a group's little menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GroupItem {
    Switch,
    NewWindow,
    Close,
    CloseAll,
}

/// The strip's first group after the windows changed, with the key of the
/// active group. Moving between windows of one group does not move the page.
pub(super) fn next_offset(
    prev_groups: usize,
    groups: &[OpenGroup],
    active: Option<usize>,
    prev_key: Option<&str>,
    offset: usize,
    capacity: impl Fn(usize) -> usize,
) -> (usize, Option<String>) {
    let key = active.and_then(|i| groups.get(i)).map(|g| g.key.clone());
    let changed = key.as_deref() != prev_key;
    let next = adjust_open(capacity(prev_groups), capacity(groups.len()), groups.len(), active, changed, offset);
    (next, key)
}

/// Old key to new key of the groups whose key changed over the same
/// windows; groups are matched by the first window index they hold.
pub(super) fn rekey_map(old: &[OpenGroup], new: &[OpenGroup]) -> HashMap<String, String> {
    old.iter()
        .filter_map(|o| {
            let first = o.windows.first()?;
            let n = new.iter().find(|n| n.windows.first() == Some(first))?;
            (n.key != o.key).then(|| (o.key.clone(), n.key.clone()))
        })
        .collect()
}

/// Whether the open little menu is of a group that no longer exists.
pub(super) fn orphan_group_menu(target: Option<&Target>, exists: impl Fn(&str) -> bool) -> bool {
    matches!(target, Some(Target::OpenGroup(k)) if !exists(k))
}

/// The window a click on a group goes to: the only one, else the active
/// one, else the newest.
pub(super) fn click_window(group: &OpenGroup, is_active: impl Fn(usize) -> bool) -> Option<usize> {
    switch_target(group, is_active)
}

/// The entries of a group's little menu.
pub(super) fn group_menu_items(n: usize, app_known: bool) -> Vec<GroupItem> {
    let mut items = vec![GroupItem::Switch];
    if app_known {
        items.push(GroupItem::NewWindow);
    }
    items.push(if n > 1 { GroupItem::CloseAll } else { GroupItem::Close });
    items
}

/// A line of a group's little menu: icon, text, message.
pub(super) type MenuLine = (&'static str, String, Message);

impl StartMenu {
    pub(super) fn window_key(&self, w: &Window) -> String {
        group_key(self.app_for_window(w).map(|a| a.id.as_str()), &w.app_id, &w.id)
    }

    fn build_groups(&mut self) {
        let keys: Vec<String> = self.windows.iter().map(|w| self.window_key(w)).collect();
        let indices: Vec<usize> = (0..keys.len()).collect();
        self.open_groups = group(&indices, |&i| keys[i].clone());
    }

    /// Groups the windows again (the windows changed).
    pub(super) fn regroup(&mut self) {
        self.build_groups();
        self.close_orphan_menu();
    }

    /// Groups the SAME windows again (the known apps changed): a group may
    /// get another key (`id:x` becomes `app:x`), and whatever points at the
    /// old key follows it.
    pub(super) fn regroup_same_windows(&mut self) {
        let old = std::mem::take(&mut self.open_groups);
        self.build_groups();
        let map = rekey_map(&old, &self.open_groups);
        if let Some(new) = self.open_active.as_ref().and_then(|k| map.get(k)) {
            self.open_active = Some(new.clone());
        }
        if let Some(new) = self.hover.key().and_then(|k| map.get(k)) {
            self.hover.rekey(new.clone());
        }
        if let Some(Target::OpenGroup(k)) = &self.menu
            && let Some(new) = map.get(k)
        {
            self.menu = Some(Target::OpenGroup(new.clone()));
        }
        self.close_orphan_menu();
    }

    /// Closes the group's little menu when its group is gone.
    fn close_orphan_menu(&mut self) {
        if orphan_group_menu(self.menu.as_ref(), |k| self.group_of(k).is_some()) {
            self.close_menu();
        }
    }

    pub(super) fn group_of(&self, key: &str) -> Option<&OpenGroup> {
        self.open_groups.iter().find(|g| g.key == key)
    }

    fn is_window_active(&self, index: usize) -> bool {
        self.windows.get(index).is_some_and(|w| w.active)
    }

    pub(super) fn active_group_index(&self) -> Option<usize> {
        active_group(&self.open_groups, |i| self.is_window_active(i))
    }

    pub(super) fn set_windows(&mut self, windows: Vec<Window>) {
        let prev_groups = self.open_groups.len();
        self.windows = windows;
        self.regroup();
        let m = &self.metrics;
        let (offset, key) = next_offset(
            prev_groups,
            &self.open_groups,
            self.active_group_index(),
            self.open_active.as_deref(),
            self.open_offset,
            |n| m.open_capacity(n),
        );
        self.open_offset = offset;
        self.open_active = key;
    }

    pub(super) fn open_page(&mut self, forward: bool) {
        let total = self.open_groups.len();
        let cap = self.metrics.open_capacity(total);
        self.open_offset = if forward {
            open_strip::next(total, cap, self.open_offset)
        } else {
            open_strip::prev(cap, self.open_offset)
        };
    }

    /// The menu shows: the strip starts at the page of the active group.
    pub(super) fn reset_open(&mut self) {
        let total = self.open_groups.len();
        let cap = self.metrics.open_capacity(total);
        self.open_offset = adjust_open(cap, cap, total, self.active_group_index(), true, 0);
    }

    /// The id of the window a click on the group goes to.
    fn click_id(&self, key: &str) -> Option<String> {
        let group = self.group_of(key)?;
        let index = click_window(group, |i| self.is_window_active(i))?;
        self.windows.get(index).map(|w| w.id.clone())
    }

    /// A click on a group's icon. The one place that decides it: today it
    /// switches to the group's window; with several, the preview opens
    /// here instead.
    pub(super) fn group_click(&mut self, key: &str) -> app::Task<Message> {
        if self.group_of(key).is_some_and(|g| g.windows.len() > 1) {
            return self.preview(preview::Event::ClickGroup(key.to_string()));
        }
        match self.click_id(key) {
            Some(id) => self.activate(id),
            None => Task::none(),
        }
    }

    pub(super) fn close_windows(&mut self, ids: Vec<String>) {
        self.close_menu();
        if let Some(tx) = &self.windows_tx {
            for id in ids {
                _ = tx.send(Command::Close(id));
            }
        }
    }

    /// The title and lines of a group's little menu; `None` if it is gone.
    pub(super) fn group_menu(&self, key: &str) -> Option<(String, Vec<MenuLine>)> {
        let group = self.group_of(key)?;
        let ids: Vec<String> = group.windows.iter().filter_map(|&i| self.windows.get(i)).map(|w| w.id.clone()).collect();
        let first = self.windows.get(*group.windows.first()?)?;
        let app = self.app_for_window(first);
        let title = app.map_or_else(|| first.app_id.clone(), |a| a.name.clone());
        let switch = self.click_id(key)?;
        let t = texts();
        let lines = group_menu_items(group.len(), app.is_some())
            .into_iter()
            .filter_map(|item| match item {
                GroupItem::Switch => Some(("window-restore-symbolic", t.switch_to.to_string(), Message::Activate(switch.clone()))),
                GroupItem::NewWindow => {
                    app.map(|a| ("window-new-symbolic", t.new_window.to_string(), Message::Launch(a.id.clone())))
                }
                GroupItem::Close => {
                    ids.first().map(|id| ("window-close-symbolic", t.close_window.to_string(), Message::CloseWindow(id.clone())))
                }
                GroupItem::CloseAll => {
                    Some(("window-close-symbolic", t.close_all_windows.to_string(), Message::CloseWindows(ids.clone())))
                }
            })
            .collect();
        Some((title, lines))
    }
}

#[cfg(test)]
mod tests;
