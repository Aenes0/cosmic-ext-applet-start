// SPDX-License-Identifier: GPL-3.0-only

//! Measurement only: how many `CursorMoved` events arrive per second during a
//! gesture (resize, drag, preview), and how many repeat the previous point.
//! Off unless `COSMIC_START_STATS=1`; it never drops or changes an event.

use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use cosmic::iced;

/// How long a window of counting lasts before it is reported.
const REPORT_EVERY: Duration = Duration::from_secs(5);
/// A silence longer than this means the gesture ended.
const GESTURE_GAP: Duration = Duration::from_secs(1);

static ENABLED: AtomicBool = AtomicBool::new(false);
static STATS: Mutex<[Tally; 3]> = Mutex::new([Tally::new(), Tally::new(), Tally::new()]);

#[derive(Clone, Copy)]
pub(super) enum Gesture {
    Resize,
    Drag,
    Preview,
}

impl Gesture {
    fn index(self) -> usize {
        self as usize
    }

    fn name(self) -> &'static str {
        match self {
            Gesture::Resize => "resize",
            Gesture::Drag => "drag",
            Gesture::Preview => "preview",
        }
    }
}

#[derive(Debug, PartialEq)]
struct Report {
    total: u64,
    same: u64,
    secs: f32,
}

struct Tally {
    start: Option<Instant>,
    last_at: Option<Instant>,
    last: Option<(f32, f32)>,
    total: u64,
    same: u64,
}

impl Default for Tally {
    fn default() -> Self {
        Self::new()
    }
}

impl Tally {
    const fn new() -> Self {
        Self { start: None, last_at: None, last: None, total: 0, same: 0 }
    }

    /// Counts one event; gives a report once the window is full.
    fn note(&mut self, now: Instant, point: (f32, f32)) -> Option<Report> {
        let gap_over = self.last_at.is_some_and(|at| now.saturating_duration_since(at) > GESTURE_GAP);
        if gap_over || self.start.is_none() {
            self.restart(now);
        }
        self.total += 1;
        if self.last == Some(point) {
            self.same += 1;
        }
        self.last = Some(point);
        self.last_at = Some(now);
        let start = self.start.unwrap_or(now);
        let elapsed = now.saturating_duration_since(start);
        if elapsed < REPORT_EVERY {
            return None;
        }
        let report = Report { total: self.total, same: self.same, secs: elapsed.as_secs_f32() };
        self.restart(now);
        Some(report)
    }

    fn restart(&mut self, now: Instant) {
        self.start = Some(now);
        self.total = 0;
        self.same = 0;
    }
}

/// Turns the counting on when `COSMIC_START_STATS` is exactly `1`.
pub(super) fn init_from_env() {
    let on = std::env::var_os("COSMIC_START_STATS").is_some_and(|v| v == "1");
    ENABLED.store(on, Ordering::Relaxed);
}

/// Notes a `CursorMoved`; one atomic read when the counting is off.
pub(super) fn note(gesture: Gesture, position: iced::Point) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let report = {
        let mut stats = STATS.lock().unwrap_or_else(|e| e.into_inner());
        stats[gesture.index()].note(Instant::now(), (position.x, position.y))
    };
    if let Some(r) = report {
        tracing::info!(
            "pointer stats: gesture={} total={} per_second={:.1} same_point={} window_s={:.1}",
            gesture.name(),
            r.total,
            r.total as f32 / r.secs.max(f32::EPSILON),
            r.same,
            r.secs
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(base: Instant, n: u64) -> Instant {
        base + Duration::from_millis(n)
    }

    #[test]
    fn a_report_comes_every_five_seconds() {
        let base = Instant::now();
        let mut tally = Tally::default();
        let mut reports = Vec::new();
        for i in 0..=500u64 {
            if let Some(r) = tally.note(ms(base, i * 10), (i as f32, 0.0)) {
                reports.push((i, r));
            }
        }
        assert_eq!(reports.len(), 1);
        let (at, report) = &reports[0];
        assert_eq!(*at, 500);
        assert_eq!(report.total, 501);
        assert_eq!(report.same, 0);
        assert_eq!(report.secs, 5.0);
    }

    #[test]
    fn same_points_are_counted() {
        let base = Instant::now();
        let mut tally = Tally::default();
        tally.note(ms(base, 0), (1.0, 1.0));
        tally.note(ms(base, 10), (1.0, 1.0));
        tally.note(ms(base, 20), (1.0, 1.0));
        tally.note(ms(base, 30), (2.0, 2.0));
        // Gaps of one second keep the same gesture going.
        for i in 1..=4u64 {
            assert!(tally.note(ms(base, 30 + i * 1_000), (3.0 + i as f32, 3.0)).is_none());
        }
        let report = tally.note(ms(base, 5_030), (9.0, 9.0)).expect("a report");
        assert_eq!(report.total, 9);
        assert_eq!(report.same, 2);
    }

    #[test]
    fn a_pause_starts_a_new_window() {
        let base = Instant::now();
        let mut tally = Tally::default();
        for i in 0..3u64 {
            assert!(tally.note(ms(base, i * 10), (i as f32, 0.0)).is_none());
        }
        let mut report = None;
        for i in 0..=500u64 {
            if let Some(r) = tally.note(ms(base, 2_020 + i * 10), (i as f32, 1.0)) {
                report = Some(r);
            }
        }
        let report = report.expect("a report");
        assert_eq!(report.total, 501);
    }

    #[test]
    fn disabled_by_default() {
        assert!(!ENABLED.load(Ordering::Relaxed));
        note(Gesture::Resize, iced::Point::new(1.0, 2.0));
        let stats = STATS.lock().unwrap_or_else(|e| e.into_inner());
        assert!(stats.iter().all(|t| t.total == 0));
    }
}
