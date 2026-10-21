// SPDX-License-Identifier: GPL-3.0-only

mod menu;
mod texts;
mod tools;

pub const APP_ID: &str = "io.github.Aenes0.CosmicStart";

fn main() -> cosmic::iced::Result {
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
    // Else the button, as the panel runs it.
    cosmic::applet::run::<menu::StartButton>(())
}
