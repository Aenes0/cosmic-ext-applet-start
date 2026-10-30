use super::*;
use crate::menu::open_strip;

fn keys(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn grouped(list: &[&str]) -> Vec<OpenGroup> {
    group(&keys(list), |k| k.clone())
}

/// Room for 2 icons when there are more than 3 groups, else for all.
fn cap(n: usize) -> usize {
    if n > 3 { 2 } else { n }
}

#[test]
fn two_windows_of_one_app_and_one_of_another_make_two_positions() {
    let groups = grouped(&["app:a", "app:b", "app:a"]);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].len(), 2);
    // The strip counts groups: two fit, no arrows, offset stays at the start.
    let (offset, _) = next_offset(2, &groups, None, None, 0, cap);
    assert_eq!(offset, 0);
    assert!(!open_strip::can_next(groups.len(), cap(groups.len()), offset));
}

#[test]
fn switching_window_inside_the_same_group_keeps_the_offset() {
    let groups = grouped(&["app:a", "app:b", "app:c", "app:d", "app:a"]);
    assert_eq!(groups.len(), 4);
    // The user paged to the end (offset 2); the active group "a" is at 0.
    let (offset, key) = next_offset(4, &groups, Some(0), Some("app:a"), 2, cap);
    assert_eq!(offset, 2);
    assert_eq!(key.as_deref(), Some("app:a"));
}

#[test]
fn switching_to_a_group_outside_the_page_reveals_it() {
    let groups = grouped(&["app:a", "app:b", "app:c", "app:d"]);
    let (offset, key) = next_offset(4, &groups, Some(3), Some("app:a"), 0, cap);
    assert_eq!(key.as_deref(), Some("app:d"));
    assert!(open_strip::visible_range(4, cap(4), offset).contains(&3));
}

#[test]
fn small_capacity_change_reveals_the_active_group() {
    // 3 groups fit (cap 3); a 4th makes the capacity 2: the active (3rd) stays in view.
    let groups = grouped(&["app:a", "app:b", "app:c", "app:d"]);
    let (offset, _) = next_offset(3, &groups, Some(2), Some("app:c"), 0, cap);
    assert!(open_strip::visible_range(4, cap(4), offset).contains(&2));
}

#[test]
fn no_active_group_clamps_the_offset() {
    let groups = grouped(&["app:a", "app:b"]);
    let (offset, key) = next_offset(4, &groups, None, Some("app:z"), 9, cap);
    assert_eq!(offset, 0);
    assert_eq!(key, None);
}

fn og(key: &str, windows: &[usize]) -> OpenGroup {
    OpenGroup { key: key.into(), windows: windows.to_vec() }
}

#[test]
fn rekey_map_follows_a_key_that_changed() {
    let old = [og("id:x", &[0, 2]), og("app:b", &[1])];
    let new = [og("app:x", &[0, 2]), og("app:b", &[1])];
    let map = rekey_map(&old, &new);
    assert_eq!(map.get("id:x").map(String::as_str), Some("app:x"));
    // Same key: absent or itself.
    assert!(map.get("app:b").is_none_or(|k| k == "app:b"));
}

#[test]
fn rekey_map_ignores_a_group_that_vanished() {
    let old = [og("id:x", &[0]), og("app:b", &[1])];
    let new = [og("app:b", &[1])];
    assert!(!rekey_map(&old, &new).contains_key("id:x"));
}

#[test]
fn an_unchanged_grouping_changes_nothing() {
    let g = [og("app:a", &[0]), og("app:b", &[1])];
    assert!(rekey_map(&g, &g).iter().all(|(k, v)| v == k));
}

#[test]
fn a_group_menu_without_its_group_is_an_orphan() {
    let exists = |k: &str| k == "app:a";
    assert!(orphan_group_menu(Some(&Target::OpenGroup("app:z".into())), exists));
    assert!(!orphan_group_menu(Some(&Target::OpenGroup("app:a".into())), exists));
    assert!(!orphan_group_menu(Some(&Target::App("app:z".into())), exists));
    assert!(!orphan_group_menu(None, exists));
}

#[test]
fn click_on_a_single_window_group_picks_that_window() {
    let group = OpenGroup { key: "app:a".into(), windows: vec![4] };
    assert_eq!(click_window(&group, |_| false), Some(4));
}

#[test]
fn click_on_a_many_windows_group_picks_the_active_else_the_last() {
    let group = OpenGroup { key: "app:a".into(), windows: vec![1, 3, 5] };
    assert_eq!(click_window(&group, |i| i == 3), Some(3));
    assert_eq!(click_window(&group, |_| false), Some(5));
}

#[test]
fn menu_of_a_single_window_closes_one() {
    assert_eq!(
        group_menu_items(1, true),
        vec![GroupItem::Switch, GroupItem::NewWindow, GroupItem::Close]
    );
}

#[test]
fn menu_of_many_windows_closes_all() {
    assert_eq!(
        group_menu_items(3, true),
        vec![GroupItem::Switch, GroupItem::NewWindow, GroupItem::CloseAll]
    );
}

#[test]
fn new_window_only_for_a_known_app() {
    assert_eq!(group_menu_items(1, false), vec![GroupItem::Switch, GroupItem::Close]);
    assert_eq!(group_menu_items(2, false), vec![GroupItem::Switch, GroupItem::CloseAll]);
}
