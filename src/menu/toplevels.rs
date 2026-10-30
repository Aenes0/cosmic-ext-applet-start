use super::*;
use std::collections::HashSet;
use std::sync::Mutex;
use std::os::{
    fd::{FromRawFd, RawFd},
    unix::net::UnixStream,
};

use cosmic::cctk::{
    self,
    cosmic_protocols::{
        toplevel_info::v1::client::zcosmic_toplevel_handle_v1,
        toplevel_management::v1::client::zcosmic_toplevel_manager_v1,
    },
    sctk::{
        self,
        reexports::{calloop, calloop_wayland_source::WaylandSource},
        registry::{ProvidesRegistryState, RegistryState},
        seat::{Capability, SeatHandler, SeatState},
    },
    toplevel_info::{ToplevelInfoHandler, ToplevelInfoState},
    toplevel_management::{ToplevelManagerHandler, ToplevelManagerState},
    wayland_client::{
        Connection, QueueHandle, WEnum, globals::registry_queue_init, protocol::wl_seat::WlSeat,
    },
    wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
};

use super::capture::{self, CaptureState, CaptureWorker, start_worker};

/// Largest side, in pixels, of a thumbnail.
const THUMB_MAX: u32 = 400;

/// Captures run only while the menu shows and its preview is open.
fn capture_gate(windows_shown: bool, preview_shown: bool) -> bool {
    windows_shown && preview_shown
}

/// The menu's message for a finished capture.
fn thumb_message(id: String, image: Option<image::RgbaImage>) -> Message {
    let handle = image.map(|img| {
        let (w, h) = img.dimensions();
        cosmic::widget::image::Handle::from_rgba(w, h, img.into_raw())
    });
    Message::Thumb(id, handle)
}

/// Keeps the thumbnail of a window; `None` forgets it.
pub(super) fn store_thumb(
    thumbs: &mut HashMap<String, cosmic::widget::image::Handle>,
    id: String,
    handle: Option<cosmic::widget::image::Handle>,
) {
    match handle {
        Some(handle) => {
            thumbs.insert(id, handle);
        }
        None => {
            thumbs.remove(&id);
        }
    }
}

/// A window of the list, from what the compositor says of it.
fn window_from(
    id: &str,
    title: &str,
    app_id: &str,
    states: &HashSet<zcosmic_toplevel_handle_v1::State>,
) -> Window {
    use zcosmic_toplevel_handle_v1::State as S;
    Window {
        id: id.to_string(),
        title: title.to_string(),
        app_id: app_id.to_string(),
        active: states.contains(&S::Activated),
        minimized: states.contains(&S::Minimized),
    }
}

pub(super) fn windows_subscription() -> Subscription<Message> {
    struct Windows;
    Subscription::run_with(std::any::TypeId::of::<Windows>(), |_| {
        stream::channel(8, |mut output: mpsc::Sender<Message>| async move {
            let (tx, rx) = calloop::channel::channel();
            _ = output.send(Message::WindowsReady(tx)).await;
            std::thread::spawn(move || {
                if let Err(err) = run_windows(output, rx) {
                    tracing::warn!("window list stopped: {err}");
                }
            });
            futures::future::pending::<()>().await;
        })
    })
}

struct State {
    registry_state: RegistryState,
    seat_state: SeatState,
    info: ToplevelInfoState,
    manager: ToplevelManagerState,
    tx: mpsc::Sender<Message>,
    capture: CaptureState,
    /// `None` when the compositor cannot capture windows.
    worker: Option<CaptureWorker>,
    /// The windows by when they opened, as the list shows them.
    order: Vec<String>,
    dirty: bool,
    /// The last list the menu was sent.
    last: Option<Vec<Window>>,
    /// The next report goes out even if the list is unchanged.
    forced: bool,
}

/// Whether the menu needs this list: it changed, or it asked anew.
fn should_send(prev: Option<&[Window]>, next: &[Window], forced: bool) -> bool {
    forced || prev != Some(next)
}

impl State {
    fn report(&mut self) {
        // Hidden, the menu needn't hear of each title that changes: it
        // sleeps, and hears all when it shows.
        if !WINDOWS_SHOWN.load(Ordering::Relaxed) || !std::mem::take(&mut self.dirty) {
            return;
        }
        let mut by_id: HashMap<String, Window> = self
            .info
            .toplevels()
            .map(|t| {
                (
                    t.identifier.clone(),
                    window_from(&t.identifier, &t.title, &t.app_id, &t.state),
                )
            })
            .collect();
        self.order.retain(|id| by_id.contains_key(id));
        let mut new: Vec<&String> = by_id.keys().filter(|id| !self.order.contains(id)).collect();
        new.sort();
        let new: Vec<String> = new.into_iter().cloned().collect();
        self.order.extend(new);
        let windows: Vec<Window> = self.order.iter().filter_map(|id| by_id.remove(id)).collect();
        let forced = std::mem::take(&mut self.forced);
        if !should_send(self.last.as_deref(), &windows, forced) {
            return;
        }
        // One clone, kept to compare with the next list.
        let sent = futures::executor::block_on(self.tx.send(Message::Windows(windows.clone())));
        if sent.is_ok() {
            self.last = Some(windows);
        }
    }

    fn handle(&self, id: &str) -> Option<zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1> {
        self.info
            .toplevels()
            .find(|t| t.identifier == id)
            .and_then(|t| t.cosmic_toplevel.clone())
    }

    fn foreign(&self, id: &str) -> Option<ExtForeignToplevelHandleV1> {
        self.info
            .toplevels()
            .find(|t| t.identifier == id)
            .map(|t| t.foreign_toplevel.clone())
    }

    fn capture(&mut self, ids: Vec<String>) {
        for id in ids {
            match &self.worker {
                Some(worker) => worker.request(id.clone(), self.foreign(&id)),
                None => {
                    _ = futures::executor::block_on(self.tx.send(Message::Thumb(id, None)));
                }
            }
        }
    }

    fn command(&mut self, command: Command) {
        match command {
            Command::Activate(id) => {
                let Some(handle) = self.handle(&id) else { return };
                // The menu lets go of the keyboard first.
                std::thread::sleep(Duration::from_millis(60));
                self.manager.manager.unset_minimized(&handle);
                if let Some(seat) = self.seat_state.seats().next() {
                    self.manager.manager.activate(&handle, &seat);
                }
            }
            Command::Close(id) => {
                if let Some(handle) = self.handle(&id) {
                    self.manager.manager.close(&handle);
                }
            }
            Command::Capture(ids) => self.capture(ids),
            Command::Refresh => {
                self.dirty = true;
                self.forced = true;
                self.report();
            }
        }
    }
}

fn run_windows(
    tx: mpsc::Sender<Message>,
    rx: calloop::channel::Channel<Command>,
) -> Result<(), Box<dyn std::error::Error>> {
    let socket = std::env::var("X_PRIVILEGED_WAYLAND_SOCKET")
        .ok()
        .and_then(|fd| fd.parse::<RawFd>().ok())
        .map(|fd| unsafe { UnixStream::from_raw_fd(fd) });
    let conn = match socket {
        Some(socket) => Connection::from_socket(socket)?,
        None => Connection::connect_to_env()?,
    };
    let (globals, event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();
    let registry_state = RegistryState::new(&globals);
    let info = ToplevelInfoState::try_new(&registry_state, &qh).ok_or("no toplevel info")?;
    let manager = ToplevelManagerState::try_new(&registry_state, &qh).ok_or("no toplevel manager")?;
    let sender = Mutex::new(tx.clone());
    let mut state = State {
        seat_state: SeatState::new(&globals, &qh),
        capture: CaptureState::new(&globals, &qh),
        worker: None,
        registry_state,
        info,
        manager,
        tx,
        order: Vec::new(),
        dirty: true,
        last: None,
        forced: false,
    };
    state.worker = state.capture.data(&conn, &qh).map(|data| {
        let done = move |id: String, image: Option<image::RgbaImage>| {
            let mut sender = sender.lock().unwrap_or_else(|e| e.into_inner());
            // The thread may wait: an answer lost leaves a card waiting.
            if let Err(err) = futures::executor::block_on(sender.send(thumb_message(id, image))) {
                tracing::debug!("thumbnail not delivered: {err}");
            }
        };
        let gate = || capture_gate(WINDOWS_SHOWN.load(Ordering::Relaxed), PREVIEW_SHOWN.load(Ordering::Relaxed));
        start_worker(data, THUMB_MAX, gate, done)
    });
    let conn_loop = conn.clone();
    let mut event_loop = calloop::EventLoop::<State>::try_new()?;
    WaylandSource::new(conn_loop, event_queue)
        .insert(event_loop.handle())
        .map_err(|err| err.error)?;
    event_loop
        .handle()
        .insert_source(rx, |event, (), state| {
            if let calloop::channel::Event::Msg(command) = event {
                state.command(command);
            }
        })
        .map_err(|err| err.error)?;
    loop {
        event_loop.dispatch(None, &mut state)?;
        state.report();
    }
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    sctk::registry_handlers![SeatState];
}

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
    fn new_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat, _: Capability) {}
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat, _: Capability) {}
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
}

impl ToplevelInfoHandler for State {
    fn toplevel_info_state(&mut self) -> &mut ToplevelInfoState {
        &mut self.info
    }
    fn new_toplevel(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &ExtForeignToplevelHandleV1) {
        self.dirty = true;
    }
    fn update_toplevel(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &ExtForeignToplevelHandleV1) {
        self.dirty = true;
    }
    fn toplevel_closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &ExtForeignToplevelHandleV1) {
        self.dirty = true;
    }
}

impl ToplevelManagerHandler for State {
    fn toplevel_manager_state(&mut self) -> &mut ToplevelManagerState {
        &mut self.manager
    }
    fn capabilities(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: Vec<WEnum<zcosmic_toplevel_manager_v1::ZcosmicToplelevelManagementCapabilitiesV1>>,
    ) {
    }
}

cctk::delegate_toplevel_info!(State);
cctk::delegate_toplevel_manager!(State);
capture::delegate_capture!(State);
sctk::delegate_seat!(State);
sctk::delegate_registry!(State);

/// Minimizes every window, Win+X's "Desktop": by itself, once, and leaves.
pub fn show_desktop() {
    let run = || -> Result<(), Box<dyn std::error::Error>> {
        let conn = Connection::connect_to_env()?;
        let (globals, mut event_queue) = registry_queue_init(&conn)?;
        let qh = event_queue.handle();
        let registry_state = RegistryState::new(&globals);
        let mut state = State {
            seat_state: SeatState::new(&globals, &qh),
            info: ToplevelInfoState::try_new(&registry_state, &qh).ok_or("no toplevel info")?,
            manager: ToplevelManagerState::try_new(&registry_state, &qh).ok_or("no toplevel manager")?,
            capture: CaptureState::disabled(&globals, &qh),
            worker: None,
            registry_state,
            tx: mpsc::channel(1).0,
            order: Vec::new(),
            dirty: false,
            last: None,
            forced: false,
        };
        // Twice: the windows, then what each is.
        event_queue.roundtrip(&mut state)?;
        event_queue.roundtrip(&mut state)?;
        for toplevel in state.info.toplevels() {
            if let Some(handle) = &toplevel.cosmic_toplevel {
                state.manager.manager.set_minimized(handle);
            }
        }
        event_queue.roundtrip(&mut state)?;
        Ok(())
    };
    if let Err(err) = run() {
        tracing::error!("cannot show the desktop: {err}");
    }
}

#[cfg(test)]
mod tests;
