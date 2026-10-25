// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! SEC-02's written policy, enforced on deny.toml before cargo-deny reads
//! it. Every setting must be one the policy names, at the value it names,
//! because a single unknown key, such as `[graph] exclude`, can hide a
//! vulnerability, a banned SDK or a license failure. Every setting the
//! policy names must be there too, since leaving one out brings back
//! cargo-deny's weaker default, and the ban list and the allowed licenses
//! must be exactly the policy's. Every suppression gives
//! its reason and an expiry date: cargo-deny 0.20 has no expiry of its own,
//! so a reason ends with `until YYYY-MM-DD`, a real date at most 90 days
//! ahead, and the suppression fails once that day passes.

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
    /// Exactly these strings, in any order.
    Exactly(&'static [&'static str]),
    /// Suppressions: tables with only these keys, each giving a reason that
    /// ends with its expiry date.
    Suppressions(&'static [&'static str]),
    /// Bans of exactly these crates, each a table with a crate and a reason
    /// only, since `wrappers` or `deny-multiple-versions` would lift the ban
    /// (PRIV-09).
    Bans(&'static [&'static str]),
    /// License exceptions, which can't carry a reason in cargo-deny 0.20;
    /// review checks the comment above each.
    Exceptions,
}

impl Setting {
    /// Whether deny.toml must hold the setting: every one but the lists of
    /// suppressions and exceptions, which may be empty.
    fn is_required(&self) -> bool {
        !matches!(self, Self::Suppressions(_) | Self::Exceptions)
    }
}

/// The SDKs that PRIV-09 bans (docs/decisions.md).
const BANNED_CRATES: [&str; 12] = [
    "sentry",
    "sentry-core",
    "posthog-rs",
    "segment",
    "analytics",
    "rudderanalytics",
    "bugsnag",
    "rollbar",
    "honeybadger",
    "mixpanel",
    "mixpanel-rs",
    "amplitude",
];

/// The licenses that the dependencies of MIT OR Apache-2.0 crates may use
/// (LEGAL-03, docs/decisions.md).
const ALLOWED_LICENSES: [&str; 11] = [
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Zlib",
    "Unicode-3.0",
    "CC0-1.0",
    "Unlicense",
    "BSL-1.0",
    "MPL-2.0",
];

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
    ("bans", "deny", Setting::Bans(&BANNED_CRATES)),
    ("bans", "skip", Setting::Suppressions(&["crate", "reason"])),
    (
        "bans",
        "skip-tree",
        Setting::Suppressions(&["crate", "reason", "depth"]),
    ),
    ("licenses", "allow", Setting::Exactly(&ALLOWED_LICENSES)),
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

/// How many days ahead an expiry date may be, so that every suppression
/// comes back for review at least once a quarter (docs/decisions.md).
const MAX_SUPPRESSION_DAYS: u64 = 90;

/// Fails if deny.toml strays from the written policy, if a suppression
/// lacks a reason or an expiry date, or if an exceptions file exists.
pub fn check_policy(root: &Path) -> Result<()> {
    let path = root.join("deny.toml");
    let text =
        fs::read_to_string(&path).with_context(|| format!("couldn't read {}", path.display()))?;
    let mut problems = policy_problems(&text, today()?)
        .with_context(|| format!("couldn't parse {}", path.display()))?;
    // cargo-deny looks for exceptions files from the manifest's folder up,
    // whatever --config says.
    for folder in root.ancestors() {
        for file in EXCEPTION_FILES {
            let path = folder.join(file);
            if path.exists() {
                problems.push(format!(
                    "{} exists; cargo-deny reads it on its own, so its entries belong in deny.toml",
                    path.display()
                ));
            }
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

/// The ways a deny.toml strays from the policy on a given day, counted from
/// 1970-01-01.
fn policy_problems(text: &str, today: u64) -> Result<Vec<String>> {
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
    for (table, key, setting) in &POLICY {
        let present = config.get(*table).and_then(|table| table.get(*key));
        if setting.is_required() && present.is_none() {
            problems.push(format!(
                "`{table}.{key}` is missing, which brings back cargo-deny's weaker default"
            ));
        }
    }
    Ok(problems)
}

fn check_setting(
    name: &str,
    value: &Value,
    setting: &Setting,
    today: u64,
    problems: &mut Vec<String>,
) {
    match setting {
        Setting::Is(expected) if value.as_str() != Some(expected) => {
            problems.push(format!("`{name}` must be \"{expected}\""));
        }
        Setting::IsTrue if value.as_bool() != Some(true) => {
            problems.push(format!("`{name}` must be true"));
        }
        Setting::Exactly(expected) => match value.as_array() {
            Some(items) if items.iter().all(Value::is_str) => {
                let listed: Vec<&str> = items.iter().filter_map(Value::as_str).collect();
                check_list(name, &listed, expected, problems);
            }
            _ => problems.push(format!("`{name}` must be a list of strings")),
        },
        Setting::Suppressions(keys) => {
            for (index, entry) in entries(value).iter().enumerate() {
                let entry_name = format!("{name}[{index}]");
                check_keys(&entry_name, entry, keys, problems);
                check_reason(&entry_name, entry, today, problems);
            }
        }
        Setting::Bans(expected) => {
            for (index, entry) in entries(value).iter().enumerate() {
                let entry_name = format!("{name}[{index}]");
                check_keys(&entry_name, entry, &["crate", "reason"], problems);
            }
            let banned: Vec<&str> = entries(value)
                .iter()
                .filter_map(|entry| entry.get("crate").and_then(Value::as_str))
                .collect();
            check_list(name, &banned, expected, problems);
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

/// Fails a list that lacks an item the policy names, or has one it doesn't.
fn check_list(name: &str, listed: &[&str], expected: &[&str], problems: &mut Vec<String>) {
    for item in expected.iter().filter(|item| !listed.contains(item)) {
        problems.push(format!("`{name}` lacks `{item}`, which the policy names"));
    }
    for item in listed.iter().filter(|item| !expected.contains(item)) {
        problems.push(format!(
            "`{name}` has `{item}`, which the policy doesn't name"
        ));
    }
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

/// Fails a suppression whose reason is missing, doesn't end with a real
/// expiry date, has expired, or runs past the limit.
fn check_reason(name: &str, entry: &Value, today: u64, problems: &mut Vec<String>) {
    if !entry.is_table() {
        return;
    }
    let reason = entry
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if reason.trim().is_empty() {
        problems.push(format!("{name} gives no reason"));
        return;
    }
    let Some(date) = expiry(reason) else {
        problems.push(format!(
            "{name} gives no expiry date; end its reason with `until YYYY-MM-DD`"
        ));
        return;
    };
    if justification(reason).is_empty() {
        problems.push(format!("{name} gives a date but no reason"));
        return;
    }
    match day_of(date) {
        None => problems.push(format!("{name} gives an impossible date, {date}")),
        Some(day) if day < today => problems.push(format!("{name} expired after {date}")),
        Some(day) if day > today + MAX_SUPPRESSION_DAYS => problems.push(format!(
            "{name} runs past {MAX_SUPPRESSION_DAYS} days from today; give a nearer date"
        )),
        Some(_) => {}
    }
}

/// What a reason says before its `until`, without trailing punctuation.
fn justification(reason: &str) -> &str {
    reason
        .rsplit_once("until ")
        .map_or(reason, |(before, _)| before)
        .trim()
        .trim_end_matches([';', ',', '.', ':', '-'])
        .trim_end()
}

/// The date that ends a reason as `until YYYY-MM-DD`, if one does.
fn expiry(reason: &str) -> Option<&str> {
    let (_, date) = reason.trim_end().rsplit_once("until ")?;
    let is_date = date.len() == 10
        && date.char_indices().all(|(position, c)| match position {
            4 | 7 => c == '-',
            _ => c.is_ascii_digit(),
        });
    is_date.then_some(date)
}

/// The day counted from 1970-01-01 of a YYYY-MM-DD date from 1970 on, or
/// None for a date that doesn't exist, such as 2027-02-30.
fn day_of(date: &str) -> Option<u64> {
    let mut parts = date.split('-').map(str::parse::<u64>);
    let (Some(Ok(year)), Some(Ok(month)), Some(Ok(day))) =
        (parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    if year < 1970 || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let days = days_from_civil(year, month, day);
    (civil_from_days(days) == (year, month, day)).then_some(days)
}

/// Today in UTC, counted in days from 1970-01-01.
fn today() -> Result<u64> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("the clock is set before 1970")?
        .as_secs();
    Ok(seconds / 86_400)
}

/// The day counted from 1970-01-01 of a proleptic Gregorian date from 1970
/// on, after Howard Hinnant's `days_from_civil`.
fn days_from_civil(year: u64, month: u64, day: u64) -> u64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year / 400;
    let year_of_era = year - era * 400;
    let shifted_month = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
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

    /// 2026-10-24, a fixed day for the tests that judge dates.
    const TODAY: u64 = 20_750;

    /// Judged on the real date, as the deny step judges it, so that a
    /// suppression valid today never fails CI.
    #[test]
    fn accepts_the_committed_policy() -> Result<()> {
        let text = include_str!("../../deny.toml");
        assert_eq!(policy_problems(text, today()?)?, Vec::<String>::new());
        Ok(())
    }

    /// The committed deny.toml, with each change applied once. Line endings
    /// are made `\n` first, since a Windows checkout may have `\r\n`.
    fn committed_with(changes: &[(&str, &str)]) -> String {
        let mut text = include_str!("../../deny.toml").replace("\r\n", "\n");
        for (old, new) in changes {
            assert!(text.contains(old), "deny.toml no longer holds {old:?}");
            text = text.replacen(old, new, 1);
        }
        text
    }

    #[test]
    fn rejects_settings_the_policy_doesnt_name() -> Result<()> {
        let text = committed_with(&[
            (
                "all-features = true",
                "all-features = true\nexclude = [\"openssl\"]",
            ),
            ("yanked = \"deny\"", "yanked = \"warn\""),
            (
                "{ crate = \"sentry\", reason = \"PRIV-09: crash-reporting SDK\" }",
                "{ crate = \"sentry\", reason = \"PRIV-09: crash-reporting SDK\", wrappers = [\"my-crate\"] }",
            ),
            ("exceptions = []", "exceptions = []\nclarify = []"),
            (
                "unknown-git = \"deny\"",
                "unknown-git = \"deny\"\n\n[licenses.private]\nignore = true",
            ),
        ]);
        // Tables and keys come out in alphabetical order.
        assert_eq!(
            policy_problems(&text, TODAY)?,
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
    fn rejects_deletions_and_list_edits() -> Result<()> {
        let text = committed_with(&[
            ("yanked = \"deny\"\n", ""),
            (
                "  { crate = \"sentry\", reason = \"PRIV-09: crash-reporting SDK\" },\n",
                "",
            ),
            ("\"MPL-2.0\",", "\"MPL-2.0\",\n  \"GPL-3.0-only\","),
        ]);
        assert_eq!(
            policy_problems(&text, TODAY)?,
            [
                "`bans.deny` lacks `sentry`, which the policy names",
                "`licenses.allow` has `GPL-3.0-only`, which the policy doesn't name",
                "`advisories.yanked` is missing, which brings back cargo-deny's weaker default",
            ]
        );
        Ok(())
    }

    #[test]
    fn checks_reasons_and_expiry_dates() -> Result<()> {
        let text = committed_with(&[
            (
                "ignore = []",
                r#"ignore = [
  { id = "RUSTSEC-0000-0000", reason = "not reachable from our code; until 2026-12-31" },
  { id = "RUSTSEC-0000-0001", reason = "no fix upstream yet" },
  "RUSTSEC-0000-0002",
  { id = "RUSTSEC-0000-0003", reason = "until 2026-12-31, then revisit" },
  { id = "RUSTSEC-0000-0004", reason = "until 2026-12-31" },
]"#,
            ),
            (
                "skip = []",
                r#"skip = [
  { crate = "windows-sys@0.52.0", reason = "two versions upstream; until 2026-10-01" },
  { crate = "windows-sys@0.59.0", reason = "two versions upstream; until 2027-02-30" },
  { crate = "windows-sys@0.60.0", reason = "two versions upstream; until 2027-06-30" },
]"#,
            ),
        ]);
        assert_eq!(
            policy_problems(&text, TODAY)?,
            [
                "advisories.ignore[1] gives no expiry date; end its reason with `until YYYY-MM-DD`",
                "advisories.ignore[2] must be a table with a reason",
                "advisories.ignore[3] gives no expiry date; end its reason with `until YYYY-MM-DD`",
                "advisories.ignore[4] gives a date but no reason",
                "bans.skip[0] expired after 2026-10-01",
                "bans.skip[1] gives an impossible date, 2027-02-30",
                "bans.skip[2] runs past 90 days from today; give a nearer date",
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
        assert_eq!(expiry("until 2027-01-31, then revisit"), None);
        assert_eq!(expiry("no date"), None);
        assert_eq!(day_of("2026-10-24"), Some(TODAY));
        assert_eq!(day_of("2028-02-29"), Some(TODAY + 493));
        assert_eq!(day_of("2027-02-29"), None);
        assert_eq!(day_of("1969-12-31"), None);
    }

    #[test]
    fn converts_between_days_and_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(11_017), (2000, 3, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2024, 1, 1), 19_723);
        assert_eq!(civil_from_days(TODAY), (2026, 10, 24));
    }
}
