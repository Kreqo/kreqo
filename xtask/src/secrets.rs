// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The secret scan (SEC-03), with no escape hatches, as Dara decided:
//! Betterleaks' built-in rules only, whatever config the scanned tree holds,
//! without their default path prefilter; inline `betterleaks:allow`
//! comments ignored; and no scanner config or ignore file allowed in the
//! tree until a revoked secret first needs one. The history scan covers the
//! current branch twice, once with binary files read as text and once with
//! archives unpacked, merge commits included and renames off, and then its
//! messages. The staged scan also reads staged binary files whole, with
//! their paths, since git shows them no diff. Findings name their file and
//! line, or the commit, note or tag whose message holds the secret, with
//! the secret masked.

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail, ensure};

use crate::check::run_command;
use crate::scratch::Scratch;
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

/// What a history scan covers.
#[derive(Clone, Copy)]
enum Scope {
    /// The current branch, and the tags at its tip. A tag elsewhere was
    /// scanned when it was pushed, and the daily scan reads it; since CI-01
    /// keeps release tags, one with a secret in its message would otherwise
    /// fail every later scan for good.
    Head,
    /// Every branch and every tag.
    AllRefs,
}

impl Scope {
    /// The revisions git's commands take.
    fn revisions(self) -> &'static str {
        match self {
            Self::Head => "HEAD",
            Self::AllRefs => "--all",
        }
    }
}

/// Scans the current branch's history, as `cargo xtask check secrets` and
/// CI do.
pub fn scan_history(root: &Path) -> Result<()> {
    tools::require(root, &tools::BETTERLEAKS)?;
    check_tree(root)?;
    scan_revisions(root, Scope::Head)
}

/// Scans every branch's and tag's history, as the daily scan does, so that a
/// ref no push run scanned is still covered.
pub fn scan_all_refs(root: &Path) -> Result<()> {
    tools::require(root, &tools::BETTERLEAKS)?;
    check_tree(root)?;
    scan_revisions(root, Scope::AllRefs)
}

/// Scans a commit message before the commit is made, as the commit-msg hook
/// does: once committed, it can't leave history (CI-01). It checks the tree
/// too, since a merge runs this hook but not the pre-commit hook.
pub fn scan_message(root: &Path, file: &Path) -> Result<()> {
    tools::require(root, &tools::BETTERLEAKS)?;
    check_tree(root)?;
    let path = root.join(file);
    let message = fs::read(&path).with_context(|| format!("couldn't read {}", path.display()))?;
    scan_bytes(root, "commit message", &message)
}

/// Scans the history the scope covers: the changes twice, once reading
/// binary files as text and once without, since `--text` keeps Betterleaks
/// from unpacking archives; then the messages.
fn scan_revisions(root: &Path, scope: Scope) -> Result<()> {
    let revisions = scope.revisions();
    let log_options = format!("--no-renames --diff-merges=first-parent {revisions}");
    all(vec![
        run_command(
            root,
            betterleaks()
                .args(["git", "."])
                .args(FLAGS)
                .arg(format!("--log-opts=--text {log_options}")),
        ),
        run_command(
            root,
            betterleaks()
                .args(["git", "."])
                .args(FLAGS)
                .arg(format!("--log-opts={log_options}")),
        ),
        scan_messages(root, scope),
    ])
}

/// Scans the messages of the scope's commits, their notes, and its tags'
/// messages, each as a file named after its object, such as `commit 1f3c…`,
/// so that a finding names the commit, note or tag, and keeps its
/// fingerprint as history grows.
fn scan_messages(root: &Path, scope: Scope) -> Result<()> {
    let commits: BTreeSet<String> = git_output(root, &["rev-list", scope.revisions()])
        .map(|listed| lines(&listed))?
        .into_iter()
        .collect();
    let mut files: Vec<(String, String)> = commits
        .iter()
        .map(|id| (format!("commit {id}"), id.clone()))
        .collect();
    // `git notes list` pairs each note's blob with the object it annotates.
    for pair in lines(&git_output(root, &["notes", "list"])?) {
        if let Some((note, annotated)) = pair.split_once(' ')
            && commits.contains(annotated)
        {
            files.push((format!("note {annotated}"), note.to_owned()));
        }
    }
    let mut tag_listing = vec![
        "for-each-ref",
        "--format=%(objecttype) %(objectname) %(*objecttype) %(*objectname)",
    ];
    if let Scope::Head = scope {
        tag_listing.extend(["--points-at", "HEAD"]);
    }
    tag_listing.push("refs/tags");
    for tag in lines(&git_output(root, &tag_listing)?) {
        let fields: Vec<&str> = tag.split_whitespace().collect();
        if let ["tag", id, ..] = fields.as_slice() {
            files.push((format!("tag {id}"), (*id).to_owned()));
        }
        // A tag can hold a blob or a tree instead of a commit, which no
        // history scan reads.
        match fields.as_slice() {
            ["blob", id] | ["tag", _, "blob", id] => {
                files.push((format!("blob {id}"), (*id).to_owned()));
            }
            ["tree", id] | ["tag", _, "tree", id] => {
                for blob in blobs_in_tree(root, id)? {
                    files.push((format!("blob {blob}"), blob));
                }
            }
            _ => {}
        }
    }
    let ids: Vec<&str> = files.iter().map(|(_, id)| id.as_str()).collect();
    let contents = read_objects(root, &ids)?;
    let scratch = Scratch::new()?;
    for ((name, _), (kind, content)) in files.iter().zip(&contents) {
        // A commit or tag object's message follows its headers and a blank
        // line; a note is all message.
        let message = match kind.as_str() {
            "blob" => content.as_slice(),
            _ => content
                .windows(2)
                .position(|pair| pair == b"\n\n")
                .and_then(|end| content.get(end + 2..))
                .unwrap_or_default(),
        };
        let path = scratch.path().join(name);
        fs::write(&path, message).with_context(|| format!("couldn't write {}", path.display()))?;
    }
    run_command(scratch.path(), betterleaks().args(["dir", "."]).args(FLAGS))
        .context("the scan of messages failed; each finding's path names its commit, note or tag")
}

/// The blobs in a tree, nested trees included.
fn blobs_in_tree(root: &Path, tree: &str) -> Result<Vec<String>> {
    let listing = git_output(root, &["ls-tree", "-r", "-z", tree])?;
    // Each entry is `<mode> <type> <id>`, a tab and its path.
    Ok(String::from_utf8_lossy(&listing)
        .split('\0')
        .filter_map(|entry| {
            let (object, _) = entry.split_once('\t')?;
            match object.split(' ').collect::<Vec<_>>()[..] {
                [_, "blob", id] => Some(id.to_owned()),
                _ => None,
            }
        })
        .collect())
}

/// The lines of git's output.
fn lines(output: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(output)
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The type and content of each object, in order, read by one
/// `git cat-file --batch`.
fn read_objects(root: &Path, ids: &[&str]) -> Result<Vec<(String, Vec<u8>)>> {
    let mut child = Command::new("git")
        .args(["cat-file", "--batch"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("couldn't run git cat-file")?;
    let mut stdin = child.stdin.take().context("couldn't write to git")?;
    let request: String = ids.iter().map(|id| format!("{id}\n")).collect();
    // Written from another thread, so that git's output never fills its pipe
    // while this one waits to write.
    let writer = std::thread::spawn(move || stdin.write_all(request.as_bytes()));
    let output = child
        .wait_with_output()
        .context("couldn't run git cat-file")?;
    ensure!(
        output.status.success(),
        "git cat-file failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    writer
        .join()
        .map_err(|_| anyhow::anyhow!("couldn't write to git cat-file"))?
        .context("couldn't write to git cat-file")?;
    let mut rest = output.stdout.as_slice();
    let mut objects = Vec::new();
    for id in ids {
        // Each object comes as `<id> <type> <size>`, a newline, its content
        // and a newline.
        let end = rest
            .iter()
            .position(|&byte| byte == b'\n')
            .context("git cat-file stopped early")?;
        let header = String::from_utf8_lossy(rest.get(..end).unwrap_or_default()).into_owned();
        let mut fields = header.split(' ');
        let (Some(_), Some(kind), Some(size)) = (fields.next(), fields.next(), fields.next())
        else {
            bail!("git cat-file couldn't read {id}: {header}");
        };
        let size: usize = size
            .parse()
            .with_context(|| format!("git cat-file printed a bad size for {id}"))?;
        let content = rest
            .get(end + 1..end + 1 + size)
            .context("git cat-file stopped early")?;
        objects.push((kind.to_owned(), content.to_vec()));
        rest = rest.get(end + 2 + size..).unwrap_or_default();
    }
    Ok(objects)
}

/// Fails if any part failed, naming every failure.
fn all(results: Vec<Result<()>>) -> Result<()> {
    let failures: Vec<String> = results
        .into_iter()
        .filter_map(Result::err)
        .map(|error| format!("{error:#}"))
        .collect();
    if failures.is_empty() {
        Ok(())
    } else {
        bail!("{}", failures.join("; "))
    }
}

/// What a git command prints, or why it failed.
fn git_output(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .context("couldn't run git")?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
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
    let blob = git_output(root, &["cat-file", "blob", &format!(":0:{path}")])?;
    scan_bytes(root, path, &blob)
}

/// Scans content through Betterleaks' stdin, under a path that names it.
fn scan_bytes(root: &Path, path: &str, content: &[u8]) -> Result<()> {
    println!("$ … | betterleaks stdin --set-attr path={path}");
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
        .write_all(content)
        .context("couldn't write to betterleaks")?;
    let status = child.wait().context("couldn't run betterleaks")?;
    if status.success() {
        Ok(())
    } else {
        bail!("betterleaks found a secret in the {path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// actions/checkout fetches only branches and tags, so the daily scan
    /// fetches the notes before it scans every ref.
    #[test]
    fn the_daily_scan_fetches_notes() {
        let workflow = include_str!("../../.github/workflows/daily.yml");
        let line = |text: &str| workflow.lines().position(|line| line.contains(text));
        let fetch = line("\"+refs/notes/*:refs/notes/*\"");
        let scan = line("run: cargo xtask scan-all-refs");
        assert!(
            fetch.is_some() && fetch < scan,
            "daily.yml must fetch the notes before it scans every ref"
        );
    }

    #[test]
    fn reads_binary_paths_from_numstat() {
        let numstat = "-\t-\tkeystore.p12\x001\t0\tREADME.md\x00-\t-\tdocs/a b.png\x00";
        assert_eq!(binary_paths(numstat), ["keystore.p12", "docs/a b.png"]);
        assert!(binary_paths("").is_empty());
    }
}
