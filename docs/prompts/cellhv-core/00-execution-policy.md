# Prompt 00 — CellHV Core Execution Policy

You are evolving `chv-agent` into CellHV Core in `kubedoio/chv`.

This prompt is mandatory context for every later prompt. Do not implement product code while executing it. Review the repository and produce a short execution declaration in the implementation PR.

## Mandatory reading

Read all files listed in `docs/prompts/cellhv-core/README.md` and inspect:

- workspace crates and dependency graph;
- accepted and proposed ADRs;
- current CI workflows;
- current `chv-agent`, Cloud Hypervisor, `chv-stord`, `chv-nwd`, and control-plane boundaries;
- current VM process and state ownership;
- current test infrastructure and available real labs.

## Non-negotiable rules

1. `chv-agent` and CellHV Core are the same runtime authority.
2. Do not create `cellhvd` or another parallel node-runtime daemon.
3. Core works without Controller, libvirt, OpenStack, CloudStack, OpenNebula, O3K, Kubernetes, Designer, Web UI, or an external database.
4. Every legacy and native mutation enters one durable operation engine.
5. Existing workloads continue running during management-plane loss.
6. Ambiguous process ownership fails closed for destructive operations.
7. Cloud Hypervisor is the only active Core 1.0 VMM target.
8. Cloud Hypervisor must never be exposed as `qemu:///system` or with unsupported QEMU/QMP semantics.
9. Network, storage, VMM, and platform compatibility are independent claims.
10. Platform-specific adapters remain outside `chv-agent` Core and use public contracts.
11. Other VMM backends, CloudStack, OpenNebula, Kubernetes, Terraform, and Designer are outside the active prompt pack.

## Engineering rules

- Work in a dedicated branch; never implement a phase directly on `main`.
- Keep PRs narrow. A phase may require several PRs.
- Evolve current code before creating replacement crates or services.
- Do not perform a flag-day rename or rewrite.
- Preserve current control-plane compatibility until the replacement path is proven.
- Prefer typed Linux interfaces over shell parsing where practical.
- Never add a generic privileged command executor.
- Never silently accept unsupported API fields, devices, storage types, networks, or domain XML.
- Do not fabricate capabilities, counters, or compatibility claims.
- Keep tenant, project, quota, billing, scheduler, global IPAM, and platform models outside Core.
- Add structured errors, tracing, metrics, migrations, tests, and rollback with each subsystem.

## Evidence-over-specification rule

Specifications guide implementation, but provisional cloud-integration decisions must be updated from real evidence.

- A connection test is not compatibility.
- A mock is not infrastructure qualification.
- When implementation evidence contradicts a proposed spec, stop, document the evidence, and propose a focused spec/ADR change.
- Do not add code merely to satisfy an unsupported speculative contract.

## Test rules

- T0/T1 prove schemas, boundaries, and state machines.
- T2 proves Linux service, persistence, and provider contracts.
- T3 on real KVM proves lifecycle and recovery.
- T5 on real OpenStack proves OpenStack behavior.
- Lower tiers cannot satisfy higher-tier claims.
- Assert forbidden outcomes and cleanup, not only final success.
- Destructive tests must prove disposable isolation.

## Required PR declaration

```markdown
## CellHV Core implementation declaration

- Phase and slice:
- Existing `chv-agent` code being evolved:
- Runtime authority impact:
- VMM backend:
- Network path:
- Storage path:
- Platform path, if any:
- Acceptance IDs:
- Explicit non-scope:
- Migration/rollback:
- Evidence location:
- Residual risks:
- Estimated effort and owner:
```

## Stop conditions

Stop and request a decision when:

- the task creates a second runtime authority;
- the task requires Cloud Hypervisor to impersonate QEMU;
- a public API or durable schema cannot remain compatible;
- a privileged operation cannot be bounded and validated;
- platform concepts would enter Core;
- behavior cannot be tested at the required tier;
- the task pulls a deferred programme into active scope;
- specifications contradict ADR-016 or the single-authority rule;
- available people or lab capacity is insufficient for the required evidence.

## Output

Return an execution declaration confirming:

- source documents read;
- branch and PR slice;
- exact scope and non-scope;
- current code to reuse;
- tests and evidence;
- owner and effort estimate;
- contradictions or unresolved decisions.
