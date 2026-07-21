# CellHV Core legacy gRPC adapter

Status: **implemented, deliberately not wired**

The existing `chv.controlplane.node.v1.LifecycleService` remains the production
VM lifecycle path. `chv-agent-core::legacy_core_adapter` provides the bounded,
transport-independent conversion needed to make that API a future compatibility
adapter over the single `cellhv-core-operations::OperationService` authority.
It does not create an executor, store, daemon, or provider path.

## Identity and idempotency mapping

For a request targeting node `N`, VM `V`, operation `O`, and decimal desired
generation `G`, the adapter emits:

- a namespaced Core operation ID containing length-prefixed `N`, `V`, and `O`;
- external operation ID `O`, requester, request timestamp, and numeric legacy
  generation `G` in `LegacyMutationIntent` audit metadata;
- an expected Core VM resource version supplied separately by a future
  coordinator;
- idempotency scope: `control-plane-node.v1/node/<len(N)>:N/vm/<len(V)>:V`;
- idempotency key: `operation/<len(O)>:O/generation/<len(G)>:G`.

Length prefixes prevent delimiter-containing opaque identifiers from producing
ambiguous identities. Namespacing prevents an operation ID used through another
API surface from colliding with the legacy request. The mapping is deterministic.
The adapter rejects an empty or mismatched target node, an empty operation ID,
and any generation that is not canonical positive decimal syntax (`7` is valid;
`0`, `07`, `+7`, and whitespace-padded forms are not).
The Core operation service fingerprints the command and expected version, so a
reused scope/key with different content remains an idempotency conflict.

## Lossless supported subset

`StartVm`, non-forced `StopVm`, non-forced `RebootVm`, and non-forced `DeleteVm`
map directly to Core commands. They require the coordinator to provide the
current Core version; legacy generation is never used as a Core compare-and-swap
version. `CreateVm` maps any canonical legacy generation to initial Core version
`1`, but only when the coordinator explicitly supplies Core version `1` and the
legacy VM specification uses fields represented by `VmDefinition`: name,
CPU and memory, boot paths, storage reference/read-only state, network reference
and MAC address, and `Running` or `Stopped` desired state.

The adapter fails closed for forced lifecycle actions, cloud-init user data,
hypervisor overrides, disk requested sizes, and legacy NIC IP/CIDR/gateway/tap
fields. Silently discarding these values would not be a compatibility adapter.
Their eventual representation requires an explicit Core contract decision.
Attachment IDs use the same exported construction functions as the NodeCache
migration importer, preventing the live compatibility and import paths from
creating different identities for the same legacy attachment.

## Production cutover gate

No `AgentServer` handler calls this adapter. Current handlers mutate the node
JSON cache and perform provider side effects in a sequence that is not atomic
with Core operation acceptance. Wiring the adapter before replacing that flow
would permit the Core journal and `NodeCache` to disagree after a crash.
`LegacyMutationIntent` retains audit metadata in memory, but the current Core
operation journal has no fields for requester, request timestamp, external
operation ID, or legacy generation. A coordinator must define and durably store
that metadata before invoking `OperationService::submit`; constructing an intent
alone is not durable audit evidence.

Production routing may be enabled only after one authoritative transaction owns
mutation acceptance and the legacy cache is either derived from Core state or
updated through a proven crash-consistent compatibility mechanism. Until then,
VM launch, stop, reboot, and deletion behavior is unchanged.

## Evidence

Unit tests in `chv-agent-core::legacy_core_adapter::tests` cover deterministic
identity mapping, the lossless create subset, invalid generation/target
rejection, shared attachment identity, and rejection of unsupported legacy fields. The module has no direct
dependency on `cellhv-core-store`, `chv-agent-runtime-ch`, `chv-stord`, or
`chv-nwd`; it submits only the shared Core operation types.
