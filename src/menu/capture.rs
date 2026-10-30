// Copyright 2023 System76 <info@system76.com>, 2026 Aenes0
// SPDX-License-Identifier: GPL-3.0-only

//! Thumbnail of one window through ext-image-copy-capture-v1.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use cosmic::cctk::wayland_client::protocol::{
    wl_buffer::WlBuffer, wl_shm::WlShm, wl_shm_pool::WlShmPool,
};
use cosmic::cctk::wayland_client::{Connection, Dispatch, QueueHandle, WEnum, globals::GlobalList};
use cosmic::cctk::wayland_protocols::ext::{
    image_capture_source::v1::client as source, image_copy_capture::v1::client as copy,
};
use cosmic::cctk::{
    GlobalData,
    cosmic_protocols::image_capture_source::v1::client as cosmic_source,
    screencopy::{
        CaptureFrame, CaptureSession, Capturer, FailureReason, Formats, ScreencopyFrameData,
        ScreencopyFrameDataExt, ScreencopySessionData, ScreencopySessionDataExt, ScreencopyState,
    },
};

mod shrink;
mod worker;

#[cfg(test)]
pub use shrink::{ShrinkError, pick_format, shrink};
pub use worker::{CaptureWorker, start_worker};
#[cfg(test)]
pub use worker::{Job, buffer_len, submit, worker_loop};

/// How long one wait on the compositor may last before the capture is dropped.
pub const TIMEOUT: Duration = Duration::from_secs(2);

const COPY_MANAGER: &str = "ext_image_copy_capture_manager_v1";
const TOPLEVEL_SOURCE_MANAGER: &str = "ext_foreign_toplevel_image_capture_source_manager_v1";

#[derive(Default)]
pub struct SessionInner {
    pub formats: Option<Formats>,
    pub res: Option<Result<(), String>>,
    pub stopped: bool,
}

/// State shared between the compositor callbacks and the capture worker.
#[derive(Default)]
pub struct Session {
    condvar: Condvar,
    inner: Mutex<SessionInner>,
}

impl Session {
    fn lock(&self) -> MutexGuard<'_, SessionInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Changes the shared state and wakes the waiting thread.
    pub fn update<F: FnOnce(&mut SessionInner)>(&self, f: F) {
        f(&mut self.lock());
        self.condvar.notify_all();
    }

    /// Waits for the buffer formats; fails when stopped or after `timeout`.
    pub fn wait_formats(&self, timeout: Duration) -> Result<Formats, String> {
        self.wait(timeout, |i| i.formats.is_some(), |i| i.formats.take())
    }

    /// Waits for the copy result; fails when stopped or after `timeout`.
    pub fn wait_result(&self, timeout: Duration) -> Result<(), String> {
        self.wait(timeout, |i| i.res.is_some(), |i| i.res.take())
            .and_then(|res| res)
    }

    fn wait<T>(
        &self,
        timeout: Duration,
        ready: impl Fn(&SessionInner) -> bool,
        take: impl FnOnce(&mut SessionInner) -> Option<T>,
    ) -> Result<T, String> {
        let (mut inner, _) = self
            .condvar
            .wait_timeout_while(self.lock(), timeout, |i| !i.stopped && !ready(i))
            .unwrap_or_else(|e| e.into_inner());
        let stopped = inner.stopped;
        take(&mut inner).ok_or_else(|| {
            let why = if stopped {
                "capture session stopped"
            } else {
                "timed out"
            };
            why.to_string()
        })
    }
}

#[derive(Default)]
pub struct SessionData {
    pub session: Arc<Session>,
    pub session_data: ScreencopySessionData,
}

pub struct FrameData {
    pub frame_data: ScreencopyFrameData,
    pub session: Arc<Session>,
}

impl ScreencopySessionDataExt for SessionData {
    fn screencopy_session_data(&self) -> &ScreencopySessionData {
        &self.session_data
    }
}

impl ScreencopyFrameDataExt for FrameData {
    fn screencopy_frame_data(&self) -> &ScreencopyFrameData {
        &self.frame_data
    }
}

/// Callback body for `ScreencopyHandler::init_done`.
pub fn on_init_done(session: &CaptureSession, formats: &Formats) {
    if let Some(data) = session.data::<SessionData>() {
        data.session.update(|i| i.formats = Some(formats.clone()));
    }
}

/// Callback body for `ScreencopyHandler::stopped`.
pub fn on_stopped(session: &CaptureSession) {
    if let Some(data) = session.data::<SessionData>() {
        data.session.update(|i| i.stopped = true);
    }
}

/// Callback body for `ScreencopyHandler::ready`.
pub fn on_ready(frame: &CaptureFrame) {
    if let Some(data) = frame.data::<FrameData>() {
        data.session.update(|i| i.res = Some(Ok(())));
    }
}

/// Callback body for `ScreencopyHandler::failed`.
pub fn on_failed(frame: &CaptureFrame, reason: WEnum<FailureReason>) {
    if let Some(data) = frame.data::<FrameData>() {
        data.session
            .update(|i| i.res = Some(Err(format!("capture failed: {reason:?}"))));
    }
}

macro_rules! capture_host {
    ($($iface:ty: $udata:ty),* $(,)?) => {
        /// Everything the Wayland handlers of the host state need.
        pub trait CaptureHost: 'static $(+ Dispatch<$iface, $udata>)* {}
        impl<T: 'static $(+ Dispatch<$iface, $udata>)*> CaptureHost for T {}
    };
}

capture_host!(
    WlShm: (),
    WlShmPool: (),
    WlBuffer: (),
    copy::ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1: GlobalData,
    source::ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1: GlobalData,
    source::ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1: GlobalData,
    cosmic_source::zcosmic_workspace_image_capture_source_manager_v1::ZcosmicWorkspaceImageCaptureSourceManagerV1: GlobalData,
    source::ext_image_capture_source_v1::ExtImageCaptureSourceV1: GlobalData,
    copy::ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1: SessionData,
    copy::ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1: FrameData,
);

/// Capture globals of the host state; `enabled` only when the compositor
/// offers everything a capture needs.
pub struct CaptureState {
    pub screencopy: ScreencopyState,
    shm: Option<WlShm>,
    enabled: bool,
}

fn has_global(globals: &GlobalList, interface: &str) -> bool {
    globals
        .contents()
        .with_list(|list| list.iter().any(|g| g.interface == interface))
}

impl CaptureState {
    /// A state that never captures: no wl_shm bound, no warning (for the
    /// one-shot paths that only need the type).
    pub fn disabled<D: CaptureHost>(globals: &GlobalList, qh: &QueueHandle<D>) -> Self {
        Self {
            screencopy: ScreencopyState::new(globals, qh),
            shm: None,
            enabled: false,
        }
    }

    pub fn new<D: CaptureHost>(globals: &GlobalList, qh: &QueueHandle<D>) -> Self {
        let screencopy = ScreencopyState::new(globals, qh);
        // Bound directly: the handlers need no shm formats from the registry.
        let shm = globals.bind::<WlShm, _, _>(qh, 1..=1, ()).ok();
        let enabled = can_capture(
            has_global(globals, COPY_MANAGER),
            has_global(globals, TOPLEVEL_SOURCE_MANAGER),
            shm.is_some(),
        );
        if !enabled {
            tracing::warn!(
                "window thumbnails disabled: compositor lacks image copy capture or wl_shm"
            );
        }
        Self {
            screencopy,
            shm,
            enabled,
        }
    }

    /// Handle for the capture worker, `None` when captures are unavailable.
    pub fn data<D: CaptureHost>(
        &self,
        conn: &Connection,
        qh: &QueueHandle<D>,
    ) -> Option<CaptureData<D>> {
        if !self.enabled {
            return None;
        }
        Some(CaptureData {
            qh: qh.clone(),
            conn: conn.clone(),
            wl_shm: self.shm.clone()?,
            capturer: self.screencopy.capturer().clone(),
        })
    }
}

/// Captures run only with both ext managers (else `create_session` would
/// abort the process) and a bound wl_shm.
pub fn can_capture(copy_manager: bool, toplevel_source: bool, shm: bool) -> bool {
    copy_manager && toplevel_source && shm
}

/// Everything the capture worker needs; `Send`.
pub struct CaptureData<D> {
    pub qh: QueueHandle<D>,
    pub conn: Connection,
    pub wl_shm: WlShm,
    pub capturer: Capturer,
}

/// Implements the capture handlers for a state with a `capture: CaptureState` field.
#[rustfmt::skip]
macro_rules! delegate_capture {
    ($ty:ty) => {
        const _: () = {
            use cosmic::cctk::screencopy::{
                CaptureFrame, CaptureSession, FailureReason, Formats, Frame, ScreencopyHandler,
                ScreencopyState,
            };
            use cosmic::cctk::wayland_client::protocol::{wl_buffer, wl_shm, wl_shm_pool};
            use cosmic::cctk::wayland_client::{Connection, Dispatch, QueueHandle, WEnum};
            use $crate::menu::capture as cap;

            type Qh = QueueHandle<$ty>;

            impl ScreencopyHandler for $ty {
                fn screencopy_state(&mut self) -> &mut ScreencopyState {
                    &mut self.capture.screencopy
                }
                fn init_done(&mut self, _: &Connection, _: &Qh, s: &CaptureSession, f: &Formats) {
                    cap::on_init_done(s, f);
                }
                fn stopped(&mut self, _: &Connection, _: &Qh, s: &CaptureSession) {
                    cap::on_stopped(s);
                }
                fn ready(&mut self, _: &Connection, _: &Qh, f: &CaptureFrame, _: Frame) {
                    cap::on_ready(f);
                }
                fn failed(&mut self, _: &Connection, _: &Qh, f: &CaptureFrame, r: WEnum<FailureReason>) {
                    cap::on_failed(f, r);
                }
            }

            // The shm objects need no handling.
            impl Dispatch<wl_shm::WlShm, ()> for $ty {
                fn event(_: &mut Self, _: &wl_shm::WlShm, _: wl_shm::Event, _: &(), _: &Connection, _: &Qh) {}
            }
            impl Dispatch<wl_shm_pool::WlShmPool, ()> for $ty {
                fn event(_: &mut Self, _: &wl_shm_pool::WlShmPool, _: wl_shm_pool::Event, _: &(), _: &Connection, _: &Qh) {}
            }
            impl Dispatch<wl_buffer::WlBuffer, ()> for $ty {
                fn event(_: &mut Self, _: &wl_buffer::WlBuffer, _: wl_buffer::Event, _: &(), _: &Connection, _: &Qh) {}
            }
        };

        cosmic::cctk::delegate_screencopy!($ty);
    };
}

pub(crate) use delegate_capture;

#[cfg(test)]
mod tests;

