# CHV documentation standard

> **Status (2026-10-03, docs/ste-standard branch):** draft for review. This
> standard applies prospectively to living documentation, as described in
> [Scope](#1-purpose-and-scope). It does not change any frozen document.

This standard defines a controlled technical English for CHV documentation. It
is modeled on the controlled-language principles popularized by ASD-STE100
(Simplified Technical English). ASD-STE100 is a proprietary, paid
specification. This document does not copy or reproduce it. This document
states CHV's own rules in CHV's own words.

## Contents

1. [Purpose and scope](#1-purpose-and-scope)
2. [Registers](#2-registers)
3. [Writing rules](#3-writing-rules)
4. [Approved-terms dictionary](#4-approved-terms-dictionary)
5. [Document structure](#5-document-structure)
6. [Change procedure](#6-change-procedure)
7. [Compliance checklist](#7-compliance-checklist)

## 1. Purpose and scope

CHV has about 247 markdown documents. Readers scan them under time pressure.
Operators read runbooks during incidents. Agents parse instructions literally.
Controlled language reduces ambiguity, translation cost, and maintenance drift.

This standard governs the words, sentences, and structure of CHV documentation.
It does not govern code, code comments, commit messages, or UI strings.

### Living documents

The standard applies to all living documents. Living documents describe the
current system and change with it.

| Cluster | Examples |
|---------|----------|
| Root docs | `README.md`, `AGENTS.md`, `CONTRIBUTING.md`, `SECURITY.md`, `DESIGN.md` |
| Top-level docs | `docs/ARCHITECTURE.md`, `docs/OPERATIONS.md`, `docs/DEPLOYMENT.md`, `docs/OBSERVABILITY.md`, `docs/PACKAGING.md`, `docs/GAP_ANALYSIS.md`, `docs/load-testing.md` |
| Install guides | `docs/install/*` |
| Runbooks | `docs/runbooks/*` |
| Release docs | `docs/release/PIPELINE.md` and the other living files in `docs/release/*` |
| Governance | `docs/governance/*` |
| Specs | `docs/specs/{component,ops,spec,contracts}/*`, `docs/specs/cellhv-*.md` |
| Decisions index | `docs/decisions/README.md` |
| Monitoring | `monitoring/README.md` |
| Examples and labs | `docs/examples/`, `docs/labs/` |

### Frozen documents

The standard applies prospectively only. Never rewrite a frozen document to
comply with this standard. A frozen document changes only when its facts
change, and only in the smallest way that fixes the facts.

| Cluster | Freeze reason |
|---------|---------------|
| Evidence | `docs/evidence/**` — captured verification output |
| Released history | released `CHANGELOG` history |
| Accepted ADRs | `docs/specs/adr/**` — change only via a new superseding ADR |
| Plans | `docs/plans/**` |
| Analysis | `docs/analysis/**` |
| Reports | `docs/reports/**` |
| Executed prompts | `docs/prompts/**` |
| Conduct | `CODE_OF_CONDUCT.md` |
| Early planning | the pre-0.1.0 release planning artifacts |

## 2. Registers

CHV has three documentation registers. Pick the register before you write.

### Operator-facing

Operators run the platform under time pressure. Runbooks, install guides, and
`docs/OPERATIONS.md` are operator-facing. These documents use the full rule
set: short sentences, imperative procedures, and approved terms without
exception. [`docs/runbooks/README.md`](../runbooks/README.md) is the reference
model for this register.

### Developer-facing

Developers extend the platform. Architecture docs, specs, and governance docs
are developer-facing. These documents use the full rule set, with one
relaxation: sentences may run to 25 words when a code identifier forces it.

### Agent-facing

Agents execute instructions from text. `AGENTS.md` and
[`docs/release/PIPELINE.md`](../release/PIPELINE.md) are deliberately
agent-oriented. Exact command strings and file paths matter more than prose
rhythm in this register. The sentence-length rule is relaxed there, so a step
may quote a long command or path verbatim. Terminology rules apply everywhere,
including agent-facing documents.

## 3. Writing rules

Each rule below has a compliant example and a non-compliant example. Several
examples are drawn from real CHV documents.

### R1. Keep sentences short

Target 20 words or fewer. Use a hard limit of 25 words in procedures. Split
longer sentences into two.

- ✅ `chv-agent` reboots the VM. The control plane records the result.
- ❌ The agent, upon receipt of the reboot instruction from the control plane
  over the gRPC channel, will proceed to reboot the virtual machine in
  question and then report the outcome back.

Code identifiers, paths, and commands do not count toward the limit.

### R2. One topic per sentence

Each sentence makes exactly one statement. Do not join causes, effects, and
exceptions in one sentence.

- ✅ The node enters `Draining`. The scheduler stops new placements.
- ❌ The node enters `Draining` and the scheduler stops new placements, unless
  an override flag is set, in which case it does not.

### R3. Use active voice

Name the actor. Put the actor before the verb.

- ✅ The control plane owns desired state.
- ❌ Desired state is owned by the control plane.

The passive voice is allowed in exactly two cases. Use it when the actor is
unknown, or when the acted-on thing is the topic of the paragraph.

### R4. Use the present tense for system behavior

Describe what the system does, not what it will do.

- ✅ The agent reboots the VM.
- ❌ The agent will reboot the VM.

### R5. Write procedures in the imperative mood

Number the steps. Start each step with a verb. Never use future tense or
conditional mood in a procedure step.

- ✅ 1. Run `chvctl node drain <NODE_ID>`. 2. Wait for the state `Drained`.
- ❌ 1. You should probably be running the drain command. 2. The node would
  then be drained.

### R6. Define every abbreviation at first use

Define each abbreviation once per document. Write the expansion first, then
the abbreviation in parentheses.

- ✅ The browser talks only to the backend-for-frontend (BFF) service.
- ❌ The browser talks only to the BFF service.

Two abbreviations need no definition: `CHV` and `VM`. Both are defined in the
[approved-terms dictionary](#4-approved-terms-dictionary).

### R7. Avoid idioms and jargon synonyms

Do not use idioms. Do not use two words for one concept. The dictionary in
section 4 is the only source of synonyms.

- ✅ The runbook covers this case.
- ❌ The runbook has you covered, and it also walks you through the scenario.

### R8. Keep paragraphs to one topic

A paragraph covers one topic in six sentences or fewer. Split anything longer.

### R9. Use sentence case for headings

Use sentence case for all headings in living documents. Capitalize the first
word and proper nouns only.

- ✅ `## Install on Debian and Ubuntu`
- ❌ `## Install On Debian And Ubuntu`

Some older clusters use Title Case. Convert them opportunistically when you
edit them for other reasons. Do not convert whole clusters in one change.
The `docs/install/` and `docs/release/` clusters already use sentence case.

### R10. Prefer tables over prose for enumerations

Use a table when you list three or more items with parallel facts. Use prose
for argument and context.

### R11. Restrict status glyphs to checklists and status tables

Use ✅, ❌, and ⚠️ only in checklists and status tables. Never use them in
running prose or headings.

### R12. Use concrete versions or placeholders, never stale ones

Match every concrete version number to the [`VERSION`](../../VERSION) file at
authoring time. Use a placeholder such as `<version>` when the document is
version-independent.

- ✅ `dpkg -i chv-agent_<version>_amd64.deb`
- ❌ `dpkg -i chv-agent_0.1.0_amd64.deb` (written when `VERSION` says `0.2.0`)

The runbooks currently hardcode `0.1.0` while `VERSION` says `0.2.0`. That
defect motivated this rule. Fix it opportunistically under this standard.

## 4. Approved-terms dictionary

The dictionary resolves the known terminology conflicts in CHV documentation.
Use the approved term. Do not use the forbidden synonyms. Crate names, field
names, and code identifiers are exempt everywhere they appear as code.

| Approved term | Forbidden synonyms | Notes |
|---------------|--------------------|-------|
| CHV | CH | CHV names the platform, only. Never use CHV for the VMM. |
| Cloud Hypervisor | CHV (for the VMM), CH | The VMM in prose. On first mention per document, write "Cloud Hypervisor (the VMM)". |
| `cloud-hypervisor` | — | The binary and package name. Always in code font. |
| node | host, server (as prose nouns) | The managed host in prose, CLI, and API contexts. The UI label "Hosts" is a deliberate synonym per [ADR-006-WebUI](../specs/adr/006-webui-navigation-revised.md); document the mapping, do not change the UI. Designer YAML keeps `servers` as a contract field name. |
| VM | instance, virtual machine (spelled out) | VM is the prose term. The UI label "Instances" is a deliberate synonym per ADR-006-WebUI. Use guest only for guest-visible behavior and guest-to-host security contexts, as in `SECURITY.md`. |
| `chv-agent` | CellHV Core (except as noted), core-native, core-managed (for the component) | `chv-agent` is the primary name of the agent runtime. Write "(CellHV Core)" once per document as a parenthetical alias, because specs and ADRs use that name. Use core-native and core-managed for modes, not for the component. See [ADR-016](../specs/adr/016-evolve-chv-agent-into-cellhv-core.md). |
| `chv-controlplane` | controlplane (bare, in prose) | The control-plane daemon binary. It embeds the BFF and serves the web UI. |
| `chv-stord` | stord (bare) | The node storage daemon. Always the full name in operator-facing text. |
| `chv-nwd` | nwd (bare) | The node network daemon. Always the full name in operator-facing text. |
| `chvctl` | chv-ctl | The CLI binary. |
| control plane | controlplane, CP (undefined) | Noun form. Use control-plane (hyphenated) as the adjective. Never write controlplane in prose; crate names are exempt. Use CP only after a first-use definition, and never in operator docs. |
| backend-for-frontend (BFF) | BFF (undefined) | Expand on first use per document. The BFF runs inside `chv-controlplane`; there is no separate BFF binary. |
| Unix socket | Unix domain socket, UDS (undefined) | Use UDS only after a first-use definition, and never in operator docs. |
| O3K | o3k | Always capitalize. |

Short forms such as CP and UDS are allowed only after an explicit definition
in the same document. Operator-facing documents never use them.

## 5. Document structure

### Metadata header

Documents that carry live state start with a dated status blockquote. Use the
convention of [`BRANCH_PROTECTION.md`](BRANCH_PROTECTION.md): a blockquote
with the date and the current state. Update the blockquote when the state
changes. Plain reference documents need no status blockquote.

### Section ordering

Order living documents as follows. Omit sections that do not apply.

1. Title (`#`) and status blockquote, if any
2. Purpose: what the document covers and for whom
3. Prerequisites or scope limits, if any
4. Main content: procedures, reference tables, or explanation
5. Related documents
6. Change history or status, if the document carries state

### Cross-links

Use relative links. Keep every document reachable from an index: the root
`README.md`, a cluster README, or `docs/decisions/README.md`. Do not create
orphan documents. Link to the ADR when a rule in your document comes from one.

## 6. Change procedure

### Adding or changing a dictionary word

1. Open a PR against this file.
2. State the conflict the new term resolves.
3. Cite the documents that use the competing terms.
4. Name the approved term, the forbidden synonyms, and a one-line rationale.
5. A maintainer approves, or the proposal is rejected with reasons.

### Application in PR review

Reviewers of documentation PRs paste this into the review:

```text
Docs standard check (docs/governance/DOCUMENTATION_STANDARD.md):
[ ] Approved terms only (see the dictionary in section 4)
[ ] Abbreviations defined at first use
[ ] Short sentences, active voice, present tense
[ ] Procedures numbered and imperative
[ ] No frozen documents modified
```

This checklist is advisory. It does not block a PR on its own.

### Relationship to CONTRIBUTING.md

`CONTRIBUTING.md` covers dev setup, code style, and the PR workflow. This
standard covers documentation language. The two documents complement each
other. A pointer from `CONTRIBUTING.md` to this standard is planned but out of
scope for this draft.

## 7. Compliance checklist

Check every box before you merge a change to a living document.

- [ ] The document is living, not frozen.
- [ ] CHV refers to the platform, never to the VMM.
- [ ] The VMM appears as Cloud Hypervisor in prose and `cloud-hypervisor` in code font.
- [ ] First mentions of the VMM read "Cloud Hypervisor (the VMM)".
- [ ] Prose uses node; UI labels Hosts and Instances are mapped, not changed.
- [ ] Prose uses VM; guest appears only in guest-visible or security contexts.
- [ ] Daemon names match the binaries in `cmd/` exactly.
- [ ] `chv-agent` is the primary agent name; CellHV Core appears at most once, as a parenthetical.
- [ ] control plane (noun) and control-plane (adjective) are used correctly.
- [ ] Every abbreviation is defined at first use in the document.
- [ ] Sentences meet the length target for the register.
- [ ] Procedures use numbered steps in the imperative mood.
- [ ] Concrete version numbers match the VERSION file, or use a placeholder.
- [ ] Headings use sentence case.
- [ ] Every relative link resolves, and the document is reachable from an index.
