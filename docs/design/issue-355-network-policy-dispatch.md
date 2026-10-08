# Design: network firewall-policy dispatch (issue #355)

Status: **ADOPTED 2026-10-08 (maintainer ruling, recorded on the
issue).** Option A + Option B's baseline half; DP4 ruled as
baseline + default-deny; DP1-DP3, DP5-DP7 adopted as recommended;
Option C rejected. Decomposition: 3 PRs, carrier-first (§5).

## 1. The finding, restated against current main

Issue #355 filed two symptoms against candidate `baa20c0e`:

1. `POST /v1/networks/update` with `firewall_rules` (or `nat_rules`,
   `dhcp_scope`, `dns_*`) is accepted, persisted to
   `network_desired_state`, and **never dispatched to any node** — dead
   configuration with a 200 response.
2. The nft table nwd creates per network (`nft add table inet chv-<net>`,
   `chv-nwd-core/src/executor.rs:1156`) is **bare** — no chains, no
   default-deny — so guest-boundary traffic on materialized networks is
   unfiltered.

Current main adds one nuance the issue predates: the **VM-create path
already applies a non-empty policy snapshot at attach time** in
core-managed mode. The orchestrator's `build_agent_vm_spec` joins
`firewall_rules_json` into `AgentNicSpec.firewall_policy_json`
(`orchestrator.rs:1697-1750`), and the core executor applies it via
`set_firewall_policy` **before** `attach_vm_nic`
(`chv-agent-runtime-ch/src/core_runtime.rs:527-562`). So the deployed
behavior today is:

| Network state | At VM create | After VM create |
|---|---|---|
| rules stored (non-empty) | policy applied on that node | **updates never propagate** |
| no rules (the common case: install.sh's seed, implicit networks) | **bare table — no default-deny** | never anything |

The defect is therefore threefold:

- **(a)** rule-less networks never engage default-deny — deliberate
  (#360: an empty ruleset applied verbatim would cut a network's guests
  off entirely, including DHCP — every producer filters empty
  rulesets), but the consequence is that the "CHV guest default-deny"
  property is never engaged on the deployed path;
- **(b)** rule updates (and clears) after a topology exists never reach
  the node — the BFF's `policy_application` note promises "applied when
  a VM spec is next dispatched", but VM **spec updates** are refused in
  core-managed mode (`agent_server.rs:288-299`), so only a *new VM
  create* on that network/node would carry the new snapshot;
- **(c)** there is no operation/task surface — the operator gets HTTP
  200 and a prose note, with no journaled operation, no failure
  reporting, no retry.

The nwd engine itself is sound and proven (prompt-01 host-safety suite;
`scripts/integration/qual/host-safety.sh`): `SetFirewallPolicy`
materializes default-deny-first chains scoped to CHV-owned interfaces,
fail-closed on unknown topology
(`chv-nwd-core/src/firewall.rs:161-337`, `handlers.rs:745-811`). The
chain from the operator API to it does not exist.

## 2. Current-state map (the seams any fix must use)

- **BFF** `update_network` (`crates/chv-webui-bff/src/handlers/networks.rs:573-749`):
  one transaction writing `network_desired_state` (COALESCE per field,
  `desired_generation + 1`), no operation row, no dispatch. Save-time
  validation already enforces the single engine vocabulary
  (`chv_common::firewall`), the M4.4 N7 lesson.
- **Dormant CP push** `NodeClient::apply_network_desired_state`
  (`node_client.rs:321-367`, proto
  `proto/controlplane/control-plane-node.proto:149-154` under
  `ReconcileService`): complete, transport-hardened (timeout, circuit
  breaker, `UNIMPLEMENTED` preserved) — and **dead on the dispatch
  side** (tests only).
- **Agent refusal**: the same RPC fails closed in core-managed mode
  (`agent_server.rs:612-623`) — the M2.2b single-writer rule: the
  legacy path performs nwd side effects and writes the network axis
  (NodeCache) behind the Core authority's back. Legacy mode applies
  topology + firewall + NAT + DHCP + DNS with warn-and-continue
  semantics on everything past topology.
- **Core store**: no networks table, no network desired state, and the
  Core `operations.kind` CHECK is VM-scoped only
  (`cellhv-core-store/migrations/0001_core_authority.sql:33-47`).
- **Networks are fleet-wide** (`networks.node_id` NULL for
  operator-created networks, `networks.rs:315-326`); materialization is
  **per-node and lazy** — nwd topology exists only on nodes where a VM
  attached. There is no single "owning node" for the networks that
  carry rules. The orchestrator's claim query resolves a network op's
  node via `networks.node_id` (`orchestrator.rs:200-204`) — NULL for
  exactly these networks.
- **Fan-out precedent**: `UpdateOverlay` (`orchestrator.rs:735-739`,
  `dispatch_update_overlay:1395-1492`) — network-scoped, fans out to
  participating nodes, journals + retries, all-`Unimplemented`
  terminal-`Failed`. Its agent leg is legacy-only today, so it is a
  shape precedent, not a reuse.
- **Attach-time precedent for the M2.2b carve-out**: the core executor
  already calls `set_firewall_policy` in core-managed mode at VM attach
  (`core_runtime.rs:527-562`). Policy application to nwd is therefore
  already sanctioned behind the Core authority when it rides a
  journaled VM operation; what is refused is the *legacy fragment
  path* (NodeCache network-axis writes, unprompted topology ensure).

## 3. Options (maintainer call)

### Option A — journaled `UpdateNetworkPolicy` operation, fan-out dispatch (the full fix)

`update_network` journals an `UpdateNetworkPolicy` operations row in
the same transaction as the desired-state write (the `CreateVolume`
precedent, `volumes.rs:475-483`); a new orchestrator arm resolves the
**target set = nodes with at least one attached VM on the network**
(derivable: `vm_nic_desired_state` × `vm_desired_state.target_node_id`)
and fans out per node like `UpdateOverlay`; a new core-managed-accepted
agent handler applies the ruleset via the existing
`HostResourceController::set_firewall_policy` (the attach-time path's
exact call). Zero targets ⇒ the op completes `Succeeded` with a
recorded "no live materialization; policy applies at next attach"
outcome rather than failing (a fleet network with rules but no VMs yet
is the normal create-then-populate order).

- *Fixes*: (b) updates propagate to live topologies; (c) full task
  surface (accept/retry/terminal-failure cause via the #502
  surfacing); the issue's "fail or clearly report when unreachable"
  expectation, satisfied by the retry/terminal machinery.
- *Cost*: the largest surface — proto RPC, agent handler, orchestrator
  arm + claim-query target resolution, BFF journaling, tests at every
  tier. Estimate 3 PRs (decomposition §5).
- *Risk*: the M2.2b carve-out must be explicit (DP3).

### Option B — attach-time convergence only (the issue's second suggestion)

Keep dispatch dead; make materialization self-sufficient: at
`ensure_and_attach_nic`, apply the network's **current** stored policy
(already happens for non-empty snapshots) and additionally engage
**default-deny with an explicit baseline** when the snapshot is empty
(DP4). Updates continue to ride the next VM create on that node.

- *Fixes*: (a) only — rule-less networks get a filtered boundary at
  materialization.
- *Cost*: small — one call-site change in `core_runtime.rs` plus the
  baseline ruleset definition; no proto, no orchestrator, no BFF
  change beyond the `policy_application` prose.
- *Leaves*: (b) and (c) entirely — updates/clears still never
  propagate, still no task surface. The BFF's "pending" note stays
  honest but the honesty is "pending forever until a new VM".

### Option C — resurrect the dormant `ApplyNetworkDesiredState` push

Un-refuse the existing RPC in core-managed mode and have the
orchestrator call it on generation change.

- *Rejected (recommendation)*: the RPC's legacy semantics are the
  wrong shape for core-managed — it writes the NodeCache network axis
  (which nothing in core-managed mode reads) and does an unprompted
  `ensure_network_topology` (nwd topology without a VM attachment —
  the lazy-materialization model would gain orphan topologies nothing
  tears down); it is fragment-shaped (full desired state) rather than
  policy-shaped; and `networks.node_id` is NULL for the networks in
  question, so "push to the owning node" has no target. Option A
  reuses its transport discipline but not its semantics.

### Recommendation

**Option A + Option B's baseline half (DP4), decomposed per §5.**
Option B alone leaves the operator-facing lie (200 + never-applied)
that the issue was filed on. Option A alone leaves rule-less networks
— the overwhelming majority (install.sh's seed, every implicit
network) — unfiltered. The pair closes both: every materialized
network is filtered (baseline default-deny at attach), and every
policy change propagates (journaled fan-out dispatch). NAT/DHCP/DNS
scopes stay out of scope (§6).

## 4. Decision points

- **DP1 — journaling shape.** `update_network` journals
  `UpdateNetworkPolicy` with idempotency key
  `update-network-policy-{network_id}-{desired_generation}` inside the
  desired-state transaction (CreateVolume precedent). Alternative:
  only journal when firewall fields actually changed (the
  `has_network_update` split already distinguishes this) — recommended,
  so name/cidr-only updates don't mint no-op operations.
- **DP2 — target set.** Nodes with ≥1 attached VM on the network
  (join `vm_nic_desired_state` × `vm_desired_state.target_node_id`,
  tombstone-aware). Alternatives: `networks.node_id` (NULL for the
  networks that matter — non-starter), all enrolled nodes (applies
  policy to nodes with no topology — nwd fails closed on unknown
  topology, so this would just mint Failed rows). Zero-target ⇒
  `Succeeded` with a recorded no-op outcome.
- **DP3 — the M2.2b carve-out.** A new `ApplyNetworkPolicy` RPC on the
  node `LifecycleService` (beside `CreateVolume`), accepted in
  core-managed mode, whose handler calls
  `HostResourceController::set_firewall_policy` — justified by the
  existing attach-time precedent (policy application already runs
  behind the Core authority when journaled; the Core store models no
  network state, so there is nothing to journal *into* Core — the CP
  operations row is the journal). Alternative: model network policy in
  the Core store (new `operations.kind`, network desired state on the
  node) — rejected as a much larger rework with no consumer of the
  node-local state, but it is the strictly-purer reading of M2.2b.
- **DP4 — empty-ruleset semantics (Option B's half).** Define the
  policy boundary's empty case as **baseline + default-deny**: DHCP
  (UDP 67/68), DNS (UDP/TCP 53) to the gateway, and
  established/related (the engine already adds conntrack) — then
  default-deny. This replaces the #360 "filter empty rulesets"
  workaround at the policy boundary: `[]` stops meaning "no policy"
  and starts meaning "no *user* rules". Alternatives: keep `[]` = no
  policy (status quo — leaves (a) unfixed); `[]` = bare default-deny
  with no baseline (the #360 cutoff — rejected).
- **DP5 — clear semantics.** A ruleset cleared to `[]` under DP4
  becomes baseline-only (a live, filtered network), not a teardown —
  topology teardown stays last-detach (#356 N5). The current
  "cleared: stays in force until teardown" `policy_application` note
  is replaced by the journaled operation's outcome.
- **DP6 — reporting.** The `policy_application` prose notes
  (`networks.rs:726-747`, pinned by
  `tests/network_policy_reporting.rs`) are replaced by the standard
  task surface: the update response carries `task_id`/operation
  outcome like every other mutation, and terminal failures surface
  their cause via #502. `last_task` on the network detail starts
  resolving to policy operations.
- **DP7 — dispatch fencing.** Reuse `desired_generation` as the
  fence: the agent applies a policy whose generation ≥ the topology's
  last-applied generation; stale generations are idempotent no-ops
  (nwd's `policy_state` + `refresh_policy_scope` already give
  idempotent re-apply). Prevents out-of-order fan-out results from
  regressing a newer policy with an older one.

## 5. Decomposition (if Option A+B is adopted)

Mirroring the #513/#522 carrier-first pattern:

1. **PR 1 — the carrier, dead-but-live**: the `ApplyNetworkPolicy`
   proto RPC + node_client method + agent core-managed handler +
   orchestrator arm (journals nothing; refuses/dispatches nothing
   until PR 2 produces operations). Zero behavior change. Pins the
   M2.2b carve-out rationale in code comments and the agent tests.
2. **PR 2 — the producer**: BFF journaling (DP1), claim-query target
   resolution (DP2), response shape change (DP6), the
   `network_policy_reporting.rs` test migration.
3. **PR 3 — the baseline + convergence**: DP4's baseline ruleset in
   the shared firewall module, applied at attach-time for empty
   snapshots; DP5/DP7 fencing tests; the end-to-end contract row
   (update a live network's rules → task Succeeded → nft chains
   changed on the target node — proven at the mock-nwd tier, with the
   qualification leg noted for M4.4's successor run).

## 6. Non-goals

- **NAT / DHCP-scope / DNS-scope dispatch** — the same dead-field
  finding, but none is safety-critical, and DHCP/DNS scopes have
  materialization coupling (dnsmasq restarts) that deserves its own
  design. The Option A carrier is shaped so a follow-up can carry
  them; tracked separately on merge.
- **Fabric/overlay policy** — `UpdateOverlay` stays legacy-only; its
  core-managed story is a separate M2.2b question.
- **The legacy-mode paths** — unchanged; the legacy
  `apply_network_desired_state` reconciler keeps its shape.
- **Re-shaping `networks.node_id` / ownership** — out of scope; DP2
  derives targets without it.

## 7. Test-surface impact summary

- `network_policy_reporting.rs` — DP6 rewrites the pinned prose into
  task-shape assertions.
- `orchestrator.rs` — new arm tests: target resolution (fleet network
  with attached VMs on two nodes ⇒ two dispatches; zero targets ⇒
  no-op Succeeded), fencing (DP7), `Unimplemented` terminal path.
- `agent_server.rs` — core-managed acceptance test (the M2.2b
  carve-out pinned), empty-baseline behavior (DP4) at the mock-nwd
  tier.
- `chv-agent-runtime-ch` — attach-time baseline application beside the
  existing non-empty-snapshot tests.
- `chv-nwd-core` — no engine change; existing pins hold.
