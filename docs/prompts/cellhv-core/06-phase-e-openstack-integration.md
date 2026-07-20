# Prompt 06 — Phase E: First Supported OpenStack Integration

Implement one evidence-selected OpenStack integration path. Do not begin until the Phase A discovery report and Phase D provider profiles are reviewed.

## Preconditions

- `OSD-001` through `OSD-005` have evidence.
- Phase C standalone/recovery profile passes.
- Phase D minimum network/storage paths pass independently.
- A focused ADR selects Path A, B, or C and names maintainers.
- Use branch `agent/cellhv-core-pe-openstack`.

## Estimated effort

6–10 engineering weeks depending on the selected path. Split implementation, networking, storage, and qualification into narrow PRs.

## Candidate paths

### Path A — generic libvirt `ch` path

Valid only if upstream Nova `LibvirtDriver` works through a bounded `ch:///system` profile without bypassing `chv-agent` Core authority.

### Path B — generic upstream changes

Valid only if changes are non-CellHV-specific, small, testable, and accepted or maintainable in the relevant upstream projects.

### Path C — official CellHV Nova driver

A bounded Nova `ComputeDriver` that uses the public `chv-agent` Core API and is maintained by CellHV.

Do not switch paths mid-implementation without updating the focused ADR and gap evidence.

## Goal

Reach a documented Preview or Supported OpenStack compatibility claim for one exact version tuple, one network path, one storage path, and one modern Linux workload profile.

## Required work

### 1. Adapter or generic-path implementation

Implement only the Nova operations required by the first profile:

- host capability and resource reporting;
- instance spawn;
- inspect/get-info;
- power on/off, reboot, and destroy;
- serial console where qualified;
- deterministic identity mapping;
- retry/idempotency mapping;
- nova-compute restart and projection rebuild.

Explicitly reject unqualified functionality such as:

- live migration;
- evacuation;
- resize/cold migration;
- snapshots;
- CPU/memory hotplug;
- SR-IOV and mediated devices;
- arbitrary libvirt/QEMU XML;
- legacy device models.

### 2. Image and boot path

Define and test:

- Glance image acquisition or cache ownership;
- checksum validation;
- supported image format;
- conversion rules, if any;
- firmware/cloud-init/config-drive behavior;
- cleanup and retry semantics;
- unsupported image behavior.

The adapter must not silently mutate or convert images outside the documented profile.

### 3. Neutron integration

Map one qualified Neutron VIF model into the Phase D network attachment contract.

Prove:

- correct MAC and endpoint identity;
- attach before VM boot where required;
- guest connectivity;
- agent/nova-compute restart recovery;
- detach/delete cleanup;
- no modification of unrelated host networking;
- explicit unsupported VIF types.

### 4. Cinder integration

Map one qualified Cinder block-device model into the Phase D storage attachment contract.

Prove:

- stable volume/attachment identity;
- attach/detach and data integrity;
- exclusivity/locking where applicable;
- retry and restart behavior;
- cleanup without deleting Cinder-owned data;
- explicit unsupported backend or feature behavior.

### 5. Authority and failure semantics

Every OpenStack mutation must enter the `chv-agent` Core operation engine.

Prove behavior for:

- duplicate Nova requests;
- timeout after Core accepted the operation;
- nova-compute crash/restart;
- OpenStack control-plane outage;
- `chv-agent` restart;
- host reboot;
- stale resource version;
- partially prepared network/storage;
- unsupported capability request.

Existing VMs must continue running during OpenStack management outage.

### 6. Capacity and state reporting

Report only accurate values for:

- vCPU capacity/usage;
- memory capacity/usage;
- supported architecture and firmware;
- supported disk/network capabilities;
- host disabled/unavailable state;
- VM power state.

Do not fabricate Placement traits or capabilities to satisfy scheduling.

### 7. Packaging and operations

Ship:

- installable integration package or configuration;
- exact version matrix;
- configuration guide;
- upgrade and rollback procedure;
- unsupported-feature matrix;
- troubleshooting and evidence collection;
- named maintenance ownership.

## Acceptance criteria

- `OS-001`: accurate host inventory and Placement reporting.
- `OS-002`: spawn, inspect, power, reboot, and destroy one instance.
- `OS-003`: Neutron mapping passes its independent network profile.
- `OS-004`: Cinder mapping passes its independent storage profile.
- `OS-005`: retries and nova-compute restart create no duplicate and stop no running VM.
- `OS-006`: versions, path, owner, and unsupported features are published.
- `OS-007`: rejected-path and QEMU-assumption evidence remains available.
- `OS-008`: no OpenStack path bypasses Core authority.
- `AGENT-CORE-005`: OpenStack removal/outage does not stop workloads or erase identity.
- compatibility claim validates under the claim contract.

## Forbidden outcomes

- claiming all OpenStack versions;
- exposing Cloud Hypervisor as QEMU;
- broadening Core domain types for Nova internals;
- direct adapter access to Core database, VMM sockets, `chv-nwd`, or `chv-stord` private APIs;
- treating a successful spawn as proof of Neutron/Cinder support;
- implementing migration/snapshot/evacuation without separate profiles;
- changing to CloudStack/OpenNebula scope in this phase.

## Deliverables

- focused integration ADR;
- implementation and real-platform tests;
- Neutron/Cinder mapping documents;
- package and operator documentation;
- exact compatibility claim tuple;
- upgrade/rollback evidence;
- residual-risk and maintenance report.

## Exit gate

Phase E passes when one exact OpenStack version tuple reaches Preview or Supported status under the compatibility-claims contract and all OS-001 through OS-008 scenarios pass on a real lab.
