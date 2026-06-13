# Contract: Architecture Designer Graph JSON

## Purpose

The graph JSON stores visual editor layout and node/edge metadata. It is not the authoritative infrastructure contract. The authoritative desired-state contract is `CHVArchitecture` YAML.

## Graph structure

```json
{
  "version": "1.0",
  "nodes": [
    {
      "id": "node-host-chv-node-01",
      "type": "host",
      "position": { "x": 120, "y": 100 },
      "data": {
        "name": "chv-node-01",
        "role": "compute"
      }
    }
  ],
  "edges": [
    {
      "id": "edge-instance-app-01-to-host-chv-node-01",
      "type": "placement",
      "source": "node-instance-app-01",
      "target": "node-host-chv-node-01",
      "data": {
        "relationship": "placed_on"
      }
    }
  ]
}
```

## Node types

```text
root
host
network
datastore
image
template
instance
user
role
backup_target
backup_policy
```

MVP node types:

```text
host
network
datastore
image
template
instance
user
role
```

## Edge types

```text
placed_on
attached_to_network
uses_datastore
uses_image
uses_template
has_role
uses_backup_policy
```

## Edge validation rules

| Source | Target | Edge type |
|---|---|---|
| instance | host | placed_on |
| instance | network | attached_to_network |
| instance | datastore | uses_datastore |
| template | image | uses_image |
| instance | template | uses_template |
| user | role | has_role |
| instance | backup_policy | uses_backup_policy |

Invalid edge combinations must be rejected in the UI before saving.

## Synchronization rules

Canvas edit:

```text
graph -> normalized model -> generated YAML
```

YAML edit:

```text
YAML -> normalized model -> graph
```

## Layout rules

The graph may store layout metadata, but layout metadata must not affect deployment semantics.

Forbidden:

```text
Using visual position to determine placement
Using visual grouping as hidden deployment semantics
```

Required:

```text
All deployment semantics must exist in YAML/model fields.
```
