use super::*;

fn lim() -> Limits {
    limits(1920.0, 1080.0)
}
fn sz(w: f32, h: f32) -> Size {
    Size::new(w, h)
}
fn pt(x: f32, y: f32) -> Point {
    Point::new(x, y)
}
fn rs(edge: Edge, on_top: bool, dx: f32, dy: f32) -> Size {
    resized(edge, sz(700.0, 700.0), pt(100.0, 100.0), pt(100.0 + dx, 100.0 + dy), on_top, lim())
}

#[test]
fn each_edge_resizes_in_both_orientations() {
    assert_eq!(rs(Edge::Left, false, -10.0, 0.0), sz(720.0, 700.0));
    assert_eq!(rs(Edge::Right, false, 10.0, 0.0), sz(720.0, 700.0));
    assert_eq!(rs(Edge::Far, false, 0.0, -30.0), sz(700.0, 730.0));
    assert_eq!(rs(Edge::Far, true, 0.0, 30.0), sz(700.0, 730.0));
    assert_eq!(rs(Edge::FarLeft, false, -10.0, -30.0), sz(720.0, 730.0));
    assert_eq!(rs(Edge::FarRight, true, 10.0, 30.0), sz(720.0, 730.0));
}

#[test]
fn resize_clamps_to_both_limits() {
    let l = lim();
    assert_eq!(rs(Edge::Right, false, 5000.0, 0.0).width, 874.0);
    assert_eq!(rs(Edge::Far, false, 0.0, -5000.0).height, l.max.height);
    assert_eq!(rs(Edge::Right, false, -5000.0, 0.0).width, 400.0);
    assert_eq!(rs(Edge::Far, false, 0.0, 5000.0).height, 480.0);
}

#[test]
fn limits_adapt_to_small_screens() {
    assert_eq!(limits(1920.0, 700.0).max.height, 668.0);
    let l = limits(1920.0, 400.0);
    assert_eq!(l.min.height, 368.0);
    assert_eq!(l.max.height, 368.0);
    assert_eq!(limits(600.0, 1080.0).max.width, 568.0);
    let d = limits(0.0, 0.0);
    assert_eq!(d, Limits { min: sz(400.0, 480.0), max: sz(874.0, 920.0) });
}

#[test]
fn stored_and_effective_sizes() {
    assert_eq!(clamp_stored(0, 0), sz(400.0, 480.0));
    assert_eq!(clamp_stored(99999, 99999), sz(874.0, 920.0));
    let l = lim();
    assert_eq!(effective(None, l), l.max);
    assert_eq!(effective(Some(sz(10000.0, 10.0)), l), sz(874.0, 480.0));
    assert_eq!(effective(Some(sz(f32::NAN, 600.0)), l).width, 874.0);
}

#[test]
fn state_machine_arm_then_move() {
    let from = sz(700.0, 700.0);
    let r = Resize::arm(Edge::Right, from);
    assert!(!r.is_idle());
    assert_eq!(r.from(), Some(from));
    let (r, out) = r.moved(pt(50.0, 50.0), false, lim());
    assert_eq!(out, None);
    assert!(matches!(r, Resize::Active { .. }));
    let (r2, out) = r.moved(pt(60.0, 50.0), false, lim());
    assert_eq!(out, Some(sz(720.0, 700.0)));
    assert_eq!(r2, r);
    let (i, out) = Resize::Idle.moved(pt(1.0, 1.0), false, lim());
    assert!(i.is_idle());
    assert_eq!(out, None);
    assert_eq!(Resize::Idle.from(), None);
}

fn touching_only(a: &Rectangle, b: &Rectangle) -> bool {
    a.x >= b.x + b.width || b.x >= a.x + a.width || a.y >= b.y + b.height || b.y >= a.y + a.height
}

#[test]
fn handles_do_not_overlap_or_touch_panel_edge() {
    let m = Rectangle::new(pt(523.0, 144.0), sz(874.0, 920.0));
    for on_top in [false, true] {
        let hs = handles(m, on_top);
        assert_eq!(hs.len(), 5);
        for i in 0..5 {
            for j in (i + 1)..5 {
                assert!(touching_only(&hs[i].1, &hs[j].1), "{i} {j} {on_top}");
            }
            let r = hs[i].1;
            if on_top {
                assert!(r.y >= m.y);
            } else {
                assert!(r.y + r.height <= m.y + m.height);
            }
        }
        let far = hs[2].1;
        assert_eq!((far.x, far.width, far.height), (m.x + 10.0, m.width - 20.0, 8.0));
        assert_eq!((hs[3].1.width, hs[3].1.height), (16.0, 16.0));
        assert_eq!((hs[4].1.width, hs[4].1.height), (16.0, 16.0));
        assert_eq!((hs[0].1.width, hs[1].1.width), (8.0, 8.0));
    }
    let h = handles(m, false);
    assert_eq!(h[2].1.y, m.y - 6.0);
    assert_eq!(h[0].1, Rectangle::new(pt(m.x - 6.0, m.y + 10.0), sz(8.0, m.height - 10.0)));
    let t = handles(m, true);
    assert_eq!(t[2].1.y, m.y + m.height - 2.0);
    assert_eq!(t[3].1.y, m.y + m.height - 10.0);
}

#[test]
fn interaction_cursors() {
    for t in [false, true] {
        assert_eq!(interaction(Edge::Left, t), Interaction::ResizingHorizontally);
        assert_eq!(interaction(Edge::Right, t), Interaction::ResizingHorizontally);
        assert_eq!(interaction(Edge::Far, t), Interaction::ResizingVertically);
    }
    assert_eq!(interaction(Edge::FarLeft, false), Interaction::ResizingDiagonallyDown);
    assert_eq!(interaction(Edge::FarRight, false), Interaction::ResizingDiagonallyUp);
    assert_eq!(interaction(Edge::FarLeft, true), Interaction::ResizingDiagonallyUp);
    assert_eq!(interaction(Edge::FarRight, true), Interaction::ResizingDiagonallyDown);
}

#[test]
fn parse_size_accepts_wxh() {
    assert_eq!(parse_size("600x700"), Some(sz(600.0, 700.0)));
    assert_eq!(parse_size(" 480X600 "), Some(sz(480.0, 600.0)));
    assert_eq!(parse_size("0x0"), Some(sz(400.0, 480.0)));
}

#[test]
fn parse_size_rejects_garbage() {
    for s in ["", "abc", "600", "600x", "x600", "-1x5", "1x2x3"] {
        assert_eq!(parse_size(s), None, "{s}");
    }
}

#[test]
fn edge_of_state() {
    assert_eq!(Resize::Idle.edge(), None);
    assert_eq!(Resize::arm(Edge::Far, sz(500.0, 500.0)).edge(), Some(Edge::Far));
}

#[test]
fn initial_pref_sources() {
    assert_eq!(initial_pref(Some("600x700"), Some((500, 500))), (Some(sz(600.0, 700.0)), false));
    assert_eq!(initial_pref(None, Some((500, 500))), (Some(sz(500.0, 500.0)), true));
    assert_eq!(initial_pref(Some("lixo"), Some((500, 500))), (Some(sz(500.0, 500.0)), true));
    assert_eq!(initial_pref(None, None), (None, true));
}

#[test]
fn width_is_always_even() {
    let l = lim();
    // Odd widths round half up to the nearest even one: 601 -> 602.
    assert_eq!(effective(Some(sz(601.0, 700.0)), l), sz(602.0, 700.0));
    assert_eq!(effective(Some(sz(874.0, 700.0)), l).width, 874.0);
    assert_eq!(effective(Some(sz(400.0, 700.0)), l).width, 400.0);
    let odd = limits(887.0, 1080.0);
    assert_eq!(odd.max.width, 855.0);
    assert_eq!(effective(None, odd).width, 854.0);
    for dx in [-3.0, 0.0, 1.0, 7.0, 500.0] {
        let s = resized(Edge::Right, sz(700.0, 700.0), pt(0.0, 0.0), pt(dx, 0.0), false, odd);
        assert!(s.width <= 855.0 && s.width % 2.0 == 0.0, "{dx} {}", s.width);
    }
}

#[test]
fn requested_keeps_the_axis_not_dragged() {
    let low = limits(1920.0, 700.0);
    assert_eq!(low.max.height, 668.0);
    let dragged = resized(Edge::Right, sz(700.0, 668.0), pt(0.0, 0.0), pt(50.0, 0.0), false, low);
    assert_eq!(dragged, sz(800.0, 668.0));
    assert_eq!(requested(Edge::Right, None, dragged), sz(800.0, 920.0));
    assert_eq!(requested(Edge::Left, Some(sz(500.0, 600.0)), dragged), sz(800.0, 600.0));
    assert_eq!(requested(Edge::Far, Some(sz(500.0, 600.0)), sz(700.0, 650.0)), sz(500.0, 650.0));
    assert_eq!(requested(Edge::Far, None, sz(700.0, 650.0)), sz(874.0, 650.0));
    assert_eq!(requested(Edge::FarLeft, Some(sz(500.0, 600.0)), sz(700.0, 650.0)), sz(700.0, 650.0));
    assert_eq!(requested(Edge::FarRight, None, sz(700.0, 650.0)), sz(700.0, 650.0));
}

#[test]
fn save_decision_and_stored_pair() {
    let a = Some(sz(500.0, 600.0));
    assert!(should_save(None, a, true));
    assert!(should_save(a, None, true));
    assert!(!should_save(a, a, true));
    assert!(!should_save(None, None, true));
    assert!(!should_save(None, a, false));
    assert_eq!(stored_pair(None), (874, 920));
    assert_eq!(stored_pair(a), (500, 600));
}

#[test]
fn finish_is_idempotent() {
    let r = Resize::arm(Edge::Left, sz(500.0, 500.0));
    let (next, was) = r.finish();
    assert!(next.is_idle() && was);
    let (next, was) = next.finish();
    assert!(next.is_idle() && !was);
    assert_eq!(Resize::Idle.finish(), (Resize::Idle, false));
    let active = Resize::Active { edge: Edge::Far, from: sz(500.0, 500.0), start: pt(1.0, 1.0) };
    assert_eq!(active.finish(), (Resize::Idle, true));
}
