// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The hook installer (SEC-03), which needs only git.

use std::fs;

use anyhow::{Result, ensure};

use crate::support::Repo;

/// The hooks land in the throwaway repository, not in the checkout that
/// built xtask: xtask takes its root from cargo when it runs.
#[test]
fn installs_the_hooks_once() -> Result<()> {
    let repo = Repo::new()?;
    repo.xtask(&["hooks"]).passes()?;
    let hooks = repo.root().join(".git").join("hooks");
    for name in ["pre-commit", "commit-msg"] {
        let script = fs::read_to_string(hooks.join(name))?;
        ensure!(script.contains("exec cargo xtask"), "{name}: {script}");
    }
    repo.xtask(&["hooks"]).passes()
}

/// A hook of the user's own stays, and no hook is written beside it.
#[test]
fn leaves_another_hook_alone() -> Result<()> {
    let repo = Repo::new()?;
    let hooks = repo.root().join(".git").join("hooks");
    fs::create_dir_all(&hooks)?;
    fs::write(hooks.join("commit-msg"), "#!/bin/sh\nexit 0\n")?;
    repo.xtask(&["hooks"]).fails_with("move it aside")?;
    ensure!(
        !hooks.join("pre-commit").exists(),
        "the pre-commit hook was written after the refusal"
    );
    Ok(())
}

/// A hooks folder set in the global git config is shared with other
/// repositories, whose commits would then run cargo xtask; a local one is
/// the repository's own.
#[test]
fn refuses_a_shared_hooks_folder() -> Result<()> {
    let repo = Repo::new()?;
    let shared = repo.outside("shared-hooks");
    // git reads a backslash in a config value as an escape.
    let path = shared.display().to_string().replace('\\', "/");
    repo.write_outside("gitconfig", format!("[core]\n\thooksPath = {path}\n"))?;
    repo.xtask(&["hooks"])
        .fails_with("set in the global git config")?;
    ensure!(!shared.exists(), "the hooks landed in the shared folder");
    repo.git(&["config", "core.hooksPath", ".githooks"])?;
    repo.xtask(&["hooks"]).passes()?;
    ensure!(
        repo.root().join(".githooks").join("pre-commit").exists(),
        "the hooks didn't land in the local folder"
    );
    Ok(())
}

/// xtask runs only from a workspace root.
#[test]
fn refuses_a_root_without_a_workspace() -> Result<()> {
    let repo = Repo::new()?;
    fs::remove_file(repo.root().join("Cargo.toml"))?;
    repo.xtask(&["hooks"]).fails_with("isn't a workspace root")
}
