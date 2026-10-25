// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Attribute checks over every Rust file in the repository: no `allow`
//! attribute, inner or outer (QA-01), and `expect(unsafe_code)` only in a
//! crate root's `#![expect(unsafe_code, reason = "ADR-NNNN")]` that names an
//! existing ADR (SEC-01). Clippy's `allow_attributes` skips inner attributes
//! and any `cfg_attr` whose condition doesn't hold, so these checks read
//! each file as tokens: comments and strings never match, and an attribute
//! that rustfmt wraps over several lines stays whole.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::Result;
use proc_macro2::{Delimiter, Ident, TokenStream, TokenTree};

use crate::git;

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
        let stream = match TokenStream::from_str(&without_shebang(&text)) {
            Ok(stream) => stream,
            Err(error) => {
                problems.push(format!(
                    "{}: couldn't read its tokens, so its attributes go unchecked: {error}",
                    relative.display()
                ));
                continue;
            }
        };
        let is_crate_root = crate_roots.contains(&canonical(&path));
        for attribute in attributes(stream) {
            let place = format!("{}:{}", relative.display(), attribute.line);
            let applied = applied(&attribute.tokens);
            if applied.iter().any(|(name, _)| name == "allow") {
                problems.push(format!(
                    "{place}: an `allow` attribute; suppress with `#[expect(lint, reason = \"…\")]` (QA-01)"
                ));
            }
            for (_, arguments) in applied.iter().filter(|(name, _)| name == "expect") {
                if !mentions(arguments, "unsafe_code") {
                    continue;
                }
                if !(attribute.inner && attribute.top_level && is_crate_root) {
                    problems.push(format!(
                        "{place}: `expect(unsafe_code)` belongs only in a crate root's `#![expect(unsafe_code, reason = \"ADR-NNNN\")]` (SEC-01)"
                    ));
                } else if !adr_number(arguments).is_some_and(|number| adr_exists(root, &number)) {
                    problems.push(format!(
                        "{place}: its reason names no ADR in docs/adr/ (SEC-01)"
                    ));
                }
            }
        }
    }
    Ok(problems)
}

/// The path with `..` and links resolved, so that two names for one file
/// compare equal; the path as it is when it can't be resolved.
fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The text with a leading shebang line blanked, as rustc ignores it, and
/// line numbers kept. `#![` starts an inner attribute instead.
fn without_shebang(text: &str) -> Cow<'_, str> {
    match text.strip_prefix("#!") {
        Some(rest) if !rest.trim_start().starts_with('[') => {
            let line_end = text.find('\n').unwrap_or(text.len());
            Cow::Owned(text.get(line_end..).unwrap_or_default().to_owned())
        }
        _ => Cow::Borrowed(text),
    }
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
/// identifier, such as `allow(…)`, and for `cfg_attr(condition, …)`, each
/// attribute after the condition, however deep. A longer path, such as
/// rstest's `case::allow(…)`, names something else.
fn applied(tokens: &TokenStream) -> Vec<(String, TokenStream)> {
    let tokens: Vec<TokenTree> = tokens.clone().into_iter().collect();
    let [TokenTree::Ident(name), TokenTree::Group(arguments)] = tokens.as_slice() else {
        return Vec::new();
    };
    if arguments.delimiter() != Delimiter::Parenthesis {
        return Vec::new();
    }
    let name = unraw(name);
    if name == "cfg_attr" {
        split_commas(arguments.stream())
            .iter()
            .skip(1)
            .flat_map(applied)
            .collect()
    } else {
        vec![(name, arguments.stream())]
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

/// Whether the identifier appears anywhere in the tokens.
fn mentions(tokens: &TokenStream, name: &str) -> bool {
    tokens.clone().into_iter().any(|token| match token {
        TokenTree::Ident(ident) => unraw(&ident) == name,
        TokenTree::Group(group) => mentions(&group.stream(), name),
        _ => false,
    })
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
    fn reads_raw_identifiers() -> Result<()> {
        let stream = TokenStream::from_str("r#expect(r#unsafe_code, r#reason = \"ADR-0001\")")
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        let applied = applied(&stream);
        let [(name, arguments)] = applied.as_slice() else {
            anyhow::bail!("expected one applied attribute, got {}", applied.len());
        };
        assert_eq!(name, "expect");
        assert!(mentions(arguments, "unsafe_code"));
        assert_eq!(adr_number(arguments), Some("0001".to_owned()));
        Ok(())
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
