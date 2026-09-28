# Hardening sweep R1 — restart-interruption markers, executor fatality, resolution egress, telemetry honesty — evidence

> Campaign: [production-readiness](/docs/prompts/production-readiness/README.md)
> Prompt: [02-single-authority-cutover](/docs/prompts/production-readiness/02-single-authority-cutover.md)
> Parent milestone: inter-milestone hardening pass between M2.3 (COMPLETE) and
> M2.4. A comprehensive review of post-M2.3 main (subagent lenses:
> correctness/concurrency, API/contract/security, regression/blast-radius, then
> process flows, downstream consumers, spec contracts) triaged 15 findings
> (F1–F3 MAJOR, F4–F8 MINOR, F9–F15 INFO) plus five review rounds on the fixes;
> this pass lands them.
> Capability maturity (honest): **CODED / CI-VERIFIED** only. KVM-VERIFIED and
> above are **unproven** on this host (see §7).

---

## 1. Baseline and change

- **Baseline SHA:** `38077c86` (post-M2.3 main; all dependabot PRs resolved).
- **PR:** [kubedoio/chv#269](https://github.com/kubedoio/chv/pull/269)
  `hardening/r1-sweep` → **merged to main as `4e3ae1f5`** (squash, 7 commits:
  R1 sweep → R2 fixes → R3 closures → R4 → R5 → R7 process/consumer/contract
  fixes → R8 closure). Reviewed through an **8-round adversarial loop**:
  R1 findings → R2 three-lens review → R3/R4/R5 delta reviews → R6 convergence
  on the original diff → R7 fresh-lens review (whole-process integration flows,
  downstream consumers/operator discoverability, authoritative-doc contract
  accuracy) → R8 delta verification (code lens clean; two doc MINORs fixed).

## 2. Problems fixed (the MAJORs)

- **F2/F2′ — permanent quarantine after agent restart.** Every `running`
  core-journal operation was re-classified `InspectRequired` by the 250 ms
  executor scan after a restart — including operations in flight in the
  *current* process — quarantining their VMs for the process lifetime with no
  egress to resolve them.
- **F3 — silent executor death.** A task panic closed executor ingress but the
  process stayed up, healthy-looking, while the authority kept acknowledging
  operations that were never executed.
- **F1 — blind core-managed telemetry.** The control plane saw zero VMs in
  core-managed mode (the report loop read a map only legacy reconcile
  populates), and the observe-only Reconciler had no `Draining` arm — a
  core-managed node reported Draining→Maintenance instantly while VMs were
  still desired-Running.
- **R7 additions:** exit-0 on agent gRPC server loss (defeated systemd
  `Restart=on-failure`, stranding the sole Core authority); InspectRequired
  operations not discoverable by operators (the "visible" half of visible +
  resolvable was unmet); BFF counting desired state as observed workloads and
  snapshot-restore deciding on desired state; authoritative specs contradicting
  the shipped semantics.

## 3. Core semantics (as merged)

`running` **without** a restart-interruption marker = in flight in this
process — excluded from the restart snapshot, never re-classified by the
executor scan. `running` **with** a marker = stuck (`InspectRequired`):

- written once at startup classification (on the service, **before**
  `AuthorityActor::spawn`, so no claim can exist; failure fails the process
  closed via `RecoveryStartup`), or by executor abandonment (best-effort);
- **visible**: `/v1/operations` and `/v1/operations/:id` entries carry
  `recovery_assessment` (stuck = `running` + assessment; in-flight = `running`
  without); the journal-scan poller warns on inspect-required set changes;
  startup classification warns;
- **resolvable**: `LifecycleService.ResolveInspectRequiredOperation`
  (node-scoped agent gRPC; the control plane fails closed with
  `Unimplemented`). The store transition requires the marker + terminal-status
  match + completed-token + evidence fingerprint → replays are idempotent;
  resolution terminal-persists under the original attempt token and the VM is
  un-quarantined on the next executor scan.

Executor fatality: `ExecutorError::Fatal` is first-wins; `chv-agent` exits
**non-zero** in both core-native and core-managed mode (after Degraded is
persisted and reported) so the supervisor restarts it — and now also on agent
gRPC server loss. Quarantine is split: failure-quarantine is process-sticky,
inspect-quarantine is re-derived per scan. `ClaimReplay` is counted, never a
failure, never quarantine-worthy.

Telemetry honesty: `Reconciler::reported_vms()` reports live `VmRuntime`
records (legacy) or cache fragments (observe-only); core-managed VM reports
carry `health_status: "Unknown"` (desired state reported as `runtime_status` —
documented residual); the observe-only `Draining` arm is fail-closed (only
provably desired-Stopped fragments drain; undecodable fragments block), issues
no migration events, and raises a `drain_blocked` control-plane alert on
change. BFF metrics/overview count only observed-known states; snapshot
restore fails closed on unknown observed state.

## 4. Invariants held

- **Harden, don't break legacy** (M2.3 decision): legacy mutation paths
  retained; legacy drain arm byte-identical; legacy telemetry unchanged.
- Single-writer: the Core runtime stays the sole effector in core-managed
  mode; the observe-only Reconciler cannot mutate.
- Resolve is destructive-recovery only: marker + token + fingerprint fencing.
- Fail-closed defaults: startup classification failure, unknown authority
  state, undecodable fragments, blank identities, control-plane resolve
  routing, snapshot restore on unknown observed state.

## 5. Spec/contract alignment (in the same merge)

`cellhv-core-operation-model.md` §6 (marker-based restart classification),
`cellhv-core-recovery-assessment-journal.md` (resolution transition shipped),
`cellhv-core-journal-executor.md` (replay counted not quarantined; panic
process-fatal; cancellation chains through the marker),
`cellhv-core-authority-actor.md` (recovery capabilities),
`ARCHITECTURE.md` (core-managed drain carve-out),
`component/chv-agent-spec.md` (observed-state deviation),
`ops/failure-matrix.md` (restart/panic/InspectRequired rows),
`OPERATIONS.md` (InspectRequired recovery runbook with grpcurl contract;
core-managed drain section).

## 6. Tests and verification

- Per-crate: executor 21, store 40, operations 33, runtime-owner 12 + canary 1,
  reconcile 29, agent_server 22, config 7, controlplane-service 130 + 7, arch
  peer tests 55.
- Full workspace: **1202 passed / 20 failed**, the 20 being documented
  host-environment-only failures (tempdir permission checks: chv-agent bin ×1,
  chv-agent-core cache ×6, chv-agent-runtime-ch ×13), verified identical on
  clean main `38077c86`.
- `cargo fmt` clean; `cargo clippy --workspace --all-targets -- -D warnings`
  clean; architecture gate (`scripts/check-cellhv-core-architecture.py`) and
  its 55 peer tests pass. No Cargo.toml/Cargo.lock changes.
- CI on the merged SHA: Rust checks, buf, E2E, UI checks, both Build and
  Package jobs — all green.

## 7. Real-host evidence (honest)

- **CODED + CI-VERIFIED only.** KVM-VERIFIED and above remain **unproven** on
  this host (no cloud-hypervisor/guest images installed; `/dev/kvm` present).
  The restart-interruption and fatality paths are exercised against
  in-memory/temp-journal test rigs, not a real supervisor-restart cycle under
  KVM. That qualification belongs to M2.5.

## 8. Remaining risks (documented, not fixed here)

- Abandonment marker is best-effort (startup classification covers authority
  outages; VM stays failure-quarantined in the window).
- `resolve_inspect_required` trusts the operator's disposition — by design;
  the audit record is the compensation.
- Identifier admission still accepts control characters (systemic; egress
  escaped on all new surfaces).
- Observed power state not reported in core-managed (BFF now honest: 0 running
  until observed-state reporting lands).
- Fatal-executor detection can lag up to the reconciler backoff (~60 s) after
  the Degraded report; executor drain completes after stord/nwd stop
  (pre-existing supervisor ordering).
- Nothing at the control plane *acts* on `drain_blocked` or stuck-`Draining`
  yet (agent surfaces both; CP reaction is future work).
- Volume-locator convention mismatch (F7), save-under-lock throughput, dead
  retry machinery, xyflow manual smoke, rollup natives, metrics
  lock-across-await, gate name-based blind spots — unchanged residuals.
