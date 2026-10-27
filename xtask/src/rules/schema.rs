// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The rules data as Rust types, one for each kind of file in
//! `conventions/`. Every type refuses a field it doesn't know, so that a
//! misspelled field fails, naming its line, instead of being skipped. The
//! facts in `state.toml` have names of their own choosing, so the file is
//! a table of [`Fact`]s, which `rules.rs` reads in the file's order.

use std::fmt;

use serde::Deserialize;

/// A rule, from `rules/<PREFIX>.toml`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// Its area's prefix and a number, such as `A11Y-03`.
    pub id: String,
    pub level: Level,
    /// What holds under the rule, in Markdown.
    pub statement: String,
    /// Why the rule exists, in Markdown.
    pub rationale: String,
    /// How compliance is shown.
    pub verification: String,
    /// Where the rule is checked.
    pub check: Check,
    /// The trigger from which the rule applies.
    pub trigger: String,
    /// Who checks the rule.
    pub who: Who,
    /// When a rule that needs the founder comes up.
    #[serde(default)]
    pub moments: Vec<String>,
    /// A decision the rule leaves open.
    pub open: Option<Open>,
    /// The version of the conventions that added the rule.
    #[expect(dead_code, reason = "part of the schema, which no view shows yet")]
    pub added: String,
    #[expect(dead_code, reason = "part of the schema, which no view shows yet")]
    pub status: Status,
    /// What changed in the rule, one entry per change.
    #[serde(default)]
    #[expect(dead_code, reason = "part of the schema, which no view shows yet")]
    pub history: Vec<String>,
    /// Versions that replace the rule's text once their trigger holds.
    #[serde(default)]
    pub later: Vec<Later>,
    /// Notes on the rule, each shown under a trigger.
    #[serde(default)]
    pub reminder: Vec<Reminder>,
}

impl Rule {
    /// The prefix of its ID, which names its area.
    pub fn prefix(&self) -> &str {
        self.id
            .rsplit_once('-')
            .map_or(&self.id, |(prefix, _)| prefix)
    }
}

/// A rule's level, in BCP 14's words.
#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Level {
    Must,
    Should,
    May,
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Must => "MUST",
            Self::Should => "SHOULD",
            Self::May => "MAY",
        })
    }
}

/// Where a rule is checked.
#[derive(Clone, Copy, Deserialize)]
pub enum Check {
    /// Automated, and blocks the merge.
    #[serde(rename = "CI")]
    Ci,
    /// Enforced by the forge's settings, and audited.
    Settings,
    /// On the pull request's checklist.
    Review,
    /// A gate checked before each release.
    Release,
    /// A scheduled audit.
    Periodic,
}

impl Check {
    /// How the views say where the rule is checked.
    pub fn described(self) -> &'static str {
        match self {
            Self::Ci => "Enforced by CI",
            Self::Settings => "Enforced by forge settings",
            Self::Review => "Checked in review",
            Self::Release => "Checked before each release",
            Self::Periodic => "Checked in a periodic audit",
        }
    }
}

/// Who checks a rule: tooling, the AI review, or the founder.
#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Who {
    Tool,
    Ai,
    You,
}

/// Whether a rule is in force. Every rule is; a rule's retirement arrives
/// with the views that show it.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Active,
}

/// A decision that a rule leaves open.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Open {
    /// The decision to make, without a final period or question mark, which
    /// the views add.
    pub question: String,
    /// The answer favored so far, or nothing when there's none yet.
    pub leaning: String,
}

/// A version of a rule that replaces its text once its trigger holds. What
/// it leaves out, the rule keeps.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Later {
    /// The trigger from which this version applies.
    pub trigger: String,
    /// The statement that replaces the rule's.
    pub statement: String,
    /// The rationale that replaces the rule's, if it changes.
    pub rationale: Option<String>,
    /// The verification that replaces the rule's, if it changes.
    pub verification: Option<String>,
    /// Where the rule is checked from then on, if that changes.
    pub check: Option<Check>,
}

/// A note on a rule, shown under a trigger.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reminder {
    /// The trigger under which LATER.md shows the note.
    pub trigger: String,
    /// The note, in Markdown.
    pub note: String,
}

/// The rules of one area.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleFile {
    /// The area's rules, in any order.
    pub rule: Vec<Rule>,
}

/// A fact in `state.toml`: whether something is so, or how many there are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged, expecting = "a fact, which is true, false or a count")]
pub enum Fact {
    Holds(bool),
    Count(u64),
}

impl fmt::Display for Fact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Holds(value) => value.fmt(f),
            Self::Count(count) => count.fmt(f),
        }
    }
}

/// A trigger: the condition from which the rules that name it apply.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    /// Its name, which rules, later versions and reminders give as their
    /// trigger.
    pub id: String,
    /// How the views name it.
    pub title: String,
    /// The condition on the facts in `state.toml`, as written.
    pub when: String,
    /// Its place in LATER.md.
    pub order: u32,
}

/// `triggers.toml`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriggerFile {
    /// The triggers, in any order.
    pub trigger: Vec<Trigger>,
}

/// A moment: a situation in which some of the founder's rules come up.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Moment {
    /// Its name, which rules list among their moments.
    pub id: String,
    /// Its heading in NOW.md.
    pub title: String,
    /// Its place in NOW.md.
    pub order: u32,
}

/// `moments.toml`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MomentFile {
    /// The moments, in any order.
    pub moment: Vec<Moment>,
}

/// A principle, which decides the cases the rules don't cover.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Principle {
    /// Its number in the list.
    pub number: u32,
    /// Its name, a short sentence such as "Coherent.".
    pub name: String,
    /// What it says.
    pub text: String,
}

/// `principles.toml`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipleFile {
    /// The principles, in the order NOW.md lists them.
    pub principle: Vec<Principle>,
}

/// A term of the glossary.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Term {
    /// The term, with a leading "The" where the text uses one, which NOW.md
    /// doesn't need to find it.
    pub term: String,
    /// What it means, as the rest of a sentence that starts with the term.
    pub definition: String,
}

/// `glossary.toml`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Glossary {
    /// The terms, in the order NOW.md lists them.
    pub term: Vec<Term>,
}

/// An area of the rules.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Area {
    /// Its rules' prefix, such as `A11Y`.
    pub prefix: String,
    /// Its heading in the views.
    pub title: String,
    /// The group of areas it belongs to.
    #[expect(dead_code, reason = "part of the schema, which no view shows yet")]
    pub group: String,
    /// What the area covers, in Markdown.
    #[expect(dead_code, reason = "part of the schema, which no view shows yet")]
    pub intro: String,
    /// More on the area, in Markdown, possibly empty.
    #[expect(dead_code, reason = "part of the schema, which no view shows yet")]
    pub notes: String,
}

/// `areas.toml`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AreaFile {
    /// The areas, in the conventions' order.
    pub area: Vec<Area>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;

    const RULE: &str = "[[rule]]\nid = \"CI-01\"\nlevel = \"MUST\"\nstatement = \"S.\"\nrationale = \"R.\"\nverification = \"V\"\ncheck = \"CI\"\ntrigger = \"always\"\nwho = \"tool\"\nadded = \"0.1.0\"\nstatus = \"active\"\n";

    #[test]
    fn reads_a_rule_with_its_optional_tables() -> anyhow::Result<()> {
        let text = format!(
            "{RULE}moments = [\"setup\"]\nhistory = [\"A change.\"]\n\n[rule.open]\nquestion = \"Q\"\nleaning = \"\"\n\n[[rule.later]]\ntrigger = \"team\"\nstatement = \"S2.\"\n\n[[rule.reminder]]\ntrigger = \"team\"\nnote = \"N.\"\n"
        );
        let file: RuleFile = toml::from_str(&text)?;
        let [rule] = file.rule.as_slice() else {
            anyhow::bail!("expected one rule");
        };
        assert!(rule.level == Level::Must && rule.who == Who::Tool);
        assert_eq!(rule.prefix(), "CI");
        assert_eq!((rule.later.len(), rule.reminder.len()), (1, 1));
        assert!(
            rule.open
                .as_ref()
                .is_some_and(|open| open.leaning.is_empty())
        );
        Ok(())
    }

    #[test]
    fn refuses_unknown_missing_and_mistyped_fields() {
        for (old, new) in [
            ("verification =", "verificaton ="),
            ("status = \"active\"\n", ""),
            ("status = \"active\"", "status = \"retired\""),
            ("level = \"MUST\"", "level = \"MUST NOT\""),
            ("check = \"CI\"", "check = \"ci\""),
            ("who = \"tool\"", "who = \"bot\""),
            ("who = \"tool\"", "who = \"you\"\nmoments = \"setup\""),
            ("added = \"0.1.0\"", "added = 1"),
            ("[[rule]]", "[[rules]]"),
        ] {
            let text = RULE.replace(old, new);
            assert_ne!(text, RULE);
            assert!(toml::from_str::<RuleFile>(&text).is_err(), "{text}");
        }
        for table in [
            // A later version's level waits for the rule batch of issue #23.
            "[[rule.later]]\ntrigger = \"t\"\nstatement = \"S.\"\nlevel = \"MAY\"\n",
            "[[rule.later]]\ntrigger = \"t\"\nrationale = \"R.\"\n",
            "[[rule.reminder]]\ntrigger = \"t\"\nnote = \"N.\"\nwhen = \"t\"\n",
            "[rule.open]\nquestion = \"Q\"\n",
        ] {
            let text = format!("{RULE}\n{table}");
            assert!(toml::from_str::<RuleFile>(&text).is_err(), "{text}");
        }
        let trigger = "[[trigger]]\nid = \"t\"\ntitle = \"T\"\nwhen = \"true\"\norder = 0\n";
        assert!(toml::from_str::<TriggerFile>(trigger).is_ok());
        assert!(
            toml::from_str::<TriggerFile>(&trigger.replace("order = 0", "order = -1")).is_err()
        );
        let area = "[[area]]\nprefix = \"CI\"\ntitle = \"T\"\ngroup = \"G\"\nintro = \"I\"\nnotes = \"\"\n";
        assert!(toml::from_str::<AreaFile>(area).is_ok());
        assert!(toml::from_str::<AreaFile>(&area.replace("notes = \"\"\n", "")).is_err());
    }

    /// Fails unless `T` reads `valid` and refuses `unknown`, which adds a
    /// field named `unknown`, for that field.
    fn refuses_the_unknown_field<T: DeserializeOwned>(valid: &str, unknown: &str) {
        assert!(toml::from_str::<T>(valid).is_ok(), "{valid}");
        let error = toml::from_str::<T>(unknown)
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(
            error.contains("unknown field `unknown`"),
            "{unknown}\n{error}"
        );
    }

    /// `refuses_the_unknown_field` for one type.
    type Refuses = fn(&str, &str);

    /// Every type refuses a field it doesn't know, at the top of each file
    /// and in each kind of entry.
    #[test]
    fn every_type_refuses_an_unknown_field() {
        let field = "unknown = 1\n";
        let rule_tables = [
            "[rule.open]\nquestion = \"Q\"\nleaning = \"\"\n",
            "[[rule.later]]\ntrigger = \"t\"\nstatement = \"S.\"\n",
            "[[rule.reminder]]\ntrigger = \"t\"\nnote = \"N.\"\n",
        ];
        refuses_the_unknown_field::<RuleFile>(RULE, &format!("{field}{RULE}"));
        refuses_the_unknown_field::<RuleFile>(RULE, &format!("{RULE}{field}"));
        for table in rule_tables {
            let valid = format!("{RULE}{table}");
            refuses_the_unknown_field::<RuleFile>(&valid, &format!("{valid}{field}"));
        }
        let files: [(&str, Refuses); 5] = [
            (
                "[[trigger]]\nid = \"t\"\ntitle = \"T\"\nwhen = \"true\"\norder = 0\n",
                refuses_the_unknown_field::<TriggerFile>,
            ),
            (
                "[[moment]]\nid = \"m\"\ntitle = \"M\"\norder = 0\n",
                refuses_the_unknown_field::<MomentFile>,
            ),
            (
                "[[principle]]\nnumber = 1\nname = \"N.\"\ntext = \"T.\"\n",
                refuses_the_unknown_field::<PrincipleFile>,
            ),
            (
                "[[term]]\nterm = \"T\"\ndefinition = \"d.\"\n",
                refuses_the_unknown_field::<Glossary>,
            ),
            (
                "[[area]]\nprefix = \"CI\"\ntitle = \"T\"\ngroup = \"G\"\nintro = \"I\"\nnotes = \"\"\n",
                refuses_the_unknown_field::<AreaFile>,
            ),
        ];
        for (valid, refuses) in files {
            refuses(valid, &format!("{field}{valid}"));
            refuses(valid, &format!("{valid}{field}"));
        }
    }

    #[test]
    fn reads_a_fact_as_true_false_or_a_count() -> anyhow::Result<()> {
        let facts: std::collections::BTreeMap<String, Fact> =
            toml::from_str("a = true\nb = false\nc = 2\n")?;
        assert_eq!(
            facts.into_values().collect::<Vec<_>>(),
            [Fact::Holds(true), Fact::Holds(false), Fact::Count(2)]
        );
        for value in ["\"yes\"", "-1", "1.5", "[true]", "{ a = 1 }"] {
            let Err(error) = toml::from_str::<std::collections::BTreeMap<String, Fact>>(&format!(
                "fact = {value}\n"
            )) else {
                anyhow::bail!("`fact = {value}` passed");
            };
            let error = error.to_string();
            assert!(
                error.contains("a fact, which is true, false or a count")
                    && error.contains("line 1"),
                "{error}"
            );
        }
        Ok(())
    }
}
