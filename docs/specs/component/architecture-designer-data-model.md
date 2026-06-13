# Specification: Data Model

## Main entities

```text
ArchitectureTopology
ArchitectureVersion
ArchitecturePlan
ArchitectureApplyRun
ArchitectureDriftReport
ArchitectureValidationResult
ArchitectureFleetCheckResult
```

## ArchitectureTopology

Fields:

```text
id
name
display_name
description
environment
status
owner_user_id
design_graph_json
latest_yaml
latest_version_id
last_validation_status
last_fleet_check_status
last_plan_id
last_apply_run_id
last_apply_task_id
last_drift_status
created_at
updated_at
archived_at
```

## ArchitectureVersion

Fields:

```text
id
architecture_id
version_number
yaml_content
design_graph_json
normalized_model_json
created_by
created_at
change_summary
```

## ArchitecturePlan

Fields:

```text
id
architecture_id
architecture_version_id
inventory_snapshot_id
mode
status
plan_json
summary_json
created_by
created_at
expires_at
confirmed_at
confirmed_by
discarded_at
```

## ArchitectureApplyRun

Fields:

```text
id
architecture_id
architecture_version_id
plan_id
task_id
status
started_at
finished_at
requested_by
result_json
logs_ref
error_message
```

## ArchitectureDriftReport

Fields:

```text
id
architecture_id
baseline_version_id
inventory_snapshot_id
status
summary_json
findings_json
created_at
```

## Status enums

Architecture status:

```text
draft
valid
invalid
planned
applying
applied
drifted
failed
archived
```

Plan status:

```text
draft
failed_validation
requires_confirmation
ready_to_apply
applying
applied
failed
expired
discarded
```

Run status:

```text
queued
running
succeeded
partially_failed
failed
cancelled
```

Drift status:

```text
unknown
no_drift
drifted
check_failed
```

## Persistence rule

Always store:

1. original YAML
2. normalized model JSON
3. graph JSON
4. validation results
5. plan result
6. apply run result

This allows auditability and future migrations.
