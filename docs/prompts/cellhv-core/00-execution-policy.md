# Prompt 00 — CellHV Core Execution Policy

You are implementing CellHV Core in `kubedoio/chv`.

This prompt is mandatory context for every later phase. Do not write product code while executing this prompt. Review the repository, confirm the operating rules, and produce a short execution declaration in the phase PR.

## Mandatory reading

Read all files listed in `docs/prompts/cellhv-core/README.md` and inspect:

- workspace crates and dependency graph;
- existing ADRs;
- current CI workflows;
- current agent, runtime, storage, network, and control-plane boundaries;
- current Cloud Hypervisor process ownership;
- current test infrastructure.

## Non-negotiable product rules

1. CellHV Core works on one Linux host without Controller, libvirt, OpenStack, CloudStack, OpenNebula, O3K, Kubernetes, Designer, Web UI, or an external database.
2. Core is the only mutation authority for CellHV-managed VMs.
3. Every mutation is represented by a durable operation before host-side effects occur.
4. Existing workloads continue running during management-plane loss.
5. Ambiguous process ownership fails closed for destructive operations.
6. Cloud Hypervisor is the primary Core 1.0 VMM.
7. Cloud Hypervisor must never be exposed as `qemu:///system` or advertised with unsupported QEMU/QMP semantics.
8. Network, storage, VMM, and cloud-platform compatibility are independent profiles.
9. Platform-specific adapters remain outside Core and use public Core APIs.
10. A future actual QEMU backend requires a separate ADR and is not part of these phases.

## Engineering rules

- Work in a dedicated branch; never implement directly on `main`.
- Keep one phase per PR.
- Preserve working existing paths behind compatibility adapters until replacement behavior is proven.
- Do not perform a flag-day rename or repository-wide rewrite.
- Prefer typed Linux interfaces over shell parsing where practical.
- Never add a generic privileged command executor.
- Never silently accept unsupported API fields, domain XML, devices, storage types, or network semantics.
- Do not fabricate capabilities, counters, or compatibility claims.
- Keep Core free of tenant, project, quota, billing, scheduler, global IPAM, or cloud-platform-specific models.
- Do not call private Core database APIs from integrations.
- Add structured errors, tracing, metrics, migrations, and rollback with each new subsystem.

## Test rules

- T0/T1 tests prove schemas, boundaries, and state machines.
- T2 tests prove privileged Linux integration and persistence.
- T3 tests on real KVM prove VM lifecycle and recovery.
- T5 tests on real cloud platforms prove platform compatibility.
- Lower-tier tests cannot satisfy higher-tier claims.
- Each test must assert forbidden outcomes and cleanup, not only the final success state.
- Destructive tests must prove they are running on disposable infrastructure.

## Required PR declaration

Every phase PR description must include:

```markdown
## CellHV Core phase declaration

- Phase:
- VMM backend:
- Core authority affected:
- Network path:
- Storage path:
- Platform integration path:
- Acceptance IDs:
- Unsupported behavior:
- Migration/rollback:
- Evidence location:
- Residual risks:
```

## Stop conditions

Stop and request an ADR or maintainer decision when:

- the task would make another component authoritative for VM state;
- the task requires Cloud Hypervisor to impersonate QEMU;
- a public API or durable schema cannot remain backward compatible;
- a privileged operation cannot be bounded and validated;
- the task introduces platform concepts into Core;
- an advertised behavior cannot be tested at the required tier;
- the current specifications contradict each other.

## Output

Return a short execution declaration confirming:

- the source-of-truth documents read;
- the phase branch that will be used;
- the exact scope and explicit non-scope;
- the tests and evidence required;
- any contradiction or unresolved ADR discovered.
