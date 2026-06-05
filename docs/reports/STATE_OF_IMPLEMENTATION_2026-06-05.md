# CHV Comprehensive State-of-Implementation Review

**Date**: 2026-06-05
**Version reviewed**: 0.2.0 (HEAD `b3f5e073`, clean working tree)
**Scope**: Whole-repository strategic assessment vs industry standards
**Mode**: Review-only (no fixes applied — clean diff)
**Agents dispatched**: 21 (Wave 1: 11 foundation + Wave 2: 10 deep-dive)
**Findings persisted**: `/tmp/claude-review/20260605-140811/`

---

## Verdict: **PRODUCTION-CAPABLE FOUNDATION, NOT YET PRODUCTION-DEFAULT**

CHV is a **strategically well-architected v0.2 platform** with discipline that exceeds most pre-1.0 Rust services and rivals KubeVirt/Proxmox at strategic-architectural choices. The codebase respects its own ADRs, the contract surface is proto-first, the release pipeline is genuinely above-average for v0.2 (signed checksums, dual SBOM, build-provenance), and the crate boundaries hold up under inspection.

The gap between **what CHV is today** and **what it should ship to a customer** is **not architectural** — it is concentrated in three workable categories:

1. **Default deployment posture is broken.** Out-of-the-box install boots with `admin/admin`, writes plaintext `http://` for the agent → control-plane channel, and never wires mTLS server certs (despite issuing them). A fresh CHV install fails OWASP A02 + A07 at first boot.
2. **Three production-blocking correctness gaps in the data plane.** Memory-migration cannot rollback, disk-migration's `PausedFinalSync` is wired in stord but rollback paths are untested, and quota enforcement has a check-then-insert TOCTOU under SQLite's deferred isolation.
3. **Hygiene gaps will become breaking changes at v1.0.** No proto `reserved` discipline, no SQLite down-migrations, no SLI/SLO docs, no SECURITY.md, no supply-chain scanning in CI, no distributed tracing.

None of these require architectural redesign. All are tractable in 2–4 sprints of focused work.

---

## Industry-Standards Alignment Scorecard

| Dimension | Grade | Rationale |
|---|---|---|
| Architecture & Boundaries | **A−** | Proto-first contracts, generated code isolated, ADR boundaries verified at Cargo-edge level. KubeVirt-comparable. |
| ADR Governance | **A−** | 18 ADRs, gaps tracked in PHASED_PLAN not hidden, proto matches ADR text verbatim, ADRs drive design. |
| Rust Idioms | **B+** | Modern dep matrix (tonic 0.12, axum 0.7, sqlx 0.8), 0 println! in libs, structured thiserror, no anyhow leakage. 1 ADR-010 violation, 5 `Box<dyn Error>` in main(). |
| Code Quality (CLAUDE.md) | **B−** | 100% workspace consistency, 0 production unwrap, CI fmt+clippy gated. **29 Svelte files violate <300 LOC rule.** Some Rust files are 2000–3000 LOC. |
| Error Handling (ADR-008) | **B+** | Excellent ChvError taxonomy, exemplary gRPC status mapping, internal-detail sanitization. **HIGH:** 28 of 33 Status::internal in stord migration flatten retryable-vs-fatal. |
| Async Safety (ADR-010) | **A−** | Correct spawn_blocking, clean tokio::select! shutdown, no block_on, no thread::spawn. 1 cosmetic violation in test mock. |
| Logging (ADR-009) | **A** | 847 tracing calls, 0 println in libs, JSON output, secret redaction visible. |
| Observability | **C+** | Prometheus wired, domain metrics good, **VM lifecycle has zero metrics**, **zero distributed tracing**, no SLI/SLO docs. |
| Security (OWASP / NIST 800-190 / CIS) | **C** (NEEDS_WORK) | mTLS plumbing, parameterized SQL, AES-GCM, bcrypt cost-12 — but **default install fails A02 + A07**. |
| Test Coverage | **C** (GAPS_FOUND) | 130+ unit tests, strong CP store/service coverage, **agent reconcile (2341 LOC) untested, JWT/auth negative cases untested, all 4 storage backends untested**. 0 proptest, 0 fuzz, 0 criterion. |
| Type Design | **C+** | Strong newtypes in domain.rs, but **NodeState schism** between crates and **raw String IDs propagated downstream**. 0 proto-to-domain TryFrom. |
| API Contracts | **C+** | /v1/ versioned, structured errors at HTTP layer. **AIP-158 violation (offset pagination), no rich error model, no `reserved` markers, stringly-typed enums in proto.** |
| Performance | **B−** | BFF list shape excellent, indexes solid. **Reconciler serial per VM, orchestrator N+1 dispatch**, std::sync::Mutex contention, no migration buffer pool. |
| Concurrency | **B+** | No confirmed races. Quota TOCTOU + 4 fire-and-forget tasks + console.log race window. |
| Migration Safety | **C** | SQL forward-additive but **0 down migrations, memory-migration uncancellable, no proto reserved discipline**. |
| Dependency / Supply Chain | **C−** | **vitest CRITICAL CVE, rsa Marvin via accidental sqlx-mysql feature**, no cargo-audit/deny in CI, no dependabot, no SECURITY.md. SLSA L1, Scorecard ~3.5/10. |
| Project Hygiene | **B−** | 13 ADRs + 6 component specs + 12 release docs is unusual rigor. **Repo root pollution (8 stale review reports), duplicate DEPLOYMENT.md, no SECURITY.md, no CODEOWNERS**. |
| Release Engineering | **A−** | Signed checksums (GPG+cosign), dual SBOM (SPDX+CycloneDX), build-provenance attestations, lifecycle tests, channel-aware. SLSA L2-adjacent. |
| Documentation (inline) | **D+** | 0 of 13 crates have //! docs, ~25% public-API /// coverage, **0 # Errors / # Panics / # Examples sections, 0 // SAFETY: comments on 10 unsafe blocks**. |

**Overall**: **B / B−** — The structural foundation is **A**-grade; the operational and security defaults are what drag the average down. A focused 2–4 sprint cleanup pushes this to **A−** territory.

---

## Findings Severity Matrix

| Wave / Domain | CRIT | HIGH | MED | LOW |
|---|---:|---:|---:|---:|
| **Wave 1: Foundation** | | | | |
| Security | 3 | 6 | 12 | 0 |
| Business Logic | 4 | 4 | 6 | 0 |
| Architecture | 0 | 3 | 6 | 0 |
| Silent Failures | 0 | 1 | 6 | 3 |
| Test Coverage | 5 | 3 | 4 | 0 |
| Type Design | 0 | 6 | 4 | 0 |
| Code Quality | 1 | 4 | 3 | 0 |
| Comments | 0 | 2 | 3 | 1 |
| Language Idioms | 0 | 1 | 5 | 5 |
| Project Health | 2 | 4 | 4 | 2 |
| ADR Compliance | 0 | 1 | 1 | 1 |
| **Wave 1 subtotal** | **15** | **35** | **54** | **12** |
| **Wave 2: Deep-Dive** | | | | |
| Performance | 3 | 4 | 3 | 2 |
| Concurrency | 0 | 3 | 6 | 0 |
| API Contracts | 0 | 5 | 4 | 4 |
| Dependency Audit | 1 | 2 | 6 | 0 |
| Error Messages | 0 | 4 | 3 | 1 |
| Dead Code | 0 | 0 | 4 | 4 |
| Naming | 0 | 0 | 3 | 1 |
| Observability | 3 | 5 | 1 | 0 |
| Config Safety | 2 | 3 | 3 | 2 |
| Migration Safety | 4 | 4 | 4 | 0 |
| **Wave 2 subtotal** | **13** | **30** | **37** | **14** |
| **TOTAL (raw)** | **28** | **65** | **91** | **26** |

After deduplication (e.g. mTLS install gap counted by Security + Config Safety; rsa CVE by Security + Dependency; quota TOCTOU by Business + Concurrency): **~22 CRITICAL, ~52 HIGH, ~80 MEDIUM, ~24 LOW**.

---

## CRITICAL Findings (must-fix for production GA)

### Security & Secrets

| # | Finding | Files | Industry Standard |
|---|---|---|---|
| C-1 | **Default `admin/admin` shipped in migrations 0008+0033, no `must_change_password` gate, installer logs in with it.** Globally identical for every CHV deployment. | `cmd/chv-controlplane/migrations/0008_users.sql`, `0033_activate_admin_account.sql`, `install.sh:484,546,1173` | NIST SP 800-53 IA-5(1); CIS hypervisor; OWASP A07 |
| C-2 | **install.sh writes `http://127.0.0.1:8443` for agent→CP, omits `server_cert_path`/`server_key_path` in controlplane.toml.** mTLS never enables on default install despite certs being issued. | `install.sh:765,740-757`, `crates/chv-agent-core/src/control_plane.rs:20`, `cmd/chv-controlplane/src/bootstrap.rs:260` | NIST 800-190 §4.5; CIS hypervisor; OWASP A02 |
| C-3 | **vitest <@vitest/mocker> arbitrary file read+exec.** Local-dev/CI surface, fix requires semver-major bump. | `ui/package-lock.json` (vitest 2.1.9 → 4.1.8) | OWASP A06 |
| C-4 | **`rsa 0.9.10` Marvin timing-side-channel (RUSTSEC-2023-0071)** pulled in via accidental `sqlx-mysql` feature enable. Root cause: `crates/chv-webui-bff/Cargo.toml:21` and `crates/chv-controlplane-store/Cargo.toml:21` redeclare sqlx without `default-features = false`. **Single-line fix kills the CVE plus ~30 transitive crates.** | `crates/chv-webui-bff/Cargo.toml`, `crates/chv-controlplane-store/Cargo.toml` | OWASP A06; SOC2 CC7.1 |

### Data-Plane Correctness

| # | Finding | Files |
|---|---|---|
| C-5 | **Memory-migration phase has no rollback path.** Code comment: "cannot rollback cleanly". Network blip mid-memory-migration leaves source paused, unrecoverable. CHV doesn't call Cloud Hypervisor `vm.send-migration` cancel because CH has no documented cancel endpoint. | `crates/chv-controlplane-service/src/migration.rs:530-560`, `crates/chv-agent-runtime-ch/src/process.rs:1329+` |
| C-6 | **Quota enforcement is check-then-insert under SQLite DEFERRED isolation.** Two concurrent CreateVm at quota boundary can both pass the SUM check. No reservation table. | `crates/chv-webui-bff/src/handlers/vms.rs:403`, `crates/chv-controlplane-store/src/desired_state.rs:444` |
| C-7 | **No fencing / STONITH for unreachable nodes.** ADR-006 documents agent autonomy + "deny new VM creation" during partition, but agent-side stale-generation rejection is NOT coded. With shared storage this is split-brain-by-design. `grep -rn "fence\|stonith"` returns 0 hits. | `crates/chv-controlplane-service/src/orchestrator.rs:413`, `crates/chv-agent-core/src/control_plane.rs` |

### Reconciliation & Performance

| # | Finding | Files |
|---|---|---|
| C-8 | **Agent reconciler VM ops are fully serial.** With 100 VMs/node and one slow `open_volume`, the entire tick blocks. Per-VM parallelism is the correct shape (kubelet's `podWorker` pool pattern). | `crates/chv-agent-core/src/reconcile.rs:1012,1096,1116,825,878` |
| C-9 | **Orchestrator N+1 in dispatch tick.** 15 ops × 1 SQL each for node_id resolution every 2s. 10–15× more DB calls than needed. Plus 4 unindexed `COUNT(*)` queries per tick. | `crates/chv-controlplane-service/src/orchestrator.rs:209-242,88-143,295-300` |
| C-10 | **`std::sync::Mutex` held in async hot paths.** Critical sections short, but blocks tokio worker on poison and competes with reconciler's per-VM cache lock (~21× per tick). | `crates/chv-agent-core/src/vm_runtime.rs:19-20`, `crates/chv-agent-runtime-ch/src/process.rs:71` |

### Observability

| # | Finding | Files |
|---|---|---|
| C-11 | **VM lifecycle on agent has ZERO metrics.** create/start/stop/delete have no counter, no histogram. Operator cannot answer "p99 VM start time" or "stop-failure rate per node". | `crates/chv-agent-runtime-ch/src/process.rs` |
| C-12 | **No distributed tracing.** Zero OpenTelemetry/OTLP, zero W3C `traceparent` propagation across BFF→CP→agent. `operation_id` is grep-only. For a system fanning across 5 services, this is a material gap. | `crates/chv-observability/src/lib.rs` |
| C-13 | **gRPC server-side latency by method: absent.** No tonic interceptor recording per-method duration/error. | repo-wide |

### Migration Safety

| # | Finding | Files |
|---|---|---|
| C-14 | **Zero `*.down.sql` rollback migrations across 43 forward migrations.** sqlx Migrator is up-only. Manual `.bak` restore is the only path. | `cmd/chv-controlplane/migrations/` |
| C-15 | **0031 destructive table-recreate uses `INSERT...SELECT *`** (column-order assumption — silent misalignment if intermediate migrations were skipped). | `cmd/chv-controlplane/migrations/0031_remove_enum_check_constraints.sql` |
| C-16 | **Proto contracts have ZERO `reserved` and ZERO `deprecated = true` markers.** First field deletion will silently corrupt peers during rolling upgrade. | `proto/**/*.proto` |

### Test Coverage

| # | Finding | Files |
|---|---|---|
| C-17 | **`crates/chv-agent-core/src/reconcile.rs` (2341 LOC) — 0 test fns.** Safety-critical reconcile loop. |
| C-18 | **`crates/chv-agent-core/src/migration.rs` (831 LOC) — 0 tests.** Per-host migration orchestration. |
| C-19 | **`crates/chv-stord-backends/{ceph,iscsi,local,lvm}.rs` — 0 tests across all 4 backends.** Data-path correctness. |
| C-20 | **`crates/chv-webui-bff/src/auth.rs` — 0 JWT negative-case tests.** No expired/tampered/wrong-aud/missing-claim tests. Security-sensitive. |
| C-21 | **`crates/chv-controlplane-store/src/credential_crypto.rs` — only roundtrip + plaintext-fallback tests.** No tampered-ciphertext, no wrong-key, no AAD tamper. S3 cred encryption-at-rest. |

### Code Quality

| # | Finding | Files |
|---|---|---|
| C-22 | **29 Svelte files violate the <300-line CLAUDE.md cap.** Worst offenders: `backup-jobs/+page.svelte` 577, `templates/+page.svelte` 542, `MobileNav.svelte` 480, `QuickActions.svelte` 477, `images/+page.svelte` 470, `CloudInitEditor.svelte` 441. | `ui/src/**` |

---

## HIGH Findings (must-fix before v1.0) — Top 30 of ~52

### Security & Auth
- **H-1** Console JWT lacks `vm_id`/`aud`/`iss` binding — token issued for VM A validates at any agent's `/vms/B/console` until LRU/expiry. (`crates/chv-agent-core/src/console_server.rs:39-45`, OWASP API3/API5)
- **H-2** API tokens get `exp = u64::MAX/2` synthesized into Claims — couples DB-managed tokens with JWT-claim semantics. (`crates/chv-webui-bff/src/auth.rs:196-199`)
- **H-3** JWT secret silent fallback: each service generates its own random secret if `/etc/chv/jwt_secret` is unwritable — agent/CP tokens become non-portable, only a `tracing::error!`.
- **H-4** Login rate-limit per-username, in-memory, evadable — no per-IP throttle, no normalization, no map-size cap. (`crates/chv-webui-bff/src/handlers/auth.rs:18-36`)
- **H-5** Console URL embeds JWT in query string (logged by reverse proxies). (`crates/chv-webui-bff/src/handlers/vms.rs:1066-1071`)
- **H-6** CA private key 0640 group-readable to `chv` group members. (`install.sh:626-637`)
- **H-7** `CredentialEncryption::new()` silent plaintext fallback when `CHV_ENCRYPTION_KEY` and `CHV_JWT_SECRET` both unset; install.sh never injects the key into systemd. S3 creds at rest are plaintext in default install. (`crates/chv-controlplane-store/src/credential_crypto.rs:64-75`)

### Domain & Data Plane
- **H-8** Node liveness threshold (60s) is shorter than reaper timeout (120s) — orphan operations against Unreachable nodes.
- **H-9** Migration validator doesn't check VM is `Running` — Pending/Stopped VMs admittable into precopy.
- **H-10** Upgrade rollback monotonicity not enforced — replaying old generations after rollback would silently apply.
- **H-11** Streaming gRPC receiver task fire-and-forget under client churn — task leak. (`crates/chv-stord-core/src/migration/service.rs:98`)
- **H-12** Console-log writer detached, races with `remove_file` — transient errors silently re-create file. (`crates/chv-agent-runtime-ch/src/process.rs:775,886,939`)
- **H-13** Disk migration leaves dest volume on rollback (no `DeleteVolume` call).

### API Contracts
- **H-14** Pagination violates AIP-158: offset pagination instead of cursor (`page_token`/`next_page_token`). Migration to cursor is breaking — must happen before any external SDK ships.
- **H-15** No `google.rpc.Status` rich error model — proto errors are bare `tonic::Status` code+text. Cross-language clients lose typed details.
- **H-16** Idempotency keyed on server-generated UUIDs, not client-supplied `Idempotency-Key` header. Retried HTTP request gets a *different* `vm_id`.
- **H-17** Stringly-typed enums in proto — `NodeListItem.state`, `RecentTask.status`, `FirewallRuleSpec.{direction,protocol,action}`, `desired_power_state`. Renames are silent breaks.

### Architecture & Type Design
- **H-18** `chv-controlplane-service` god-crate (~19,200 LOC, 33 files). Recommend split along orchestration concerns before it ossifies.
- **H-19** Layering inversion: `chv-controlplane-service` depends on `chv-webui-bff`. `bff_mutations.rs` lives inside the service crate.
- **H-20** `NodeState` schism: 11 variants in `chv-controlplane-types` vs 10 in `chv-agent-core` (no `Unreachable` on agent side).
- **H-21** Raw `String` IDs propagated across `chv-agent-core/spec.rs`, `chv-agent-core/migration.rs`, `chv-stord-core/migration/*`, `chv-webui-bff/mutations.rs` — newtypes from `chv-controlplane-types` not propagated downstream.
- **H-22** Zero `TryFrom<proto::X> for X` impls — proto types reach business logic raw.

### Errors & Observability
- **H-23** Correlation ID collected but never embedded in error response body. Operators must grep logs by timestamp.
- **H-24** `BffError::Internal` is a black hole: 242 callsites all return literal "Internal server error". Inner detail logged but not in response.
- **H-25** 28 of 33 `Status::internal` callsites in `chv-stord-core/migration/{sender,receiver}.rs` flatten retryable-vs-fatal. Should be `Aborted`/`DataLoss`/`DeadlineExceeded`/`Cancelled`.
- **H-26** Information-leak risk: `format!("db error: {}", e)` and serde::Error wrapped raw into `Internal`. One bug-fix away from leaking schema.
- **H-27** Reconcile loop has tick counter but no error counter. Stuck reconciler keeps ticking.
- **H-28** SQLite query latency: zero metrics. No `db_query_duration_seconds`, no error counter — silent ALTER swallow has no alert path.
- **H-29** No SLI/SLO docs, no Prometheus rule files, no Grafana dashboards in repo. The `/metrics` surface is decorative without thresholds.

### Project Hygiene
- **H-30** No `SECURITY.md`, no `CODEOWNERS`, no `dependabot.yml`, no CodeQL, no OSSF Scorecard, no `cargo audit`/`cargo deny` in CI, no npm audit. SLSA L1, Scorecard ~3.5/10.

---

## What's Done Well (genuine strengths)

- **Boundary discipline.** Verified at the Cargo dependency edge: control plane physically cannot dial stord/nwd. UI cannot import `gen/rust/control-plane-node-api`. ADR-002 holds at the type level.
- **Proto-first contracts with isolated `gen/rust`.** Build-script driven, no hand-edits. Many Rust gRPC projects get this wrong.
- **Operation journal with `ON CONFLICT (idempotency_key) DO NOTHING`** + atomic claim via `UPDATE...RETURNING` — exactly-once semantics at the storage layer, eliminating double-dispatch on tick overlap.
- **`chv-errors` taxonomy.** 14 typed variants, `From<ChvError> for tonic::Status` covers 9 specific codes, internal-detail sanitization at the gRPC boundary.
- **`MigrationReaper`** runs every 60s with phase-aware timeouts (MigrateVm 7200s, snapshots 600s, StartVm 300s) — better than libvirt's "stuck forever" default.
- **State machines are explicit.** Node FSM and Migration FSM use enums with `valid_transition` guards — KubeVirt/Nova pattern done right.
- **Release pipeline.** Signed checksums (GPG + cosign), dual-format SBOM, build-provenance attestations, channel-aware (stable/RC), gated environments, package lifecycle tests for upgrade/downgrade. SLSA L2-adjacent for v0.2 — well above peer projects.
- **CI rigor.** `cargo fmt --all -- --check` and `cargo clippy --workspace -- -D warnings` are gating; TS strict, svelte-check 0 errors / 0 warnings.
- **No `println!` in libs, no production `.unwrap()`, no `panic!`/`todo!`/`unimplemented!()` in lib code.** ADR adherence visible in code.
- **Test determinism.** Zero `sleep` in test files (37 in `src/`, 0 in tests).
- **Operation-sensitive backoff** in agent reconcile, orchestrator, backup_worker. ADR-008 §5 spirit honored.
- **Documentation depth for a v0.2.** 18 ADRs, 6 component specs, 12 release-engineering docs, 5 industrial-grade DR runbooks — unusual rigor.

---

## Three Systemic Patterns

### Pattern 1 — "Built right, wired wrong"

The single most common shape across findings: **code is implemented correctly, but not connected to the production path.**
- mTLS code paths are right; install.sh doesn't enable them.
- AES-GCM credential encryption is right; systemd doesn't inject the key.
- ChvError taxonomy is right; BffError::Internal flattens it back to "Internal server error" at the HTTP edge.
- `Idempotency` key column exists; the BFF generates it from the server-side UUID instead of accepting a client header.
- `operation_id` is everywhere in logs; no error response carries it.
- Newtypes like `VmId` exist; `agent-core/spec.rs` uses raw `String`.
- Generation-based optimistic concurrency works; HTTP API uses body field instead of `If-Match`/`ETag`.

**Action**: A 1-sprint "wire it to production" pass closes this category: install.sh emits HTTPS+TLS-paths, systemd injects encryption key, BffError::Internal carries `request_id`, BFF accepts `Idempotency-Key` header. None require new code.

### Pattern 2 — "Foundation strong, propagation weak"

Foundation crates (`chv-errors`, `chv-controlplane-types::domain`) embody the right patterns. Downstream crates (`chv-agent-core`, `chv-stord-core`, `chv-webui-bff/mutations.rs`) drop those patterns and revert to raw types.
- Strong newtypes in domain.rs → raw `String` in agent migration.
- Closed enums in domain.rs → stringly-typed enums in fragments and proto.
- Structured `ChvError::QuotaExceeded { resource, limit, used, requested }` → BFF re-flattens to `BadRequest("missing X")`.

**Action**: Add `TryFrom<proto::X> for DomainX` at every service entry point. Make domain newtypes the only types crossing module boundaries.

### Pattern 3 — "Pre-1.0 OK, v1.0 not"

Several findings are perfectly defensible at v0.2 but become breaking changes at v1.0:
- Offset pagination (HIGH-14)
- Stringly-typed proto enums (HIGH-17)
- No `reserved` markers (CRIT-16)
- No down migrations (CRIT-14)
- Default admin/admin (CRIT-1) — already breaking

**Action**: Treat v1.0 as a freeze point. Author an "API Evolution" ADR (proto-evolution policy: never reuse field numbers, always `reserved`, additive within major, `[deprecated]` one minor before removal). Add CI lint that diffs proto vs main for reused field numbers.

---

## Recommended 4-Sprint Sequencing

**Sprint 1 (Security defaults — 1 week)**
- Fix install.sh: HTTPS, TLS paths, `https://` agent URL, generate random admin password, drop `admin/admin` migration. Closes C-1, C-2, H-6, H-7.
- Fix sqlx feature leak: `default-features = false` in BFF + store. Closes C-4 + ~30 transitive crates.
- Add SECURITY.md, dependabot.yml, cargo-audit/cargo-deny CI gates. Closes H-30 partially.
- `npm audit fix --force` + vitest 4.x bump. Closes C-3.

**Sprint 2 (Data-plane correctness — 2 weeks)**
- Memory-migration cancel path (best-effort `vm.pause` dest + `vm.resume` source) + runbook. Closes C-5.
- Quota: `BEGIN IMMEDIATE` for quota-gated paths or single `INSERT ... WHERE (SELECT count) < limit`. Closes C-6.
- Agent reconciler `buffer_unordered(8)` over per-VM operations. Closes C-8.
- Orchestrator: fold `node_id` into claim `UPDATE...RETURNING`; in-process gauge cache. Closes C-9.
- Replace `Arc<std::sync::Mutex<HashMap>>` with `DashMap` in vm_runtime + process. Closes C-10.

**Sprint 3 (Observability + contracts — 2 weeks)**
- Agent VM lifecycle RED metrics + tonic server-side interceptor. Closes C-11, C-13.
- OpenTelemetry OTLP exporter + W3C `traceparent` interceptors. Closes C-12.
- Author `docs/OBSERVABILITY.md` with SLI/SLO + Prometheus rule files. Closes H-29.
- Embed `request_id` in every BFF error body; `BffError::Internal` carries `Option<String>`. Closes H-23, H-24.
- Add `reserved` discipline to proto + `[deprecated = true]` on earmarked fields + API-evolution ADR. Closes C-16.

**Sprint 4 (Tests + cleanup — 2 weeks)**
- Unit suite for `chv-agent-core::{reconcile, migration, state_machine}` (target ~30 tests). Closes C-17, C-18.
- JWT negative-case suite in `chv-webui-bff::auth`. Closes C-20.
- Tamper/wrong-key tests in `credential_crypto`. Closes C-21.
- Storage backend tests (mockable iSCSI/Ceph). Closes C-19.
- Repo-root cleanup (delete 8 stale review docs). Closes Wave 2 dead-code findings.
- Decompose 29 Svelte components below 300 LOC. Closes C-22.

After these 4 sprints, CHV's industry-standards alignment moves from **B/B−** to **A−**, suitable for first-customer GA.

---

## Findings Persisted

```
/tmp/claude-review/20260605-140811/
  wave1-findings.md     — 132 lines, 11 foundation agents
  wave2-findings.md     — 150 lines, 10 deep-dive agents
```

Files persist in `/tmp/` until next reboot.

---

*This is a strategic state assessment. No fixes were applied (working tree was clean). The next user action — if alignment is the goal — is the 4-sprint sequencing above.*
