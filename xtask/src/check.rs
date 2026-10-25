// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask check`: what CI runs, as one command for sessions and CI
//! alike. Each step names the rules it serves.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
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
    /// The API docs, with rustdoc's warnings denied (CI-02).
    Docs,
    /// Each crate's project, license and lints (T2, LEGAL-01, QA-01).
    Crates,
    /// cargo-deny: advisories, bans, licenses and sources (SEC-02, PRIV-09, LEGAL-03, ARCH-13).
    Deny,
    /// reuse lint (LEGAL-02).
    Reuse,
    /// Betterleaks over the git history (SEC-03).
    Secrets,
    /// zizmor over the workflows, online when GH_TOKEN is set (CI-07, CI-08, CI-09).
    Workflows,
}

impl Step {
    fn name(self) -> &'static str {
        match self {
            Self::Fmt => "fmt",
            Self::Clippy => "clippy",
            Self::Test => "test",
            Self::Docs => "docs",
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
    check_cargo_aliases(root)?;
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
        Step::Docs => run_command(
            root,
            cargo_command()
                .args([
                    "doc",
                    "--workspace",
                    "--no-deps",
                    "--all-features",
                    "--locked",
                ])
                .env("RUSTDOCFLAGS", "--deny warnings"),
        ),
        Step::Crates => crates::check(root),
        Step::Deny => {
            tools::require(root, &tools::CARGO_DENY)?;
            // Both run, so that a problem in deny.toml, such as an expired
            // suppression, never hides a newly published advisory.
            let policy = deny::check_policy(root);
            // The binary itself, so that no cargo alias can stand in for it.
            let scan = run_command(
                root,
                tools::CARGO_DENY
                    .command()
                    .args(["--locked", "--config", "deny.toml", "check"]),
            );
            both(policy, scan)
        }
        Step::Reuse => {
            tools::require(root, &tools::REUSE)?;
            run_command(root, tools::REUSE.command().arg("lint"))
        }
        Step::Secrets => secrets::scan_history(root),
        Step::Workflows => {
            tools::require(root, &tools::ZIZMOR)?;
            // A workflow zizmor can't parse fails rather than going unaudited.
            let mut zizmor = tools::ZIZMOR.command();
            zizmor.arg("--strict-collection");
            // The online audits, such as whether a pinned SHA is a real commit
            // of its action, need a token: CI passes the job's own, read-only
            // one. Without a token, zizmor stays offline.
            if std::env::var_os("GH_TOKEN").is_none() {
                zizmor.arg("--offline");
            }
            run_command(root, zizmor.arg("."))
        }
    }
}

/// External cargo commands that the steps run, which a cargo alias could
/// replace; cargo ignores an alias only for its built-in commands.
const ALIASABLE: [&str; 3] = ["fmt", "clippy", "deny"];

/// Fails if a cargo alias could stand in for a command the steps run: the
/// repository's own config may define only `xtask`, and no config above it
/// or in CARGO_HOME may define one of the aliasable commands.
fn check_cargo_aliases(root: &Path) -> Result<()> {
    let mut problems = Vec::new();
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(|home| PathBuf::from(home).join(".cargo"))
        });
    let folders = root
        .ancestors()
        .map(|folder| folder.join(".cargo"))
        .chain(cargo_home);
    for folder in folders {
        for name in ["config.toml", "config"] {
            let path = folder.join(name);
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let in_repository = path.starts_with(root);
            for alias in
                aliases(&text).with_context(|| format!("couldn't parse {}", path.display()))?
            {
                let allowed = if in_repository {
                    alias == "xtask"
                } else {
                    !ALIASABLE.contains(&alias.as_str())
                };
                if !allowed {
                    problems.push(format!(
                        "{} defines the cargo alias `{alias}`",
                        path.display()
                    ));
                }
            }
        }
    }
    if problems.is_empty() {
        return Ok(());
    }
    bail!(
        "{}; an alias could stand in for a command the checks run",
        problems.join("; ")
    )
}

/// The aliases a cargo config defines.
fn aliases(config: &str) -> Result<Vec<String>> {
    let config: toml::Table = config.parse()?;
    Ok(config
        .get("alias")
        .and_then(toml::Value::as_table)
        .map(|aliases| aliases.keys().cloned().collect())
        .unwrap_or_default())
}

/// Fails if either part failed, naming both failures when both did.
fn both(first: Result<()>, second: Result<()>) -> Result<()> {
    match (first, second) {
        (Err(first), Err(second)) => bail!("{first:#}; {second:#}"),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

/// Runs the cargo that runs xtask with these arguments.
fn cargo(root: &Path, args: &[&str]) -> Result<()> {
    run_command(root, cargo_command().args(args))
}

/// The cargo that runs xtask, so every step uses the pinned toolchain.
fn cargo_command() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// CI's `cargo xtask check` lines, together, run every step, so that a
    /// new step can't run only locally.
    #[test]
    fn ci_runs_every_step() {
        let workflow = include_str!("../../.github/workflows/ci.yml");
        let mut in_ci: Vec<&str> = workflow
            .lines()
            .filter_map(|line| line.trim().strip_prefix("run: cargo xtask check "))
            .flat_map(str::split_whitespace)
            .collect();
        in_ci.sort_unstable();
        in_ci.dedup();
        let mut every: Vec<&str> = Step::value_variants()
            .iter()
            .map(|step| step.name())
            .collect();
        every.sort_unstable();
        assert_eq!(in_ci, every);
    }

    #[test]
    fn reads_cargo_aliases() -> Result<()> {
        let config = "[alias]\nxtask = \"run --package xtask --\"\ndeny = \"help\"\n";
        assert_eq!(aliases(config)?, ["deny", "xtask"]);
        assert!(aliases("[build]\njobs = 4\n")?.is_empty());
        Ok(())
    }
}
