use super::*;

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

fn set(list: &[&str]) -> HashSet<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn bounds_leave_the_margin_on_all_four_sides() {
    let r = bounds_inside(1000.0, 800.0);
    assert_eq!((r.x, r.y, r.width, r.height), (MARGIN, MARGIN, 1000.0 - 2.0 * MARGIN, 800.0 - 2.0 * MARGIN));
}

#[test]
fn bounds_are_never_negative() {
    let r = bounds_inside(10.0, 0.0);
    assert_eq!((r.width, r.height), (0.0, 0.0));
    let r = bounds_inside(2.0 * MARGIN, 3.0 * MARGIN);
    assert_eq!((r.width, r.height), (0.0, MARGIN));
}

#[test]
fn nothing_blocks_when_no_condition_holds() {
    assert!(!blocked(Blockers::default()));
}

#[test]
fn each_condition_alone_blocks() {
    let each = [
        Blockers { hidden: true, ..Default::default() },
        Blockers { menu: true, ..Default::default() },
        Blockers { dragging: true, ..Default::default() },
        Blockers { searching: true, ..Default::default() },
        Blockers { other_page: true, ..Default::default() },
        Blockers { tools: true, ..Default::default() },
        Blockers { uninstall: true, ..Default::default() },
        Blockers { resizing: true, ..Default::default() },
    ];
    for (n, b) in each.into_iter().enumerate() {
        assert!(blocked(b), "condition {n}");
    }
}

#[test]
fn thumbs_skip_the_ones_in_flight() {
    let got = thumbs_to_request(&ids(&["a", "b", "c"]), &set(&["b"]), &set(&[]));
    assert_eq!(got, ids(&["a", "c"]));
}

#[test]
fn thumbs_skip_minimized_windows_that_have_a_photo() {
    let got = thumbs_to_request(&ids(&["a", "b"]), &set(&[]), &set(&["a"]));
    assert_eq!(got, ids(&["b"]));
}

#[test]
fn a_minimized_window_without_photo_is_asked() {
    // "a" is minimized but not in the with-photo set.
    assert_eq!(thumbs_to_request(&ids(&["a"]), &set(&[]), &set(&[])), ids(&["a"]));
}

#[test]
fn a_photo_of_the_shown_group_is_kept() {
    assert!(should_keep(true, &ids(&["a", "b"]), "b"));
}

#[test]
fn a_photo_of_a_window_of_another_group_is_dropped() {
    assert!(!should_keep(true, &ids(&["a", "b"]), "z"));
}

#[test]
fn a_photo_after_close_is_dropped() {
    // After Close the shown ids are empty (nothing shows), whatever arrives.
    assert!(!should_keep(false, &[], "a"));
    assert!(!should_keep(false, &ids(&["a"]), "a"));
}

fn open_ctx() -> preview::Ctx<impl Fn(&str) -> usize> {
    let bounds = iced::Rectangle::new(iced::Point::ORIGIN, iced::Size::new(1000.0, 800.0));
    preview::Ctx { blocked: false, bounds, count: |_: &str| 2 }
}

#[test]
fn reset_hover_only_grows_the_counters() {
    let ctx = open_ctx();
    let mut h = preview::Hover::default();
    preview::step(&mut h, Event::EnterIcon("a".into()), &ctx);
    let (g, m) = (h.generation(), h.measure());
    reset_hover(&mut h);
    assert!(h.generation() > g && h.measure() > m);
    assert_eq!(*h.phase(), preview::Phase::Idle);
    let (g, m) = (h.generation(), h.measure());
    reset_hover(&mut h);
    assert!(h.generation() >= g && h.measure() >= m);
}

#[test]
fn an_old_timer_does_not_open_after_reopening() {
    let ctx = open_ctx();
    let mut h = preview::Hover::default();
    preview::step(&mut h, Event::EnterIcon("a".into()), &ctx);
    let old = h.generation();
    let old_measure = h.measure();
    reset_hover(&mut h);
    preview::step(&mut h, Event::EnterIcon("a".into()), &ctx);
    assert!(matches!(h.phase(), preview::Phase::Waiting { .. }));
    assert_ne!(h.generation(), old);
    assert_ne!(h.measure(), old_measure);
    preview::step(&mut h, Event::Timer(old), &ctx);
    assert!(matches!(h.phase(), preview::Phase::Waiting { .. }));
    preview::step(&mut h, Event::Anchor(old_measure, Some(iced::Rectangle::default())), &ctx);
    assert_eq!(h.anchor(), None);
}

#[test]
fn env_target_wants_windows_and_an_app() {
    assert_eq!(env_target(Some("windows:Firefox")), Some("firefox".to_string()));
    assert_eq!(env_target(Some("windows:")), None);
    assert_eq!(env_target(Some("recommended")), None);
    assert_eq!(env_target(Some("windows")), None);
    assert_eq!(env_target(None), None);
}

#[test]
fn the_group_of_an_app_is_found_ignoring_case() {
    let windows = vec![("app:a".to_string(), "Org.A".to_string()), ("id:b".to_string(), "b".to_string())];
    assert_eq!(group_for_app(&windows, "org.a"), Some("app:a".to_string()));
    assert_eq!(group_for_app(&windows, "b"), Some("id:b".to_string()));
    assert_eq!(group_for_app(&windows, "c"), None);
}

#[test]
fn mouse_events_become_preview_messages() {
    use iced::mouse::{Event as M, ScrollDelta};
    let w = window::Id::unique();
    let moved = iced::Event::Mouse(M::CursorMoved { position: iced::Point::new(3.0, 4.0) });
    assert!(matches!(
        mouse_event(moved, w),
        Some(Message::PreviewMouse(id, preview::Event::Pointer(p))) if id == w && p == iced::Point::new(3.0, 4.0)
    ));
    assert!(matches!(
        mouse_event(iced::Event::Mouse(M::CursorLeft), w),
        Some(Message::PreviewMouse(_, preview::Event::PointerGone))
    ));
    let wheel = iced::Event::Mouse(M::WheelScrolled { delta: ScrollDelta::Lines { x: 0.0, y: 1.0 } });
    assert!(matches!(mouse_event(wheel, w), Some(Message::PreviewMouse(_, preview::Event::Wheel))));
    assert!(mouse_event(iced::Event::Mouse(M::CursorEntered), w).is_none());
    assert!(mouse_event(iced::Event::Window(window::Event::Closed), w).is_none());
}
