use super::*;

/// TOKIO_WORKER_THREADS as the programs started from here should have it:
/// as it came, not the 1 this one runs with.
fn workers() -> (String, String) {
    let value = crate::WORKERS
        .get()
        .cloned()
        .flatten()
        .map(|v| v.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(1, |n| n.get()).to_string()
        });
    ("TOKIO_WORKER_THREADS".to_owned(), value)
}

/// Start a program and leave it be; reaped by a thread, not left a zombie.
pub(super) fn spawn(program: &str, args: &[&str]) {
    let mut command = std::process::Command::new(program);
    command.args(args);
    restore_workers(&mut command);
    crate::backend::restore_backend(&mut command);
    match command.spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
        }
        Err(err) => tracing::error!("cannot run {program}: {err}"),
    }
}

fn restore_workers(command: &mut std::process::Command) {
    match crate::WORKERS.get().cloned().flatten() {
        Some(value) => command.env("TOKIO_WORKER_THREADS", value),
        None => command.env_remove("TOKIO_WORKER_THREADS"),
    };
}

/// This program, where it is now: after a reinstall the running one's file
/// is gone, and Linux names it "… (deleted)"; the new one is at that path.
pub fn self_exe() -> PathBuf {
    std::env::current_exe()
        .map(|exe| {
            let path = exe.to_string_lossy();
            PathBuf::from(path.strip_suffix(" (deleted)").unwrap_or(&path))
        })
        .unwrap_or_else(|_| PathBuf::from("cosmic-ext-applet-start"))
}

/// Start a program apart from the panel: not on the applet's own Wayland
/// socket, which the panel passes down (WAYLAND_SOCKET), but on the
/// compositor's, in its own session, to outlive the applet.
pub fn spawn_clean(program: &str, args: &[&str]) {
    let mut command = std::process::Command::new(program);
    command.args(args);
    // Itself (the menu) runs with one thread; other programs as they were.
    if program != self_exe().to_string_lossy() {
        restore_workers(&mut command);
    }
    // The button's (or the menu's) renderer choice is its own alone.
    crate::backend::restore_backend(&mut command);
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if name == "WAYLAND_SOCKET" || name == "X_PRIVILEGED_WAYLAND_SOCKET" || name.starts_with("COSMIC_PANEL_") {
            command.env_remove(&key);
        }
    }
    unsafe {
        use std::os::unix::process::CommandExt;
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    match command.spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
        }
        Err(err) => tracing::error!("cannot run {program}: {err}"),
    }
}

impl StartMenu {
    pub(super) fn launch(&mut self, index: usize, exec: Option<String>) -> app::Task<Message> {
        let app = &self.apps[index];
        let Some(exec) = exec.or_else(|| app.exec.clone()) else {
            return self.hide();
        };
        let id = app.id.clone();
        let terminal = app.terminal;
        let (workers_key, workers_value) = workers();
        let original = crate::BACKEND.get().cloned().flatten();
        let backend_value = crate::backend::child_backend_env(original.as_deref());
        self.launched.retain(|(launched, _)| *launched != id);
        self.launched.insert(0, (id.clone(), now()));
        self.launched.truncate(12);
        if let Some(Err(err)) = config().map(|c| c.set("launched", self.launched.clone())) {
            tracing::warn!("cannot save the apps used: {err}");
        }
        let launch = Task::perform(
            async move {
                cosmic::desktop::spawn_desktop_exec(
                    exec,
                    [
                        (workers_key, std::ffi::OsString::from(workers_value)),
                        ("ICED_BACKEND".to_owned(), backend_value),
                    ],
                    Some(&id),
                    terminal,
                )
                .await;
            },
            |()| cosmic::action::app(Message::Nothing),
        );
        Task::batch([self.hide(), launch])
    }

    pub(super) fn activate(&mut self, id: String) -> app::Task<Message> {
        if let Some(tx) = &self.windows_tx {
            _ = tx.send(Command::Activate(id));
        }
        self.hide()
    }
}
