# Evidence: CellHV Core Phase C1 Runtime Observation and Adoption

Date: 2026-07-22

Branch: `agent/cellhv-core-runtime-observation-adoption`

Draft PR: [#191](https://github.com/kubedoio/chv/pull/191)

## Summary of Implementation

This slice implements a recovery-safe Cloud Hypervisor inspection, classification, and re-adoption model without requiring a retained `tokio::process::Child` process handle.

### 1. Ownership Marker Schema (`OwnerMarkerV1`)
- Extended `OwnerMarkerV1` in `crates/cellhv-core-runtime-ownership` to include `created_at_utc`.
- Enforces `#[serde(deny_unknown_fields)]` and strictVisible-ASCII/UUID/hex validations for all fields.

### 2. Complete Bounded Duplicate-Candidate Detection
- Bounded procfs candidate discovery scanning up to `131072` entries or 2-second wall-clock time limit.
- Anchored socket path suffix matching (`/{vm_id}/vm.sock`) ensuring exact directory boundary evaluation.

### 3. Explicit Classification Model
- Implemented typed classification enum:
  - `OwnedRunning`: Proven cellhv-owned process, live socket, responsive API.
  - `OwnedStopped` / `ExitedOwned`: Proven process exited cleanly, pidfd verified dead.
  - `OwnedUnresponsive`: Process alive and matched, but API socket non-responsive.
  - `Missing`: Neither process nor socket exists.
  - `StaleMarker`: Marker exists, but PID and socket are missing.
  - `ForeignProcess`: Process or socket owned by a non-matching host/VM/generation identity.
  - `DuplicateCandidates`: Multiple candidate Cloud Hypervisor processes detected.
  - `IdentityMismatch` / `SocketMismatch` / `HostMismatch`: Identity check failure.
  - `PermissionDenied`: `/proc` or socket inspection blocked by OS permissions.
  - `CapabilityUnavailable`: System lacks pidfd or required inspection capability.
  - `AmbiguousPreserve`: Fail-closed fallback when evidence is incomplete or inconsistent.

### 4. Recovery-Safe Adopted Handle (`AdoptedVmHandle`)
- Implemented in `crates/chv-agent-runtime-ch/src/process.rs`.
- Does **NOT** hold a `tokio::process::Child` handle.
- Dropping `AdoptedVmHandle` performs **zero** mutations (no `sigkill`, no socket deletion).
- Automatically re-validates active process identity prior to invoking API operations.

## Test Tier Summary

- **T1 Deterministic Tests**: 15 unit tests in `cellhv-core-runtime-ownership` + 35 unit tests in `chv-agent-runtime-ch`.
- **T2 Isolated Linux Process Tests**: `adopted_vm_handle_revalidates_successfully` mock `/proc` environment test in `chv-agent-runtime-ch`.
- **Explicit Missing T3 Evidence**: No real KVM qualification or production VM launch was performed in this slice.

## Unwired Confirmation

All new inspection, classification, and adoption capabilities are callable from tests and prepared for runtime composition, but remain strictly **production-unwired**. No VM lifecycle capabilities are advertised.
