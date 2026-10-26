// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The workflow audit (CI-07, CI-08, CI-09): zizmor reads every workflow,
//! action and Dependabot config that git lists.

use anyhow::Result;

use crate::support::Repo;

/// A workflow with no findings.
const CLEAN: &str = "name: Clean\non: push\npermissions: {}\njobs:\n  clean:\n    runs-on: ubuntu-24.04\n    steps:\n      - run: echo clean\n";

/// A workflow with an action that isn't pinned by its commit's SHA.
const UNPINNED: &str = "name: Unpinned\non: push\npermissions: {}\njobs:\n  unpinned:\n    runs-on: ubuntu-24.04\n    steps:\n      - uses: actions/checkout@v4\n        with:\n          persist-credentials: false\n";

/// A repository whose workflows pass, with the real zizmor config.
fn audited_repo() -> Result<Repo> {
    let repo = Repo::new()?;
    repo.write(
        ".github/zizmor.yml",
        include_str!("../../../.github/zizmor.yml"),
    )?;
    repo.write(".github/workflows/clean.yml", CLEAN)?;
    repo.commit_all("Add a workflow")?;
    repo.xtask(&["check", "workflows"]).passes()?;
    Ok(repo)
}

#[test]
#[ignore = "needs zizmor; cargo xtask check tool-tests runs it"]
fn the_audit_reads_a_tracked_workflow_that_git_ignores() -> Result<()> {
    let repo = audited_repo()?;
    repo.write(".gitignore", ".github/workflows/\n")?;
    repo.write(".github/workflows/unpinned.yml", UNPINNED)?;
    repo.git(&[
        "add",
        "--force",
        ".gitignore",
        ".github/workflows/unpinned.yml",
    ])?;
    repo.xtask(&["check", "workflows"])
        .fails_with("unpinned.yml")
}

#[test]
#[ignore = "needs zizmor; cargo xtask check tool-tests runs it"]
fn the_audit_reads_nested_workflows() -> Result<()> {
    let repo = audited_repo()?;
    repo.write("templates/app/.github/workflows/ci.yml", UNPINNED)?;
    repo.xtask(&["check", "workflows"])
        .fails_with("templates/app/.github/workflows/ci.yml")
}

#[test]
#[ignore = "needs zizmor; cargo xtask check tool-tests runs it"]
fn the_audit_reads_the_dependabot_config() -> Result<()> {
    let repo = audited_repo()?;
    repo.write(
        ".github/dependabot.yml",
        "version: 2\nupdates:\n  - package-ecosystem: cargo\n    directory: /\n    schedule:\n      interval: weekly\n    insecure-external-code-execution: allow\n",
    )?;
    repo.xtask(&["check", "workflows"])
        .fails_with("dependabot-execution")
}
