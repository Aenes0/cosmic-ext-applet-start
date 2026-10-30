//! Hidden-menu memory pageout.
//!
//! Some seconds after the menu hides (CPU renderer only), private anonymous
//! pages and the heap are handed back to the kernel with `MADV_PAGEOUT`.
//! The content is preserved: pages come back through a page fault.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Extra wait after the 500 ms `Message::Trim` delay (about 10 s in total).
const PAGEOUT_DELAY: Duration = Duration::from_millis(9500);

/// Smallest range worth a syscall.
const MIN_RANGE_LEN: usize = 64 * 1024;

/// Bumped on every schedule so older threads notice and abort.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Parses one `/proc/self/maps` line into `(start, end)` when it is a
/// private anonymous or heap read-write mapping.
fn parse_line(line: &str) -> Option<(usize, usize)> {
    let mut fields = line.split_whitespace();
    let range = fields.next()?;
    let perms = fields.next()?;
    let _offset = fields.next()?;
    let _dev = fields.next()?;
    let _inode = fields.next()?;
    let path = fields.next();
    if perms != "rw-p" || !matches!(path, None | Some("[heap]")) {
        return None;
    }
    let (start, end) = range.split_once('-')?;
    let start = usize::from_str_radix(start, 16).ok()?;
    let end = usize::from_str_radix(end, 16).ok()?;
    end.checked_sub(start)?;
    Some((start, end))
}

/// Ranges of `maps` that are safe and useful to page out.
pub(super) fn pageout_ranges(maps: &str, min_len: usize) -> Vec<(usize, usize)> {
    maps.lines()
        .filter_map(parse_line)
        .filter(|(start, end)| end.saturating_sub(*start) >= min_len)
        .collect()
}

/// True while the pageout should go on.
fn keep_going(scheduled_gen: u64, current_gen: u64, shown: bool) -> bool {
    scheduled_gen == current_gen && !shown
}

/// Advises the kernel to page out `[start, start + len)`; true on success.
fn advise_pageout(start: usize, len: usize) -> bool {
    // SAFETY: madvise is only a hint. MADV_PAGEOUT keeps the content (pages
    // return by page fault), so concurrent writers just fault again. The
    // ranges come from /proc/self/maps and are page aligned. Failures
    // (EINVAL, ENOMEM, EAGAIN, no swap) are harmless.
    let rc = unsafe { libc::madvise(start as *mut libc::c_void, len, libc::MADV_PAGEOUT) };
    rc == 0
}

/// Pages out every eligible range, stopping as soon as the menu is back.
fn run(generation: u64) {
    let Ok(maps) = std::fs::read_to_string("/proc/self/maps") else {
        return;
    };
    let current = || (GENERATION.load(Ordering::Relaxed), super::WINDOWS_SHOWN.load(Ordering::Relaxed));
    for (start, end) in pageout_ranges(&maps, MIN_RANGE_LEN) {
        let (gen_now, shown) = current();
        if !keep_going(generation, gen_now, shown) {
            return;
        }
        if !advise_pageout(start, end - start) {
            tracing::debug!("pageout refused for {start:#x}-{end:#x}");
        }
    }
}

/// Schedules a pageout off the UI thread (CPU renderer only).
pub(super) fn schedule() {
    if !crate::backend::is_cpu() {
        return;
    }
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    let spawned = std::thread::Builder::new()
        .name("start-menu-pageout".into())
        .spawn(move || {
            std::thread::sleep(PAGEOUT_DELAY);
            let shown = super::WINDOWS_SHOWN.load(Ordering::Relaxed);
            if keep_going(generation, GENERATION.load(Ordering::Relaxed), shown) {
                run(generation);
            }
        });
    if let Err(err) = spawned {
        tracing::warn!("could not spawn pageout thread: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: usize = 0x10000;

    fn one(line: &str) -> Vec<(usize, usize)> {
        pageout_ranges(line, MIN)
    }

    #[test]
    fn includes_anonymous_rw_private() {
        let l = "7f0000000000-7f0000100000 rw-p 00000000 00:00 0 \n";
        assert_eq!(one(l), vec![(0x7f0000000000, 0x7f0000100000)]);
    }

    #[test]
    fn includes_heap() {
        let l = "55d000000000-55d000200000 rw-p 00000000 00:00 0   [heap]";
        assert_eq!(one(l), vec![(0x55d000000000, 0x55d000200000)]);
    }

    #[test]
    fn excludes_shared_mappings() {
        let l = "7f0000000000-7f0000100000 rw-s 00000000 00:01 5 /memfd:x (deleted)";
        assert!(one(l).is_empty());
        let l = "7f0000000000-7f0000100000 rw-s 00000000 00:00 0";
        assert!(one(l).is_empty());
    }

    #[test]
    fn excludes_stack_and_special() {
        for name in ["[stack]", "[vvar]", "[vdso]", "[vsyscall]"] {
            let l = format!("7ffc00000000-7ffc00200000 rw-p 00000000 00:00 0 {name}");
            assert!(one(&l).is_empty(), "{name}");
        }
    }

    #[test]
    fn excludes_file_backed_rw() {
        let l = "7f0000000000-7f0000100000 rw-p 00001000 08:01 42 /usr/lib/libx.so";
        assert!(one(l).is_empty());
    }

    #[test]
    fn excludes_other_permissions() {
        for perms in ["r--p", "r-xp", "---p", "rwxp"] {
            let l = format!("7f0000000000-7f0000100000 {perms} 00000000 00:00 0");
            assert!(one(&l).is_empty(), "{perms}");
        }
    }

    #[test]
    fn ignores_malformed_lines() {
        let maps = "\nshort\n\
            7f0000000000 rw-p 00000000 00:00 0\n\
            zzzz-7f0000100000 rw-p 00000000 00:00 0\n\
            7f0000000000-xyz rw-p 00000000 00:00 0\n\
            7f0000100000-7f0000000000 rw-p 00000000 00:00 0\n\
            7f0000000000-7f0000100000 rw-p\n\
            -\n";
        assert!(one(maps).is_empty());
    }

    #[test]
    fn applies_minimum_size() {
        let small = "7f0000000000-7f0000008000 rw-p 00000000 00:00 0";
        let exact = "7f0000100000-7f0000110000 rw-p 00000000 00:00 0";
        let maps = format!("{small}\n{exact}\n");
        assert_eq!(one(&maps), vec![(0x7f0000100000, 0x7f0000110000)]);
    }

    #[test]
    fn empty_input_gives_nothing() {
        assert!(one("").is_empty());
    }

    #[test]
    fn keeps_going_only_when_hidden_and_current() {
        assert!(keep_going(3, 3, false));
        assert!(!keep_going(3, 3, true));
        assert!(!keep_going(3, 4, false));
    }

    #[test]
    fn pageout_preserves_content() {
        // SAFETY: sysconf has no preconditions.
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(4096) as usize;
        let mut data: Vec<u8> = (0..1 << 20).map(|i| (i * 31 % 251) as u8).collect();
        let expected = data.clone();
        let base = data.as_mut_ptr() as usize;
        let start = base.div_ceil(page) * page;
        let end = (base + data.len()) / page * page;
        assert!(end > start);
        // The kernel may refuse (no swap): only the content matters.
        let _ = advise_pageout(start, end - start);
        assert_eq!(data, expected);
    }
}
