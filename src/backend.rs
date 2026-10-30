//! Which renderer the menu draws with, and keeping that choice to itself.
use cosmic::cosmic_config::{Config, ConfigGet};
use std::{
    ffi::{OsStr, OsString},
    sync::atomic::{AtomicBool, Ordering},
};

/// A renderer the menu can draw with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Wgpu,
    TinySkia,
}

/// True when the renderer chosen for the menu is tiny-skia (the CPU).
static CPU: AtomicBool = AtomicBool::new(false);

/// True when the menu draws on the CPU (tiny-skia).
pub fn is_cpu() -> bool {
    CPU.load(Ordering::Relaxed)
}

/// First non-empty candidate of a comma list, as iced reads ICED_BACKEND.
pub fn parse(value: &str) -> Option<Backend> {
    let first = value.split(',').map(str::trim).find(|v| !v.is_empty())?;
    match first {
        "wgpu" => Some(Backend::Wgpu),
        "tiny-skia" => Some(Backend::TinySkia),
        _ => None,
    }
}

/// Override (for tests) > the user's ICED_BACKEND > the config key > wgpu.
/// A value not recognized counts as absent at its level.
pub fn decide(
    override_env: Option<&str>,
    iced_env: Option<&str>,
    config: Option<&str>,
) -> Backend {
    [override_env, iced_env, config]
        .into_iter()
        .flatten()
        .find_map(parse)
        .unwrap_or(Backend::Wgpu)
}

/// What to write to ICED_BACKEND for the menu itself, if anything.
fn env_action(
    override_env: Option<&str>,
    iced_env: Option<&str>,
    decision: Backend,
) -> Option<&'static str> {
    let override_choice = override_env.and_then(parse);
    let user_choice = iced_env.and_then(parse);
    let from_user = override_choice.is_none() && user_choice.is_some();
    if decision == Backend::TinySkia && !from_user {
        return Some("tiny-skia");
    }
    let inherited_other = iced_env
        .is_some_and(|v| !v.trim().is_empty() && parse(v) != Some(Backend::Wgpu));
    (override_choice == Some(Backend::Wgpu) && inherited_other).then_some("wgpu")
}

/// The key `menu_backend` of the app's config; any failure is absent.
fn read_config() -> Option<String> {
    Config::new(crate::APP_ID, 1).ok()?.get::<String>("menu_backend").ok()
}

/// Chooses the menu's renderer. Call before any thread starts: it sets
/// ICED_BACKEND when the choice needs it.
pub fn init_menu() {
    let original = crate::BACKEND.get_or_init(|| std::env::var_os("ICED_BACKEND"));
    let override_env = std::env::var("COSMIC_START_BACKEND").ok();
    let iced_env = original.as_ref().map(|v| v.to_string_lossy().into_owned());
    let config = read_config();
    let decision = decide(override_env.as_deref(), iced_env.as_deref(), config.as_deref());
    if let Some(value) = env_action(override_env.as_deref(), iced_env.as_deref(), decision) {
        // SAFETY: nothing else runs yet.
        unsafe { std::env::set_var("ICED_BACKEND", value) };
    }
    CPU.store(decision == Backend::TinySkia, Ordering::Relaxed);
}

/// The button always draws on the CPU; remembers what ICED_BACKEND was.
pub fn init_button() {
    crate::BACKEND.get_or_init(|| std::env::var_os("ICED_BACKEND"));
    // SAFETY: nothing else runs yet.
    unsafe { std::env::set_var("ICED_BACKEND", "tiny-skia") };
}

/// ICED_BACKEND for a program started from here: as it came, or empty
/// (which iced reads as absent) so the menu's own choice never leaks.
pub fn child_backend_env(original: Option<&OsStr>) -> OsString {
    original.map(OsStr::to_os_string).unwrap_or_default()
}

fn apply_restore(command: &mut std::process::Command, backend: Option<&Option<OsString>>) {
    match backend {
        Some(Some(value)) => command.env("ICED_BACKEND", value),
        Some(None) => command.env_remove("ICED_BACKEND"),
        None => return,
    };
}

/// Gives a started program the ICED_BACKEND this one came with.
pub fn restore_backend(command: &mut std::process::Command) {
    apply_restore(command, crate::BACKEND.get());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn decide_table() {
        let tiny = Backend::TinySkia;
        let wgpu = Backend::Wgpu;
        assert_eq!(decide(None, None, None), wgpu);
        assert_eq!(decide(None, None, Some("tiny-skia")), tiny);
        assert_eq!(decide(None, None, Some("lixo")), wgpu);
        assert_eq!(decide(None, Some("wgpu"), Some("tiny-skia")), wgpu);
        assert_eq!(decide(None, Some("tiny-skia"), Some("wgpu")), tiny);
        assert_eq!(decide(Some("wgpu"), Some("tiny-skia"), Some("tiny-skia")), wgpu);
        assert_eq!(decide(Some("tiny-skia"), Some("wgpu"), None), tiny);
        assert_eq!(decide(None, Some("tiny-skia,wgpu"), None), tiny);
        assert_eq!(decide(None, Some(""), Some("tiny-skia")), tiny);
        assert_eq!(decide(Some("lixo"), None, Some("tiny-skia")), tiny);
    }

    #[test]
    fn parse_reads_the_first_candidate() {
        assert_eq!(parse("wgpu"), Some(Backend::Wgpu));
        assert_eq!(parse(" tiny-skia , wgpu"), Some(Backend::TinySkia));
        assert_eq!(parse(",,wgpu"), Some(Backend::Wgpu));
        assert_eq!(parse(""), None);
        assert_eq!(parse("vulkan"), None);
    }

    #[test]
    fn env_action_sets_only_what_is_needed() {
        let tiny = Backend::TinySkia;
        let wgpu = Backend::Wgpu;
        assert_eq!(env_action(None, None, wgpu), None);
        assert_eq!(env_action(None, None, tiny), Some("tiny-skia"));
        assert_eq!(env_action(None, Some("tiny-skia"), tiny), None);
        assert_eq!(env_action(Some("wgpu"), Some("tiny-skia"), wgpu), Some("wgpu"));
        assert_eq!(env_action(Some("wgpu"), None, wgpu), None);
        assert_eq!(env_action(Some("tiny-skia"), Some("tiny-skia"), tiny), Some("tiny-skia"));
    }

    #[test]
    fn child_env_is_the_original_or_empty() {
        assert_eq!(child_backend_env(None), OsString::new());
        assert_eq!(child_backend_env(Some(OsStr::new("wgpu"))), OsString::from("wgpu"));
        assert_ne!(child_backend_env(None), OsString::from("tiny-skia"));
    }

    fn env_of(command: &Command) -> Option<Option<OsString>> {
        command
            .get_envs()
            .find(|(k, _)| *k == "ICED_BACKEND")
            .map(|(_, v)| v.map(OsStr::to_os_string))
    }

    #[test]
    fn restore_removes_when_the_original_was_absent() {
        let mut command = Command::new("true");
        apply_restore(&mut command, Some(&None));
        assert_eq!(env_of(&command), Some(None));
    }

    #[test]
    fn restore_sets_the_original_value() {
        let mut command = Command::new("true");
        apply_restore(&mut command, Some(&Some(OsString::from("wgpu"))));
        assert_eq!(env_of(&command), Some(Some(OsString::from("wgpu"))));
    }

    #[test]
    fn restore_leaves_the_command_alone_without_a_record() {
        let mut command = Command::new("true");
        apply_restore(&mut command, None);
        assert_eq!(env_of(&command), None);
    }
}
