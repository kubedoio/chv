# CellHV O3K Core Client Contract v1

Status: Proposed; registered for T1; not implemented or executed

## Authority boundary

O3K is a management-plane integration above CellHV Core. `chv-agent` remains
the sole CellHV Core runtime authority under ADR-016 and ADR-017. An O3K
integration must use the public Core API and must not add another daemon,
authoritative VM database, operation engine, process owner, or lifecycle
authority. Storage and network host preparation remain delegated to
`chv-stord` and `chv-nwd` through Core.

This contract is pinned to `https://github.com/kubedoio/o3k` revision
`53fd2cb36ee79f42da49c8181d6ceed12b41b3aa`. Its Go module still declares
`github.com/cobaltcore-dev/o3k`; the repository and module identities are
recorded separately rather than silently normalized.

## Audited current state

At the pinned revision O3K constructs and consumes a concrete
`*hypervisor.VMManager`, maintains its own database and task scheduling paths,
and reaches libvirt-oriented lifecycle behavior directly. It has no injectable
CellHV Core client boundary. Its existing compatibility tests are source and
unit tests, not an O3K-to-Core process integration. Those facts block execution
of this profile today and contradict any claim that O3K is already a Core
client.

CellHV's native API currently exposes durable VM-definition CRUD and an
operation journal, but all executable capabilities are false and power actions
are unsupported. Definition acceptance is not VM launch or OpenStack compute
compatibility.

## Required client ownership

The future client and translation layer belong in O3K. A generic test client in
CHV must not be presented as O3K evidence. The O3K boundary must be injectable
and must project Core state rather than creating another authoritative VM or
operation store.

O3K instance UUID maps directly to Core VM UUID. Durable intent identities must
be deterministic, surface-namespaced, and stable across client retries and
restarts, for example `o3k:nova:{operation}:{instance_uuid}:{generation}`.
Replaying the same intent and payload must return the same Core operation;
reusing it with a changed payload must conflict. Random per-attempt idempotency
keys are forbidden.

## Registered scenarios

| ID | Purpose | Maximum tier |
|---|---|---|
| `OCORE-001` | Pin and crosswalk the audited O3K source contract | T1 |
| `OCORE-002` | Verify deterministic identity, replay, and conflict semantics | T1 |
| `OCORE-003` | Exercise definition journaling and restart with real isolated processes | T1 |
| `OCORE-004` | Verify capability-negative and unsupported-action behavior | T1 |
| `OCORE-005` | Verify client restart projection without transferring authority | T1 |

The machine-readable registration is
`docs/acceptance/cellhv-o3k-core-client-contract-v1.json`. Every scenario is
strictly T1. This profile is structurally ineligible for T5.

## Evidence rules

Execution requires an O3K-owned Core client, an injectable O3K compute
boundary, and an isolated real-process harness. Evidence must include exact
revisions, commands, process logs, Core journal records, capability responses,
restart boundaries, and cleanup results. It must record the exact first blocker
instead of converting a blocked or skipped scenario into a pass.

No O3K integration was run while registering this contract. This document
makes no OpenStack, Nova, libvirt, Neutron, Cinder, VM launch, power-operation,
or production qualification claim. ADR-018 T5 ecosystem qualification remains
a separate evidence programme.
