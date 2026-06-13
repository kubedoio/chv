# Research Notes: GUI Topology Designer Alternatives

## Summary

The strongest practical GUI option for CHV is **Svelte Flow**, because the CHV WebUI is already Svelte-based and the required designer is a node/edge topology editor, not a generic workflow engine.

## Cloudify reference

Cloudify is conceptually close because it uses YAML blueprints to describe logical infrastructure/application topology. Its model includes blueprint sections such as inputs, node templates, relationships, workflows and outputs. This confirms that the topology -> blueprint -> deployment pattern is mature, but CHV should not adopt Cloudify DSL/TOSCA directly.

Decision for CHV:

- Borrow: blueprint concept, topology relationships, validation, execution history.
- Avoid: generic TOSCA DSL, generic plugin engine, arbitrary workflow engine in MVP.

## Terraform / HCP Terraform reference

The key pattern to borrow is **plan before apply**. Plans must require confirmation before deployment unless a future trusted automation mode explicitly enables auto-apply.

Decision for CHV:

- Every topology deployment must run validation and plan first.
- The plan must be visible before deployment.
- Destructive operations must require explicit confirmation.

## Pulumi Deployments reference

Useful patterns include deployment settings, UI/API-triggered deployment, scheduled operations, custom runners, logs and drift detection.

Decision for CHV:

- Add drift detection early, even if read-only in MVP.
- Store every deployment run with logs and status.
- Keep secrets outside YAML and reference them by `secret_ref` only.

## GUI alternatives

| Option | Fit for CHV | Recommendation |
|---|---:|---|
| Extend existing custom SVG topology canvas | Medium | Keep as live topology viewer, not as editor |
| Svelte Flow | High | Recommended |
| React Flow | Medium | Mature, but framework mismatch |
| Rete.js | Medium-low | Too workflow/dataflow-oriented |
| JointJS / jsPlumb | Medium | Strong but heavier and less natural for Svelte MVP |

## Final recommendation

Build a new **Architecture Designer** with Svelte Flow. Keep existing CHV topology canvas for current-state fleet view and drift visualization.
