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
