//! Pure grouping of open windows by app.

use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenGroup {
    pub key: String,
    /// Indices into the window list.
    pub windows: Vec<usize>,
}

pub fn group_key(app: Option<&str>, app_id: &str, window_id: &str) -> String {
    if let Some(app) = app {
        return format!("app:{app}");
    }
    if app_id.is_empty() {
        return format!("window:{window_id}");
    }
    format!("id:{}", app_id.to_lowercase())
}

pub fn group<W>(windows: &[W], key_of: impl Fn(&W) -> String) -> Vec<OpenGroup> {
    let mut groups: Vec<OpenGroup> = Vec::new();
    let mut pos: HashMap<String, usize> = HashMap::new();
    for (i, w) in windows.iter().enumerate() {
        let key = key_of(w);
        match pos.get(&key) {
            Some(&p) => groups[p].windows.push(i),
            None => {
                pos.insert(key.clone(), groups.len());
                groups.push(OpenGroup { key, windows: vec![i] });
            }
        }
    }
    groups
}

impl OpenGroup {
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    pub fn is_active(&self, is_active: impl Fn(usize) -> bool) -> bool {
        self.windows.iter().any(|&i| is_active(i))
    }
}

pub fn active_group(groups: &[OpenGroup], is_active: impl Fn(usize) -> bool) -> Option<usize> {
    groups.iter().position(|g| g.is_active(&is_active))
}

/// Active window of the group, else the last (newest) one.
pub fn switch_target(group: &OpenGroup, is_active: impl Fn(usize) -> bool) -> Option<usize> {
    group
        .windows
        .iter()
        .copied()
        .find(|&i| is_active(i))
        .or_else(|| group.windows.last().copied())
}

pub fn count_label(n: usize) -> Option<String> {
    match n {
        0 | 1 => None,
        2..=9 => Some(n.to_string()),
        _ => Some("9+".to_string()),
    }
}

#[cfg(test)]
mod tests;
