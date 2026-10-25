// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The secret scan (SEC-03): the pre-commit and commit-msg hooks, and the
//! history scan that CI runs.

use std::process::Command;

use anyhow::{Context, Result, ensure};

use crate::support::{Repo, fake_token};

#[test]
#[ignore = "needs Betterleaks; cargo xtask check tool-tests runs it"]
fn the_hook_finds_a_token_in_a_staged_file() -> Result<()> {
    let repo = Repo::new()?;
    repo.xtask(&["pre-commit"]).passes()?;
    repo.write("settings.txt", format!("token = {}\n", fake_token()))?;
    repo.git(&["add", "settings.txt"])?;
    repo.xtask(&["pre-commit"]).fails_with("github-pat")
}

/// Git shows no diff for a file with NUL bytes, so the hook reads it whole.
#[test]
#[ignore = "needs Betterleaks; cargo xtask check tool-tests runs it"]
fn the_hook_finds_a_token_in_a_staged_binary_file() -> Result<()> {
    let repo = Repo::new()?;
    let mut content = b"\0\0header\0".to_vec();
    content.extend(fake_token().as_bytes());
    content.push(0);
    repo.write("data.bin", content)?;
    repo.git(&["add", "data.bin"])?;
    repo.xtask(&["pre-commit"])
        .fails_with("secrets in staged binary files: data.bin")
}

/// Nor for a path that `.gitattributes` marks `-diff`.
#[test]
#[ignore = "needs Betterleaks; cargo xtask check tool-tests runs it"]
fn the_hook_finds_a_token_in_a_file_marked_no_diff() -> Result<()> {
    let repo = Repo::new()?;
    repo.write(".gitattributes", "notes.txt -diff\n")?;
    repo.write("notes.txt", format!("token = {}\n", fake_token()))?;
    repo.git(&["add", ".gitattributes", "notes.txt"])?;
    repo.xtask(&["pre-commit"])
        .fails_with("secrets in staged binary files: notes.txt")
}

/// Rules about paths fire on a staged binary file too.
#[test]
#[ignore = "needs Betterleaks; cargo xtask check tool-tests runs it"]
fn the_hook_finds_a_staged_keystore() -> Result<()> {
    let repo = Repo::new()?;
    repo.write("release.p12", b"\x30\x82\x01\x00\0keystore\0")?;
    repo.git(&["add", "release.p12"])?;
    repo.xtask(&["pre-commit"]).fails_with("release.p12")
}

#[test]
#[ignore = "needs Betterleaks; cargo xtask check tool-tests runs it"]
fn the_hook_ignores_inline_allow_comments() -> Result<()> {
    let repo = Repo::new()?;
    repo.write(
        "settings.txt",
        format!(
            "token = {} # betterleaks:allow gitleaks:allow\n",
            fake_token()
        ),
    )?;
    repo.git(&["add", "settings.txt"])?;
    repo.xtask(&["pre-commit"]).fails_with("github-pat")
}

#[test]
#[ignore = "needs Betterleaks; cargo xtask check tool-tests runs it"]
fn a_scanner_config_in_the_tree_fails_the_scans() -> Result<()> {
    let repo = Repo::new()?;
    repo.write("tools/.gitleaks.toml", "[extend]\nuseDefault = false\n")?;
    let reason = "could change what the secret scan reports";
    repo.xtask(&["pre-commit"]).fails_with(reason)?;
    repo.xtask(&["check", "secrets"]).fails_with(reason)
}

/// With merges shown against their first parent, a token that only the
/// merge brings in is found.
#[test]
#[ignore = "needs Betterleaks; cargo xtask check tool-tests runs it"]
fn the_history_scan_reads_merge_commits() -> Result<()> {
    let repo = Repo::new()?;
    repo.write("a.txt", "base\n")?;
    repo.commit_all("Add a")?;
    repo.git(&["switch", "--quiet", "--create", "side"])?;
    repo.write("a.txt", "side\n")?;
    repo.commit_all("Change a on the side")?;
    repo.git(&["switch", "--quiet", "main"])?;
    repo.write("a.txt", "main\n")?;
    repo.commit_all("Change a on main")?;
    // The merge conflicts, and its resolution brings in the token.
    ensure!(
        repo.git(&["merge", "--quiet", "side"]).is_err(),
        "the merge didn't conflict"
    );
    repo.write("a.txt", format!("token = {}\n", fake_token()))?;
    repo.commit_all("Merge side")?;
    repo.xtask(&["check", "secrets"]).fails_with("github-pat")
}

/// A compressed archive only shows the token once it's unpacked.
#[test]
#[ignore = "needs Betterleaks; cargo xtask check tool-tests runs it"]
fn the_history_scan_unpacks_archives() -> Result<()> {
    let repo = Repo::new()?;
    let packed = repo.outside("packed");
    std::fs::create_dir(&packed)?;
    std::fs::write(
        packed.join("settings.txt"),
        format!("token = {}\n", fake_token()),
    )?;
    let archive = repo.root().join("bundle.tar.gz");
    let status = Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&packed)
        .arg("settings.txt")
        .status()
        .context("couldn't run tar")?;
    ensure!(status.success(), "tar failed");
    let bytes = std::fs::read(&archive)?;
    ensure!(
        !bytes.windows(4).any(|window| window == b"ghp_".as_slice()),
        "the archive holds the token uncompressed"
    );
    repo.commit_all("Add a bundle")?;
    repo.xtask(&["check", "secrets"])
        .fails_with("bundle.tar.gz")
}

/// Betterleaks' default path prefilter skipped `.svg` files and vendored
/// folders; it's off, and rename detection is off, so a moved file is read
/// again.
#[test]
#[ignore = "needs Betterleaks; cargo xtask check tool-tests runs it"]
fn the_history_scan_reads_every_path() -> Result<()> {
    let token = fake_token();
    let repo = Repo::new()?;
    repo.write(
        "logo.svg",
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" data-token=\"{token}\"/>\n"),
    )?;
    repo.commit_all("Add a logo")?;
    repo.xtask(&["check", "secrets"]).fails_with("logo.svg")?;

    let repo = Repo::new()?;
    repo.write(
        "node_modules/pkg/index.js",
        format!("const token = \"{token}\";\n"),
    )?;
    repo.commit_all("Vendor a package")?;
    repo.git(&["mv", "node_modules/pkg/index.js", "index.js"])?;
    repo.commit_all("Move the package's code")?;
    repo.xtask(&["check", "secrets"]).fails_with("index.js")
}

#[test]
#[ignore = "needs Betterleaks; cargo xtask check tool-tests runs it"]
fn the_scans_read_commit_messages() -> Result<()> {
    let repo = Repo::new()?;
    let message = format!("Add the settings\n\ntoken = {}\n", fake_token());
    let file = repo.write_outside("COMMIT_EDITMSG", &message)?;
    let file = file.to_str().context("the scratch path isn't UTF-8")?;
    repo.xtask(&["commit-msg", file]).fails_with("github-pat")?;
    repo.commit_all(&message)?;
    repo.xtask(&["check", "secrets"]).fails_with("github-pat")
}

/// Each message is scanned as a file named after its object, so that a
/// finding names the commit, note or tag, and keeps its fingerprint as
/// history grows.
#[test]
#[ignore = "needs Betterleaks; cargo xtask check tool-tests runs it"]
fn a_finding_in_a_message_names_its_object() -> Result<()> {
    let token = fake_token();
    let repo = Repo::new()?;
    let commit = repo.commit_all(&format!("Add the settings\n\ntoken = {token}\n"))?;
    repo.commit_all("Add nothing")?;
    repo.xtask(&["check", "secrets"])
        .fails_with(&format!("commit {commit}"))?;

    let repo = Repo::new()?;
    let head = repo.git(&["rev-parse", "HEAD"])?;
    repo.git(&[
        "notes",
        "add",
        "--message",
        &format!("token = {token}"),
        "HEAD",
    ])?;
    repo.xtask(&["check", "secrets"])
        .fails_with(&format!("note {}", head.trim()))?;

    let repo = Repo::new()?;
    repo.git(&[
        "tag",
        "--annotate",
        "v1",
        "--message",
        &format!("token = {token}"),
    ])?;
    let tag = repo.git(&["rev-parse", "v1"])?;
    repo.xtask(&["check", "secrets"])
        .fails_with(&format!("tag {}", tag.trim()))
}

/// A tag elsewhere was scanned when it was pushed, and the daily scan
/// reads it; since CI-01 keeps release tags, one with a token in its
/// message would otherwise fail every pull request for good.
#[test]
#[ignore = "needs Betterleaks; cargo xtask check tool-tests runs it"]
fn a_scan_of_head_reads_only_the_tags_at_head() -> Result<()> {
    let token = fake_token();
    let repo = Repo::new()?;
    repo.git(&["switch", "--quiet", "--create", "side"])?;
    repo.commit_all("Work on the side")?;
    repo.git(&[
        "tag",
        "--annotate",
        "annotated",
        "--message",
        &format!("token = {token}"),
    ])?;
    repo.commit_all(&format!("More work\n\ntoken = {token}\n"))?;
    repo.git(&["tag", "lightweight"])?;
    repo.git(&["switch", "--quiet", "main"])?;
    repo.xtask(&["check", "secrets"]).passes()?;
    repo.xtask(&["scan-all-refs"]).fails_with("github-pat")?;
    repo.git(&[
        "tag",
        "--annotate",
        "at-head",
        "--message",
        &format!("token = {token}"),
    ])?;
    repo.xtask(&["check", "secrets"]).fails_with("github-pat")
}

/// The daily scan reads notes on any commit, and what a tag holds when it
/// holds a blob or a tree instead of a commit.
#[test]
#[ignore = "needs Betterleaks; cargo xtask check tool-tests runs it"]
fn the_daily_scan_reads_notes_and_what_tags_hold() -> Result<()> {
    let token = fake_token();
    let repo = Repo::new()?;
    repo.git(&["switch", "--quiet", "--create", "side"])?;
    let side = repo.commit_all("Work on the side")?;
    repo.git(&[
        "notes",
        "add",
        "--message",
        &format!("token = {token}"),
        &side,
    ])?;
    repo.git(&["switch", "--quiet", "main"])?;
    repo.xtask(&["check", "secrets"]).passes()?;
    repo.xtask(&["scan-all-refs"])
        .fails_with(&format!("note {side}"))?;

    let repo = Repo::new()?;
    let file = repo.write_outside("settings.txt", format!("token = {token}\n"))?;
    let file = file.to_str().context("the scratch path isn't UTF-8")?;
    let blob = repo.git(&["hash-object", "-w", file])?;
    let blob = blob.trim();
    repo.git(&["tag", "key", blob])?;
    repo.xtask(&["check", "secrets"]).passes()?;
    repo.xtask(&["scan-all-refs"])
        .fails_with(&format!("blob {blob}"))?;

    let repo = Repo::new()?;
    repo.write("snapshot/settings.txt", format!("token = {token}\n"))?;
    repo.git(&["add", "snapshot"])?;
    let tree = repo.git(&["write-tree"])?;
    repo.git(&["rm", "--quiet", "--cached", "-r", "snapshot"])?;
    std::fs::remove_dir_all(repo.root().join("snapshot"))?;
    repo.git(&[
        "tag",
        "--annotate",
        "snapshot",
        "--message",
        "A snapshot",
        tree.trim(),
    ])?;
    repo.xtask(&["check", "secrets"]).passes()?;
    repo.xtask(&["scan-all-refs"]).fails_with("github-pat")
}

/// A merge runs the commit-msg hook but not the pre-commit hook, so the
/// message hook checks the tree too.
#[test]
#[ignore = "needs Betterleaks; cargo xtask check tool-tests runs it"]
fn the_message_hook_refuses_a_scanner_file_in_the_tree() -> Result<()> {
    let repo = Repo::new()?;
    repo.write(".gitignore", ".betterleaksignore\n")?;
    repo.write(".betterleaksignore", "")?;
    let file = repo.write_outside("MERGE_MSG", "Merge branch 'side'\n")?;
    let file = file.to_str().context("the scratch path isn't UTF-8")?;
    repo.xtask(&["commit-msg", file])
        .fails_with("could change what the secret scan reports")
}

/// `git commit --verbose` puts the staged diff below git's cut line, which
/// git drops, so the commit that removes a leaked token passes.
#[test]
#[ignore = "needs Betterleaks; cargo xtask check tool-tests runs it"]
fn the_message_hook_reads_only_above_the_cut_line() -> Result<()> {
    let token = fake_token();
    let repo = Repo::new()?;
    let diff = format!(
        "# ------------------------ >8 ------------------------\n# Do not modify or remove the line above.\n# Everything below it will be ignored.\ndiff --git a/settings.txt b/settings.txt\n-token = {token}\n+token = revoked\n"
    );
    let file = repo.write_outside(
        "COMMIT_EDITMSG",
        format!("Remove the revoked token\n\n{diff}"),
    )?;
    let file = file.to_str().context("the scratch path isn't UTF-8")?;
    repo.xtask(&["commit-msg", file]).passes()?;
    repo.write_outside("COMMIT_EDITMSG", format!("Add {token}\n\n{diff}"))?;
    repo.xtask(&["commit-msg", file]).fails_with("github-pat")
}
