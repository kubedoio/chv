# CellHV Core Phased Implementation Prompts

These prompts turn the CellHV Core decisions into bounded implementation handoffs.

## Critical identity rule

`chv-agent` is the CellHV Core runtime implementation.

- Evolve `chv-agent` in place.
- Do not create a parallel `cellhvd` binary or service.
- Keep the existing binary/service name until a separate naming ADR is accepted.
- Legacy control-plane gRPC and the native local API must enter one operation engine and one durable store.

See ADR-016.

## Product framing

CellHV Core is a **self-contained compute runtime with optional ecosystem bridges**. It is not loosely coupled from Linux, KVM, Cloud Hypervisor, or its selected provider contracts. It is independent from any mandatory upper management plane.

## Source of truth

Before executing any phase, read:

- `docs/specs/adr/016-evolve-chv-agent-into-cellhv-core.md`
- `docs/specs/adr/015-layered-ecosystem-compatibility.md`
- `docs/specs/cellhv-core-foundation-spec.md`
- `docs/specs/cellhv-core-api-cloud-integration-spec.md`
- `docs/specs/cellhv-core-acceptance-test-spec.md`
- `docs/specs/contracts/cellhv-compatibility-claims-v1.md`
- `docs/specs/contracts/cellhv-libvirt-compatibility-profile-v1.md`
- `docs/plans/2026-07-19-cellhv-core-foundation-implementation.md`
- `docs/plans/2026-07-19-cellhv-core-test-harness-implementation.md`

ADR-016 and the Core-authority, truthful-VMM-identity, and multi-axis compatibility rules are locked. Cloud-platform integration choices remain provisional until discovery evidence exists.

## Active execution order

1. [`00-execution-policy.md`](00-execution-policy.md)
2. [`01-phase-a-baseline-agent-migration.md`](01-phase-a-baseline-agent-migration.md)
3. [`02-phase-a-openstack-discovery.md`](02-phase-a-openstack-discovery.md)
4. [`03-phase-b-agent-local-authority.md`](03-phase-b-agent-local-authority.md)
5. [`04-phase-c-standalone-runtime-and-recovery.md`](04-phase-c-standalone-runtime-and-recovery.md)
6. [`05-phase-d-provider-and-privilege-hardening.md`](05-phase-d-provider-and-privilege-hardening.md)
7. [`06-phase-e-openstack-integration.md`](06-phase-e-openstack-integration.md)
8. [`07-phase-f-controller-o3k-and-qualification.md`](07-phase-f-controller-o3k-and-qualification.md)

CloudStack, OpenNebula, broad libvirt productisation, additional VMMs, Kubernetes, Terraform, and Designer receive separate prompt packs after Core authority and the first OpenStack path are stable.

## Planning assumptions

These are planning estimates, not promises.

Minimum capacity:

- one dedicated senior Rust/Linux virtualization engineer;
- half-time infrastructure/test engineer;
- disposable KVM and OpenStack labs;
- regular architecture review.

Indicative sequence:

| Period | Target |
|---|---|
| Q3 2026 | Phase A and start Phase B |
| Q4 2026 | complete Phase B; begin Phase C |
| Q1 2027 | complete Phase C and Phase D |
| Q2 2027 | Phase E OpenStack integration |
| Q3 2027 | Phase F and Core 1.0 qualification |

With less capacity, extend the timeline rather than weakening recovery or test gates.

## Mandatory workflow

- Each prompt runs on a dedicated branch and PR.
- A phase may use multiple narrow PRs; do not bundle unrelated decisions.
- Verify the previous prompt's evidence before coding.
- Every PR names acceptance IDs, explicit non-scope, rollback, evidence, and residual risk.
- Update provisional specifications only when implementation evidence changes the decision.
- Mock tests do not prove KVM, provider, or cloud-platform behavior.
- Never infer OpenStack support from `ch:///system` connection success.
- Never expose Cloud Hypervisor as QEMU.

## Branch naming

```text
agent/cellhv-core-pa-baseline
agent/cellhv-core-pa-openstack-discovery
agent/cellhv-core-pb-local-authority
agent/cellhv-core-pc-runtime-recovery
agent/cellhv-core-pd-providers
agent/cellhv-core-pe-openstack
agent/cellhv-core-pf-qualification
```

## Completion rule

A prompt is complete only when:

- code, tests, documentation, and evidence are committed;
- required acceptance scenarios pass at the stated tier;
- unsupported behavior is explicit;
- `chv-agent` remains the single runtime authority;
- the PR contains a residual-risk section;
- no future programme was pulled into the current scope.
