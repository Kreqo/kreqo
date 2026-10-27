// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The text handling that the views share with the prototype, which wrote
//! them in Python: its whitespace, its word characters, and the patterns it
//! split sentences, counted words and found glossary terms with. The views
//! stay byte for byte the same only if these do.

/// Whether a character is whitespace to Python: Unicode's whitespace, and
/// the four information separators, U+001C to U+001F.
pub fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Whether a character is a word character, as Python's `\w` matches: a
/// letter, a digit or `_`. Rust's letters also take in the rest of what
/// Unicode counts as alphabetic, such as Devanagari's vowel signs, circled
/// letters like Ⓐ, and letters newer than Python 3.12's Unicode 15.0; none
/// of the conventions' text uses them.
pub fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// A text's first sentence: the text up to the first `.`, `!` or `?` that
/// whitespace and then a capital letter, `` ` ``, `*` or `[` follow, as the
/// prototype's split on `` (?<=[.!?])\s+(?=[A-Z`*\[]) `` cut it. A text
/// with no such end is one sentence.
pub fn first_sentence(text: &str) -> &str {
    for (at, c) in text.char_indices() {
        if !matches!(c, '.' | '!' | '?') {
            continue;
        }
        let end = at + c.len_utf8();
        let rest = &text[end..];
        let next = rest.trim_start_matches(is_space);
        if next.len() < rest.len()
            && next.starts_with(|c: char| c.is_ascii_uppercase() || matches!(c, '`' | '*' | '['))
        {
            return &text[..end];
        }
    }
    text
}

/// How many words lines hold, as the prototype counted them for NOW.md's
/// reading time: Markdown's `*`, `` ` ``, `|`, `#`, `\` and `-` part words,
/// as whitespace does.
pub fn words(lines: &[String]) -> usize {
    lines
        .iter()
        .map(|line| {
            line.split(|c: char| is_space(c) || matches!(c, '*' | '`' | '|' | '#' | '\\' | '-'))
                .filter(|word| !word.is_empty())
                .count()
        })
        .sum()
}

/// Whether a text, already lowercased, uses a glossary term, as the
/// prototype's `\bterm\b`, ignoring case, found it: the whole term, less a
/// leading "The ", anywhere between word boundaries.
pub fn mentions(lowercased: &str, term: &str) -> bool {
    let term = term.strip_prefix("The ").unwrap_or(term).to_lowercase();
    lowercased
        .char_indices()
        .map(|(at, _)| at)
        .chain([lowercased.len()])
        .any(|at| {
            lowercased[at..].starts_with(&term)
                && boundary(lowercased, at)
                && boundary(lowercased, at + term.len())
        })
}

/// Whether `\b` matches at a byte offset of a text: a word character on one
/// side of it, and none on the other.
fn boundary(text: &str, at: usize) -> bool {
    let before = text[..at].chars().next_back().is_some_and(is_word);
    let after = text[at..].chars().next().is_some_and(is_word);
    before != after
}

#[cfg(test)]
mod tests {
    use super::*;

    // The expected answers are the prototype's, from Python 3.12.

    #[test]
    fn treats_what_python_treats_as_whitespace_as_whitespace() {
        for c in [
            '\t', '\n', '\u{b}', '\u{1c}', '\u{1f}', ' ', '\u{85}', '\u{a0}', '\u{3000}',
        ] {
            assert!(is_space(c), "{c:?}");
        }
        for c in ['\u{1b}', '\u{200b}', '\u{feff}', 'a', '_'] {
            assert!(!is_space(c), "{c:?}");
        }
    }

    #[test]
    fn cuts_the_first_sentence_where_the_prototype_did() {
        for (text, first) in [
            ("One. Two.", "One."),
            ("One.Two. Three", "One.Two."),
            ("e.g. the x. Then", "e.g. the x."),
            ("A.  \n B", "A."),
            ("x! `y`", "x!"),
            ("x? *y*", "x?"),
            ("x. [y]", "x."),
            ("x. y. Z", "x. y."),
            ("x.\u{a0}Y", "x."),
            ("x.\u{1c}Y", "x."),
            ("Ends.", "Ends."),
            ("a. 1b. C", "a. 1b."),
            ("x. É", "x. É"),
        ] {
            assert_eq!(first_sentence(text), first, "{text:?}");
        }
    }

    #[test]
    fn counts_words_as_the_prototype_did() {
        let lines = |lines: &[&str]| {
            lines
                .iter()
                .map(|&line| line.to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            words(&lines(&[
                "## Principles",
                "",
                "**ADR-01** · MUST · a-b `c`|d\\e"
            ])),
            11
        );
        assert_eq!(
            words(&lines(&[
                "one\u{1c}two",
                "three\u{a0}four",
                "five\u{200b}six"
            ])),
            5
        );
        assert_eq!(words(&lines(&["--- *** ``` ||| ### \\\\"])), 0);
    }

    #[test]
    fn finds_terms_as_the_prototype_did() {
        for (term, text, used) in [
            ("ADR", "Each ADRs list", false),
            ("ADR", "an adr.", true),
            ("ADR", "ADR-0001 says", true),
            ("The forge", "the forge's pull requests", true),
            ("The forge", "a forger", false),
            ("Sign-off list", "the sign-off lists", false),
            ("Sign-off list", "its Sign-Off List.", true),
            // An occurrence that isn't a whole word overlaps one that is.
            ("a a", "xa a a", true),
            ("The organization", "The organizations", false),
            ("The template", "the Template", true),
            ("Tier 1", "Tier 10", false),
            ("Tier 1", "on Tier 1.", true),
            ("Maintainer", "maintainer_x", false),
            ("Maintainer", "(maintainer)", true),
            ("The The", "the the", true),
            ("The The", "The", true),
            ("RFC", "RFCs", false),
            ("RFC", "an RFC", true),
            ("Public interface", "public\ninterface", false),
        ] {
            assert_eq!(
                mentions(&text.to_lowercase(), term),
                used,
                "{term:?} in {text:?}"
            );
        }
    }
}
