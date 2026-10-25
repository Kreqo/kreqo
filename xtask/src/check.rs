// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask check`: what CI runs, as one command for sessions and CI
//! alike. Each step names the rules it serves.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::ValueEnum;

use crate::{crates, deny, git, secrets, tools};

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
    /// zizmor over the workflows, actions and Dependabot configs, online when
    /// GH_TOKEN is set (CI-07, CI-08, CI-09).
    Workflows,
    /// xtask's own tests that run the pinned Betterleaks and zizmor, which a
    /// plain `cargo test` skips (QA-09).
    ToolTests,
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
            Self::ToolTests => "tool-tests",
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
    check_cargo_config(root)?;
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
        Step::Docs => run_command(root, &mut docs()),
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
            let token = std::env::var("GH_TOKEN")
                .ok()
                .filter(|token| !token.is_empty());
            let in_ci = std::env::var("CI").is_ok_and(|ci| ci == "true");
            let mut zizmor = zizmor(token, in_ci)?;
            // The files come from git, so that an ignore rule can't hide a
            // tracked one: what zizmor's own collection finds, workflows in
            // nested folders and Dependabot configs included.
            let workflows = git::files(
                root,
                &[
                    ":(glob)**/.github/workflows/*.yml",
                    ":(glob)**/.github/workflows/*.yaml",
                    ":(glob)**/.github/dependabot.yml",
                    ":(glob)**/.github/dependabot.yaml",
                    ":(glob)**/action.yml",
                    ":(glob)**/action.yaml",
                ],
            )?;
            if workflows.is_empty() {
                bail!("git lists no workflow files to audit");
            }
            run_command(
                root,
                zizmor
                    .args(["--config", ".github/zizmor.yml"])
                    .args(&workflows),
            )
        }
        Step::ToolTests => {
            tools::require(root, &tools::BETTERLEAKS)?;
            tools::require(root, &tools::ZIZMOR)?;
            cargo(
                root,
                &[
                    "test",
                    "--package",
                    "xtask",
                    "--all-features",
                    "--locked",
                    "--",
                    "--ignored",
                ],
            )
        }
    }
}

/// rustdoc over the workspace, with its warnings, such as a broken
/// intra-doc link, denied (CI-02).
fn docs() -> Command {
    let mut command = cargo_command();
    command
        .args([
            "doc",
            "--workspace",
            "--no-deps",
            "--all-features",
            "--locked",
        ])
        .env("RUSTDOCFLAGS", "--deny warnings");
    command
}

/// Settings that zizmor reads from the environment, which could turn its
/// online audits off, send the token to another host, or fail a run on an
/// empty value. Only GH_TOKEN carries the token.
const ZIZMOR_SETTINGS: [&str; 5] = [
    "GITHUB_TOKEN",
    "ZIZMOR_GITHUB_TOKEN",
    "ZIZMOR_OFFLINE",
    "ZIZMOR_NO_ONLINE_AUDITS",
    "GH_HOST",
];

/// zizmor, failing on a workflow it can't parse rather than leaving it
/// unaudited. The online audits, such as whether a pinned SHA is a real
/// commit of its action, need a token: CI passes the job's own, read-only
/// one, to the step that runs this. Without one, zizmor stays offline, and
/// in CI, the step fails.
fn zizmor(token: Option<String>, in_ci: bool) -> Result<Command> {
    let mut zizmor = tools::ZIZMOR.command();
    zizmor.arg("--strict-collection");
    for setting in ZIZMOR_SETTINGS {
        zizmor.env_remove(setting);
    }
    match token {
        Some(token) => {
            zizmor.env("GH_TOKEN", token);
        }
        None if in_ci => {
            bail!("CI runs zizmor's online audits, but GH_TOKEN is missing or empty")
        }
        None => {
            zizmor.arg("--offline").env_remove("GH_TOKEN");
        }
    }
    Ok(zizmor)
}

/// External cargo commands that the steps run, which a cargo alias could
/// replace; cargo ignores an alias only for its built-in commands.
const ALIASABLE: [&str; 3] = ["fmt", "clippy", "deny"];

/// Fails if a cargo config in the repository could change what the steps
/// run, or one above it or in CARGO_HOME could alias a command they run.
/// The repository's `.cargo/config.toml` may hold only the `xtask` alias:
/// another alias could stand in for a step, `rustflags` could lower every
/// lint, and `[env]` or `include` could do either. Cargo also reads a
/// legacy `.cargo/config`, so none may exist.
fn check_cargo_config(root: &Path) -> Result<()> {
    let mut problems = Vec::new();
    let folder = root.join(".cargo");
    let legacy = folder.join("config");
    if legacy.exists() {
        problems.push(format!(
            "{} exists; cargo reads it too, so the repository's cargo settings belong in config.toml",
            legacy.display()
        ));
    }
    let path = folder.join("config.toml");
    match fs::read_to_string(&path) {
        Ok(text) => {
            let found = repository_config_problems(&text)
                .with_context(|| format!("couldn't parse {}", path.display()))?;
            problems.extend(
                found
                    .into_iter()
                    .map(|problem| format!("{} {problem}", path.display())),
            );
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("couldn't read {}", path.display()));
        }
    }
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(|home| PathBuf::from(home).join(".cargo"))
        });
    let resolved_root = fs::canonicalize(root).ok();
    let folders = folders_up(root)
        .into_iter()
        .filter(|folder| folder != root && Some(folder) != resolved_root.as_ref())
        .map(|folder| folder.join(".cargo"))
        .chain(cargo_home);
    for folder in folders {
        for name in ["config.toml", "config"] {
            let path = folder.join(name);
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            for alias in
                aliases(&text).with_context(|| format!("couldn't parse {}", path.display()))?
            {
                if ALIASABLE.contains(&alias.as_str()) {
                    problems.push(format!(
                        "{} defines the cargo alias `{alias}`, which could stand in for a command the checks run",
                        path.display()
                    ));
                }
            }
        }
    }
    if problems.is_empty() {
        return Ok(());
    }
    bail!("{}", problems.join("; "))
}

/// What the repository's cargo config sets beyond the `xtask` alias.
fn repository_config_problems(config: &str) -> Result<Vec<String>> {
    let config: toml::Table = config.parse()?;
    let mut problems = Vec::new();
    for (key, value) in &config {
        match (key.as_str(), value.as_table()) {
            ("alias", Some(aliases)) => {
                for alias in aliases.keys().filter(|alias| *alias != "xtask") {
                    problems.push(format!(
                        "defines the cargo alias `{alias}`, which could stand in for a command the checks run"
                    ));
                }
            }
            _ => problems.push(format!(
                "sets `{key}`, but the repository's cargo config may hold only the `xtask` alias"
            )),
        }
    }
    Ok(problems)
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

/// The folder and each folder above it, as spelled and then with links
/// resolved, since cargo and cargo-deny may see the path either way; each
/// real folder once.
pub fn folders_up(folder: &Path) -> Vec<PathBuf> {
    let resolved = fs::canonicalize(folder).ok();
    let mut seen = BTreeSet::new();
    folder
        .ancestors()
        .chain(resolved.iter().flat_map(|path| path.ancestors()))
        .filter(|folder| {
            seen.insert(fs::canonicalize(folder).unwrap_or_else(|_| folder.to_path_buf()))
        })
        .map(Path::to_path_buf)
        .collect()
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
    use crate::scratch::Scratch;
    use std::ffi::OsStr;

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

    /// A `cargo xtask check` step after the first in its job runs even when
    /// an earlier step failed, so that one failure never hides another.
    #[test]
    fn ci_runs_each_check_step_after_a_failure() {
        let workflow = include_str!("../../.github/workflows/ci.yml");
        for steps in steps_by_job(workflow) {
            let checks = steps.iter().filter(|step| {
                step.iter()
                    .any(|line| line.trim().starts_with("run: cargo xtask check "))
            });
            for step in checks.skip(1) {
                assert!(
                    step.iter()
                        .any(|line| line.trim() == "if: ${{ !cancelled() }}"),
                    "this step runs only when the steps before it passed:\n{}",
                    step.join("\n")
                );
            }
        }
    }

    /// The steps of each job in a workflow, each step as its lines.
    fn steps_by_job(workflow: &str) -> Vec<Vec<Vec<&str>>> {
        let mut jobs: Vec<Vec<Vec<&str>>> = Vec::new();
        for line in workflow.lines() {
            let indent = line.len() - line.trim_start().len();
            if indent == 2 && !line.trim().is_empty() {
                jobs.push(Vec::new());
            } else if line.starts_with("      - ") {
                if let Some(job) = jobs.last_mut() {
                    job.push(vec![line]);
                }
            } else if indent >= 8
                && let Some(step) = jobs.last_mut().and_then(|job| job.last_mut())
            {
                step.push(line);
            }
        }
        jobs
    }

    #[test]
    fn denies_rustdoc_warnings() {
        assert!(
            docs().get_envs().any(|(key, value)| key == "RUSTDOCFLAGS"
                && value == Some(OsStr::new("--deny warnings")))
        );
    }

    #[test]
    fn holds_zizmor_to_the_token_it_gets() -> Result<()> {
        let removed = |command: &Command| -> BTreeSet<String> {
            command
                .get_envs()
                .filter(|(_, value)| value.is_none())
                .map(|(key, _)| key.to_string_lossy().into_owned())
                .collect()
        };
        let offline_flag = |command: &Command| command.get_args().any(|arg| arg == "--offline");
        let online = zizmor(Some("read-only".to_owned()), true)?;
        assert_eq!(
            removed(&online),
            BTreeSet::from(ZIZMOR_SETTINGS.map(String::from))
        );
        assert!(
            online
                .get_envs()
                .any(|(key, value)| key == "GH_TOKEN" && value == Some(OsStr::new("read-only")))
        );
        assert!(!offline_flag(&online));
        let offline = zizmor(None, false)?;
        assert!(offline_flag(&offline));
        assert!(removed(&offline).contains("GH_TOKEN"));
        assert!(zizmor(None, true).is_err());
        Ok(())
    }

    #[test]
    fn reads_cargo_aliases() -> Result<()> {
        let config = "[alias]\nxtask = \"run --package xtask --\"\ndeny = \"help\"\n";
        assert_eq!(aliases(config)?, ["deny", "xtask"]);
        assert!(aliases("[build]\njobs = 4\n")?.is_empty());
        Ok(())
    }

    #[test]
    fn allows_only_the_xtask_alias_in_the_repository() -> Result<()> {
        assert_eq!(
            repository_config_problems(include_str!("../../.cargo/config.toml"))?,
            Vec::<String>::new()
        );
        let config = r#"include = "more.toml"

[alias]
xtask = "run --package xtask --locked --"
fmt = "clippy --version"

[build]
rustflags = ["--cap-lints=allow"]

[env]
CARGO_ALIAS_CLIPPY = "test --no-run"
PATH = "stubs"
"#;
        assert_eq!(
            repository_config_problems(config)?,
            [
                "defines the cargo alias `fmt`, which could stand in for a command the checks run",
                "sets `build`, but the repository's cargo config may hold only the `xtask` alias",
                "sets `env`, but the repository's cargo config may hold only the `xtask` alias",
                "sets `include`, but the repository's cargo config may hold only the `xtask` alias",
            ]
        );
        Ok(())
    }

    #[test]
    fn refuses_a_legacy_cargo_config() -> Result<()> {
        let scratch = Scratch::new()?;
        let folder = scratch.path().join(".cargo");
        fs::create_dir(&folder)?;
        fs::write(
            folder.join("config"),
            "[build]\nrustflags = [\"--cap-lints=allow\"]\n",
        )?;
        let Err(error) = check_cargo_config(scratch.path()) else {
            anyhow::bail!("a legacy .cargo/config passed");
        };
        assert!(
            format!("{error:#}").contains("cargo reads it too"),
            "{error:#}"
        );
        Ok(())
    }
}
