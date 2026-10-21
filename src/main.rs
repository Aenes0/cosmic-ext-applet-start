// SPDX-License-Identifier: GPL-3.0-only

mod menu;
mod texts;
mod tools;

pub const APP_ID: &str = "io.github.Aenes0.CosmicStart";

/// TOKIO_WORKER_THREADS as it came, for the programs started from here.
pub static WORKERS: std::sync::OnceLock<Option<std::ffi::OsString>> = std::sync::OnceLock::new();
/// ICED_BACKEND as it came, for the programs started from here.
pub static BACKEND: std::sync::OnceLock<Option<std::ffi::OsString>> = std::sync::OnceLock::new();

fn main() -> cosmic::iced::Result {
    // The D-Bus library starts a runtime with a thread per core (32 here),
    // for the little it does: one is plenty. Set before any thread starts.
    WORKERS.get_or_init(|| std::env::var_os("TOKIO_WORKER_THREADS"));
    // SAFETY: nothing else runs yet.
    unsafe { std::env::set_var("TOKIO_WORKER_THREADS", "1") };
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        // A write that fails is dropped: told on stderr, if that is what
        // failed, it would abort the program.
        .log_internal_errors(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,cosmic_ext_applet_start=info".into()),
        )
        .init();
    // The menu, which stays running, hidden between uses: a second run
    // only asks the first to show or hide.
    // Win+X's "Desktop", by itself.
    if std::env::args().any(|arg| arg == "--show-desktop") {
        menu::show_desktop();
        return Ok(());
    }
    let tools = std::env::args().any(|arg| arg == "--tools");
    if tools || std::env::args().any(|arg| arg == "--menu") {
        let running = if tools { menu::tools_running() } else { menu::toggle_running() };
        if running {
            return Ok(());
        }
        own_output();
        menu::migrate();
        let settings = cosmic::app::Settings::default()
            .no_main_window(true)
            .exit_on_close(false)
            .transparent(true);
        return cosmic::app::run::<menu::StartMenu>(settings, tools);
    }
    // Else the button, as the panel runs it. It draws on the CPU: the GPU
    // would cost it memory and the driver's wakeups, for a button. The
    // menu, which scrolls long lists, draws on the GPU.
    BACKEND.get_or_init(|| std::env::var_os("ICED_BACKEND"));
    // SAFETY: nothing else runs yet.
    unsafe { std::env::set_var("ICED_BACKEND", "tiny-skia") };
    cosmic::applet::run::<menu::StartButton>(())
}

/// The menu outlives what started it (the panel's button, a shortcut, a
/// terminal), and so the pipe its output went to: once that closed, the
/// next warning written aborted it, and the next Super started it over,
/// slowly. Its own line to the journal instead, as systemd-cat; or nowhere.
fn own_output() {
    use std::{
        io::Write,
        os::{fd::AsRawFd, unix::net::UnixStream},
    };
    let journal = UnixStream::connect("/run/systemd/journal/stdout").and_then(|mut stream| {
        stream.shutdown(std::net::Shutdown::Read)?;
        // Identifier, unit, priority (info), level prefix, and no copies
        // to syslog, the kernel log or the console.
        stream.write_all(b"cosmic-ext-applet-start\n\n6\n0\n0\n0\n0\n")?;
        Ok(std::os::fd::OwnedFd::from(stream))
    });
    let target = match journal {
        Ok(fd) => fd,
        Err(_) => match std::fs::OpenOptions::new().write(true).open("/dev/null") {
            Ok(file) => file.into(),
            Err(_) => return,
        },
    };
    // SAFETY: duplicates an open descriptor over stdout and stderr.
    unsafe {
        libc::dup2(target.as_raw_fd(), 1);
        libc::dup2(target.as_raw_fd(), 2);
    }
}
