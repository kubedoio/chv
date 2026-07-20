# Prompt 04 — Phase C: Standalone Runtime and Recovery

Complete the first trustworthy standalone CellHV Core profile by evolving `chv-agent` and reusing the existing Cloud Hypervisor runtime code.

## Preconditions

- Phase B local authority is merged.
- The qualification tuple is complete and approved.
- `chv-agent` has one durable store and one operation engine.
- Use branch `agent/cellhv-core-pc-runtime-recovery`.

## Estimated effort

6–8 engineering weeks, split into narrow PRs:

1. VMM contract and existing-runtime adaptation;
2. minimal real VM lifecycle;
3. daemon re-adoption;
4. host reboot and database recovery;
5. ownership conflict and leak/fault qualification.

## Goal

Run and recover one qualified Linux VM on one supported KVM host through `chv-agent` without Controller or libvirt, using one pre-existing disk and one pre-existing network endpoint.

## Required work

### 1. Narrow VMM contract

Define only the Cloud Hypervisor operations needed by the qualified profile:

- validate runtime and host capability;
- prepare supported VM configuration;
- create/start process or systemd unit;
- inspect power/runtime state;
- graceful stop and force stop;
- reboot;
- delete owned runtime artifacts;
- retrieve serial-console endpoint;
- enumerate and re-adopt owned runtime processes.

Do not expose arbitrary Cloud Hypervisor JSON, QMP, QEMU command lines, or cloud-platform models.

### 2. Reuse the existing Cloud Hypervisor implementation

Refactor `chv-agent-runtime-ch` rather than creating a competing adapter.

Requirements:

- pinned version validation;
- deterministic runtime directories and API sockets;
- stable ownership markers tied to VM UUID;
- one VM process/unit per VM;
- safe argument/config construction;
- structured error mapping;
- serial console for the qualified guest;
- truthful capabilities;
- explicit cleanup;
- no QEMU identity or QMP compatibility.

### 3. Minimal attachments

Support only:

- one pre-existing raw file or block-device path;
- one pre-existing bridge or TAP endpoint;
- virtio block and virtio net;
- one qualified firmware or direct-kernel boot mode;
- serial console.

Do not provision LVM, bridges, VLANs, NAT, RBD, Neutron, or Cinder in this phase.

### 4. Durable execution

For create/start/stop/reboot/delete:

```text
validate -> record operation -> prepare -> perform action -> inspect -> persist observed state -> emit event
```

Every host-side action must correspond to an operation step. Retry must not duplicate units, processes, sockets, disks, NIC ownership, or VM records.

### 5. Daemon restart and re-adoption

On `chv-agent` startup inspect:

- durable VM records;
- incomplete operations;
- VM systemd units/processes;
- Cloud Hypervisor sockets;
- runtime directories and ownership markers;
- attachment records and observable state.

Classify explicit states such as:

- consistent-running;
- consistent-stopped;
- recoverable-incomplete;
- database-only;
- runtime-only;
- externally-running-unowned;
- ownership-conflict;
- operator-action-required.

Preserve ambiguous running workloads. Block destructive actions until identity is proven.

### 6. Host reboot policy

Define and test:

- requested versus observed state;
- which VMs are restarted after host boot;
- how manually stopped VMs remain stopped;
- service ordering;
- missing attachment behavior;
- missing or stale sockets/units;
- recovery event reporting.

Do not let a manager projection override local requested state without a versioned operation.

### 7. Database and migration safety

Prove:

- integrity checks before normal activation;
- corrupt database fails closed;
- original database and diagnostics are preserved;
- no empty replacement store is generated;
- interrupted migration blocks or recovers safely;
- backup/restore preserves host and VM identity.

### 8. Fault and leak qualification

Use representative failpoints for create, start, stop, delete, commit boundaries, observed-state updates, agent restart, and host reboot.

Run 100 lifecycle cycles and compare before/after inventories for:

- processes;
- systemd units;
- sockets;
- runtime directories;
- TAP ownership;
- storage mappings/files;
- database records.

## Acceptance criteria

- `CORE-INSTALL-001`: standalone `chv-agent` Core mode is healthy.
- `CORE-VM-001`: one qualified Linux VM runs through the native API.
- `CORE-ATTACH-001`: one pre-existing disk and network endpoint attach correctly.
- `CORE-IDEMP-001`: repeated requests create no duplicates.
- `CORE-RECOVERY-001`: killing `chv-agent` does not stop the VM; restart re-adopts it.
- `CORE-RECOVERY-002`: host reboot preserves identity and requested-state policy.
- `CORE-STORE-001`: corruption fails closed.
- `CORE-OPS-001`: crash after commit does not duplicate a VM.
- `CORE-LEAK-001`: 100 cycles leave no leaks.
- `CORE-AUTH-001`: conflicting ownership blocks destructive mutation.
- `VMM-ID-001` through `VMM-ID-004` pass.

## Forbidden outcomes

- a parallel runtime daemon;
- managed provider expansion;
- libvirt required for standalone lifecycle;
- manager or client access to Cloud Hypervisor sockets;
- automatically stopping an ambiguous process;
- automatically restarting a manually stopped VM;
- optimistic observed state without inspection;
- broad VMM features not required by the qualification tuple.

## Deliverables

- narrow VMM contract and reused adapter;
- real-KVM lifecycle implementation;
- recovery/re-adoption engine;
- host-reboot and database procedures;
- fault-injection and leak suite;
- support/unsupported matrix;
- upgrade and rollback instructions;
- complete qualification evidence.

## Exit gate

Phase C passes only when a disposable supported host can run the guest, survive agent death, survive host reboot according to policy, recover without identity loss, and complete leak/fault qualification without Controller or libvirt.
