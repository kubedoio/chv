# CellHV Core Runtime Ownership Recovery Boundary

**Status:** Phase C design baseline; not production-wired  
**Authority:** `chv-agent` remains the sole VM lifecycle authority

This boundary verifies whether an existing Cloud Hypervisor process may be
re-adopted after agent restart. It does not discover arbitrary processes,
launch VMs, mutate journal state, or create another lifecycle authority.

## Canonical layout

For data root `R` and canonical VM identifier `V`:

```text
R/vms/V/
  vm.sock
  owner-v1.json
```

`V` must be both a validated `VmId` and the marker's validated safe
`runtime_directory_name`; the API basename is exactly `vm.sock`. Paths are
constructed from components, never accepted as caller-provided paths. Every
ancestor is descriptor-walked as a real non-symlink directory. The final
directory passed to `MarkerStore` must additionally be owned by the effective
agent uid and have no group/other permission bits; this slice does not claim
that the same private-mode check is applied to every ancestor. The marker
must be a regular file with link count one. The API endpoint must be a Unix
socket. Inspection opens directories and files with no-follow semantics and
compares metadata from the opened descriptor, not from a prior path lookup.

## Owner marker v1

`owner-v1.json` has a bounded strict schema with no unknown fields. The writer
emits compact deterministic struct-field order, but the reader accepts
equivalent JSON whitespace and object-key ordering. This slice therefore does
not claim canonical JSON or require byte-canonical input:

```json
{
  "schema_version": 1,
  "host_id": "stable-host-id",
  "vm_id": "vm-id",
  "operation_id": "operation-id",
  "runtime_generation": "018f6f20-7b6d-7d10-8000-000000000001",
  "active_attempt_token": "opaque-visible-ascii-token",
  "config_fingerprint": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "publication_nonce": "random-safe-component",
  "pid": 1234,
  "proc_start_ticks": 987654,
  "boot_id": "018f6f20-7b6d-7d10-8000-000000000002",
  "executable": { "device": 2049, "inode": 42 },
  "uid": 1000,
  "gid": 1000,
  "cgroup_fingerprint": "/cellhv/vm-id",
  "runtime_directory_name": "vm-id",
  "api_socket_name": "vm.sock",
  "runtime_directory": { "device": 2049, "inode": 43 },
  "api_socket": { "device": 2049, "inode": 44 }
}
```

`proc_start_ticks` is field 22 of `/proc/PID/stat`, parsed after the final `)`
so spaces and parentheses in the command name cannot shift fields. Executable
identity comes from `fstat` of `/proc/PID/exe`; pathname text is evidence only.
The socket identity comes from `fstat` of the opened Unix socket endpoint.

Markers are written only by the runtime authority: create a new same-directory
temporary file with mode `0600` and exclusive/no-follow flags, write the
deterministic JSON, `fsync` the file, publish with
`renameat2(RENAME_NOREPLACE)`, then `fsync` the VM directory. Publication never
overwrites an existing marker. A marker is never updated in place. Failed
writes remove only the writer's uniquely named temporary.

## Inspection result

Inspection returns exactly one classification and evidence safe for journaling:

- `OwnershipMatched`: every marker field matches the requested durable launch
  and the ordered process-before, socket/API, process-after, and pidfd-liveness
  evidence matched. This is evidence only, not an adoption/control capability.
- `OwnedAliveSocketUnavailable`: exact process identity is stable and live but
  the authenticated API socket is absent or unresponsive.
- `ExitedOwned`: both ordered process observations and socket are absent and
  pidfd liveness independently reports false. Absence with a live pidfd is
  `AmbiguousPreserve`.
- `ForeignConflict`: valid evidence names another host, VM, operation,
  generation, attempt, or config. Do not signal,
  connect, unlink, or overwrite.
- `AmbiguousPreserve`: only part of the process/socket identity matches, observations
  race, or permission prevents a complete proof. Quarantine for operator review.
- `DuplicateConflict`: more than one candidate process exists.
- `CorruptOwnership`: marker schema, bounds, encoding, file type, ownership,
  permissions, or link invariants fail. Do not repair automatically.

PID existence alone is never ownership evidence. A reused PID is
`AmbiguousPreserve`, never `OwnershipMatched`. Socket pathname existence alone is likewise
insufficient; device and inode must match the marker.

## Control remains pending

This slice deliberately returns no adopted process or control handle. A later
Linux adapter must retain identity-bound pidfd and connected socket
capabilities and revalidate them around every action. `OwnershipMatched` alone
never authorizes signalling, unlinking, journal resolution, or an API mutation.

A narrow runtime-neutral trait supplies observations for deterministic tests:

```rust
trait Observation {
    type Error;
    fn process_before(&self, pid: u32) -> Result<Option<ProcessIdentity>, Self::Error>;
    fn socket(&self, vm: &VmId) -> Result<Option<SocketIdentity>, Self::Error>;
    fn process_after(&self, pid: u32) -> Result<Option<ProcessIdentity>, Self::Error>;
    fn pidfd_alive(&self, pid: u32) -> Result<bool, Self::Error>;
    fn duplicate_candidates(&self, vm: &VmId) -> Result<bool, Self::Error>;
}
```

Linux filesystem and `/proc` access belong in a later adapter. The classifier
itself must be pure over bounded typed observations, enabling fake process,
socket, PID-reuse, race, symlink, and hardlink evidence tests without KVM.

## Integration constraint

`chv-agent-runtime-ch` may eventually hold either a launched or identity-bound
re-adopted process handle behind one control interface. The current launch path and
its `HashMap<Uuid, VmProcess>` remain unchanged in this slice. No executor or
production composition may call inspection until durable recovery transitions
and their acceptance evidence are reviewed.

## Machine-enforced boundary and evidence level

`scripts/check-cellhv-core-architecture.py` enforces the current unwired
boundary. The Linux observation module is `pub(crate)` inside
`crates/chv-agent-runtime-ch`, making compiler visibility the authoritative
construction boundary. API, compatibility, storage, network, and other
provider packages may not directly or transitively depend on the ownership
crate or the guarded general-purpose process-inspection crates.
`cmd/chv-agent` may neither depend directly on the ownership crate nor
construct an observer in this phase. Source scanning catches common direct,
qualified, and aliased implementations and scans every agent command module,
but is defense in depth rather than a claim to parse every valid Rust program.
Mutation tests in `tests/test_cellhv_core_architecture.py` exercise the compiler
visibility declaration, transitive dependency rule, implementation location,
aliases, and non-main agent modules.

An unwired `LinuxOwnershipObservation` now collects bounded process, pidfd,
socket, peer-credential, and API-probe evidence. Its duplicate-candidate method
returns `CapabilityUnavailable`; because the classifier treats unavailable
duplicate proof as ambiguity, it cannot currently yield a positive real-world
ownership classification. This is intentional fail-closed incompleteness, not
recovery readiness.

Those checks are T0 architecture evidence. The pure classifier, hostile marker
store, and in-process Linux observer tests are T1 deterministic evidence. They
do not constitute an isolated-process T2 result, real-KVM T3 recovery, a usable
positive recovery decision, or permission to change VM launch and management
behavior.
