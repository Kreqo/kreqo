// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Kreqo's project tasks, run as `cargo xtask` (ARCH-06).

mod attributes;
mod check;
mod crates;
mod deny;
mod git;
mod hooks;
#[cfg(test)]
mod scratch;
mod secrets;
mod tools;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Kreqo's project tasks (ARCH-06)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Runs what CI runs: every step, or only the ones named.
    Check {
        /// The steps to run, in order; all of them when none is named.
        #[arg(value_enum)]
        steps: Vec<check::Step>,
    },
    /// Installs the git hooks (SEC-03).
    Hooks,
    /// Scans the staged changes for secrets; the pre-commit hook runs it (SEC-03).
    PreCommit,
    /// Scans a commit message for secrets; the commit-msg hook runs it (SEC-03).
    CommitMsg {
        /// The file that holds the message.
        file: PathBuf,
    },
    /// Scans every branch's and tag's history for secrets; the daily scan runs it (SEC-03).
    ScanAllRefs,
    /// Prints the pinned tool versions as GitHub Actions outputs (CI-07).
    Tools,
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

/// Runs the command. `Ok(false)` means a check failed and has said why.
fn run() -> Result<bool> {
    let cli = Cli::parse();
    let root = workspace_root()?;
    match cli.command {
        Command::Check { steps } => check::run(&root, &steps),
        Command::Hooks => hooks::install(&root).map(|()| true),
        Command::PreCommit => hooks::pre_commit(&root).map(|()| true),
        Command::CommitMsg { file } => hooks::commit_msg(&root, &file).map(|()| true),
        Command::ScanAllRefs => secrets::scan_all_refs(&root).map(|()| true),
        Command::Tools => {
            tools::print_github_outputs();
            Ok(true)
        }
    }
}

/// The workspace root, which holds this crate's folder. `cargo run` sets
/// CARGO_MANIFEST_DIR when it runs xtask; the value compiled in is only a
/// fallback, because a build shared between checkouts would otherwise check
/// the checkout it was compiled in.
fn workspace_root() -> Result<PathBuf> {
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from);
    let root = manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .context("xtask's folder has no parent")?;
    ensure!(
        root.join("Cargo.toml").is_file(),
        "{} isn't a workspace root; run cargo xtask from the repository",
        root.display()
    );
    Ok(root)
}
