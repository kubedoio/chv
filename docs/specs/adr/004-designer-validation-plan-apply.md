# ADR-004-Designer Deployment Must Be Validation-, Plan- and Task-Gated

Date: 2026-06-13
Status: Accepted (2026-06-16, Phase 7)

## Context

The Architecture Designer can create real infrastructure resources. Direct execution from a canvas without validation or plan review would be unsafe, especially for production environments.

CHV already follows the product principle that every mutation must produce a task and resource state must remain legible.

## Decision

Every deploy action must go through this sequence:

```text
Save Draft
  -> Static Validation
  -> Fleet Consistency Check
  -> Plan Generation
  -> User Confirmation
  -> Task-Based Apply
  -> Apply Result
  -> Drift Baseline
```

## Rationale

This follows mature infrastructure-control-plane patterns:

- validate before plan
- plan before apply
- confirmation before destructive changes
- execution tracked as tasks
- drift detection after deployment

## Blocking rules

Deployment must be blocked if:

- YAML schema is invalid
- references are missing
- CIDRs overlap
- IPs duplicate
- required hosts/datastores/networks are unavailable
- requested capacity is impossible
- raw secrets are present
- user lacks permission

## Warning rules

Warnings require acknowledgement but may not block deployment:

- using a host above soft capacity threshold
- deploying to degraded but schedulable host
- public network exposure
- static IP near DHCP range
- non-redundant datastore
- missing backup policy

## Consequences

The backend must implement:

- validator
- fleet checker
- planner
- reconciler
- task integration
- apply run history
- drift checker

## Non-goals

- No silent one-click production deployment.
- No hidden execution outside CHV task system.
