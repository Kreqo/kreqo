// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! SEC-02's rule for deny.toml: every suppression gives its reason and an
//! expiry date. cargo-deny 0.20 has no expiry of its own, so a reason ends
//! with `until YYYY-MM-DD`, and the suppression fails once that day passes.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

/// The arrays in deny.toml whose entries suppress a finding and can carry a
/// `reason`. License exceptions can't, so review checks theirs.
const SUPPRESSIONS: [(&str, &str); 3] = [
    ("advisories", "ignore"),
    ("bans", "skip"),
    ("bans", "skip-tree"),
];

/// Fails if a suppression in deny.toml lacks a reason or an expiry date, or
/// has expired.
pub fn check_suppressions(root: &Path) -> Result<()> {
    let path = root.join("deny.toml");
    let text =
        fs::read_to_string(&path).with_context(|| format!("couldn't read {}", path.display()))?;
    let problems = suppression_problems(&text, &today()?)
        .with_context(|| format!("couldn't parse {}", path.display()))?;
    if problems.is_empty() {
        return Ok(());
    }
    for problem in &problems {
        eprintln!("deny.toml: {problem} (SEC-02)");
    }
    bail!("{} suppression(s) break SEC-02", problems.len())
}

/// The problems with a deny.toml's suppressions on a given day.
fn suppression_problems(text: &str, today: &str) -> Result<Vec<String>> {
    let config: toml::Table = text.parse()?;
    let mut problems = Vec::new();
    for (table, key) in SUPPRESSIONS {
        let entries = config
            .get(table)
            .and_then(|table| table.get(key))
            .and_then(toml::Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        for (index, entry) in entries.iter().enumerate() {
            let entry_name = format!("{table}.{key}[{index}]");
            let reason = entry
                .get("reason")
                .and_then(toml::Value::as_str)
                .unwrap_or_default();
            if reason.trim().is_empty() {
                problems.push(format!("{entry_name} gives no reason"));
            } else {
                match expiry(reason) {
                    None => problems.push(format!(
                        "{entry_name} gives no expiry date; end its reason with `until YYYY-MM-DD`"
                    )),
                    Some(date) if date < today => {
                        problems.push(format!("{entry_name} expired after {date}"));
                    }
                    Some(_) => {}
                }
            }
        }
    }
    Ok(problems)
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
            suppression_problems(text, "2026-10-24")?,
            [
                "advisories.ignore[1] gives no expiry date; end its reason with `until YYYY-MM-DD`",
                "advisories.ignore[2] gives no reason",
                "bans.skip[0] expired after 2026-10-01",
            ]
        );
        Ok(())
    }

    #[test]
    fn accepts_a_configuration_without_suppressions() -> Result<()> {
        assert!(suppression_problems("[advisories]\nignore = []\n", "2026-10-24")?.is_empty());
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
