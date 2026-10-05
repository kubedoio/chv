# #378 design writeup — accept-time rejection of volume snapshot/clone/restore/delete-snapshot on core-managed nodes

**Issue:** kubedoio/chv#378 — M4.5 qualification leg D: volume
snapshot/clone/restore/delete-snapshot is fail-closed on core-managed nodes
(agent returns `Unimplemented` — single-writer enforcement, correct), but the
BFF accepts the request with HTTP 200 "accepted", the CP journals a
`volume_desired_state` intent, the orchestrator retries the refused dispatch,
and the op eventually fails with no snapshot ever created. The operator saw
success at step 1 and silence after.

**Status:** DECIDED & IMPLEMENTED. The investigation below is unchanged
(evidence cited `file:line` at main `60df52cf`); §6.1 records the maintainer
decision (2026-10-05) and §12 the as-built record.

---

## 1. Problem statement

Single-writer enforcement is correct and stays: the agent's legacy volume-op
RPCs fail closed in core-managed mode because Core M1 does not model volume
snapshot/clone (`agent_server.rs:1762-1914`), and the legacy reconcile path
that would execute the journaled intent is legacy-only (`reconcile.rs:916-919`).
The defect is the **acceptance UX**: every layer before the agent says yes.

The fix direction from the issue: reject at accept-time with a clear
`InvalidArgument` ("volume snapshot/clone is not supported on core-managed
nodes"). That requires the CP to know each node's authority mode, which the
`nodes` table does not carry today (verified §3).

Two premise corrections from the investigation (details in §2.3):

1. **The "600 s window" is not on this path at current main.** The
   accept→terminal-failure window is **~70 s** (4 dispatch attempts at
   10/20/40 s backoff, `MAX_DISPATCH_RETRIES = 3`). The 600 s bound the issue
   cites is the stuck-`Running` reaper (`orchestrator.rs:381`), which only
   re-arms ops left `Running` by a crash window — `Unimplemented` returns
   instantly, so the reaper never fires for it.
2. **The issue's line references have moved** (§2.4).

## 2. Current behavior, end to end (ground truth at `60df52cf`)

### 2.1 The four surfaces

| Surface | BFF route | BFF handler | CP lifecycle (journal + accept) | Agent fail-closed gate |
|---|---|---|---|---|
| snapshot | `POST /v1/volumes/snapshot` (`router.rs:290`) | `handlers/volumes.rs:265` | `lifecycle.rs:921` `snapshot_volume` | `agent_server.rs:1769-1773` |
| restore | `POST /v1/volumes/restore-snapshot` (`router.rs:294`) | `handlers/volumes.rs:304` | `lifecycle.rs:959` `restore_volume` | `agent_server.rs:1816-1820` |
| delete-snapshot | `POST /v1/volumes/delete-snapshot` (`router.rs:298`) | `handlers/volumes.rs:343` | `lifecycle.rs:997` `delete_volume_snapshot` | `agent_server.rs:1863-1867` |
| clone | `POST /v1/volumes/clone` (`router.rs:302`) | `handlers/volumes.rs:382` | `lifecycle.rs:1038` `clone_volume` | `agent_server.rs:1910-1914` |

**All four have the identical accept-then-fail shape.** Each agent gate is the
same three lines: `if self.core_authority.is_some() { return
Err(Status::unimplemented("… is unsupported in core-managed mode")) }`.

### 2.2 The trace

1. **BFF** (`handlers/volumes.rs:265-425`): authz (`require_operator_or_admin`,
   `require_volume_owner`), payload validation, then the CP mutation service.
   Returns HTTP 200 `{"accepted": true, "task_id": …}` on ack
   (`volumes.rs:296-301`).
2. **BFF→CP mutation bridge** (`bff_mutations.rs:478/525/572/619`): looks up
   the volume's `node_id` from the CP database, calls the `LifecycleService`
   **in-process** (the BFF router is hosted inside the `chv-controlplane`
   binary sharing the same SQLite pool and repos — `cmd/chv-controlplane/src/bootstrap.rs:271-297`).
3. **CP lifecycle** (`lifecycle.rs:921-1134`): `create_operation_and_emit`
   journals an `operations` row (`SnapshotVolume`/`RestoreVolume`/
   `DeleteVolumeSnapshot`/`CloneVolume`) via `create_or_get`
   (`lifecycle.rs:360-421`), then `persist_intent_and_accept`
   (`lifecycle.rs:509`) writes the `volume_desired_state` intent
   (`snapshot_op` = create/restore/delete; clone additionally materializes the
   target `volumes` row, `lifecycle.rs:1101-1131`), then returns
   `ok_ack("snapshot volume accepted")`. **Nothing checks the node's mode.**
4. **Orchestrator** (`orchestrator.rs:166-194` claims `Accepted` → `Running`;
   dispatch arms at `orchestrator.rs:902-953`) → `node_client.snapshot_volume`
   (`node_client.rs:803-841`, same shape at `:843/:883/:923`) →
5. **Agent** returns `tonic::Status::unimplemented("snapshot_volume is
   unsupported in core-managed mode")` (`agent_server.rs:1769-1773`).
6. **Error flattening** (`node_client.rs:148-162`): `with_timeout` maps every
   non-timeout tonic status — `Unimplemented` included — to
   `ChvError::Internal { reason: "snapshot_volume failed: status:
   Unimplemented, message: …" }`. The status **code** is discarded; only the
   text survives.
7. **Dispatch failure handling** (`orchestrator.rs:1219-1242`):
   `dispatch_operation` marks the op `Failed` with `error_code:
   "AGENT_REJECTED"` and returns `Err`.
8. **Retry resurrection** (`orchestrator.rs:276-355`): the tick's error
   handler warns `"dispatch failed"`, reads `retry_count`, and calls
   `mark_for_retry` — whose UPDATE has **no status guard**
   (`chv-controlplane-store/src/operations.rs:151-177`) — flipping the just-
   written `Failed` back to `RetryPending` with 10/20/40 s backoff
   (`orchestrator.rs:294-296`, `MAX_DISPATCH_RETRIES = 3` at `:20`).
9. **Terminal** (`orchestrator.rs:326-354`): on the 4th failure the op is set
   `Failed` with `error_code: "DISPATCH_FAILED"` and message "permanently
   failed after 3 retries: snapshot_volume failed: status: Unimplemented, …"
   — the agent's clear one-line explanation is buried in the retry-exhaustion
   text. No snapshot exists. The journaled `volume_desired_state` intent
   (snapshot_op) persists forever.

**Net operator experience (chvctl):** `chvctl volume snapshot` prints
"Snapshot created." (`cmd/chvctl/src/commands/volume.rs:48-55`), then ~70 s of
`dispatch failed` warn lines in the CP log, then nothing. chvctl reaches
snapshot and clone only (no restore/delete-snapshot subcommands,
`volume.rs:8-27`) through the same BFF routes; the web UI calls none of these
routes (no references under `ui/src`).

### 2.3 Where the 600 s figure actually lives

`reap_stuck_operations` (`orchestrator.rs:373-403`) resets ops stuck in
`Running` for 600 s (snapshot/clone family, `orchestrator.rs:381`) back to
`Accepted`, preserving `retry_count`. It is a crash-window backstop: an op is
only `Running` between claim and failure handling, and `Unimplemented` fails
in milliseconds. Even when it does fire (CP restart mid-dispatch), the reaped
op re-enters the same bounded retry cycle and goes terminal on the next
failure (retry_count is preserved). So at current main the accept→terminal
window is **~70 s, 4 dispatch attempts** — the M4.5 evidence doc's
"600-second reaper then resets the op Running → Accepted, where it re-claims"
(`docs/evidence/production-readiness/v0.3.0-rc1/04-real-host-qualification/m4.5-storage.md`
§4.4) overstates the window; the UX defect (200-accepted then silent failure)
is real regardless.

### 2.4 Line-reference corrections (issue → current main)

| Issue said | Current main |
|---|---|
| `agent_server.rs ~:1544` | volume family gates at `:1769/:1816/:1863/:1910` (VM snapshot/restore at `:2364/:2401`; `:1544` is inside `resize_vm`'s gate region `:1526-1530`) |
| `reconcile.rs ~:965` | fail-closed gate `reconcile.rs:916-919` (`require_mutation`, defined `:195-200`); legacy application of the journaled snapshot/clone intent at `:979+` |
| `orchestrator.rs ~:376` | reaper `orchestrator.rs:373-403` (600 s bound at `:381`) — but the retry machinery at `:276-355` is what bounds this path (~70 s) |

### 2.5 Sibling surfaces with the same shape (wider than the issue's four)

The same accept-then-`Unimplemented` shape exists for every legacy RPC the
agent refuses in core-managed mode, all journaled+accepted by the CP and
retried identically: VM snapshot/restore (`agent_server.rs:2357/:2394`),
`resize_vm` (`:1522`), `attach_volume`/`detach_volume` (`:1562/:1618` — i.e.
the BFF volume `mutate` attach/detach actions), `update_overlay` (`:2874`),
coredump, and more (~25 gates). The issue scopes the four volume surfaces; the
carrier work (§4) fixes the *ability* to reject all of them — which surfaces
to reject is a one-line-per-surface policy list. Flagged for the maintainer,
not designed here.

## 3. Authority-mode knowledge in the CP today

**The CP does not know a node's authority mode.** Verified:

- `nodes` table (`cmd/chv-controlplane/migrations/0001_initial.sql:1-13`):
  node_id, hostname, display_name, enrollment_token_id, certificate_serial,
  agent_version, control_plane_version, enrolled_at, last_seen_at, timestamps.
  Later `nodes` ALTERs added only `agent_ws_address` (`0029`).
- `node_inventory` (`0001_initial.sql:28-45`; `0002` added storage_classes/
  network_capabilities/labels, `0022` added hypervisor_capabilities): no mode
  column; `labels` is a free-form JSON map.
- Telemetry: `NodeStateReport` (`proto/controlplane/control-plane-node.proto:377-384`)
  carries state/generation/health/last_error only; CP ingestion
  (`telemetry.rs:98-146`) upserts `node_observed_state` — nothing mode-shaped.
- Enrollment: `EnrollmentRequest` (`proto:54-58`) carries a `NodeInventory` —
  a possible first-contact carrier.
- The mode is an **agent-local config fact**: `AgentAuthorityMode`
  (`crates/chv-config/src/lib.rs:431`, values Legacy | CoreManaged | CoreNative,
  default Legacy) wired at agent startup (`cmd/chv-agent/src/main.rs:842-869`).

### 3.1 Candidate carriers

| Carrier | Mechanism | Staleness / lifecycle | Cost |
|---|---|---|---|
| **(a) Typed column on `node_inventory` (or `nodes`) + proto field** | Add `string authority_mode = 13` to `NodeInventory` (`proto:24-43`); agent sets it from config in `build_inventory*` (`crates/chv-agent-core/src/inventory.rs:88-120`); CP persists via the existing `report_node_inventory` path (`crates/chv-controlplane-service/src/inventory.rs:60+`, `nodes.rs:313-336` `upsert_inventory`). | Reported on enrollment and every 6th telemetry tick (5 s tick, `cmd/chv-agent/src/main.rs:1018,1450-1470` → ~30 s cadence) through the deferred-message queue that survives CP outages. Mode is static per agent process (config at startup), so staleness ≈ ≤30 s after a mode flip + reconnect. | Proto field + `ALTER TABLE` (precedent: `0022`, `0029`) + upsert + a lookup. Typed, greppable, queryable. |
| **(b) `labels` map entry** (e.g. `chv/authority-mode`) | Same transport as (a) but stuffed into the existing `labels` JSON (`NodeInventory.labels`, proto `:32`; agent currently sends an empty map, `inventory.rs:114`). | Same as (a). | Zero migration, zero proto change; but stringly-typed, invisible in schema, and the lookup parses JSON per check. |
| **(c) `NodeStateReport` field** | New field on the hot 5 s telemetry path; CP writes it to `nodes`/`node_observed_state`. | ~5 s staleness — better than (a) but pointless: mode cannot change without an agent restart, which also re-reports inventory. | Touches the highest-frequency path and a second table for no real freshness gain. |
| **(d) Derived from behavior** (CP remembers "this agent answered Unimplemented") | Learn the mode from dispatch failures. | Implicit, stale after flaps/upgrades, surprises on rolling upgrades. | Rejected — a capability cache built from errors is the wrong shape. |

Note for all carriers: **the BFF cannot avoid the CP store anyway** — the BFF
router runs inside `chv-controlplane` against the same SQLite pool
(`bootstrap.rs:271`). There is no separate BFF data plane, so every option
below implies the carrier exists in the CP database.

## 4. Goals & non-goals

**Goals**

- An operator issuing volume snapshot/clone/restore/delete-snapshot against a
  core-managed node gets an immediate, explicit rejection at the API surface —
  no 200-accepted, no journaled intent, no retry noise.
- The rejection message names the reason ("not supported on core-managed
  nodes") and is greppable.
- Legacy-mode behavior is byte-for-byte unchanged.
- The agent's fail-closed dispatch remains the enforcement boundary (the
  accept-time check is UX hardening, not a security control).

**Non-goals** (see also §9)

- Implementing volume ops under Core authority — a Core M1 modeling effort
  (also unlocks #379's LVM reachability); explicitly out of scope.
- Changing the legacy-mode paths.
- The clone target-id 16-byte `ResourceId` cap (`chv-controlplane-types/src/domain.rs:96-99`)
  — pre-existing validation, noted in the M4.5 harness traps; not designed
  around here.
- Rejecting the sibling surfaces of §2.5 (mechanism generalizes; list
  extension is policy).

## 5. Options — where to reject

### Option A — BFF-only rejection

The BFF handler checks the node's mode (from the shared DB) before forwarding.

- **API surface:** no route/contract change; the BFF returns 400/501 directly.
- **chvctl UX:** same as B/C (client surfaces the error text).
- **Divergence risk (decisive):** the CP gRPC lifecycle
  (`server.rs:475-529`) remains accept-then-fail for any non-BFF caller — the
  policy exists in two places that can drift. And since the BFF shares the CP
  store, the carrier work is identical; A saves nothing over B.
- **Rejected** unless the maintainer specifically wants BFF-local policy.

### Option B — CP lifecycle rejection at accept (recommended)

Each of the four `lifecycle.rs` handlers gains a pre-journal gate: resolve the
target node's authority mode from the store; if core-managed, return
`ControlPlaneServiceError::InvalidArgument("volume snapshot/clone/restore/
delete-snapshot is not supported on core-managed nodes (Core M1 does not model
volume snapshot operations)")` **before** `create_operation_and_emit` —
exactly the pre-validation-before-journaling shape clone already uses for
source/target (`lifecycle.rs:1047-1073`, the #381 fix).

- **API surface:** no proto/BFF change at all. `map_ack` already maps
  `InvalidArgument → BffError::BadRequest` (`bff_mutations.rs:84`) → HTTP 400
  `{"code":"BAD_REQUEST","message":…}` (`error.rs:131`). chvctl prints the
  message via its existing error path.
- **Covers every caller** of the CP lifecycle (BFF, chvctl-via-BFF, direct
  gRPC).
- **No rows journaled on rejection** — no operations row, no VDS intent, no
  clone target volume row (the clone residue trap of a phantom target volume
  disappears for rejected requests).

### Option C — both layers (defense in depth)

B plus a BFF-side check for a friendlier/earlier error. Cost: duplicated
policy and a second test surface; benefit: none today because the BFF and CP
share the store and process. **Not recommended now**; the agent's fail-closed
dispatch already provides the real defense in depth.

### Error code choice (orthogonal sub-decision)

- **`InvalidArgument` (issue's direction, recommended):** 400 with the
  message; zero new error plumbing. Slightly imprecise semantically (the
  request is well-formed; the *node* can't do it).
- **`Unsupported` → HTTP 501:** the CP already has
  `ControlPlaneServiceError::Unsupported` mapping to gRPC `Unimplemented`
  (`error.rs:53-55, :83`) and the BFF has `BffError::NotImplemented` → 501
  (`error.rs:138-140`) — but `map_ack` does not map `Unsupported` today (it
  falls to `_ => BffError::Internal` → 500, `bff_mutations.rs:87`), so this
  needs one new map_ack arm. More precise status, slightly more churn, and
  501-from-a-200-shaped-API is a contract change chvctl/UI consumers must
  tolerate.

### The already-journaled-intent race (applies to any option)

- **Ops accepted before the node flipped to core-managed / before the fix
  deployed:** they run today's bounded path — ~70 s of retries →
  `Failed/DISPATCH_FAILED`. Unchanged by this fix; acceptable.
- **Stale VDS intents:** the journaled `snapshot_op` intent is never cleaned.
  If the node later flips *back* to legacy, the legacy reconciler will execute
  it (`reconcile.rs:979+`) — a surprise snapshot long after the operator
  forgot. Accept-time rejection prevents *new* stale intents but not
  pre-existing ones. A one-shot scrub of pending `snapshot_op` intents for
  core-managed nodes is a possible follow-up; flagged as residual risk (§8)
  rather than designed here.

## 6. Decision points (maintainer) and recommendations

1. **Rejection layer** — A / B / C. **Recommend B** (CP lifecycle, pre-journal):
   single policy location, covers all callers, zero BFF changes, and the
   carrier work is identical for all options because the BFF shares the CP
   store.
2. **Authority-mode carrier** — typed column + proto field (a) vs labels (b)
   vs telemetry field (c). **Recommend (a)** on `node_inventory` (or `nodes`):
   typed and greppable like the rest of the contracts; ~30 s staleness is
   fine because mode only changes at agent restart; migrations `0022`/`0029`
   are precedent. Choose (b) only if a zero-migration hotfix is wanted.
3. **Unknown-mode policy** — fail-open (unknown/absent → accept, current
   behavior) vs fail-closed. **Recommend fail-open:** enforcement stays at the
   agent; a fail-closed default would break accepts during the carrier
   roll-out window and for nodes whose inventory hasn't landed yet.
4. **`Unimplemented` fast-fail scope (§7)** — bundle with this fix or ship as
   a separate piece. **Recommend separate**: it changes retry semantics for
   every refused RPC (§2.5), deserves its own tests and its own risk
   discussion. *(Status: shipped separately as recommended — #498, with the
   overlay fan-out leg completed by its follow-up PR; see §12.)*

### 6.1 Decision (maintainer, 2026-10-05)

All four recommendations adopted, as written:

1. **Rejection layer: Option B** — CP lifecycle, pre-journal, mirroring
   clone's existing #381 pre-validation shape.
2. **Carrier: (a)** — typed proto field + typed `node_inventory` column
   (`AuthorityMode` enum on `NodeInventory`, reported at enrollment + every
   ~30 s inventory cycle).
3. **Unknown-mode policy: fail-open** — the rejection fires only on a
   definite `core-managed` report; the agent dispatch stays the enforcement.
4. **`Unimplemented` fast-fail: separate future PR** — out of scope here
   (see §7, §11). *(Status: shipped as #498; the `UpdateOverlay` fan-out
   leg — the one surface whose dispatch aggregates per-node failures — was
   completed by #498's follow-up PR; see §12.)*

Error carrier: `InvalidArgument` → HTTP 400 via the existing `map_ack` arm;
zero new plumbing.

## 7. The second, independent piece: stop retrying `Unimplemented`

**Is retrying `Unimplemented` for 600 s (or 70 s) ever right?** No. gRPC
`UNIMPLEMENTED` is terminal-class for that method on that peer: a verbatim
retry cannot succeed unless the *agent binary changes* (rolling upgrade), and
a 10/20/40 s curve never bridges an upgrade window. Current main treats every
RPC error identically — `with_timeout` flattens all statuses into
`ChvError::Internal` (`node_client.rs:158-160`), and the tick handler retries
anything (`orchestrator.rs:276-355`); there is no retryability
classification anywhere in the orchestrator (the only special-casing is
`BackendUnavailable` → circuit breaker + client eviction,
`node_client.rs:795-799`, `orchestrator.rs:1220-1222`). The #368-era paths
inherit this too (e.g. the dispatch shim's `generation != 1` → Unimplemented
gate in `agent_server.rs`).

A fast-fail design (sketch at decision time — implemented since, by #498
plus its overlay fan-out follow-up; the as-built record is §12):

1. `with_timeout` preserves the tonic `Code` (e.g. a dedicated `ChvError`
   variant or a code field) instead of stringifying — the message text
   already says exactly what the operator needs ("… is unsupported in
   core-managed mode"); today it is discarded into a generic Internal.
2. The tick handler classifies `Unimplemented` (and arguably
   `InvalidArgument`/`FailedPrecondition` — same question, wider blast
   radius, maintainer's call) as **terminal at dispatch**: mark
   `Failed` immediately with a greppable `error_code` (e.g.
   `UNSUPPORTED_BY_AGENT`) carrying the agent's message, and **skip
   `mark_for_retry` entirely** — note the resurrection hazard: because
   `mark_for_retry` has no status guard (`operations.rs:151-177`), any path
   that still calls it will flip the terminal `Failed` back to
   `RetryPending`. The fast-fail must bypass the shared retry arm, not just
   write Failed first.
3. Leave the 600 s reaper alone — it is a crash-window backstop, not on this
   path (§2.3), and shortening it would change semantics for genuinely
   long-running snapshot dispatches on legacy nodes.

**Blast radius:** this changes behavior for *all* refused RPCs (§2.5), which
is the point, but also means an op dispatched during an agent rolling
upgrade (method about to become supported) now fails fast instead of
retrying into success. Operator re-issues; acceptable and more honest than
70 s of noise, but it is the reason to ship it as its own change.

## 8. Test strategy

House patterns: CP lifecycle tests use a real `TestDb` + seeded repos
(`tests.rs:3746-3767`, the #380/#381 clone suite); BFF integration tests use
a recording `MutationService` + real SQLite pool
(`crates/chv-webui-bff/tests/volume_snapshot_clone.rs`); orchestrator tests
drive `tick()` against mock agents served over real sockets
(`orchestrator.rs:2665-2739`, the #368 suite).

**Carrier (agent + CP inventory):**
- agent: `build_inventory*` embeds `authority_mode` from config for all three
  modes (`chv-agent-core` inventory tests);
- CP: `report_node_inventory` persists it (round-trip, `inventory.rs` tests);
  the lifecycle's mode lookup reads it back.

**CP lifecycle rejection (pins option B):**
- core-managed node + each of the four ops → `InvalidArgument` with the
  agreed message;
- **no rows journaled on rejection**: no `operations` row, no
  `volume_desired_state` snapshot intent, and (clone) no target `volumes`
  row — the #381-style no-residue assertion;
- legacy node + unknown/absent-mode node → still accepted (fail-open pinned);
- placement nuance: clone checks the **source's** node (placement node,
  `lifecycle.rs:1080-1088`), not the request's node_id.

**BFF (contract pass-through):**
- the four routes return HTTP 400 with the CP's message when the mutation
  service rejects (a `RecordingMutations` returning `BffError::BadRequest` —
  pins that `map_ack`'s `InvalidArgument` arm keeps working and the 200-
  accepted shape is not silently restored).

**Orchestrator (fast-fail piece, if shipped):**
- mock agent returns tonic `Unimplemented` → after one `tick()`, the op is
  terminally `Failed` with the terminal error code and the agent's message;
  `retry_count` stays 0; no `RetryPending` row ever appears (pins the no-
  resurrection requirement of §7.2);
- a `BackendUnavailable`/timeout mock still retries (guard against
  over-failing transient classes);
- existing retry-curve tests keep passing for retryable errors.

**chvctl:** the error path already surfaces BFF 4xx text; no new test surface
beyond the existing field-shape pins (`volume_snapshot_clone.rs`).

## 9. Rollout & rollback

Order matters only for the carrier:

1. **Agent-side mode reporting first** (proto field is additive; old CPs
   ignore unknown fields; new-column migration is a no-op for old binaries).
   Rollback: inert.
2. **CP rejection second.** Fail-open on unknown mode means zero behavior
   change until a node has actually reported core-managed. Rollback: revert
   the CP change; journaled state is untouched because rejections journal
   nothing.
3. **Fast-fail piece independently**, after its own soak.

In-flight ops accepted before step 2: run today's bounded ~70 s path to
`Failed/DISPATCH_FAILED` (§5 race note). No migration rollback hazard: the
new column is write-only for old code paths.

## 10. Residual risks

- **Carrier staleness (≤ ~30 s + reconnect):** a node that just flipped to
  core-managed can still get ops accepted in the window; the agent's
  fail-closed dispatch remains the enforcement. Fail-open on unknown mode
  extends this to never-reported nodes — deliberate (§6.3). The mode read
  and the journal write are also not atomic: a mode flip landing between
  them is the same already-disclosed carrier-staleness outcome with a
  microsecond-wide window, and the agent-side `Unimplemented` enforcement
  is the unchanged backstop.
- **Stale journaled intents** (§5): pre-existing `snapshot_op` intents for
  core-managed nodes are neither cleaned by this fix nor executed; they
  execute if the node ever flips back to legacy. Possible follow-up scrub.
- **Policy list drift:** the four rejected surfaces are an explicit list in
  the CP; the sibling surfaces of §2.5 (VM snapshot/restore, attach/detach,
  resize, overlay…) keep the accept-then-fail UX until separately decided —
  the complaint class survives in narrowed form.
- **Fast-fail piece (if shipped):** verbatim-retry-to-success across an agent
  rolling upgrade is lost; ops fail fast with a clear code instead.
- **`InvalidArgument` semantics:** 400 for "unsupported here" is a small lie
  the issue chose; if API consumers start branching on it, the 501 option
  (§5) remains open.

## 11. Non-scope (restated)

- Volume ops under Core authority (Core M1 modeling; also unlocks #379).
- Any legacy-mode behavior change.
- The clone 16-byte `ResourceId` target-id trap (`domain.rs:96-99`) —
  pre-existing validation, keep rejecting before journaling as today.
- The 600 s reaper bound and the generic dispatch-retry curve for
  non-`Unimplemented` errors.

## 12. Implementation record (as built)

The design is implemented as decided (§6.1); the following are the concrete
mechanics and the places where the implementation chose a specific shape the
writeup left open. None of them change the semantics the design pins.

- **The proto enum carries all three agent modes, not two.** The writeup's
  §6.2 sketch listed `UNSPECIFIED/LEGACY/CORE_MANAGED`; the enum also
  carries `CORE_NATIVE` for completeness/future-proofing of the
  config→proto mapping — it is unreachable in the CP today, because a
  core-native agent returns into `run_core_native`
  (`cmd/chv-agent/src/main.rs`) before enrollment and before the legacy
  gRPC server binds, so it never enrolls or reports inventory at all. The
  rejection helper fail-opens on it regardless (it fires only on a
  definite `core-managed` report). The enum is
  `AuthorityMode { AUTHORITY_MODE_UNSPECIFIED = 0; AUTHORITY_MODE_LEGACY = 1;
  AUTHORITY_MODE_CORE_MANAGED = 2; AUTHORITY_MODE_CORE_NATIVE = 3; }`
  (`proto/controlplane/control-plane-node.proto`), field 13 on
  `NodeInventory`. Values follow the repo's enum convention (every value
  prefixed with the enum name, like `MigrationPhase`).
- **The column stores the agent config's kebab-case spelling**
  (`legacy` / `core-managed` / `core-native`) as text, with the three
  spellings exported as `chv_controlplane_store::
  AUTHORITY_MODE_{LEGACY,CORE_MANAGED,CORE_NATIVE}` constants. Migration
  `0057_node_authority_mode.sql` (nullable, backfill-free). The upsert
  COALESCEs the new column like the version columns: an UNSPECIFIED
  re-report (pre-#378 agent binary) cannot wipe an established mode, while a
  mode flip at agent restart overwrites on the next report (~30 s).
- **Legacy→core-native flip staleness (known, accepted):** a node enrolled
  as legacy that later flips to core-native keeps its stale legacy mode
  (or NULL) until it re-reports — the old accept-then-fail UX persists for
  that node in that window. This is consistent with the adopted fail-open
  policy: mode only changes at agent restart, and the agent-side
  `Unimplemented` enforcement remains the backstop.
- **COALESCE + binary-downgrade combo (known, accepted):** if a node flips
  to legacy while simultaneously downgrading to a pre-#378 agent binary
  (which reports no `authority_mode` → UNSPECIFIED), the COALESCE preserves
  the last-known mode and the node stays wrongly rejected until a
  mode-capable binary re-reports. Direction-consistent with the
  "UNSPECIFIED can't wipe an established mode" guarantee — a disappearing
  field is treated as no-new-information, not as legacy. Rare, self-healing
  on upgrade.
- **Node resolution per surface.** All four surfaces check the node the
  dispatch actually uses. The orchestrator resolves the dispatch node from
  the VOLUMES row — `(SELECT node_id FROM volumes WHERE volume_id =
  operations.resource_id)` in the Accepted/RetryPending claim queries
  (`orchestrator.rs:183-187` / `:216-219`) — never from the request, so
  snapshot/restore/delete-snapshot resolve `volumes.node_id` for the
  request's volume (`ensure_volume_snapshot_family_supported`,
  `lifecycle.rs`); the request's own `node_id` field is effectively
  advisory for the mode check on those three surfaces. The BFF always
  sends the volume's node, so BFF/chvctl callers see no difference; a
  direct-gRPC caller naming a mismatched node is checked against the
  volume's node — the node that will actually dispatch the op — closing
  both mismatch directions the round-2 review flagged (request names a
  core-managed node for a legacy-owned volume: no false 400; request
  names a legacy node for a core-managed-owned volume: rejected instead
  of ~70 s of `Unimplemented` retries). When the volume's node is
  unknown — no `volumes` row yet, or a NULL `volumes.node_id` (the
  column is `ON DELETE SET NULL`) — the check fails open rather than
  falling back to the request node. Clone checks the placement node —
  the source volume's node — exactly the #381 resolution and the same
  node the target row materializes on (so the orchestrator's
  volumes-row resolution returns it), pinned by
  `clone_volume_rejects_core_managed_placement_node` (request
  names a legacy node, source lives on a core-managed node → rejected).
- **The rejection helper is one private method** on
  `LifecycleServiceImplementation`
  (`ensure_volume_snapshot_family_supported`, `lifecycle.rs`), called by
  the snapshot/restore/delete-snapshot handlers before
  `create_operation_and_emit` (it resolves the volume's node and fails
  open on unknown); clone calls the underlying node-level check
  (`ensure_node_not_core_managed`) with its already-resolved placement
  node. Message shapes:
  "volume snapshot / restore / snapshot deletion / clone is not supported
  on core-managed nodes".
- **Unknown enum ints fail open too.** The ingestion mapping
  (`inventory.rs::authority_mode_text`) uses
  `AuthorityMode::try_from(i32)` and maps every unrecognized value to NULL
  alongside UNSPECIFIED — a future mode added to the proto is inert on an
  old CP instead of being mis-stored.
- **Enrollment carries the mode as well** (the writeup flagged the
  enrollment `NodeInventory` as a possible first-contact carrier; it is
  wired through the same `authority_mode_text` mapping), so a node is
  knowable from its first contact, not just after the first ~30 s cycle.
- **The BFF contract is pinned end-to-end over HTTP**, not only with a
  recording mock: the new test drives the real router → real
  `ControlPlaneMutationService` → real lifecycle against a real TestDb
  (`volume_snapshot_rejected_over_http_on_core_managed_node`), because the
  BFF and CP share the store and the interesting seam is `map_ack`'s
  `InvalidArgument` arm plus the no-journal property together.
- **Red/green spot-check:** inverting the helper's mode comparison (reject
  `legacy` instead of `core-managed`) fails exactly the four rejection
  tests, the legacy-accept pin, and the mismatched-accept pin
  (`snapshot_family_ignores_core_managed_request_node_when_volume_on_legacy_node`
  fails *because* the check follows the volume's legacy node rather than
  the request's core-managed node — the pin on the #495 volume-node
  resolution), and nothing else; neutralizing the rejection entirely
  fails the four rejection tests plus the mismatched-reject pin
  (`snapshot_family_rejects_legacy_request_node_when_volume_on_core_managed_node`).

### 12.1 The §7 `Unimplemented` fast-fail (as built, #498 + its overlay follow-up)

The §7 piece was shipped separately as recommended (§6.1 item 4), in #498,
with one surface completed by #498's follow-up PR. As built:

- **Identity carrier**: a typed `ChvError::Unimplemented { reason }` variant
  (chv-errors), populated by the node client's `with_timeout` for tonic
  `Code::Unimplemented` — no status-text string-matching; every other tonic
  code flattens to `Internal` exactly as before, and the message text is
  byte-identical between the two arms so the agent's refusal explanation
  still rides the error.
- **Tick bypass**: the tick's error handler classifies `Unimplemented` as
  terminal at dispatch and skips the shared retry arm entirely. This bypass
  is load-bearing: `mark_for_retry`'s UPDATE has no status guard, so any
  path that still called it would resurrect the terminal `Failed` row back
  to `RetryPending`.
- **Terminal row**: `dispatch_operation`'s single-node error arm writes
  `Failed` with `error_code: UNSUPPORTED_BY_AGENT` (distinct from
  `AGENT_REJECTED` and `DISPATCH_FAILED`) carrying the agent's own refusal
  message, before the error propagates to the tick.
- **Terminal-write-failure convention**: if that terminal write itself
  fails, the `Unimplemented` identity is deliberately flattened to
  `Internal` — the row stays retryable and a re-dispatch re-derives the
  terminal outcome. Never a stranded un-retryable row.
- **Overlay fan-out completion (follow-up PR)**: `UpdateOverlay` is the one
  dispatch whose error is an aggregation over per-node failures
  (`OverlayManager::send_fabric_update`). #498's single-node arm did not
  cover it: the aggregation rebuilt a fresh `Internal`, erasing the
  refusal identity, and `dispatch_update_overlay` had no error arm writing
  terminal rows — so an all-core-managed network still ran the full
  10/20/40 s retry curve. The follow-up closes both gaps: (a) when EVERY
  per-node failure is an `Unimplemented` refusal, `send_fabric_update`
  returns one `Unimplemented` carrying the deterministic per-node roll-up
  (fan-out order — deterministic because the planner walks the peer list
  returned by `get_fabric_peers_for_network`, which the store orders
  `BY node_id`; the roll-up text reads "refused by all N failing
  node(s)" since the partial-success shape means the failing nodes, not
  all participants, refused) instead of `Internal`; (b) `dispatch_update_overlay`
  gained the terminal-write arm mirroring the single-node path (`Failed` /
  `UNSUPPORTED_BY_AGENT` before propagating, same flatten-on-write-failure
  convention), so the tick's existing bypass lands the operation terminal.
  Mixed fan-out failures (some refusals, some other classes) keep the
  `Internal` aggregation and the retry curve unchanged — a partial refusal
  is not terminal-class for the operation.
- **Pinned by**: `unimplemented_dispatch_fails_fast_without_retry`,
  `unimplemented_snapshot_vm_dispatch_fails_fast_without_retry`,
  `unavailable_dispatch_retries_exactly_as_before`,
  `transport_unavailable_dispatch_retries_as_before` (the controls), and
  the overlay legs
  `update_overlay_fan_out_all_unimplemented_fails_fast_without_retry` /
  `update_overlay_fan_out_mixed_failure_keeps_retry_semantics`
  (orchestrator tests).
