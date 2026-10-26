<!--
SPDX-FileCopyrightText: Kreqo contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Solo-first edits

Made on 2026-10-23 while converting CONVENTIONS 0.1.0 (draft) to TOML. Every other rule is unchanged,
apart from its phase, now a trigger (see triggers.toml), and the new `who` and `moments` fields.

19 rules changed:

## Rules that wait until there are other people

These rules govern contributors, maintainers and a community. Their text is unchanged; they now apply
before you open the project to outside contributions, instead of from the first commit or the first release.

| Rule | Was | Now |
|---|---|---|
| REV-03 | first-release | open-to-contributions |
| GOV-01 | always | open-to-contributions |
| GOV-08 | always | open-to-contributions |
| GOV-09 | always | open-to-contributions |
| GOV-10 | always | open-to-contributions |
| GOV-12 | always | open-to-contributions |
| GOV-15 | first-release | open-to-contributions |
| GOV-18 | first-release | open-to-contributions |
| GOV-19 | first-release | open-to-contributions |
| GOV-21 | always | open-to-contributions |
| LEGAL-17 | first-release | open-to-contributions |
| BIZ-06 | first-release | open-to-contributions |

## Rules with a solo version now

Each gets a version for a one-person project. Where noted, the original version, or an adapted one
shown under the rule, returns at its trigger.

### CI-03

- Statement rewritten
- Rationale rewritten
- Verification rewritten
- Check rewritten
- An adapted version of the original, shown below, replaces it at the trigger 'second-maintainer'

**Before:** Every change has a human reviewer other than its author; AI tools never count as reviewers. Changes written by an AI agent are reviewed by the founder. While there is one maintainer, the founder's hand-written changes are exempt: they wait 24 hours after the PR opens, pass the review checklist, are labeled `self-merged`, and are re-reviewed before release.

**Now:** Every change is reviewed by a reviewer other than its author before it merges. While there is one maintainer, an independent AI review counts: one run by a session or tool that didn't write the change, attached to the pull request. Changes on the sign-off list also need the founder's sign-off: anything that needs an ADR, risk-tier paths (GOV-02), licensing, user data formats and migrations, releases, and anything published in the project's name. The founder's hand-written changes get the same AI review.

*Why, before:* A second human catches what the author can't see; a bot approval is not that.

*Why, now:* A second look catches what the author can't see; with one maintainer, an independent AI review is the second look available.

**Later, at the trigger 'second-maintainer':** Every change has a human reviewer other than its author; AI tools never count as reviewers (CI-18). Changes written by an AI agent are reviewed by a maintainer who didn't direct the agent, and risk-tier paths follow CI-15.

### CI-18

- Statement rewritten
- Rationale rewritten
- Verification rewritten
- Its original version returns at the trigger 'second-maintainer'

**Before:** AI review bots run from a separate, clearly labeled bot account, never count as an approval, and block a PR only when a human endorses their comment.

**Now:** AI reviews run from a separate, clearly labeled bot identity and name the model that ran them. While there is one maintainer, an independent AI review counts as the review CI-03 requires, and a finding it marks as blocking holds the merge until it is fixed or the founder dismisses it with a reason.

*Why, before:* A bot comment is advice; Scorecard and reviewers alike don't count it as code review.

*Why, now:* A labeled bot keeps the record honest about who reviewed what; with one maintainer, its findings are the second opinion.

### GOV-02

- Statement rewritten
- Verification rewritten
- Its original version returns at the trigger 'second-maintainer'

**Before:** The author of a change owns its intent, specification, tests and consequences, and can explain what it does and why, whether or not AI wrote the code. Every change passes CI and human review. Changes to risk-tier paths (`unsafe`, cryptography, data formats and migrations, sync, security-sensitive code, the plugin boundary, the public API of shared crates) get line-level human review; other changes get intent-level review plus the automated gates. Each project lists its risk-tier paths in a machine-readable risk file that the review tooling enforces; once CI-15 applies, that file is CODEOWNERS.

**Now:** The author of a change owns its intent, specification, tests and consequences, and can explain what it does and why, whether or not AI wrote the code; for an agent's change, the author is the person who directed it. Every change passes CI and review (CI-03). While there is one maintainer, changes to risk-tier paths get a line-level AI review and the founder's sign-off on intent; other changes get the AI review plus the automated gates. Each project lists its risk-tier paths in a machine-readable risk file that the review tooling enforces.

### GOV-04

- Statement rewritten
- Rationale rewritten
- Verification rewritten

**Before:** Before contributing upstream, contributors check the upstream's AI policy in the dated table below, re-verified at the time, and follow it. Where AI assistance is banned, the contribution is written by hand. Disclosure uses the upstream's own format, never Kreqo's trailer where the upstream forbids it.

**Now:** Before contributing upstream, contributors check the upstream's AI policy in the conventions' dated table of upstream AI policies, re-verified at the time, and follow it. A named human opens every upstream issue and pull request, writes its text and answers its review; an agent never acts in another project on its own. Where AI assistance is banned, the contribution is written by hand. Disclosure uses the upstream's own format, never Kreqo's trailer where the upstream forbids it.

*Why, before:* Upstream policies differ and change; a Kreqo trailer is forbidden in several of them.

*Why, now:* Upstream policies differ and change, a Kreqo trailer is forbidden in several, and upstream maintainers talk to accountable people.

### GOV-05

- Statement rewritten
- Rationale rewritten
- Waits for the trigger 'open-to-contributions' (was 'always')

**Before:** A named human opens every pull request and issue, writes its description (GOV-14) and answers its review, in Kreqo's own repositories and upstream. An agent may push commits to a branch for its human operator, and never acts in another project on its own. Undisclosed agent activity leads to a ban under the code of conduct.

**Now:** A named human opens every pull request and issue in Kreqo's repositories, writes its description (GOV-14) and answers its review. An agent may push commits to a branch for its human operator. Undisclosed agent activity leads to a ban under the code of conduct.

*Why, before:* Maintainers and upstreams talk to accountable people, not to agents.

*Why, now:* Contributors and maintainers talk to accountable people, not to agents.

### GOV-06

- Statement rewritten

**Before:** The code of conduct is [Contributor Covenant 3.0](https://www.contributor-covenant.org/version/3/0/code_of_conduct/), with its reporting section completed as GOV-21 requires. Its scope covers the forge, chat, forum and events; app names, descriptions, screenshots and bundled content; and content an AI agent posts under a person's name.

**Now:** The code of conduct is [Contributor Covenant 3.0](https://www.contributor-covenant.org/version/3/0/code_of_conduct/), with a private reporting address; GOV-21 completes its reporting section before the project opens to outside contributions. Its scope covers the forge, chat, forum and events; app names, descriptions, screenshots and bundled content; and content an AI agent posts under a person's name.

### GOV-14

- Statement rewritten
- Rationale rewritten
- Verification rewritten
- Its original version returns at the trigger 'open-to-contributions'

**Before:** Text written for people (issues, PR titles and descriptions, reviews, RFCs, chat) is written by its human author. AI may translate or proofread, with disclosure, and quoted tool output is marked as such. Unedited AI output is never posted to a person. User documentation drafted with AI ships only after human revision, with disclosure (GOV-03). Branch commit messages are outside this rule, because the squash commit that lands carries the human-written PR title and description (CI-13).

**Now:** Text that speaks for a person or for the project, such as replies in issues and reviews, chat, forum posts, announcements and RFCs, is written by its human author. AI may translate or proofread, with disclosure, and quoted tool output is marked as such. Unedited AI output is never posted as a person's words. User documentation drafted with AI ships only after human revision, with disclosure (GOV-03). Pull request and issue descriptions that an agent writes for its operator are records, disclosed as AI-written.

*Why, before:* People want to talk to people; every peer studied restricts AI-written text to humans.

*Why, now:* People want to talk to people; records an agent keeps for its operator aren't conversation.

## Glossary

- **Risk-tier paths**, changed. Before: the paths listed in a project's risk file whose changes need line-level human review (GOV-02): `unsafe` code, cryptography, data formats and migrations, sync, security-sensitive code, the plugin boundary, and the public API of shared crates. Now: the paths listed in a project's risk file whose changes get the strictest review (GOV-02): `unsafe` code, cryptography, data formats and migrations, sync, security-sensitive code, the plugin boundary, and the public API of shared crates.
- **Outside contributions**, new: issues, discussions or pull requests from anyone who isn't a maintainer. The project opens to them deliberately, and the rules marked for that trigger apply before it does.
- **Sign-off list**, new: the changes that need the founder's sign-off on top of review while there is one maintainer (CI-03).

## Not done in this step

- The front matter (How this document works, Context, the open questions and the version history) still
  describes phases and the RFC process; it needs a pass before the full document is regenerated.
- A few rules that apply now still route decisions through an RFC (BIZ-01 for license changes, REV-04
  for design disputes), while GOV-01 now waits for outside contributions. Until then, decide whether an
  ADR stands in for those RFCs.
- LIFE-08 and LIFE-09, the experiment rules, aren't included: they were drafts awaiting your approval.
