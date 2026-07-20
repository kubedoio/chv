# Prompt 05 — Phase D: Provider Contracts and Privilege Hardening

Harden the minimum network, storage, and privileged-operation paths required by the first supported cloud integration. Preserve existing `chv-nwd` and `chv-stord` unless evidence supports a narrower change.

## Preconditions

- Phase C standalone lifecycle and recovery profiles pass.
- Phase A OpenStack discovery identifies the minimum network and storage paths needed by the selected candidate.
- Use branch `agent/cellhv-core-pd-providers`.

## Estimated effort

4–6 engineering weeks. Split network, storage, and privilege work into separate PRs where possible.

## Goal

Define stable attachment contracts and qualify the smallest provider set needed for OpenStack, without moving all Linux networking/storage code into `chv-agent` or redesigning every existing provider.

## Required work

### 1. Current provider audit

For `chv-nwd` and `chv-stord`, document:

- public/local APIs;
- privileged operations;
- ownership model;
- persistence and recovery behavior;
- idempotency;
- current supported paths;
- shell command or path-construction risks;
- cleanup and leak behavior;
- which operations are actually needed by the selected OpenStack path.

### 2. Network attachment contract

Define a narrow Core-facing contract covering only selected profiles, such as:

- consume pre-existing bridge/TAP;
- create or validate a managed endpoint only if required;
- attach/detach;
- inspect ownership and connectivity state;
- recover after agent/provider/host restart;
- clean up only owned resources.

Requirements:

- stable endpoint ID;
- explicit owner and lifecycle owner;
- MAC/TAP uniqueness;
- no modification of unrelated bridges, interfaces, namespaces, routes, or nftables tables;
- explicit unsupported VLAN/NAT/overlay behavior unless qualified;
- deterministic cleanup.

### 3. Storage attachment contract

Define a narrow contract for the selected profiles, such as:

- consume pre-existing raw file/block path;
- provision through one selected existing provider only if required;
- attach/detach;
- inspect ownership, lock, size, format, and availability;
- recover after restart;
- release only owned resources.

Requirements:

- stable attachment ID;
- data-integrity checks;
- exclusivity/locking where applicable;
- path traversal prevention;
- no deletion of in-use or unowned resources;
- explicit unsupported snapshot/migration behavior.

### 4. Privilege boundary

Use current daemon boundaries first. Introduce a new helper only if the audit proves it reduces privilege and does not duplicate `chv-nwd`/`chv-stord`.

Privileged APIs must be typed and allowlisted. Forbid:

- arbitrary shell commands;
- arbitrary filesystem paths;
- arbitrary systemd units;
- arbitrary nftables expressions;
- arbitrary LVM/RBD commands;
- caller-selected executable paths.

Use peer credentials, service identity, and filesystem permissions appropriate to Unix-socket callers.

### 5. Provider contract tests

Each advertised provider/path must run a reusable contract:

```text
validate -> prepare/consume -> attach -> inspect -> interrupt -> recover -> detach/release -> repeat cleanup -> leak check
```

Network tests include guest connectivity and unrelated-rule preservation.

Storage tests include guest write/read and data integrity.

Tests must cover provider daemon restart, `chv-agent` restart, and host reboot where advertised.

### 6. Observability and supportability

Add:

- structured operations/events;
- provider-specific error categories;
- bounded metrics without per-resource unbounded labels;
- ownership inventories;
- redacted diagnostics;
- operator recovery procedures.

## Acceptance criteria

- all advertised network-path contract tests pass at T2/T4;
- all advertised storage-path contract tests pass at T2/T4;
- `OS-003` and `OS-004` prerequisites are available for the selected OpenStack path;
- `AGENT-CORE-004`: no independent provider path can bypass agent/Core operation authority for VM attachment state;
- negative tests prove unrelated network/storage state is preserved;
- repeated detach/release is safe and idempotent;
- no arbitrary privileged command surface exists;
- provider restart and host reboot preserve or explicitly fail attachments according to the published profile.

## Forbidden outcomes

- implementing every planned provider;
- replacing `chv-nwd` and `chv-stord` without evidence and migration plan;
- moving broad privileged logic into `chv-agent`;
- assuming libvirt network/storage success from hypervisor URI success;
- adding Ceph cluster management;
- implementing migration, snapshots, distributed SDN, or backup scheduling unless required by the selected profile;
- deleting unowned or ambiguous resources.

## Deliverables

- provider audit;
- network and storage contracts;
- minimum provider hardening;
- privilege-boundary implementation and threat analysis;
- reusable provider contract tests;
- support/unsupported matrices;
- operator recovery documentation;
- rollback and evidence reports.

## Exit gate

Phase D passes when the exact network and storage paths required by the selected OpenStack candidate are independently qualified, recoverable, leak-free, and unable to mutate unrelated host resources.
