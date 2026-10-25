// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The files that git tracks or would track, for checks that must see what a
//! commit can carry and nothing that git ignores: build output stays out,
//! and so does the local setup material.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

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
