// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The secret scan (SEC-03), with no escape hatches, as Dara decided:
//! Betterleaks' built-in rules only, whatever config the scanned tree holds,
//! without their default path prefilter; inline `betterleaks:allow`
//! comments ignored; and no scanner config or ignore file allowed in the
//! tree until a revoked secret first needs one. The history scan covers the
//! current branch twice, once with binary files read as text and once with
//! archives unpacked, merge commits included and renames off. The staged
//! scan also reads staged binary files whole, with their paths, since git
//! shows them no diff. Findings name their file and line, with the secret
//! masked.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use crate::check::run_command;
use crate::{git, tools};

/// A config that keeps Betterleaks' built-in rules and turns off their
/// default path prefilter, which skips paths such as `.svg` files, lock
/// files and `node_modules/`. Passed through the environment, it outranks a
/// config file in the scanned tree.
const BUILT_IN_RULES: &str = "prefilter = '''false'''\n\n[extend]\nuseDefault = true\n";

/// Git settings for every scan: rename detection off, so that a file moved
/// after it was committed is read again, and colors off, which would hide
/// the diff from Betterleaks.
const GIT_SETTINGS: [(&str, &str); 2] = [("diff.renames", "false"), ("color.ui", "never")];

/// Files that would change what Betterleaks reports.
const SCANNER_FILES: [&str; 4] = [
    ".betterleaks.toml",
    ".gitleaks.toml",
    ".betterleaksignore",
    ".gitleaksignore",
];

/// Flags for every scan: findings with their place, secrets masked, and
/// inline allow comments ignored.
const FLAGS: [&str; 4] = [
    "--no-banner",
    "--redact",
    "--verbose",
    "--ignore-gitleaks-allow",
];

/// Scans the current branch's history, as `cargo xtask check secrets` and
/// CI do.
pub fn scan_history(root: &Path) -> Result<()> {
    tools::require(root, &tools::BETTERLEAKS)?;
    check_tree(root)?;
    scan_revisions(root, "HEAD")
}

/// Scans every branch's and tag's history, as the daily scan does, so that a
/// ref no push run scanned is still covered.
pub fn scan_all_refs(root: &Path) -> Result<()> {
    tools::require(root, &tools::BETTERLEAKS)?;
    check_tree(root)?;
    scan_revisions(root, "--all")
}

/// Scans the history of the given revisions twice: once reading binary
/// files as text, and once without, since `--text` keeps Betterleaks from
/// unpacking archives.
fn scan_revisions(root: &Path, revisions: &str) -> Result<()> {
    let log_options = format!("--no-renames --diff-merges=first-parent {revisions}");
    let as_text = run_command(
        root,
        betterleaks()
            .args(["git", "."])
            .args(FLAGS)
            .arg(format!("--log-opts=--text {log_options}")),
    );
    let with_archives = run_command(
        root,
        betterleaks()
            .args(["git", "."])
            .args(FLAGS)
            .arg(format!("--log-opts={log_options}")),
    );
    match (as_text, with_archives) {
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(first), Err(second)) => bail!("{first:#}; {second:#}"),
        (Ok(()), Ok(())) => Ok(()),
    }
}

/// Scans the staged changes, as the pre-commit hook does. Both scans run, so
/// that every finding shows up at once.
pub fn scan_staged(root: &Path) -> Result<()> {
    tools::require(root, &tools::BETTERLEAKS)?;
    check_tree(root)?;
    let diff = run_command(
        root,
        betterleaks()
            .args(["git", ".", "--pre-commit", "--staged"])
            .args(FLAGS),
    );
    let mut leaked = Vec::new();
    for path in staged_binary_files(root)? {
        if scan_staged_file(root, &path).is_err() {
            leaked.push(path);
        }
    }
    match (diff, leaked.is_empty()) {
        (diff, true) => diff,
        (Ok(()), false) => bail!("secrets in staged binary files: {}", leaked.join(", ")),
        (Err(error), false) => bail!(
            "{error:#}; secrets in staged binary files: {}",
            leaked.join(", ")
        ),
    }
}

/// Betterleaks, held to its built-in rules and to the git settings above.
fn betterleaks() -> Command {
    let mut command = tools::BETTERLEAKS.command();
    command
        .env("BETTERLEAKS_CONFIG_TOML", BUILT_IN_RULES)
        .env_remove("BETTERLEAKS_CONFIG")
        .env_remove("GITLEAKS_CONFIG")
        .env_remove("GITLEAKS_CONFIG_TOML")
        .env("GIT_CONFIG_COUNT", GIT_SETTINGS.len().to_string());
    for (index, (key, value)) in GIT_SETTINGS.iter().enumerate() {
        command
            .env(format!("GIT_CONFIG_KEY_{index}"), key)
            .env(format!("GIT_CONFIG_VALUE_{index}"), value);
    }
    command
}

/// Fails if the tree holds a scanner config or ignore file, at the root,
/// where Betterleaks reads them even when git ignores them, or anywhere git
/// tracks or would track.
fn check_tree(root: &Path) -> Result<()> {
    let mut found: Vec<String> = SCANNER_FILES
        .iter()
        .filter(|name| root.join(name).exists())
        .map(|name| (*name).to_owned())
        .collect();
    let pathspecs: Vec<String> = SCANNER_FILES
        .iter()
        .map(|name| format!(":(glob)**/{name}"))
        .collect();
    let pathspecs: Vec<&str> = pathspecs.iter().map(String::as_str).collect();
    for path in git::files(root, &pathspecs)? {
        let path = path.display().to_string();
        if !found.contains(&path) {
            found.push(path);
        }
    }
    if found.is_empty() {
        return Ok(());
    }
    bail!(
        "{} could change what the secret scan reports; none is allowed until a revoked secret needs one (SEC-03)",
        found.join(", ")
    )
}

/// The staged files that git counts as binary, which `git diff` shows
/// without content: files with NUL bytes, and paths marked `-diff`.
fn staged_binary_files(root: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args([
            "diff",
            "--cached",
            "--numstat",
            "--no-renames",
            "--diff-filter=d",
            "-z",
        ])
        .current_dir(root)
        .output()
        .context("couldn't run git diff")?;
    if !output.status.success() {
        bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let listed = String::from_utf8(output.stdout).context("git listed a non-UTF-8 path")?;
    Ok(binary_paths(&listed))
}

/// The paths that `git diff --numstat -z` counts as binary.
fn binary_paths(numstat: &str) -> Vec<String> {
    numstat
        .split('\0')
        .filter_map(
            |record| match record.splitn(3, '\t').collect::<Vec<_>>()[..] {
                ["-", "-", path] => Some(path.to_owned()),
                _ => None,
            },
        )
        .collect()
}

/// Scans one staged file's content in full, with its path, so that rules
/// about paths, such as one for keystores, can fire.
fn scan_staged_file(root: &Path, path: &str) -> Result<()> {
    // `:0:` names the staged stage, even for a path that looks like one.
    let blob = Command::new("git")
        .args(["cat-file", "blob", &format!(":0:{path}")])
        .current_dir(root)
        .output()
        .with_context(|| format!("couldn't read the staged {path}"))?;
    if !blob.status.success() {
        bail!("couldn't read the staged {path}");
    }
    println!("$ git cat-file blob :0:{path} | betterleaks stdin --set-attr path={path}");
    let mut child = betterleaks()
        .arg("stdin")
        .args(FLAGS)
        .arg("--set-attr")
        .arg(format!("path={path}"))
        .current_dir(root)
        .stdin(Stdio::piped())
        .spawn()
        .context("couldn't run betterleaks")?;
    child
        .stdin
        .take()
        .context("couldn't write to betterleaks")?
        .write_all(&blob.stdout)
        .context("couldn't write to betterleaks")?;
    let status = child.wait().context("couldn't run betterleaks")?;
    if status.success() {
        Ok(())
    } else {
        bail!("betterleaks found a secret in the staged {path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_binary_paths_from_numstat() {
        let numstat = "-\t-\tkeystore.p12\x001\t0\tREADME.md\x00-\t-\tdocs/a b.png\x00";
        assert_eq!(binary_paths(numstat), ["keystore.p12", "docs/a b.png"]);
        assert!(binary_paths("").is_empty());
    }
}
