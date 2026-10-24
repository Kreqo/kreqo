// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Git hooks (SEC-03): `cargo xtask hooks` installs a pre-commit hook that
//! runs `cargo xtask pre-commit`, which scans the staged changes for secrets.

use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::secrets;

/// The pre-commit hook: a shim, so that the logic stays in Rust (ARCH-06).
const PRE_COMMIT: &str =
    "#!/bin/sh\n# Installed by `cargo xtask hooks` (SEC-03).\nexec cargo xtask pre-commit\n";

/// Installs the pre-commit hook, leaving any other hook in place.
pub fn install(root: &Path) -> Result<()> {
    refuse_shared_hooks_path(root)?;
    // `--git-path` honors core.hooksPath and finds the hooks of worktrees.
    let output = Command::new("git")
        .args(["rev-parse", "--git-path", "hooks"])
        .current_dir(root)
        .output()
        .context("couldn't run git")?;
    if !output.status.success() {
        bail!(
            "git rev-parse failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let hooks = String::from_utf8(output.stdout).context("git printed a non-UTF-8 path")?;
    let hooks = root.join(hooks.trim());
    let path = hooks.join("pre-commit");
    match fs::read_to_string(&path) {
        Ok(existing) if existing == PRE_COMMIT => {
            println!("The pre-commit hook is already installed.");
            return Ok(());
        }
        Ok(_) => bail!(
            "{} already exists; move it aside, then run cargo xtask hooks again",
            path.display()
        ),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("couldn't read {}", path.display()));
        }
    }
    fs::create_dir_all(&hooks).with_context(|| format!("couldn't create {}", hooks.display()))?;
    fs::write(&path, PRE_COMMIT).with_context(|| format!("couldn't write {}", path.display()))?;
    make_executable(&path)?;
    println!("Installed {}.", path.display());
    Ok(())
}

/// Fails if core.hooksPath comes from a git config that other repositories
/// share, where the hook would make their commits run `cargo xtask`.
fn refuse_shared_hooks_path(root: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["config", "--show-scope", "--get", "core.hooksPath"])
        .current_dir(root)
        .output()
        .context("couldn't run git")?;
    // git exits with 1 when the setting is absent.
    if !output.status.success() {
        return Ok(());
    }
    let printed = String::from_utf8_lossy(&output.stdout);
    let scope = printed.split('\t').next().unwrap_or_default();
    if matches!(scope, "local" | "worktree") {
        return Ok(());
    }
    bail!(
        "core.hooksPath is set in the {scope} git config, so the hook would land in a folder other repositories share; set a local core.hooksPath or unset it there, then run cargo xtask hooks again"
    )
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .with_context(|| format!("couldn't make {} executable", path.display()))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> {
    // Git for Windows runs hooks through its own shell, without a mode bit.
    Ok(())
}

/// Scans the staged changes for secrets, as the pre-commit hook does.
pub fn pre_commit(root: &Path) -> Result<()> {
    secrets::scan_staged(root)
}
