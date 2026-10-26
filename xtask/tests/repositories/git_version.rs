// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The oldest git the checks support, 2.31, which the secret scan's git
//! settings need (SEC-03). A stand-in git reports an older version.

use std::fs;
use std::os::unix::fs::PermissionsExt;

use anyhow::{Context, Result};

use crate::support::Repo;

/// Every command that scans with git or installs the hooks refuses an
/// older git before it runs anything else.
#[test]
fn the_scans_and_the_hooks_need_git_2_31() -> Result<()> {
    let repo = Repo::new()?;
    let folder = repo.outside("old-git");
    fs::create_dir(&folder)?;
    let git = folder.join("git");
    fs::write(
        &git,
        "#!/bin/sh\nif [ \"$1\" = --version ]; then\n  echo 'git version 2.30.9'\n  exit 0\nfi\nexit 1\n",
    )?;
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755))?;
    let search = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(std::iter::once(folder).chain(std::env::split_paths(&search)))?;
    let path = path.to_str().context("PATH isn't UTF-8")?;
    let message = repo.write_outside("COMMIT_EDITMSG", "Add x\n")?;
    let message = message.to_str().context("the scratch path isn't UTF-8")?;
    for args in [
        &["hooks"][..],
        &["pre-commit"],
        &["commit-msg", message],
        &["check", "secrets"],
        &["scan-all-refs"],
    ] {
        repo.xtask_with(args, &[("PATH", path)])
            .fails_with("the checks need git 2.31 or later, but this machine has 2.30")?;
    }
    Ok(())
}
