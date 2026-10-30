use super::*;

/// The plan, after reading the entry and its origin again: what the menu
/// knew may have changed since it was read.
fn plan_checked(known: &uninstall::Origin, mut facts: uninstall::DesktopFacts) -> Result<Plan, String> {
    let locales = locales();
    let de = desktop_entry::read(&facts.path, &locales)?;
    facts.exec = de.exec().map(str::to_owned);
    facts.flatpak_ref = de.flatpak().map(str::to_owned);
    facts.flatpak_part_of = de.desktop_entry("X-Flatpak-Part-Of").map(str::to_owned);
    let now = uninstall::detect(&facts, &uninstall::RealFs);
    if !now.removable() || now != *known {
        return Err(texts().app_changed.into());
    }
    uninstall::plan(&now, &facts, &uninstall::SystemRunner)
}

impl StartMenu {
    pub(super) fn uninstall_running(&self) -> bool {
        self.uninstall.as_ref().is_some_and(Dialog::running)
    }

    pub(super) fn ask_uninstall(&mut self, id: String) -> app::Task<Message> {
        if self.uninstall_running() {
            return Task::none();
        }
        let Some(app) = self.apps.iter().find(|a| a.id == id) else {
            return Task::none();
        };
        let (facts, origin) = (app.facts(), app.origin.clone());
        self.uninstall = Some(Dialog::new(id.clone()));
        self.menu = Some(Target::Uninstall(id.clone()));
        self.menu_anchor = None;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || plan_checked(&origin, facts))
                    .await
                    .unwrap_or_else(|err| Err(err.to_string()))
            },
            move |result| cosmic::action::app(Message::UninstallPlan(id.clone(), result)),
        )
    }

    pub(super) fn planned(&mut self, id: &str, result: Result<Plan, String>) {
        if let Some(dialog) = self.uninstall.as_mut().filter(|d| d.id == id)
            && let Some(phase) = uninstall_ui::advance(&dialog.phase, DialogEvent::Planned(result))
        {
            dialog.phase = phase;
        }
    }

    pub(super) fn confirm_uninstall(&mut self) -> app::Task<Message> {
        let Some(dialog) = self.uninstall.as_mut() else {
            return Task::none();
        };
        let Phase::Ready(plan) = &dialog.phase else {
            return Task::none();
        };
        let (plan, delete_data) = (plan.clone(), dialog.delete_data);
        if let Some(phase) = uninstall_ui::advance(&dialog.phase, DialogEvent::Confirm) {
            dialog.phase = phase;
        }
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || uninstall::run(&plan, delete_data, &uninstall::SystemRunner))
                    .await
                    .unwrap_or_else(|err| Err(err.to_string()))
            },
            |result| cosmic::action::app(Message::UninstallDone(result)),
        )
    }

    /// Drops the app uninstalled last from the pinned and the launched, if
    /// the apps just read no longer have it.
    pub(super) fn forget_uninstalled(&mut self) {
        let Some(id) = self.last_uninstalled.take() else {
            return;
        };
        let apps = &self.apps;
        if let Some(pinned) = uninstall_ui::prune(&self.pinned, &id, |i| apps.iter().any(|a| a.id == i)) {
            self.pinned = pinned;
            save_pinned(&self.pinned);
        }
        if !apps.iter().any(|a| a.id == id) {
            self.launched.retain(|(l, _)| *l != id);
            let (groups, gone) = self.groups.forget(&id).emptied();
            self.groups = groups;
            self.pinned = groups::unpin(&self.pinned, &gone);
            save_groups(&self.groups);
            save_pinned(&self.pinned);
        }
    }

    pub(super) fn uninstalled(&mut self, result: Result<(), String>) -> app::Task<Message> {
        let Some(dialog) = self.uninstall.take() else {
            return Task::none();
        };
        match uninstall_ui::advance(&dialog.phase, DialogEvent::Done(result)) {
            Some(phase) => {
                self.uninstall = Some(Dialog { phase, ..dialog });
                Task::none()
            }
            None => {
                self.close_menu();
                // Steam and Wine exit 0 even if the user cancels: the id
                // is only forgotten once the reread list lacks it.
                self.last_uninstalled = Some(dialog.id);
                // The folders' signature misses deep subfolders: force it.
                self.apps_signature = None;
                reload_apps(None)
            }
        }
    }
}
