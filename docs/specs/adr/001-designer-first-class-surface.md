# ADR-001-Designer Introduce Architecture Designer as a First-Class CHV Surface

Date: 2026-06-13
Status: Proposed

## Context

CHV currently has a fleet/resource management WebUI with a left-side navigation and live resource views. The next product requirement is to let users design complete infrastructure topologies before creating resources.

The required capability is broader than a VM creation wizard. It must support designing servers/hosts, networks, datastores, images, templates, instances, users and permissions, then exporting the design as YAML or deploying it directly from the WebUI.

## Decision

Introduce a new top-level WebUI section called **Architecture Designer**.

It must appear above Fleet Overview in the left panel under a new DESIGN group.

```text
DESIGN
  Architecture Designer
  Saved Topologies

Fleet Overview

INFRASTRUCTURE
  Default Cloud
    Hosts
    Instances
    Networks
    Storage
    Images
```

The Architecture Designer is the design-time view. Fleet Overview remains the current-state operational view.

## Rationale

The user starts from intent: "I want this architecture." The WebUI must therefore provide a design surface before operational resource lists.

Putting the designer above Fleet Overview makes the user workflow explicit:

1. design desired topology
2. validate/check/plan
3. deploy
4. observe actual fleet

## Consequences

- Add new routes: `/architectures`, `/architectures/new`, `/architectures/{id}`, `/architectures/{id}/plan`, `/architectures/{id}/runs`.
- Add a new first-class object: `ArchitectureTopology`.
- Add dashboard cards for saved topologies.
- Current resources and desired topologies must be intentionally separated.

## Non-goals

- Do not hide this under Settings.
- Do not make it only a YAML editor.
- Do not make it only a generic visual diagram without executable contract.
