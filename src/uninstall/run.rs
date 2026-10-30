// SPDX-License-Identifier: GPL-3.0-only
//! The removal itself: what a `Plan` says, step by step.

use std::path::{Path, PathBuf};

use super::{Fs, Plan, RealFs, Runner, Step, cut};
use crate::texts::texts;

fn exec_cmd(r: &dyn Runner, program: &str, args: Vec<String>) -> Result<String, String> {
    let out = r.run(program, &args)?;
    if out.ok {
        Ok(out.stdout)
    } else {
        Err(cut(&out.stderr))
    }
}

fn trash(r: &dyn Runner, paths: &[PathBuf]) -> Result<(), String> {
    let mut args = vec!["trash".to_string(), "--".to_string()];
    args.extend(paths.iter().map(|p| p.to_string_lossy().into_owned()));
    exec_cmd(r, "gio", args).map(|_| ())
}

fn waydroid_remove(r: &dyn Runner, pkg: &str, desktop: &Path) -> Result<(), String> {
    exec_cmd(
        r,
        "waydroid",
        vec!["app".into(), "remove".into(), pkg.into()],
    )?;
    // `app remove` exits 0 even when it failed: look at the list.
    let list = exec_cmd(r, "waydroid", vec!["app".into(), "list".into()])?;
    let still = list
        .lines()
        .filter_map(|l| l.trim().strip_prefix("packageName:"))
        .any(|v| v.trim() == pkg);
    if still {
        return Err(texts().err_waydroid_kept.replace("{}", pkg));
    }
    trash(r, &[desktop.to_path_buf()])
}

fn run_step(step: &Step, delete_data: bool, r: &dyn Runner, fs: &dyn Fs) -> Result<(), String> {
    match step {
        Step::Flatpak { reference, system } => {
            let scope = if *system { "--system" } else { "--user" };
            let mut a: Vec<String> = ["uninstall", "-y", "--noninteractive", scope]
                .map(String::from)
                .into();
            if delete_data {
                a.push("--delete-data".into());
            }
            a.push(reference.clone());
            exec_cmd(r, "flatpak", a).map(|_| ())
        }
        Step::Cmd { program, args } => exec_cmd(r, program, args.clone()).map(|_| ()),
        Step::Wine { prefix } => {
            // Looked at again: the prefix may have gone since the plan.
            if !fs.exists(&prefix.join("system.reg")) {
                return Err(texts()
                    .err_wine_gone
                    .replace("{}", &prefix.display().to_string()));
            }
            let a = [
                format!("WINEPREFIX={}", prefix.display()),
                "wine".into(),
                "uninstaller".into(),
            ];
            exec_cmd(r, "env", a.into()).map(|_| ())
        }
        Step::WaydroidRemove { package, desktop } => waydroid_remove(r, package, desktop),
        Step::Trash(paths) => trash(r, paths),
        Step::Pacman { package } => {
            // The preview may differ from what pacman removes by now; pacman
            // itself still refuses anything another package needs.
            let a = ["/usr/bin/pacman", "-Rs", "--noconfirm", "--", package].map(String::from);
            exec_cmd(r, "pkexec", a.into()).map(|_| ())
        }
    }
}

/// Does what the plan says, stopping at the first failure.
pub fn run_with(plan: &Plan, delete_data: bool, r: &dyn Runner, fs: &dyn Fs) -> Result<(), String> {
    plan.steps
        .iter()
        .try_for_each(|s| run_step(s, delete_data, r, fs))
}

/// `run_with` on the real file system.
pub fn run(plan: &Plan, delete_data: bool, r: &dyn Runner) -> Result<(), String> {
    run_with(plan, delete_data, r, &RealFs)
}
