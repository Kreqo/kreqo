<!--
SPDX-FileCopyrightText: Kreqo contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Kreqo

This is Kreqo's monorepo: every Kreqo crate, tool and app lives here, under one copy of the conventions and the tooling that enforces them. Its foundation is being built now, before any app code.

- Decisions: docs/decisions.md records Dara's decisions and the working rules this file summarizes; read it before working on the foundation. In Dara's local checkout, docs/context/BRIEF.md holds the full setup plan; read it at the start of any session on the foundation's design.
- Setup material is never committed or pushed, to any branch: docs/context/, conventions/compile.py, conventions/migration/ and the root CONVENTIONS.md. .git/info/exclude lists them; never force-add them. GitHub keeps a PR's commits after the squash, so whatever is pushed stays public.
- Rules: conventions/ is the single source of truth; take rule text only from there. conventions/views/ is generated. The root CONVENTIONS.md predates the TOML and is stale; never edit it by hand. M3 generates its replacement. Rule text follows the guideline in conventions/README.md.
- Approved: the solo-first edits in conventions/CHANGES.md and the first and second rule batches, the experiment rules LIFE-08 and LIFE-09 included.
- Choices (GOV-27, principle 14): make the reversible choices your task leaves open, and list each, with the alternatives you weighed, in the PR description, where Dara can veto it; a default from the conventions, an ADR or common practice needs no decision. Rule changes (proposed with before and after) and choices that are costly or impossible to undo go to Dara first, batched, each with your recommendation, and changes on the sign-off list need Dara's sign-off (CI-03). Never apply the sign-off label.
- Layout (T1): crates/, tools/, apps/, experiments/, xtask/, conventions/, templates/, docs/. New projects come only from cargo xtask new once it exists (M8), never from copying another project.
- Code: Rust for all logic and project tasks, in cargo xtask (ARCH-01, ARCH-06). M2 ports the local Python prototype and checks the Rust output against the committed views.
- Every file or check you add names the rule it serves; anything else becomes an issue.
- Checks catch honest mistakes and shortcuts, an AI agent's included; deliberate rewrites of xtask, the workflows or tool config are review's. Every fix to a check comes with a test that fails without it (QA-09).
- Each milestone lands as one PR, which Dara squash-merges with its title and description as the commit message (CI-13, T10). Until M7 builds the AI review, a separate session runs /code-review on each PR, and its report goes on the PR, labeled with the model.
- Commits: the author and committer are Dara Kong <dara@kreqo.org> (GOV-16). Keep the Co-Authored-By trailer (GOV-03), and end each PR description with it so the squash commit keeps it. Never add Signed-off-by (GOV-08).
- PR descriptions you write are records, disclosed as AI-written, with the sections CI-12 lists until M4's template exists. In the GOV-03 block, say how AI was used and what it produced, name Dara's decisions in docs/decisions.md as the human creative input, and leave what the human verified to Dara.
- Build and test only the packages you touch; before pushing, run cargo xtask check, which runs what CI runs. In a fresh clone, run cargo xtask hooks to install the secret-scanning pre-commit and commit-msg hooks (SEC-03); sessions with several repositories skip this repository's hooks, so run it yourself. Never commit with --no-verify.
- American English. Rule text says "the forge", not GitHub.
