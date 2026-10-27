<!--
SPDX-FileCopyrightText: Kreqo contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Kreqo rules as data

The TOML files are the source of truth; the views are generated from them. The `trigger`, `who` and `moments` values are a first pass that Dara hasn't reviewed yet.

- `rules/*.toml`: the 316 rules, one file per area. Each rule has `trigger` (when it starts to apply), `who` (`tool`, `ai` or `you`), and for `you` rules, `moments` (when they come up). `[[rule.later]]` holds a version that replaces the rule at a later trigger; `[[rule.reminder]]` holds a note for one.
- `state.toml`: facts about Kreqo today. Change one, regenerate the views, and rules move between them.
- `triggers.toml`, `moments.toml`, `principles.toml`, `glossary.toml`, `areas.toml`: the rest of the data.
- `views/NOW.md`, `views/CHECKED.md` and `views/LATER.md`: generated, never edited by hand. `cargo xtask rules generate` checks the data, refusing any field it doesn't know, and writes them; `cargo xtask check rules`, which CI runs, fails when they differ from what the data generates. Until M3 changes their text, they still name `compile.py`, the Python prototype that wrote them before M2 ported it.
- `CHANGES.md`: the solo-first edits made while converting the conventions to TOML, approved on 2026-10-24 (`docs/decisions.md`).

## Writing the rules

These points go into "How this document works" when M3 brings that section into the conventions, and they apply until then. They govern the sentences a change touches, so older text follows them as it's next edited. Point 5 reaches further: when a rule changes, the change's review also checks every summary that covers it.

1. **Generality.** A statement says what holds for every project in the rule's scope. A requirement that only one app has goes in that app's README or `project.toml`.
2. **Examples.** Examples follow "such as" or "for example", don't narrow the rule, and come from more than one kind of project where they can.
3. **Once.** Each requirement is stated in one rule. Other rules refer to what it defines by its glossary term, or by its rule ID where no term fits, and never restate it. When two rules state the same requirement, it stays in the rule whose subject it is, if that rule applies by the time the requirement can matter and is at least as strong. Otherwise it stays in the rule with the earlier trigger, or the stronger level when the triggers are the same. If one rule has the earlier trigger and the other the stronger level, both texts stay until the founder decides.
4. **Outside facts.** A statement doesn't copy facts that change on someone else's schedule, such as a store's current target or a bill's progress. The statement states the requirement; the rationale links the source and can give a dated example. A version Kreqo chose, such as WCAG 2.2, stays in the statement, so changing it is a rule change.
5. **Summaries.** Area intros and notes, the glossary's definitions, the comments in `state.toml` and the trigger titles summarize the rules. A summary says what its rules cover and may say why. It may leave out levels, triggers and exceptions, but every claim it makes, including when something applies, holds under the rules.

## Levels

The "Levels" item of "How this document works", worded so that no level hides a negation in its statement. M3 brings it in with that section, after the standard BCP 14 sentence.

> **Levels.** MUST, SHOULD and MAY. A statement says what holds, prohibitions included ("never", "no"), so it reads correctly without its level. Deviating from a SHOULD requires a deviation record (DOC-06); a MUST changes only through this document. Imperatives are used sparingly: before 1.0, every MUST passes two questions, whether anyone would break it without the rule, and whether tooling could enforce it instead.
