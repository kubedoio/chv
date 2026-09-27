# CellHV Core legacy gRPC adapter

Status: **implemented, wired in core-managed authority mode**

The existing `chv.controlplane.node.v1.LifecycleService` remains the production
VM lifecycle path, but in core-managed authority mode its five lifecycle
handlers route through `chv-agent-core::legacy_core_adapter`, which provides the
bounded, transport-independent conversion needed to make that API a
compatibility adapter over the single `cellhv-core-operations::OperationService`
authority. The adapter itself does not create an executor, store, daemon, or
provider path; it only translates legacy requests into Core submission
envelopes.

## Identity and idempotency mapping

For a request targeting node `N`, VM `V`, operation `O`, and decimal desired
generation `G`, the adapter emits:

- a namespaced Core operation ID containing length-prefixed `N`, `V`, and `O`;
- external operation ID `O`, requester, request timestamp, and numeric legacy
  generation `G` in both `LegacyMutationIntent` audit metadata and the durable
  `OperationRequestMetadata` carried on `SubmitMutation`; the latter is written
  into the operation journal by `OperationService::submit` (see migration
  `0004_operation_request_metadata.sql`);
- an expected Core VM resource version supplied separately by the current Core
  authority in core-managed mode;
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

In core-managed authority mode the `AgentServer` lifecycle handlers call this
adapter and submit its `LegacyMutationIntent.submission` through
`OperationService::submit`. The adapter's audit metadata (requester, external
operation ID, request timestamp, legacy desired generation) is carried as
`OperationRequestMetadata` on the submission and durably persisted into the
operation journal (`operations` columns `requested_by`,
`external_operation_id`, `request_unix_ms`, `legacy_generation`, added by
migration 0004) in the same atomic transaction as operation acceptance — it is
never memory-only. `LegacyMutationIntent` also retains its own copy for the
in-memory compatibility path; both always agree by construction.

Requester/audit identity is therefore available in the durable journal even
before a separate authorization story lands. The only remaining cutover
condition is that the legacy NodeCache must either be derived from Core state
or updated through a proven crash-consistent compatibility mechanism, so that
no two partially committed views of desired state can disagree after a crash.
Until that is proven, core-managed mode is an explicit opt-in, and the default
authority mode is unchanged.

## Evidence

Unit tests in `chv-agent-core::legacy_core_adapter::tests` cover deterministic
identity mapping, the lossless create subset, invalid generation/target
rejection, shared attachment identity, rejection of unsupported legacy fields,
and end-to-end surface of the submitted audit metadata in the journal entry
`request_metadata`. The module has no direct
dependency on `cellhv-core-store`, `chv-agent-runtime-ch`, `chv-stord`, or
`chv-nwd`; it submits only the shared Core operation types.
