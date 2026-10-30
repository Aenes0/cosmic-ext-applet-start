use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use cosmic::cctk::screencopy::{Formats, ScreencopyHandler};
use cosmic::cctk::wayland_client::protocol::wl_shm;

use super::*;

const RED: [u8; 4] = [200, 0, 0, 255];
const GREEN: [u8; 4] = [0, 200, 0, 255];
const BLUE: [u8; 4] = [0, 0, 200, 255];
const GRAY: [u8; 4] = [100, 100, 100, 255];

/// Raw `w`x`h` picture with `stride` bytes per row, `paint(x, y)` per pixel,
/// padding bytes set to 0xEE.
fn raw_image(w: u32, h: u32, stride: u32, paint: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let mut raw = vec![0xEE; (stride * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let off = (y * stride + x * 4) as usize;
            raw[off..off + 4].copy_from_slice(&paint(x, y));
        }
    }
    raw
}

fn quadrants(x: u32, y: u32) -> [u8; 4] {
    match (x < 2, y < 2) {
        (true, true) => RED,
        (false, true) => GREEN,
        (true, false) => BLUE,
        (false, false) => GRAY,
    }
}

fn expect_img(res: Result<image::RgbaImage, ShrinkError>) -> image::RgbaImage {
    match res {
        Ok(img) => img,
        Err(err) => panic!("shrink failed: {err:?}"),
    }
}

#[test]
fn shrink_keeps_quadrant_averages() {
    let raw = raw_image(4, 4, 16, quadrants);
    let img = expect_img(shrink(&raw, 4, 4, 16, 2, false));
    assert_eq!(img.dimensions(), (2, 2));
    assert_eq!(img.get_pixel(0, 0).0, RED);
    assert_eq!(img.get_pixel(1, 0).0, GREEN);
    assert_eq!(img.get_pixel(0, 1).0, BLUE);
    assert_eq!(img.get_pixel(1, 1).0, GRAY);
}

#[test]
fn shrink_opaque_forces_alpha() {
    let raw = raw_image(4, 4, 16, |_, _| [10, 20, 30, 7]);
    let img = expect_img(shrink(&raw, 4, 4, 16, 2, true));
    assert!(img.pixels().all(|p| p.0 == [10, 20, 30, 255]));
    let copy = expect_img(shrink(&raw, 4, 4, 16, 8, true));
    assert!(copy.pixels().all(|p| p.0 == [10, 20, 30, 255]));
}

#[test]
fn shrink_keeps_alpha_when_not_opaque() {
    let raw = raw_image(4, 4, 16, |_, _| [10, 20, 30, 7]);
    let img = expect_img(shrink(&raw, 4, 4, 16, 2, false));
    assert!(img.pixels().all(|p| p.0 == [10, 20, 30, 7]));
}

#[test]
fn shrink_ignores_stride_padding() {
    let raw = raw_image(4, 4, 24, quadrants);
    let img = expect_img(shrink(&raw, 4, 4, 24, 2, false));
    assert_eq!(img.get_pixel(0, 0).0, RED);
    assert_eq!(img.get_pixel(1, 0).0, GREEN);
    assert_eq!(img.get_pixel(0, 1).0, BLUE);
    assert_eq!(img.get_pixel(1, 1).0, GRAY);
}

#[test]
fn shrink_small_picture_is_exact_copy_with_stride() {
    let paint = |x: u32, y: u32| [x as u8, y as u8, (x + y) as u8, 255];
    let raw = raw_image(3, 2, 20, paint);
    let img = expect_img(shrink(&raw, 3, 2, 20, 8, false));
    assert_eq!(img.dimensions(), (3, 2));
    for y in 0..2 {
        for x in 0..3 {
            assert_eq!(img.get_pixel(x, y).0, paint(x, y));
        }
    }
}

#[test]
fn shrink_rejects_bad_input_without_panic() {
    let raw = raw_image(4, 4, 16, quadrants);
    assert!(shrink(&raw[..raw.len() - 1], 4, 4, 16, 2, false).is_err());
    assert!(shrink(&raw, 4, 4, 15, 2, false).is_err());
    assert!(shrink(&raw, 0, 4, 16, 2, false).is_err());
    assert!(shrink(&raw, 4, 0, 16, 2, false).is_err());
    assert!(shrink(&raw, 4, 4, 16, 0, false).is_err());
    assert!(shrink(&[], 4, 4, 16, 2, false).is_err());
    assert!(shrink(&raw, u32::MAX, u32::MAX, u32::MAX, 2, false).is_err());
}

#[test]
fn shrink_short_last_row_padding_is_fine() {
    // The last row needs only w*4 bytes, not a whole stride.
    let raw = raw_image(4, 4, 24, quadrants);
    let trimmed = &raw[..raw.len() - 8];
    assert!(shrink(trimmed, 4, 4, 24, 2, false).is_ok());
}

#[test]
fn shrink_odd_big_sizes_respect_max() {
    for (w, h) in [(801, 3), (3, 801), (1601, 5), (5, 4001)] {
        let raw = vec![90; (w * h * 4) as usize];
        let img = expect_img(shrink(&raw, w, h, w * 4, 400, false));
        let (iw, ih) = img.dimensions();
        assert!(iw >= 1 && ih >= 1, "{w}x{h} gave {iw}x{ih}");
        assert!(iw.max(ih) <= 400, "{w}x{h} gave {iw}x{ih}");
        assert!(img.pixels().all(|p| p.0 == [90; 4]));
    }
}

#[test]
fn pick_format_prefers_abgr_then_xbgr() {
    let with = |list: Vec<wl_shm::Format>| Formats {
        buffer_size: (4, 4),
        shm_formats: list,
        ..Formats::default()
    };
    let both = with(vec![wl_shm::Format::Xbgr8888, wl_shm::Format::Abgr8888]);
    assert_eq!(pick_format(&both), Some((wl_shm::Format::Abgr8888, false)));
    let x_only = with(vec![wl_shm::Format::Argb8888, wl_shm::Format::Xbgr8888]);
    assert_eq!(pick_format(&x_only), Some((wl_shm::Format::Xbgr8888, true)));
    assert_eq!(pick_format(&with(vec![wl_shm::Format::Argb8888])), None);
    assert_eq!(pick_format(&with(Vec::new())), None);
}

#[test]
fn wait_formats_fails_at_once_when_stopped() {
    let session = Session::default();
    session.update(|i| i.stopped = true);
    let start = Instant::now();
    assert!(session.wait_formats(Duration::from_secs(10)).is_err());
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn wait_result_fails_at_once_when_stopped() {
    let session = Session::default();
    session.update(|i| i.stopped = true);
    let start = Instant::now();
    assert!(session.wait_result(Duration::from_secs(10)).is_err());
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn waits_fail_on_timeout() {
    let session = Session::default();
    assert!(session.wait_formats(Duration::from_millis(50)).is_err());
    assert!(session.wait_result(Duration::from_millis(50)).is_err());
}

#[test]
fn wait_formats_succeeds_when_another_thread_answers() {
    let session = Arc::new(Session::default());
    let other = session.clone();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(30));
        other.update(|i| {
            i.formats = Some(Formats {
                buffer_size: (7, 9),
                ..Formats::default()
            });
        });
    });
    let got = session.wait_formats(Duration::from_secs(5));
    let _ = worker.join();
    match got {
        Ok(formats) => assert_eq!(formats.buffer_size, (7, 9)),
        Err(err) => panic!("expected formats, got {err}"),
    }
}

#[test]
fn wait_result_reports_both_outcomes() {
    let ok = Session::default();
    ok.update(|i| i.res = Some(Ok(())));
    assert_eq!(ok.wait_result(Duration::from_secs(5)), Ok(()));
    let bad = Session::default();
    bad.update(|i| i.res = Some(Err("boom".to_string())));
    assert_eq!(
        bad.wait_result(Duration::from_secs(5)),
        Err("boom".to_string())
    );
}

#[test]
fn can_capture_needs_all_three() {
    for bits in 0..8u8 {
        let (a, b, c) = (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
        assert_eq!(can_capture(a, b, c), bits == 7, "{a} {b} {c}");
    }
}

#[test]
fn buffer_len_accepts_normal_sizes() {
    assert_eq!(
        buffer_len(3440, 1440),
        Some((3440 * 1440 * 4, 3440 * 1440 * 4))
    );
    assert_eq!(buffer_len(1, 1), Some((4, 4)));
}

#[test]
fn buffer_len_rejects_empty_huge_and_overflow() {
    assert_eq!(buffer_len(0, 10), None);
    assert_eq!(buffer_len(10, 0), None);
    assert_eq!(buffer_len(4097, 4097), None);
    assert_eq!(buffer_len(u32::MAX, u32::MAX), None);
    assert_eq!(buffer_len(u32::MAX, 1), None);
}

type Out = Vec<(String, Option<u32>)>;

fn job(id: &str, handle: Option<u32>) -> Job<u32> {
    Job {
        id: id.to_string(),
        handle,
    }
}

/// Queues `jobs`, closes the channel and runs the loop to the end.
fn run_loop(
    jobs: Vec<Job<u32>>,
    gate: impl Fn() -> bool,
    work: impl FnMut(&u32) -> Option<u32>,
) -> Out {
    let (tx, rx) = std::sync::mpsc::channel();
    for j in jobs {
        let _ = tx.send(j);
    }
    drop(tx);
    let out = std::cell::RefCell::new(Vec::new());
    worker_loop(rx, gate, work, |id, res| out.borrow_mut().push((id, res)));
    out.into_inner()
}

#[test]
fn worker_answers_each_request_once_in_order() {
    let jobs = (0..5).map(|n| job(&format!("w{n}"), Some(n))).collect();
    let out = run_loop(jobs, || true, |h| Some(h * 10));
    let want: Out = (0..5).map(|n| (format!("w{n}"), Some(n * 10))).collect();
    assert_eq!(out, want);
}

#[test]
fn worker_answers_none_without_handle() {
    let calls = AtomicUsize::new(0);
    let out = run_loop(
        vec![job("a", None), job("b", Some(1))],
        || true,
        |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            Some(7)
        },
    );
    assert_eq!(
        out,
        vec![("a".to_string(), None), ("b".to_string(), Some(7))]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn worker_closed_gate_skips_job_but_answers() {
    let calls = AtomicUsize::new(0);
    let jobs = (0..3).map(|n| job(&format!("g{n}"), Some(n))).collect();
    let out = run_loop(
        jobs,
        || false,
        |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            Some(1)
        },
    );
    let want: Out = (0..3).map(|n| (format!("g{n}"), None)).collect();
    assert_eq!(out, want);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn worker_gate_is_checked_before_every_job() {
    let left = AtomicUsize::new(2);
    let gate = || {
        left.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok()
    };
    let jobs = (0..4).map(|n| job(&format!("k{n}"), Some(n))).collect();
    let out = run_loop(jobs, gate, |h| Some(*h));
    let got: Vec<Option<u32>> = out.into_iter().map(|(_, r)| r).collect();
    assert_eq!(got, vec![Some(0), Some(1), None, None]);
}

#[test]
fn worker_job_failure_answers_none() {
    let out = run_loop(vec![job("x", Some(1))], || true, |_| None);
    assert_eq!(out, vec![("x".to_string(), None)]);
}

#[test]
fn submit_after_worker_ended_answers_none() {
    let (tx, rx) = std::sync::mpsc::channel::<Job<u32>>();
    drop(rx);
    let out = std::cell::RefCell::new(Vec::new());
    let done = |id: String, res: Option<u32>| out.borrow_mut().push((id, res));
    submit(Some(&tx), &done, "late".to_string(), Some(3));
    submit(None, &done, "never".to_string(), Some(3));
    assert_eq!(
        out.into_inner(),
        vec![("late".to_string(), None), ("never".to_string(), None)]
    );
}

#[test]
fn worker_runs_one_job_at_a_time() {
    let (tx, rx) = std::sync::mpsc::channel();
    let producers: Vec<_> = (0..4u32)
        .map(|p| {
            let tx = tx.clone();
            std::thread::spawn(move || {
                for n in 0..5 {
                    let _ = tx.send(job(&format!("{p}-{n}"), Some(n)));
                }
            })
        })
        .collect();
    drop(tx);
    for p in producers {
        let _ = p.join();
    }
    let (now, max, answers) = (
        AtomicUsize::new(0),
        AtomicUsize::new(0),
        AtomicUsize::new(0),
    );
    let work = |h: &u32| {
        let cur = now.fetch_add(1, Ordering::SeqCst) + 1;
        max.fetch_max(cur, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(1));
        now.fetch_sub(1, Ordering::SeqCst);
        Some(*h)
    };
    worker_loop(
        rx,
        || true,
        work,
        |_, _| {
            answers.fetch_add(1, Ordering::SeqCst);
        },
    );
    assert_eq!(max.load(Ordering::SeqCst), 1);
    assert_eq!(answers.load(Ordering::SeqCst), 20);
}

#[allow(dead_code)]
struct Fake {
    capture: CaptureState,
}

delegate_capture!(Fake);

fn assert_handler<T: ScreencopyHandler>() {}

fn assert_host<T: CaptureHost>() {}

#[test]
fn macro_implements_handlers() {
    assert_handler::<Fake>();
    assert_host::<Fake>();
}
