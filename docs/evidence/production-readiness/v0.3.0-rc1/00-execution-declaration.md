# CHV Production-Readiness — Prompt 00 Execution Declaration

> Campaign: [production-readiness](/docs/prompts/production-readiness/README.md)
> Capability maturity vocabulary: CODED / CI-VERIFIED / KVM-VERIFIED / MULTI-HOST-VERIFIED / RELEASED / FIELD-QUALIFIED.
> Evidence root for this candidate: `docs/evidence/production-readiness/v0.3.0-rc1/`

Provisional candidate line: **`v0.3.0-rc.1`** (next RC after the supported `0.2.x` line; finalized in Prompt 05 using the versioning policy).

---

## 1. Frozen evidence frame (baseline `020e2b22`)

| Item | Value | Source |
|---|---|---|
| Baseline `main` SHA | `020e2b22a523b6e7e697a48bc2c020088bbf90e4` | `git log` |
| `VERSION` | `0.2.0` | `VERSION` |
| Git tags | `v0.1.0-mvp1`, `v0.2.0` | `git tag` |
| GitHub Releases published | **none** | `gh release list` (empty) |
| CI for baseline | `CI` green (15:09 run); re-validating the docs-only #254 merge | `gh run list` |
| Security workflow for baseline | `Security` **success** | `gh run list` |
| Nightly Packages workflow | recurring **failure** (flaky/known, independent of campaign changes) | `gh run list` |
| Open PRs | none | `gh pr list` |
| Repo visibility / protection | public; `main` **no branch protection** (HTTP 404) | `gh api` |
| Developer list | @zoorpha @senolcolak | `.github/CODEOWNERS` |

### Anchor issue state (read on 2026-09-26)

| Issue | State | Truth from current code |
|---|---|---|
| #227 scope firewall to CHV traffic | OPEN | **Real, unmitigated**: base chains `input/forward/output` `policy drop`, no `iifname`/`oifname` guard → applying CHV policy drops all host/CNI/Docker/SSH traffic. DHCP path sanitization is already safe. |
| #231 single-authority cutover | OPEN | **Real gap**: default agent mode is `Legacy` (direct side effects, dual authority). Core SQLite authority + executor exist but executor is not invoked by any daemon; accepted ops would never run. |
| #185 EPIC standalone Core | OPEN | same as #231. |
| #233 typed startup validation | CLOSED | Done (PR #253): `validate_security_mode` typed, pre-construction, no panic. |
| #229 pin Rust toolchain / generated lint | OPEN | **Not done**: no `rust-toolchain.toml`; CI floats `stable` everywhere; only crate-level `allow(clippy::result_large_err)`. |
| #230 advisory cleanup | OPEN | **Partly stale**: `RUSTSEC-2026-0173` ignore is stale (proc-macro-error2 absent from `Cargo.lock`); `quick-xml` RUSTSEC-2026-0194/0195 still valid; `rsa` 0071, rustls-pemfile 0134, crossbeam-epoch 0204 still valid. |
| #146 remove proc-macro-error2 | OPEN | **Now resolvable**: dependency is already absent from `Cargo.lock` (tabled upgraded). |
| #177 remove quick-xml | OPEN | Still present (`quick-xml 0.38.4`) — genuine debt. |
| #232 stord mTLS wiring | CLOSED | Done (PR #252): config wired, fail-closed validation, ring provider installed. Protocol implemented; **no two-instance mTLS evidence yet**. |
| #234 frontend majors | OPEN | Deferred by campaign. |
| #235 tonic/prost upgrade | OPEN | Coordinated with #229/MSRV; deferred. |

### Real-host evidence reality

- `docs/evidence/cellhv-core-kvm/qualification.md` claims a passed lifecycle matrix but carries **no date, no SHA, no logs**; `evidence.md` is a one-word stub. → **No durable real-KVM evidence exists for any recent commit.**
- `integration-kvm.yml` runs only on a self-hosted `chv-kvm` runner, is gated behind a manual `kvm-test` label, and `kvm-smoke.sh` performs **install + health** checks, not VM boot/recovery.
- This workspace host (`/dev/kvm` present, 16 cores/31 GiB) is available as the campaign's KVM host.

---

## 2. Capability maturity reconciliation (current code)

| Capability | Maturity | Notes / evidence |
|---|---|---|
| VM lifecycle (Create/Start/Stop/Reboot/Delete) | CODED + CI-VERIFIED | Unit/integration on mock runtime; default mode uses legacy direct side effects; **no real KVM**. |
| Single durable lifecycle authority | CODED (not production-wired) | `cellhv-core-store`/`operations`/`executor` exist; `JournalExecutor` never started by a daemon (`scan_ready` test-only); default mode = Legacy. |
| Network policy (VXLAN/FDB/overlay) | CODED (unsafe) | Host-global default-drop present (#227); DHCP sanitization safe; zero privileged tests. |
| Storage: local file | CODED + CI-VERIFIED (unit) | Default backend; no real-KVM evidence. |
| Storage: LVM | CODED | `LVMBackend` present; no real evidence. |
| Storage: Ceph RBD / iSCSI | CODED | Present; **no external system** on-site → not qualifiable for the RC; remain CODED. |
| Storage migration (two stord, mTLS) | CODED + CI-VERIFIED (unit) | Full protocol (bulk copy, dirty rounds, paused final sync, CRC/sparse) implemented; `#232` wiring merged; **no two-instance mTLS run**. |
| Backup management (BFF) | CODED — broken | `backup_worker` execute is guaranteed-fail no-op; restore = DB record only. **Backup is not DR.** Snapshot/restore via CHV API exists separately (agent). |
| Packaging | CODED + CI-VERIFIED | `release.yml`/Makefile produce tar/deb/rpm/SHA256SUMS/SBOM/attestation; **no GitHub Release published**. |
| Toolchain reproducibility | NOT reproducible | Floating `stable`; no `rust-toolchain.toml`; generated-code lint policy crate-level only (#229). |
| Advisory policy | Implied some stale ignores | cargo-deny passes; stale `RUSTSEC-2026-0173`; `.cargo/audit.toml` not consumed by CI. |
| Security reporting path | Partial | Private advisory channel documented; email contact is a placeholder `security@<your-domain>`. |
| Repository protection | None enforced | No branch protection; CODEOWNERS advisory only. |
| UI / BFF / CLI | CODED + CI-VERIFIED | Existing; used by reference deployment. |
| Designer (topology planner) | CODED + CI-VERIFIED | Extensive suite; unchanged by campaign (no promotion of claims). |

---

## 3. Release boundary (smallest supportable candidate)

- **VMM:** Cloud Hypervisor only (`chv-agent` runtime CH; `v43.0` pin used by `kvm-smoke.sh`).
- **Host OS profile:** Linux x86_64; `.deb` (Debian/Ubuntu) + `.rpm` packages; exact support list confirmed at Prompt 05 from install docs. This box is the KVM host.
- **Network profile:** one CHV-owned VXLAN overlay topology, host-safe after Prompt 01 (must not affect unrelated host/container/SSH/forwarded traffic).
- **Storage profile(s):** local file + LVM only. Migration exercised over the local backend between two stord instances (single host, two processes, mTLS).
- **Lifecycle authority:** after Prompt 02, exactly one durable CellHV Core authority; legacy/control-plane paths become compatibility adapters only.
- **Backup/restore:** **excluded from the RC supported matrix** unless a minimal restore-verified path is added and qualified — a backup without restore validation is not DR and will be labeled unsupported in the RC.
- **Migration:** RC may claim single-host qualified migration; **multi-host migration remains unproven** on this infrastructure and will be stated as such.
- **UI/BFF/CLI:** supported for the reference deployment.

## 4. Current blockers (grounded, must clear before a stable-production claim)

1. Host-global default-drop network policy (#227) — unsafe confinement.
2. Dual lifecycle authority in default mode + production-uninvoked Core executor (#231/#185).
3. No durable real-KVM evidence for this line (evidence docs are stubs).
4. Backup/restore management is broken no-op + record-only — no DR claim possible.
5. No published GitHub Release — RELEASED tier unproven.
6. Toolchain/generated-code reproducibility gap (#229).
7. Stale advisory ignores + placeholder security contact + no branch protection.

## 5. Evidence root and required real-host topology

- Evidence root: `docs/evidence/production-readiness/v0.3.0-rc1/`
- Available topology (single host): `ubuntu` box, `/dev/kvm`, 16 vCPU / 31 GiB.
- Planned: control-plane + agent + nwd + two stord instances on this host; clean-host install validated inside isolated Linux containers/VMs **on the same physical host** — labeled precisely as same-physical-host evidence.
- **Not available:** a second physical host. => MULTI-HOST-VERIFIED and FIELD-QUALIFIED tiers will be reported **unprovable on this infrastructure**; capabilities are capped at KVM-VERIFIED (single host) with explicit residual risk.

## 6. Explicit non-scope (deferred programme)

NetBox projection; VMware import/migration; additional VMMs; external Ceph/iSCSI qualification; another storage backend; Kubernetes/operator machinery (incl. any CNI-coexistence claim — no K8s-capable worker); speculative control-plane HA; database replacement; major UI redesign; broad OpenStack compatibility claims; Designer expansion; frontend/tonic major toolchains (#234/#235) beyond what Prompt 03 coordination strictly needs.

## 7. Residual unknowns to resolve in later prompts

- Toolchain pin choice / MSRV (Prompt 03); generated-code lint policy (Prompt 03).
- Whether advisory ignores that are stale can be removed while Security stays green (Prompt 03).
- Real security contact value for `SECURITY.md` (Prompt 03) — decision needed from maintainers.
- Whether release signing/provenance secrets are configured (Prompt 05): checksums at minimum; signatures only if secrets exist.
- Cloud Hypervisor + guest image availability on this box (Prompt 04 install).
- Whether the reference-deployment window (Prompt 06) is feasible on a single shared host without disrupting other activity.

---

## Contradictions found (stale documentation vs. code)

- `docs/specs/component/live-migration-spec.md:212` claims "RoundStart/RoundComplete never sent" — **contradicted** by current `sender.rs:413-583` (dirty rounds implemented). Marked stale in Prompt 04 scope.
- `legacy_core_adapter.rs` doc comment "deliberately not called" is **stale** — the adapter is wired and dispatched from `cmd/chv-agent/src/main.rs`.
- `CoreRuntimeOwner` module doc "not wired" is stale — it is used by `cmd/chv-agent` startup.
- `docs/evidence/cellhv-core-kvm/qualification.md` implies a passed KVM matrix with no SHA/date/logs — not usable as evidence.
