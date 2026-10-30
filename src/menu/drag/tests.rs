use super::*;
use cosmic::iced::Size;
use super::super::groups::GroupRef;

fn p(x: f32, y: f32) -> Point {
    Point::new(x, y)
}
fn r(x: f32, w: f32) -> Rectangle {
    Rectangle::new(p(x, 0.0), Size::new(w, 10.0))
}
fn t(slot: Slot, rect: Rectangle) -> Target {
    Target { slot, rect, layer: Layer::Tile }
}
fn v(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}
fn eq(a: &str, b: &str) -> bool {
    a == b
}
fn alias(a: &str, b: &str) -> bool {
    let (a, b) = (a.to_lowercase(), b.to_lowercase());
    a == b || a.ends_with(&format!(".{b}")) || b.ends_with(&format!(".{a}"))
}
fn reo(from: usize, to: Slot) -> Action {
    Action::Reorder { from, to }
}

#[test]
fn threshold_not_crossed() {
    let d = Drag::arm(Source::Pinned(0), p(0.0, 0.0)).moved(p(5.9, 0.0), &[], Instant::now());
    assert!(!d.is_active());
}

#[test]
fn threshold_crossed_and_euclidean() {
    assert!(Drag::arm(Source::Pinned(0), p(0.0, 0.0)).moved(p(6.1, 0.0), &[], Instant::now()).is_active());
    // 4.5,4.5 -> 6.36 > 6 although each axis < 6
    assert!(Drag::arm(Source::Pinned(0), p(0.0, 0.0)).moved(p(4.5, 4.5), &[], Instant::now()).is_active());
    assert!(!Drag::arm(Source::Pinned(0), p(0.0, 0.0)).moved(p(4.0, 4.0), &[], Instant::now()).is_active());
}

#[test]
fn armed_release_is_cancel() {
    assert_eq!(Drag::arm(Source::App("a".into()), p(0.0, 0.0)).released(), Action::Cancel);
}

#[test]
fn drag_release_without_target_is_cancel() {
    let d = Drag::arm(Source::Pinned(0), p(0.0, 0.0)).moved(p(50.0, 50.0), &[], Instant::now());
    assert_eq!(d.released(), Action::Cancel);
}

#[test]
fn release_over_target_gives_action() {
    let t = [t(Slot::Before(2), r(40.0, 20.0))];
    let d = Drag::arm(Source::Pinned(0), p(0.0, 0.0)).moved(p(50.0, 5.0), &t, Instant::now());
    assert_eq!(d.released(), reo(0, Slot::Before(2)));
    let d = Drag::arm(Source::App("x".into()), p(0.0, 0.0)).moved(p(50.0, 5.0), &t, Instant::now());
    assert_eq!(d.released(), Action::Pin { app: "x".into(), to: Slot::Before(2) });
}

#[test]
fn dragging_updates_over() {
    let t = [t(Slot::End, r(40.0, 20.0))];
    let d = Drag::arm(Source::Pinned(0), p(0.0, 0.0))
        .moved(p(50.0, 5.0), &t, Instant::now())
        .moved(p(100.0, 5.0), &t, Instant::now());
    assert_eq!(d, Drag::Dragging { source: Source::Pinned(0), at: p(100.0, 5.0), over: None, hover: None });
}

#[test]
fn reorder_left_right_end() {
    let l = v(&["a", "b", "c", "d"]);
    assert_eq!(apply(&l, &reo(3, Slot::Before(1)), &eq), v(&["a", "d", "b", "c"]));
    assert_eq!(apply(&l, &reo(0, Slot::Before(3)), &eq), v(&["b", "c", "a", "d"]));
    assert_eq!(apply(&l, &reo(0, Slot::End), &eq), v(&["b", "c", "d", "a"]));
    assert_eq!(apply(&l, &reo(1, Slot::Before(1)), &eq), l);
    assert_eq!(apply(&l, &reo(1, Slot::Before(2)), &eq), l);
    assert_eq!(l, v(&["a", "b", "c", "d"]));
}

#[test]
fn pin_before_and_end() {
    let l = v(&["a", "b"]);
    let pin = |to| Action::Pin { app: "n".into(), to };
    assert_eq!(apply(&l, &pin(Slot::Before(1)), &eq), v(&["a", "n", "b"]));
    assert_eq!(apply(&l, &pin(Slot::End), &eq), v(&["a", "b", "n"]));
    assert_eq!(apply(&l, &pin(Slot::Before(9)), &eq), v(&["a", "b", "n"]));
}

#[test]
fn pin_alias_reorders_without_duplicating() {
    let l = v(&["a", "org.x.App", "c"]);
    let act = Action::Pin { app: "app".into(), to: Slot::Before(0) };
    assert_eq!(apply(&l, &act, &alias), v(&["org.x.App", "a", "c"]));
}

#[test]
fn pin_exact_duplicate_not_duplicated() {
    let l = v(&["a", "b"]);
    let act = Action::Pin { app: "a".into(), to: Slot::End };
    assert_eq!(apply(&l, &act, &eq), v(&["b", "a"]));
}

#[test]
fn missing_from_and_cancel_unchanged() {
    let l = v(&["a", "b"]);
    assert_eq!(apply(&l, &reo(5, Slot::End), &eq), l);
    assert_eq!(apply(&l, &Action::Cancel, &eq), l);
}

/// Slot the pointer lands on, through a real drag.
fn at(point: Point, targets: &[Target]) -> Option<Slot> {
    over(&Drag::arm(Source::Pinned(0), p(-100.0, -100.0)).moved(point, targets, Instant::now()))
}

#[test]
fn over_adjacent_and_edge() {
    let t = [t(Slot::Before(0), r(0.0, 10.0)), t(Slot::Before(1), r(10.0, 10.0))];
    assert_eq!(at(p(5.0, 5.0), &t), Some(Slot::Before(0)));
    assert_eq!(at(p(10.0, 5.0), &t), Some(Slot::Before(1)));
    assert_eq!(at(p(20.0, 5.0), &t), None);
}

fn open_popup_targets() -> [Target; 4] {
    [
        popup(Layer::PopupTile, r(100.0, 20.0)),
        popup(Layer::PopupArea, r(100.0, 100.0)),
        t(Slot::Before(0), r(0.0, 500.0)),
        Target { slot: Slot::Body, rect: r(0.0, 1000.0), layer: Layer::Body },
    ]
}

fn member_drag(point: Point, targets: &[Target]) -> Drag {
    Drag::arm(Source::Member(g(), "a".into()), p(-100.0, -100.0)).moved(point, targets, Instant::now())
}

#[test]
fn open_popup_blocks_targets_below() {
    let all = open_popup_targets();
    let d = member_drag(p(110.0, 5.0), &all);
    assert_eq!(over(&d), Some(Slot::MemberEnd(g())));
    assert_eq!(d.released(), place(Item::App("a".into()), None));
    // outside the card, inside the margin
    let d = member_drag(p(100.0 - 10.0, 5.0), &all);
    assert_eq!(over(&d), None);
    assert_eq!(d.released(), Action::Cancel);
    // popup closed: the rest counts again
    let rest = without_popup(&all);
    let d = member_drag(p(100.0 - 10.0, 5.0), &rest);
    assert_eq!(over(&d), Some(Slot::Before(0)));
    let d = member_drag(p(700.0, 5.0), &rest);
    assert_eq!(over(&d), Some(Slot::Body));
    assert_eq!(d.released(), Action::Ungroup { app: "a".into() });
}

#[test]
fn cancel_returns_idle() {
    let d = Drag::arm(Source::Pinned(0), p(0.0, 0.0)).moved(p(50.0, 0.0), &[], Instant::now());
    assert!(d.cancel().is_idle());
}

#[test]
fn apply_with_new_slots_leaves_the_list() {
    let l = v(&["a", "b"]);
    for to in [Slot::Into(GroupRef::Loose), Slot::Body] {
        assert_eq!(apply(&l, &reo(0, to), &eq), l);
        assert_eq!(apply(&l, &Action::Pin { app: "n".into(), to }, &eq), l);
    }
}

#[test]
fn place_and_ungroup_leave_the_list() {
    let l = v(&["a", "b"]);
    let place = Action::Place { item: Item::App("a".into()), into: GroupRef::User(1), before: None };
    assert_eq!(apply(&l, &place, &eq), l);
    let place = Action::Place { item: Item::Pinned(0), into: GroupRef::Loose, before: Some(0) };
    assert_eq!(apply(&l, &place, &eq), l);
    assert_eq!(apply(&l, &Action::Ungroup { app: "a".into() }, &eq), l);
}

use std::time::Duration;

fn g() -> GroupRef {
    GroupRef::Category(1)
}

fn release(source: Source, slot: Slot) -> Action {
    let tg = [t(slot, r(40.0, 20.0))];
    Drag::arm(source, p(0.0, 0.0)).moved(p(50.0, 5.0), &tg, Instant::now()).released()
}

fn place(item: Item, before: Option<usize>) -> Action {
    Action::Place { item, into: g(), before }
}

#[test]
fn table_pinned() {
    let s = || Source::Pinned(2);
    assert_eq!(release(s(), Slot::Before(1)), reo(2, Slot::Before(1)));
    assert_eq!(release(s(), Slot::End), reo(2, Slot::End));
    assert_eq!(release(s(), Slot::Into(g())), place(Item::Pinned(2), None));
    for slot in [Slot::Member(g(), 0), Slot::MemberEnd(g()), Slot::Body] {
        assert_eq!(release(s(), slot), Action::Cancel);
    }
}

#[test]
fn table_app_and_member() {
    for source in [Source::App("a".into()), Source::Member(g(), "a".into())] {
        let s = || source.clone();
        assert_eq!(release(s(), Slot::Before(1)), Action::Pin { app: "a".into(), to: Slot::Before(1) });
        assert_eq!(release(s(), Slot::End), Action::Pin { app: "a".into(), to: Slot::End });
        assert_eq!(release(s(), Slot::Into(g())), place(Item::App("a".into()), None));
        assert_eq!(release(s(), Slot::Member(g(), 3)), place(Item::App("a".into()), Some(3)));
        assert_eq!(release(s(), Slot::MemberEnd(g())), place(Item::App("a".into()), None));
    }
    assert_eq!(release(Source::App("a".into()), Slot::Body), Action::Cancel);
    assert_eq!(release(Source::Member(g(), "a".into()), Slot::Body), Action::Ungroup { app: "a".into() });
}

#[test]
fn table_group_and_none() {
    let grp = Source::Group(g());
    let key = g().pin().unwrap();
    assert_eq!(release(grp.clone(), Slot::Before(1)), Action::Pin { app: key.clone(), to: Slot::Before(1) });
    assert_eq!(release(grp.clone(), Slot::End), Action::Pin { app: key, to: Slot::End });
    for slot in [Slot::Into(g()), Slot::Member(g(), 0), Slot::MemberEnd(g()), Slot::Body] {
        assert_eq!(release(grp.clone(), slot), Action::Cancel);
    }
    assert_eq!(release(Source::Group(GroupRef::Loose), Slot::End), Action::Cancel);
    for source in [Source::Pinned(0), Source::App("a".into()), Source::Member(g(), "a".into())] {
        let d = Drag::arm(source, p(0.0, 0.0)).moved(p(50.0, 5.0), &[], Instant::now());
        assert_eq!(d.released(), Action::Cancel);
    }
}

fn spec(slot: Slot, layer: Layer) -> Spec {
    Spec { slot, layer, zone: Zone::Whole }
}

#[test]
fn targets_sorted_stable_and_none_dropped() {
    let specs = vec![
        vec![spec(Slot::Body, Layer::Body)],
        vec![spec(Slot::Before(0), Layer::Tile), spec(Slot::Before(1), Layer::Tile)],
        vec![spec(Slot::Into(g()), Layer::Center)],
        vec![spec(Slot::End, Layer::Area)],
    ];
    let rects = [Some(r(0.0, 9.0)), Some(r(10.0, 9.0)), None, Some(r(20.0, 9.0))];
    let out = targets(&specs, &rects);
    let slots: Vec<Slot> = out.iter().map(|x| x.slot).collect();
    assert_eq!(slots, vec![Slot::Before(0), Slot::Before(1), Slot::End, Slot::Body]);
    assert_eq!(out[0].rect, r(10.0, 9.0));
}

#[test]
fn zone_and_center_numbers() {
    let rc = Rectangle::new(p(100.0, 50.0), Size::new(80.0, 40.0));
    assert_eq!(center(rc), Rectangle::new(p(120.0, 60.0), Size::new(40.0, 20.0)));
    assert_eq!(zone(rc, Zone::Whole), rc);
    assert_eq!(zone(rc, Zone::Center), center(rc));
    assert_eq!(zone(rc, Zone::Left), Rectangle::new(p(94.0, 44.0), Size::new(46.0, 52.0)));
    assert_eq!(zone(rc, Zone::Right), Rectangle::new(p(140.0, 44.0), Size::new(46.0, 52.0)));
}

#[test]
fn tile_specs_halves_and_center() {
    let v = tile_specs(Slot::Before(1), Slot::Before(2), Layer::Tile, None);
    assert_eq!(v, vec![
        Spec { slot: Slot::Before(1), layer: Layer::Tile, zone: Zone::Left },
        Spec { slot: Slot::Before(2), layer: Layer::Tile, zone: Zone::Right },
    ]);
    let v = tile_specs(Slot::Before(1), Slot::End, Layer::Tile, Some(Slot::Into(g())));
    assert_eq!(v.len(), 3);
    assert_eq!(v[1].slot, Slot::End);
    assert_eq!(v[2], Spec { slot: Slot::Into(g()), layer: Layer::Center, zone: Zone::Center });
}

fn popup(layer: Layer, rect: Rectangle) -> Target {
    Target { slot: Slot::MemberEnd(g()), rect, layer }
}

#[test]
fn left_popup_cases() {
    let area = [popup(Layer::PopupArea, r(100.0, 100.0)), t(Slot::Body, r(0.0, 500.0))];
    assert!(!left_popup(p(150.0, 5.0), &area));
    assert!(!left_popup(p(100.0 - 27.0, 5.0), &area));
    assert!(left_popup(p(100.0 - 29.0, 5.0), &area));
    assert!(left_popup(p(200.0 + 29.0, 5.0), &area));
    assert!(!left_popup(p(1000.0, 5.0), &[t(Slot::Body, r(0.0, 500.0))]));
}

#[test]
fn without_popup_filters_layers() {
    let all = [
        popup(Layer::PopupTile, r(0.0, 5.0)),
        popup(Layer::PopupArea, r(0.0, 5.0)),
        t(Slot::End, r(0.0, 5.0)),
    ];
    let out = without_popup(&all);
    assert_eq!(out, vec![all[2]]);
}

#[test]
fn wants_by_source() {
    let all = Wants { centers: true, folders: true, popup: true, unpin: false };
    let none = Wants { centers: false, folders: false, popup: false, unpin: false };
    let app = Wants { centers: true, folders: true, popup: false, unpin: false };
    assert_eq!(wants(&Source::Group(g()), false), none);
    assert_eq!(wants(&Source::Pinned(0), true), Wants { unpin: true, ..none });
    assert_eq!(wants(&Source::Pinned(0), false), Wants { unpin: true, ..app });
    assert_eq!(wants(&Source::App("a".into()), false), app);
    assert_eq!(wants(&Source::Member(g(), "a".into()), false), all);
}

fn dwell_targets() -> [Target; 2] {
    [
        Target { slot: Slot::Into(g()), rect: r(40.0, 20.0), layer: Layer::Center },
        Target { slot: Slot::Before(0), rect: r(0.0, 100.0), layer: Layer::Tile },
    ]
}

fn over(d: &Drag) -> Option<Slot> {
    match d {
        Drag::Dragging { over, .. } => *over,
        _ => None,
    }
}

fn hover_none(d: &Drag) -> bool {
    matches!(d, Drag::Dragging { hover: None, .. })
}

#[test]
fn dwell_sequence() {
    let tg = dwell_targets();
    let t0 = Instant::now();
    let ms = |n| t0 + Duration::from_millis(n);
    let d = Drag::arm(Source::App("a".into()), p(0.0, 0.0)).moved(p(50.0, 5.0), &tg, t0);
    assert_eq!(over(&d), Some(Slot::Before(0)));
    assert!(d.dwelling());
    let d = d.moved(p(50.0, 5.0), &tg, ms(449));
    assert_eq!(over(&d), Some(Slot::Before(0)));
    assert!(d.dwelling());
    let d = d.moved(p(50.0, 5.0), &tg, ms(450));
    assert_eq!(over(&d), Some(Slot::Into(g())));
    assert!(!d.dwelling());
    let d = d.moved(p(55.0, 5.0), &tg, ms(500));
    assert_eq!(over(&d), Some(Slot::Into(g())));
    let d = d.moved(p(10.0, 5.0), &tg, ms(600));
    assert!(hover_none(&d));
    assert_eq!(over(&d), Some(Slot::Before(0)));
    assert!(!d.dwelling());
    let d = d.moved(p(50.0, 5.0), &tg, ms(700)).moved(p(50.0, 5.0), &tg, ms(1000));
    assert_eq!(over(&d), Some(Slot::Before(0)));
    let d = d.moved(p(50.0, 5.0), &tg, ms(1150));
    assert_eq!(over(&d), Some(Slot::Into(g())));
}

fn lt(slot: Slot, rect: Rectangle, layer: Layer) -> Target {
    Target { slot, rect, layer }
}
fn drag_at(source: Source, ts: &[Target], pt: Point, t0: Instant, t1: Instant) -> Drag {
    Drag::arm(source, p(0.0, 0.0)).moved(pt, ts, t0).moved(pt, ts, t1)
}

#[test]
fn release_on_app_center_merges() {
    let ts = [lt(Slot::OnApp(3), r(0.0, 100.0), Layer::Center)];
    let t0 = Instant::now();
    let t1 = t0 + DWELL;
    let g = GroupRef::User(0);
    let merge = |s| drag_at(s, &ts, p(50.0, 5.0), t0, t1).released();
    assert_eq!(merge(Source::Pinned(1)), Action::Merge { item: Item::Pinned(1), onto: 3 });
    assert_eq!(merge(Source::App("a".into())), Action::Merge { item: Item::App("a".into()), onto: 3 });
    assert_eq!(merge(Source::Member(g, "m".into())), Action::Merge { item: Item::App("m".into()), onto: 3 });
    assert_eq!(merge(Source::Group(g)), Action::Cancel);
}

#[test]
fn popup_center_dwell_and_priority() {
    let area = lt(Slot::Body, r(0.0, 200.0), Layer::PopupArea);
    let tile = lt(Slot::MemberEnd(GroupRef::User(0)), r(0.0, 100.0), Layer::PopupTile);
    let cen = lt(Slot::OnApp(2), r(0.0, 100.0), Layer::PopupCenter);
    let ts = [cen, tile, area];
    let t0 = Instant::now();
    let early = drag_at(Source::App("a".into()), &ts, p(50.0, 5.0), t0, t0 + Duration::from_millis(100));
    assert!(matches!(early, Drag::Dragging { over: Some(Slot::MemberEnd(_)), .. }));
    assert!(early.dwelling());
    let late = drag_at(Source::App("a".into()), &ts, p(50.0, 5.0), t0, t0 + DWELL);
    assert!(matches!(late, Drag::Dragging { over: Some(Slot::OnApp(2)), .. }));
}

#[test]
fn open_popup_ignores_center_accepts_popup_center() {
    let area = lt(Slot::Body, r(0.0, 200.0), Layer::PopupArea);
    let below = lt(Slot::OnApp(1), r(0.0, 100.0), Layer::Center);
    let t0 = Instant::now();
    let d = drag_at(Source::App("a".into()), &[area, below], p(50.0, 5.0), t0, t0 + DWELL);
    assert!(matches!(d, Drag::Dragging { over: Some(Slot::Body), .. }));
    let pc = lt(Slot::OnApp(2), r(0.0, 100.0), Layer::PopupCenter);
    let d = drag_at(Source::App("a".into()), &[pc, area, below], p(50.0, 5.0), t0, t0 + DWELL);
    assert!(matches!(d, Drag::Dragging { over: Some(Slot::OnApp(2)), .. }));
}

#[test]
fn without_popup_drops_popup_center() {
    let all = [
        lt(Slot::OnApp(0), r(0.0, 10.0), Layer::PopupCenter),
        lt(Slot::OnApp(1), r(0.0, 10.0), Layer::Center),
    ];
    let out = without_popup(&all);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].layer, Layer::Center);
}

#[test]
fn targets_sort_popup_center_first() {
    let specs = vec![
        tile_specs(Slot::Before(0), Slot::End, Layer::Tile, Some(Slot::OnApp(0))),
        tile_specs(Slot::Before(1), Slot::End, Layer::PopupTile, Some(Slot::OnApp(1))),
        vec![Spec { slot: Slot::OnApp(2), layer: Layer::PopupCenter, zone: Zone::Center }],
    ];
    let rects = [Some(r(0.0, 10.0)), Some(r(0.0, 10.0)), Some(r(0.0, 10.0))];
    let out = targets(&specs, &rects);
    assert_eq!(out[0].layer, Layer::PopupCenter);
    assert!(Layer::PopupCenter < Layer::PopupTile);
}

#[test]
fn apply_merge_keeps_list() {
    let l = v(&["a", "b"]);
    assert_eq!(apply(&l, &Action::Merge { item: Item::Pinned(0), onto: 1 }, &eq), l);
}

#[test]
fn member_over_center_before_dwell_does_not_ungroup() {
    let ts = [
        lt(Slot::OnApp(3), r(0.0, 100.0), Layer::Center),
        lt(Slot::Body, r(0.0, 1000.0), Layer::Body),
    ];
    let t0 = Instant::now();
    let member = || Source::Member(GroupRef::User(0), "m".into());
    let early = drag_at(member(), &ts, p(50.0, 5.0), t0, t0 + Duration::from_millis(100));
    assert_eq!(over(&early), None);
    assert_eq!(early.released(), Action::Cancel);
    let late = drag_at(member(), &ts, p(50.0, 5.0), t0, t0 + DWELL);
    assert_eq!(late.released(), Action::Merge { item: Item::App("m".into()), onto: 3 });
}

fn below_spec() -> Vec<Spec> {
    vec![Spec { slot: Slot::Unpin, layer: Layer::Unpin, zone: Zone::Below }]
}

#[test]
fn zone_below_is_strip_under_rect() {
    let area = Rectangle::new(p(10.0, 20.0), Size::new(200.0, 100.0));
    let z = zone(area, Zone::Below);
    assert_eq!((z.x, z.y, z.width, z.height), (10.0, 124.0, 200.0, 30.0));
}

#[test]
fn targets_with_below_spec() {
    let area = Rectangle::new(p(10.0, 20.0), Size::new(200.0, 100.0));
    let out = targets(&[below_spec()], &[Some(area)]);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].slot, Slot::Unpin);
    assert_eq!(out[0].layer, Layer::Unpin);
    assert_eq!(out[0].rect, zone(area, Zone::Below));
}

#[test]
fn unpin_wins_over_body() {
    let area = Rectangle::new(p(0.0, 0.0), Size::new(100.0, 100.0));
    let body = Target { slot: Slot::Body, rect: Rectangle::new(p(0.0, 0.0), Size::new(500.0, 500.0)), layer: Layer::Body };
    let mut all = targets(&[below_spec()], &[Some(area)]);
    all.insert(0, body);
    all.sort_by_key(|t| t.layer);
    let d = Drag::arm(Source::Pinned(2), p(1.0, 1.0)).moved(p(50.0, 110.0), &all, Instant::now());
    assert_eq!(over(&d), Some(Slot::Unpin));
    assert_eq!(d.released(), Action::Unpin { index: 2 });
}

fn released_over_unpin(source: Source) -> Action {
    Drag::Dragging { source, at: p(0.0, 0.0), over: Some(Slot::Unpin), hover: None }.released()
}

#[test]
fn unpin_release_table() {
    assert_eq!(released_over_unpin(Source::Pinned(3)), Action::Unpin { index: 3 });
    assert_eq!(released_over_unpin(Source::App("a".into())), Action::Cancel);
    assert_eq!(released_over_unpin(Source::Member(g(), "a".into())), Action::Cancel);
    assert_eq!(released_over_unpin(Source::Group(g())), Action::Cancel);
}

#[test]
fn unpin_apply_keeps_list() {
    let list = v(&["a", "b"]);
    assert_eq!(apply(&list, &Action::Unpin { index: 0 }, &eq), list);
}
