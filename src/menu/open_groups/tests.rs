use super::*;

struct W {
    id: &'static str,
    app: Option<&'static str>,
    app_id: &'static str,
    active: bool,
}

fn w(id: &'static str, app: Option<&'static str>, app_id: &'static str, active: bool) -> W {
    W { id, app, app_id, active }
}

fn key(w: &W) -> String {
    group_key(w.app, w.app_id, w.id)
}

#[test]
fn groups_interleaved_in_first_appearance_order() {
    let ws = [
        w("1", Some("a"), "x", false),
        w("2", Some("b"), "y", false),
        w("3", Some("a"), "x", false),
        w("4", Some("a"), "x", false),
    ];
    let g = group(&ws, key);
    assert_eq!(g.len(), 2);
    assert_eq!(g[0], OpenGroup { key: "app:a".into(), windows: vec![0, 2, 3] });
    assert_eq!(g[1], OpenGroup { key: "app:b".into(), windows: vec![1] });
}

#[test]
fn unknown_app_uses_lowercase_app_id() {
    assert_eq!(group_key(None, "Firefox", "1"), "id:firefox");
    let ws = [w("1", None, "Firefox", false), w("2", None, "firefox", false)];
    assert_eq!(group(&ws, key).len(), 1);
}

#[test]
fn empty_app_id_never_groups() {
    let ws = [w("a", None, "", false), w("b", None, "", false)];
    let g = group(&ws, key);
    assert_eq!(g.len(), 2);
    assert_eq!(g[0].key, "window:a");
    assert_eq!(g[1].key, "window:b");
}

#[test]
fn known_app_wins_over_app_id() {
    assert_eq!(group_key(Some("org.app"), "Other", "1"), "app:org.app");
}

#[test]
fn active_group_and_is_active() {
    let ws = [w("1", Some("a"), "", false), w("2", Some("b"), "", true)];
    let g = group(&ws, key);
    assert_eq!(active_group(&g, |i| ws[i].active), Some(1));
    assert_eq!(active_group(&g, |_| false), None);
    assert!(g[1].is_active(|i| ws[i].active));
    assert!(!g[0].is_active(|i| ws[i].active));
}

#[test]
fn switch_target_prefers_active_else_last() {
    let g = OpenGroup { key: "a".into(), windows: vec![3, 5, 7] };
    assert_eq!(switch_target(&g, |i| i == 5), Some(5));
    assert_eq!(switch_target(&g, |_| false), Some(7));
    let e = OpenGroup { key: "e".into(), windows: vec![] };
    assert_eq!(switch_target(&e, |_| false), None);
    assert_eq!(e.len(), 0);
}

#[test]
fn count_label_rules() {
    assert_eq!(count_label(0), None);
    assert_eq!(count_label(1), None);
    assert_eq!(count_label(2), Some("2".into()));
    assert_eq!(count_label(9), Some("9".into()));
    assert_eq!(count_label(10), Some("9+".into()));
}

#[test]
fn empty_list_gives_no_groups() {
    let ws: [W; 0] = [];
    assert_eq!(group(&ws, key), vec![]);
}

#[test]
fn collision_window_prefix_in_app_id_vs_windowless_id() {
    let ws = [w("1", None, "window:abc", false), w("abc", None, "", false)];
    let g = group(&ws, key);
    assert_eq!(g.len(), 2);
    assert_ne!(g[0].key, g[1].key);
}

#[test]
fn collision_app_prefix_in_app_id_vs_known_app() {
    let ws = [w("1", Some("firefox"), "x", false), w("2", None, "app:firefox", false)];
    let g = group(&ws, key);
    assert_eq!(g.len(), 2);
    assert_ne!(g[0].key, g[1].key);
}

#[test]
fn known_and_unknown_origins_never_merge() {
    // Different origins stay separate by design, even with the same id.
    let ws = [
        w("1", Some("org.mozilla.firefox"), "x", false),
        w("2", None, "org.mozilla.firefox", false),
    ];
    let g = group(&ws, key);
    assert_eq!(g.len(), 2);
    assert_eq!(g[0].key, "app:org.mozilla.firefox");
    assert_eq!(g[1].key, "id:org.mozilla.firefox");
}
