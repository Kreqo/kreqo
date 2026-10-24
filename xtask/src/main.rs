// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Kreqo's project tasks, run as `cargo xtask` (ARCH-06).

mod attributes;
mod check;
mod crates;
mod deny;
mod git;
mod hooks;
mod tools;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
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
        Command::Tools => {
            tools::print_github_outputs();
            Ok(true)
        }
    }
}

/// The workspace root, which holds this crate's folder.
fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("xtask's folder has no parent")
}
