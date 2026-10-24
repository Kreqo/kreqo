// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! SEC-02's written policy, enforced on deny.toml before cargo-deny reads
//! it. Every setting must be one the policy names, at the value it names,
//! because a single unknown key, such as `[graph] exclude`, can hide a
//! vulnerability, a banned SDK or a license failure. Every suppression gives
//! its reason and an expiry date: cargo-deny 0.20 has no expiry of its own,
//! so a reason ends with `until YYYY-MM-DD`, and the suppression fails once
//! that day passes.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use toml::{Table, Value};

/// What a setting in deny.toml may hold under the written policy.
enum Setting {
    /// Exactly this string.
    Is(&'static str),
    /// Exactly `true`.
    IsTrue,
    /// A list of strings, such as the allowed licenses.
    Strings,
    /// Suppressions: tables with only these keys, each giving a reason that
    /// ends with its expiry date.
    Suppressions(&'static [&'static str]),
    /// Bans: tables with a crate and a reason only, since `wrappers` or
    /// `deny-multiple-versions` would lift the ban (PRIV-09).
    Bans,
    /// License exceptions, which can't carry a reason in cargo-deny 0.20;
    /// review checks the comment above each.
    Exceptions,
}

/// Every setting SEC-02's written policy names (docs/decisions.md).
const POLICY: [(&str, &str, Setting); 14] = [
    ("graph", "all-features", Setting::IsTrue),
    ("advisories", "unsound", Setting::Is("all")),
    ("advisories", "unmaintained", Setting::Is("workspace")),
    ("advisories", "yanked", Setting::Is("deny")),
    (
        "advisories",
        "ignore",
        Setting::Suppressions(&["id", "crate", "reason"]),
    ),
    ("bans", "multiple-versions", Setting::Is("deny")),
    ("bans", "deny", Setting::Bans),
    ("bans", "skip", Setting::Suppressions(&["crate", "reason"])),
    (
        "bans",
        "skip-tree",
        Setting::Suppressions(&["crate", "reason", "depth"]),
    ),
    ("licenses", "allow", Setting::Strings),
    ("licenses", "unused-allowed-license", Setting::Is("allow")),
    ("licenses", "exceptions", Setting::Exceptions),
    ("sources", "unknown-registry", Setting::Is("deny")),
    ("sources", "unknown-git", Setting::Is("deny")),
];

/// Files that cargo-deny reads on its own, past this check.
const EXCEPTION_FILES: [&str; 3] = [
    "deny.exceptions.toml",
    ".deny.exceptions.toml",
    ".cargo/deny.exceptions.toml",
];

/// Fails if deny.toml strays from the written policy, if a suppression
/// lacks a reason or an expiry date, or if an exceptions file exists.
pub fn check_policy(root: &Path) -> Result<()> {
    let path = root.join("deny.toml");
    let text =
        fs::read_to_string(&path).with_context(|| format!("couldn't read {}", path.display()))?;
    let mut problems = policy_problems(&text, &today()?)
        .with_context(|| format!("couldn't parse {}", path.display()))?;
    for file in EXCEPTION_FILES {
        if root.join(file).exists() {
            problems.push(format!(
                "{file} exists; cargo-deny reads it on its own, so its entries belong in deny.toml"
            ));
        }
    }
    if problems.is_empty() {
        return Ok(());
    }
    for problem in &problems {
        eprintln!("deny.toml: {problem} (SEC-02)");
    }
    bail!("{} problem(s) with SEC-02's written policy", problems.len())
}

/// The ways a deny.toml strays from the policy on a given day.
fn policy_problems(text: &str, today: &str) -> Result<Vec<String>> {
    let config: Table = text.parse()?;
    let mut problems = Vec::new();
    for (table_name, table) in &config {
        let Some(table) = table.as_table() else {
            problems.push(format!("`{table_name}` isn't a setting the policy names"));
            continue;
        };
        for (key, value) in table {
            let name = format!("{table_name}.{key}");
            let Some((_, _, setting)) = POLICY
                .iter()
                .find(|(table, setting_key, _)| table == table_name && setting_key == key)
            else {
                problems.push(format!("`{name}` isn't a setting the policy names"));
                continue;
            };
            check_setting(&name, value, setting, today, &mut problems);
        }
    }
    Ok(problems)
}

fn check_setting(
    name: &str,
    value: &Value,
    setting: &Setting,
    today: &str,
    problems: &mut Vec<String>,
) {
    match setting {
        Setting::Is(expected) if value.as_str() != Some(expected) => {
            problems.push(format!("`{name}` must be \"{expected}\""));
        }
        Setting::IsTrue if value.as_bool() != Some(true) => {
            problems.push(format!("`{name}` must be true"));
        }
        Setting::Strings
            if !value
                .as_array()
                .is_some_and(|items| items.iter().all(Value::is_str)) =>
        {
            problems.push(format!("`{name}` must be a list of strings"));
        }
        Setting::Suppressions(keys) => {
            for (index, entry) in entries(value).iter().enumerate() {
                let entry_name = format!("{name}[{index}]");
                check_keys(&entry_name, entry, keys, problems);
                check_reason(&entry_name, entry, today, problems);
            }
        }
        Setting::Bans => {
            for (index, entry) in entries(value).iter().enumerate() {
                let entry_name = format!("{name}[{index}]");
                check_keys(&entry_name, entry, &["crate", "reason"], problems);
            }
        }
        Setting::Exceptions => {
            for (index, entry) in entries(value).iter().enumerate() {
                check_keys(
                    &format!("{name}[{index}]"),
                    entry,
                    &["allow", "crate"],
                    problems,
                );
            }
        }
        _ => {}
    }
}

fn entries(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or_default()
}

/// Fails an entry that isn't a table, or that has a key outside `keys`.
fn check_keys(name: &str, entry: &Value, keys: &[&str], problems: &mut Vec<String>) {
    let Some(entry) = entry.as_table() else {
        problems.push(format!("{name} must be a table with a reason"));
        return;
    };
    for key in entry.keys() {
        if !keys.contains(&key.as_str()) {
            problems.push(format!(
                "{name} sets `{key}`, which the policy doesn't allow"
            ));
        }
    }
}

/// Fails a suppression whose reason is missing, gives no expiry date, or has
/// expired.
fn check_reason(name: &str, entry: &Value, today: &str, problems: &mut Vec<String>) {
    let reason = entry
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if entry.is_table() && reason.trim().is_empty() {
        problems.push(format!("{name} gives no reason"));
        return;
    }
    match expiry(reason) {
        None if entry.is_table() => problems.push(format!(
            "{name} gives no expiry date; end its reason with `until YYYY-MM-DD`"
        )),
        Some(date) if date < today => problems.push(format!("{name} expired after {date}")),
        _ => {}
    }
}

/// The date in a reason's `until YYYY-MM-DD`, if it has one.
fn expiry(reason: &str) -> Option<&str> {
    let (_, rest) = reason.rsplit_once("until ")?;
    let date = rest.get(..10)?;
    let is_date = date.char_indices().all(|(position, c)| match position {
        4 | 7 => c == '-',
        _ => c.is_ascii_digit(),
    });
    is_date.then_some(date)
}

/// Today's date in UTC, as YYYY-MM-DD.
fn today() -> Result<String> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("the clock is set before 1970")?
        .as_secs();
    let (year, month, day) = civil_from_days(seconds / 86_400);
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}

/// The proleptic Gregorian date of a day counted from 1970-01-01, after
/// Howard Hinnant's `civil_from_days`.
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let days = days + 719_468;
    let era = days / 146_097;
    let day_of_era = days % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_committed_policy() -> Result<()> {
        let text = include_str!("../../deny.toml");
        assert_eq!(policy_problems(text, "2026-10-24")?, Vec::<String>::new());
        Ok(())
    }

    #[test]
    fn rejects_settings_the_policy_doesnt_name() -> Result<()> {
        let text = r#"
            [graph]
            all-features = true
            exclude = ["openssl"]

            [advisories]
            yanked = "warn"

            [bans]
            deny = [{ crate = "sentry", reason = "PRIV-09", wrappers = ["my-crate"] }]

            [licenses]
            clarify = []

            [licenses.private]
            ignore = true
        "#;
        // Tables and keys come out in alphabetical order.
        assert_eq!(
            policy_problems(text, "2026-10-24")?,
            [
                "`advisories.yanked` must be \"deny\"",
                "bans.deny[0] sets `wrappers`, which the policy doesn't allow",
                "`graph.exclude` isn't a setting the policy names",
                "`licenses.clarify` isn't a setting the policy names",
                "`licenses.private` isn't a setting the policy names",
            ]
        );
        Ok(())
    }

    #[test]
    fn checks_reasons_and_expiry_dates() -> Result<()> {
        let text = r#"
            [advisories]
            ignore = [
              { id = "RUSTSEC-0000-0000", reason = "not reachable from our code; until 2027-01-31" },
              { id = "RUSTSEC-0000-0001", reason = "no fix upstream yet" },
              "RUSTSEC-0000-0002",
            ]

            [bans]
            skip = [{ crate = "windows-sys@0.52.0", reason = "until 2026-10-01" }]
        "#;
        assert_eq!(
            policy_problems(text, "2026-10-24")?,
            [
                "advisories.ignore[1] gives no expiry date; end its reason with `until YYYY-MM-DD`",
                "advisories.ignore[2] must be a table with a reason",
                "bans.skip[0] expired after 2026-10-01",
            ]
        );
        Ok(())
    }

    #[test]
    fn reads_expiry_dates() {
        assert_eq!(
            expiry("upstream fix pending; until 2027-01-31"),
            Some("2027-01-31")
        );
        assert_eq!(expiry("until 2027-1-31"), None);
        assert_eq!(expiry("no date"), None);
    }

    #[test]
    fn converts_days_to_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(11_017), (2000, 3, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
    }
}
