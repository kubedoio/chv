# ADR-016: Evolve `chv-agent` into the CellHV Core Runtime

## Status

Accepted

## Date

2026-07-21

## Context

The CellHV Core foundation documents used `cellhvd` to describe the future standalone runtime while the repository already contains `chv-agent`, which owns node-local VM lifecycle, Cloud Hypervisor integration, reconciliation, telemetry, and coordination with storage and networking services.

Treating `cellhvd` and `chv-agent` as separate services would create duplicate process ownership, a flag-day migration, additional packaging and upgrade complexity, and an unclear authority boundary.

The intended product change is an evolution of the existing node runtime, not the introduction of a second runtime daemon.

## Decision

1. The existing `chv-agent` implementation evolves in place into the CellHV Core runtime.
2. `chv-agent` and the conceptual CellHV Core daemon are the same runtime authority.
3. No parallel `cellhvd` binary, service, database, operation engine, or VM process owner will be introduced.
4. Until a separate naming and packaging ADR is accepted, the binary and systemd service remain named `chv-agent`.
5. A future rename may provide a temporary compatibility alias or package transition, but it must not result in two active runtime services.
6. The existing agent code is refactored incrementally: durable local authority, standalone APIs, recovery, and provider boundaries are added behind compatibility paths.
7. During migration, the current control-plane protocol may coexist with the new native Core API, but both enter the same operation engine and durable state.
8. Controller loss must not affect local VM survival, and Controller state becomes a projection rather than the sole authoritative VM record.
9. Existing `chv-stord` and `chv-nwd` code may be retained, narrowed, or reorganized through later ADRs; this decision does not require replacing them immediately.

## Consequences

### Positive

- reuses working lifecycle and Cloud Hypervisor code;
- avoids a flag-day rewrite;
- creates one clear VM authority;
- reduces packaging and upgrade risk;
- allows compatibility with the current control plane during migration;
- makes implementation prompts concrete and repository-aware.

### Negative

- the existing agent must be carefully untangled from control-plane-owned desired state;
- transitional APIs and state migration require explicit compatibility tests;
- the historical `chv-agent` name may not match final product branding;
- some current assumptions may need staged deprecation rather than removal.

## Rejected alternatives

### Create a new `cellhvd` beside `chv-agent`

Rejected because two runtime daemons would create ambiguous process and state ownership.

### Rewrite the node runtime from scratch

Rejected because it discards validated code and creates excessive implementation and regression risk.

### Keep `chv-agent` permanently dependent on the central control plane

Rejected because CellHV Core must be useful and recoverable as a standalone compute runtime.

## Acceptance conditions

- implementation plans and prompts describe `chv-agent` as the Core implementation;
- no new parallel runtime binary is created;
- the first durable-state change is implemented inside or directly beneath `chv-agent`;
- compatibility tests prove that legacy control-plane and native local operations enter one authority path;
- process ownership tests prove only one runtime controls each VM;
- any future rename is handled by a separate ADR.
