<!--
SPDX-FileCopyrightText: Kreqo contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Kreqo rules as data

The TOML files are the source of truth; the views are generated from them. The `trigger`, `who` and `moments` values are a first pass that Dara hasn't reviewed yet.

- `rules/*.toml`: the 319 rules, one file per area. Each rule has `trigger` (when it starts to apply), `who` (`tool`, `ai` or `you`), and for `you` rules, `moments` (when they come up). `[[rule.later]]` holds a version that replaces the rule at a later trigger; `[[rule.reminder]]` holds a note for one.
- `state.toml`: facts about Kreqo today. Change one, regenerate the views, and rules move between them.
- `triggers.toml`, `moments.toml`, `principles.toml`, `glossary.toml`, `areas.toml`: the rest of the data.
- `views/NOW.md`, `views/CHECKED.md` and `views/LATER.md`: generated, never edited by hand. A Python prototype kept outside the repository checks the data and writes them until M2 ports it to `cargo xtask`; the port's output must match these files exactly.
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

## Source, scope and adoption

The items of "How this document works" that the first rule batch rewrote for the monorepo, and the sentence that follows the document's opening one, which gives its version. M3 brings them in with that section, and they apply until then.

> It is generated from the rules in the monorepo's `conventions/`, the one copy every project follows.

> **Source.** The rules live once, as data in the monorepo's `conventions/`: `rules/*.toml`, one file per area, beside the principles, glossary, areas, triggers, moments and state they use. This document, the views of what applies now and what waits for a trigger, the release checklist (REL-05), the pull-request checklist and each project's compliance summary are generated from that source, and CI validates it: no reused IDs, a verification for every rule, every Release check present in the checklist.
>
> **Scope.** Every crate, tool, app, server, website and experiment in the organization. In the monorepo, a project is a folder with its own `project.toml` (LIFE-01), every crate belongs to exactly one project, and "the project", with no project named before it, means Kreqo. Rules about the organization or the repository itself, such as accounts, the forge's settings, reviews, merges, commits, workflows and checks over every file, apply across the repository; the others, the rules for projects, apply to the projects they concern. An experiment follows the rules that apply across the repository and, of the rules for projects, only those LIFE-08 names. A repository outside the monorepo follows the profile for its kind (CI-05). A project may add stricter rules in its README or `project.toml`, never weaker ones.
>
> **Adoption.** Every project in the monorepo follows the current version, which the monorepo's CI enforces, and every new project starts from a project template through `cargo xtask new`; an experiment keeps the lint and formatting configuration its `project.toml` pins (LIFE-08). Deviations (DOC-06) cover only SHOULDs, so a new or stricter MUST lands together with the fixes it needs in every project, or waits for a trigger that phases it in. A repository outside the monorepo names the version it follows in its `project.toml` (LIFE-01), and upgrading it is a deliberate pull request.
