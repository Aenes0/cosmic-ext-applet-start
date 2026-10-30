// SPDX-License-Identifier: GPL-3.0-only

mod anim;
mod backend;
mod menu;
mod texts;
mod uninstall;
mod tools;

pub const APP_ID: &str = "io.github.Aenes0.CosmicStart";

/// TOKIO_WORKER_THREADS as it came, for the programs started from here.
pub static WORKERS: std::sync::OnceLock<Option<std::ffi::OsString>> = std::sync::OnceLock::new();
/// ICED_BACKEND as it came, for the programs started from here.
pub static BACKEND: std::sync::OnceLock<Option<std::ffi::OsString>> = std::sync::OnceLock::new();

/// Log filter when RUST_LOG is unset. usvg's SVG warnings filled the
/// journal (~7 100 lines), so only its errors pass.
const DEFAULT_LOG: &str = "warn,cosmic_ext_applet_start=info,usvg=error";

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
                .unwrap_or_else(|_| DEFAULT_LOG.into()),
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
        // Before anything else: it may set ICED_BACKEND.
        backend::init_menu();
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
    backend::init_button();
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

#[cfg(test)]
mod tests {
    use super::DEFAULT_LOG;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tracing_subscriber::{EnvFilter, layer::SubscriberExt};

    struct Counter(Arc<AtomicUsize>);

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Counter {
        fn on_event(
            &self,
            _event: &tracing::Event<'_>,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn the_default_filter_parses_and_quiets_usvg() {
        assert!(DEFAULT_LOG.contains("usvg=error"));
        assert!(EnvFilter::try_new(DEFAULT_LOG).is_ok());
    }

    #[test]
    fn usvg_warnings_are_dropped_by_default() {
        let count = Arc::new(AtomicUsize::new(0));
        let subscriber = tracing_subscriber::registry()
            .with(EnvFilter::new(DEFAULT_LOG))
            .with(Counter(count.clone()));
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(target: "usvg::parser::svgtree", "x");
            tracing::warn!(target: "cosmic_ext_applet_start::menu", "y");
            tracing::error!(target: "usvg::parser::svgtree", "z");
        });
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }
}
