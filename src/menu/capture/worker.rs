// Copyright 2023 System76 <info@system76.com>, 2026 Aenes0
// SPDX-License-Identifier: GPL-3.0-only

//! One serial capture worker; it reuses a single memfd for every window.

use std::os::fd::{AsFd, OwnedFd};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use cosmic::cctk::screencopy::{
    CaptureOptions, CaptureSession, CaptureSource, ScreencopyFrameData, ScreencopySessionData,
};
use cosmic::cctk::wayland_client::protocol::wl_shm;
use cosmic::cctk::wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1;

use super::shrink::{pick_format, shrink};
use super::{CaptureData, CaptureHost, FrameData, Session, SessionData, TIMEOUT};

/// Largest capture buffer accepted, in bytes.
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// Bytes and pool size (i32 for the wire) of a `w`x`h` ARGB buffer;
/// `None` when empty, too big or overflowing.
pub fn buffer_len(w: u32, h: u32) -> Option<(u64, i32)> {
    let len = u64::from(w)
        .checked_mul(u64::from(h))
        .and_then(|px| px.checked_mul(4))?;
    if len == 0 || len > MAX_BYTES {
        return None;
    }
    Some((len, i32::try_from(len).ok()?))
}

/// One capture request; `handle` is `None` for a window without handle.
pub struct Job<H> {
    pub id: String,
    pub handle: Option<H>,
}

/// Receives the id and the thumbnail (or `None`) of each request.
pub type Done = Arc<dyn Fn(String, Option<image::RgbaImage>) + Send + Sync>;

/// Handle to the capture thread.
pub struct CaptureWorker {
    tx: Option<Sender<Job<ExtForeignToplevelHandleV1>>>,
    done: Done,
}

impl CaptureWorker {
    /// Queues a capture; `done` runs exactly once for this `id`.
    pub fn request(&self, id: String, handle: Option<ExtForeignToplevelHandleV1>) {
        submit(self.tx.as_ref(), &*self.done, id, handle);
    }
}

/// Sends the job; when nobody can take it, answers `None` at once.
pub fn submit<H, R>(
    tx: Option<&Sender<Job<H>>>,
    done: &dyn Fn(String, Option<R>),
    id: String,
    handle: Option<H>,
) {
    match tx {
        Some(tx) => {
            if let Err(err) = tx.send(Job { id, handle }) {
                done(err.0.id, None);
            }
        }
        None => done(id, None),
    }
}

/// Serves jobs in order until the channel closes; each gets one answer.
pub fn worker_loop<H, R>(
    rx: Receiver<Job<H>>,
    gate: impl Fn() -> bool,
    mut work: impl FnMut(&H) -> Option<R>,
    done: impl Fn(String, Option<R>),
) {
    for Job { id, handle } in rx {
        let res = match handle {
            Some(h) if gate() => work(&h),
            _ => None,
        };
        done(id, res);
    }
}

/// Starts the capture thread; the memfd is made once, on that thread.
pub fn start_worker<D: CaptureHost>(
    data: CaptureData<D>,
    max: u32,
    gate: impl Fn() -> bool + Send + 'static,
    done: impl Fn(String, Option<image::RgbaImage>) + Send + Sync + 'static,
) -> CaptureWorker {
    let done: Done = Arc::new(done);
    let (tx, rx) = channel();
    let thread_done = done.clone();
    let spawned = std::thread::Builder::new()
        .name("start-menu-capture".into())
        .spawn(move || {
            let fd =
                rustix::fs::memfd_create(c"start-menu-screencopy", rustix::fs::MemfdFlags::CLOEXEC)
                    .map_err(|e| tracing::warn!("no memfd for window thumbnails: {e}"))
                    .ok();
            let work = |h: &ExtForeignToplevelHandleV1| capture(&data, fd.as_ref(), h, max);
            worker_loop(rx, gate, work, |id, res| thread_done(id, res));
        });
    let tx = match spawned {
        Ok(_) => Some(tx),
        Err(err) => {
            tracing::warn!("cannot start the capture thread: {err}");
            None
        }
    };
    CaptureWorker { tx, done }
}

/// Captures one window as a thumbnail of at most `max` pixels.
fn capture<D: CaptureHost>(
    data: &CaptureData<D>,
    fd: Option<&OwnedFd>,
    handle: &ExtForeignToplevelHandleV1,
    max: u32,
) -> Option<image::RgbaImage> {
    let res = match fd {
        Some(fd) => capture_with(data, fd, handle, max, TIMEOUT),
        None => Err("no memfd".to_string()),
    };
    if let Some(fd) = fd {
        // Gives the memory back until the next window.
        if let Err(e) = rustix::fs::ftruncate(fd, 0) {
            tracing::debug!("cannot shrink the capture memfd: {e}");
        }
    }
    match res {
        Ok(img) => Some(img),
        Err(why) => {
            tracing::debug!("no thumbnail of a window: {why}");
            None
        }
    }
}

fn capture_with<D: CaptureHost>(
    data: &CaptureData<D>,
    fd: &OwnedFd,
    handle: &ExtForeignToplevelHandleV1,
    max: u32,
    timeout: Duration,
) -> Result<image::RgbaImage, String> {
    let session = Arc::new(Session::default());
    let source = CaptureSource::Toplevel(handle.clone());
    let udata = SessionData {
        session: session.clone(),
        session_data: ScreencopySessionData::default(),
    };
    let capture_session = data
        .capturer
        .create_session(&source, CaptureOptions::empty(), &data.qh, udata)
        .map_err(|_| "cannot capture this window".to_string())?;
    data.conn.flush().map_err(|e| format!("flush: {e}"))?;
    let formats = session.wait_formats(timeout)?;
    let (w, h) = formats.buffer_size;
    let (format, opaque) = pick_format(&formats)
        .ok_or_else(|| format!("no usable format in {:?}", formats.shm_formats))?;
    let (len, pool_len) = buffer_len(w, h).ok_or_else(|| format!("bad window size {w}x{h}"))?;
    // The fd must be big enough before a pool of that size exists.
    rustix::fs::ftruncate(fd, len).map_err(|e| format!("ftruncate: {e}"))?;
    let size = (
        i32::try_from(w).map_err(|_| "window too wide".to_string())?,
        i32::try_from(h).map_err(|_| "window too tall".to_string())?,
    );
    let buf = Buf {
        fd,
        size,
        pool_len,
        format,
    };
    shoot(data, &capture_session, &session, &buf, timeout)?;
    let map_len = usize::try_from(len).map_err(|_| "window too big".to_string())?;
    // SAFETY: the memfd is private to this worker and only this thread maps it;
    // the compositor wrote to it and finished (the frame is ready) before this
    // read, and the map covers only bytes the fd was truncated to.
    let mmap = unsafe { memmap2::MmapOptions::new().len(map_len).map(&fd.as_fd()) }
        .map_err(|e| format!("mmap: {e}"))?;
    shrink(&mmap, w, h, w * 4, max, opaque).map_err(|e| format!("shrink: {e:?}"))
}

/// Shared memory buffer for one frame.
struct Buf<'a> {
    fd: &'a OwnedFd,
    size: (i32, i32),
    pool_len: i32,
    format: wl_shm::Format,
}

/// Asks for one frame and waits for it; the buffer and pool are always destroyed.
fn shoot<D: CaptureHost>(
    data: &CaptureData<D>,
    capture_session: &CaptureSession,
    session: &Arc<Session>,
    buf: &Buf<'_>,
    timeout: Duration,
) -> Result<(), String> {
    let (w, h) = buf.size;
    let pool = data
        .wl_shm
        .create_pool(buf.fd.as_fd(), buf.pool_len, &data.qh, ());
    let buffer = pool.create_buffer(0, w, h, w * 4, buf.format, &data.qh, ());
    let frame_data = FrameData {
        frame_data: ScreencopyFrameData::default(),
        session: session.clone(),
    };
    capture_session.capture(&buffer, &[], &data.qh, frame_data);
    let res = data.conn.flush().map_err(|e| format!("flush: {e}"));
    let res = res.and_then(|()| session.wait_result(timeout));
    buffer.destroy();
    pool.destroy();
    res
}
