// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Throwaway repositories, and xtask run in them, with git's settings held
//! to the test's own rather than the machine's.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::{Context, Result, bail, ensure};

/// Settings in the environment that would point git elsewhere or change
/// what it reads, or turn on CI's behavior in xtask.
const CLEARED: [&str; 15] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_NAMESPACE",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_EXTERNAL_DIFF",
    "CI",
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "ZIZMOR_GITHUB_TOKEN",
    "ZIZMOR_OFFLINE",
];

/// A git repository in a fresh folder, removed when it's dropped.
pub struct Repo {
    /// The folder that holds the repository, the git config that stands in
    /// for the user's, and anything else the test needs outside the
    /// repository.
    scratch: PathBuf,
    root: PathBuf,
}

impl Repo {
    /// A repository on `main` whose first commit holds the Cargo.toml that
    /// makes it a workspace root for xtask.
    pub fn new() -> Result<Self> {
        let scratch = scratch_folder()?;
        let root = scratch.join("repo");
        fs::create_dir(&root).with_context(|| format!("couldn't create {}", root.display()))?;
        let repo = Self { scratch, root };
        repo.write_outside("gitconfig", "")?;
        repo.git(&["init", "--quiet", "--initial-branch=main"])?;
        repo.write("Cargo.toml", "[workspace]\n")?;
        repo.commit_all("Start")?;
        Ok(repo)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The path of a file outside the repository.
    pub fn outside(&self, name: &str) -> PathBuf {
        self.scratch.join(name)
    }

    /// Writes a file outside the repository and returns its path.
    pub fn write_outside(&self, name: &str, content: impl AsRef<[u8]>) -> Result<PathBuf> {
        let path = self.outside(name);
        fs::write(&path, content).with_context(|| format!("couldn't write {}", path.display()))?;
        Ok(path)
    }

    /// Writes a file in the repository, creating its folders.
    pub fn write(&self, path: &str, content: impl AsRef<[u8]>) -> Result<()> {
        let path = self.root.join(path);
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)
                .with_context(|| format!("couldn't create {}", folder.display()))?;
        }
        fs::write(&path, content).with_context(|| format!("couldn't write {}", path.display()))
    }

    /// Runs git in the repository and returns what it printed.
    pub fn git(&self, args: &[&str]) -> Result<String> {
        let output = self
            .command("git")
            .args(args)
            .output()
            .context("couldn't run git")?;
        if !output.status.success() {
            bail!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        String::from_utf8(output.stdout).context("git printed non-UTF-8 text")
    }

    /// Commits everything in the working tree and returns the commit's id.
    pub fn commit_all(&self, message: &str) -> Result<String> {
        self.git(&["add", "--all"])?;
        self.git(&["commit", "--quiet", "--allow-empty", "--message", message])?;
        Ok(self.git(&["rev-parse", "HEAD"])?.trim().to_owned())
    }

    /// Runs xtask on the repository, as `cargo xtask` would from its root.
    pub fn xtask(&self, args: &[&str]) -> Run {
        self.xtask_with(args, &[])
    }

    /// Runs xtask with more environment settings.
    pub fn xtask_with(&self, args: &[&str], settings: &[(&str, &str)]) -> Run {
        let mut command = self.command(env!("CARGO_BIN_EXE_xtask"));
        command
            .args(args)
            // xtask's root is the folder above its manifest's.
            .env("CARGO_MANIFEST_DIR", self.root.join("xtask"))
            .env("CARGO_HOME", self.outside("cargo-home"));
        for (key, value) in settings {
            command.env(key, value);
        }
        Run {
            shown: format!("xtask {}", args.join(" ")),
            output: command.output(),
        }
    }

    /// A command run in the repository with the test's git settings: the
    /// test's own global config, no system config, a fixed identity, and
    /// no repository found above the scratch folder.
    fn command(&self, program: impl AsRef<OsStr>) -> Command {
        let mut command = Command::new(program);
        command
            .current_dir(&self.root)
            .env("GIT_CONFIG_GLOBAL", self.outside("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CEILING_DIRECTORIES", &self.scratch)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.org")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.org");
        for key in CLEARED {
            command.env_remove(key);
        }
        command
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        // A drop can't report a failure, and a folder left in the system's
        // temporary folder does no harm.
        let _ = fs::remove_dir_all(&self.scratch);
    }
}

/// A fresh folder under the system's temporary folder.
fn scratch_folder() -> Result<PathBuf> {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    loop {
        let name = format!(
            "kreqo-xtask-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            // Left behind by an earlier process with the same id.
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error).with_context(|| format!("couldn't create {}", path.display()));
            }
        }
    }
}

/// What a run of xtask did.
pub struct Run {
    shown: String,
    output: io::Result<Output>,
}

impl Run {
    /// Fails unless the run succeeded.
    pub fn passes(&self) -> Result<()> {
        let (success, printed) = self.result()?;
        ensure!(success, "{} failed:\n{printed}", self.shown);
        Ok(())
    }

    /// Fails unless the run failed and printed the text.
    pub fn fails_with(&self, text: &str) -> Result<()> {
        let (success, printed) = self.result()?;
        ensure!(!success, "{} passed:\n{printed}", self.shown);
        ensure!(
            printed.contains(text),
            "{} failed without saying {text:?}:\n{printed}",
            self.shown
        );
        Ok(())
    }

    /// Whether the run succeeded, and what it printed.
    fn result(&self) -> Result<(bool, String)> {
        let output = self
            .output
            .as_ref()
            .map_err(|error| anyhow::anyhow!("couldn't run {}: {error}", self.shown))?;
        let printed = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        Ok((output.status.success(), printed))
    }
}

/// A GitHub token, as Betterleaks' `github-pat` rule finds it, put together
/// at run time so that no file in the repository holds one.
pub fn fake_token() -> String {
    let alphabet: Vec<char> = ('a'..='z').chain('A'..='Z').chain('0'..='9').collect();
    let body: String = (0..36)
        .filter_map(|index| alphabet.get((index * 7 + 3) % alphabet.len()))
        .collect();
    format!("ghp_{body}")
}
