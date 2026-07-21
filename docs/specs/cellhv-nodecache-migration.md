# CellHV NodeCache Migration Contract

Status: Phase B compatibility component; not wired into production startup.

## Boundary

`cellhv-nodecache-migration` is a read-only adapter from the serialized
`chv-agent-core::cache::NodeCache` version-1 format to the single Core authority.

While NodeCache remains the compatibility cache, each save preserves its JSON
format and uses a crash-consistent same-directory replacement: a uniquely named
mode-`0600` temporary file is written and synced, renamed atomically, and the
parent directory is synced. Saves and startup cutover take the same sibling
authority-file advisory lock. Per-path monotonic sequencing prevents a cancelled
older blocking save from replacing a newer snapshot after its async waiter has
gone away. Only a successful rename advances the committed sequence; reserving
a newer sequence does not suppress an older write if the newer job fails or is
never started. A failed serialization, temporary-file write, or pre-rename sync
leaves the last good cache in place and removes the writer-owned temporary file.

The cache directory is pre-provisioned, service-owned state: save does not create
it. Its immediate parent must already exist as a real directory owned by the
effective service user and must not be group- or world-writable. Symlink and
non-regular cache destinations are rejected. A
parent-directory sync failure reported after a successful rename is ambiguous:
the new file is visible, but its survival across a crash is not guaranteed, so
callers must treat the error as indeterminate and reload rather than assume the
old snapshot remains. Normal failures remove their temporary file; abrupt
process or host termination may leave a uniquely named stale temporary file.
This phase does not scavenge those files because an age/ownership policy has not
yet been accepted.
It enters that authority only through `cellhv-core-operations::OperationService`;
the store remains private to the application-service boundary. The adapter is
not a database, daemon, operation engine, or VM lifecycle path. It does not
invoke Cloud Hypervisor, `chv-stord`, or `chv-nwd`.

The source byte sequence is the rollback artifact. The adapter never changes or
deletes it. An operator or future startup coordinator MUST durably archive those
exact bytes before calling cutover. The SHA-256 digest of the exact bytes is
recorded in `migration_state`; JSON reformatting therefore produces a different
source and cannot replay the original import.

## State Machine

```text
absent --import--> imported --cutover--> cutover
                       |
                    rollback
                       |
                     absent
```

- Import atomically writes the host identity, VM definitions, attachments, and
  `migration_state` marker. The marker includes the imported host and sorted VM
  identity manifest. It requires an empty Core authority.
- Repeating import with the exact checksum is a no-op. A different checksum or
  an occupied authority is a conflict.
- Rollback is permitted only before cutover, while no operation journal rows
  exist, and while the live host and complete VM identity set exactly match the
  import manifest. Any drift fails closed before deletion. It removes only the
  import transaction's authority records; it never touches the cache or VM
  processes.
- Cutover is idempotent for the exact checksum and irreversible through this
  API. After cutover the JSON cache MUST NOT be independently writable for VM
  state. ADR-019 proposes the process-wide `legacy-vm-authority`,
  `core-vm-authority`, and `blocked` modes that enforce that ownership switch.

## Deterministic Mapping

| NodeCache value | Core value |
|---|---|
| `node_id` | `HostIdentity.id` |
| VM fragment map key | `VmDefinition.id` |
| numeric VM generation | VM `resource_version` |
| VM name, CPU, memory and boot paths | corresponding Core definition fields |
| disk `volume_id` | storage attachment ID and provider reference |
| NIC network and MAC | network reference and MAC; attachment ID is the existing `VM-ID-NETWORK-ID` convention |
| desired `Running` / `Stopped` | requested power state |
| no durable legacy observation | observed state `unknown` |

The legacy `node_id` must also satisfy ADR-019's Core host-ID policy. In
particular, a literal placeholder such as `unknown` is not an importable Core
identity. It is never silently rewritten; startup either establishes a valid
fresh identity under the ADR-019 decision table or enters `blocked`.

Input maps are normalized through sorted maps and output definitions are sorted
by VM identity. Identifiers are never regenerated.

## Fail-Closed Compatibility

Parsing rejects unknown NodeCache and VM-spec fields. Cache version, map and
fragment identity, kind, generation, fragment metadata, policy JSON, auxiliary
fragment JSON, attachment projections, and domain constraints are validated
before the store transaction.

Fields without a lossless Core representation are explicit `Unsupported`
errors when populated: cloud-init userdata, hypervisor overrides, disk size
hints, prepared NIC runtime fields, provider volume handles, and queued control
plane messages. This deliberately makes some real caches ineligible until a
separate lossless mapping is designed; none of that data is silently discarded.

Enrollment material, node state, auxiliary desired-state fragments, and error
metadata remain in the retained cache compatibility artifact. They are
validated where structurally applicable but do not become VM authority.

## Production Gate

No production caller exists in this phase slice. Wiring requires all of the
following in one reviewed change:

1. atomic archival and fsync of the exact source bytes;
2. startup exclusion preventing NodeCache VM writes after cutover;
3. legacy requests routed through the single Core operation engine;
4. explicit operator recovery for unsupported or malformed caches;
5. restart tests proving that a cutover marker cannot reactivate JSON authority.

In `core-vm-authority` mode, compatibility-only cache fields may still be
persisted, but a save must first prove that the complete VM-authoritative JSON
projection is unchanged from the frozen post-import projection. This permits
enrollment and telemetry compatibility without restoring JSON VM authority.
