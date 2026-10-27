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
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use serde::de::DeserializeOwned;
use toml::Spanned;

use crate::git;
use schema::{
    Area, AreaFile, Check, Fact, Glossary, Moment, MomentFile, Principle, PrincipleFile, Rule,
    RuleFile, Term, Trigger, TriggerFile, Who,
};
use views::Views;

/// The views' folder, from the root.
const VIEWS: &str = "conventions/views";

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
    let views = views::render(&Rules::load(root)?);
    let out = root.join(VIEWS);
    fs::create_dir_all(&out).with_context(|| format!("couldn't create {}", out.display()))?;
    for (name, text) in views.files() {
        let path = out.join(name);
        fs::write(&path, text).with_context(|| format!("couldn't write {}", path.display()))?;
    }
    println!("{} Views written to {}.", views.summary, out.display());
    Ok(())
}

/// The `rules` step of `cargo xtask check`: fails if the rules data has a
/// problem, or if the committed views aren't what the data generates.
pub fn check(root: &Path) -> Result<()> {
    let views = views::render(&Rules::load(root)?);
    let problems = stale(root, &views)?;
    if problems.is_empty() {
        println!("{}", views.summary);
        return Ok(());
    }
    for problem in &problems {
        eprintln!("{problem}");
    }
    bail!("{} problem(s) with the committed views", problems.len())
}

/// What's wrong with the committed views: each view that's missing or
/// differs from what the data generates, and each other file in their
/// folder, which nothing generates or checks.
fn stale(root: &Path, views: &Views) -> Result<Vec<String>> {
    let mut problems = Vec::new();
    for (name, text) in views.files() {
        let path = root.join(VIEWS).join(name);
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
    let generated: Vec<PathBuf> = views
        .files()
        .iter()
        .map(|(name, _)| Path::new(VIEWS).join(name))
        .collect();
    for path in git::files(root, &[&format!(":(glob){VIEWS}/**")])? {
        if !generated.contains(&path) {
            problems.push(format!(
                "{}: not a view that the rules data generates; remove it",
                root.join(&path).display()
            ));
        }
    }
    Ok(problems)
}

/// The number of the first line in which two texts differ, if they do,
/// counting lines as an editor does.
fn first_difference(old: &[u8], new: &[u8]) -> Option<usize> {
    if old == new {
        return None;
    }
    let same = old
        .split_inclusive(|&byte| byte == b'\n')
        .zip(new.split_inclusive(|&byte| byte == b'\n'))
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

/// A rule's text as it reads in the current state.
pub struct Current<'a> {
    pub statement: &'a str,
    pub rationale: &'a str,
    pub verification: &'a str,
    pub check: Check,
}

impl Rules {
    /// Reads the data in the root's `conventions/` and checks it, reporting
    /// every problem.
    pub fn load(root: &Path) -> Result<Self> {
        Self::read(root)?.checked().or_else(|problems| {
            for problem in &problems {
                eprintln!("{problem}");
            }
            bail!(
                "{} problem(s) with the rules data in {}",
                problems.len(),
                root.join("conventions").display()
            )
        })
    }

    /// Reads the data in the root's `conventions/`, failing on a file that
    /// can't be read or doesn't fit its type; `checked` finds every other
    /// problem.
    fn read(root: &Path) -> Result<Self> {
        let folder = root.join("conventions");
        let mut rules = Vec::new();
        for path in rule_files(root)? {
            rules.extend(read::<RuleFile>(&root.join(path))?.rule);
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
                Err(found) => problems.extend(
                    found
                        .into_iter()
                        .map(|problem| format!("trigger {}: {problem}", trigger.id)),
                ),
            }
        }
        let triggers: BTreeSet<&str> = self.triggers.iter().map(|t| t.id.as_str()).collect();
        let moments: BTreeSet<&str> = self.moments.iter().map(|m| m.id.as_str()).collect();
        let mut seen = BTreeSet::new();
        let mut places = BTreeMap::new();
        for rule in &self.rules {
            let id = &rule.id;
            if !seen.insert(id) {
                problems.push(format!("{id}: duplicate ID"));
            }
            match place(&self.areas, rule) {
                // Two IDs for one place, such as CI-1 and CI-01, are one
                // rule's ID written two ways.
                Ok(place) => {
                    if let Some(other) = places.insert(place, id)
                        && other != id
                    {
                        problems.push(format!("{id}: the same area and number as {other}"));
                    }
                }
                Err(problem) => problems.push(format!("{id}: {problem}")),
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
        // Once the checks pass, each rule has a place of its own, so no
        // two compare equal.
        self.rules.sort_by_key(|rule| place(areas, rule).ok());
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

/// Reads a TOML file as its type. The parser sees LF line endings, as the
/// prototype's did, so that a file checked out with CRLF, as git does on
/// Windows, reads the same, multi-line strings included.
fn read<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let text =
        fs::read_to_string(path).with_context(|| format!("couldn't read {}", path.display()))?;
    toml::from_str(&text.replace("\r\n", "\n"))
        .with_context(|| format!("{} is invalid", path.display()))
}

/// The rule files, from the root, by name: every `*.toml` file in
/// `conventions/rules/` that git tracks or would track. The prototype read
/// every one; a file that git ignores, such as a local draft, stays out of
/// the views, which a commit would carry into public.
fn rule_files(root: &Path) -> Result<Vec<PathBuf>> {
    git::files(root, &[":(glob)conventions/rules/*.toml"])
}

/// The facts of `state.toml`, in the file's order, which NOW.md's summary
/// of the state follows. toml's tables sort their keys, so the facts' spans
/// give the order back.
fn read_state(path: &Path) -> Result<Vec<(String, Fact)>> {
    let facts: BTreeMap<String, Spanned<Fact>> = read(path)?;
    let mut facts: Vec<_> = facts.into_iter().collect();
    facts.sort_by_key(|(_, fact)| fact.span().start);
    Ok(facts
        .into_iter()
        .map(|(name, fact)| (name, fact.into_inner()))
        .collect())
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

/// Whether a text is a number in ASCII digits alone, as every number in
/// the rules' IDs and conditions is; `str::parse` would also take a
/// leading `+`.
fn is_number(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// A rule's place in the conventions, its area's position and its number,
/// or why its ID gives none.
fn place(areas: &[Area], rule: &Rule) -> Result<(usize, u64), String> {
    let malformed = || "an ID is an area's prefix, a hyphen and a number".to_owned();
    let (prefix, number) = rule.id.rsplit_once('-').ok_or_else(malformed)?;
    let area = areas
        .iter()
        .position(|area| area.prefix == prefix)
        .ok_or_else(|| format!("no area has the prefix {prefix}"))?;
    if !is_number(number) {
        return Err(malformed());
    }
    let number = number
        .parse()
        .map_err(|_| format!("{number} is too large for a rule's number"))?;
    Ok((area, number))
}

/// Whether a trigger's condition holds in the state, or what's wrong with
/// each of its clauses that has a problem. A condition joins clauses with
/// " or ", and a clause is `true`, a fact that is true or false, `not` and
/// such a fact, or a count `>=` a number (`triggers.toml`).
fn holds(when: &str, state: &[(String, Fact)]) -> Result<bool, Vec<String>> {
    let mut holds = false;
    let mut problems = Vec::new();
    // Every clause is read, even after one that holds, so that each one's
    // problem is reported.
    for clause in when
        .split(" or ")
        .map(|clause| clause.trim_matches(text::is_space))
    {
        match clause_holds(clause, state) {
            Ok(value) => holds |= value,
            Err(problem) => problems.push(problem),
        }
    }
    if problems.is_empty() {
        Ok(holds)
    } else {
        Err(problems)
    }
}

/// Whether one clause of a condition holds in the state, or what's wrong
/// with it.
fn clause_holds(clause: &str, state: &[(String, Fact)]) -> Result<bool, String> {
    let fact = |name: &str| {
        state
            .iter()
            .find(|(fact, _)| fact == name)
            .map(|(_, value)| *value)
            .ok_or_else(|| format!("unknown fact '{name}'"))
    };
    if clause == "true" {
        return Ok(true);
    }
    if let Some((name, number)) = count_clause(clause) {
        return match fact(name)? {
            Fact::Count(count) => Ok(count >= number),
            Fact::Holds(_) => Err(format!(
                "'{clause}' compares '{name}', which is true or false, not a count"
            )),
        };
    }
    if clause.contains(['<', '>', '=']) {
        return Err(format!(
            "'{clause}' compares in a way that conditions don't; a comparison reads `fact >= N`"
        ));
    }
    let (negated, name) = clause
        .strip_prefix("not ")
        .map_or((false, clause), |name| (true, name));
    match fact(name)? {
        Fact::Holds(value) => Ok(value != negated),
        Fact::Count(_) => Err(format!(
            "'{clause}' tests '{name}', which is a count, not true or false"
        )),
    }
}

/// The fact and the number of a clause of the form `fact >= N`, which the
/// prototype matched with `(\w+) >= (\d+)`, though only ASCII digits make
/// a number here.
fn count_clause(clause: &str) -> Option<(&str, u64)> {
    let (name, number) = clause.split_once(" >= ")?;
    if name.is_empty() || !name.chars().all(text::is_word) || !is_number(number) {
        return None;
    }
    // A number too large for a count is one that no fact reaches, as in the
    // prototype, whose numbers have no limit.
    Some((name, number.parse().unwrap_or(u64::MAX)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;
    use anyhow::ensure;

    /// A rule's TOML: its ID and `more`, with the other fields filled in.
    fn rule(id: &str, more: &str) -> String {
        format!(
            "[[rule]]\nid = \"{id}\"\nlevel = \"MUST\"\nstatement = \"{id}'s statement.\"\nrationale = \"{id}'s rationale.\"\nverification = \"{id}'s verification\"\ncheck = \"CI\"\nadded = \"0.1.0\"\nstatus = \"active\"\n{more}\n\n"
        )
    }

    /// Valid rules data, each file's name in `conventions/` and its text:
    /// the QA area comes before CI, and the CI rules are out of order in
    /// their file.
    fn data() -> Vec<(String, String)> {
        let triggers = [
            ("always", "true", 0),
            ("open", "open", 1),
            ("team", "maintainers >= 2", 2),
            ("shipped", "shipped", 3),
        ];
        let files = [
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
        ];
        files
            .into_iter()
            .map(|(name, text)| (name.to_owned(), text))
            .collect()
    }

    /// A scratch checkout of git that holds the data in `conventions/`.
    fn checkout(data: &[(String, String)]) -> Result<Scratch> {
        let scratch = Scratch::new()?;
        let folder = scratch.path().join("conventions");
        for (name, text) in data {
            let path = folder.join(name);
            fs::create_dir_all(path.parent().context("a file without a folder")?)?;
            fs::write(path, text)?;
        }
        let status = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(scratch.path())
            .status()
            .context("couldn't run git")?;
        ensure!(status.success(), "git init failed");
        Ok(scratch)
    }

    /// The data, read, then checked.
    fn load(data: &[(String, String)]) -> Result<Result<Rules, Vec<String>>> {
        let scratch = checkout(data)?;
        Ok(Rules::read(scratch.path())?.checked())
    }

    /// Adds `more` to the end of the file `name` in the data.
    fn append(data: &mut [(String, String)], name: &str, more: &str) {
        for (file, text) in data {
            if file == name {
                text.push_str(more);
            }
        }
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
        let triggers: String = [
            ("always", "true"),
            ("lost", "nobody or nothing"),
            ("compares", "open >= 2"),
            ("tests", "not maintainers"),
        ]
        .iter()
        .map(|(id, when)| {
            format!(
                "[[trigger]]\nid = \"{id}\"\ntitle = \"{id}\"\nwhen = \"{when}\"\norder = 9\n\n"
            )
        })
        .collect();
        append(&mut data, "triggers.toml", &triggers);
        append(
            &mut data,
            "moments.toml",
            "[[moment]]\nid = \"setup\"\ntitle = \"Again\"\norder = 1\n",
        );
        append(
            &mut data,
            "areas.toml",
            "[[area]]\nprefix = \"CI\"\ntitle = \"Again\"\ngroup = \"\"\nintro = \"\"\nnotes = \"\"\n",
        );
        let rules = [
            rule("CI-02", "trigger = \"always\"\nwho = \"tool\""),
            rule("CI-2", "trigger = \"always\"\nwho = \"tool\""),
            rule("XX-01", "trigger = \"always\"\nwho = \"tool\""),
            rule("QA-1a", "trigger = \"always\"\nwho = \"tool\""),
            rule("QA02", "trigger = \"always\"\nwho = \"tool\""),
            // The prototype's numbers have no limit, so a u64's do.
            rule("QA-4294967296", "trigger = \"always\"\nwho = \"tool\""),
            rule("QA-18446744073709551616", "trigger = \"always\"\nwho = \"tool\""),
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
        .concat();
        append(&mut data, "rules/QA.toml", &rules);
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
                "trigger lost: unknown fact 'nothing'",
                "trigger compares: 'open >= 2' compares 'open', which is true or false, not a count",
                "trigger tests: 'not maintainers' tests 'maintainers', which is a count, not true or false",
                "CI-02: duplicate ID",
                "CI-2: the same area and number as CI-02",
                "XX-01: no area has the prefix XX",
                "QA-1a: an ID is an area's prefix, a hyphen and a number",
                "QA02: an ID is an area's prefix, a hyphen and a number",
                "QA-18446744073709551616: 18446744073709551616 is too large for a rule's number",
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
            ("maintainers >= 18446744073709551616", false),
            ("open or maintainers >= 2", false),
            ("open or shipped", true),
            // The prototype trimmed each clause.
            ("  open   or shipped  ", true),
        ] {
            assert_eq!(holds(when, &state), Ok(expected), "{when}");
        }
        let comparison = |clause: &str| {
            format!(
                "'{clause}' compares in a way that conditions don't; a comparison reads `fact >= N`"
            )
        };
        for (when, problems) in [
            ("nobody", vec!["unknown fact 'nobody'".to_owned()]),
            (
                "open and shipped",
                vec!["unknown fact 'open and shipped'".to_owned()],
            ),
            // A clause after one that holds is still read, and each bad
            // clause is reported.
            (
                "shipped or nobody",
                vec!["unknown fact 'nobody'".to_owned()],
            ),
            (
                "nobody or nothing",
                vec![
                    "unknown fact 'nobody'".to_owned(),
                    "unknown fact 'nothing'".to_owned(),
                ],
            ),
            ("not  open", vec!["unknown fact ' open'".to_owned()]),
            (
                "open >= 1",
                vec!["'open >= 1' compares 'open', which is true or false, not a count".to_owned()],
            ),
            (
                "maintainers",
                vec![
                    "'maintainers' tests 'maintainers', which is a count, not true or false"
                        .to_owned(),
                ],
            ),
            ("maintainers>=2", vec![comparison("maintainers>=2")]),
            ("maintainers > 1", vec![comparison("maintainers > 1")]),
            (
                "not maintainers >= 2",
                vec![comparison("not maintainers >= 2")],
            ),
        ] {
            assert_eq!(holds(when, &state), Err(problems), "{when}");
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
        fs::write(&path, "fact = true\nother = -1\n")?;
        let Err(error) = read_state(&path) else {
            bail!("`other = -1` passed");
        };
        let error = format!("{error:#}");
        assert!(
            error.contains("line 2") && error.contains("a fact, which is true, false or a count"),
            "{error}"
        );
        Ok(())
    }

    /// A file checked out with CRLF line endings, as git does on Windows,
    /// reads as it does with LF, multi-line strings included.
    #[test]
    fn reads_crlf_line_endings_as_lf() -> Result<()> {
        let scratch = Scratch::new()?;
        let path = scratch.path().join("moments.toml");
        fs::write(
            &path,
            "[[moment]]\r\nid = \"setup\"\r\ntitle = \"\"\"\r\nWhen you\r\nset up\"\"\"\r\norder = 0\r\n",
        )?;
        let file: MomentFile = read(&path)?;
        let [moment] = file.moment.as_slice() else {
            bail!("expected one moment");
        };
        assert_eq!(moment.title, "When you\nset up");
        Ok(())
    }

    /// A rule file that git ignores, such as a local draft, stays out of the
    /// views.
    #[test]
    fn leaves_out_rule_files_that_git_ignores() -> Result<()> {
        let mut data = data();
        data.push((
            "rules/DRAFT.toml".to_owned(),
            rule("QA-09", "trigger = \"always\"\nwho = \"tool\""),
        ));
        let scratch = checkout(&data)?;
        assert!(
            Rules::load(scratch.path())?
                .rules
                .iter()
                .any(|rule| rule.id == "QA-09")
        );
        fs::write(
            scratch.path().join(".git").join("info").join("exclude"),
            "/conventions/rules/DRAFT.toml\n",
        )?;
        assert!(
            Rules::load(scratch.path())?
                .rules
                .iter()
                .all(|rule| rule.id != "QA-09")
        );
        Ok(())
    }

    #[test]
    fn checks_the_views_against_the_data() -> Result<()> {
        let scratch = checkout(&data())?;
        let root = scratch.path();
        let stale_views =
            || -> Result<Vec<String>> { stale(root, &views::render(&Rules::load(root)?)) };
        let missing = stale_views()?;
        assert_eq!(missing.len(), 3, "{missing:?}");
        assert!(missing.iter().all(|problem| problem.contains(": missing;")));
        generate(root)?;
        assert_eq!(stale_views()?, Vec::<String>::new());
        check(root)?;
        // A file in the views' folder that the engine doesn't write, such as
        // a view it no longer generates.
        let old = root.join(VIEWS).join("OLD.md");
        fs::write(&old, "An old view.\n")?;
        let extra = stale_views()?;
        assert_eq!(extra.len(), 1, "{extra:?}");
        assert!(
            extra[0].ends_with("OLD.md: not a view that the rules data generates; remove it"),
            "{extra:?}"
        );
        assert!(check(root).is_err());
        fs::remove_file(&old)?;
        // A second maintainer starts CI-10 and changes the state's summary
        // on NOW.md's sixth line.
        fs::write(
            root.join("conventions").join("state.toml"),
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
        assert!(check(root).is_err());
        generate(root)?;
        check(root)
    }

    #[test]
    fn finds_the_first_line_that_differs() {
        assert_eq!(first_difference(b"a\nb\n", b"a\nb\n"), None);
        assert_eq!(first_difference(b"a\nb\n", b"a\nc\n"), Some(2));
        assert_eq!(first_difference(b"a\nb\n", b"a\nb\nc\n"), Some(3));
        // A missing final newline is on the last line, and an extra blank
        // line is a line of its own.
        assert_eq!(first_difference(b"a\nb\n", b"a\nb"), Some(2));
        assert_eq!(first_difference(b"a\nb\n\n", b"a\nb\n"), Some(3));
        assert_eq!(first_difference(b"a\r\nb\n", b"a\nb\n"), Some(1));
        assert_eq!(first_difference(b"", b"a\n"), Some(1));
    }

    /// xtask's test data for the golden views, each file's name in
    /// `conventions/` and its text.
    fn testdata() -> Result<Vec<(String, String)>> {
        let folder = crate::workspace_root()?.join("xtask/testdata/rules");
        let mut data = Vec::new();
        for sub in ["", "rules"] {
            for entry in fs::read_dir(folder.join(sub))? {
                let path = entry?.path();
                if path
                    .extension()
                    .is_some_and(|extension| extension == "toml")
                {
                    let name = Path::new(sub).join(path.file_name().context("a nameless file")?);
                    data.push((
                        name.to_string_lossy().into_owned(),
                        fs::read_to_string(&path)?,
                    ));
                }
            }
        }
        Ok(data)
    }

    /// The views of xtask's test data, at one maintainer and at two, are
    /// the ones the prototype wrote, byte for byte. The data takes the paths
    /// that the conventions' own data doesn't take yet: later versions that
    /// hold, one rule with two of them, a rule that waits while its later
    /// version holds, moments and triggers out of order and tied, facts out
    /// of their names' order, an area without rules and a multi-line
    /// string, which a Windows checkout gives CRLF line endings. When the
    /// views' text changes on purpose, `UPDATE_GOLDEN_VIEWS=1 cargo test`
    /// rewrites `xtask/testdata/rules/views/`, and its diff shows the change.
    #[test]
    fn writes_the_views_the_prototype_wrote_for_the_test_data() -> Result<()> {
        let goldens = crate::workspace_root()?.join("xtask/testdata/rules/views");
        let update = std::env::var_os("UPDATE_GOLDEN_VIEWS").is_some();
        for (state, maintainers) in [
            ("one-maintainer", "maintainers = 1"),
            ("two-maintainers", "maintainers = 2"),
        ] {
            let mut data = testdata()?;
            for (name, text) in &mut data {
                if name == "state.toml" {
                    ensure!(
                        text.contains("maintainers = 1"),
                        "the test data's state.toml no longer has one maintainer"
                    );
                    *text = text.replace("maintainers = 1", maintainers);
                }
            }
            let scratch = checkout(&data)?;
            let views = views::render(&Rules::load(scratch.path())?);
            for (name, text) in views.files() {
                let path = goldens.join(state).join(name);
                if update {
                    fs::write(&path, text)?;
                } else if let Some(line) = first_difference(&fs::read(&path)?, text.as_bytes()) {
                    bail!(
                        "{} differs from what the test data generates, from line {line}",
                        path.display()
                    );
                }
            }
        }
        Ok(())
    }

    /// Git keeps the committed views' and the golden views' LF line endings
    /// in every checkout, Windows included, so that they compare byte for
    /// byte (.gitattributes).
    #[test]
    fn keeps_the_views_line_endings_in_every_checkout() -> Result<()> {
        let root = crate::workspace_root()?;
        let names = ["NOW.md", "CHECKED.md", "LATER.md"];
        let mut paths: Vec<String> = names.iter().map(|name| format!("{VIEWS}/{name}")).collect();
        for state in ["one-maintainer", "two-maintainers"] {
            paths.extend(
                names
                    .iter()
                    .map(|name| format!("xtask/testdata/rules/views/{state}/{name}")),
            );
        }
        for path in paths {
            let output = std::process::Command::new("git")
                .args(["check-attr", "eol", "--", &path])
                .current_dir(&root)
                .output()
                .context("couldn't run git")?;
            ensure!(
                output.status.success(),
                "git check-attr failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
            assert_eq!(
                String::from_utf8(output.stdout)?,
                format!("{path}: eol: lf\n")
            );
        }
        Ok(())
    }
}
