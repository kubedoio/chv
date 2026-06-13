# ADR-003-Designer Use CHVArchitecture YAML as the Source of Truth

Date: 2026-06-13
Status: Proposed

## Context

The designer must produce a durable, reviewable, portable and versionable representation of the topology. A pure UI graph JSON would be too tied to the frontend. A generic TOSCA/Cloudify DSL would be too broad for CHV's product scope.

## Decision

Define a CHV-native YAML contract:

```yaml
apiVersion: chv.kubedo.io/v1alpha1
kind: CHVArchitecture
```

This YAML is the source of truth for a topology.

The visual graph is an editing representation. The database model is operational state. The YAML remains the authoritative desired-state document.

## Rationale

YAML provides:

- export/import
- reviewability
- GitOps future compatibility
- human readability
- decoupling from UI internals
- stable backend contract

## Model separation

```text
Visual graph JSON
  = UI layout and node/edge positions

CHVArchitecture YAML
  = desired state contract

Normalized DB state
  = queryable operational representation

Runtime resources
  = actual CHV resources
```

## Consequences

The implementation must include:

- graph -> normalized model converter
- normalized model -> YAML generator
- YAML -> normalized model parser
- normalized model -> graph converter
- schema validation
- reference validation
- version history

## Non-goals

- Do not expose raw Cloudify/TOSCA DSL.
- Do not store passwords or tokens inside YAML.
- Do not make UI graph JSON the only saved format.
