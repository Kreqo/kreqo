// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask check`: what CI runs, as one command for sessions and CI
//! alike. Each step names the rules it serves.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::ValueEnum;

use crate::{crates, deny, secrets, tools};

/// A step of `cargo xtask check`.
#[derive(Clone, Copy, ValueEnum)]
pub enum Step {
    /// rustfmt (QA-01).
    Fmt,
    /// Clippy on every target, with warnings denied (QA-01, SEC-01).
    Clippy,
    /// The workspace's tests (CI-04).
    Test,
    /// Each crate's project, license and lints (T2, LEGAL-01, QA-01).
    Crates,
    /// cargo-deny: advisories, bans, licenses and sources (SEC-02, PRIV-09, LEGAL-03).
    Deny,
    /// reuse lint (LEGAL-02).
    Reuse,
    /// Betterleaks over the git history (SEC-03).
    Secrets,
    /// zizmor over the workflows (CI-07, CI-08, CI-09).
    Workflows,
}

impl Step {
    fn name(self) -> &'static str {
        match self {
            Self::Fmt => "fmt",
            Self::Clippy => "clippy",
            Self::Test => "test",
            Self::Crates => "crates",
            Self::Deny => "deny",
            Self::Reuse => "reuse",
            Self::Secrets => "secrets",
            Self::Workflows => "workflows",
        }
    }
}

/// Runs the steps, every one when none is named, and reports each failure.
/// Returns whether all of them passed.
pub fn run(root: &Path, steps: &[Step]) -> Result<bool> {
    let steps = if steps.is_empty() {
        Step::value_variants()
    } else {
        steps
    };
    let mut failed = Vec::new();
    for &step in steps {
        println!("\n== {}", step.name());
        if let Err(error) = run_step(root, step) {
            eprintln!("{} failed: {error:#}", step.name());
            failed.push(step.name());
        }
    }
    let ran: Vec<_> = steps.iter().map(|step| step.name()).collect();
    if failed.is_empty() {
        println!("\nPassed: {}.", ran.join(", "));
        Ok(true)
    } else {
        eprintln!("\nFailed: {}.", failed.join(", "));
        Ok(false)
    }
}

fn run_step(root: &Path, step: Step) -> Result<()> {
    match step {
        Step::Fmt => cargo(root, &["fmt", "--all", "--check"]),
        Step::Clippy => cargo(
            root,
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--locked",
                "--",
                "--deny",
                "warnings",
            ],
        ),
        Step::Test => cargo(root, &["test", "--workspace", "--all-features", "--locked"]),
        Step::Crates => crates::check(root),
        Step::Deny => {
            tools::require(root, &tools::CARGO_DENY)?;
            // Both run, so that a problem in deny.toml, such as an expired
            // suppression, never hides a newly published advisory.
            let policy = deny::check_policy(root);
            let scan = cargo(
                root,
                &["deny", "--locked", "--config", "deny.toml", "check"],
            );
            both(policy, scan)
        }
        Step::Reuse => {
            tools::require(root, &tools::REUSE)?;
            run_command(root, Command::new("reuse").arg("lint"))
        }
        Step::Secrets => secrets::scan_history(root),
        Step::Workflows => {
            tools::require(root, &tools::ZIZMOR)?;
            run_command(root, Command::new("zizmor").args(["--offline", "."]))
        }
    }
}

/// Fails if either part failed, naming both failures when both did.
fn both(first: Result<()>, second: Result<()>) -> Result<()> {
    match (first, second) {
        (Err(first), Err(second)) => bail!("{first:#}; {second:#}"),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

/// Runs the cargo that runs xtask, so every step uses the pinned toolchain.
fn cargo(root: &Path, args: &[&str]) -> Result<()> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    run_command(root, Command::new(cargo).args(args))
}

/// Runs a command from the workspace root and fails unless it succeeds.
pub fn run_command(root: &Path, command: &mut Command) -> Result<()> {
    let shown = show(command);
    println!("$ {shown}");
    let status = command
        .current_dir(root)
        .status()
        .with_context(|| format!("couldn't run {shown}"))?;
    if status.success() {
        Ok(())
    } else {
        bail!("{shown} exited with {status}")
    }
}

/// A command as the log shows it, with the program's file name only.
fn show(command: &Command) -> String {
    let program = Path::new(command.get_program())
        .file_name()
        .unwrap_or(command.get_program());
    std::iter::once(program)
        .chain(command.get_args())
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}
