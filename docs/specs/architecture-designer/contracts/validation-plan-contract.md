# Contract: Validation, Fleet Check and Plan Results

## Severity levels

```text
error    blocks deployment
warning  requires acknowledgement
info     non-blocking
```

## Finding object

```json
{
  "severity": "error",
  "code": "MISSING_DATASTORE",
  "message": "Instance app-01 references datastore ceph-rbd, but it does not exist.",
  "path": "instances[0].disks[1].datastore",
  "resource_ref": "instances/app-01",
  "blocking": true,
  "suggestion": "Create datastore ceph-rbd or select an existing datastore."
}
```

## Validation result

```json
{
  "status": "valid|warning|invalid",
  "summary": {
    "errors": 0,
    "warnings": 0,
    "info": 0
  },
  "findings": []
}
```

## Fleet check result

```json
{
  "status": "valid|warning|invalid",
  "inventory_snapshot_id": "inv_01HX...",
  "checked_at": "2026-06-13T09:00:00Z",
  "findings": []
}
```

## Plan result

```json
{
  "plan_id": "plan_01HX...",
  "architecture_id": "arch_01HX...",
  "architecture_version": 3,
  "status": "requires_confirmation",
  "mode": "apply",
  "summary": {
    "create": 4,
    "update": 1,
    "delete": 0,
    "replace": 0,
    "warnings": 1
  },
  "changes": [
    {
      "action": "create",
      "resource_type": "instance",
      "resource_name": "app-01",
      "resource_ref": "instances/app-01",
      "description": "Create instance app-01 on chv-node-01",
      "requires_confirmation": false
    }
  ],
  "warnings": []
}
```

## Plan statuses

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

## Destructive actions

The following actions must require typed confirmation:

```text
delete instance
delete datastore
remove disk
reduce disk size
remove network from instance
change public exposure
remove platform user
remove role assignment
```

## Plan expiry

A plan must expire after inventory changes or after a configurable TTL.

Recommended MVP TTL:

```text
15 minutes
```

Reason: a stale plan may no longer match actual fleet capacity or IP usage.
