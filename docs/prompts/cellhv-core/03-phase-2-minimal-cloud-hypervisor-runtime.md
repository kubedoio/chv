# Prompt 03 — Phase 2: Minimal Cloud Hypervisor Runtime

Implement CellHV Core M1: one real Linux VM on one real KVM host through the native Core API.

## Preconditions

- Phase 1 is merged and its API/store/operation tests pass.
- The qualification tuple from Phase 0 names the supported host, kernel, Cloud Hypervisor, firmware, and guest image.
- Use branch `agent/cellhv-core-p02-runtime`.

## Goal

Connect the durable operation engine to a narrow Cloud Hypervisor VMM adapter and prove a minimal standalone VM lifecycle using only pre-existing network and storage endpoints.

## Required work

### 1. VMM adapter contract

Define a platform-neutral VMM interface owned by Core. It should include only capabilities required by the current phase:

- validate host/runtime availability;
- prepare VM runtime definition;
- create process/unit;
- start;
- inspect;
- graceful stop;
- force stop;
- reboot;
- pause/resume only if fully supported and tested;
- delete runtime artifacts;
- retrieve serial-console endpoint;
- enumerate/adopt existing runtime processes.

The interface must not expose QMP, QEMU command lines, arbitrary Cloud Hypervisor JSON, or cloud-platform types.

### 2. Cloud Hypervisor adapter

Extract and adapt reusable code from `chv-agent-runtime-ch` rather than rewriting working behavior blindly.

Requirements:

- pinned Cloud Hypervisor binary/version validation;
- truthful capability reporting;
- one process per VM;
- deterministic runtime directory and API socket layout;
- stable ownership markers linked to Core VM UUID;
- safe command/config construction without shell interpolation;
- structured error translation;
- serial console support for the qualified guest;
- explicit cleanup behavior;
- no QEMU identity or QMP compatibility.

### 3. Process supervision

Use the selected systemd strategy from an ADR or documented candidate decision.

Prove:

- `cellhvd` is not the parent whose death kills the VM;
- VM CPU/memory accounting is visible through cgroups/systemd;
- unit/process names are derived safely from stable IDs;
- unrelated units cannot be controlled;
- logs are available through structured application logs and/or journald;
- restart policy does not override explicit operator intent.

### 4. Minimal attachments

Support only:

- one pre-existing raw file or block device;
- one pre-existing Linux bridge or TAP endpoint;
- virtio block and virtio net;
- serial console;
- one qualified firmware or direct-kernel boot path.

Core records attachment identity and ownership but does not provision LVM, bridges, VLANs, NAT, RBD, Neutron, Cinder, or cloud networks in this phase.

### 5. Execute durable operations

Wire create/start/stop/reboot/delete operations through:

```text
validate -> commit operation -> prepare -> perform VMM action -> inspect -> persist observed state -> emit event
```

Requirements:

- every host action corresponds to a persisted operation step;
- retries cannot launch duplicate VMs;
- inspection, not optimistic assumption, determines observed state;
- timeout and interruption are explicit;
- rollback/compensation is bounded and safe;
- unsupported pause/resume behavior is not advertised.

### 6. Real KVM acceptance fixture

Add a safe real-host test path using:

- disposable qualified host;
- pinned guest image and checksum;
- cloud-init fixture;
- reserved VM IDs, paths, and interfaces;
- host safety marker;
- cleanup/leak inventory;
- evidence capture.

Do not make mock tests satisfy T3 scenarios.

## Acceptance criteria

- `CORE-INSTALL-001` standalone installation and healthy local API.
- `CORE-VM-001` create and start one qualified Linux VM through native API.
- `CORE-ATTACH-001` VM uses one pre-existing disk and one pre-existing network endpoint.
- `CORE-IDEMP-001` repeated lifecycle requests do not duplicate VM processes, units, sockets, or records.
- `VMM-ID-001` Cloud Hypervisor is never exposed as QEMU.
- `VMM-ID-002` reported VMM/version/capabilities match the actual process.
- `VMM-ID-003` unsupported QEMU/QMP behavior fails explicitly.
- `VMM-ID-004` process, unit, socket, and ownership are auditable.
- Guest reaches expected boot-complete signal and serial console.
- Cleanup leaves no leaked process, unit, socket, runtime directory, TAP ownership, or temporary file.

## Forbidden outcomes

- managed networking or storage provisioning;
- libvirt dependency for standalone lifecycle;
- direct VMM access from Controller or external clients;
- shell-built unvalidated command lines;
- fabricated guest-ready status;
- automatic restart of a VM explicitly requested to remain stopped;
- accepting unsupported devices and silently dropping them;
- adding broad VMM abstractions not required by the current profile.

## Deliverables

- VMM adapter contract;
- Cloud Hypervisor adapter;
- process-supervision integration;
- minimal attachment handling;
- real-KVM scenario and evidence collection;
- support matrix and unsupported list;
- migration notes from existing agent runtime;
- rollback instructions.

## Exit gate

Phase 2 passes only when a clean supported Linux host can install Core, create and run one qualified guest through the native API, stop/start/delete it idempotently, and clean up without requiring any management plane.
