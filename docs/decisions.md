<!--
SPDX-FileCopyrightText: Kreqo contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Decisions

Dara's decisions on the monorepo and its foundation, oldest first, and the working rules that sessions follow while building it, so that decisions are public and written down (principle 13). The log starts in M0. In M4, its significant entries become ADRs in `docs/adr/` (DOC-04), each linked from its entry here. CLAUDE.md only summarizes this log (DOC-13).

## Working rules from the setup plan

The setup brief stays outside the repository, so the parts of its plan that sessions follow while building the foundation are recorded here.

- Every file and check names the rule it serves; anything else becomes an issue.
- Checks catch honest mistakes and shortcuts, an AI agent's included, not deliberate rewrites of xtask, the workflows or the tools' configuration, which review covers. Every fix to a check comes with a test that fails without it (QA-09).
- Existing tools come before custom code, and custom code is Rust in `cargo xtask` (ARCH-01, ARCH-06). `cargo xtask check` runs what CI runs, so a session checks its work before pushing. Sessions build and test only the packages a change touches and leave the full run to `cargo xtask check` and CI.
- When a rule's literal text is impractical, sessions build what meets its intent and propose a wording change with before and after. Rule text is never edited silently, and nothing deviates from a rule without a deviation record (DOC-06). Rule changes go to Dara in batches; the conventions stay 0.1.0 (draft), and IDs are never reused.
- New projects come only from `cargo xtask new` once it exists (M8), never from copying another project.
- Until the tooling exists, the default branch is protected by hand: pull requests only, squash merges only, no force pushes. Until M7 builds the AI review, a separate session runs `/code-review` on each pull request, and its report goes on the pull request, labeled with the model. Dara merges every pull request and signs off explicitly on changes from the sign-off list.
- A fresh clone installs the secret-scanning pre-commit and commit-msg hooks with `cargo xtask hooks` (SEC-03). Sessions with several repositories skip this repository's hooks, so they run it themselves; CI, not hooks, is what enforces. Nobody commits with `--no-verify`: when a hook can't build xtask, fix or stash that change instead.
- Until M4 adds the pull request template, a description gives the sections CI-12 lists. Its GOV-03 block says how AI was used and what it produced, names Dara's decisions in this log as the human creative input, and leaves what the human verified to Dara.
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

## 2026-10-24: the review of PR #1

Answers to the design questions that the independent review of PR #1 raised. Its findings and how each was handled are in the pull request.

### The attribute checks read Rust files as tokens

xtask depends directly on proc-macro2, already in the lockfile through clap and serde, and reads every Rust file that git tracks or would track as tokens. Comments and strings never match, and an attribute that rustfmt wraps stays whole. The same pass checks SEC-01's crate-root `#![expect(unsafe_code, reason = "ADR-NNNN")]`, which must name an existing ADR.

- **Rules:** QA-01, SEC-01, ARCH-05.

### The secret scan has no escape hatches

Betterleaks runs with its built-in rules only, whatever config the scanned tree holds, and ignores inline `betterleaks:allow` comments. A scanner config or ignore file anywhere in the tree fails the scan, until a revoked secret first needs one; CI-01 rules out removing it from history. Binary files, files marked `-diff` and merge commits are scanned too. The hook still blocks commits while xtask doesn't build, and nobody commits with `--no-verify`.

- **Rules:** SEC-03, CI-01.

### Suppressions expire within 90 days

A cargo-deny suppression's reason ends with a real date at most 90 days ahead, so that every suppression comes back for review at least once a quarter.

- **Rules:** SEC-02.

### zizmor runs its online audits in CI

CI hands zizmor the job's read-only token, so it also checks that each pinned SHA is a real commit of its action, matches its version comment and has no known vulnerability. Without a token, as on a laptop, it stays offline.

- **Rules:** CI-07, CI-09.

### What the review left for later

- **Scheduled workflows lapse:** GitHub stops a public repository's scheduled workflows after 60 days without activity, which would end SEC-02's daily scan unnoticed. A Periodic check, like the one for interaction limits, will catch it ([#3](https://github.com/Kreqo/kreqo/issues/3)).
- **LIFE-01:** the conventions version and the README badge, in M5 ([#4](https://github.com/Kreqo/kreqo/issues/4)).
- **LEGAL-08:** checking `reuse lint`'s licenses against the allowed list, in M5 ([#5](https://github.com/Kreqo/kreqo/issues/5)).
- **The views:** "enforced by tooling" follows `who = "tool"`, not whether a check exists; M3's coverage report settles it ([#6](https://github.com/Kreqo/kreqo/issues/6)).
- **QA-01's copy check:** with the first nested workspace or repository outside the monorepo ([#7](https://github.com/Kreqo/kreqo/issues/7)).
- **CI-02's semver check:** with the first library release ([#8](https://github.com/Kreqo/kreqo/issues/8)).
- **reuse's dependencies:** pinning them by hash, with the dependency bot in M5 ([#9](https://github.com/Kreqo/kreqo/issues/9)).

## 2026-10-24: the follow-up review of PR #1

Answers to the two design questions that the follow-up review of PR #1 raised, and what its fixes changed. Its findings and how each was handled are in the pull request.

### Only the root's build folder is ignored

.gitignore leaves out `/target/` at the root only, so that a source folder named `target` can't vanish from a commit. The crates step fails on a nested workspace, so none has a build folder yet; each adds its own line once nested workspaces are allowed.

- **Rules:** QA-14.

### Generated code waits for the first derive crate

Code that a derive crate generates with `quote!` can't use `expect`, since an expectation would fire in the user's crate. Until a crate generates code, the `allow` check flags these attributes like any other; the first derive crate that needs one brings Dara a proposal for QA-01.

- **Rules:** QA-01.

### The secret scan covers every path, message and ref

Betterleaks' default path prefilter is off, so `.svg` files, lock files and vendored folders are scanned, and git's rename detection is off, so a moved file is read again. The history scan runs twice, once reading binary files as text and once unpacking archives, and it also reads the commit messages and notes it covers and every tag's message. The pre-commit hook reads staged binary files whole, with their paths, and a commit-msg hook scans each message before the commit is made. A daily scan covers every branch and tag, so that a push GitHub creates no run for is still scanned within a day.

- **Rules:** SEC-03, CI-01.

### What the follow-up review left for later

- **Templates:** the member check fails on any Cargo.toml outside `members` and ignores `[workspace] exclude`, so M8's project templates need a name or rule of their own ([#10](https://github.com/Kreqo/kreqo/issues/10)).
- **ADRs behind unsafe code:** the check confirms that an `expect(unsafe_code)` names an existing ADR; review checks that the ADR covers that code.
- **Running xtask directly, or sharing a build folder:** run outside cargo, as `target/debug/xtask`, it falls back to the path it was compiled in; running it through cargo, as every hook and workflow does, avoids that. Under a build folder that several checkouts share, cargo can reuse one checkout's build for another, so a test reads the first checkout's copies of the files it includes, such as deny.toml; a build folder per checkout avoids that.
- **One test:** `ci_runs_every_step` shows that CI runs each step; the `ci` job, which requires every other job, is what makes each one block a merge.

## 2026-10-24: the second follow-up review of PR #1

Answers to the four design questions that the second follow-up review of PR #1 raised, and what its fixes changed. Its findings and how each was handled are in the pull request.

### What the checks defend against

The checks catch honest mistakes and shortcuts in ordinary code, configuration and pull request text, including an AI agent's attempts to get CI green, such as a lowered lint level, a cargo setting or a pasted token. They don't try to stop a change that deliberately rewrites xtask, the workflows or the tools' configuration: CI runs the pull request's own copies of all three, so no check can. Review catches that, and those paths get Dara's sign-off as risk-tier paths once GOV-02's risk file exists. A review finding that only such a change could exploit is recorded rather than fixed.

- **Why:** a check that a pull request can rewrite can't stop that pull request; review can.
- **Rules:** CI-03, GOV-02, QA-01, SEC-01, SEC-03.

### Tests that need the pinned tools have a step of their own

Every fix to a check comes with a test that fails without it. The tests that run xtask in throwaway git repositories live in `xtask/tests/repositories/`. A plain `cargo test` skips the ones that need Betterleaks or zizmor, and `cargo xtask check tool-tests` runs them, in CI's Linux checks job, where the pinned tools are installed, and in a full local check. Tests that need only git run on all three platforms.

- **Rules:** QA-09, CI-04.

### A pull request's title and description are scanned before it merges

The squash commit's message is the pull request's title and description (CI-13), which no hook sees, and CI-01 keeps whatever it holds. The Description workflow scans them whenever a pull request is opened, edited, reopened or synchronized, passing them through environment variables (CI-09), and its `description` job is a required check beside `ci`. A message edited in the merge dialog is scanned only once it's on the default branch.

- **Rules:** SEC-03, CI-01, CI-09, CI-13.

### Deferred findings become issues

From now on, each item that a review leaves for later becomes a forge issue, as REV-01 and the working rules ask, and this log links it. Dara sees each issue's draft before it's filed.

- **Rules:** REV-01.

### What the checks hold to now

- **Lint levels:** the attribute check flags any `warn` or `expect` of `warnings` or of a lint the workspace table denies, and any `expect` of a group that holds one, outside SEC-01's crate-root `expect`. It also reads the attributes that `doc(test(attr(…)))` gives doctests, and it reads a shebang as rustc does. The repository's `.cargo/config.toml` may hold only the `xtask` alias, since `rustflags`, `[env]` or an `include` could lower every lint or stand in for a step.
- **The secret scan:** a scan of HEAD reads the tags at HEAD only, since a tag elsewhere was scanned when it was pushed, and CI-01 keeps release tags. The daily scan reads every tag, the notes, which it fetches, and what a tag holds when it holds a blob or a tree. Messages, notes and tag messages are scanned as files named after their objects, so a finding names its commit, note or tag and keeps its fingerprint. The commit-msg hook also checks the tree, since a merge skips the pre-commit hook, and it reads a message only above git's cut line. The scans keep git's colors off, leave out the settings of `git -c` and read history without text conversion, so they need git 2.31 or later.
- **zizmor:** it also reads nested workflows and Dependabot configs, it runs even when the Check step fails, and only GH_TOKEN reaches it, with the settings that could turn its online audits off cleared.

- **Rules:** QA-01, SEC-01, SEC-03, CI-01, CI-07, CI-09.

### What the bar leaves to review

- **Crate roots:** a crate root that `#[path]` or a link mounts into another crate still counts as crate-level, and `#[path]` modules without the `.rs` extension and attribute names that `macro_rules!` builds go unchecked.
- **The workflows' own settings:** a workflow step can set `CARGO_ALIAS_*` or other settings for the steps after it, and the Audit step's xtask, which the pull request builds, sees the read-only token.
- **Text conversion in the hook:** Betterleaks runs the staged diff itself, so a textconv driver can change what the pre-commit hook reads; CI's history scan reads past it.

### What the second follow-up review left for later

- **Archives by name:** Betterleaks picks the archives it unpacks by their names, such as `.zip` and `.tar.gz`, so it doesn't read inside `.docx`, `.jar` or other files that are zip archives under another name ([#11](https://github.com/Kreqo/kreqo/issues/11)).
- **Deleted refs:** a ref that started no push run and is deleted before the daily scan escapes it, though GitHub may keep serving its commits. The forge's own push protection could cover it ([#12](https://github.com/Kreqo/kreqo/issues/12)).

## 2026-10-25: the last follow-up review of PR #1

Dara's split of the last follow-up review's findings: this pull request fixes what it owes, which is the blocking finding, the tests that passed without their fixes (QA-09), its own regression and stale text; the rest become issues or are recorded here. Its findings and how each was handled are in the pull request.

### What the checks hold to, after the last review

The repository's `.cargo/config.toml` may hold the `xtask` alias only as `run --package xtask --locked --`, since anything more, such as `--config`, could bring back the `rustflags` and `[env]` the file may not hold. The commit-msg hook cuts a message only at git's own cut line, with the comment string git is set to use.

- **Rules:** QA-01, SEC-01, SEC-03.

### What the bar leaves to review, after the last review

- **zizmor's ignores:** a `# zizmor: ignore[…]` comment in a workflow, or a `disable` or `ignore` entry in `.github/zizmor.yml`, lowers an audit the way an attribute lowers a lint, and only a change to the workflows or the tools' configuration can add one.
- **Reasons that only look like words:** a suppression's reason passes with an invisible letter, such as U+3164, or a doubled `until`.
- **A cut line on purpose:** a `git commit -m` message that carries git's exact cut line stops the commit-msg hook there, though git keeps what follows; CI's scan of the pushed commit reads it.
- **A cargo config in a subfolder:** cargo reads one only when it runs in that folder, so only local runs see it; CI runs from the root.

### What the last follow-up review left for later

- **Lint groups in the workspace table:** a group allowed at a higher priority than a denied lint it holds lowers that lint in every crate ([#13](https://github.com/Kreqo/kreqo/issues/13)).
- **Cargo settings that weaken tests:** `overflow-checks` or `debug-assertions` turned off in the dev or test profile, or a member's `rust-version` below the workspace's ([#14](https://github.com/Kreqo/kreqo/issues/14)).
- **The Description run's log:** GitHub prints a step's environment in its log, so the log repeats the title and description; reading them from `GITHUB_EVENT_PATH` would keep them out ([#15](https://github.com/Kreqo/kreqo/issues/15)).
- **Tags of trees and of tags:** a tagged tree's files are scanned without their paths, and a tag of a tag has its inner message unread ([#16](https://github.com/Kreqo/kreqo/issues/16)).
- **A byte-order mark before a shebang:** the attribute check doesn't skip one, so an honest file that starts with both fails the crates step ([#17](https://github.com/Kreqo/kreqo/issues/17)).

### What the review of the last six commits left as is

- **The hook's comment string:** the commit-msg hook reads core.commentString even on git before 2.45, which ignores it; it reads `auto` only in lowercase, where git accepts any case; and it prefers core.commentString, where git 2.45 and later take whichever of the two keys was set last. Each way, the hook reads more of an honest message than git keeps and may refuse a commit that git would accept; only a message that carries a cut line on purpose, as recorded above, could get past it.
- **The list form of the `xtask` alias:** cargo accepts `xtask = ["run", "--package", "xtask", "--locked", "--"]`, but the check refuses every form but the one string, so it fails closed.
- **A test that reads the machine's cargo home:** `reads_the_cargo_configs_in_and_above_the_checkout` also reads the machine's CARGO_HOME, so on a machine whose cargo home config defines a `clippy` alias, it could pass without the walk above the checkout. CI has no such config.

## 2026-10-26: before PR #1 merges

### CI-07 names an action's repository

Approved from the first rule batch, since M1's workflows need it, as QA-01 and SEC-01 were. A workflow's `uses:` names an action by its repository and can't take a URL, so "fully qualified URL" couldn't be followed as written; the SHA pins already meet the rule's intent. The TOML holds the new text with a history entry, and the views are regenerated.

- **Rules:** CI-07.

The text it replaced:

- **CI-07, statement:** Workflow steps reference actions by fully qualified URL and full commit SHA, with the version as a comment, and container images by digest.
