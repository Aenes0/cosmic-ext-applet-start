// SPDX-License-Identifier: GPL-3.0-only
//! Uninstall of an app from the menu: where it came from (`detect`, pure),
//! what would go (`plan`) and the removal itself (`run`). Every outside
//! effect goes through `Fs` and `Runner`, so the tests touch nothing real
//! (`RealFs` and `SystemRunner` are the real ones, used by the caller).

use std::path::{Path, PathBuf};

use crate::texts::texts;

mod exec;
mod run;
mod system;
pub use exec::{ParsedExec, parse_exec};
pub use run::run;
pub use system::{RealFs, SystemRunner};

/// Where an app came from, and so how it is removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Protected,
    Flatpak { reference: String, system: bool },
    Steam { app_id: String },
    Waydroid { package: String },
    Wine { prefix: PathBuf },
    AppImage { file: PathBuf },
    Pacman,
    LooseDesktop,
    Unknown,
}

impl Origin {
    /// Whether the menu may offer "Uninstall" at all.
    pub fn removable(&self) -> bool {
        !matches!(self, Origin::Protected | Origin::Unknown)
    }
}

/// What is known of a `.desktop` entry, read by the caller.
#[derive(Debug, Clone, Default)]
pub struct DesktopFacts {
    pub appid: String,
    pub path: PathBuf,
    /// Raw `Exec` value, as in the file.
    pub exec: Option<String>,
    /// `X-Flatpak`.
    pub flatpak_ref: Option<String>,
    /// `X-Flatpak-Part-Of`.
    pub flatpak_part_of: Option<String>,
    /// The same id also exists in another data directory (an override).
    pub shadows_other_data_dir: bool,
}

/// The file system, as far as detection needs it.
pub trait Fs {
    fn exists(&self, path: &Path) -> bool;
    /// A regular file, not a symlink.
    fn is_regular_nolink(&self, path: &Path) -> bool;
    fn canonicalize(&self, path: &Path) -> Option<PathBuf>;
    fn home(&self) -> Option<PathBuf>;
    fn owned_by_me(&self, path: &Path) -> bool;
}

/// Result of a program run.
#[derive(Debug, Clone, Default)]
pub struct Out {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Runs programs, with no shell.
pub trait Runner {
    fn run(&self, program: &str, args: &[String]) -> Result<Out, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Flatpak { reference: String, system: bool },
    Cmd { program: String, args: Vec<String> },
    Wine { prefix: PathBuf },
    WaydroidRemove { package: String, desktop: PathBuf },
    Trash(Vec<PathBuf>),
    Pacman { package: String },
}

/// What will be done, ready to show and to run.
#[derive(Debug, Clone)]
pub struct Plan {
    pub label: String,
    pub preview: Vec<String>,
    pub supports_data_delete: bool,
    steps: Vec<Step>,
}

impl Plan {
    /// Whether the plan only opens the other program's own uninstaller
    /// (Wine's, Steam's): nothing is removed by the menu itself.
    pub fn opens_other_program(&self) -> bool {
        matches!(self.steps.first(), Some(Step::Wine { .. }))
            || matches!(self.steps.first(), Some(Step::Cmd { program, .. }) if program == "xdg-open")
    }
}

// ---- Validation ----

fn is_ref(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with(['-', '.'])
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn is_android_pkg(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with(['-', '.'])
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_'))
}

fn is_system_android(s: &str) -> bool {
    s.starts_with("com.android.") || s.starts_with("org.lineageos.")
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

fn is_pacman_pkg(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with(['-', '.'])
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '_' | '-' | '@'))
}

fn is_protected(appid: &str) -> bool {
    appid.starts_with("io.github.Aenes0.")
        || appid.starts_with("cosmic-")
        || appid.starts_with("com.system76.Cosmic")
}

fn base(tok: &str) -> &str {
    tok.rsplit('/').next().unwrap_or(tok)
}

/// Shells, wine, proton, flatpak and steam launchers: they hide what they run.
fn is_wrapper(argv: &[String]) -> bool {
    let shell = matches!(base(&argv[0]), "sh" | "bash" | "dash" | "zsh" | "fish");
    shell
        || argv.iter().any(|t| {
            let b = base(t).to_lowercase();
            b.contains("proton") || b.contains("wine") || b == "flatpak" || b == "steam"
        })
}

// ---- Detection ----

fn detect_flatpak(f: &DesktopFacts, fs: &dyn Fs) -> Option<Origin> {
    let r = f.flatpak_ref.as_deref()?;
    if f.flatpak_part_of.is_some() || !is_ref(r) {
        return Some(Origin::Unknown);
    }
    // The key must name this very app: a web app or portal entry of a
    // browser carries the browser's ref, and must not remove the browser.
    // An id that only starts with the ref counts just for the entries
    // Flatpak itself exports; anywhere else the id must equal the ref.
    let own =
        f.appid == r || (in_flatpak_exports(&f.path) && f.appid.starts_with(&format!("{r}.")));
    if !own {
        return Some(Origin::Unknown);
    }
    let user = fs
        .home()
        .map(|h| h.join(".local/share/flatpak/app").join(r));
    if user.is_some_and(|p| fs.exists(&p)) {
        return Some(Origin::Flatpak {
            reference: r.into(),
            system: false,
        });
    }
    if fs.exists(&Path::new("/var/lib/flatpak/app").join(r)) {
        return Some(Origin::Flatpak {
            reference: r.into(),
            system: true,
        });
    }
    Some(Origin::Unknown)
}

/// The `.desktop` was exported by Flatpak (system or user installation).
fn in_flatpak_exports(path: &Path) -> bool {
    !path
        .components()
        .any(|c| c == std::path::Component::ParentDir)
        && path
            .parent()
            .is_some_and(|d| d.ends_with("flatpak/exports/share/applications"))
}

fn detect_wine(p: &ParsedExec, fs: &dyn Fs) -> Origin {
    let prefix = p
        .vars
        .iter()
        .find(|(k, _)| k == "WINEPREFIX")
        .map(|(_, v)| PathBuf::from(v));
    let Some(prefix) = prefix else {
        return Origin::Unknown;
    };
    let is_wine = matches!(base(&p.argv[0]), "wine" | "wine64");
    let valid = is_wine
        && prefix.is_absolute()
        && !prefix
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        && fs.exists(&prefix)
        && fs.owned_by_me(&prefix)
        && fs.exists(&prefix.join("system.reg"));
    if valid {
        Origin::Wine { prefix }
    } else {
        Origin::Unknown
    }
}

/// The `.desktop` is a direct child of the user's own applications
/// directory (the same rule `plan` applies, by `in_apps_dir`).
fn in_user_apps(path: &Path, fs: &dyn Fs) -> bool {
    let dir = fs.home().map(|h| h.join(".local/share/applications"));
    dir.is_some_and(|d| path.parent() == Some(d.as_path()))
}

/// Only `argv[0]` counts; `None` (rule does not fit) when it is no safe
/// AppImage under `$HOME`, so later rules still get their turn.
fn detect_appimage(p: &ParsedExec, f: &DesktopFacts, fs: &dyn Fs) -> Option<Origin> {
    let tok = &p.argv[0];
    if !tok.to_lowercase().ends_with(".appimage") || !in_user_apps(&f.path, fs) {
        return None;
    }
    let path = Path::new(tok);
    let home = fs.home().and_then(|h| fs.canonicalize(&h));
    let real = fs.canonicalize(path);
    let ok = path.is_absolute()
        && fs.is_regular_nolink(path)
        && matches!((&home, &real), (Some(h), Some(r)) if r.starts_with(h));
    match real {
        Some(file) if ok => Some(Origin::AppImage { file }),
        _ => None,
    }
}

/// Rules that read the `Exec`.
fn detect_by_exec(p: &ParsedExec, f: &DesktopFacts, fs: &dyn Fs) -> Option<Origin> {
    const STEAM: &str = "steam://rungameid/";
    if let Some(rest) = p.argv.iter().find_map(|t| t.strip_prefix(STEAM)) {
        let id = rest.to_string();
        return Some(if is_digits(&id) {
            Origin::Steam { app_id: id }
        } else {
            Origin::Unknown
        });
    }
    if p.argv.len() >= 4 && p.argv[0] == "waydroid" && p.argv[1] == "app" && p.argv[2] == "launch" {
        let pkg = &p.argv[3];
        let good = is_android_pkg(pkg) && !is_system_android(pkg) && in_user_apps(&f.path, fs);
        return Some(if good {
            Origin::Waydroid {
                package: pkg.clone(),
            }
        } else {
            Origin::Unknown
        });
    }
    if p.vars.iter().any(|(k, _)| k == "WINEPREFIX") {
        return Some(detect_wine(p, fs));
    }
    detect_appimage(p, f, fs)
}

/// Where the app came from; the first rule that fits wins.
pub fn detect(f: &DesktopFacts, fs: &dyn Fs) -> Origin {
    if is_protected(&f.appid) {
        return Origin::Protected;
    }
    if let Some(o) = detect_flatpak(f, fs) {
        return o;
    }
    let parsed = match f.exec.as_deref().map(parse_exec) {
        Some(None) => return Origin::Unknown,
        Some(Some(p)) => Some(p),
        None => None,
    };
    if let Some(o) = parsed.as_ref().and_then(|p| detect_by_exec(p, f, fs)) {
        return o;
    }
    if f.path
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return Origin::Unknown;
    }
    if f.path.starts_with("/usr/") && !f.path.starts_with("/usr/local/") {
        return Origin::Pacman;
    }
    let in_loose = in_user_apps(&f.path, fs);
    let wrapped = parsed.as_ref().is_some_and(|p| is_wrapper(&p.argv));
    if in_loose && !f.shadows_other_data_dir && !wrapped {
        return Origin::LooseDesktop;
    }
    Origin::Unknown
}

// ---- Plan ----

pub(crate) fn cut(s: &str) -> String {
    let t = s.trim();
    if t.chars().count() <= 400 {
        return t.to_string();
    }
    let mut out: String = t.chars().take(399).collect();
    out.push('…');
    out
}

fn call(r: &dyn Runner, program: &str, args: &[&str]) -> Result<Out, String> {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    r.run(program, &args)
}

fn call_ok(r: &dyn Runner, program: &str, args: &[&str]) -> Result<String, String> {
    let out = call(r, program, args)?;
    if out.ok {
        Ok(out.stdout)
    } else {
        Err(cut(&out.stderr))
    }
}

fn plan_pacman(f: &DesktopFacts, r: &dyn Runner) -> Result<Plan, String> {
    let path = f.path.to_string_lossy().into_owned();
    let owner = call_ok(r, "pacman", &["-Qqo", "--", &path])?;
    let pkg = owner.lines().next().unwrap_or("").trim().to_string();
    if !is_pacman_pkg(&pkg) {
        return Err(texts().err_no_owner.into());
    }
    let group = call_ok(r, "pacman", &["-Qqg", "cosmic"]).unwrap_or_default();
    if group.lines().any(|l| l.trim() == pkg) {
        return Err(texts().err_cosmic_pkg.replace("{}", &pkg));
    }
    let removal = call_ok(
        r,
        "pacman",
        &["-Rs", "-p", "--print-format", "%n %v", "--", &pkg],
    )?;
    let mut preview: Vec<String> = removal.lines().map(|l| l.trim().to_string()).collect();
    preview.retain(|l| !l.is_empty());
    let files = call_ok(r, "pacman", &["-Qql", "--", &pkg]).unwrap_or_default();
    for l in files
        .lines()
        .filter(|l| l.ends_with(".desktop") && *l != path)
    {
        preview.push(format!("App: {}", base(l)));
    }
    Ok(Plan {
        label: "pacman".into(),
        preview,
        supports_data_delete: false,
        steps: vec![Step::Pacman { package: pkg }],
    })
}

fn plan_waydroid(pkg: &str, f: &DesktopFacts, r: &dyn Runner) -> Result<Plan, String> {
    if !is_android_pkg(pkg) || is_system_android(pkg) {
        return Err(texts().err_pkg_not_allowed.into());
    }
    let status = call_ok(r, "waydroid", &["status"])?;
    if !in_apps_dir(&f.path) {
        return Err(texts().err_shortcut_place.into());
    }
    let running = status
        .lines()
        .any(|l| l.starts_with("Session:") && l.contains("RUNNING"));
    if !running {
        return Err(texts().err_start_waydroid.into());
    }
    Ok(Plan {
        label: "Waydroid".into(),
        preview: vec![pkg.to_string()],
        supports_data_delete: false,
        steps: vec![Step::WaydroidRemove {
            package: pkg.into(),
            desktop: f.path.clone(),
        }],
    })
}

fn in_apps_dir(path: &Path) -> bool {
    path.parent()
        .is_some_and(|d| d.ends_with(".local/share/applications"))
}

fn simple(label: &str, preview: Vec<String>, data: bool, step: Step) -> Plan {
    Plan {
        label: label.into(),
        preview,
        supports_data_delete: data,
        steps: vec![step],
    }
}

/// What removing this app would do. Reads only: nothing is changed.
pub fn plan(origin: &Origin, f: &DesktopFacts, r: &dyn Runner) -> Result<Plan, String> {
    let shown = f.path.display().to_string();
    match origin {
        Origin::Protected | Origin::Unknown => Err(texts().err_unknown_origin.into()),
        Origin::Flatpak { reference, system } if is_ref(reference) => Ok(simple(
            "Flatpak",
            vec![reference.clone()],
            true,
            Step::Flatpak {
                reference: reference.clone(),
                system: *system,
            },
        )),
        Origin::Steam { app_id } if is_digits(app_id) => {
            let args = vec![format!("steam://uninstall/{app_id}")];
            let step = Step::Cmd {
                program: "xdg-open".into(),
                args,
            };
            Ok(simple(
                "Steam",
                vec![texts().steam_uninstaller.replace("{}", app_id)],
                false,
                step,
            ))
        }
        Origin::Waydroid { package } => plan_waydroid(package, f, r),
        Origin::Wine { prefix } => {
            let step = Step::Wine {
                prefix: prefix.clone(),
            };
            let preview = vec![
                texts().wine_uninstaller.to_owned(),
                prefix.display().to_string(),
            ];
            Ok(simple("Wine", preview, false, step))
        }
        Origin::AppImage { .. } if !in_apps_dir(&f.path) => Err(texts().err_shortcut_place.into()),
        Origin::AppImage { file } => Ok(simple(
            "AppImage",
            vec![file.display().to_string(), shown],
            false,
            Step::Trash(vec![file.clone(), f.path.clone()]),
        )),
        Origin::Pacman => plan_pacman(f, r),
        Origin::LooseDesktop => Ok(simple(
            "Atalho",
            vec![shown],
            false,
            Step::Trash(vec![f.path.clone()]),
        )),
        _ => Err(texts().err_invalid.into()),
    }
}

#[cfg(test)]
mod tests;
