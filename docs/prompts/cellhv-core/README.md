# CellHV Core Phased Implementation Prompts

These prompts turn the CellHV Core specifications into bounded implementation handoffs for coding agents and human contributors.

## Source of truth

Before executing any phase, read and obey:

- `docs/specs/cellhv-core-foundation-spec.md`
- `docs/specs/adr/015-layered-ecosystem-compatibility.md`
- `docs/specs/cellhv-core-api-cloud-integration-spec.md`
- `docs/specs/cellhv-core-acceptance-test-spec.md`
- `docs/specs/contracts/cellhv-compatibility-claims-v1.md`
- `docs/specs/contracts/cellhv-libvirt-compatibility-profile-v1.md`
- `docs/plans/2026-07-19-cellhv-core-foundation-implementation.md`
- `docs/plans/2026-07-19-cellhv-core-test-harness-implementation.md`

If a prompt conflicts with an accepted ADR or contract, the ADR or contract wins. Stop and report the contradiction rather than inventing a resolution.

## Execution order

1. [`00-execution-policy.md`](00-execution-policy.md)
2. [`01-phase-0-baseline-and-architecture-guards.md`](01-phase-0-baseline-and-architecture-guards.md)
3. [`02-phase-1-core-domain-state-and-api.md`](02-phase-1-core-domain-state-and-api.md)
4. [`03-phase-2-minimal-cloud-hypervisor-runtime.md`](03-phase-2-minimal-cloud-hypervisor-runtime.md)
5. [`04-phase-3-recovery-and-single-authority.md`](04-phase-3-recovery-and-single-authority.md)
6. [`05-phase-4-network-and-storage-contracts.md`](05-phase-4-network-and-storage-contracts.md)
7. [`06-phase-5-privileged-helper-and-standard-providers.md`](06-phase-5-privileged-helper-and-standard-providers.md)
8. [`07-phase-6-compatibility-discovery.md`](07-phase-6-compatibility-discovery.md)
9. [`08-phase-7-openstack-integration.md`](08-phase-7-openstack-integration.md)
10. [`09-phase-8-cloudstack-and-opennebula.md`](09-phase-8-cloudstack-and-opennebula.md)
11. [`10-phase-9-managed-endpoint-controller-and-o3k.md`](10-phase-9-managed-endpoint-controller-and-o3k.md)
12. [`11-phase-10-release-qualification.md`](11-phase-10-release-qualification.md)

## Mandatory workflow

- Run one phase per branch and pull request.
- Do not begin a phase until the previous phase's exit evidence is committed and reviewed.
- Every implementation PR must name the affected acceptance IDs.
- Every implementation PR must include rollback instructions.
- Mock tests may prove contracts but never infrastructure or compatibility claims.
- Cloud Hypervisor must never be advertised as QEMU.
- Network, storage, VMM, and cloud-platform compatibility are separate claims.
- A platform adapter remains outside Core and uses only public Core contracts.

## Branch naming

Use:

```text
agent/cellhv-core-p00-baseline
agent/cellhv-core-p01-state-api
agent/cellhv-core-p02-runtime
agent/cellhv-core-p03-recovery
agent/cellhv-core-p04-attachments
agent/cellhv-core-p05-providers
agent/cellhv-core-p06-compatibility-discovery
agent/cellhv-core-p07-openstack
agent/cellhv-core-p08-cloud-platforms
agent/cellhv-core-p09-managed-endpoint
agent/cellhv-core-p10-qualification
```

## Completion rule

A phase is complete only when:

- implementation, tests, documentation, and evidence are present;
- all phase acceptance scenarios pass at the required tier;
- unsupported behavior is explicit;
- no architectural boundary has been weakened;
- the PR contains a concise residual-risk section.
