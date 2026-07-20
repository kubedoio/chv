# ADR-015: Evidence-Driven Ecosystem Integration Strategy

## Status

Proposed

## Date

2026-07-21

## Related decisions

- ADR-016: `chv-agent` evolves into CellHV Core.
- ADR-017: Core compatibility invariants are accepted and authoritative.

## Context

CellHV Core is intended to support management systems such as OpenStack, CellHV Controller, O3K, and later CloudStack and OpenNebula.

The major Linux cloud platforms have mature QEMU/KVM assumptions, usually through libvirt. The upstream libvirt Cloud Hypervisor driver exposes `ch:///system`, but upstream recognition does not prove that a cloud platform supports it. Networking and storage also have independent assumptions that cannot be solved by choosing a hypervisor URI.

The project needs an integration-selection process, but the actual paths remain unproven.

## Proposed strategy

1. OpenStack is the first external cloud-platform discovery and implementation target.
2. A time-boxed OpenStack discovery compares:
   - upstream Nova `LibvirtDriver` with `ch:///system`;
   - small generic Nova/libvirt generalisation;
   - an official CellHV Nova `ComputeDriver` using the native Core API.
3. The selected OpenStack path is based on real lab evidence, security, maintenance cost, upstream viability, and preservation of `chv-agent` Core authority.
4. `ch:///system` remains an optional bounded compatibility profile. It is not presumed to be the winning OpenStack path.
5. Platform-specific adapters are acceptable when they are safer and smaller than importing or emulating QEMU/libvirt semantics.
6. Network and storage mappings are selected and qualified independently from VM lifecycle.
7. CloudStack and OpenNebula remain strategic follow-on programmes after the Core authority and first OpenStack path are stable.
8. CellHV Controller and O3K use the native Core API because they are CellHV-controlled integrations.
9. Every supported platform path publishes exact versions, network/storage profiles, unsupported features, maintainer ownership, and evidence.

## Explicit non-decisions

This ADR does not yet decide:

- whether OpenStack uses generic libvirt, generic upstream changes, or a native driver;
- whether the bounded `ch:///system` profile becomes a supported product feature;
- the CloudStack integration path;
- the OpenNebula integration path;
- long-term libvirt network/storage coexistence;
- support for Kubernetes, Terraform, or Designer;
- any additional VMM backend.

## Rationale

One universal API or URI would reduce maintenance only if the upper platforms genuinely support its semantics. Implementing large compatibility surfaces before testing those assumptions creates more risk than a focused adapter.

The discovery-first approach produces evidence before implementation commitments and allows the project to choose the smallest maintainable path for each platform without weakening the accepted invariants in ADR-017.

## Consequences

### Positive

- OpenStack integration begins with evidence instead of speculation;
- broad cloud work no longer blocks Core authority and recovery;
- a native adapter is not treated as architectural failure;
- `ch:///system` can be evaluated without becoming mandatory;
- CloudStack and OpenNebula scope is deferred until the first path is stable.

### Negative

- the project cannot promise zero-change integration in advance;
- OpenStack may require a CellHV-maintained adapter;
- cloud-platform laboratories and maintenance ownership are required;
- different platforms may ultimately use different bridges.

## Required discovery evidence

The OpenStack discovery must record:

- exact platform, libvirt, Cloud Hypervisor, kernel, and guest versions;
- configuration required;
- first successful and failing actions;
- generated domain XML or API payloads;
- QEMU-specific assumptions;
- Neutron and Cinder expectations;
- effect on Core authority;
- generic upstream option;
- native-driver effort estimate;
- security and maintenance risk;
- recommended next step.

A connection or VM boot alone is not a support result.

## Acceptance conditions

This ADR may move to Accepted when:

- the time-boxed OpenStack discovery is complete;
- a focused follow-up ADR selects the first OpenStack path;
- network and storage requirements for that path are identified separately;
- maintenance ownership and version policy are named;
- no selected path bypasses `chv-agent` Core authority;
- the implementation and qualification schedule is resourced.

CloudStack and OpenNebula evidence is not required to accept the first OpenStack strategy.
