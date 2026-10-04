# #368 design writeup — journaled create re-drive (C1, implemented)

**Issue:** kubedoio/chv#368 — a transient backend failure terminally fails a
journaled VM create and nothing ever re-drives it (N7).

**Outcome:** the C1 design below (recommended option in §4) is implemented and
ships with this document. §2 (why a CP-level re-drive alone cannot converge)
and §4 (the Core-owner decision) are retained verbatim as the design
rationale; §7 records the test matrix **as implemented**, and §10 records
where the implementation diverges from the original writeup (all named, none
semantic). The worktree branch is `fix/journaled-create-redrive-c1`.

---

## 1. State-machine map (what exists today)

### CP side (`chv-controlplane-service`)

| Surface | Who writes it | When |
|---|---|---|
| `vm_desired_state` (`vms`) | `LifecycleService::create_vm` / `start_vm` / `stop_vm` | CreateVm accept writes `desired_power_state="Created"` (gen 1); StartVm writes `"Running"` (gen+1) |
| `vm_observed_state` | agent telemetry (`VmStateReport` → `ObservedStateRepository` upsert) | Only for VMs the agent **reports**. In core-managed mode `reported_vms()` derives from NodeCache fragments, which are projected **only from Succeeded Core ops** (plus the startup `rebuild_from_core`, which seeds from the journal's *live* VM list — so after an agent restart a phantom is reported with `runtime_status` = its *desired* state) |
| `operations` | `Orchestrator::dispatch_operation` | CreateVm/StartVm marked **Succeeded at submit-level ack** (the M4.7-confirmed lie: "CP op Succeeded while the core effect failed"). RPC-level failures retry via `mark_for_retry` (MAX_DISPATCH_RETRIES, 10/20/40 s backoff) then Failed |
| drift gauges | `Orchestrator::tick` | desired≠observed generation is counted **for metrics only** — no VM convergence loop exists anywhere in the CP |

### Agent / Core side (core-managed mode)

1. **Submission:** gRPC (`apply_vm_desired_state` gen 1 / `create_vm` / `start_vm`) →
   `legacy_core_adapter` → `authority.submit` → journal `accept_operation`.
   CreateVm acceptance **durably INSERTs the `vms` row at version 1 before any
   effect**; a failed create does *not* tombstone it.
2. **Execution:** `JournalExecutor` claims `accepted` ops. An effector error
   (e.g. `RuntimeFailure::RuntimeUnavailable` from a dead nwd/stord) terminal-
   persists the op as `failed` and it is **never re-claimed** (M2.4).
   Restart-interrupted (`running`) ops become `InspectRequired` → operator
   `resolve` RPC. Ambiguous in-process failures quarantine the VM.
3. **Identity:** `vms` has an unconditional PRIMARY KEY and no `DELETE FROM
   vms` exists anywhere — a tombstoned id is **never reusable**.
4. **Power ops on a never-created VM:** `StartVm` effector → process adapter
   `start_vm` → no runtime entry and no persisted payload to re-derive →
   `NotFound` → terminal `failed` (the observed `StartVm-46ff7fa0
   {"code":"NOT_FOUND"}`).

### What converges today vs. what never does

- Converges: Succeeded ops (generation ack); **manual** delete of the phantom
  (works — M4.7 evidence); retry-create with a **new** VM id after cleanup
  (the qual harness's F1b pattern).
- Never converges: **desired-Running / observed-absent after a terminally
  failed create** — the #368 class. The zombie is silent until an agent
  restart, after which it is worse: reported as its desired state.

## 2. Proof that CP-level alone cannot converge this case

After a terminally-failed journaled create, the journal holds a **live** `vms`
row for the id. Enumerate every path the CP could drive through the existing
authority submission surface (`OperationService::submit` / the v1 Core API):

1. **Re-submit CreateVm (expected version 1):** `persist_accepted_desired_state`
   INSERTs into `vms` → PK conflict → `StoreError::Conflict` → RPC CONFLICT.
   True for a live row *and* for a tombstoned one (unconditional PK, no
   `DELETE FROM vms`). **Impossible.**
2. **UpdateVm:** refused at the agent's dispatch boundary and fails closed
   `Unsupported` in the executor (M1 scope; resize-through-Core deferred #234).
   **Impossible.**
3. **StartVm / RebootVm:** journaled and executed, but the effector calls the
   process adapter against a VM that was never created → NotFound → terminal
   `failed`. Power ops cannot re-provision storage/network (the create
   effector owns that). **Cannot converge.**
4. **DeleteVm:** works — converges to **absence** (best-effort residue drain),
   never to Running, and burns the id forever. **Converges the wrong way.**
5. **`resolve_inspect_required`:** applies only to restart-interrupted
   (ambiguous) operations. A clean effector failure is *decided*, not
   ambiguous; re-classifying terminal failures as inspect-required would erase
   the M2.4 ambiguity contract. **Not admissible.**

Additionally, the CP's own idempotency blocks a naive re-drive: a re-submitted
`CreateVm` with the same generation+spec discriminator resolves to the
**original** (Succeeded) operation row via `create_or_get` and is never
re-dispatched (the orchestrator only claims `Accepted`/`RetryPending`).

Therefore any re-drive that converges to a **booted VM with the same id**
requires a Core-executor/journal semantics change. That is the decision to
put to the Core owner.

## 3. Residue analysis (the hard part)

- The CreateVm effector **already unwinds partial creates best-effort**
  (`teardown_partial_create`: reverse NIC detach, reverse volume
  detach+close, vm-dir removal). Residue survives exactly when the failing
  provider is the one being torn down against (both halves hit the dead
  daemon) — the M4.7 F3 shape.
- The effector is **substantially idempotent against residue**, so
  re-execution is the correct convergence action:
  - stord `open_volume` replays the existing session for the same
    volume+absolute path; the local backend's `attach` is path-based (no
    AlreadyExists); seeding skips already-provisioned volumes;
  - `ensure_vm_runtime_dir` is `create_dir_all`; nwd's
    ensure-topology/attach handlers are idempotent.
- A **pre-cleanup delete is the wrong arm**: it tombstones the id forever
  (destroying resource identity) and cannot even drain the residue it doesn't
  know — the delete-time fallback drain keys off the NodeCache projection,
  which is **absent** for failed creates (projection only follows Succeeded
  outcomes). This rules out "cleanup-then-recreate-same-id" designs on two
  independent grounds.

## 4. Decision needed (Core owner)

> How may a VM whose journaled CreateVm terminally failed be re-driven to
> convergence, given that the desired state is durably journaled, the failed
> op must stay terminal (M2.4), the VM id is permanently occupied, and the
> executor is the single writer?

Options:

- **C1 (recommended): a requeue/recreate submission primitive.** New
  authority mutation (e.g. `RequeueCreate { vm_id }`) that CAS-checks the
  `vms` row is live at the expected version, requires the latest create op
  for the VM to be terminally `failed`, and inserts a **new** CreateVm-kind
  operation (fresh idempotency key, definition re-derived from the live
  journal row) in `accepted` state. The existing create effector re-executes
  residue-idempotently. **Required new executor semantics:** a claim/execute-
  time fence so a requeued create never runs against a tombstoned VM (a
  concurrent DeleteVm tombstones at accept; nothing today can produce a
  claimable create for a tombstoned VM, so this window is new and must be
  closed inside Core).
- **C2: bounded journal-level retry of transient effector failures (issue
  option 1).** Classify `RuntimeFailure::RuntimeUnavailable` as
  retryable-with-backoff at claim time (`retry_count`/`max_retries` already
  exist as claim/restart semantics). Touches terminality directly, needs the
  same delete-fence, and still does not cover crash windows
  (InspectRequired stays manual) — the CP reconcile remains necessary above
  it. Strictly more invasive to M2.4 than C1 for less coverage.
- **C3: allow create-over-tombstone (partial PK `WHERE deleted_at IS NULL`)
  + CP delete-and-recreate re-drive.** Executor untouched, but changes
  journal identity semantics (id reuse), destructively cycles the VM, and
  hits the fallback-drain blindness from §3. Not recommended.

## 5. CP-level pieces (needed under C1 or C2; per issue option 2)

- **P1 — evidence channel:** the agent reports terminally-failed core creates
  to the CP through the existing `VmStateReport` telemetry
  (`runtime_status="Failed"`, `last_error`=failure code; the CP already
  upserts this into `vm_observed_state`). No proto change required; the
  agent-side loop already holds the authority handle.
- **P2 — reconcile re-drive:** in the orchestrator tick, for VMs whose
  desired state still demands them AND whose agent-reported state says the
  create terminally failed AND with no incomplete create op in flight: submit
  the re-drive through the chosen Core primitive, **bounded** (reuse the
  dispatch retry pattern: backoff + max attempts), then mark the CP VM row
  Failed-with-reason on exhaustion. No new config knob if the existing
  dispatch-retry bounds are reused.
- **P3 — visibility:** P1's report makes the BFF render `Failed` with a
  reason instead of a phantom; on re-drive success it flips to Running.

## 6. Failure-mode table (composite C1 + P1/P2/P3)

| Scenario | Behavior |
|---|---|
| Transient backend failure, then recovery (#368 class) | Create fails terminally → agent reports Failed → CP re-drive re-executes the residue-idempotent create → VM boots, observed flips to Running. No operator action. |
| Permanent backend failure | Each re-drive is a new journaled op and fails again; CP bounds re-drives (backoff + max) → VM row marked Failed with the effector's failure code. Bounded, logged residue; never a silent zombie. |
| Crash during re-drive (agent dies mid-execution) | The requeued op is `running` with an attempt token → restart classification marks it `InspectRequired` (existing M2.4 machinery, unchanged). CP gates further re-drives on "no incomplete create op for the VM"; operator resolves via the existing resolve RPC; reconcile re-arms if still failed. |
| Concurrent operator delete during re-drive | Delete accepted first (version CAS + tombstone-at-accept) → C1's claim/execute fence refuses to run the requeued create for a tombstoned VM → op terminal-fails with a conflict code; CP observes desired state gone and stops re-driving. No double-create, no resurrection. |

## 7. Test matrix (as implemented)

Core tier:

- `cellhv-core-store`: requeue accepted only for live-row + terminally-failed
  latest create (CAS arms: wrong version → precondition, succeeded latest →
  conflict, tombstoned → precondition); requeue is idempotent under duplicate
  submission; the requeued operation's request is a canonical create envelope
  derived from the live row; the claim-time tombstone fence terminally fails
  a claimed create against a tombstoned VM with `VM_TOMBSTONED` in the same
  transaction (no run, no quarantine).
- `cellhv-core-operations`: the service facade + actor requeue paths
  (durability, replay convergence, refusal error classes); cross-crate
  envelope-shape pin; `crash_during_redrive_preserves_the_requeued_create_as_inspect_required`
  — a re-drive claimed then abandoned across a journal restart is classified
  `InspectRequired` by the existing restart machinery, and the original
  failed create stays terminal.
- `cellhv-core-executor`: the fence surfaces as `ClaimDisposition::Refused`
  with a `claim_refusals` counter, no effector run.
- Residue idempotency: `chv-agent-runtime-ch`
  `create_reexecution_over_attached_residue_converges` (mock tier — a second
  create execution over the same definition re-runs the full open/attach set
  and converges) and `chv-stord-core`
  `create_redrive_residue_is_idempotent_against_the_local_backend` (local
  backend — re-open with the same provisioning hints does not re-seed/resize;
  re-attach is idempotent).

Agent tier (`chv-agent-core`):

- #368 P1 merge (`reconcile.rs`, the five `apply_core_create_states` tests):
  a terminally failed latest create reports `Failed` with its public-safe
  code; `Unsupported` without a code falls back to `CREATE_FAILED`; an
  in-flight (`Accepted`/`Running`, the inspect-required shape) create reports
  `Pending` instead of the desired-state phantom; a fragmentless failed
  create is appended as a synthetic record (observed generation "0");
  succeeded creates do not report `Failed`.
- #368 review round 2 — resolve→P1-report join
  (`agent_server.rs` `resolve_inspect_required_as_failed_reports_failed_state_for_redrive`):
  a re-drive claimed and interrupted by a process crash is classified
  InspectRequired after a journal restart, P1 reports `Pending` while it is
  unresolved, and the operator's resolve-as-failed (through the real resolve
  RPC against a real authority) makes P1 report `Failed` with
  `OPERATOR_RESOLUTION` — the exact telemetry state the CP pass keys on.

CP tier (`chv-controlplane-service`, orchestrator tests):

- `redrive_failed_create_issues_new_recreate_operation` — happy-path
  selection: the zombie shape gets a NEW `RecreateVm` op at the original
  create's generation; the failed op stays terminal; a healthy VM is not
  touched.
- `redrive_refuses_operator_deleted_vm` — operator delete stops the loop.
- `redrive_refuses_incomplete_create_family_in_flight` — any incomplete
  create-family op (`create`/`CreateVm`/`RecreateVm` in
  Accepted/RetryPending/Running) holds the gate closed (the CP half of
  crash-during-re-drive safety).
- `redrive_backoff_spaces_attempts` — re-drives are spaced by the
  dispatch-retry backoff curve.
- `redrive_exhaustion_marks_vm_failed_with_reason` — exhaustion marks a
  terminal Failed `RecreateVm` op with `CREATE_REDRIVE_EXHAUSTED` and the
  reported failure code; the fixed marker key converges repeat ticks.
- `redrive_skips_failed_vm_without_journaled_create` — no journaled create to
  derive a generation from → refuse rather than guess.
- `recreate_vm_dispatches_via_apply_vm_desired_state` — end-to-end at mock
  tier: the re-drive dispatches to the agent as `ApplyVmDesiredState` fenced
  on the original create's generation, and the agent's ok ack converges the
  operation.
- `redrive_rearms_after_operator_resolves_inspect_required` (review round 2)
  — the CP half of the resolve→report→re-drive join: the agent-tier test's
  exact reported state (`Failed`/`OPERATOR_RESOLUTION`, fed through the real
  telemetry ingestion) re-arms the pass — attempt+1 issues — while the
  unresolved (`Pending`-reported) window does not.

## 8. Residual risk of the recommended design

- Operations-table growth: each re-drive is a new `RecreateVm` row and
  exhaustion adds one marker row; they accumulate with no retention,
  consistent with the pre-existing no-retention pattern for the whole
  `operations` table, and are bounded at ≤ `MAX_DISPATCH_RETRIES`+1 rows per
  failed-VM lifetime.
- Operator note on exhaustion: `CREATE_REDRIVE_EXHAUSTED` is permanent for
  that VM's life — the bound never resets. The only recovery is delete +
  recreate (a new VM id; no re-drive debt carries across lives).
- The requeue primitive widens the executor's reachable states by one
  (claimable create for a previously-failed VM); the tombstone fence is the
  load-bearing guard and must be tested adversarially.
- Residue idempotency is pinned at the mock tier and against the stord
  **local** backend; the non-local backends (ceph/iscsi/lvm) carry the same
  idempotent open/attach contract but are **not separately pinned** — an
  explicit attach-idempotency pin per backend remains open work.
- P1's report makes the phantom visible but the startup-rebuild path can
  still report a phantom as its desired state after an agent restart; P1
  prefers the journal's terminal-failure signal (`apply_core_create_states`
  merges Failed/Unsupported from the Core create states over the projected
  fragment) but the rebuild-seeded fragment itself is unchanged.
- The dispatch shim's create-vs-redrive routing assumes create-family ops
  arrive at generation 1; the shim's `generation != 1` → Unimplemented gate
  (which precedes it) makes this true today, but a future "create at higher
  generation" change must revisit that gate or re-drive silently stops
  routing (noted at the routing block in `chv-agent-core/src/agent_server.rs`).

## 9. Non-scope (unchanged from the issue)

The nwd/stord restart triggers themselves (M4.4 trigger closed by #369; the
daemon-restart-window RPC failures remain separately investigated). No config
knobs are proposed: the re-drive bounds reuse the orchestrator's existing
dispatch-retry constants.

## 10. Implementation record (divergences from the writeup above)

The C1 design is implemented as written; the following are the concrete
mechanics and the named places where the implementation chose a specific
shape the writeup left open. None of them change the semantics the design
pins.

- **P2's re-drive is a new CP operation type, `RecreateVm`.** The writeup's
  §5 said "submit the re-drive through the chosen Core primitive"; the
  implementation journals the re-drive in the CP `operations` table as a new
  `RecreateVm` operation (deterministic idempotency key
  `recreate:{vm}:{create_generation}:{attempt}`) dispatched through the same
  `apply_vm_desired_state` arm a fresh create uses, at the ORIGINAL create's
  generation. The agent's dispatch shim routes it to the Core requeue
  primitive when the journal holds a terminally failed create and acks
  idempotently when it has converged.
- **The agent shim replays on task identity before routing on create
  status.** A dispatcher retry of either the original create or the re-drive
  must converge on the already-journaled operation, so the shim derives both
  the legacy create operation id and the `:requeue`-suffixed re-drive id from
  the incoming task and replays idempotently BEFORE consulting the latest
  create state (None → fresh create; Failed → re-drive; Succeeded →
  idempotent ok; Accepted/Running → failed-precondition).
- **The Core requeue operation id is the CP operation id with a `:requeue`
  suffix** (the legacy adapter's `legacy_requeue_operation_id`), keeping the
  re-drive's journal identity distinct from the original create's while
  still deriving deterministically from the dispatched task.
- **P3 is a Failed-wins render, not an exhaustion-only marker.** The BFF
  renders `runtime_status='Failed'` as the VM's power state whenever the
  agent reports it (a `CASE WHEN` over the observed/desired columns) and
  surfaces `last_error`, rather than flipping a flag only when re-drives
  exhaust. The exhaustion marker (`CREATE_REDRIVE_EXHAUSTED`) is the
  backstop that guarantees a reason is present. The same precedence applies
  to all three render surfaces — the VM list, the VM detail view, and the
  node-detail hosted-VM list (the node list was missed in the first pass and
  closed in the review round; pinned by the BFF
  `vm_failed_state_render` suite).
- **`redrive_failed_creates` derives `create_generation` from the FIRST
  journaled create op** for the VM; a failed VM with no journaled create is
  skipped with a warning (refuse rather than guess — pinned by
  `redrive_skips_failed_vm_without_journaled_create`).
- **Re-drive spacing reuses the dispatch-retry backoff curve** (10 s ·
  2^(attempt−1) from the previous re-drive's `updated_at`) and the same
  `MAX_DISPATCH_RETRIES = 3` bound; each re-drive is a new `RecreateVm`
  operation (the CP retry machinery's `mark_for_retry` is not used).
