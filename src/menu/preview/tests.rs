use super::*;

mod behavior;
mod geometry;
mod property;

fn r(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle { x, y, width: w, height: h }
}
fn pt(x: f32, y: f32) -> Point {
    Point::new(x, y)
}
fn k(s: &str) -> String {
    s.to_string()
}
fn screen() -> Rectangle {
    r(0.0, 0.0, 874.0, 700.0)
}
/// Context where every group has `n` windows.
fn cxn(n: usize) -> Ctx<impl Fn(&str) -> usize> {
    Ctx { blocked: false, bounds: screen(), count: move |_| n }
}
fn cx() -> Ctx<impl Fn(&str) -> usize> {
    cxn(1)
}
fn bcx() -> Ctx<impl Fn(&str) -> usize> {
    Ctx { blocked: true, bounds: screen(), count: |_| 1 }
}
/// Context where no group exists any more.
fn gone() -> Ctx<impl Fn(&str) -> usize> {
    cxn(0)
}
fn anchor() -> Rectangle {
    r(100.0, 500.0, 40.0, 40.0)
}
/// Card of one window over `anchor()`: (0, 312, 240, 180).
fn card() -> Rectangle {
    card_rect(anchor(), 1, screen())
}
fn phase_of(h: &Hover) -> Phase {
    h.phase().clone()
}

/// Delivers the answer of the current measure to the machine.
fn answer(h: &mut Hover, r: Option<Rectangle>) -> Vec<Effect> {
    let m = h.measure();
    step(h, Event::Anchor(m, r), &cx())
}

/// Idle -> Waiting -> (anchor) -> Shown with the pointer inside the card.
fn shown(h: &mut Hover, key: &str) {
    step(h, Event::EnterIcon(k(key)), &cx());
    let g = h.generation();
    step(h, Event::Anchor(h.measure(), Some(anchor())), &cx());
    let fx = step(h, Event::Timer(g), &cx());
    assert_eq!(fx, vec![Effect::StartCapture(k(key))]);
    step(h, Event::Pointer(pt(120.0, 350.0)), &cx());
    assert_eq!(phase_of(h), Phase::Shown { key: k(key) });
}

fn leaving(h: &mut Hover, key: &str) -> u64 {
    shown(h, key);
    let fx = step(h, Event::Pointer(pt(900.0, 900.0)), &cx());
    assert_eq!(phase_of(h), Phase::Leaving { key: k(key) });
    assert_eq!(fx, vec![Effect::Wait(CLOSE, h.generation())]);
    h.generation()
}

fn waiting(h: &mut Hover, key: &str) -> u64 {
    let fx = step(h, Event::EnterIcon(k(key)), &cx());
    let g = h.generation();
    assert_eq!(fx, vec![Effect::Measure(k(key), h.measure()), Effect::Wait(OPEN, g)]);
    g
}

#[test]
fn idle_enter_starts_waiting() {
    let mut h = Hover::default();
    let g = waiting(&mut h, "a");
    assert_eq!(phase_of(&h), Phase::Waiting { key: k("a") });
    assert_eq!(g, 1);
}

#[test]
fn waiting_enter_other_restarts() {
    let mut h = Hover::default();
    waiting(&mut h, "a");
    answer(&mut h, Some(anchor()));
    let g = waiting(&mut h, "b");
    assert_eq!(g, 2);
    assert!(h.anchor().is_none());
}

#[test]
fn waiting_leave_pointer_gone_and_pointer_outside_go_idle() {
    let mut h = Hover::default();
    waiting(&mut h, "a");
    step(&mut h, Event::LeaveIcon(k("a")), &cx());
    assert_eq!(phase_of(&h), Phase::Idle);

    waiting(&mut h, "a");
    step(&mut h, Event::PointerGone, &cx());
    assert_eq!(phase_of(&h), Phase::Idle);

    waiting(&mut h, "a");
    step(&mut h, Event::Pointer(pt(900.0, 900.0)), &cx());
    assert_eq!(phase_of(&h), Phase::Waiting { key: k("a") }, "no anchor yet");
    answer(&mut h, Some(anchor()));
    step(&mut h, Event::Pointer(pt(900.0, 900.0)), &cx());
    assert_eq!(phase_of(&h), Phase::Idle);
}

#[test]
fn waiting_timer_rules() {
    let mut h = Hover::default();
    let g = waiting(&mut h, "a");
    step(&mut h, Event::Timer(g), &cx());
    assert_eq!(phase_of(&h), Phase::Idle, "anchor missing");

    let g = waiting(&mut h, "a");
    answer(&mut h, Some(anchor()));
    step(&mut h, Event::Timer(g), &gone());
    assert_eq!(phase_of(&h), Phase::Idle, "group gone");
}

#[test]
fn click_opens_at_once() {
    for start in ["idle", "waiting", "leaving"] {
        let mut h = Hover::default();
        match start {
            "waiting" => {
                waiting(&mut h, "a");
            }
            "leaving" => {
                leaving(&mut h, "a");
            }
            _ => {}
        }
        let fx = step(&mut h, Event::ClickGroup(k("b")), &cx());
        assert_eq!(phase_of(&h), Phase::Shown { key: k("b") }, "{start}");
        let m = h.measure();
        assert_eq!(fx, vec![Effect::Measure(k("b"), m), Effect::StartCapture(k("b"))]);
    }
}

#[test]
fn click_blocked_or_missing_stays_idle() {
    let mut h = Hover::default();
    assert!(step(&mut h, Event::ClickGroup(k("a")), &bcx()).is_empty());
    assert!(step(&mut h, Event::ClickGroup(k("a")), &gone()).is_empty());
    assert!(step(&mut h, Event::EnterIcon(k("a")), &bcx()).is_empty());
    assert_eq!(phase_of(&h), Phase::Idle);
    assert_eq!(h.generation(), 0);
}

#[test]
fn shown_enter_other_switches_on_the_spot() {
    let mut h = Hover::default();
    shown(&mut h, "a");
    let fx = step(&mut h, Event::EnterIcon(k("b")), &cx());
    let m = h.measure();
    assert_eq!(phase_of(&h), Phase::Shown { key: k("b") });
    assert_eq!(fx, vec![Effect::Measure(k("b"), m), Effect::StartCapture(k("b"))]);
    assert!(h.anchor().is_none());
}

#[test]
fn shown_leave_icon_inside_stays_outside_leaves() {
    let mut h = Hover::default();
    shown(&mut h, "a");
    let g = h.generation();
    assert!(step(&mut h, Event::LeaveIcon(k("a")), &cx()).is_empty());
    assert_eq!(h.generation(), g);
    step(&mut h, Event::Pointer(pt(900.0, 900.0)), &cx());
    assert!(matches!(h.phase(), Phase::Leaving { .. }));
}

#[test]
fn leaving_returns_when_pointer_comes_back() {
    let mut h = Hover::default();
    let g = leaving(&mut h, "a");
    let fx = step(&mut h, Event::Pointer(pt(120.0, 350.0)), &cx());
    assert!(fx.is_empty());
    assert_eq!(phase_of(&h), Phase::Shown { key: k("a") });
    let fx = step(&mut h, Event::Timer(g), &cx());
    assert!(fx.is_empty(), "old timer cancelled by gen");
    assert_eq!(phase_of(&h), Phase::Shown { key: k("a") });

    h = Hover::default();
    leaving(&mut h, "a");
    step(&mut h, Event::EnterIcon(k("a")), &cx());
    assert_eq!(phase_of(&h), Phase::Shown { key: k("a") });
}

#[test]
fn leaving_enter_other_switches() {
    let mut h = Hover::default();
    leaving(&mut h, "a");
    let fx = step(&mut h, Event::EnterIcon(k("b")), &cx());
    assert_eq!(phase_of(&h), Phase::Shown { key: k("b") });
    assert_eq!(fx, vec![Effect::Measure(k("b"), h.measure()), Effect::StartCapture(k("b"))]);
}

#[test]
fn leaving_timer_inside_reopens_else_idle() {
    let mut h = Hover::default();
    let g = leaving(&mut h, "a");
    // pointer returns without the state machine seeing it as a transition
    h.pointer = Some(pt(120.0, 350.0));
    step(&mut h, Event::Timer(g), &cx());
    assert_eq!(phase_of(&h), Phase::Shown { key: k("a") });

    h = Hover::default();
    let g = leaving(&mut h, "a");
    let fx = step(&mut h, Event::Timer(g), &cx());
    assert_eq!(phase_of(&h), Phase::Idle);
    assert_eq!(fx, vec![Effect::StopCapture]);

    h = Hover::default();
    let g = leaving(&mut h, "a");
    let fx = step(&mut h, Event::Timer(g), &gone());
    assert_eq!(fx, vec![Effect::StopCapture]);
}

// (a) lost exit event
#[test]
fn regression_lost_exit_still_closes() {
    let mut h = Hover::default();
    shown(&mut h, "a");
    let fx = step(&mut h, Event::Pointer(pt(900.0, 20.0)), &cx());
    assert_eq!(phase_of(&h), Phase::Leaving { key: k("a") });
    assert_eq!(fx, vec![Effect::Wait(CLOSE, h.generation())]);
    let gn = h.generation();
    let fx = step(&mut h, Event::Timer(gn), &cx());
    assert_eq!(phase_of(&h), Phase::Idle);
    assert_eq!(fx, vec![Effect::StopCapture]);
}

// (b) duplicated events
#[test]
fn regression_duplicates_are_idempotent() {
    let mut h = Hover::default();
    waiting(&mut h, "a");
    let g = h.generation();
    assert!(step(&mut h, Event::EnterIcon(k("a")), &cx()).is_empty());
    assert_eq!((h.generation(), phase_of(&h)), (g, Phase::Waiting { key: k("a") }));

    let mut h = Hover::default();
    shown(&mut h, "a");
    let (g, p) = (h.generation(), phase_of(&h));
    for _ in 0..2 {
        assert!(step(&mut h, Event::EnterIcon(k("a")), &cx()).is_empty());
        assert!(step(&mut h, Event::ClickGroup(k("a")), &cx()).is_empty());
        assert!(step(&mut h, Event::LeaveIcon(k("a")), &cx()).is_empty());
    }
    assert_eq!((h.generation(), phase_of(&h)), (g, p));

    let mut h = Hover::default();
    let g = leaving(&mut h, "a");
    for _ in 0..2 {
        assert!(step(&mut h, Event::LeaveIcon(k("a")), &cx()).is_empty());
        assert!(step(&mut h, Event::PointerGone, &cx()).is_empty());
    }
    assert_eq!((h.generation(), phase_of(&h)), (g, Phase::Leaving { key: k("a") }));
}

// (c) block during waiting / shown
#[test]
fn regression_block_in_waiting_then_next_hover_opens() {
    let mut h = Hover::default();
    let g = waiting(&mut h, "a");
    answer(&mut h, Some(anchor()));
    step(&mut h, Event::Timer(g), &bcx());
    assert_eq!(phase_of(&h), Phase::Idle);
    shown(&mut h, "a");
}

#[test]
fn regression_block_in_shown_then_next_hover_opens() {
    let mut h = Hover::default();
    shown(&mut h, "a");
    let fx = step(&mut h, Event::Pointer(pt(120.0, 350.0)), &bcx());
    assert_eq!(phase_of(&h), Phase::Idle);
    assert_eq!(fx, vec![Effect::StopCapture]);
    shown(&mut h, "b");
}

// (d) Close in every phase
#[test]
fn regression_close_from_every_phase() {
    for n in 0..4 {
        let mut h = Hover::default();
        match n {
            1 => {
                waiting(&mut h, "a");
            }
            2 => shown(&mut h, "a"),
            3 => {
                leaving(&mut h, "a");
            }
            _ => {}
        }
        let was_shown = h.is_shown();
        let fx = step(&mut h, Event::Close, &cx());
        assert_eq!(phase_of(&h), Phase::Idle);
        assert_eq!(fx.contains(&Effect::StopCapture), was_shown, "phase {n}");
        shown(&mut h, "a");
    }
}

// (e) stale generations
#[test]
fn regression_stale_timer_and_anchor_ignored() {
    let mut h = Hover::default();
    let g1 = waiting(&mut h, "a");
    let m1 = h.measure();
    let g2 = waiting(&mut h, "b");
    let m2 = h.measure();
    assert_ne!(m1, m2);
    step(&mut h, Event::Anchor(m1, Some(anchor())), &cx());
    assert!(h.anchor().is_none());
    step(&mut h, Event::Anchor(m2, Some(anchor())), &cx());
    let fx = step(&mut h, Event::Timer(g1), &cx());
    assert!(fx.is_empty());
    assert_eq!(phase_of(&h), Phase::Waiting { key: k("b") });
    step(&mut h, Event::Timer(g2), &cx());
    assert_eq!(phase_of(&h), Phase::Shown { key: k("b") });
}

// R2
#[test]
fn anchor_none_resolves_to_idle() {
    let mut h = Hover::default();
    waiting(&mut h, "a");
    answer(&mut h, None);
    assert_eq!(phase_of(&h), Phase::Idle);

    let mut h = Hover::default();
    step(&mut h, Event::ClickGroup(k("a")), &cx());
    let fx = answer(&mut h, None);
    assert_eq!(phase_of(&h), Phase::Idle);
    assert_eq!(fx, vec![Effect::StopCapture]);
}

// R3
#[test]
fn shown_without_anchor_treats_pointer_as_outside() {
    let mut h = Hover::default();
    step(&mut h, Event::ClickGroup(k("a")), &cx());
    let fx = step(&mut h, Event::Pointer(pt(120.0, 350.0)), &cx());
    assert_eq!(phase_of(&h), Phase::Leaving { key: k("a") });
    assert_eq!(fx, vec![Effect::Wait(CLOSE, h.generation())]);

    // the measure answer is not discarded: pointer still inside => back to Shown
    let m = h.measure();
    step(&mut h, Event::Anchor(m, Some(anchor())), &cx());
    assert_eq!(phase_of(&h), Phase::Shown { key: k("a") });
    assert_eq!(h.anchor(), Some(anchor()));
}

// R4
#[test]
fn vanished_group_goes_idle() {
    let mut h = Hover::default();
    shown(&mut h, "a");
    let fx = step(&mut h, Event::Pointer(pt(120.0, 350.0)), &gone());
    assert_eq!(phase_of(&h), Phase::Idle);
    assert_eq!(fx, vec![Effect::StopCapture]);
}

#[test]
fn pointer_gone_clears_pointer() {
    let mut h = Hover::default();
    shown(&mut h, "a");
    assert!(inside(&h, 1, screen()));
    step(&mut h, Event::PointerGone, &cx());
    assert!(!inside(&h, 1, screen()));
}

// (g)
#[test]
fn regression_continuous_motion_outside_does_not_postpone() {
    let mut h = Hover::default();
    shown(&mut h, "a");
    let mut waits = 0;
    for i in 0..10 {
        let fx = step(&mut h, Event::Pointer(pt(800.0 + i as f32, 900.0)), &cx());
        waits += fx.iter().filter(|e| matches!(e, Effect::Wait(..))).count();
    }
    assert_eq!(waits, 1);
    let gn = h.generation();
    step(&mut h, Event::Timer(gn), &cx());
    assert_eq!(phase_of(&h), Phase::Idle);
}
