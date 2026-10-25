// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The files that git tracks or would track, for checks that must see what a
//! commit can carry and nothing that git ignores: build output stays out,
//! and so does the local setup material.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// The oldest git the checks support: the secret scan holds git to its
/// settings with GIT_CONFIG_COUNT, which arrived in git 2.31.
const OLDEST: (u32, u32) = (2, 31);

/// Fails unless this machine's git is at least the oldest the checks
/// support.
pub fn require_version(root: &Path) -> Result<()> {
    let output = Command::new("git")
        .arg("--version")
        .current_dir(root)
        .output()
        .context("couldn't run git")?;
    let printed = String::from_utf8_lossy(&output.stdout);
    match version(&printed) {
        Some(found) if found >= OLDEST => Ok(()),
        Some((major, minor)) => bail!(
            "the checks need git {}.{} or later, but this machine has {major}.{minor}",
            OLDEST.0,
            OLDEST.1
        ),
        None => bail!("couldn't read git's version from {:?}", printed.trim()),
    }
}

/// The major and minor version in `git --version`'s output, such as
/// `git version 2.39.3 (Apple Git-146)` or `git version 2.47.0.windows.1`.
fn version(printed: &str) -> Option<(u32, u32)> {
    let number = printed
        .trim()
        .strip_prefix("git version ")?
        .split_whitespace()
        .next()?;
    let mut parts = number.split('.');
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

/// The files matching the pathspecs that git tracks or would track, relative
/// to the root, sorted. Files deleted from the working tree are left out.
pub fn files(root: &Path, pathspecs: &[&str]) -> Result<Vec<PathBuf>> {
    require_top_level(root)?;
    let output = Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
        ])
        .args(pathspecs)
        .current_dir(root)
        .output()
        .context("couldn't run git ls-files")?;
    if !output.status.success() {
        bail!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let listed = String::from_utf8(output.stdout).context("git listed a non-UTF-8 path")?;
    let mut files: Vec<PathBuf> = listed
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .filter(|path| root.join(path).is_file())
        .collect();
    files.sort();
    files.dedup();
    Ok(files)
}

/// Fails unless the root is the top of a git checkout. Outside git, the
/// checks have no file list; inside an enclosing repository that ignores
/// this folder, git would list nothing, and every check would pass.
fn require_top_level(root: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(root)
        .output()
        .context("couldn't run git")?;
    if !output.status.success() {
        bail!(
            "{} isn't in a git checkout, so the checks have no file list",
            root.display()
        );
    }
    let top = String::from_utf8(output.stdout).context("git printed a non-UTF-8 path")?;
    let top = PathBuf::from(top.trim_end_matches(['\n', '\r']));
    let same = match (fs::canonicalize(&top), fs::canonicalize(root)) {
        (Ok(top), Ok(root)) => top == root,
        _ => false,
    };
    if !same {
        bail!(
            "{} isn't the top of its git checkout ({}), so git's file list can't be trusted",
            root.display(),
            top.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_git_versions() {
        assert_eq!(version("git version 2.43.0\n"), Some((2, 43)));
        assert_eq!(
            version("git version 2.39.3 (Apple Git-146)\n"),
            Some((2, 39))
        );
        assert_eq!(version("git version 2.47.0.windows.1\n"), Some((2, 47)));
        assert_eq!(version("git help\n"), None);
        assert!((2, 30) < OLDEST && (2, 31) >= OLDEST && (3, 0) >= OLDEST);
    }
}
