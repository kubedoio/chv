# Prompt 02 — Phase A2: Time-Boxed OpenStack Compatibility Discovery

Run the OpenStack discovery before committing to a libvirt or native Nova integration architecture.

## Goal

Produce real evidence about whether upstream Nova `LibvirtDriver` plus `ch:///system` can manage Cloud Hypervisor, what QEMU assumptions block it, and whether a native CellHV Nova driver is the smaller sustainable path.

This is a discovery task, not an OpenStack support implementation.

## Branch

`agent/cellhv-core-pa-openstack-discovery`

## Time box

Maximum 5 engineering days. Stop when the time box expires and report incomplete areas honestly.

## Environment

Use a disposable, version-recorded DevStack or equivalent standalone OpenStack lab with:

- Nova compute;
- Placement;
- minimal Neutron networking;
- optional Cinder only after basic compute discovery;
- upstream libvirt with the Cloud Hypervisor driver enabled;
- pinned Cloud Hypervisor binary;
- disposable Linux cloud image;
- no production credentials or infrastructure.

## Candidate paths

### Path A — unchanged Nova libvirt path

Configure Nova to use the upstream libvirt Cloud Hypervisor URI where possible.

Record:

- configuration required;
- connection success/failure;
- reported hypervisor type and capabilities;
- generated domain XML;
- first failing Nova/libvirt operation;
- QEMU-specific code or capability assumption;
- process ownership and whether CellHV Core can remain authoritative;
- networking and storage expectations encountered.

Do not patch around multiple failures merely to make a demo pass.

### Path B — generic upstream generalisation

For each Path A blocker, assess whether a small non-CellHV-specific change to Nova or libvirt could support non-QEMU libvirt backends.

Record:

- exact project and code location;
- proposed generic behavior;
- upstream maintenance plausibility;
- security and compatibility risk;
- likely test burden;
- whether it preserves `chv-agent` Core authority.

Do not implement broad upstream changes in this discovery PR.

### Path C — native CellHV Nova driver

Produce a bounded engineering estimate for a Nova `ComputeDriver` using the native `chv-agent` Core API.

Map at least:

- host capability/resource reporting;
- spawn and destroy;
- power operations and `get_info`;
- image preparation;
- Neutron VIF attachment;
- Cinder block attachment;
- console;
- retry/idempotency;
- nova-compute restart;
- unsupported migration, resize, snapshot, evacuation, and passthrough behavior.

Identify what can be reused from existing Nova drivers and what CellHV must maintain.

## Required evidence

Create a structured gap report containing:

```yaml
openstack_version:
nova_version:
libvirt_version:
cloud_hypervisor_version:
host_kernel:
candidate:
configuration:
first_success:
first_failure:
libvirt_api_or_xml:
qemu_specific_assumption:
network_expectation:
storage_expectation:
core_authority_impact:
generic_upstream_option:
native_driver_effort:
security_risk:
maintenance_risk:
result:
recommended_next_step:
```

Attach redacted logs, relevant XML/API payloads, configuration, and exact source references.

## Acceptance criteria

- `OSD-001`: Nova reaches the candidate libvirt connection or records the exact first blocker.
- `OSD-002`: QEMU-specific assumptions are catalogued with source/config evidence.
- `OSD-003`: Neutron and Cinder expectations are separated from VM lifecycle.
- `OSD-004`: native driver effort and maintenance cost are estimated.
- `OSD-005`: Path A, B, or C recommendation is evidence-based.
- No path reports Cloud Hypervisor as QEMU.
- No discovery patch bypasses the future `chv-agent` operation authority.

## Decision rules

Recommend Path A only if it works without CellHV-specific platform code and does not bypass Core authority.

Recommend Path B only if the required changes are generic, small, testable, and plausibly maintainable upstream.

Recommend Path C when a bounded native driver is safer and smaller than reproducing QEMU/libvirt semantics.

It is acceptable to recommend another focused discovery step. It is not acceptable to declare OpenStack support.

## Explicit non-scope

- production Nova driver;
- production libvirt delegation layer;
- complete Neutron/Cinder integration;
- migration or snapshot support;
- CloudStack/OpenNebula work;
- changes to Core runtime behavior.

## Deliverables

- reproducible lab instructions;
- candidate results and evidence;
- structured gap report;
- recommended path with rejected alternatives;
- owner and effort range for Phase E;
- proposed focused ADR only if evidence supports a decision.
