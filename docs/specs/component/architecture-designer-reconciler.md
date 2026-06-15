# Specification: Backend Validator, Planner and Reconciler

## Goal

Implement backend components that convert a saved topology into executable CHV resource operations safely and idempotently.

## Components

```text
Architecture API Handler
Architecture Repository
YAML Parser
Schema Validator
Reference Validator
Fleet Consistency Checker
Planner
Reconciler
Task Adapter
Drift Checker
Version Store
Run Store
```

## Processing flow

```text
ArchitectureTopology
  -> parse YAML/model
  -> static validation
  -> fleet consistency check
  -> desired/current diff
  -> plan
  -> task-based apply
  -> store result
  -> update drift baseline
```

## Reconciler principles

1. Idempotent operations only.
2. Never apply without a valid plan.
3. Never apply stale plans.
4. Never store raw secrets from YAML.
5. Every mutation must map to a CHV task.
6. Every task must be linkable to architecture, version and resource.
7. Partial failure must be reported clearly.
8. Re-running apply after failure must be safe where possible.

## Desired/current diff

Compare desired topology against current CHV resources.

Output actions:

```text
create
update
delete
replace
noop
blocked
```

## Operation ordering

Recommended apply order:

1. Roles
2. Users
3. Datastore references
4. Networks
5. Images
6. Templates
7. Instances
8. Disk attachments
9. Network attachments
10. Cloud-init configuration
11. Backup policy bindings
12. Drift baseline update

Recommended destroy order:

1. Backup policy bindings
2. Instance network attachments
3. Instance disk attachments
4. Instances
5. Templates
6. Images if managed by topology
7. Networks if managed by topology
8. Datastore references if managed by topology
9. Users/roles if managed by topology

## Plan safety

Plan must include:

```text
resource type
resource name
action
description
before state
after state
risk level
requires confirmation
blocking reason if any
```

## Apply run states

```text
queued
running
succeeded
partially_failed
failed
cancelled
```

## Drift checker

Compare last applied desired state to current resource state.

Drift types (wire codes — these strings appear verbatim in the
`code` field of every `DriftFinding` JSON object):

```text
DRIFT_MISSING_RESOURCE
DRIFT_UNEXPECTED_RESOURCE
DRIFT_FIELD_CHANGED
DRIFT_CAPACITY_CHANGED
DRIFT_NETWORK_CHANGED
DRIFT_PERMISSION_CHANGED
DRIFT_ATTACHMENT_CHANGED
```

Wire codes adopt the `DRIFT_` prefix for grep-friendliness and to distinguish
them from validation `Finding.code` values.

## MVP implementation note

For the first implementation, drift detection may be read-only and report-only. It does not need automatic remediation.
