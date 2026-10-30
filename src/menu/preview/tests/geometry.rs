use super::*;

// (h) place
#[test]
fn place_above_when_it_fits() {
    let bounds = r(0.0, 0.0, 874.0, 700.0);
    let p = place(r(400.0, 600.0, 40.0, 40.0), Size::new(224.0, 164.0), bounds);
    assert_eq!(p, pt(308.0, 600.0 - GAP - 164.0));
}

#[test]
fn place_below_when_above_does_not_fit() {
    let bounds = r(0.0, 0.0, 874.0, 700.0);
    let p = place(r(400.0, 50.0, 40.0, 40.0), Size::new(224.0, 164.0), bounds);
    assert_eq!(p, pt(308.0, 50.0 + 40.0 + GAP));
}

#[test]
fn place_clamps_to_sides() {
    let bounds = r(0.0, 0.0, 874.0, 700.0);
    let s = Size::new(224.0, 164.0);
    assert_eq!(place(r(0.0, 600.0, 40.0, 40.0), s, bounds).x, 0.0);
    assert_eq!(place(r(850.0, 600.0, 24.0, 40.0), s, bounds).x, 874.0 - 224.0);
}

#[test]
fn place_in_400_and_874_wide_menus() {
    let s = Size::new(224.0, 164.0);
    let narrow = r(0.0, 0.0, 400.0, 600.0);
    assert_eq!(place(r(380.0, 500.0, 40.0, 40.0), s, narrow).x, 400.0 - 224.0);
    let wide = r(0.0, 0.0, 874.0, 600.0);
    assert_eq!(place(r(380.0, 500.0, 40.0, 40.0), s, wide).x, 400.0 - 112.0);
}

#[test]
fn place_neither_side_fits_and_tiny_bounds() {
    let s = Size::new(224.0, 164.0);
    let b = r(0.0, 0.0, 300.0, 200.0);
    let p = place(r(100.0, 80.0, 40.0, 40.0), s, b);
    assert_eq!(p.y, 200.0 - 164.0);
    let tiny = r(10.0, 20.0, 100.0, 100.0);
    assert_eq!(place(r(30.0, 60.0, 40.0, 40.0), s, tiny), pt(10.0, 20.0));
}

// (i) grid
#[test]
fn grid_counts() {
    let b = Size::new(874.0, 2000.0);
    let (c, rows, sz) = grid(1, b);
    assert_eq!((c, rows), (1, 1));
    assert_eq!(sz, Size::new(2.0 * PAD + CARD_W, 2.0 * PAD + CARD_H));
    assert_eq!((grid(7, Size::new(2000.0, 2000.0)).0, grid(7, Size::new(2000.0, 2000.0)).1), (7, 1));
    assert_eq!(grid(8, Size::new(2000.0, 2000.0)).1, 2);
    let (c, rows, _) = grid(15, Size::new(2000.0, 2000.0));
    assert_eq!((c, rows), (7, 3));
    let (_, rows, sz) = grid(0, b);
    assert_eq!(rows, 0);
    assert_eq!(sz.height, 2.0 * PAD);
}

#[test]
fn grid_narrow_and_tall_limits() {
    let (c, rows, sz) = grid(5, Size::new(500.0, 2000.0));
    assert_eq!(c, 2);
    assert_eq!(rows, 3);
    assert_eq!(sz.width, 2.0 * PAD + 2.0 * CARD_W + GAP);
    assert_eq!(grid(3, Size::new(10.0, 2000.0)).0, 1);
    let (_, _, sz) = grid(15, Size::new(2000.0, 300.0));
    assert_eq!(sz.height, 300.0);
}

// (j) bridge / inside / card_rect
fn hover_at(a: Rectangle, n: usize, p: Point) -> Hover {
    let mut h = Hover::default();
    step(&mut h, Event::ClickGroup(k("a")), &cxn(n));
    let gn = h.generation();
    step(&mut h, Event::Anchor(gn, Some(a)), &cxn(n));
    step(&mut h, Event::Pointer(p), &cxn(n));
    h
}

fn centre(c: Rectangle) -> Point {
    pt(c.center_x(), c.center_y())
}

#[test]
fn bridge_card_above_and_below() {
    let a = anchor();
    let above_card = r(50.0, 300.0, 224.0, 164.0);
    let above = bridge(a, above_card).expect("gap above");
    assert_eq!(above, r(100.0, 464.0, 40.0, 36.0));
    let below_card = r(50.0, 548.0, 224.0, 164.0);
    let below = bridge(a, below_card).expect("gap below");
    assert_eq!(below, r(100.0, 540.0, 40.0, 8.0));
    assert!(bridge(a, r(50.0, 480.0, 224.0, 164.0)).is_none());
}

#[test]
fn card_rect_uses_place_and_grid() {
    let c = card_rect(anchor(), 1, screen());
    assert_eq!(c, r(0.0, 312.0, 240.0, 180.0));
    let size = grid(3, screen().size()).2;
    let c3 = card_rect(anchor(), 3, screen());
    assert_eq!((c3.width, c3.height), (size.width, size.height));
    assert_eq!(c3.position(), place(anchor(), size, screen()));
}

#[test]
fn inside_counts_the_gap_but_not_beside_it() {
    let h = hover_at(anchor(), 1, pt(120.0, 496.0));
    assert!(inside(&h, 1, screen()), "gap above");
    let h = hover_at(anchor(), 1, pt(300.0, 496.0));
    assert!(!inside(&h, 1, screen()), "beside gap");
    let top = r(100.0, 50.0, 40.0, 40.0);
    let h = hover_at(top, 1, pt(120.0, 94.0));
    assert!(inside(&h, 1, screen()), "gap below");
    let h = hover_at(top, 1, pt(300.0, 94.0));
    assert!(!inside(&h, 1, screen()));
    let h = hover_at(anchor(), 1, pt(120.0, 520.0));
    assert!(inside(&h, 1, screen()), "on the icon");
}

#[test]
fn inside_false_without_pointer_or_anchor() {
    assert!(!inside(&Hover::default(), 1, screen()));
    let mut h = Hover::default();
    step(&mut h, Event::ClickGroup(k("a")), &cx());
    step(&mut h, Event::Pointer(pt(120.0, 350.0)), &cx());
    assert!(!inside(&h, 1, screen()), "no anchor");
}

#[test]
fn card_centre_is_inside_and_keeps_shown() {
    let mut h = Hover::default();
    shown(&mut h, "a");
    let g = h.generation();
    let fx = step(&mut h, Event::Pointer(centre(card())), &cx());
    assert!(fx.is_empty());
    assert_eq!((phase_of(&h), h.generation()), (Phase::Shown { key: k("a") }, g));
    assert!(inside(&h, 1, screen()));
}

#[test]
fn leaving_returns_on_card_centre() {
    let mut h = Hover::default();
    leaving(&mut h, "a");
    step(&mut h, Event::Pointer(centre(card())), &cx());
    assert_eq!(phase_of(&h), Phase::Shown { key: k("a") });
}

#[test]
fn card_below_the_icon_hit_tests() {
    let top = r(100.0, 50.0, 40.0, 40.0);
    let c = card_rect(top, 1, screen());
    assert_eq!(c.y, 98.0);
    let mut h = hover_at(top, 1, centre(c));
    assert!(inside(&h, 1, screen()));
    step(&mut h, Event::Pointer(pt(120.0, 94.0)), &cx());
    assert!(inside(&h, 1, screen()), "gap");
}

#[test]
fn bigger_group_grows_the_card_and_hit_test() {
    let a = r(400.0, 500.0, 40.0, 40.0);
    let c1 = card_rect(a, 1, screen());
    let c8 = card_rect(a, 8, screen());
    assert!(c8.width > c1.width && c8.height > c1.height);
    let p = pt(c8.x + c8.width - 4.0, c8.y + 4.0);
    assert!(!c1.contains(p));
    assert!(inside(&hover_at(a, 8, p), 8, screen()));
    assert!(!inside(&hover_at(a, 1, p), 1, screen()));
}
