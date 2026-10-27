// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The rules data as Rust types, one for each kind of file in
//! `conventions/`. Every type refuses a field it doesn't know, so that a
//! misspelled field fails, naming its line, instead of being skipped.
//! `state.toml` holds facts of any name, so `rules.rs` reads it.

use std::fmt;

use serde::Deserialize;

/// A rule, from `rules/<PREFIX>.toml`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// Its area's prefix and a number, such as `A11Y-03`.
    pub id: String,
    pub level: Level,
    pub statement: String,
    pub rationale: String,
    /// How compliance is shown.
    pub verification: String,
    pub check: Check,
    /// The trigger from which the rule applies.
    pub trigger: String,
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
    pub question: String,
    /// The answer favored so far, or nothing when there's none yet.
    pub leaning: String,
}

/// A version of a rule that replaces its text once its trigger holds. What
/// it leaves out, the rule keeps.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Later {
    pub trigger: String,
    pub statement: String,
    pub rationale: Option<String>,
    pub verification: Option<String>,
    pub check: Option<Check>,
}

/// A note on a rule, shown under a trigger.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reminder {
    pub trigger: String,
    pub note: String,
}

/// The rules of one area.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleFile {
    pub rule: Vec<Rule>,
}

/// A trigger: the condition from which the rules that name it apply.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    pub id: String,
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
    pub trigger: Vec<Trigger>,
}

/// A moment: a situation in which some of the founder's rules come up.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Moment {
    pub id: String,
    pub title: String,
    /// Its place in NOW.md.
    pub order: u32,
}

/// `moments.toml`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MomentFile {
    pub moment: Vec<Moment>,
}

/// A principle, which decides the cases the rules don't cover.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Principle {
    pub number: u32,
    pub name: String,
    pub text: String,
}

/// `principles.toml`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipleFile {
    pub principle: Vec<Principle>,
}

/// A term of the glossary.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Term {
    pub term: String,
    pub definition: String,
}

/// `glossary.toml`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Glossary {
    pub term: Vec<Term>,
}

/// An area of the rules.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Area {
    /// Its rules' prefix, such as `A11Y`.
    pub prefix: String,
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

/// `areas.toml`, which lists the areas in the conventions' order.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AreaFile {
    pub area: Vec<Area>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(toml::from_str::<TriggerFile>(&format!("{trigger}level = \"MUST\"\n")).is_err());
        let area = "[[area]]\nprefix = \"CI\"\ntitle = \"T\"\ngroup = \"G\"\nintro = \"I\"\nnotes = \"\"\n";
        assert!(toml::from_str::<AreaFile>(area).is_ok());
        assert!(toml::from_str::<AreaFile>(&area.replace("notes = \"\"\n", "")).is_err());
        assert!(
            toml::from_str::<Glossary>("[[terms]]\nterm = \"T\"\ndefinition = \"D\"\n").is_err()
        );
    }
}
