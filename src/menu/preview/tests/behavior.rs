use super::*;

#[test]
fn anchor_none_in_shown_and_leaving_stops_capture() {
    let mut h = Hover::default();
    shown(&mut h, "a");
    let m = h.measure();
    let fx = step(&mut h, Event::Anchor(m, None), &cx());
    assert_eq!(phase_of(&h), Phase::Idle);
    assert_eq!(fx, vec![Effect::StopCapture]);

    let mut h = Hover::default();
    leaving(&mut h, "a");
    let m = h.measure();
    let fx = step(&mut h, Event::Anchor(m, None), &cx());
    assert_eq!(phase_of(&h), Phase::Idle);
    assert_eq!(fx, vec![Effect::StopCapture]);

    let mut h = Hover::default();
    waiting(&mut h, "a");
    let m = h.measure();
    let fx = step(&mut h, Event::Anchor(m, None), &cx());
    assert_eq!(phase_of(&h), Phase::Idle);
    assert!(fx.is_empty(), "waiting never captured");
}

#[test]
fn relayout_remeasures_without_changing_phase() {
    for n in 0..3 {
        let mut h = Hover::default();
        match n {
            0 => {
                waiting(&mut h, "a");
            }
            1 => shown(&mut h, "a"),
            _ => {
                leaving(&mut h, "a");
            }
        }
        let (g, m, p) = (h.generation(), h.measure(), phase_of(&h));
        let fx = step(&mut h, Event::Relayout, &cx());
        assert_eq!(fx, vec![Effect::Measure(k("a"), m)], "phase {n}");
        assert_eq!((h.generation(), h.measure(), phase_of(&h)), (g, m, p));
    }
}

#[test]
fn relayout_answer_moves_the_anchor_and_hit_test() {
    let mut h = Hover::default();
    shown(&mut h, "a");
    let m = h.measure();
    step(&mut h, Event::Relayout, &cx());
    let moved = r(600.0, 500.0, 40.0, 40.0);
    step(&mut h, Event::Anchor(m, Some(moved)), &cx());
    assert_eq!(h.anchor(), Some(moved));
    step(&mut h, Event::Pointer(pt(620.0, 520.0)), &cx());
    assert!(inside(&h, 1, screen()), "new icon place");
    assert_eq!(phase_of(&h), Phase::Shown { key: k("a") });
    step(&mut h, Event::Pointer(pt(120.0, 520.0)), &cx());
    assert!(!inside(&h, 1, screen()), "old icon place");
}

#[test]
fn relayout_in_idle_does_nothing() {
    let mut h = Hover::default();
    assert!(step(&mut h, Event::Relayout, &cx()).is_empty());
    assert_eq!((phase_of(&h), h.generation()), (Phase::Idle, 0));
}

#[test]
fn wheel_over_card_keeps_the_preview() {
    let mut h = Hover::default();
    shown(&mut h, "a");
    step(&mut h, Event::Pointer(centre_of_card()), &cx());
    let g = h.generation();
    assert!(step(&mut h, Event::Wheel, &cx()).is_empty());
    assert_eq!((phase_of(&h), h.generation()), (Phase::Shown { key: k("a") }, g));
}

fn centre_of_card() -> Point {
    pt(card().center_x(), card().center_y())
}

#[test]
fn wheel_elsewhere_closes() {
    let mut h = Hover::default();
    shown(&mut h, "a");
    step(&mut h, Event::Pointer(pt(120.0, 520.0)), &cx());
    let fx = step(&mut h, Event::Wheel, &cx());
    assert_eq!(phase_of(&h), Phase::Idle);
    assert_eq!(fx, vec![Effect::StopCapture], "icon is not the card");

    let mut h = Hover::default();
    shown(&mut h, "a");
    step(&mut h, Event::Pointer(pt(120.0, 496.0)), &cx());
    step(&mut h, Event::Wheel, &cx());
    assert_eq!(phase_of(&h), Phase::Idle, "bridge is not the card");

    let mut h = Hover::default();
    leaving(&mut h, "a");
    let fx = step(&mut h, Event::Wheel, &cx());
    assert_eq!((phase_of(&h), fx), (Phase::Idle, vec![Effect::StopCapture]));
}

#[test]
fn wheel_in_waiting_and_idle() {
    let mut h = Hover::default();
    waiting(&mut h, "a");
    let fx = step(&mut h, Event::Wheel, &cx());
    assert_eq!(phase_of(&h), Phase::Idle);
    assert!(fx.is_empty());

    let mut h = Hover::default();
    assert!(step(&mut h, Event::Wheel, &cx()).is_empty());
    assert_eq!((phase_of(&h), h.generation()), (Phase::Idle, 0));
}

fn anchor_b() -> Rectangle {
    r(300.0, 500.0, 40.0, 40.0)
}

#[test]
fn switching_icons_with_pointer_before_anchor_shows_the_new_one() {
    let mut h = Hover::default();
    shown(&mut h, "a");
    step(&mut h, Event::EnterIcon(k("b")), &cx());
    let (g, m) = (h.generation(), h.measure());
    step(&mut h, Event::Pointer(pt(320.0, 520.0)), &cx());
    step(&mut h, Event::Anchor(m, Some(anchor_b())), &cx());
    assert_eq!(phase_of(&h), Phase::Shown { key: k("b") });
    assert_eq!(h.anchor(), Some(anchor_b()));
    // the pending close timer of the transient Leaving must not close it
    let fx = step(&mut h, Event::Timer(g + 1), &cx());
    assert!(fx.is_empty());
    assert_eq!(phase_of(&h), Phase::Shown { key: k("b") });
}

#[test]
fn switching_icons_with_anchor_before_pointer_shows_the_new_one() {
    let mut h = Hover::default();
    shown(&mut h, "a");
    step(&mut h, Event::EnterIcon(k("b")), &cx());
    let m = h.measure();
    step(&mut h, Event::Anchor(m, Some(anchor_b())), &cx());
    step(&mut h, Event::Pointer(pt(320.0, 520.0)), &cx());
    assert_eq!(phase_of(&h), Phase::Shown { key: k("b") });
    assert_eq!(h.anchor(), Some(anchor_b()));
}

#[test]
fn relayout_answer_survives_shown_leaving_shown() {
    let mut h = Hover::default();
    shown(&mut h, "a");
    let m = h.measure();
    let fx = step(&mut h, Event::Relayout, &cx());
    assert_eq!(fx, vec![Effect::Measure(k("a"), m)]);
    step(&mut h, Event::Pointer(pt(900.0, 900.0)), &cx());
    assert!(matches!(h.phase(), Phase::Leaving { .. }));
    step(&mut h, Event::Pointer(pt(120.0, 350.0)), &cx());
    assert_eq!(phase_of(&h), Phase::Shown { key: k("a") });
    let moved = r(600.0, 500.0, 40.0, 40.0);
    step(&mut h, Event::Anchor(m, Some(moved)), &cx());
    assert_eq!(h.anchor(), Some(moved));
}

#[test]
fn stale_timer_and_anchor_after_close_do_not_match_on_reopen() {
    let mut h = Hover::default();
    shown(&mut h, "a");
    let (g_old, m_old) = (h.generation(), h.measure());
    step(&mut h, Event::Close, &cx());
    assert!(h.generation() > g_old && h.measure() > m_old);
    step(&mut h, Event::EnterIcon(k("a")), &cx());
    assert_ne!(h.generation(), g_old);
    assert_ne!(h.measure(), m_old);
    step(&mut h, Event::Anchor(m_old, Some(anchor())), &cx());
    assert!(h.anchor().is_none());
    let fx = step(&mut h, Event::Timer(g_old), &cx());
    assert!(fx.is_empty());
    assert_eq!(phase_of(&h), Phase::Waiting { key: k("a") });
}

#[test]
fn rekey_keeps_phase_and_drops_old_measures() {
    let mut h = Hover::default();
    h.rekey(k("x"));
    assert_eq!((phase_of(&h), h.generation(), h.measure()), (Phase::Idle, 0, 0));

    shown(&mut h, "id:x");
    let (g, m) = (h.generation(), h.measure());
    h.rekey(k("app:x"));
    assert_eq!(phase_of(&h), Phase::Shown { key: k("app:x") });
    assert_eq!((h.generation(), h.anchor()), (g, Some(anchor())));
    assert_eq!(h.measure(), m + 1);
    step(&mut h, Event::Anchor(m, Some(anchor_b())), &cx());
    assert_eq!(h.anchor(), Some(anchor()), "old measure dropped");
    let fx = step(&mut h, Event::Relayout, &cx());
    assert_eq!(fx, vec![Effect::Measure(k("app:x"), m + 1)]);
    step(&mut h, Event::Anchor(m + 1, Some(anchor_b())), &cx());
    assert_eq!(h.anchor(), Some(anchor_b()));

    let mut h = Hover::default();
    leaving(&mut h, "id:x");
    h.rekey(k("app:x"));
    assert_eq!(phase_of(&h), Phase::Leaving { key: k("app:x") });
}

#[test]
fn click_on_the_shown_group_changes_nothing() {
    let mut h = Hover::default();
    shown(&mut h, "a");
    let g = h.generation();
    assert!(step(&mut h, Event::ClickGroup(k("a")), &cx()).is_empty());
    assert_eq!((phase_of(&h), h.generation()), (Phase::Shown { key: k("a") }, g));
}
