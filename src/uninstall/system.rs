// SPDX-License-Identifier: GPL-3.0-only
//! The real system: programs run and files looked at for real.

use std::path::{Path, PathBuf};

use super::{Fs, Out, Runner};

pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(&self, program: &str, args: &[String]) -> Result<Out, String> {
        let o = std::process::Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|e| format!("{program}: {e}"))?;
        Ok(Out {
            ok: o.status.success(),
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        })
    }
}

pub struct RealFs;

impl Fs for RealFs {
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
    fn is_regular_nolink(&self, path: &Path) -> bool {
        path.symlink_metadata().is_ok_and(|m| m.is_file())
    }
    fn canonicalize(&self, path: &Path) -> Option<PathBuf> {
        path.canonicalize().ok()
    }
    fn home(&self) -> Option<PathBuf> {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
    }
    fn owned_by_me(&self, path: &Path) -> bool {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: getuid has no preconditions and cannot fail.
        let me = unsafe { libc::getuid() };
        path.metadata().is_ok_and(|m| m.uid() == me)
    }
}
