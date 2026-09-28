# Hardening sweep R2 — post-M2.4 comprehensive review — evidence

> Campaign: [production-readiness](/docs/prompts/production-readiness/README.md)
> Prompt: [02-single-authority-cutover](/docs/prompts/production-readiness/02-single-authority-cutover.md)
> Parent milestone: inter-milestone hardening sweep **R2** (successor of the
> M2.3-era [R1 sweep](hardening-r1-sweep.md), PR #269). M2.4 is **COMPLETE**;
> this sweep re-reviewed post-M2.4 `main` with five fresh lenses and fixed
> everything actionable. Capability maturity (honest): **CODED /
> CI-VERIFIED** only — same boundary as M2.4: the proofs run the real
> composition (real executor, store, control-plane services) with
> mock/fake runtimes where side effects would otherwise be required;
> KVM-VERIFIED and above remain unproven on this host (M2.5 scope).

---

## 1. Baseline and change

- **Baseline SHA:** `3cf7fdc2` (post-M2.4 evidence main).
- **PR:** [kubedoio/chv#272](https://github.com/kubedoio/chv/pull/272)
  `hardening-r2-sweep` → merged to main as `62db8bf6` (squash; the branch
  carries 5 thematic commits — core / agent / controlplane / packaging /
  docs — plus the architecture-guard redesign and the three review-loop
  fix commits the loop itself produced).
- CI on the merged SHA: see §8.

## 2. Method

Five parallel review lenses over post-M2.4 `main` (same discipline as R1;
findings consolidated and spot-checked before fixing):

- **Lens A** — core correctness & concurrency (executor, authority actor,
  store transactions).
- **Lens B** — durability (bootstrap atomicity, bounded enumeration, TLS
  material writes, backups, indexes).
- **Lens C** — security & trust boundaries (gRPC interceptor coverage, path
  traversal at every join, file permissions, secrets).
- **Lens D** — ops & consumer surface (runbooks, metrics, drain vs
  systemd timeouts, fleet-wide recovery, log quality).
- **Lens E** — tests & spec drift (flakes, superstition timing, composition
  gaps, stale spec status headers).

Consolidated result: **3 MAJOR + ~18 MINOR + ~10 INFO**. All three MAJORs and
every fixed MINOR were independently spot-checked (read the code path,
confirmed the failure mode is real) before a fix was written. The fix set
below is everything that landed in PR #272; feature-sized findings are
documented residuals (§6); a few findings were verified to be
non-findings (§7).

## 3. MAJOR findings and fixes

### 3.1 (A1) Failure-quarantine wedge: restart was the only release

A VM failure-quarantined by an in-process execution failure (ambiguous
claim/finish/result) was only released by agent restart. `WorkOutcome::Failure`
quarantines without recording a releasable state; `QuarantineState` had one
undifferentiated map. Additionally, a `Ready` operation whose VM became
quarantined between scan and claim was pushed back with its scheduled-execution
permit consumed — a permanent permit leak (the scheduler wedges after
capacity is exhausted).

**Fix (executor):** `QuarantineState` now separates failure entries from
restart/inspect entries.

- `reconcile_failures` runs on every `scan_ready` and releases a failure
  entry only when the operation's journal status is **terminal**. The live
  release path is absence from the incomplete-operations snapshot plus a
  terminal point-lookup status (an op that left the incomplete set
  completed); the snapshot-terminal arm is defensive (the incomplete
  snapshot only carries `accepted`/`running` ops) and reviewer-verified as
  such.
- Conservative keeps: snapshot status `Ready` (the claim never took — the
  failure marker would be lying about a claim) and absent+`Running` (an
  unmarked best-effort failure — restart classification owns it).
- VM-only entries (quarantine recorded against a VM whose op id was not
  retained) move to a `sticky` set that resolution never releases.
- A `Ready` operation skipped for a quarantined VM is re-queued without
  consuming a permit.

**Tests:** `reconcile_failures_releases_only_resolved_entries` (matrix over
snapshot dispositions + point lookups) and
`resolution_releases_failure_quarantine_and_readmits_dropped_work`
(end-to-end: failure → quarantine → operator resolution → terminal status →
quarantine released → successor operation admitted and executed, via a
`FlippingResult` runtime that fails the first effect and succeeds the
second).

### 3.2 (C-MAJOR) LifecycleService had no mTLS peer-identity interceptor

The control plane's node-facing gRPC surface wrapped four of five services
with `PeerIdentityInterceptor` (enrollment, inventory, telemetry, reconcile) —
but `LifecycleServiceServer` was constructed plain. In TLS mode, lifecycle
RPCs were accepted from any connected peer **without a parseable client
certificate**: the mutation surface was the one node-facing service an
anonymous socket could call.

**Fix:** `LifecycleServiceServer::with_interceptor(make_intercept())` in
`container.rs`, identical to its siblings. `CHV_ALLOW_INSECURE=1` keeps the
documented dev/test no-op; production (no insecure flag) now rejects
cert-less lifecycle calls. Scope note (reviewer-verified): the lifecycle
shim does not additionally pin `meta.target_node_id` to the caller's
certificate identity (unlike enrollment/inventory/telemetry/reconcile, whose
requests carry the caller's own node id) — the interceptor enforces "an
enrolled client certificate is required", the operator-tool model for this
surface. Agents present their enrolled cert on this surface by construction
(the node client pool connects with the enrolled identity), so no legitimate
caller is broken.

### 3.3 (D-MAJOR) `TimeoutStopSec=5` defeated the 60s executor drain budget

SIGTERM starts a bounded graceful drain (60s budget) for in-flight journal
operations; systemd's default 5s stop timeout then SIGKILLed the agent
mid-drain. Every `systemctl restart chv-agent` was therefore an
operator-triggered restart-interruption generator — the exact InspectRequired
path the campaign spends its fault-injection budget proving safe.

**Fix:** `TimeoutStopSec=75` in `packaging/systemd/chv-agent.service` and the
generated unit in `scripts/install.sh`, with the rationale commented at both
sites. The other services keep 5s (they have no drain budget).

## 4. MINOR / INFO fixes

| # | Area | Finding → fix |
|---|------|---------------|
| A2 | executor supervision | fatality only observed between long-parked operations → 500ms watchdog arm in the agent's core-managed select loop; redundant body check removed |
| A3 | executor reaper | task-loss panicked the reaper (killing the executor) → `Option` + in-place join fallback |
| B1 | store | incomplete-operations scan is a full-table shape → migration `0005_operations_recovery_index` (`status, vm_id`) |
| B2 | store/API | `list_operations` unbounded → newest-N ascending with service cap `MAX_LISTED_OPERATIONS = 1000` |
| B3 | store | bootstrap not crash-atomic (scrap file bricked startup with `AlreadyExists`) → single-transaction `apply_migrations` + **staged publish** (`create_new_staged`: bootstrap in a private sibling, `rename_noreplace` into place — the R2 review caught that the first version's in-place migration-target write still bricked the node: `activate` routes existing files to `open_existing`, so the scrap-replacement arm was unreachable; the staged publish makes the final path never a scrap and the pristine-host-less state recoverable) |
| B5 | agent | TLS cert/key/CA writes truncate-in-place → temp+fsync+rename+dir-fsync, 0600 keys (both enroll and rotate paths). R1 review caught that the first version omitted `.write(true)` (OpenOptions defaults to read-only: create/truncate without write access fails with InvalidInput before any file is created — every enrollment/rotation write silently failed, a regression from the previously working plain writes for cert+CA). Fixed and pinned by a test that drives the real function (content, 0600 key mode, atomic overwrite, no leaked temp siblings) |
| C-min | CP + agent | path-traversal cluster: `image_ref` joins under `/var/lib/chv/{kernels,images}` now require a single safe path component (`is_safe_path_component`: rejects separators, `.`/`..`, control characters — deliberately permits printable names like `image:latest` that predate the check); `volume_id` rejected unless a single safe component at `VmSpec::validate` and the legacy reconcile boundary (stord allowlist is empty by default); the CP resolve relay validates `meta.target_node_id` as a safe component before socket-pattern substitution |
| D2 | CP | `ResolveInspectRequiredOperation` rejected with `Unsupported` → relays to the owning agent via `NodeClientPool` (agent remains the journal authority; CP fails closed without egress; validation before egress) |
| D3 | executor | abandonment invisible until the next scan diff → in-process failures pushed to a bounded event ring (`MAX_FAILURE_EVENTS = 256`, `vm_id` added to `ExecutionFailure`); the runtime-owner poller drains it every poll and logs vm/op-id/code at error level (the architecture guard forbids a logging facade in the core crate — the first attempt added `tracing` and was caught by the CI guard, redesigned to the ring) |
| D4 | agent metrics | no journal-health/stuck-op metrics → `chv_agent_journal_{scan_failures_total,healthy,inspect_required}` on `:9901`, core-managed mode only (absent, not zeroed, in legacy; core-native serves no metrics endpoint — see residuals) |
| D6 | agent | missing cache parent silently degraded every save to a warn → parent created at startup |
| INFO | config | jwt_secret shared file created 0600 from creation (no umask exposure window) |
| INFO | CP store | controlplane.db tightened to owner-only after connect; pre-migration backup checkpoints the WAL (`wal_checkpoint(TRUNCATE)`) before the file copy |
| INFO | BFF/stub | storage-pool allocatable bytes `saturating_sub` (no wrapped negative on drift) |
| INFO | runtime-ch | genisoimage `/usr/bin` fallback when PATH lookup is NotFound |
| INFO | agent-core | per-tick network reconciliation logs demoted to debug |
| E | tests | 6 superstition sleeps before bound-UDS connects removed (listener is bound before the server task spawns — connect succeeds via the kernel backlog); supervisor tests moved off shared `/tmp/chv-*` paths to per-test tempdirs with deadline-bounded exit polls; new composition test pins concurrent byte-identical creates → exactly one `accepted` + one `replay`, VM listed once |
| E | specs | six stale status headers corrected (authority-actor, native-listener, runtime-authority-lease, startup-authority, operation-model, journal-executor pending list); plan.md §1 marked as a historical snapshot; native-api-v1 documents the request_id-is-operation-identity contract; OPERATIONS.md documents the CP relay and the journal metric families |

## 5. Tests

- **New:** 2 executor (quarantine matrix, resolution release + readmit),
  3 store (scrap replacement, never-clobber, bounded listing), 2
  control-plane relay (fail-closed, validation-before-egress), 1 agent
  composition (concurrent identical creates), plus the review-round pins:
  1 store (foreign views-only database is never scrap), 1 interceptor
  (cert-less request → UNAUTHENTICATED), 2 `is_safe_path_component`
  (tag-style names accepted, traversal vectors rejected), 1 agent
  durability (write_file_durable publishes, 0600 keys, atomic overwrite,
  no temp leaks), 3 startup recovery (interrupted migration target with
  cache re-imports; host-less authority without cache fails closed;
  stale staging sibling does not block), 1 socket-pattern gate
  (resolve_agent_socket rejects traversal ids, accepts printable ones,
  non-substituting patterns unaffected). **+17** tests vs
  baseline.
- **Full workspace:** `cargo test --workspace --no-fail-fast` → **1230
  passed / 20 failed**; the 20 are the known env-only set, byte-identical to
  clean main (chv-agent bin ×1, chv-agent-core cache ×6, chv-agent-runtime-ch
  ×13 — all the owner-writable-parent permission checks this host's `/tmp`
  fails; enumerated in the M2.4 evidence §8 and unchanged here).
- `cargo fmt --all` clean; `cargo clippy --workspace --all-targets -- -D
  warnings` clean.

## 6. Documented residuals (deferred, feature-sized)

1. **Journal retention/compaction + core.db backup story** — operations grow
   unboundedly; no node-local backup equivalent to the CP pre-migration
   backup. (R1 residual, restated.)
2. **Agent gRPC auth layer** — the agent's node-local socket relies on
   filesystem permissions (0700 runtime dir). (R1 residual.)
3. **Full BFF/WebUI/chvctl InspectRequired surfacing** — the CP relay landed
   here is the first actionable step; UI surfaces remain future work.
4. **`disk_seed_path` node-side allowlist** — absolute and `file://` seed
   paths pass through the CP verbatim by design (operator escape hatch); the
   node-side stord allowlist constrains what the agent opens, but the agent
   does not independently validate the seed source. Deployments must set the
   stord path allowlist to make this a hard boundary.
5. **VM runtime dir 0o775** — required for stord group access; accepted
   design constraint (R1 residual).
6. **Core-native metrics endpoint** — `run_core_native` serves the native
   Core API only; the `:9901` metrics server (and therefore the journal
   families) exists in the legacy/core-managed startup path only.
7. **End-to-end relay + interceptor coverage** — the relay is tested for
   fail-closed, validation-before-egress, and reach-failure semantics, but
   no test drives a live agent socket through the CP relay, and no test
   exercises the intercepted CP lifecycle server over the wire without a
   certificate (the interceptor itself is unit-tested and enforced on the
   enrollment surface by `peer_identity_enforcement.rs`).
8. **Pre-gate node-id rows** — the enrollment node-id gate and the
   `resolve_agent_socket` substitution gate protect every path going
   forward (the substitution gate is the last line for any caller), but
   node rows enrolled before the gate with traversal-shaped ids would
   now fail at socket resolution with a clear InvalidArgument instead of
   dialing — an operator-visible behavior change for data that was never
   functional. No migration backfills validation of existing rows.
9. **`is_bootstrap_scrap` read-only probe on a hot WAL** — the probe opens
   the file READ-ONLY; a scrap carrying a `-wal` sidecar with an
   unrecoverable `-shm` index can fail the probe open and be classified
   as NOT scrap (fail-closed `AlreadyExists`, manual removal required).
   With the staged publish the scrap arm is belt-and-suspenders only
   (the production path never creates a scrap at the final path), so
   this limitation is accepted.

## 7. Verified non-findings (reviewer claims checked against code)

- **"Metrics-bind failure is silent"** — the bind failure logs at
  `error!` level with the bind address and the agent continues (correct for
  an auxiliary endpoint).
- **"CoreNative mode uses the dev-default jwt secret"** —
  `materialize_agent_jwt_secret` skips resolution in CoreNative mode
  because the mode returns into `run_core_native` before `ConsoleServer`
  (the only jwt consumer) is constructed; the code comment documents this
  and the dispatch order confirms it.
- **"OPERATIONS.md references phantom metrics"** — every `chv_agent_*` name
  in OPERATIONS.md is emitted by `metrics_server.rs` (the three journal
  families were the gap and are fixed in this sweep; re-verified post-fix:
  documented ⊆ emitted).

## 8. Review loop and CI

Adversarial review loop on PR #272, three parallel lenses over the five
thematic commits (plus, from R2 on, the fix commits the loop itself
produced):

- **Lens 1 — core commit** (executor/operations/store/runtime-owner):
  **0 MAJOR**, 2 MINOR, several INFO. Both MINORs fixed in the R1-fix
  commit: bootstrap scrap detection now fails closed on ANY user schema
  object (a foreign views-only database is never deleted as scrap — pinned
  by a new store test), and the quarantine unit test now pins the release
  side (not only the conservative keeps). INFO fixes: the snapshot-Terminal
  release arm documented as defensive (reviewer-verified unreachable from
  the real snapshot source — the live release path is absent + terminal
  point-lookup), the bounded listing gained its serving index in migration
  0005, and the failure-event log wording no longer overstates the
  best-effort abandonment marking. Reviewer-verified non-findings include:
  no release of non-terminal operations (no TOCTOU — the failure-map lock
  is never held across an await), no VM un-quarantine with an unresolved
  stuck op, permit accounting correct on all four paths, bootstrap
  atomicity confirmed by an actual crash experiment, reaper fallback sound.
- **Lens 3 — controlplane/packaging/docs commits**: **1 doc MAJOR** (the
  OPERATIONS.md relay example used `grpcurl -insecure` with no client
  certificate — exactly what the new interceptor rejects; fixed with the
  mTLS invocation and stated requirement), one MAJOR-variant (operator-
  supplied `meta.target_node_id` flows into the agent-socket pattern
  substitution; now required to be a single safe path component — default
  deployments substitute nothing and were never exposed), and MINORs all
  fixed: relay error mapping (`NodeUnavailable` → gRPC UNAVAILABLE for
  reach failures, clear `InvalidArgument` for payload shape, requested_by
  defaulting), `with_operation_id_metadata` parity, WAL-checkpoint busy-row
  warning, `-wal`/`-shm` sidecar permission tightening, image_ref
  compatibility (`is_safe_path_component` keeps `image:latest`-style names
  valid while rejecting traversal vectors), journal-metric scope corrected
  to core-managed-only. Reviewer-verified non-findings: no legitimate
  caller breaks on the interceptor (the only in-tree gRPC client of the CP
  never calls lifecycle; BFF mutations are in-process), bootstrap pool
  ordering sound, relay tests genuinely pin the behavior, systemd placement
  correct, saturating_sub correct.
- **Lens 2 — agent commit** (main.rs, chv-agent-core, runtime-ch, config):
  **1 MAJOR** — `write_file_durable` omitted `.write(true)`: OpenOptions
  defaults to read-only access, so create/truncate failed with InvalidInput
  before any file was created and every enrollment/rotation write silently
  failed (the node would have re-enrolled with a fresh node_id on every
  boot; cert rotation would never persist). The reviewer verified the
  failure empirically against the workspace's exact tokio version and
  traced the blast radius. Fixed (`.write(true)`), plus the two MINOR
  edge semantics: a failed rename now cleans up its temp sibling, and a
  post-rename directory-fsync failure is a loud warning instead of an
  error (the rename already committed — reporting the material unwritten
  while it exists on disk would be worse). Pinned by a new test driving
  the real function. Reviewer-verified non-findings: temp-name collisions
  impossible (distinct target names embed in the temp name; enrollment and
  rotation are sequential in one process), the 500 ms fatality watchdog
  cannot starve the 5 s interval (tokio select/Interval interaction
  traced), the concurrent-creates composition test is deterministic
  (single-threaded authority actor + Immediate-transaction idempotency
  re-check), bind-before-spawn holds at every sleep-removal site,
  `write_secret_private` (jwt) already had `.write(true)` correct,
  volume-id validation matches the stord locator requirement with no
  broken fixtures, and the observed test failures in the review
  environment are the known pre-existing environmental set.
- An additional direct pin landed from the review: `intercept()` rejects a
  cert-less request with UNAUTHENTICATED (the behavior the lifecycle
  interceptor fix relies on).

**R2 verification round** (after the R1-fix commits, two lenses — a
fix-verification pass over the fix commits themselves and a convergence
sweep over the full branch diff):

- **Convergence lens found 1 MAJOR** the R1 lenses missed: the
  interrupted-bootstrap scrap self-healing was **unreachable in the
  production startup path** — `activate` routes every existing-file boot
  to `open_existing`, while the scrap-replacement arm lived in
  `create_new` (only invoked when the file does not exist), and the
  migration cutover's `create_migration_target` wrote the schema
  directly at the final path. A power loss mid-bootstrap still bricked
  the node exactly as before the fix; the branch's own test pinned the
  store layer and could not see it. Fixed systemically: the migration
  target now bootstraps in a staging sibling and publishes with
  `rename_noreplace` (final path never a scrap; crash leaves only a
  staging sibling that cannot block the next attempt); the adjacent
  pre-existing window (crash between publish and import → pristine
  host-less authority shadowed by `service.host()?` before the re-import
  arm) is closed with a cache-gated `host_optional` tolerance — without
  a cache a host-less authority still fails closed. Three new tests
  drive the full `StartupTransaction` recovery (re-import, fail-closed,
  stale-staging tolerance).
- **Socket-pattern injection closed systemically**: the convergence and
  fix-verification lenses both noted sibling substitution sites
  (orchestrator dispatch, migration ×6, reaper, overlay) using
  DB-sourced node ids with no gate. `resolve_agent_socket` itself now
  rejects traversal-shaped ids when the pattern substitutes
  `{node_id}` (single point, all callers), with request-boundary gates
  at `parse_node_id` (all 35 lifecycle RPC entry points), `migrate_vm`
  source/destination, and enrollment (`inventory.node_id` — where
  agent-supplied node data enters). Pinned by a direct unit test.
- Fix-verification lens: both R1-fix commits verified correct (0 MAJOR);
  MINORs fixed — the relay now pre-checks the agent's 8000-byte note
  bound, and the relay node-id gate uses `is_safe_path_component`
  (node ids are agent-config-supplied at enrollment, not
  enrollment-generated — the stricter `is_safe_id` would have rejected
  legitimately enrolled ids) via the shared `parse_node_id` boundary.
- `/v1/operations` newest-1000 bound documented in the Core native API
  spec (matching the events row).
- Verified non-findings from the round: `is_safe_path_component` has no
  Linux traversal bypass and breaks no repo fixture; the relay's
  pre-validations are strictly permissive versus the agent's;
  `NodeUnavailable` breaks no exhaustive match or From impl; the
  migration-0005 listing index serves the query via backward index scan
  and no deployed DB can hold the old 0005; `write_file_durable` is
  fully correct with the fix (empirically re-confirmed); the WAL
  sidecar tightening is start-of-run best-effort and SQLite cannot
  widen the files mid-run; the failure ring, task_vms map, quarantine
  release ordering, watchdog, reaper, interceptor placement, metrics
  families, and systemd placement all re-verified.

- **R3 verification round** (fix-verification lens over the R2-fix commit
  `b8bca954` + the R3 pin): **clean — 0 MAJOR, 0 MINOR**, three
  non-blocking INFOs. The staged publish was traced step-by-step against
  the fresh-authority path (no step missed or reordered; `published`-flag
  scoping identical; `remove_database_files` can never touch the final
  path by name construction); every (cache, marker, pristine) combination
  walks to the intended outcome (corrupt/unrelated authorities still fail
  closed; only the genuine pristine-with-cache state is adopted, and its
  content comes entirely from the trusted owner-validated cache import);
  no `resolve_agent_socket` caller was missed and the best-effort sites
  preserve their surrounding semantics; the full chv-controlplane-service
  suite confirmed no enrollment/RPC fixture regression from the boundary
  gates. The loop converged: R1 found MAJORs (fixed), R2 found one more
  MAJOR plus MINORs (fixed), R3 found nothing actionable. One INFO pin
  landed (same-pid stale-staging tolerance); the two remaining INFOs are
  recorded here as accepted (no direct store-level error-path test for
  `create_new_staged` — no injection point without hooks, real states
  covered by the startup tests; commit-message RPC count nit).

CI on the merged SHA `62db8bf6`: **CI workflow green** (Rust checks, E2E
tests, UI checks, Build and Package all pass), **Security green**. The
separate **Nightly Packages** workflow fails on `62db8bf6` — and
identically on every prior main SHA back through August (the release
binaries require `GLIBC_2.38` while the oldest Debian smoke target ships
2.36): a pre-existing toolchain/runner drift in the nightly packaging
job, orthogonal to and untouched by this sweep, reported to the operator.

> **Follow-up (same day, post-sweep): fixed in PR #273 (`d8e8e2e0`).**
> The root cause was not live toolchain drift but a fix that was never
> applied here: `package-pr.yml` has been pinned to `ubuntu-22.04`
> (glibc 2.35) since August, while the nightly and release build jobs
> stayed on `ubuntu-latest` (24.04, glibc 2.39) — the Nightly Packages
> workflow had never had a single green run. PR #273 pins both build
> jobs to `ubuntu-22.04` (+ `arduino/setup-protoc`, since apt protoc on
> 22.04 is too old for the workspace's proto3 optional fields), and its
> `workflow_dispatch` dry-run verification exposed and fixed two further
> latent release-pipeline defects no run had ever reached: artifact
> round-trips stripping the executable bit in `release.yml` (binaries
> packaged non-executable), and the shared package lifecycle test
> asserting dpkg conffile semantics on rpm (where `rpm -e` legitimately
> moves a modified `%config(noreplace)` file to `.rpmsave`). CI,
> Security, and Nightly Packages are all green on `d8e8e2e0` — the
> nightly's first green push-triggered run in its history.

## 9. Remaining risks

- The quarantine redesign changes the executor's most safety-critical
  release path; it is matrix-tested and composition-tested but not yet
  KVM-exercised (M2.5).
- The lifecycle interceptor tightens the CP wire contract in TLS
  deployments; agents present enrolled certs by construction, but a
  deployment with a custom non-enrolled caller on that surface would now be
  rejected (fail-closed by design).
- The resolve relay adds a CP→agent egress path for journal mutation
  commands; it is a relay of an operator action, gated by the same mTLS
  interceptor as every other lifecycle call, but its audit story (who
  resolved what) currently lives in the node journal only.
