use super::*;

// (f) property: never stuck
struct Lcg(u64);
impl Lcg {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }
}

fn random_event(rng: &mut Lcg, waits: &[u64], cur: u64, meas: u64) -> Event {
    let name = ["a", "b", "c"][rng.next(3) as usize].to_string();
    match rng.next(11) {
        0 => Event::EnterIcon(name),
        1 => Event::LeaveIcon(name),
        2 => Event::ClickGroup(name),
        3 | 4 => Event::Pointer(pt(rng.next(500) as f32, rng.next(700) as f32)),
        5 => Event::PointerGone,
        6 => {
            if rng.next(2) == 0 {
                Event::Relayout
            } else {
                Event::Wheel
            }
        }
        7 => Event::Close,
        8 => Event::Anchor(meas, Some(anchor())),
        9 => Event::Anchor(meas, None),
        _ => Event::Timer(waits.get(rng.next(waits.len().max(1) as u64) as usize).copied().unwrap_or(cur)),
    }
}

#[test]
fn property_never_stuck() {
    let mut rng = Lcg(42);
    for case in 0..200 {
        let mut h = Hover::default();
        let mut waits: Vec<u64> = Vec::new();
        let mut pending: Vec<Event> = Vec::new();
        for _ in 0..(1 + rng.next(40)) {
            let ev = if !pending.is_empty() && rng.next(2) == 0 {
                pending.remove(0)
            } else {
                random_event(&mut rng, &waits, h.generation(), h.measure())
            };
            let blocked = rng.next(6) == 0;
            let present = rng.next(5) != 0;
            let ctx = Ctx { blocked, bounds: screen(), count: move |_: &str| usize::from(present) };
            for e in step(&mut h, ev, &ctx) {
                match e {
                    Effect::Wait(_, g) => waits.push(g),
                    Effect::Measure(_, g) if rng.next(2) == 0 => {
                        pending.push(Event::Anchor(g, Some(anchor())));
                    }
                    _ => {}
                }
            }
        }
        for _ in 0..4 {
            if *h.phase() == Phase::Idle {
                break;
            }
            for e in step(&mut h, Event::Pointer(pt(10000.0, 10000.0)), &cx()) {
                if let Effect::Wait(_, g) = e {
                    waits.push(g);
                }
            }
            if *h.phase() == Phase::Idle {
                break;
            }
            let g = h.generation();
            assert!(waits.contains(&g), "case {case}: stuck in {:?} without timer", h.phase());
            for e in step(&mut h, Event::Timer(g), &cx()) {
                if let Effect::Wait(_, g) = e {
                    waits.push(g);
                }
            }
        }
        assert_eq!(*h.phase(), Phase::Idle, "case {case}");
    }
}
