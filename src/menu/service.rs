use super::*;

const BUS_NAME: &str = "io.github.Aenes0.CosmicStart";
const BUS_PATH: &str = "/io/github/Aenes0/CosmicStart";

/// Asks the running menu to show or hide; false when none runs, and this
/// one should be it.
pub fn toggle_running() -> bool {
    call_running("Toggle")
}

/// The same for the tools' menu (Win+X).
pub fn tools_running() -> bool {
    call_running("Tools")
}

fn call_running(method: &str) -> bool {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() else {
        return false;
    };
    runtime.block_on(async {
        let Ok(conn) = zbus::Connection::session().await else {
            return false;
        };
        conn.call_method(Some(BUS_NAME), BUS_PATH, Some(BUS_NAME), method, &())
            .await
            .is_ok()
    })
}

struct Service(mpsc::Sender<Message>);

#[zbus::interface(name = "io.github.Aenes0.CosmicStart")]
impl Service {
    async fn toggle(&self) {
        _ = self.0.clone().send(Message::Toggle).await;
    }

    async fn tools(&self) {
        _ = self.0.clone().send(Message::Tools).await;
    }
}

pub(super) fn dbus_service() -> Subscription<Message> {
    struct Bus;
    Subscription::run_with(std::any::TypeId::of::<Bus>(), |_| {
        stream::channel(4, |output: mpsc::Sender<Message>| async move {
            let conn = async {
                zbus::connection::Builder::session()?
                    .name(BUS_NAME)?
                    .serve_at(BUS_PATH, Service(output))?
                    .build()
                    .await
            }
            .await;
            match conn {
                // Kept for as long as the menu runs.
                Ok(conn) => {
                    let _conn = conn;
                    futures::future::pending::<()>().await;
                }
                Err(err) => tracing::error!("cannot serve the start menu on D-Bus: {err}"),
            }
        })
    })
}
