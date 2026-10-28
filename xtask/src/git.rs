// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The files that git tracks or would track, for checks that must see what a
//! commit can carry and nothing that git ignores: build output stays out,
//! and so does the local setup material. Tests keep their git in their own
//! scratch checkouts (QA-09).

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// The oldest git the checks support: the secret scan holds git to its
/// settings with GIT_CONFIG_COUNT, which arrived in git 2.31.
const OLDEST: (u32, u32) = (2, 31);

/// Settings in the environment that point git at another repository or
/// change what it reads. Git exports GIT_DIR to the commands it starts in a
/// linked worktree, such as `git rebase -x`'s, `git bisect run`'s, an alias's
/// and a hook's, and GIT_CONFIG_PARAMETERS to those it starts under `git -c`.
pub const REDIRECTS: [&str; 10] = [
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
];

/// Clears the settings that would point git, run by the command or by
/// anything it starts, at a repository other than the one its folder is in.
pub fn isolate(command: &mut Command) -> &mut Command {
    for key in REDIRECTS {
        command.env_remove(key);
    }
    command
}

/// Makes a test's scratch folder a git repository of its own, without the
/// machine's templates. Under `git rebase -x` in a linked worktree, a plain
/// `git init` would reinitialize the worktree's repository instead, and
/// write `core.bare = true` into the config that all its checkouts share.
#[cfg(test)]
pub fn init(folder: &Path) -> Result<()> {
    init_from(Command::new("git"), folder)
}

/// `init`, run as a git command that may carry settings, as a test does
/// when the command that starts it has them.
#[cfg(test)]
fn init_from(mut git: Command, folder: &Path) -> Result<()> {
    let status = isolate(&mut git)
        .args(["init", "--quiet", "--template="])
        .current_dir(folder)
        .status()
        .context("couldn't run git")?;
    anyhow::ensure!(status.success(), "git init failed in {}", folder.display());
    Ok(())
}

/// Fails unless this machine's git is at least the oldest the checks
/// support.
pub fn require_version(root: &Path) -> Result<()> {
    let output = Command::new("git")
        .arg("--version")
        .current_dir(root)
        .output()
        .context("couldn't run git")?;
    let printed = String::from_utf8_lossy(&output.stdout);
    match version(&printed) {
        Some(found) if found >= OLDEST => Ok(()),
        Some((major, minor)) => bail!(
            "the checks need git {}.{} or later, but this machine has {major}.{minor}",
            OLDEST.0,
            OLDEST.1
        ),
        None => bail!("couldn't read git's version from {:?}", printed.trim()),
    }
}

/// The major and minor version in `git --version`'s output, such as
/// `git version 2.39.3 (Apple Git-146)` or `git version 2.47.0.windows.1`.
fn version(printed: &str) -> Option<(u32, u32)> {
    let number = printed
        .trim()
        .strip_prefix("git version ")?
        .split_whitespace()
        .next()?;
    let mut parts = number.split('.');
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

/// The files matching the pathspecs that git tracks or would track, relative
/// to the root, sorted. Files deleted from the working tree are left out,
/// and so is anything else in a file's place, such as a link whose target
/// is gone.
pub fn files(root: &Path, pathspecs: &[&str]) -> Result<Vec<PathBuf>> {
    let mut files = entries(root, pathspecs)?;
    files.retain(|path| root.join(path).is_file());
    Ok(files)
}

/// The paths matching the pathspecs that git tracks or would track and that
/// the working tree still holds, relative to the root, sorted: the files
/// and anything else in a file's place, such as a link whose target is
/// gone, for a check that must fail on it rather than pass it by.
pub fn entries(root: &Path, pathspecs: &[&str]) -> Result<Vec<PathBuf>> {
    require_top_level(root)?;
    let output = Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
        ])
        .args(pathspecs)
        .current_dir(root)
        .output()
        .context("couldn't run git ls-files")?;
    if !output.status.success() {
        bail!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let listed = String::from_utf8(output.stdout).context("git listed a non-UTF-8 path")?;
    let mut entries: Vec<PathBuf> = listed
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .filter(|path| root.join(path).symlink_metadata().is_ok())
        .collect();
    entries.sort();
    entries.dedup();
    Ok(entries)
}

/// Fails unless the root is the top of a git checkout. Outside git, the
/// checks have no file list; inside an enclosing repository that ignores
/// this folder, git would list nothing, and every check would pass. Git
/// says how far up the top is, so no paths need comparing, whatever form a
/// git prints them in.
fn require_top_level(root: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-cdup"])
        .current_dir(root)
        .output()
        .context("couldn't run git")?;
    if !output.status.success() {
        bail!(
            "{} isn't in a git checkout that git can read, so the checks have no file list: {}",
            root.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let up = String::from_utf8_lossy(&output.stdout);
    let up = up.trim_end_matches(['\n', '\r']);
    if !up.is_empty() {
        bail!(
            "{} isn't the top of its git checkout, which is {up} from it, so git's file list can't be trusted",
            root.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;
    use std::collections::BTreeSet;
    use std::fs;

    #[test]
    fn trusts_git_only_at_the_top_of_a_checkout() -> Result<()> {
        let scratch = Scratch::new()?;
        let top = scratch.path().join("repo");
        fs::create_dir_all(top.join("inner"))?;
        init(&top)?;
        require_top_level(&top)?;
        let Err(error) = require_top_level(&top.join("inner")) else {
            bail!("a folder inside a checkout passed as its top");
        };
        assert!(
            format!("{error:#}").contains("isn't the top of its git checkout, which is ../"),
            "{error:#}"
        );
        // git's own reason comes through, such as a safe.directory hint. Its
        // wording varies between versions, but it starts with `fatal:`.
        let broken = scratch.path().join("broken");
        fs::create_dir(&broken)?;
        fs::write(broken.join(".git"), "gitdir: missing\n")?;
        let Err(error) = require_top_level(&broken) else {
            bail!("a broken checkout passed");
        };
        assert!(
            format!("{error:#}").contains("so the checks have no file list: fatal: "),
            "{error:#}"
        );
        Ok(())
    }

    /// A test's scratch repository lands in its folder even when the
    /// environment points git elsewhere, as GIT_DIR does under `git rebase
    /// -x` in a linked worktree.
    #[test]
    fn creates_scratch_repositories_in_their_folders() -> Result<()> {
        let scratch = Scratch::new()?;
        let elsewhere = scratch.path().join("elsewhere");
        let folder = scratch.path().join("repo");
        fs::create_dir(&folder)?;
        let mut git = Command::new("git");
        git.env("GIT_DIR", &elsewhere)
            .env("GIT_WORK_TREE", &elsewhere);
        init_from(git, &folder)?;
        assert!(folder.join(".git").join("HEAD").is_file());
        assert!(!elsewhere.exists(), "git init wrote to GIT_DIR");
        // Without the machine's templates, no exclude file of theirs
        // applies.
        assert!(!folder.join(".git").join("info").exists());
        Ok(())
    }

    /// The settings, named here rather than taken from the constant, so
    /// that dropping one fails this test.
    #[test]
    fn clears_every_setting_that_points_git_elsewhere() {
        let mut command = Command::new("git");
        let removed: BTreeSet<_> = isolate(&mut command)
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(key, _)| key.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            removed,
            BTreeSet::from(
                [
                    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
                    "GIT_CONFIG",
                    "GIT_CONFIG_COUNT",
                    "GIT_CONFIG_PARAMETERS",
                    "GIT_DIR",
                    "GIT_EXTERNAL_DIFF",
                    "GIT_INDEX_FILE",
                    "GIT_NAMESPACE",
                    "GIT_OBJECT_DIRECTORY",
                    "GIT_WORK_TREE",
                ]
                .map(String::from)
            )
        );
    }

    #[test]
    fn reads_git_versions() {
        assert_eq!(version("git version 2.43.0\n"), Some((2, 43)));
        assert_eq!(
            version("git version 2.39.3 (Apple Git-146)\n"),
            Some((2, 39))
        );
        assert_eq!(version("git version 2.47.0.windows.1\n"), Some((2, 47)));
        assert_eq!(version("git help\n"), None);
        assert!((2, 30) < OLDEST && (2, 31) >= OLDEST && (3, 0) >= OLDEST);
    }
}
