//! One row of the open windows: what fits, and the page shown.

use std::ops::Range;

pub const CELL: f32 = 65.0;
pub const GAP: f32 = 8.0;
pub const ARROW: f32 = 28.0;

/// Largest `n` with `n*cell + (n-1)*gap <= width`; at least 1.
pub fn fit(width: f32, cell: f32, gap: f32) -> usize {
    let step = cell + gap;
    if step <= 0.0 || width <= 0.0 {
        return 1;
    }
    (((width + gap) / step).floor() as usize).max(1)
}

/// Icons per page for a strip with `inner` usable width: everything if it
/// fits, otherwise less (arrows take room).
pub fn capacity_in(total: usize, inner: f32) -> usize {
    let full = fit(inner, CELL, GAP);
    if total <= full {
        full
    } else {
        fit(inner - 2.0 * (ARROW + GAP), CELL, GAP)
    }
}

/// Offset limited so the last page never shows a hole.
pub fn clamp(total: usize, cap: usize, offset: usize) -> usize {
    offset.min(total.saturating_sub(cap))
}

/// Indices shown for the (clamped) offset.
pub fn visible_range(total: usize, cap: usize, offset: usize) -> Range<usize> {
    let start = clamp(total, cap, offset);
    start..start.saturating_add(cap).min(total)
}

pub fn can_prev(offset: usize) -> bool {
    offset > 0
}

pub fn can_next(total: usize, cap: usize, offset: usize) -> bool {
    offset.saturating_add(cap) < total
}

/// One full page forward, stuck to the end.
pub fn next(total: usize, cap: usize, offset: usize) -> usize {
    clamp(total, cap, offset.saturating_add(cap))
}

/// One full page back.
pub fn prev(cap: usize, offset: usize) -> usize {
    offset.saturating_sub(cap)
}

/// Smallest move that shows `index`; no-op if already visible.
pub fn reveal(total: usize, cap: usize, offset: usize, index: usize) -> usize {
    let moved = if index < offset {
        index
    } else if index >= offset.saturating_add(cap) {
        index.saturating_add(1).saturating_sub(cap)
    } else {
        offset
    };
    clamp(total, cap, moved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_real_widths() {
        assert_eq!(fit(758.0, 65.0, 8.0), 10);
        assert_eq!(fit(686.0, 65.0, 8.0), 9);
        assert_eq!(fit(1.0, 65.0, 8.0), 1);
    }

    #[test]
    fn capacity_drops_with_overflow() {
        for t in 0..=10 {
            assert_eq!(capacity_in(t, 758.0), 10);
        }
        assert_eq!(capacity_in(11, 758.0), 9);
    }

    #[test]
    fn capacity_in_with_and_without_arrows() {
        let cases = [
            (328.0, 4, 3),
            (400.0, 5, 4),
            (510.0, 7, 6),
            (618.0, 8, 7),
            (758.0, 10, 9),
        ];
        for (inner, plain, arrows) in cases {
            let full = fit(inner, CELL, GAP);
            assert_eq!(capacity_in(full, inner), plain, "inner {inner}");
            assert_eq!(capacity_in(full + 1, inner), arrows, "inner {inner}");
        }
    }

    #[test]
    fn capacity_in_never_zero() {
        assert!(capacity_in(5, 0.0) >= 1);
    }

    #[test]
    fn no_overflow_has_no_arrows() {
        assert_eq!(visible_range(7, 10, 0), 0..7);
        assert_eq!(visible_range(7, 10, 5), 0..7);
        assert!(!can_prev(0));
        assert!(!can_next(7, 10, 0));
    }

    #[test]
    fn paging_forward_and_back() {
        assert_eq!(next(20, 9, 0), 9);
        assert_eq!(next(20, 9, 9), 11);
        assert_eq!(next(20, 9, 11), 11);
        assert_eq!(prev(9, 11), 2);
        assert_eq!(prev(9, 2), 0);
        assert_eq!(visible_range(20, 9, 11), 11..20);
    }

    #[test]
    fn arrows_at_edges() {
        assert!(!can_prev(0));
        assert!(can_prev(1));
        assert!(can_next(20, 9, 10));
        assert!(!can_next(20, 9, 11));
    }

    #[test]
    fn clamp_when_windows_close() {
        assert_eq!(clamp(9, 9, 3), 0);
        assert_eq!(clamp(12, 9, 5), 3);
        assert_eq!(visible_range(9, 9, 3), 0..9);
    }

    #[test]
    fn reveal_both_ways_and_noop() {
        assert_eq!(reveal(20, 9, 11, 3), 3);
        assert_eq!(reveal(20, 9, 0, 12), 4);
        assert_eq!(reveal(20, 9, 4, 8), 4);
        assert_eq!(reveal(20, 9, 0, 19), 11);
        assert_eq!(reveal(5, 10, 0, 3), 0);
    }

    #[test]
    fn empty_and_tiny_capacity_never_panic() {
        assert_eq!(visible_range(0, 10, 3), 0..0);
        assert_eq!(next(0, 10, 0), 0);
        assert_eq!(reveal(0, 10, 0, 4), 0);
        assert!(visible_range(5, 0, 2).is_empty());
        assert_eq!(next(5, 0, 2), 2);
        assert_eq!(reveal(5, 0, 2, 4), 5);
        assert_eq!(visible_range(5, 1, 9), 4..5);
        assert_eq!(reveal(5, 1, 0, 99), 4);
        assert!(!can_next(0, 0, 0));
        assert_eq!(next(5, usize::MAX, usize::MAX), 0);
    }
}
