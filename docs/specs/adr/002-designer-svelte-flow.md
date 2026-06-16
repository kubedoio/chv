# ADR-002-Designer Use Svelte Flow for the Editable Designer Canvas

Date: 2026-06-13
Status: Accepted (2026-06-16, Phase 7)

## Context

CHV already has a custom topology canvas for live visualization. However, an editable designer requires drag/drop, custom nodes, connectable handles, edge validation, minimap, controls, panels, selection, graph serialization and two-way synchronization with a YAML model.

Extending the current custom SVG topology viewer into a full topology editor would create high maintenance cost.

## Decision

Use **Svelte Flow** for the Architecture Designer canvas.

Keep the existing custom CHV topology canvas for live/current-state topology views.

## Rationale

Svelte Flow matches CHV's Svelte WebUI stack and already provides primitives for node-based editors:

- draggable nodes
- zoom and pan
- selectable nodes/edges
- add/remove edges
- custom Svelte nodes
- minimap
- controls
- panels
- connection validation examples
- drag/drop examples
- layout/subflow support

## Consequences

Create a new component namespace:

```text
ui/src/lib/components/architectures/
  ArchitectureDesignerPage.svelte
  ArchitectureDesignerCanvas.svelte
  ArchitectureNodeHost.svelte
  ArchitectureNodeNetwork.svelte
  ArchitectureNodeDatastore.svelte
  ArchitectureNodeImage.svelte
  ArchitectureNodeTemplate.svelte
  ArchitectureNodeInstance.svelte
  ArchitectureInspectorPanel.svelte
  ArchitectureValidationPanel.svelte
  ArchitectureYamlPanel.svelte
  ArchitecturePlanPanel.svelte
```

## Alternatives considered

### Extend current TopologyCanvas

Rejected for editor use. Keep it for live topology and drift visualization.

### React Flow

Rejected for MVP because CHV is Svelte-based.

### Rete.js

Rejected for MVP because it is more visual-programming/dataflow-oriented.

### JointJS/jsPlumb

Deferred. Could be revisited if Svelte Flow cannot satisfy enterprise graph editing requirements.
