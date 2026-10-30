use super::*;

const IDS: [&str; 6] = ["a", "b", "c", "d", "e", "f"];
const CATS: [usize; 6] = [5, 5, 5, 0, 0, 9];

fn apps() -> Vec<AppKey<'static>> {
    IDS.iter().zip(CATS).map(|(id, category)| AppKey { id, category }).collect()
}

fn s(t: &str) -> String {
    t.to_string()
}

fn base() -> Groups {
    Groups { user: vec![(1, s("One")), (7, s("Seven"))], next: 8, ..Groups::default() }
}

fn ids(g: &Groups, gr: GroupRef) -> Vec<&'static str> {
    members_ids(g, gr)
}

fn members_ids(g: &Groups, gr: GroupRef) -> Vec<&'static str> {
    g.members(gr, &apps()).into_iter().map(|i| IDS[i]).collect()
}

#[test]
fn key_parse_roundtrip() {
    for (n, k) in CATEGORY_KEYS.iter().enumerate() {
        let g = GroupRef::Category(n);
        assert_eq!(g.key(), format!("cat:{k}"));
        assert_eq!(GroupRef::parse(&g.key()), Some(g));
    }
    assert_eq!(GroupRef::Category(99).key(), "cat:other");
    assert_eq!(GroupRef::User(7).key(), "usr:7");
    assert_eq!(GroupRef::parse("usr:7"), Some(GroupRef::User(7)));
    assert_eq!(GroupRef::parse("usr:4294967295"), Some(GroupRef::User(u32::MAX)));
    assert_eq!(GroupRef::Loose.key(), "none");
    assert_eq!(GroupRef::parse("none"), Some(GroupRef::Loose));
}

#[test]
fn parse_rejects_garbage() {
    for bad in [
        "cat:5", "cat:GAMES", "cat:", "cat:x", "usr:", "usr:x", "usr:-1", "usr:1.5", "usr: 1",
        "", " none", "none ", "group:cat:games", "cat:games:1", "usr:4294967296", "usr:+1",
    ] {
        assert_eq!(GroupRef::parse(bad), None, "{bad:?}");
    }
}

#[test]
fn pin_roundtrip() {
    assert_eq!(GroupRef::Category(5).pin(), Some(s("group:cat:games")));
    assert_eq!(GroupRef::User(3).pin(), Some(s("group:usr:3")));
    assert_eq!(GroupRef::Loose.pin(), None);
    assert!(GroupRef::User(3).pinnable() && GroupRef::Category(0).pinnable());
    assert!(!GroupRef::Loose.pinnable());
    assert_eq!(GroupRef::from_pin("group:cat:games"), Some(GroupRef::Category(5)));
    assert_eq!(GroupRef::from_pin("group:usr:3"), Some(GroupRef::User(3)));
    assert_eq!(GroupRef::from_pin("group:none"), None);
    assert_eq!(GroupRef::from_pin("cat:games"), None);
    assert_eq!(GroupRef::from_pin("group:"), None);
    assert_eq!(GroupRef::from_pin("group:group:cat:games"), None);
}

#[test]
fn from_stored_normalizes() {
    let g = Groups::from_stored(
        vec![(1, s("A")), (1, s("B")), (2, s("C"))],
        vec![
            (s("a"), s("usr:1")),
            (s("a"), s("usr:2")),
            (s("b"), s("bogus")),
            (s("c"), s("usr:9")),
            (s("d"), s("none")),
        ],
        vec![
            (s("usr:1"), vec![s("x"), s("y"), s("x")]),
            (s("usr:1"), vec![s("z")]),
            (s("usr:9"), vec![s("q")]),
            (s("cat:5"), vec![s("q")]),
            (s("none"), vec![s("w")]),
        ],
    );
    assert_eq!(g.user, vec![(1, s("A")), (2, s("C"))]);
    assert_eq!(g.place, vec![(s("a"), GroupRef::User(1)), (s("d"), GroupRef::Loose)]);
    assert_eq!(
        g.order,
        vec![
            (GroupRef::User(1), vec![s("x"), s("y")]),
            (GroupRef::Loose, vec![s("w")]),
        ]
    );
}

#[test]
fn stored_roundtrip() {
    let g = base()
        .place(apps()[0], GroupRef::User(7), None, &apps())
        .place(apps()[3], GroupRef::Loose, None, &apps());
    let (u, p, o) = g.to_stored();
    assert_eq!(p[0], (s("a"), s("usr:7")));
    assert_eq!(Groups::from_stored(u, p, o).with_next(g.next), g);
}

#[test]
fn members_explicit_then_rest() {
    let mut g = base();
    g.order = vec![(GroupRef::Category(5), vec![s("c"), s("old"), s("d"), s("a")])];
    // "d" is not a member of games; "old" is not installed.
    assert_eq!(ids(&g, GroupRef::Category(5)), vec!["c", "a", "b"]);
    assert_eq!(ids(&g, GroupRef::Category(0)), vec!["d", "e"]);
}

#[test]
fn visible_skips_empty_and_orders() {
    let g = base().place(apps()[5], GroupRef::Loose, None, &apps());
    let g = g.place(apps()[0], GroupRef::User(7), None, &apps());
    let v = g.visible(&apps());
    let keys: Vec<GroupRef> = v.iter().map(|(g, _)| *g).collect();
    assert_eq!(
        keys,
        vec![GroupRef::User(7), GroupRef::Category(0), GroupRef::Category(5), GroupRef::Loose]
    );
    assert_eq!(v[0].1, vec![0]);
    assert_eq!(v[2].1, vec![1, 2]);
    assert_eq!(v[3].1, vec![5]);
}

#[test]
fn exists_and_name() {
    let g = base();
    assert!(g.exists(GroupRef::User(7)) && !g.exists(GroupRef::User(99)));
    assert!(g.exists(GroupRef::Category(9)) && !g.exists(GroupRef::Category(10)));
    assert!(g.exists(GroupRef::Loose));
    assert_eq!(g.name(7), Some("Seven"));
    assert_eq!(g.name(8), None);
}

#[test]
fn place_reorder_left_right_end() {
    let a = apps();
    let games = GroupRef::Category(5); // a b c
    let g = base().place(a[2], games, Some(0), &a);
    assert_eq!(ids(&g, games), vec!["c", "a", "b"]);
    let g = base().place(a[0], games, Some(2), &a); // before "c": a stays before c
    assert_eq!(ids(&g, games), vec!["b", "a", "c"]);
    let g = base().place(a[0], games, None, &a);
    assert_eq!(ids(&g, games), vec!["b", "c", "a"]);
    let g = base().place(a[0], games, Some(1), &a); // before itself's next: no-op
    assert_eq!(ids(&g, games), vec!["a", "b", "c"]);
    assert!(g.place.is_empty());
}

#[test]
fn place_moves_between_groups() {
    let a = apps();
    let g = base().place(a[0], GroupRef::Category(0), Some(1), &a);
    assert_eq!(ids(&g, GroupRef::Category(0)), vec!["d", "a", "e"]);
    assert_eq!(ids(&g, GroupRef::Category(5)), vec!["b", "c"]);
    assert_eq!(g.group_of(a[0]), GroupRef::Category(0));
    let g = g.place(a[0], GroupRef::Loose, None, &a);
    assert_eq!(ids(&g, GroupRef::Loose), vec!["a"]);
    assert_eq!(g.group_of(a[0]), GroupRef::Loose);
    assert!(g.order.iter().all(|(r, l)| *r == GroupRef::Loose || !l.contains(&s("a"))));
}

#[test]
fn place_back_home_removes_record() {
    let a = apps();
    let g = base().place(a[0], GroupRef::User(1), None, &a);
    assert_eq!(g.place.len(), 1);
    let g = g.place(a[0], GroupRef::Category(5), None, &a);
    assert!(g.place.is_empty());
    assert_eq!(g.group_of(a[0]), GroupRef::Category(5));
}

#[test]
fn place_before_past_end_and_unknown_group() {
    let a = apps();
    let g = base().place(a[3], GroupRef::Category(5), Some(500), &a);
    assert_eq!(ids(&g, GroupRef::Category(5)), vec!["a", "b", "c", "d"]);
    let same = g.place(a[0], GroupRef::User(99), Some(0), &a);
    assert_eq!(same, g);
}

#[test]
fn place_is_immutable() {
    let a = apps();
    let g = base();
    let before = g.clone();
    let _ = g.place(a[0], GroupRef::Loose, None, &a);
    let _ = g.forget("a");
    assert_eq!(g, before);
}

#[test]
fn forget_clears_everything() {
    let a = apps();
    let g = base().place(a[0], GroupRef::User(1), None, &a).forget("a");
    assert!(g.place.is_empty());
    assert!(g.order.iter().all(|(_, l)| !l.contains(&s("a"))));
    assert_eq!(g.group_of(a[0]), GroupRef::Category(5));
}

#[test]
fn every_app_in_exactly_one_group() {
    let a = apps();
    let targets = [
        GroupRef::User(1),
        GroupRef::User(7),
        GroupRef::User(99),
        GroupRef::Loose,
        GroupRef::Category(0),
        GroupRef::Category(5),
        GroupRef::Category(9),
    ];
    let mut seed: u64 = 12345;
    let mut next = |n: usize| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as usize) % n
    };
    let mut g = base();
    for step in 0..600 {
        let app = a[next(a.len())];
        let other = a[next(a.len())];
        let roll = next(12);
        if roll == 0 {
            g = g.forget(app.id);
        } else if roll == 1 {
            g = g.merge(app, other, "Mix", &a).0;
        } else if roll == 2 {
            g = g.delete([1, 7, 8, 9][next(4)]);
        } else if roll == 3 {
            g = g.emptied().0;
        } else {
            let before = if next(3) == 0 { None } else { Some(next(8)) };
            g = g.place(app, targets[next(targets.len())], before, &a);
        }
        let (u, p, o) = g.to_stored();
        assert_eq!(Groups::from_stored(u, p, o).with_next(g.next), g, "step {step}");
        let mut seen = vec![0; a.len()];
        for (_, m) in g.visible(&a) {
            for i in m {
                seen[i] += 1;
            }
        }
        assert_eq!(seen, vec![1; a.len()], "step {step}");
    }
}

fn named(g: &Groups, id: u32) -> String {
    g.name(id).unwrap_or("").to_string()
}

#[test]
fn from_stored_next_and_with_next() {
    let g = Groups::from_stored(vec![(3, s("A")), (9, s("B"))], vec![], vec![]);
    assert_eq!(g.next, 10);
    assert_eq!(Groups::from_stored(vec![], vec![], vec![]).next, 1);
    assert_eq!(g.clone().with_next(20).next, 20);
    assert_eq!(g.with_next(2).next, 10);
}

#[test]
fn from_stored_drops_order_of_missing_user() {
    let g = Groups::from_stored(
        vec![(1, s("A"))],
        vec![],
        vec![(s("usr:2"), vec![s("a")]), (s("usr:1"), vec![s("b")])],
    );
    assert_eq!(g.order, vec![(GroupRef::User(1), vec![s("b")])]);
}

#[test]
fn merge_creates_group_in_order() {
    let a = apps();
    let (g, r) = base().merge(a[1], a[0], "Pair", &a);
    assert_eq!(r, GroupRef::User(8));
    assert_eq!(g.next, 9);
    assert_eq!(named(&g, 8), "Pair");
    assert_eq!(ids(&g, r), vec!["a", "b"]);
    assert_eq!(g.group_of(a[0]), r);
}

#[test]
fn merge_onto_user_group_inserts_after() {
    let a = apps();
    let g = base()
        .place(a[0], GroupRef::User(1), None, &a)
        .place(a[3], GroupRef::User(1), None, &a)
        .place(a[4], GroupRef::User(1), None, &a);
    let (g, r) = g.merge(a[5], a[0], "X", &a);
    assert_eq!(r, GroupRef::User(1));
    assert_eq!(ids(&g, r), vec!["a", "f", "d", "e"]);
    assert_eq!(g.next, 8);
    let (g, _) = g.merge(a[4], a[0], "X", &a);
    assert_eq!(ids(&g, r), vec!["a", "e", "f", "d"]);
}

#[test]
fn merge_with_itself_is_noop() {
    let a = apps();
    let g = base().place(a[0], GroupRef::User(1), None, &a);
    let (m, r) = g.merge(a[0], a[0], "X", &a);
    assert_eq!(m, g);
    assert_eq!(r, GroupRef::User(1));
    let (m, r) = base().merge(a[2], a[2], "X", &a);
    assert_eq!((m, r), (base(), GroupRef::Category(5)));
}

#[test]
fn merge_leaves_previous_group_and_it_becomes_empty() {
    let a = apps();
    let g = base().place(a[0], GroupRef::User(1), None, &a);
    let (g, r) = g.merge(a[0], a[3], "Z", &a);
    assert_eq!(r, GroupRef::User(8));
    assert_eq!(ids(&g, r), vec!["d", "a"]);
    let (g, gone) = g.emptied();
    assert_eq!(gone, vec![GroupRef::User(1), GroupRef::User(7)]);
    assert_eq!(g.user.iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![8]);
}

#[test]
fn merge_empty_name_falls_back() {
    let a = apps();
    let (g, _) = base().merge(a[1], a[0], "   ", &a);
    assert_eq!(named(&g, 8), "Group");
    let (g, _) = base().merge(a[1], a[0], &"x".repeat(50), &a);
    assert_eq!(named(&g, 8).chars().count(), NAME_MAX);
}

#[test]
fn ids_never_reused() {
    let a = apps();
    let g = Groups::default();
    let (g, r1) = g.merge(a[1], a[0], "A", &a);
    let (g, r2) = g.merge(a[4], a[3], "B", &a);
    assert_eq!((r1, r2), (GroupRef::User(1), GroupRef::User(2)));
    let g = g.delete(2);
    let (g, r3) = g.merge(a[4], a[3], "C", &a);
    assert_eq!(r3, GroupRef::User(3));
    assert_eq!(g.next, 4);
}

#[test]
fn with_next_applies_to_new_groups() {
    let a = apps();
    let g = Groups::from_stored(vec![(1, s("A"))], vec![], vec![]).with_next(10);
    let (_, r) = g.merge(a[1], a[0], "N", &a);
    assert_eq!(r, GroupRef::User(10));
}

#[test]
fn rename_cases() {
    let g = base();
    assert_eq!(named(&g.rename(1, "  Hi  "), 1), "Hi");
    assert_eq!(g.rename(1, ""), g);
    assert_eq!(g.rename(1, "   \t "), g);
    assert_eq!(g.rename(99, "Hi"), g);
    assert_eq!(named(&g.rename(1, &"y".repeat(40)), 1), "y".repeat(32));
    let emoji = "\u{e9}\u{1F600}".repeat(20);
    let n = named(&g.rename(1, &emoji), 1);
    assert_eq!(n.chars().count(), 32);
    assert_eq!(n, emoji.chars().take(32).collect::<String>());
    assert_eq!(named(&g.rename(1, "Hi"), 7), "Seven");
}

#[test]
fn delete_sends_apps_loose() {
    let a = apps();
    let g = base()
        .place(a[0], GroupRef::User(1), None, &a)
        .place(a[1], GroupRef::User(1), None, &a)
        .place(a[3], GroupRef::User(7), None, &a);
    let d = g.delete(1);
    assert!(!d.exists(GroupRef::User(1)));
    assert_eq!(d.group_of(a[0]), GroupRef::Loose);
    assert_eq!(d.group_of(a[1]), GroupRef::Loose);
    assert_eq!(d.group_of(a[3]), GroupRef::User(7));
    assert!(d.order.iter().all(|(r, _)| *r != GroupRef::User(1)));
    assert_eq!(d.next, g.next);
    assert_eq!(ids(&d, GroupRef::User(7)), vec!["d"]);
    assert_eq!(g.delete(99), g);
}

#[test]
fn emptied_only_user_groups_without_place() {
    let a = apps();
    let g = base().place(a[0], GroupRef::User(7), None, &a);
    let (e, gone) = g.emptied();
    assert_eq!(gone, vec![GroupRef::User(1)]);
    assert!(e.exists(GroupRef::User(7)) && !e.exists(GroupRef::User(1)));
    assert!(e.order.iter().all(|(r, _)| *r != GroupRef::User(1)));
    assert!(e.exists(GroupRef::Category(3)));
    let (same, none) = e.emptied();
    assert!(none.is_empty());
    assert_eq!(same, e);
}

#[test]
fn next_name_skips_taken() {
    let mut g = base();
    assert_eq!(g.next_name("Novo grupo"), "Novo grupo");
    g.user.push((2, s("Novo grupo")));
    assert_eq!(g.next_name("Novo grupo"), "Novo grupo 2");
    g.user.push((3, s("Novo grupo 2")));
    g.user.push((4, s("Novo grupo 3")));
    assert_eq!(g.next_name("Novo grupo"), "Novo grupo 4");
}

#[test]
fn unpin_removes_gone_groups() {
    let pinned = vec![s("firefox.desktop"), s("group:usr:1"), s("group:cat:games"), s("group:usr:7")];
    let out = unpin(&pinned, &[GroupRef::User(1), GroupRef::Loose]);
    assert_eq!(out, vec![s("firefox.desktop"), s("group:cat:games"), s("group:usr:7")]);
    assert_eq!(unpin(&pinned, &[]), pinned);
}
