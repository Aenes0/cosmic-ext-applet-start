//! Groups of apps: the categories' folders, the user's and "Ungrouped". Pure: no iced.

/// How many categories there are: `texts().categories.len()`.
pub const CATEGORIES: usize = 10;

/// Stable keys of the categories, in `texts().categories` order: saved instead of the index.
pub const CATEGORY_KEYS: [&str; CATEGORIES] = [
    "productivity",
    "social",
    "internet",
    "creativity",
    "multimedia",
    "games",
    "development",
    "system",
    "utilities",
    "other",
];

/// A group: a category (index in `texts().categories`), one the user made, or the apps of none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GroupRef {
    Category(usize),
    User(u32),
    Loose,
}

/// An app as the groups see it: one per `AppEntry`, same index; `id` is the exact `AppEntry.id`.
#[derive(Clone, Copy, Debug)]
pub struct AppKey<'a> {
    pub id: &'a str,
    pub category: usize,
}

/// Where the apps are. Membership is unique by construction: `place` (one record per app) or,
/// without one, `Category(app.category)`. `order` is cosmetic.
#[derive(Clone, Debug, PartialEq)]
pub struct Groups {
    /// The user's groups: stable id, name; in creation order.
    pub user: Vec<(u32, String)>,
    /// Only exceptions: app id -> group other than its category.
    pub place: Vec<(String, GroupRef)>,
    /// Explicit order per group (partial).
    pub order: Vec<(GroupRef, Vec<String>)>,
    /// Id the next user group gets; never decreases, so ids are not reused.
    pub next: u32,
}

impl Default for Groups {
    fn default() -> Groups {
        Groups { user: Vec::new(), place: Vec::new(), order: Vec::new(), next: 1 }
    }
}

/// Max characters of a group name.
pub const NAME_MAX: usize = 32;

/// Trimmed name cut to `NAME_MAX` characters; empty stays empty.
fn clean_name(name: &str) -> String {
    name.trim().chars().take(NAME_MAX).collect::<String>().trim_end().to_string()
}

/// Removes the pinned entries of the groups that are gone.
pub fn unpin(pinned: &[String], gone: &[GroupRef]) -> Vec<String> {
    let dead: Vec<String> = gone.iter().filter_map(GroupRef::pin).collect();
    pinned.iter().filter(|p| !dead.contains(p)).cloned().collect()
}

/// What is saved: `groups`, `group_of`, `group_order` in the config.
pub type Stored = (Vec<(u32, String)>, Vec<(String, String)>, Vec<(String, Vec<String>)>);

impl GroupRef {
    /// Stable text key, saved in the config.
    pub fn key(&self) -> String {
        match self {
            GroupRef::Category(n) => format!("cat:{}", CATEGORY_KEYS[(*n).min(CATEGORIES - 1)]),
            GroupRef::User(id) => format!("usr:{id}"),
            GroupRef::Loose => "none".to_string(),
        }
    }

    /// Exact inverse of `key`.
    pub fn parse(text: &str) -> Option<GroupRef> {
        if text == "none" {
            return Some(GroupRef::Loose);
        }
        if let Some(k) = text.strip_prefix("cat:") {
            return CATEGORY_KEYS.iter().position(|c| *c == k).map(GroupRef::Category);
        }
        let digits = text.strip_prefix("usr:")?;
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digits.parse().ok().map(GroupRef::User)
    }

    /// Whether the group can be pinned (everything but `Loose`).
    pub fn pinnable(&self) -> bool {
        !matches!(self, GroupRef::Loose)
    }

    /// The pinned-entry text: `group:` + key.
    pub fn pin(&self) -> Option<String> {
        self.pinnable().then(|| format!("group:{}", self.key()))
    }

    /// Inverse of `pin`; `group:none` is not pinnable.
    pub fn from_pin(text: &str) -> Option<GroupRef> {
        GroupRef::parse(text.strip_prefix("group:")?).filter(GroupRef::pinnable)
    }
}

impl Groups {
    /// Builds normalized groups from the saved form, dropping anything invalid or repeated.
    pub fn from_stored(
        user: Vec<(u32, String)>,
        place: Vec<(String, String)>,
        order: Vec<(String, Vec<String>)>,
    ) -> Groups {
        let mut out = Groups::default();
        for (id, name) in user {
            if out.user.iter().all(|(u, _)| *u != id) {
                out.user.push((id, name));
            }
        }
        for (app, key) in place {
            let Some(g) = GroupRef::parse(&key).filter(|g| out.exists(*g)) else { continue };
            if out.place.iter().all(|(a, _)| *a != app) {
                out.place.push((app, g));
            }
        }
        for (key, list) in order {
            let Some(g) = GroupRef::parse(&key).filter(|g| out.exists(*g)) else { continue };
            if out.order.iter().any(|(o, _)| *o == g) {
                continue;
            }
            let mut uniq: Vec<String> = Vec::new();
            for id in list {
                if !uniq.contains(&id) {
                    uniq.push(id);
                }
            }
            out.order.push((g, uniq));
        }
        out.next = out.user.iter().map(|(id, _)| id.saturating_add(1)).max().unwrap_or(1);
        out
    }

    /// Raises `next` to at least the saved value.
    pub fn with_next(self, stored: u32) -> Groups {
        let next = self.next.max(stored);
        Groups { next, ..self }
    }

    /// Drops `app` onto `onto`: joins its user group or makes a new one named `name`.
    pub fn merge(&self, app: AppKey, onto: AppKey, name: &str, apps: &[AppKey]) -> (Groups, GroupRef) {
        let target = self.group_of(onto);
        if app.id == onto.id {
            return (self.clone(), target);
        }
        if let GroupRef::User(_) = target {
            let pos = self.members(target, apps).iter().position(|&i| apps[i].id == onto.id);
            let at = pos.map(|p| p + 1);
            return (self.place(app, target, at, apps), target);
        }
        let id = self.next;
        let name = Some(clean_name(name)).filter(|n| !n.is_empty()).unwrap_or_else(|| "Group".to_string());
        let mut out = self.clone();
        out.next = id.saturating_add(1);
        out.user.push((id, name));
        let made = GroupRef::User(id);
        let out = out.place(onto, made, None, apps).place(app, made, None, apps);
        (out, made)
    }

    /// Renames a user group; blank names and unknown ids change nothing.
    pub fn rename(&self, id: u32, name: &str) -> Groups {
        let name = clean_name(name);
        let mut out = self.clone();
        if let Some(entry) = out.user.iter_mut().find(|(u, _)| *u == id) {
            if !name.is_empty() {
                entry.1 = name;
            }
        }
        out
    }

    /// Deletes a user group; its apps become ungrouped.
    pub fn delete(&self, id: u32) -> Groups {
        let g = GroupRef::User(id);
        let mut out = self.clone();
        out.user.retain(|(u, _)| *u != id);
        out.order.retain(|(o, _)| *o != g);
        for (_, to) in out.place.iter_mut().filter(|(_, to)| *to == g) {
            *to = GroupRef::Loose;
        }
        out
    }

    /// Removes user groups with no app; returns them.
    pub fn emptied(&self) -> (Groups, Vec<GroupRef>) {
        let gone: Vec<GroupRef> = self
            .user
            .iter()
            .map(|(id, _)| GroupRef::User(*id))
            .filter(|g| self.place.iter().all(|(_, to)| to != g))
            .collect();
        let mut out = self.clone();
        out.user.retain(|(id, _)| !gone.contains(&GroupRef::User(*id)));
        out.order.retain(|(o, _)| !gone.contains(o));
        (out, gone)
    }

    /// `base`, or `base 2`, `base 3`... the first not used by a user group.
    pub fn next_name(&self, base: &str) -> String {
        let taken = |n: &str| self.user.iter().any(|(_, u)| u == n);
        (1u32..)
            .map(|i| if i == 1 { base.to_string() } else { format!("{base} {i}") })
            .find(|n| !taken(n))
            .unwrap_or_else(|| base.to_string())
    }

    /// The saved form.
    pub fn to_stored(&self) -> Stored {
        let place = self.place.iter().map(|(a, g)| (a.clone(), g.key())).collect();
        let order = self.order.iter().map(|(g, l)| (g.key(), l.clone())).collect();
        (self.user.clone(), place, order)
    }

    /// The group an app is in.
    pub fn group_of(&self, app: AppKey) -> GroupRef {
        match self.place.iter().find(|(a, _)| a == app.id) {
            Some((_, g)) => *g,
            None => GroupRef::Category(app.category),
        }
    }

    /// Indices in `apps` of the members of `g`: explicit order first, then the rest as in `apps`.
    pub fn members(&self, g: GroupRef, apps: &[AppKey]) -> Vec<usize> {
        let all: Vec<usize> = (0..apps.len()).filter(|&i| self.group_of(apps[i]) == g).collect();
        let explicit = self.order.iter().find(|(o, _)| *o == g).map(|(_, l)| l.as_slice());
        let mut out: Vec<usize> = Vec::new();
        for id in explicit.unwrap_or(&[]) {
            if let Some(&i) = all.iter().find(|&&i| apps[i].id == id) {
                if !out.contains(&i) {
                    out.push(i);
                }
            }
        }
        let rest: Vec<usize> = all.iter().copied().filter(|i| !out.contains(i)).collect();
        out.extend(rest);
        out
    }

    /// Non-empty groups: the user's, then the categories, then `Loose`.
    pub fn visible(&self, apps: &[AppKey]) -> Vec<(GroupRef, Vec<usize>)> {
        let refs = self
            .user
            .iter()
            .map(|(id, _)| GroupRef::User(*id))
            .chain((0..CATEGORIES).map(GroupRef::Category))
            .chain(std::iter::once(GroupRef::Loose));
        refs.map(|g| (g, self.members(g, apps))).filter(|(_, m)| !m.is_empty()).collect()
    }

    /// Whether the group exists.
    pub fn exists(&self, g: GroupRef) -> bool {
        match g {
            GroupRef::User(id) => self.user.iter().any(|(u, _)| *u == id),
            GroupRef::Category(n) => n < CATEGORIES,
            GroupRef::Loose => true,
        }
    }

    /// Name of a user group.
    pub fn name(&self, id: u32) -> Option<&str> {
        self.user.iter().find(|(u, _)| *u == id).map(|(_, n)| n.as_str())
    }

    /// Moves `app` into `into`, before the member at index `before` (`None` = end).
    pub fn place(&self, app: AppKey, into: GroupRef, before: Option<usize>, apps: &[AppKey]) -> Groups {
        if !self.exists(into) {
            return self.clone();
        }
        let mut list: Vec<String> =
            self.members(into, apps).iter().map(|&i| apps[i].id.to_string()).collect();
        let from = list.iter().position(|id| id == app.id);
        if let Some(f) = from {
            list.remove(f);
        }
        let at = before
            .map(|i| if from.is_some_and(|f| i > f) { i - 1 } else { i })
            .unwrap_or(list.len())
            .min(list.len());
        list.insert(at, app.id.to_string());
        let mut out = self.forget(app.id);
        out.order.retain(|(g, _)| *g != into);
        out.order.push((into, list));
        if into != GroupRef::Category(app.category) {
            out.place.push((app.id.to_string(), into));
        }
        out
    }

    /// Drops every trace of an app (uninstalled).
    pub fn forget(&self, app_id: &str) -> Groups {
        let mut out = self.clone();
        out.place.retain(|(a, _)| a != app_id);
        for (_, l) in out.order.iter_mut() {
            l.retain(|id| id != app_id);
        }
        out
    }
}

#[cfg(test)]
mod tests;
