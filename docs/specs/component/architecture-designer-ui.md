# Specification: Architecture Designer UI

## Goal

Create a WebUI panel that lets users visually design a CHV architecture, save it on the dashboard, export YAML, validate it, check it against current resources, generate a plan and deploy it.

## Navigation placement

Add a new DESIGN group above Fleet Overview.

```text
DESIGN
  Architecture Designer
  Saved Topologies

Fleet Overview
```

## Routes

```text
/architectures
/architectures/new
/architectures/{id}
/architectures/{id}?tab=canvas
/architectures/{id}?tab=yaml
/architectures/{id}?tab=validation
/architectures/{id}?tab=plan
/architectures/{id}?tab=runs
/architectures/{id}?tab=drift
```

## Main layout

```text
Left:      node palette
Center:    Svelte Flow canvas
Right:     inspector / validation / YAML / plan drawer
Top bar:   save, validate, check fleet, export YAML, plan, deploy
Bottom:    status strip / current version / last validation / last plan
```

## Top actions

```text
Save Draft
Validate
Check Against Fleet
Generate YAML
Export YAML
Plan
Deploy
Destroy Plan
History
```

## Node palette

MVP palette:

```text
Host
Network
Datastore
Image
Template
Instance
User
Role
```

Future palette:

```text
Backup Target
Backup Policy
Firewall Rule
Placement Group
Project
Secret Reference
```

## Canvas behavior

Required:

- drag nodes from palette
- move nodes
- connect nodes
- reject invalid connections
- select node or edge
- open inspector on selection
- context menu on node/edge
- minimap
- zoom/pan controls
- fit-to-view
- show validation badges on nodes
- show deployment status badges on nodes after apply

## Inspector behavior

The right-side inspector edits the selected object.

Examples:

- Host: name, role, labels, resource limits, management IP
- Network: type, bridge, VLAN, CIDR, gateway, DHCP range, DNS
- Datastore: type, path/pool/export, capabilities
- Image: source, format, datastore, checksum
- Template: image, CPU, memory, disk, default network
- Instance: template, CPU, memory, disks, networks, cloud-init users, backup policy
- User: display name, email, auth type, role assignment
- Role: permissions

## YAML mode

The user can switch to YAML mode or split mode.

Modes:

```text
Canvas
YAML
Split
Plan
```

YAML edit flow:

```text
edit YAML
  -> parse
  -> validate
  -> update graph
```

Canvas edit flow:

```text
edit graph
  -> update model
  -> regenerate YAML
```

## Saved topology dashboard

`/architectures` shows cards:

```text
Name
Environment
Status
Validation status
Drift status
Instances
Networks
Datastores
Last applied version
Last apply task
Owner
Updated time
```

Status values:

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

## Deploy UX

Clicking Deploy must not immediately mutate infrastructure.

Required flow:

```text
Deploy clicked
  -> validate
  -> check fleet
  -> generate plan
  -> show plan summary
  -> require confirmation
  -> create apply task
```

## Destructive operation UX

Destructive plans require typed confirmation:

```text
Type architecture name to confirm deployment.
```

Additional warning for production environment:

```text
This topology targets production. Review the plan carefully before applying.
```

## Acceptance criteria

1. Architecture Designer appears above Fleet Overview.
2. User can create a topology visually.
3. User can save topology as draft.
4. User can export YAML.
5. User can import YAML and render graph.
6. Invalid graph connections are rejected.
7. Static validation is visible in UI.
8. Fleet consistency check is visible in UI.
9. Plan is visible before apply.
10. Apply creates normal CHV tasks.
11. Saved topologies appear on dashboard.
12. Drift state is visible after apply.
