# CellHV Core Phase D NWD Blockers

Status: Design required; no Phase D readiness claim

## Decision

The current `chv-nwd` implementation is not a safe CellHV Core preparation
provider. A bounded hardening attempt was reverted because lifecycle ownership,
filesystem authority, and compensation cannot be made correct by input
validation and error propagation alone. Production VM behavior remains
unchanged.

## Blocking facts

- `crates/chv-nwd-core/src/handlers.rs` treats the in-memory `TopologyTable` as
  active authority while persistence helpers log and swallow SQLite failures.
  Production `cmd/chv-nwd/src/main.rs` constructs `NetworkServer` without a
  `TopologyStore`, so restart ownership is not durable.
- `crates/chv-nwd-core/src/executor.rs` receives request-controlled bridge,
  namespace, network, and NIC identifiers and invokes privileged `ip`,
  `bridge`, `nft`, `dnsmasq`, and `kill` operations. A deterministic name is
  not proof that NWD created or still owns the corresponding host object.
- Existing deployment contracts intentionally use the pre-created `chvbr0`
  bridge, while agent code also derives short and hashed bridge names. Name
  validation cannot distinguish an installed compatibility resource, a legacy
  resource, a foreign collision, and an NWD-created resource.
- Preparation contains multiple externally visible steps. A failure after one
  step cannot be safely compensated without durable per-step ownership facts.
  Deleting all deterministically named objects risks deleting foreign or
  replacement resources.
- Delete cannot safely remove durable ownership before host teardown, or remove
  host resources before recording durable delete intent. Both orderings need a
  journaled state machine and restart reconciliation.
- Overlay updates mutate multiple FDB entries. Reverse commands are not a
  transaction and may fail or race; recording old state after incomplete
  compensation would be false.
- dnsmasq PID files are not process identity. Signalling a recycled PID can
  affect an unrelated process unless start-time identity and executable/cgroup
  ownership are verified.
- SQLite path validation followed by pathname reopen leaves a replacement
  interval. SQLite journal/WAL/SHM sidecars also require a secured database
  directory and explicit file policy.
- Legacy rows may contain custom names. Loading them as active grants
  privileged authority; dropping or keying quarantine only by network ID can
  hide or overwrite distinct unresolved ownership evidence.
- Idempotency currently compares only part of the topology definition. A safe
  replay requires a canonical full-request fingerprint and explicit generation
  semantics.

## Required design before implementation

1. Define a durable NWD operation/state machine with per-resource ownership
   records, creation identity, generations, canonical request fingerprints,
   and journaled `prepare`, `apply`, `delete`, and `reconcile` steps.
2. Define host-object adoption policy separately from creation. The policy must
   cover `chvbr0`, legacy custom names, deterministic-name collisions, and
   proof required before mutation or deletion.
3. Define a restart reconciler that observes host resources without granting
   authority from names alone. Transitional records must not be exposed as
   successfully active.
4. Define overlay convergence as desired-state reconciliation from a complete
   durable rule set, not speculative reverse commands.
5. Replace PID-file signalling with process identity evidence that survives PID
   reuse, or delegate dnsmasq lifecycle to a supervised service boundary.
6. Define a secured storage layout for the database and all SQLite sidecars,
   including owner, mode, symlink, hard-link, locking, and atomic-open rules.
7. Define a lossless quarantine record keyed by the full legacy row identity.
   Quarantined rows must be non-actionable and visible to operators until an
   explicit migration/adoption decision is recorded.
8. Add a command-runner and persistence boundary for deterministic failure and
   race injection before enabling the provider from CellHV Core.

## Minimum evidence

Phase D NWD readiness requires tests for crash/restart at every journaled step,
foreign deterministic-name collisions, replacement races, persistence failure,
teardown failure, PID reuse, SQLite sidecar attacks, legacy quarantine
collisions, full-fingerprint replay/conflict, and overlay convergence after
partial command failure. Real namespace/bridge/nft tests must run only in an
isolated disposable network namespace or VM.
