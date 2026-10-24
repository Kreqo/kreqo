<!--
SPDX-FileCopyrightText: Kreqo contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Decisions

Dara's decisions on the monorepo and its foundation, oldest first, and the working rules that sessions follow while building it, so that decisions are public and written down (principle 13). The log starts in M0. In M4, its significant entries become ADRs in `docs/adr/` (DOC-04), each linked from its entry here. CLAUDE.md only summarizes this log (DOC-13).

## Working rules from the setup plan

The setup brief stays outside the repository, so the parts of its plan that sessions follow while building the foundation are recorded here.

- Every file and check names the rule it serves; anything else becomes an issue.
- Existing tools come before custom code, and custom code is Rust in `cargo xtask` (ARCH-01, ARCH-06). `cargo xtask check` runs what CI runs, so a session checks its work before pushing. Sessions build and test only the packages a change touches and leave the full run to `cargo xtask check` and CI.
- When a rule's literal text is impractical, sessions build what meets its intent and propose a wording change with before and after. Rule text is never edited silently, and nothing deviates from a rule without a deviation record (DOC-06). Rule changes go to Dara in batches; the conventions stay 0.1.0 (draft), and IDs are never reused.
- New projects come only from `cargo xtask new` once it exists (M8), never from copying another project.
- Until the tooling exists, the default branch is protected by hand: pull requests only, squash merges only, no force pushes. Until M7 builds the AI review, a separate session runs `/code-review` on each pull request, and its report goes on the pull request, labeled with the model. Dara merges every pull request and signs off explicitly on changes from the sign-off list.
- A fresh clone installs the secret-scanning pre-commit hook with `cargo xtask hooks` (SEC-03). Sessions with several repositories skip this repository's hooks, so they run it themselves; CI, not hooks, is what enforces.
- Everything is in American English, and rule text says "the forge", not GitHub.

## 2026-10-24: setup

Answers to the questions asked before M1.

### Setup material is never committed

`docs/context/` (the handoff brief from the conversation that designed the conventions, and drafts written for separate repositories), the Python prototype of the rules engine (`conventions/compile.py`, `conventions/migration/`) and the pre-TOML `CONVENTIONS.md` stay in Dara's local checkout. `.git/info/exclude` keeps them out of git, and Dara backs them up outside the repository. Nothing private is pushed to any branch: GitHub keeps a pull request's commits after a squash merge, even once its branch is deleted. M2 checks the Rust port of the engine against the committed views instead of running the Python, and M3 generates the new `CONVENTIONS.md`. What later milestones need from the brief moves into this log and forge issues.

- **Why:** a clean history, and the brief holds private context. With one maintainer, CI-01 allows no history rewrite, so whatever is committed stays.
- **Rules:** CI-01, ARCH-06.

### Each milestone lands as one pull request

An empty root commit opens the repository. Each milestone of foundation v0.1 then lands as one pull request, squash-merged as one commit once CI and the AI review pass; M0 and M1 share the first. Every commit on the default branch, the first included, meets a pull request's standard.

- **Why:** every commit on the default branch is usable, reviews stay small, and when the time box ends, what's merged already works.
- **Rules:** CI-01, CI-03, CI-13.

### The repository is public from its first commit (T7)

While the project is closed to outside contributions, interaction limits restrict issues, comments and pull requests to collaborators. They last at most six months, so renewing them is a Periodic check, and CONTRIBUTING says so.

- **Why:** transparent by default (principle 13), and GitHub's standard runners, macOS and Windows included, are free for public repositories.

### The solo-first edits are approved

All 19 edits in `conventions/CHANGES.md`, which the TOML already holds. Twelve contributor and community rules wait for `open-to-contributions`: GOV-01, GOV-08, GOV-09, GOV-10, GOV-12, GOV-15, GOV-18, GOV-19, GOV-21, REV-03, LEGAL-17 and BIZ-06. CI-03, CI-18, GOV-02, GOV-04, GOV-05, GOV-06 and GOV-14 have versions for one person. Since GOV-08 waits, CI has no DCO check yet.

- **Why:** an independent AI review counts as review, Dara signs off only on a short list, and rules about contributors wait until there are contributors.

### Every design choice goes to Dara

For now, agents don't make reversible choices on their own. They bring every design choice to Dara, batched, each with a recommendation first, and skip choices with a conventional answer. Rule changes, anything irreversible and changes on the sign-off list (CI-03) go to Dara in any case.

- **Why:** Dara's preference for now. Two proposed policies, defaults over decisions and reversibility decides who decides, are drafted for the first rule batch, so this may change.

### An ADR stands in for an RFC while the project is closed

Until the project opens to outside contributions, the RFC that BIZ-01 (license changes) and REV-04 (design disputes) require is an ADR, since RFCs (GOV-01) wait for outside contributions. A license change still needs Dara's sign-off (CI-03). The wording goes into the first rule batch.

- **Rules:** BIZ-01, REV-04, GOV-01.

### The first rule batch is drafted for approval

Each change comes with before and after: monorepo wording for the rules written for one repository per project; solo versions of LIFE-02, PRIV-08 and CI-01, which still need a second person; the ADR stand-in above; and the two policies as a principle or rule. A fresh session reviews the batch before it goes to Dara.

### Experiments are supported in v0.1

LIFE-08 and LIFE-09, drafted for separate repositories, are revised for the solo model and the monorepo, then go to Dara for approval, so foundation v0.1 supports experiments under `experiments/`, such as the UI toolkit bake-off.

### The AI review runs outside CI (T3)

`cargo xtask review <PR>` runs `claude -p` with the `kreqo-review` skill in a fresh context and posts its report through a custom GitHub App that can only read contents and write pull-request comments. CI checks deterministically that a report exists for the pull request's latest commit, and a batch run covers pull requests without one, the dependency bot's included. The App's key stays on Dara's machine, so no session can post a review of its own change.

- **Why:** Dara asked to keep AI checks out of CI.
- **Rules:** CI-03, CI-18.

### The other design defaults are accepted

- **T1:** the repository is `kreqo`. `crates/` holds shared libraries, `tools/` command-line tools, `apps/` the apps, each with its core, CLI and later its shell, and `experiments/` the experiments; `xtask/`, `conventions/`, `templates/` and `docs/` hold the tooling, the rules, the project templates and the documentation.
- **T2:** each project's metadata lives in a `project.toml` in its folder: stage, kind, license group, platform tiers, whether it takes the stricter lints, risk-tier paths, project facts and, once the PLAT-12 ADR fixes it, an app's ID. Every crate belongs to exactly one project, which a check confirms.
- **T4:** Dara signs off with a label, verified by a required job that checks the label's actor is Dara's account and that no app applied it. Agents never apply it.
- **T5:** forge settings are rulesets kept as JSON in the repository and audited on a schedule through the forge's API.
- **T6:** the dependency bot is Renovate or Dependabot, whichever covers Cargo, SHA-pinned actions with version comments, `rust-toolchain.toml`, grouping, and a minimum release age that security fixes skip. The pick goes to Dara with a recommendation.
- **T8:** rule IDs freeze when Dara tags `conventions-v0.1.0` at the end of foundation v0.1.
- **T9:** the Claude Doc "Conventions" is retired once the TOML is the source, with a note pointing to the repository.
- **T10:** Dara merges every pull request in v0.1.

### Documentation, art, icons and media are CC BY 4.0

LEGAL-08's default license for user documentation, art, icons and media.

- **Why:** the most reusable choice. It fits GPL apps and MIT OR Apache-2.0 crates with attribution alone, and people who share content built on Kreqo's art aren't pulled into share-alike; the trademark, not copyleft, protects the brand. Licenses only get freer (BIZ-01), so it can't tighten later.
- **Rules:** LEGAL-08, which records its choice in an ADR.

### Commits carry Dara's project address

Commits are authored and committed as `Dara Kong <dara@kreqo.org>`. Agents keep the `Co-Authored-By` trailer (GOV-03) and never add `Signed-off-by` (GOV-08).

- **Why:** a project address outlives any forge and keeps personal addresses out of public history.
- **Rules:** GOV-16.

### Project mail is forwarded

`security@kreqo.org` (SEC-04), `conduct@kreqo.org` (GOV-06) and `dara@kreqo.org` forward to Dara's inbox through the registrar. Until a mailbox is needed, replies come from Dara's own address or, for security, through the forge's private reports.

### Foundation v0.1 ends on Friday 2026-11-20

Whatever is unfinished on that date is recorded as deferred, and v0.1 closes anyway.

- **Why:** so the foundation can't become the project. Perfection is not a merge criterion (REV-01).

## 2026-10-24: M1

Answers to M1's design questions.

### M1 builds the milestone's row

The root workspace, `cargo xtask` with `check`, `hooks` and `pre-commit`, the pinned toolchain, the workspace lints, cargo-deny, REUSE, xtask's `project.toml`, and CI with the secret scanner and zizmor. Crates inherit the workspace lint table instead of carrying a copied block, as the brief plans for QA-01; its new wording goes to Dara before the M0 and M1 pull request merges. `project.toml` starts with `kind`, `stage` and `license`, and other fields join it with the checks that use them. Later: the Nix flake (PLAT-04) and a platform matrix generated from `project.toml` in M5; per-group license runs and the check that no MIT OR Apache-2.0 crate depends on a GPL crate with the first GPL crate (LEGAL-03); stricter lints for parser, importer and sync crates with the first such crate; `clippy.toml` with the first project-wide ban (QA-01).

- **Rules:** QA-01, SEC-01, CI-16, REL-04, LEGAL-01, LEGAL-02, LEGAL-03, SEC-02, SEC-03, PRIV-09, CI-02, CI-04, CI-07, CI-08, CI-09.

### The toolchain is Rust 1.99.1, and crates support the two releases before it

`rust-toolchain.toml` pins Rust 1.99.1 with rustfmt and Clippy. The workspace uses edition 2024 and resolver 3. REL-04's MSRV window is the latest stable release minus two, so crates declare 1.97 today. xtask depends on clap, anyhow, serde, toml and cargo_metadata: maintained, widely used and permissively licensed crates, the first rung of ARCH-05's ladder.

- **Rules:** CI-16, REL-04, ARCH-05.

### Files name Kreqo contributors as copyright holders

Every file that can carry a header reads `SPDX-FileCopyrightText: Kreqo contributors` and names its license, so no name is added file by file, and history records who wrote what; no AI is named (LEGAL-09). Code, configuration and workflows are MIT OR Apache-2.0; the conventions, the documentation and CLAUDE.md are CC BY 4.0. REUSE.toml covers the two generated cases: Cargo.lock (CC0-1.0) and the views of the rules (CC BY 4.0). Dependencies of MIT OR Apache-2.0 crates may use MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, CC0-1.0, Unlicense, BSL-1.0 and MPL-2.0.

- **Rules:** LEGAL-02, LEGAL-03, LEGAL-08, LEGAL-09.

### CI uses three pinned actions and pinned tools

CI uses only actions/checkout, Swatinem/rust-cache and taiki-e/install-action, each pinned by full commit SHA, on ubuntu-24.04, windows-2025 and macos-15. Its tools are pinned too: cargo-deny 0.20.2, zizmor 1.31.1, reuse 6.2.0 and Betterleaks 1.9.0, which CI checks against the SHA-256 listed in its release's signed checksums file. The versions live in `xtask/src/tools.rs`: the workflows read them from `cargo xtask tools`, and `cargo xtask check` fails on a machine that has other versions. zizmor runs offline and requires every action to be pinned by SHA, GitHub's own included.

- **Rules:** CI-07, CI-08, CI-09, SEC-03.

### What cargo-deny blocks

SEC-02's written policy: an advisory that a crate is vulnerable, unsound or yanked blocks merges and releases, whatever its severity, and an unmaintained crate blocks when the workspace depends on it directly. Every suppression gives its reason and an expiry date. cargo-deny 0.20 has no expiry of its own, so ignored advisories and skipped duplicates end their reason with `until YYYY-MM-DD`, which `cargo xtask check deny` enforces. A license exception can't carry a reason in cargo-deny 0.20, so a comment above it gives both, and review checks it. PRIV-09's ban list starts with the crates.io clients for Sentry, PostHog, Segment, RudderStack, Bugsnag, Rollbar, Honeybadger, Mixpanel and Amplitude.

- **Rules:** SEC-02, PRIV-09.

### QA-01 and SEC-01 fit the monorepo

The first two changes of the first rule batch, approved after a fresh-context review whose five findings were fixed first. The TOML holds the new text, each rule with a history entry. QA-01: every crate inherits one lint table, nested workspaces and repositories outside the monorepo carry generated copies of it, an `allow` attribute, inner or outer, fails CI, and the root `clippy.toml` holds the bans for every crate. SEC-01's verification names the workspace lint table. LIFE-08's revision decides whether a frozen experiment's copy is exempt from the copy check.

- **Why:** a copied block served one repository per project; in the monorepo, crates inherit one table, and only workspaces that can't inherit it carry a copy.
- **Rules:** QA-01, SEC-01.

The text it replaced:

- **QA-01, statement:** Code is formatted with rustfmt and passes Clippy; warnings fail CI. Each crate carries the template's versioned lint block verbatim between marker comments, and CI diffs it against the template. Suppressions use `#[expect(lint, reason = "…")]`; a bare `#[allow]` fails CI. The block enforces SEC-01 and QA-02: `undocumented_unsafe_blocks` and `missing_safety_doc` workspace-wide; `unwrap_used`, `expect_used`, `panic` and `indexing_slicing` in parser, importer and sync crates. Project-wide bans use `disallowed-methods` with a reason each.
- **QA-01, rationale:** A verbatim block diffed by CI keeps lints identical across crates, and `expect` reports suppressions that no longer fire.
- **QA-01, verification:** CI: rustfmt, Clippy, lint-block diff, `allow` grep
- **SEC-01, verification:** CI: lint block (QA-01) with `undocumented_unsafe_blocks`; `expect` reason resolved to an ADR; Miri job
