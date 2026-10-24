// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The external tools that `cargo xtask check` runs, at the versions CI
//! installs (CI-07). The workflows read these versions from `cargo xtask
//! tools`, and `require` checks that this machine has the same ones.

use std::io;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

/// A tool that `cargo xtask check` runs.
pub struct Tool {
    name: &'static str,
    /// The command that prints the tool's version.
    version_command: &'static [&'static str],
    /// The version CI installs.
    version: &'static str,
    source: Source,
}

/// Where a tool comes from, for the install hint.
enum Source {
    Crates,
    PyPi,
    Release(&'static str),
}

pub const CARGO_DENY: Tool = Tool {
    name: "cargo-deny",
    version_command: &["cargo-deny", "--version"],
    version: "0.20.2",
    source: Source::Crates,
};

pub const ZIZMOR: Tool = Tool {
    name: "zizmor",
    version_command: &["zizmor", "--version"],
    version: "1.31.1",
    source: Source::Crates,
};

pub const REUSE: Tool = Tool {
    name: "reuse",
    version_command: &["reuse", "--version"],
    version: "6.2.0",
    source: Source::PyPi,
};

pub const BETTERLEAKS: Tool = Tool {
    name: "betterleaks",
    version_command: &["betterleaks", "version"],
    version: "1.9.0",
    source: Source::Release("https://github.com/betterleaks/betterleaks/releases"),
};

/// The SHA-256 of Betterleaks' Linux x86-64 archive, which CI checks before
/// running it. The release's checksums file, signed by its release workflow,
/// lists the same digest.
const BETTERLEAKS_LINUX_X64_SHA256: &str =
    "f8b185a39ffcece2a1ca82bf3a4e7435cd81963ffd16b7a9128daf75f35f6de7";

impl Tool {
    fn install_hint(&self) -> String {
        match self.source {
            Source::Crates => format!("cargo install --locked {}@{}", self.name, self.version),
            Source::PyPi => format!("pipx install {}=={}", self.name, self.version),
            Source::Release(url) => format!("download {} {} from {url}", self.name, self.version),
        }
    }
}

/// Fails unless the tool runs here at the version CI installs.
pub fn require(root: &Path, tool: &Tool) -> Result<()> {
    let (program, args) = tool
        .version_command
        .split_first()
        .context("a tool without a version command")?;
    let output = match Command::new(program).args(args).current_dir(root).output() {
        Ok(output) => output,
        Err(error) if error.kind() == io::ErrorKind::NotFound => bail!(
            "{} isn't installed; install it with: {}",
            tool.name,
            tool.install_hint()
        ),
        Err(error) => {
            return Err(error).with_context(|| format!("couldn't run {program}"));
        }
    };
    let printed = String::from_utf8_lossy(&output.stdout);
    if reports_version(&printed, tool.version) {
        return Ok(());
    }
    bail!(
        "CI runs {} {}, but this machine has {}; install it with: {}",
        tool.name,
        tool.version,
        printed
            .lines()
            .next()
            .unwrap_or("an unknown version")
            .trim(),
        tool.install_hint()
    )
}

/// Whether a version command's output names exactly this version.
fn reports_version(printed: &str, version: &str) -> bool {
    printed
        .split(|c: char| c.is_whitespace() || c == ',')
        .any(|word| word.strip_prefix('v').unwrap_or(word) == version)
}

/// Prints the versions CI installs, as `name=value` lines for `$GITHUB_OUTPUT`.
pub fn print_github_outputs() {
    for tool in [&CARGO_DENY, &ZIZMOR] {
        println!("{0}={0}@{1}", tool.name, tool.version);
    }
    println!("reuse={}", REUSE.version);
    println!("betterleaks={}", BETTERLEAKS.version);
    println!("betterleaks-sha256={BETTERLEAKS_LINUX_X64_SHA256}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_exact_versions_only() {
        assert!(reports_version("cargo-deny 0.20.2\n", "0.20.2"));
        assert!(reports_version(
            "reuse, version 6.2.0\n\nThis program",
            "6.2.0"
        ));
        assert!(reports_version("v1.9.0\n", "1.9.0"));
        assert!(!reports_version("zizmor 1.31.10\n", "1.31.1"));
        assert!(!reports_version("", "1.9.0"));
    }

    #[test]
    fn hints_install_the_pinned_versions() {
        assert_eq!(
            CARGO_DENY.install_hint(),
            "cargo install --locked cargo-deny@0.20.2"
        );
        assert_eq!(REUSE.install_hint(), "pipx install reuse==6.2.0");
    }
}
