# CHV Architecture Designer

> **Status:** Implemented (Phases 0–7 shipped 2026-06; **GO disposition recorded** 2026-06-16). The ADRs in this section are `Accepted`. See [`docs/plans/2026-06-13-architecture-designer-implementation-plan.md`](../../plans/2026-06-13-architecture-designer-implementation-plan.md) for the phased implementation history, [`docs/release/architecture-designer-release-notes.md`](../../release/architecture-designer-release-notes.md) for the consolidated release notes, and [`go-no-go-2026-06-16.md`](go-no-go-2026-06-16.md) for the recorded ship-gate decision.

Date: 2026-06-13
Project: CHV / CellHV

## Purpose

The Architecture Designer is a planned WebUI feature that adds a **design-time** topology editor above the current Fleet Overview. Users will be able to visually compose infrastructure topologies (hosts, networks, datastores, images, templates, instances, users, roles, backup policies), save them, export them as YAML, validate them against the current fleet, generate a plan, and deploy the topology through the existing CHV task system.

The feature is intentionally narrower than a generic TOSCA/Cloudify engine — see [ADR-006-Designer](../adr/006-designer-no-tosca-engine.md).

## Document map

```text
docs/
├── decisions/README.md           # ADR index (Architecture Designer ADRs section)
├── specs/
│   ├── adr/                      # 6 Architecture Designer ADRs (001-Designer..006-Designer)
│   ├── architecture-designer/    # ← THIS DIRECTORY: contracts, prompts, research, this README
│   │   ├── README.md             # ← you are here
│   │   ├── contracts/            # YAML, graph, API, validation/plan-result contracts
│   │   ├── prompts/              # Coding-agent prompts (phased implementation guidance)
│   │   └── research-notes.md     # Source-of-decisions research notes
│   └── component/                # Binding component specs (data-model, reconciler, UI, validation, security)
├── plans/
│   └── 2026-06-13-architecture-designer-roadmap.md   # Phased implementation roadmap
├── examples/
│   ├── chvarchitecture-example.yaml
│   └── architecture-plan-result-example.yaml
└── schemas/
    └── chvarchitecture-v1alpha1.schema.json          # JSON Schema for the YAML contract
```

## ADRs (Accepted)

- [ADR-001-Designer](../adr/001-designer-first-class-surface.md) — Architecture Designer as a first-class CHV surface
- [ADR-002-Designer](../adr/002-designer-svelte-flow.md) — Svelte Flow for the editable canvas
- [ADR-003-Designer](../adr/003-designer-yaml-source-of-truth.md) — `CHVArchitecture` YAML as the source of truth
- [ADR-004-Designer](../adr/004-designer-validation-plan-apply.md) — Validation-, plan-, and task-gated deployment
- [ADR-005-Designer](../adr/005-designer-separate-desired-vs-live.md) — Separate desired-topology designer from live-fleet topology
- [ADR-006-Designer](../adr/006-designer-no-tosca-engine.md) — No generic Cloudify/TOSCA engine in MVP

## Component specs

- [`component/architecture-designer-data-model.md`](../component/architecture-designer-data-model.md)
- [`component/architecture-designer-reconciler.md`](../component/architecture-designer-reconciler.md)
- [`component/architecture-designer-ui.md`](../component/architecture-designer-ui.md)
- [`component/architecture-designer-validation.md`](../component/architecture-designer-validation.md)
- [`component/architecture-designer-security.md`](../component/architecture-designer-security.md)

## Contracts

- [`contracts/yaml-contract.md`](contracts/yaml-contract.md) — `CHVArchitecture` YAML structure and rules
- [`contracts/graph-contract.md`](contracts/graph-contract.md) — Topology graph model (nodes/edges)
- [`contracts/api-contract.md`](contracts/api-contract.md) — Designer BFF API surface
- [`contracts/validation-plan-contract.md`](contracts/validation-plan-contract.md) — Validation result + plan result formats

## Examples and schema

- [`docs/examples/chvarchitecture-example.yaml`](../../examples/chvarchitecture-example.yaml)
- [`docs/examples/architecture-plan-result-example.yaml`](../../examples/architecture-plan-result-example.yaml)
- [`docs/schemas/chvarchitecture-v1alpha1.schema.json`](../../schemas/chvarchitecture-v1alpha1.schema.json)

## Coding-agent prompts (phased)

The seven [`prompts/`](prompts/) files describe the phased implementation steps for an automated coding agent. Each prompt is self-contained and assumes the previous phase has landed.

## Implementation roadmap

See [`docs/plans/2026-06-13-architecture-designer-roadmap.md`](../../plans/2026-06-13-architecture-designer-roadmap.md) for the phased implementation plan.

## Core principle

CHV must not become a generic Cloudify/TOSCA clone. The Designer borrows the useful pattern from topology-oriented orchestrators but stays scoped to CHV-native virtualization resources: hosts, networks, datastores, images, templates, instances, users, roles, permissions, backup targets, backup policies.

## Non-goals for the first implementation

- Generic Cloudify/TOSCA DSL compatibility
- Generic Terraform/Ansible execution engine
- Full visual SDN designer
- Billing or quota product
- Multi-cluster federation
- GPU passthrough designer
- Live migration designer
- Bare-metal provisioning

## Success definition

A user can create a topology in the WebUI, save it, export the generated YAML, run validation, compare against current CHV resources, preview the plan, deploy it through task-gated execution, and later detect drift.

## Terminology

The Designer uses the same operator-facing terminology as the rest of the WebUI: **Default Cloud → Hosts → Instances** (per ADR-006-WebUI). The YAML contract uses `servers` for the host list and `instances` for the VM list.
