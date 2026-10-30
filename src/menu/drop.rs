//! What a drop does to the pinned and the groups. Pure.

use super::drag::{self, Action, Item};
use super::groups::{self, AppKey, GroupRef, Groups};

/// The pinned and the groups after a drop.
#[derive(Debug, Clone, PartialEq)]
pub struct Dropped {
    pub pinned: Vec<String>,
    pub groups: Groups,
    /// A group just created by a merge: the menu opens it for naming.
    pub open: Option<GroupRef>,
}

/// What `action` does; `None` when nothing changes (nothing to save).
/// `find` resolves a pinned entry (maybe a loose id like "firefox") to its index in `keys`;
/// `same` tells whether a pinned entry is a given app or group key (for Pin without duplicates).
pub fn resolve_drop(
    action: &Action,
    pinned: &[String],
    groups: &Groups,
    keys: &[AppKey],
    find: &dyn Fn(&str) -> Option<usize>,
    same: &dyn Fn(&str, &str) -> bool,
) -> Option<Dropped> {
    let name = crate::texts::texts().new_group;
    resolve_drop_named(action, pinned, groups, keys, find, same, name)
}

/// Like `resolve_drop`, with the base name a newly created group gets.
pub fn resolve_drop_named(
    action: &Action,
    pinned: &[String],
    groups: &Groups,
    keys: &[AppKey],
    find: &dyn Fn(&str) -> Option<usize>,
    same: &dyn Fn(&str, &str) -> bool,
    new_name: &str,
) -> Option<Dropped> {
    let raw = match action {
        Action::Cancel => None,
        Action::Reorder { .. } | Action::Pin { .. } => {
            return Some(Dropped {
                pinned: drag::apply(pinned, action, same),
                groups: groups.clone(),
                open: None,
            });
        }
        Action::Unpin { index } => {
            let rest = pinned.iter().enumerate().filter(|(n, _)| n != index).map(|(_, p)| p.clone());
            return (*index < pinned.len()).then(|| plain(rest.collect(), groups.clone()));
        }
        Action::Merge { item, onto } => merge(item, *onto, pinned, groups, keys, find, same, new_name),
        Action::Place { item: Item::App(id), into, before } => {
            let app = exact(keys, id)?;
            Some(plain(pinned.to_vec(), groups.place(app, *into, *before, keys)))
        }
        Action::Place { item: Item::Pinned(i), into, before } => {
            let (app, rest) = absorb(pinned, *i, keys, find)?;
            Some(plain(rest, groups.place(app, *into, *before, keys)))
        }
        Action::Ungroup { app } => {
            let key = exact(keys, app)?;
            Some(plain(pinned.to_vec(), groups.place(key, GroupRef::Loose, None, keys)))
        }
    }?;
    let (groups, gone) = raw.groups.emptied();
    Some(Dropped { pinned: groups::unpin(&raw.pinned, &gone), groups, open: raw.open })
}

/// Removes the group `id` (its apps become loose) and its pinned entry.
pub fn delete_group(pinned: &[String], groups: &Groups, id: u32) -> (Vec<String>, Groups) {
    (groups::unpin(pinned, &[GroupRef::User(id)]), groups.delete(id))
}

fn plain(pinned: Vec<String>, groups: Groups) -> Dropped {
    Dropped { pinned, groups, open: None }
}

/// The app of pinned entry `i` and the pinned without it; `None` for groups or unknown apps.
fn absorb<'a>(
    pinned: &[String],
    i: usize,
    keys: &[AppKey<'a>],
    find: &dyn Fn(&str) -> Option<usize>,
) -> Option<(AppKey<'a>, Vec<String>)> {
    let entry = pinned.get(i)?;
    if GroupRef::from_pin(entry).is_some() {
        return None;
    }
    let app = *keys.get(find(entry)?)?;
    let rest = pinned.iter().enumerate().filter(|(n, _)| *n != i).map(|(_, p)| p.clone()).collect();
    Some((app, rest))
}

/// Dropping `item` onto the app at `onto`: a new user group or joining the existing one.
fn merge(
    item: &Item,
    onto: usize,
    pinned: &[String],
    groups: &Groups,
    keys: &[AppKey],
    find: &dyn Fn(&str) -> Option<usize>,
    same: &dyn Fn(&str, &str) -> bool,
    new_name: &str,
) -> Option<Dropped> {
    let (app, rest) = match item {
        Item::App(id) => (exact(keys, id)?, pinned.to_vec()),
        Item::Pinned(i) => absorb(pinned, *i, keys, find)?,
    };
    let target_app = *keys.get(onto)?;
    if app.id == target_app.id {
        return None;
    }
    let name = groups.next_name(new_name);
    let (merged, target) = groups.merge(app, target_app, &name, keys);
    if groups.exists(target) {
        return Some(plain(rest, merged));
    }
    let pinned = pin_group(&rest, target, target_app.id, same);
    Some(Dropped { pinned, groups: merged, open: Some(target) })
}

/// Swaps the first entry that is `app_id` for the group's pin, without duplicating it.
fn pin_group(pinned: &[String], group: GroupRef, app_id: &str, same: &dyn Fn(&str, &str) -> bool) -> Vec<String> {
    let Some(at) = pinned.iter().position(|e| same(e, app_id)) else {
        return pinned.to_vec();
    };
    let pin = group.pin();
    let dup = pin.as_ref().is_some_and(|p| pinned.contains(p));
    let mut out = pinned.to_vec();
    match (pin, dup) {
        (Some(p), false) => out[at] = p,
        _ => {
            out.remove(at);
        }
    }
    out
}

/// The key whose id is exactly `id`.
fn exact<'a>(keys: &[AppKey<'a>], id: &str) -> Option<AppKey<'a>> {
    keys.iter().find(|k| k.id == id).copied()
}

#[cfg(test)]
mod tests;
