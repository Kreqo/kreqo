// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The rules engine (ARCH-06): it reads the conventions' data in
//! `conventions/`, checks it, and generates the views in
//! `conventions/views/`, which say what applies now, what tooling and the AI
//! review check, and what waits for a trigger (principle 12). It ports the
//! Python prototype that wrote the views until M2, and writes them byte for
//! byte as the prototype did. `cargo xtask rules generate` writes them, and
//! `cargo xtask check rules` fails when the committed ones differ from what
//! the data generates.

mod schema;
mod text;
mod views;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use serde::de::DeserializeOwned;
use toml::{Spanned, Value};

use schema::{
    Area, AreaFile, Check, Glossary, Moment, MomentFile, Principle, PrincipleFile, Rule, RuleFile,
    Term, Trigger, TriggerFile, Who,
};
use views::Views;

/// A `cargo xtask rules` command.
#[derive(Subcommand)]
pub enum Command {
    /// Checks the rules data in conventions/ and writes its views in
    /// conventions/views/.
    Generate,
}

/// Runs a `cargo xtask rules` command.
pub fn run(root: &Path, command: Command) -> Result<()> {
    match command {
        Command::Generate => generate(root),
    }
}

/// Checks the rules data and writes its views.
pub fn generate(root: &Path) -> Result<()> {
    let folder = root.join("conventions");
    let views = views::render(&Rules::load(&folder)?);
    let out = folder.join("views");
    fs::create_dir_all(&out).with_context(|| format!("couldn't create {}", out.display()))?;
    for (name, text) in views.files() {
        let path = out.join(name);
        fs::write(&path, text).with_context(|| format!("couldn't write {}", path.display()))?;
    }
    println!("{} Views written to {}.", views.summary, out.display());
    Ok(())
}

/// The `rules` step of `cargo xtask check`: fails if the rules data has a
/// problem, or if a committed view differs from what the data generates.
pub fn check(root: &Path) -> Result<()> {
    let folder = root.join("conventions");
    let views = views::render(&Rules::load(&folder)?);
    let problems = stale(&folder, &views)?;
    if problems.is_empty() {
        println!("{}", views.summary);
        return Ok(());
    }
    for problem in &problems {
        eprintln!("{problem}");
    }
    bail!("{} view(s) out of date", problems.len())
}

/// What's wrong with the committed views in `folder`: each one that's
/// missing or differs from what the data generates.
fn stale(folder: &Path, views: &Views) -> Result<Vec<String>> {
    let mut problems = Vec::new();
    for (name, text) in views.files() {
        let path = folder.join("views").join(name);
        match fs::read(&path) {
            Ok(committed) => {
                if let Some(line) = first_difference(&committed, text.as_bytes()) {
                    problems.push(format!(
                        "{}: differs from what the rules data generates, from line {line}; run `cargo xtask rules generate`",
                        path.display()
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => problems.push(format!(
                "{}: missing; run `cargo xtask rules generate`",
                path.display()
            )),
            Err(error) => {
                return Err(error).with_context(|| format!("couldn't read {}", path.display()));
            }
        }
    }
    Ok(problems)
}

/// The number of the first line in which two texts differ, if they do.
fn first_difference(old: &[u8], new: &[u8]) -> Option<usize> {
    if old == new {
        return None;
    }
    let same = old
        .split(|&byte| byte == b'\n')
        .zip(new.split(|&byte| byte == b'\n'))
        .take_while(|(old, new)| old == new)
        .count();
    Some(same + 1)
}

/// The rules data, checked, with the rules in the conventions' order: by
/// area, then by number.
pub struct Rules {
    /// The facts of `state.toml`, in its order.
    pub state: Vec<(String, Fact)>,
    pub triggers: Vec<Trigger>,
    pub moments: Vec<Moment>,
    pub principles: Vec<Principle>,
    pub glossary: Vec<Term>,
    pub areas: Vec<Area>,
    pub rules: Vec<Rule>,
    /// The triggers whose condition holds in the state.
    holding: BTreeSet<String>,
}

/// A fact's value in `state.toml`: whether something is so, or how many
/// there are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// A rule's text as it reads in the current state.
pub struct Current<'a> {
    pub statement: &'a str,
    pub rationale: &'a str,
    pub verification: &'a str,
    pub check: Check,
}

impl Rules {
    /// Reads the data in `folder` and checks it, reporting every problem.
    pub fn load(folder: &Path) -> Result<Self> {
        Self::read(folder)?.checked().or_else(|problems| {
            for problem in &problems {
                eprintln!("{problem}");
            }
            bail!(
                "{} problem(s) with the rules data in {}",
                problems.len(),
                folder.display()
            )
        })
    }

    /// Reads the data in `folder`, failing on a file that can't be read or
    /// doesn't fit its type; `checked` finds every other problem.
    fn read(folder: &Path) -> Result<Self> {
        let mut rules = Vec::new();
        for path in rule_files(folder)? {
            rules.extend(read::<RuleFile>(&path)?.rule);
        }
        Ok(Self {
            state: read_state(&folder.join("state.toml"))?,
            triggers: read::<TriggerFile>(&folder.join("triggers.toml"))?.trigger,
            moments: read::<MomentFile>(&folder.join("moments.toml"))?.moment,
            principles: read::<PrincipleFile>(&folder.join("principles.toml"))?.principle,
            glossary: read::<Glossary>(&folder.join("glossary.toml"))?.term,
            areas: read::<AreaFile>(&folder.join("areas.toml"))?.area,
            rules,
            holding: BTreeSet::new(),
        })
    }

    /// The data with the triggers that hold worked out and the rules in
    /// order, or every problem it has.
    fn checked(mut self) -> Result<Self, Vec<String>> {
        let mut problems = Vec::new();
        duplicates(
            "trigger",
            self.triggers.iter().map(|t| &t.id),
            &mut problems,
        );
        duplicates("moment", self.moments.iter().map(|m| &m.id), &mut problems);
        duplicates("area", self.areas.iter().map(|a| &a.prefix), &mut problems);
        for trigger in &self.triggers {
            match holds(&trigger.when, &self.state) {
                Ok(true) => {
                    self.holding.insert(trigger.id.clone());
                }
                Ok(false) => {}
                Err(problem) => problems.push(format!("trigger {}: {problem}", trigger.id)),
            }
        }
        let triggers: BTreeSet<&str> = self.triggers.iter().map(|t| t.id.as_str()).collect();
        let moments: BTreeSet<&str> = self.moments.iter().map(|m| m.id.as_str()).collect();
        let mut seen = BTreeSet::new();
        for rule in &self.rules {
            let id = &rule.id;
            if !seen.insert(id) {
                problems.push(format!("{id}: duplicate ID"));
            }
            if place(&self.areas, rule).is_none() {
                problems.push(format!(
                    "{id}: an ID is an area's prefix, a hyphen and a number"
                ));
            }
            if !triggers.contains(rule.trigger.as_str()) {
                problems.push(format!("{id}: unknown trigger {}", rule.trigger));
            }
            if rule.who == Who::You && rule.moments.is_empty() {
                problems.push(format!(
                    "{id}: a rule that needs you names the moments it comes up"
                ));
            }
            for moment in &rule.moments {
                if !moments.contains(moment.as_str()) {
                    problems.push(format!("{id}: unknown moment {moment}"));
                }
            }
            let named = rule.later.iter().map(|version| &version.trigger);
            for trigger in named.chain(rule.reminder.iter().map(|note| &note.trigger)) {
                if !triggers.contains(trigger.as_str()) {
                    problems.push(format!(
                        "{id}: unknown trigger {trigger} in a later version or reminder"
                    ));
                }
            }
        }
        if !problems.is_empty() {
            return Err(problems);
        }
        let areas = &self.areas;
        // Stable, as the prototype's sort is, so that rules with the same
        // place, such as CI-1 and CI-01, keep the files' order.
        self.rules.sort_by_key(|rule| place(areas, rule));
        Ok(self)
    }

    /// Whether a trigger's condition holds in the state.
    pub fn holds(&self, trigger: &str) -> bool {
        self.holding.contains(trigger)
    }

    /// A rule's text in the current state: each later version whose trigger
    /// holds replaces it, field by field, in the order they're listed.
    pub fn current<'a>(&self, rule: &'a Rule) -> Current<'a> {
        let mut current = Current {
            statement: &rule.statement,
            rationale: &rule.rationale,
            verification: &rule.verification,
            check: rule.check,
        };
        for version in rule.later.iter().filter(|v| self.holds(&v.trigger)) {
            current.statement = &version.statement;
            if let Some(rationale) = &version.rationale {
                current.rationale = rationale;
            }
            if let Some(verification) = &version.verification {
                current.verification = verification;
            }
            if let Some(check) = version.check {
                current.check = check;
            }
        }
        current
    }
}

/// Reads a TOML file as its type.
fn read<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let text =
        fs::read_to_string(path).with_context(|| format!("couldn't read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("{} is invalid", path.display()))
}

/// The rule files, by name: every file in `rules/` whose name ends in
/// `.toml`, as the prototype found them.
fn rule_files(folder: &Path) -> Result<Vec<PathBuf>> {
    let dir = folder.join("rules");
    let mut files = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("couldn't read {}", dir.display()))? {
        let entry = entry.with_context(|| format!("couldn't read {}", dir.display()))?;
        if entry.file_name().to_string_lossy().ends_with(".toml") {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

/// The facts of `state.toml`, in the file's order, which NOW.md's summary
/// of the state follows. toml's tables sort their keys, so the facts' spans
/// give the order back.
fn read_state(path: &Path) -> Result<Vec<(String, Fact)>> {
    let facts: BTreeMap<String, Spanned<Value>> = read(path)?;
    let mut facts: Vec<_> = facts.into_iter().collect();
    facts.sort_by_key(|(_, value)| value.span().start);
    facts
        .into_iter()
        .map(|(name, value)| {
            let fact = match value.get_ref() {
                Value::Boolean(holds) => Some(Fact::Holds(*holds)),
                Value::Integer(count) => u64::try_from(*count).ok().map(Fact::Count),
                _ => None,
            };
            let Some(fact) = fact else {
                bail!(
                    "{}: `{name} = {}`, but a fact is true, false or a count",
                    path.display(),
                    value.get_ref()
                );
            };
            Ok((name, fact))
        })
        .collect()
}

/// Reports each ID that more than one entry of a file has.
fn duplicates<'a>(kind: &str, ids: impl Iterator<Item = &'a String>, problems: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            problems.push(format!("{kind} {id}: duplicate ID"));
        }
    }
}

/// A rule's place in the conventions, its area's position and its number,
/// if its ID has both.
fn place(areas: &[Area], rule: &Rule) -> Option<(usize, u32)> {
    let (prefix, number) = rule.id.rsplit_once('-')?;
    let area = areas.iter().position(|area| area.prefix == prefix)?;
    if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((area, number.parse().ok()?))
}

/// Whether a trigger's condition holds in the state, or what's wrong with
/// it. A condition joins clauses with " or ", and a clause is `true`, a
/// fact that is true or false, `not` and such a fact, or a count `>=` a
/// number (`triggers.toml`).
fn holds(when: &str, state: &[(String, Fact)]) -> Result<bool, String> {
    let fact = |name: &str| {
        state
            .iter()
            .find(|(fact, _)| fact == name)
            .map(|(_, value)| *value)
            .ok_or_else(|| format!("unknown fact '{name}'"))
    };
    let mut holds = false;
    // Every clause is read, so that none goes unchecked.
    for clause in when
        .split(" or ")
        .map(|clause| clause.trim_matches(text::is_space))
    {
        holds |= if clause == "true" {
            true
        } else if let Some((name, number)) = count_clause(clause) {
            match fact(name)? {
                Fact::Count(count) => count >= number,
                Fact::Holds(_) => {
                    return Err(format!(
                        "'{clause}' compares '{name}', which is true or false, not a count"
                    ));
                }
            }
        } else {
            let (negated, name) = clause
                .strip_prefix("not ")
                .map_or((false, clause), |name| (true, name));
            match fact(name)? {
                Fact::Holds(value) => value != negated,
                Fact::Count(_) => {
                    return Err(format!(
                        "'{clause}' tests '{name}', which is a count, not true or false"
                    ));
                }
            }
        };
    }
    Ok(holds)
}

/// The fact and the number of a clause of the form `fact >= N`, which the
/// prototype matched with `(\w+) >= (\d+)`, though only ASCII digits make
/// a number here.
fn count_clause(clause: &str) -> Option<(&str, u64)> {
    let (name, number) = clause.split_once(" >= ")?;
    if name.is_empty()
        || !name.chars().all(text::is_word)
        || number.is_empty()
        || !number.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    Some((name, number.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    /// A rule's TOML: its ID and `more`, with the other fields filled in.
    fn rule(id: &str, more: &str) -> String {
        format!(
            "[[rule]]\nid = \"{id}\"\nlevel = \"MUST\"\nstatement = \"{id}'s statement.\"\nrationale = \"{id}'s rationale.\"\nverification = \"{id}'s verification\"\ncheck = \"CI\"\nadded = \"0.1.0\"\nstatus = \"active\"\n{more}\n\n"
        )
    }

    /// Valid rules data, each file's name and text: the QA area comes
    /// before CI, and the CI rules are out of order in their file.
    fn data() -> Vec<(&'static str, String)> {
        let triggers = [
            ("always", "true", 0),
            ("open", "open", 1),
            ("team", "maintainers >= 2", 2),
            ("shipped", "shipped", 3),
        ];
        vec![
            (
                "state.toml",
                "maintainers = 1\nopen = false\nshipped = true\n".to_owned(),
            ),
            (
                "triggers.toml",
                triggers
                    .iter()
                    .map(|(id, when, order)| {
                        format!("[[trigger]]\nid = \"{id}\"\ntitle = \"Trigger {id}\"\nwhen = \"{when}\"\norder = {order}\n\n")
                    })
                    .collect(),
            ),
            (
                "moments.toml",
                "[[moment]]\nid = \"setup\"\ntitle = \"When you set up\"\norder = 0\n\n".to_owned(),
            ),
            (
                "principles.toml",
                "[[principle]]\nnumber = 1\nname = \"Coherent.\"\ntext = \"One way of doing each thing.\"\n".to_owned(),
            ),
            (
                "glossary.toml",
                "[[term]]\nterm = \"The forge\"\ndefinition = \"the service hosting the repositories.\"\n".to_owned(),
            ),
            (
                "areas.toml",
                ["QA", "CI"]
                    .iter()
                    .map(|prefix| {
                        format!("[[area]]\nprefix = \"{prefix}\"\ntitle = \"Area {prefix}\"\ngroup = \"Group\"\nintro = \"About {prefix}.\"\nnotes = \"\"\n\n")
                    })
                    .collect(),
            ),
            (
                "rules/CI.toml",
                [
                    rule("CI-10", "trigger = \"team\"\nwho = \"ai\""),
                    rule(
                        "CI-02",
                        "trigger = \"always\"\nwho = \"you\"\nmoments = [\"setup\"]\n\n[[rule.later]]\ntrigger = \"always\"\nstatement = \"First.\"\n\n[[rule.later]]\ntrigger = \"shipped\"\nstatement = \"Second.\"\ncheck = \"Review\"\n\n[[rule.later]]\ntrigger = \"team\"\nstatement = \"Not yet.\"\nrationale = \"Never.\"",
                    ),
                ]
                .concat(),
            ),
            ("rules/QA.toml", rule("QA-01", "trigger = \"open\"\nwho = \"tool\"")),
            // Not a rule file, so never read.
            ("rules/notes.txt", "[[rule]]\nid = \n".to_owned()),
        ]
    }

    /// Writes the data into the `conventions/` folder under `root`.
    fn write(root: &Path, data: &[(&str, String)]) -> Result<()> {
        let folder = root.join("conventions");
        fs::create_dir_all(folder.join("rules"))?;
        for (name, text) in data {
            fs::write(folder.join(name), text)?;
        }
        Ok(())
    }

    /// The data, read, then checked.
    fn load(data: &[(&str, String)]) -> Result<Result<Rules, Vec<String>>> {
        let scratch = Scratch::new()?;
        write(scratch.path(), data)?;
        Ok(Rules::read(&scratch.path().join("conventions"))?.checked())
    }

    #[test]
    fn puts_the_rules_in_the_conventions_order() -> Result<()> {
        let Ok(rules) = load(&data())? else {
            bail!("the data has problems");
        };
        let ids: Vec<&str> = rules.rules.iter().map(|rule| rule.id.as_str()).collect();
        assert_eq!(ids, ["QA-01", "CI-02", "CI-10"]);
        Ok(())
    }

    #[test]
    fn applies_the_later_versions_that_hold() -> Result<()> {
        let Ok(rules) = load(&data())? else {
            bail!("the data has problems");
        };
        assert!(rules.holds("always") && rules.holds("shipped"));
        assert!(!rules.holds("open") && !rules.holds("team"));
        let ci_02 = rules
            .rules
            .iter()
            .find(|rule| rule.id == "CI-02")
            .context("CI-02 is missing")?;
        let current = rules.current(ci_02);
        // The last version that holds wins, and what no holding version
        // replaces stays the rule's own.
        assert_eq!(current.statement, "Second.");
        assert_eq!(current.rationale, "CI-02's rationale.");
        assert_eq!(current.verification, "CI-02's verification");
        assert!(matches!(current.check, Check::Review));
        Ok(())
    }

    #[test]
    fn reports_every_problem_in_the_data() -> Result<()> {
        let mut data = data();
        let more = [
            (
                "triggers.toml",
                [
                    ("always", "true"),
                    ("lost", "nobody"),
                    ("compares", "open >= 2"),
                    ("tests", "not maintainers"),
                ]
                .iter()
                .map(|(id, when)| {
                    format!("[[trigger]]\nid = \"{id}\"\ntitle = \"{id}\"\nwhen = \"{when}\"\norder = 9\n\n")
                })
                .collect::<String>(),
            ),
            (
                "moments.toml",
                "[[moment]]\nid = \"setup\"\ntitle = \"Again\"\norder = 1\n".to_owned(),
            ),
            (
                "areas.toml",
                "[[area]]\nprefix = \"CI\"\ntitle = \"Again\"\ngroup = \"\"\nintro = \"\"\nnotes = \"\"\n".to_owned(),
            ),
            (
                "rules/QA.toml",
                [
                    rule("CI-02", "trigger = \"always\"\nwho = \"tool\""),
                    rule("XX-01", "trigger = \"always\"\nwho = \"tool\""),
                    rule("QA-1a", "trigger = \"always\"\nwho = \"tool\""),
                    rule("QA02", "trigger = \"always\"\nwho = \"tool\""),
                    rule("QA-03", "trigger = \"later\"\nwho = \"tool\""),
                    rule("QA-04", "trigger = \"always\"\nwho = \"you\""),
                    rule(
                        "QA-05",
                        "trigger = \"always\"\nwho = \"you\"\nmoments = [\"setup\", \"release\"]",
                    ),
                    rule(
                        "QA-06",
                        "trigger = \"always\"\nwho = \"ai\"\n\n[[rule.later]]\ntrigger = \"gone\"\nstatement = \"S.\"\n\n[[rule.reminder]]\ntrigger = \"never\"\nnote = \"N.\"",
                    ),
                ]
                .concat(),
            ),
        ];
        for (name, text) in more {
            for (file, old) in &mut data {
                if *file == name {
                    old.push_str(&text);
                }
            }
        }
        let Err(problems) = load(&data)? else {
            bail!("the data passed");
        };
        assert_eq!(
            problems,
            [
                "trigger always: duplicate ID",
                "moment setup: duplicate ID",
                "area CI: duplicate ID",
                "trigger lost: unknown fact 'nobody'",
                "trigger compares: 'open >= 2' compares 'open', which is true or false, not a count",
                "trigger tests: 'not maintainers' tests 'maintainers', which is a count, not true or false",
                "CI-02: duplicate ID",
                "XX-01: an ID is an area's prefix, a hyphen and a number",
                "QA-1a: an ID is an area's prefix, a hyphen and a number",
                "QA02: an ID is an area's prefix, a hyphen and a number",
                "QA-03: unknown trigger later",
                "QA-04: a rule that needs you names the moments it comes up",
                "QA-05: unknown moment release",
                "QA-06: unknown trigger gone in a later version or reminder",
                "QA-06: unknown trigger never in a later version or reminder",
            ]
        );
        Ok(())
    }

    #[test]
    fn evaluates_conditions() {
        let state = [
            ("maintainers".to_owned(), Fact::Count(1)),
            ("open".to_owned(), Fact::Holds(false)),
            ("shipped".to_owned(), Fact::Holds(true)),
        ];
        for (when, expected) in [
            ("true", true),
            ("open", false),
            ("not open", true),
            ("not shipped", false),
            ("maintainers >= 0", true),
            ("maintainers >= 1", true),
            ("maintainers >= 2", false),
            ("open or maintainers >= 2", false),
            ("open or shipped", true),
            // The prototype trimmed each clause.
            ("  open   or shipped  ", true),
        ] {
            assert_eq!(holds(when, &state), Ok(expected), "{when}");
        }
        for (when, problem) in [
            ("nobody", "unknown fact 'nobody'"),
            ("open and shipped", "unknown fact 'open and shipped'"),
            // A clause after one that holds is still checked.
            ("shipped or nobody", "unknown fact 'nobody'"),
            ("not  open", "unknown fact ' open'"),
            (
                "open >= 1",
                "'open >= 1' compares 'open', which is true or false, not a count",
            ),
            (
                "maintainers",
                "'maintainers' tests 'maintainers', which is a count, not true or false",
            ),
        ] {
            assert_eq!(holds(when, &state), Err(problem.to_owned()), "{when}");
        }
    }

    #[test]
    fn reads_the_facts_in_the_files_order() -> Result<()> {
        let scratch = Scratch::new()?;
        let path = scratch.path().join("state.toml");
        fs::write(&path, "zeta = true # A comment\nalpha = 2\nmid = false\n")?;
        assert_eq!(
            read_state(&path)?,
            [
                ("zeta".to_owned(), Fact::Holds(true)),
                ("alpha".to_owned(), Fact::Count(2)),
                ("mid".to_owned(), Fact::Holds(false)),
            ]
        );
        for value in ["\"yes\"", "-1", "1.5", "[true]"] {
            fs::write(&path, format!("fact = {value}\n"))?;
            let Err(error) = read_state(&path) else {
                bail!("`fact = {value}` passed");
            };
            assert!(
                format!("{error:#}").contains("but a fact is true, false or a count"),
                "{error:#}"
            );
        }
        Ok(())
    }

    #[test]
    fn checks_the_views_against_the_data() -> Result<()> {
        let scratch = Scratch::new()?;
        write(scratch.path(), &data())?;
        let folder = scratch.path().join("conventions");
        let stale_views =
            || -> Result<Vec<String>> { stale(&folder, &views::render(&Rules::load(&folder)?)) };
        let missing = stale_views()?;
        assert_eq!(missing.len(), 3, "{missing:?}");
        assert!(missing.iter().all(|problem| problem.contains(": missing;")));
        generate(scratch.path())?;
        assert_eq!(stale_views()?, Vec::<String>::new());
        check(scratch.path())?;
        // A second maintainer starts CI-10 and changes the state's summary
        // on NOW.md's sixth line.
        fs::write(
            folder.join("state.toml"),
            "maintainers = 2\nopen = false\nshipped = true\n",
        )?;
        let changed = stale_views()?;
        assert_eq!(changed.len(), 3, "{changed:?}");
        assert!(
            changed[0].ends_with(
                "NOW.md: differs from what the rules data generates, from line 6; run `cargo xtask rules generate`"
            ),
            "{changed:?}"
        );
        assert!(check(scratch.path()).is_err());
        generate(scratch.path())?;
        check(scratch.path())
    }

    #[test]
    fn finds_the_first_line_that_differs() {
        assert_eq!(first_difference(b"a\nb\n", b"a\nb\n"), None);
        assert_eq!(first_difference(b"a\nb\n", b"a\nc\n"), Some(2));
        assert_eq!(first_difference(b"a\nb\n", b"a\nb\nc\n"), Some(3));
        assert_eq!(first_difference(b"a\nb\n", b"a\nb"), Some(3));
        assert_eq!(first_difference(b"a\r\nb\n", b"a\nb\n"), Some(1));
        assert_eq!(first_difference(b"", b"a\n"), Some(1));
    }

    /// Git keeps the committed views' LF line endings in every checkout,
    /// Windows included, so that the check compares them byte for byte
    /// (.gitattributes).
    #[test]
    fn keeps_the_views_line_endings_in_every_checkout() -> Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .context("xtask's folder has no parent")?;
        for name in ["NOW.md", "CHECKED.md", "LATER.md"] {
            let path = format!("conventions/views/{name}");
            let output = std::process::Command::new("git")
                .args(["check-attr", "eol", "--", &path])
                .current_dir(root)
                .output()
                .context("couldn't run git")?;
            assert_eq!(
                String::from_utf8(output.stdout)?,
                format!("{path}: eol: lf\n")
            );
        }
        Ok(())
    }
}
