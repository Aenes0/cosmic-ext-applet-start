//! Icons resolved by name, kept out of the view: resolving one can take
//! tens of milliseconds when the theme has to be searched.

use super::*;

/// Handles by key, filled on `update` and only read on `view`.
#[derive(Default)]
pub(super) struct IconCache {
    map: HashMap<String, icon::Handle>,
}

impl IconCache {
    pub(super) fn get(&self, key: &str) -> Option<&icon::Handle> {
        self.map.get(key)
    }

    /// Keeps only the wanted keys, resolving the ones not yet here.
    pub(super) fn refill<'a>(
        &mut self,
        wanted: impl IntoIterator<Item = (&'a str, Vec<&'a str>)>,
        resolve: impl Fn(&[&str]) -> icon::Handle,
    ) {
        let mut next = HashMap::new();
        for (key, names) in wanted {
            if next.contains_key(key) {
                continue;
            }
            let handle = match self.map.remove(key) {
                Some(handle) => handle,
                None if names.is_empty() => continue,
                None => resolve(&names),
            };
            next.insert(key.to_owned(), handle);
        }
        self.map = next;
    }

    pub(super) fn clear(&mut self) {
        self.map.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn cheap(count: &Cell<usize>) -> impl Fn(&[&str]) -> icon::Handle + '_ {
        move |_| {
            count.set(count.get() + 1);
            icon::from_svg_bytes(&b"<svg/>"[..])
        }
    }

    #[test]
    fn unknown_window_icon_is_resolved_once() {
        let count = Cell::new(0);
        let mut cache = IconCache::default();
        cache.refill([("claude", vec!["claude", "application-default"])], cheap(&count));
        cache.refill([("claude", vec!["claude", "application-default"])], cheap(&count));
        assert_eq!(count.get(), 1);
        assert!(cache.get("claude").is_some());
        assert!(cache.get("claude").is_some());
    }

    #[test]
    fn refill_forgets_keys_no_longer_wanted() {
        let count = Cell::new(0);
        let mut cache = IconCache::default();
        cache.refill([("a", vec!["a"]), ("b", vec!["b"])], cheap(&count));
        cache.refill([("b", vec!["b"])], cheap(&count));
        assert!(cache.get("a").is_none());
        assert!(cache.get("b").is_some());
        assert_eq!(count.get(), 2);
    }

    #[test]
    fn clear_makes_the_next_refill_resolve_again() {
        let count = Cell::new(0);
        let mut cache = IconCache::default();
        cache.refill([("a", vec!["a"])], cheap(&count));
        cache.clear();
        cache.refill([("a", vec!["a"])], cheap(&count));
        assert_eq!(count.get(), 2);
    }
}
