<!--
SPDX-FileCopyrightText: Kreqo contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Kreqo rules as data

The TOML files are the source of truth; the views are generated from them. The `trigger`, `who` and `moments` values are a first pass that Dara hasn't reviewed yet.

- `rules/*.toml`: the 316 rules, one file per area. Each rule has `trigger` (when it starts to apply), `who` (`tool`, `ai` or `you`), and for `you` rules, `moments` (when they come up). `[[rule.later]]` holds a version that replaces the rule at a later trigger; `[[rule.reminder]]` holds a note for one.
- `state.toml`: facts about Kreqo today. Change one, regenerate the views, and rules move between them.
- `triggers.toml`, `moments.toml`, `principles.toml`, `glossary.toml`, `areas.toml`: the rest of the data.
- `views/NOW.md`, `views/CHECKED.md` and `views/LATER.md`: generated, never edited by hand. A Python prototype kept outside the repository checks the data and writes them until M2 ports it to `cargo xtask`; the port's output must match these files exactly.
- `CHANGES.md`: the solo-first edits made while converting the conventions to TOML, approved on 2026-10-24 (`docs/decisions.md`).
