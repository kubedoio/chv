# ADR-006-Designer Do Not Adopt a Generic Cloudify/TOSCA Engine in MVP

Date: 2026-06-13
Status: Proposed

## Context

Cloudify is close to the desired topology/blueprint/deployment concept. However, Cloudify/TOSCA is designed for generic cloud/application orchestration with plugins, workflows, node types and relationships across multiple domains.

CHV needs a focused virtualization topology designer.

## Decision

Do not adopt Cloudify DSL, TOSCA, or a generic plugin workflow engine in the MVP.

Instead, create a CHV-native YAML contract:

```yaml
apiVersion: chv.kubedo.io/v1alpha1
kind: CHVArchitecture
```

## Rationale

Generic DSL adoption would introduce complexity before CHV has stable native topology deployment semantics. It would also make the UI harder to reason about.

CHV should be opinionated:

- hosts
- networks
- datastores
- images
- templates
- instances
- users
- roles
- backup policies

## What to borrow from Cloudify

- Blueprints as topology documents
- Node/relationship mental model
- Validation before deployment
- Execution history
- Saved blueprint catalog

## What to avoid in MVP

- TOSCA compatibility
- arbitrary workflows
- plugin runtime
- generic multi-cloud orchestration
- application deployment DSL
