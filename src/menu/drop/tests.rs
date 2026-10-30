use super::*;
use crate::menu::drag::Slot;

const KEYS: [AppKey<'static>; 4] = [
    AppKey { id: "a", category: 0 },
    AppKey { id: "org.x.Firefox", category: 2 },
    AppKey { id: "b", category: 0 },
    AppKey { id: "c", category: 0 },
];

fn find(s: &str) -> Option<usize> {
    let s = if s == "firefox" { "org.x.Firefox" } else { s };
    KEYS.iter().position(|k| k.id == s)
}

fn same(a: &str, b: &str) -> bool {
    a == b
}

fn v(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn run(action: Action, pinned: &[String]) -> Option<Dropped> {
    resolve_drop(&action, pinned, &Groups::default(), &KEYS, &find, &same)
}

fn place_app(id: &str, into: GroupRef, before: Option<usize>) -> Action {
    Action::Place { item: Item::App(id.into()), into, before }
}

#[test]
fn cancel_is_none() {
    assert_eq!(run(Action::Cancel, &v(&["a"])), None);
}

#[test]
fn reorder_delegates() {
    let d = run(Action::Reorder { from: 0, to: Slot::End }, &v(&["a", "b"])).unwrap();
    assert_eq!(d.pinned, v(&["b", "a"]));
    assert_eq!(d.groups, Groups::default());
}

#[test]
fn pin_with_and_without_duplicate() {
    let new = run(Action::Pin { app: "c".into(), to: Slot::End }, &v(&["a", "b"])).unwrap();
    assert_eq!(new.pinned, v(&["a", "b", "c"]));
    let dup = run(Action::Pin { app: "a".into(), to: Slot::End }, &v(&["a", "b"])).unwrap();
    assert_eq!(dup.pinned, v(&["b", "a"]));
}

#[test]
fn place_app_into_category_and_loose() {
    let d = run(place_app("a", GroupRef::Category(2), None), &v(&["a"])).unwrap();
    assert_eq!(d.groups.group_of(KEYS[0]), GroupRef::Category(2));
    assert_eq!(d.pinned, v(&["a"]));
    let l = run(place_app("a", GroupRef::Loose, None), &[]).unwrap();
    assert_eq!(l.groups.group_of(KEYS[0]), GroupRef::Loose);
}

#[test]
fn place_app_before_orders_members() {
    let d = run(place_app("c", GroupRef::Category(0), Some(0)), &[]).unwrap();
    assert_eq!(d.groups.members(GroupRef::Category(0), &KEYS), vec![3, 0, 2]);
}

#[test]
fn place_unknown_app_is_none() {
    assert_eq!(run(place_app("zzz", GroupRef::Loose, None), &[]), None);
    assert_eq!(run(place_app("firefox", GroupRef::Loose, None), &[]), None);
}

#[test]
fn place_pinned_is_absorbed() {
    let pinned = v(&["a", "firefox", "b"]);
    let action = Action::Place { item: Item::Pinned(1), into: GroupRef::Category(0), before: None };
    let d = run(action, &pinned).unwrap();
    assert_eq!(d.pinned, v(&["a", "b"]));
    assert_eq!(d.groups.group_of(KEYS[1]), GroupRef::Category(0));
    assert_eq!(pinned, v(&["a", "firefox", "b"]));
}

#[test]
fn place_pinned_group_or_out_of_range_is_none() {
    let pinned = v(&["a", "group:cat:games"]);
    let g = Action::Place { item: Item::Pinned(1), into: GroupRef::Category(0), before: None };
    assert_eq!(run(g, &pinned), None);
    let o = Action::Place { item: Item::Pinned(9), into: GroupRef::Category(0), before: None };
    assert_eq!(run(o, &pinned), None);
    let u = Action::Place { item: Item::Pinned(0), into: GroupRef::Loose, before: None };
    assert_eq!(run(u, &v(&["nope"])), None);
}

#[test]
fn ungroup_goes_loose() {
    let d = run(Action::Ungroup { app: "a".into() }, &v(&["a"])).unwrap();
    assert_eq!(d.groups.group_of(KEYS[0]), GroupRef::Loose);
    assert_eq!(d.pinned, v(&["a"]));
    assert_eq!(run(Action::Ungroup { app: "zzz".into() }, &[]), None);
}

#[test]
fn inputs_are_not_mutated() {
    let groups = Groups::default();
    let pinned = v(&["a", "b"]);
    let _ = resolve_drop(&place_app("a", GroupRef::Loose, None), &pinned, &groups, &KEYS, &find, &same);
    assert_eq!(groups, Groups::default());
    assert_eq!(pinned, v(&["a", "b"]));
}

fn merge(item: Item, onto: usize) -> Action {
    Action::Merge { item, onto }
}

fn with_user(apps: &[&str]) -> Groups {
    let mut g = Groups::default();
    g.user.push((1, "G".into()));
    g.next = 2;
    for a in apps {
        g.place.push((a.to_string(), GroupRef::User(1)));
    }
    g
}

fn run_g(action: Action, pinned: &[String], groups: &Groups) -> Option<Dropped> {
    resolve_drop(&action, pinned, groups, &KEYS, &find, &same)
}

#[test]
fn merge_app_onto_loose_creates_group() {
    let d = run(merge(Item::App("a".into()), 2), &[]).unwrap();
    assert_eq!(d.open, Some(GroupRef::User(1)));
    assert_eq!(d.groups.group_of(KEYS[0]), GroupRef::User(1));
    assert_eq!(d.groups.group_of(KEYS[2]), GroupRef::User(1));
}

#[test]
fn merge_onto_pinned_app_pins_the_group() {
    let d = run(merge(Item::App("a".into()), 2), &v(&["x", "b", "y"])).unwrap();
    assert_eq!(d.pinned, v(&["x", "group:usr:1", "y"]));
}

#[test]
fn merge_pinned_onto_pinned_leaves_one_entry() {
    let d = run(merge(Item::Pinned(0), 2), &v(&["a", "x", "b"])).unwrap();
    assert_eq!(d.pinned, v(&["x", "group:usr:1"]));
    assert_eq!(d.open, Some(GroupRef::User(1)));
    let e = run(merge(Item::Pinned(2), 0), &v(&["a", "x", "b"])).unwrap();
    assert_eq!(e.pinned, v(&["group:usr:1", "x"]));
}

#[test]
fn merge_onto_app_in_user_group_joins_it() {
    let g = with_user(&["b", "c"]);
    let pinned = v(&["a", "group:usr:1"]);
    let d = run_g(merge(Item::App("a".into()), 2), &pinned, &g).unwrap();
    assert_eq!(d.open, None);
    assert_eq!(d.groups.group_of(KEYS[0]), GroupRef::User(1));
    assert_eq!(d.pinned, pinned);
}

#[test]
fn merge_invalid_is_none() {
    assert_eq!(run(merge(Item::App("a".into()), 0), &[]), None);
    assert_eq!(run(merge(Item::Pinned(0), 2), &v(&["group:cat:games"])), None);
    assert_eq!(run(merge(Item::App("a".into()), 9), &[]), None);
}

#[test]
fn emptied_user_group_is_removed_and_unpinned() {
    let g = with_user(&["b"]);
    let pinned = v(&["group:usr:1", "a"]);
    let d = run_g(place_app("b", GroupRef::Loose, None), &pinned, &g).unwrap();
    assert!(!d.groups.exists(GroupRef::User(1)));
    assert_eq!(d.pinned, v(&["a"]));
    let u = run_g(Action::Ungroup { app: "b".into() }, &pinned, &g).unwrap();
    assert!(!u.groups.exists(GroupRef::User(1)));
    assert_eq!(u.pinned, v(&["a"]));
}

#[test]
fn emptied_category_stays() {
    let mut g = Groups::default();
    g.place.push(("b".into(), GroupRef::Category(5)));
    let d = run_g(place_app("b", GroupRef::Loose, None), &v(&["group:cat:games"]), &g).unwrap();
    assert!(d.groups.exists(GroupRef::Category(5)));
    assert_eq!(d.pinned, v(&["group:cat:games"]));
}

#[test]
fn delete_group_frees_apps_and_unpins() {
    let g = with_user(&["b"]);
    let (pinned, groups) = delete_group(&v(&["group:usr:1", "a"]), &g, 1);
    assert_eq!(pinned, v(&["a"]));
    assert!(!groups.exists(GroupRef::User(1)));
    assert_eq!(groups.group_of(KEYS[2]), GroupRef::Loose);
}

#[test]
fn unpin_app_and_group_entries() {
    let pinned = v(&["a", "group:3", "b"]);
    let app = run(Action::Unpin { index: 0 }, &pinned).unwrap();
    assert_eq!(app.pinned, v(&["group:3", "b"]));
    assert_eq!(app.open, None);
    let group = run(Action::Unpin { index: 1 }, &pinned).unwrap();
    assert_eq!(group.pinned, v(&["a", "b"]));
    assert_eq!(group.groups, Groups::default());
}

#[test]
fn unpin_invalid_index_is_none() {
    assert_eq!(run(Action::Unpin { index: 5 }, &v(&["a"])), None);
}

#[test]
fn unpin_does_not_mutate_input() {
    let pinned = v(&["a", "b"]);
    let _ = run(Action::Unpin { index: 0 }, &pinned);
    assert_eq!(pinned, v(&["a", "b"]));
}
