//! The newest apps, and which of them are new.

/// An app counts as "new" for this long (15 days, inclusive).
pub const NEW_FOR: i64 = 15 * 86_400;
/// Tolerated clock skew for timestamps in the future.
pub const SKEW: i64 = 60;
/// A "new" app must be added more than this after the oldest known one.
const BASELINE_GUARD: i64 = 3_600;

fn valid(t: i64, now: i64) -> bool {
    t > 0 && t <= now.saturating_add(SKEW)
}

/// Install time: `created` if sane, else `modified` if sane, else 0 (unknown).
pub fn pick_added(created: Option<i64>, modified: Option<i64>, now: i64) -> i64 {
    [created, modified]
        .into_iter()
        .flatten()
        .find(|&t| valid(t, now))
        .unwrap_or(0)
}

/// Indices into `added` of the `n` newest (desc.), with the "new" badge flag.
pub fn newest_apps(added: &[i64], now: i64, n: usize) -> Vec<(usize, bool)> {
    let mut ranked: Vec<(usize, i64)> = added
        .iter()
        .copied()
        .enumerate()
        .filter(|&(_, a)| valid(a, now))
        .collect();
    let baseline = ranked.iter().map(|&(_, a)| a).min().unwrap_or(0);
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked
        .into_iter()
        .take(n)
        .map(|(i, a)| {
            let fresh = now.saturating_sub(a) <= NEW_FOR && a > baseline.saturating_add(BASELINE_GUARD);
            (i, fresh)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 2_000_000_000;
    const DAY: i64 = 86_400;

    #[test]
    fn border_15_days_inclusive() {
        let a = [NOW - 100 * DAY, NOW - NEW_FOR];
        assert_eq!(newest_apps(&a, NOW, 4), vec![(1, true), (0, false)]);
    }

    #[test]
    fn border_plus_one_second_no_badge() {
        let a = [NOW - 100 * DAY, NOW - NEW_FOR - 1];
        assert_eq!(newest_apps(&a, NOW, 4), vec![(1, false), (0, false)]);
    }

    #[test]
    fn tie_smaller_index_first() {
        let a = [NOW - 100 * DAY, NOW - DAY, NOW - DAY, NOW - DAY];
        let idx: Vec<usize> = newest_apps(&a, NOW, 4).iter().map(|x| x.0).collect();
        assert_eq!(idx, vec![1, 2, 3, 0]);
    }

    #[test]
    fn fewer_than_four_and_empty() {
        assert!(newest_apps(&[], NOW, 4).is_empty());
        assert_eq!(newest_apps(&[NOW - DAY], NOW, 4).len(), 1);
    }

    #[test]
    fn takes_only_n() {
        let a: Vec<i64> = (1..=10).map(|d| NOW - d * DAY).collect();
        let r = newest_apps(&a, NOW, 4);
        assert_eq!(r.iter().map(|x| x.0).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    }

    #[test]
    fn future_zero_negative_ignored() {
        let a = [0, -5, NOW + SKEW + 1, NOW - DAY, NOW - 50 * DAY];
        let r = newest_apps(&a, NOW, 4);
        assert_eq!(r, vec![(3, true), (4, false)]);
    }

    #[test]
    fn skew_tolerated() {
        let a = [NOW - 50 * DAY, NOW + SKEW];
        assert_eq!(newest_apps(&a, NOW, 4)[0].0, 1);
    }

    #[test]
    fn one_hour_guard_same_minute() {
        let a = [NOW - DAY, NOW - DAY + 30, NOW - DAY + 59];
        assert!(newest_apps(&a, NOW, 4).iter().all(|x| !x.1));
    }

    #[test]
    fn guard_needs_more_than_one_hour() {
        let a = [NOW - DAY, NOW - DAY + 3_600, NOW - DAY + 3_601];
        assert_eq!(newest_apps(&a, NOW, 4), vec![(2, true), (1, false), (0, false)]);
    }

    #[test]
    fn old_apps_in_top_without_badge() {
        let a = [NOW - 400 * DAY, NOW - 300 * DAY, NOW - 200 * DAY];
        let r = newest_apps(&a, NOW, 4);
        assert_eq!(r, vec![(2, false), (1, false), (0, false)]);
    }

    #[test]
    fn pick_added_valid_created() {
        assert_eq!(pick_added(Some(NOW - DAY), Some(NOW - 5 * DAY), NOW), NOW - DAY);
    }

    #[test]
    fn pick_added_future_created_uses_modified() {
        assert_eq!(pick_added(Some(NOW + 10 * DAY), Some(NOW - DAY), NOW), NOW - DAY);
        assert_eq!(pick_added(Some(NOW + SKEW), None, NOW), NOW + SKEW);
    }

    #[test]
    fn pick_added_none_is_zero() {
        assert_eq!(pick_added(None, None, NOW), 0);
        assert_eq!(pick_added(Some(0), Some(NOW + DAY), NOW), 0);
    }
}
