// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Attribute checks over every Rust file in the repository: no `allow`
//! attribute, inner or outer (QA-01); no `warn` or `expect` that lowers
//! `warnings` or a lint the workspace table denies, since a source
//! attribute outranks the table and Clippy's `--deny warnings` (QA-01,
//! SEC-01); and `expect(unsafe_code)` only in a crate root's
//! `#![expect(unsafe_code, reason = "ADR-NNNN")]` that names an existing ADR
//! (SEC-01). Clippy's `allow_attributes` skips inner attributes and any
//! `cfg_attr` whose condition doesn't hold, so these checks read each file
//! as tokens: comments and strings never match, and an attribute that
//! rustfmt wraps over several lines stays whole.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result};
use proc_macro2::{Delimiter, Ident, TokenStream, TokenTree};
use toml::Value;

use crate::git;

/// The lint groups that hold a lint the workspace table denies, as the
/// pinned toolchain's `clippy-driver -W help` lists them, so that an
/// `expect` of a group can't suppress such a lint. `unsafe_code` is in no
/// group but `warnings`, which is checked on its own. A `warn` of a group
/// needs no entry: Clippy's `--deny warnings` still fails the lint.
const GROUPS: [(&str, &[&str]); 4] = [
    ("clippy::allow_attributes", &["clippy::restriction"]),
    (
        "clippy::allow_attributes_without_reason",
        &["clippy::restriction"],
    ),
    (
        "clippy::missing_safety_doc",
        &["clippy::all", "clippy::style"],
    ),
    (
        "clippy::undocumented_unsafe_blocks",
        &["clippy::restriction"],
    ),
];

/// An attribute in a Rust file.
struct Attribute {
    /// Whether it's `#![…]` rather than `#[…]`.
    inner: bool,
    /// Whether it sits at the top of its file rather than inside an item, as
    /// a crate-level attribute must.
    top_level: bool,
    line: usize,
    /// What its brackets hold.
    tokens: TokenStream,
}

/// Checks every Rust file that git tracks or would track, and returns its
/// problems. `crate_roots` are the files rustc starts each crate from.
pub fn check(root: &Path, crate_roots: &BTreeSet<PathBuf>) -> Result<Vec<String>> {
    let crate_roots: BTreeSet<PathBuf> = crate_roots.iter().map(|path| canonical(path)).collect();
    let denied = denied_lints(root)?;
    let mut problems = Vec::new();
    for relative in git::files(root, &[":(glob)**/*.rs"])? {
        let path = root.join(&relative);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                problems.push(format!(
                    "{}: couldn't read it, so its attributes go unchecked: {error}",
                    relative.display()
                ));
                continue;
            }
        };
        let name = relative.display().to_string();
        let source = Source {
            root,
            name: &name,
            is_crate_root: crate_roots.contains(&canonical(&path)),
            denied: &denied,
        };
        problems.extend(source.problems(&text));
    }
    Ok(problems)
}

/// A Rust source file, with what its checks need to know.
struct Source<'a> {
    root: &'a Path,
    /// Its path from the root, as problems name it.
    name: &'a str,
    is_crate_root: bool,
    /// The lints the workspace table denies.
    denied: &'a BTreeSet<String>,
}

impl Source<'_> {
    /// The problems with the attributes in the file's text.
    fn problems(&self, text: &str) -> Vec<String> {
        let stream = match TokenStream::from_str(&without_shebang(text)) {
            Ok(stream) => stream,
            Err(error) => {
                return vec![format!(
                    "{}: couldn't read its tokens, so its attributes go unchecked: {error}",
                    self.name
                )];
            }
        };
        let mut problems = Vec::new();
        for attribute in attributes(stream) {
            let place = format!("{}:{}", self.name, attribute.line);
            let crate_level = attribute.inner && attribute.top_level && self.is_crate_root;
            for (name, arguments) in applied(&attribute.tokens) {
                match name.as_str() {
                    "allow" => problems.push(format!(
                        "{place}: an `allow` attribute; suppress with `#[expect(lint, reason = \"…\")]` (QA-01)"
                    )),
                    "warn" | "expect" => {
                        for lint in lints(&arguments) {
                            if let Some(problem) =
                                self.lowering(&name, &lint, &arguments, crate_level)
                            {
                                problems.push(format!("{place}: {problem}"));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        problems
    }

    /// What's wrong with a `warn` or `expect` of one lint, if anything.
    fn lowering(
        &self,
        name: &str,
        lint: &str,
        arguments: &TokenStream,
        crate_level: bool,
    ) -> Option<String> {
        if lint == "warnings" {
            return Some(format!(
                "`{name}(warnings)` would let every warning pass CI (QA-01)"
            ));
        }
        if name == "expect" && lint == "unsafe_code" {
            if !crate_level {
                return Some(
                    "`expect(unsafe_code)` belongs only in a crate root's `#![expect(unsafe_code, reason = \"ADR-NNNN\")]` (SEC-01)".to_owned(),
                );
            }
            if !adr_number(arguments).is_some_and(|number| adr_exists(self.root, &number)) {
                return Some("its reason names no ADR in docs/adr/ (SEC-01)".to_owned());
            }
            return None;
        }
        if self.denied.contains(lint) {
            return Some(format!(
                "`{name}({lint})` lowers a lint that the workspace table denies (QA-01)"
            ));
        }
        if name != "expect" {
            return None;
        }
        GROUPS
            .iter()
            .find(|(held, groups)| self.denied.contains(*held) && groups.contains(&lint))
            .map(|(held, _)| {
                format!(
                    "`expect({lint})` would suppress `{held}`, which the workspace table denies (QA-01)"
                )
            })
    }
}

/// The lints the root Cargo.toml's `[workspace.lints]` denies or forbids,
/// each as an attribute names it, such as `unsafe_code` or
/// `clippy::missing_safety_doc`.
fn denied_lints(root: &Path) -> Result<BTreeSet<String>> {
    let path = root.join("Cargo.toml");
    let text =
        fs::read_to_string(&path).with_context(|| format!("couldn't read {}", path.display()))?;
    denied_in(&text).with_context(|| format!("couldn't parse {}", path.display()))
}

fn denied_in(manifest: &str) -> Result<BTreeSet<String>> {
    let manifest: toml::Table = manifest.parse()?;
    let tools = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("lints"))
        .and_then(Value::as_table);
    let mut denied = BTreeSet::new();
    for (tool, lints) in tools.into_iter().flatten() {
        for (lint, setting) in lints.as_table().into_iter().flatten() {
            let level = setting
                .as_str()
                .or_else(|| setting.get("level").and_then(Value::as_str));
            if matches!(level, Some("deny" | "forbid")) {
                let lint = lint.replace('-', "_");
                denied.insert(if tool == "rust" {
                    lint
                } else {
                    format!("{tool}::{lint}")
                });
            }
        }
    }
    Ok(denied)
}

/// The lints a lint attribute's arguments name, such as `unsafe_code` or
/// `clippy::missing_safety_doc`, leaving out its `reason = "…"`.
fn lints(arguments: &TokenStream) -> Vec<String> {
    split_commas(arguments.clone())
        .into_iter()
        .filter_map(|part| {
            let tokens: Vec<TokenTree> = part.into_iter().collect();
            if let [TokenTree::Ident(key), TokenTree::Punct(equals), ..] = tokens.as_slice()
                && unraw(key) == "reason"
                && equals.as_char() == '='
            {
                return None;
            }
            let path: String = tokens
                .iter()
                .map(|token| match token {
                    TokenTree::Ident(ident) => unraw(ident),
                    other => other.to_string(),
                })
                .collect();
            (!path.is_empty()).then_some(path)
        })
        .collect()
}

/// The path with `..` and links resolved, so that two names for one file
/// compare equal; the path as it is when it can't be resolved.
fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The text with a leading shebang line blanked and line numbers kept, as
/// rustc's `strip_shebang` reads it: `#!` starts a shebang unless the next
/// token, past whitespace and comments other than doc comments, is `[`.
fn without_shebang(text: &str) -> Cow<'_, str> {
    match text.strip_prefix("#!") {
        Some(rest) if !past_trivia(rest).starts_with('[') => {
            let line_end = text.find('\n').unwrap_or(text.len());
            Cow::Owned(text.get(line_end..).unwrap_or_default().to_owned())
        }
        _ => Cow::Borrowed(text),
    }
}

/// The text past its leading whitespace and comments, as rustc's lexer
/// reads them. A doc comment is a token, so it stops there.
fn past_trivia(mut text: &str) -> &str {
    loop {
        text = text.trim_start_matches(is_rust_whitespace);
        if let Some(rest) = text.strip_prefix("//") {
            // `//!` and `///` are doc comments, but `////` isn't.
            if rest.starts_with('!') || (rest.starts_with('/') && !rest.starts_with("//")) {
                return text;
            }
            text = rest
                .find('\n')
                .and_then(|end| rest.get(end..))
                .unwrap_or_default();
        } else if let Some(rest) = text.strip_prefix("/*") {
            // `/*!` and `/**` are doc comments, but `/***` and `/**/` aren't.
            if rest.starts_with('!')
                || (rest.starts_with('*') && !rest.starts_with("**") && !rest.starts_with("*/"))
            {
                return text;
            }
            text = past_block_comment(rest);
        } else {
            return text;
        }
    }
}

/// The text past the end of a block comment whose `/*` is already read,
/// counting nested comments; nothing when it never ends.
fn past_block_comment(text: &str) -> &str {
    let mut depth = 1_usize;
    let mut chars = text.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        match (c, chars.peek()) {
            ('/', Some((_, '*'))) => {
                chars.next();
                depth += 1;
            }
            ('*', Some((slash, '/'))) => {
                let end = slash + 1;
                chars.next();
                depth -= 1;
                if depth == 0 {
                    return text.get(end..).unwrap_or_default();
                }
            }
            _ => {}
        }
    }
    ""
}

/// Whether rustc's lexer reads the character as whitespace: Unicode's
/// Pattern_White_Space, which includes the bidi marks.
fn is_rust_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n'
            | '\u{B}'
            | '\u{C}'
            | '\r'
            | ' '
            | '\u{85}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{2028}'
            | '\u{2029}'
    )
}

/// Every attribute in a token stream, nested ones included.
fn attributes(stream: TokenStream) -> Vec<Attribute> {
    let mut found = Vec::new();
    collect_attributes(stream, true, &mut found);
    found
}

fn collect_attributes(stream: TokenStream, top_level: bool, found: &mut Vec<Attribute>) {
    let tokens: Vec<TokenTree> = stream.into_iter().collect();
    let mut index = 0;
    while let Some(token) = tokens.get(index) {
        match token {
            TokenTree::Punct(hash) if hash.as_char() == '#' => {
                let inner = matches!(tokens.get(index + 1), Some(TokenTree::Punct(bang)) if bang.as_char() == '!');
                let after = index + 1 + usize::from(inner);
                match tokens.get(after) {
                    Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Bracket => {
                        found.push(Attribute {
                            inner,
                            top_level,
                            line: hash.span().start().line,
                            tokens: group.stream(),
                        });
                        index = after + 1;
                    }
                    _ => index += 1,
                }
            }
            TokenTree::Group(group) => {
                collect_attributes(group.stream(), false, found);
                index += 1;
            }
            _ => index += 1,
        }
    }
}

/// The attributes that an attribute's tokens apply, each as its name and
/// its arguments: the attribute itself when its path is a single
/// identifier, such as `allow(…)`; for `cfg_attr(condition, …)`, each
/// attribute after the condition, however deep; and for
/// `doc(test(attr(…)))`, each attribute it gives every doctest. A longer
/// path, such as rstest's `case::allow(…)`, names something else.
fn applied(tokens: &TokenStream) -> Vec<(String, TokenStream)> {
    let tokens: Vec<TokenTree> = tokens.clone().into_iter().collect();
    let [TokenTree::Ident(name), TokenTree::Group(arguments)] = tokens.as_slice() else {
        return Vec::new();
    };
    if arguments.delimiter() != Delimiter::Parenthesis {
        return Vec::new();
    }
    let name = unraw(name);
    match name.as_str() {
        "cfg_attr" => split_commas(arguments.stream())
            .iter()
            .skip(1)
            .flat_map(applied)
            .collect(),
        "doc" => split_commas(arguments.stream())
            .iter()
            .filter_map(|part| arguments_of(part, "test"))
            .flat_map(split_commas)
            .filter_map(|part| arguments_of(&part, "attr"))
            .flat_map(split_commas)
            .flat_map(|attribute| applied(&attribute))
            .collect(),
        _ => vec![(name, arguments.stream())],
    }
}

/// What the parentheses of `name(…)` hold, if the tokens are that.
fn arguments_of(tokens: &TokenStream, name: &str) -> Option<TokenStream> {
    let tokens: Vec<TokenTree> = tokens.clone().into_iter().collect();
    match tokens.as_slice() {
        [TokenTree::Ident(ident), TokenTree::Group(group)]
            if unraw(ident) == name && group.delimiter() == Delimiter::Parenthesis =>
        {
            Some(group.stream())
        }
        _ => None,
    }
}

/// The comma-separated parts of a token stream.
fn split_commas(stream: TokenStream) -> Vec<TokenStream> {
    let mut parts = vec![TokenStream::new()];
    for token in stream {
        match &token {
            TokenTree::Punct(comma) if comma.as_char() == ',' => parts.push(TokenStream::new()),
            _ => {
                if let Some(part) = parts.last_mut() {
                    part.extend([token]);
                }
            }
        }
    }
    parts
}

/// An identifier's name without a raw identifier's `r#`, since rustc
/// honors `r#allow` as `allow`.
fn unraw(ident: &Ident) -> String {
    let name = ident.to_string();
    match name.strip_prefix("r#") {
        Some(name) => name.to_owned(),
        None => name,
    }
}

/// The four digits of the first `ADR-NNNN` in a `reason = "…"`.
fn adr_number(arguments: &TokenStream) -> Option<String> {
    let tokens: Vec<TokenTree> = arguments.clone().into_iter().collect();
    let reason = tokens.windows(3).find_map(|window| match window {
        [
            TokenTree::Ident(key),
            TokenTree::Punct(equals),
            TokenTree::Literal(value),
        ] if unraw(key) == "reason" && equals.as_char() == '=' => Some(value.to_string()),
        _ => None,
    })?;
    reason.match_indices("ADR-").find_map(|(index, _)| {
        let digits = reason.get(index + 4..index + 8)?;
        digits
            .chars()
            .all(|c| c.is_ascii_digit())
            .then(|| digits.to_owned())
    })
}

/// Whether docs/adr/ holds the ADR with this number (DOC-04).
fn adr_exists(root: &Path, number: &str) -> bool {
    let prefix = format!("{number}-");
    fs::read_dir(root.join("docs").join("adr")).is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with(&prefix) && name.ends_with(".md")
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lines of the attributes in `text` that apply `name(…)`.
    fn lines_using(text: &str, name: &str) -> Result<Vec<usize>> {
        let stream = TokenStream::from_str(text).map_err(|error| anyhow::anyhow!("{error}"))?;
        Ok(attributes(stream)
            .into_iter()
            .filter(|attribute| {
                applied(&attribute.tokens)
                    .iter()
                    .any(|(applied, _)| applied == name)
            })
            .map(|attribute| attribute.line)
            .collect())
    }

    #[test]
    fn finds_allow_attributes_however_written() -> Result<()> {
        let text = r##"//! Docs that mention allow(x) and #[allow(dead_code)].
#![cfg_attr(
    not(target_os = "redox"),
    allow(dead_code, reason = "long enough that rustfmt wraps this attribute")
)]
#[expect(clippy::needless_return, reason = "allow(x) in a string")]
#[doc = "allow(dead_code)"]
fn f() {
    let _ = "#[allow(dead_code)]";
}
mod m {
    #![allow(unused)]
}
#[disallow(dead_code)]
fn g() {}
#[r#allow(dead_code)]
fn h() {}
#[case::allow(1)]
#[cfg_attr(test, cfg_attr(unix, allow(unused)))]
fn i() {}
"##;
        assert_eq!(lines_using(text, "allow")?, [2, 12, 16, 19]);
        Ok(())
    }

    #[test]
    fn finds_allow_attributes_given_to_doctests() -> Result<()> {
        let text = "#![doc(test(no_crate_inject, attr(deny(warnings), allow(unused))))]\n#![doc(html_root_url = \"https://example.org\")]\n#![doc(test(attr(cfg_attr(unix, allow(dead_code)))))]\n";
        assert_eq!(lines_using(text, "allow")?, [1, 3]);
        Ok(())
    }

    #[test]
    fn reads_raw_identifiers() -> Result<()> {
        let stream = TokenStream::from_str("r#expect(r#unsafe_code, r#reason = \"ADR-0001\")")
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        let applied = applied(&stream);
        let [(name, arguments)] = applied.as_slice() else {
            anyhow::bail!("expected one applied attribute, got {}", applied.len());
        };
        assert_eq!(name, "expect");
        assert_eq!(lints(arguments), ["unsafe_code"]);
        assert_eq!(adr_number(arguments), Some("0001".to_owned()));
        Ok(())
    }

    /// The committed workspace table's denied lints.
    fn denied() -> Result<BTreeSet<String>> {
        denied_in(include_str!("../../Cargo.toml"))
    }

    #[test]
    fn flags_lowered_lint_levels() -> Result<()> {
        let denied = denied()?;
        let source = Source {
            root: Path::new("."),
            name: "lib.rs",
            is_crate_root: true,
            denied: &denied,
        };
        let text = r##"#![warn(unsafe_code)]
#![expect(warnings, reason = "quiet")]
#![warn(r#warnings, clippy::undocumented_unsafe_blocks)]
#![expect(clippy::style, reason = "a group that holds a denied lint")]
#![expect(unsafe_code, reason = "ADR-0001, which doesn't exist")]
#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]
#[expect(clippy::needless_return, reason = "kept for clarity")]
fn f() {}
mod m {
    #![expect(unsafe_code, reason = "ADR-0001")]
}
"##;
        assert_eq!(
            source.problems(text),
            [
                "lib.rs:1: `warn(unsafe_code)` lowers a lint that the workspace table denies (QA-01)",
                "lib.rs:2: `expect(warnings)` would let every warning pass CI (QA-01)",
                "lib.rs:3: `warn(warnings)` would let every warning pass CI (QA-01)",
                "lib.rs:3: `warn(clippy::undocumented_unsafe_blocks)` lowers a lint that the workspace table denies (QA-01)",
                "lib.rs:4: `expect(clippy::style)` would suppress `clippy::missing_safety_doc`, which the workspace table denies (QA-01)",
                "lib.rs:5: its reason names no ADR in docs/adr/ (SEC-01)",
                "lib.rs:11: `expect(unsafe_code)` belongs only in a crate root's `#![expect(unsafe_code, reason = \"ADR-NNNN\")]` (SEC-01)",
            ]
        );
        Ok(())
    }

    #[test]
    fn reads_the_denied_lints() -> Result<()> {
        assert_eq!(
            denied()?,
            BTreeSet::from(
                [
                    "unsafe_code",
                    "clippy::allow_attributes",
                    "clippy::allow_attributes_without_reason",
                    "clippy::undocumented_unsafe_blocks",
                    "clippy::missing_safety_doc",
                ]
                .map(String::from)
            )
        );
        let table = "[workspace.lints.rust]\nunsafe-code = { level = \"forbid\", priority = 1 }\nmissing_docs = \"warn\"\n";
        assert_eq!(
            denied_in(table)?,
            BTreeSet::from(["unsafe_code".to_owned()])
        );
        Ok(())
    }

    /// Every lint group that holds a denied lint is in `GROUPS`, as the
    /// pinned toolchain lists them, so that a toolchain update that moves a
    /// lint into a new group fails here first (CI-16).
    #[test]
    fn groups_match_the_toolchain() -> Result<()> {
        let output = std::process::Command::new("clippy-driver")
            .args(["-W", "help"])
            .output()
            .context("couldn't run clippy-driver")?;
        anyhow::ensure!(
            output.status.success(),
            "clippy-driver -W help failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let groups = lint_groups(&String::from_utf8(output.stdout)?);
        anyhow::ensure!(
            groups.iter().any(|(group, _)| group == "clippy::all"),
            "clippy-driver -W help listed no Clippy groups"
        );
        for lint in denied()? {
            let holding: Vec<&str> = groups
                .iter()
                .filter(|(group, members)| group != "warnings" && members.contains(&lint))
                .map(|(group, _)| group.as_str())
                .collect();
            let listed = GROUPS
                .iter()
                .find(|(held, _)| *held == lint)
                .map_or(&[][..], |(_, groups)| *groups);
            assert_eq!(holding, listed, "the groups that hold `{lint}`");
        }
        Ok(())
    }

    /// The lint groups that `-W help` lists, sorted, each with its members,
    /// named as attributes name them.
    fn lint_groups(help: &str) -> Vec<(String, Vec<String>)> {
        let name = |lint: &str| lint.trim().replace('-', "_");
        let mut groups = Vec::new();
        let mut in_groups = false;
        for line in help.lines() {
            if !line.starts_with(' ') && line.ends_with(':') {
                in_groups = line.starts_with("Lint groups");
                continue;
            }
            let Some((group, members)) = line.trim().split_once("  ") else {
                continue;
            };
            if in_groups && group != "name" && !group.starts_with('-') {
                groups.push((name(group), members.split(", ").map(name).collect()));
            }
        }
        groups.sort();
        groups
    }

    #[test]
    fn tells_inner_top_level_attributes_from_the_rest() -> Result<()> {
        let text = "#![expect(unsafe_code)]\n#[expect(unsafe_code)]\nfn f() {}\nmod m {\n    #![expect(unsafe_code)]\n}\n";
        let stream = TokenStream::from_str(text).map_err(|error| anyhow::anyhow!("{error}"))?;
        let kinds: Vec<(bool, bool)> = attributes(stream)
            .iter()
            .map(|attribute| (attribute.inner, attribute.top_level))
            .collect();
        assert_eq!(kinds, [(true, true), (false, true), (true, false)]);
        Ok(())
    }

    #[test]
    fn blanks_a_shebang_but_keeps_inner_attributes() {
        assert_eq!(
            without_shebang("#!/usr/bin/env -S cargo 'x'\n#![allow(x)]\n"),
            "\n#![allow(x)]\n"
        );
        assert_eq!(without_shebang("#![allow(x)]\n"), "#![allow(x)]\n");
        assert_eq!(without_shebang("#! [allow(x)]\n"), "#! [allow(x)]\n");
    }

    #[test]
    fn reads_a_shebang_as_rustc_does() -> Result<()> {
        // Whitespace and comments other than doc comments may come between
        // `#!` and `[`, which keeps it an inner attribute.
        for text in [
            "#!/**/[allow(x)]\n",
            "#!/* x /* nested */ */ [allow(x)]\n",
            "#!// x\n[allow(x)]\n",
            "#!//// x\n[allow(x)]\n",
            "#!\u{200E}[allow(x)]\n",
        ] {
            assert_eq!(without_shebang(text), text);
        }
        // A doc comment is a token, so `#!` starts a shebang.
        assert_eq!(without_shebang("#!/// x\n[allow(x)]\n"), "\n[allow(x)]\n");
        assert_eq!(without_shebang("#!/** x */[allow(x)]\n"), "\n");
        assert_eq!(without_shebang("#!/* never closed [allow(x)]\n"), "\n");
        let denied = denied()?;
        let source = Source {
            root: Path::new("."),
            name: "main.rs",
            is_crate_root: true,
            denied: &denied,
        };
        assert_eq!(
            source.problems("#!/* x */ [expect(unsafe_code, reason = \"no ADR\")]\nfn main() {}\n"),
            ["main.rs:1: its reason names no ADR in docs/adr/ (SEC-01)"]
        );
        Ok(())
    }

    #[test]
    fn reads_the_adr_number_from_the_reason() -> Result<()> {
        let arguments =
            |text: &str| TokenStream::from_str(text).map_err(|error| anyhow::anyhow!("{error}"));
        assert_eq!(
            adr_number(&arguments(
                r#"unsafe_code, reason = "ADR-0007: FFI to libfoo""#
            )?),
            Some("0007".to_owned())
        );
        assert_eq!(
            adr_number(&arguments(r#"unsafe_code, reason = "faster""#)?),
            None
        );
        assert_eq!(adr_number(&arguments("unsafe_code")?), None);
        Ok(())
    }
}
