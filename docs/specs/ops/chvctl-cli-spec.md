# `chvctl` CLI Spec

## Purpose
Provide local operator-safe inspection and limited recovery workflows for the CHV platform.

## Principles
- read-first by default
- mutation commands gated by maintenance mode or explicit force policy
- output aligned with node state machine and operation IDs
- structured output via `--output json|yaml` for automation

## Implemented Commands

### Authentication
- `chvctl login` — Authenticate against the BFF and store token locally

### Virtual Machines
- `chvctl vm list` — List all VMs
- `chvctl vm show <vm_id>` — Show VM details
- `chvctl vm create` — Create a new VM
- `chvctl vm start <vm_id>` — Start a VM
- `chvctl vm stop <vm_id>` — Stop a VM
- `chvctl vm reboot <vm_id>` — Reboot a VM
- `chvctl vm delete <vm_id>` — Delete a VM
- `chvctl vm resize <vm_id> --cpu <n> --memory-mb <n>` — Resize VM resources

### Nodes
- `chvctl node list` — List compute nodes
- `chvctl node show <node_id>` — Show node details
- `chvctl node drain <node_id>` — Drain node (evacuate VMs via live migration)
- `chvctl node maintenance <node_id>` — Maintenance subcommands

### Storage
- `chvctl volume list` — List volumes
- `chvctl volume show <volume_id>` — Show volume details
- `chvctl volume create` — Create a volume
- `chvctl volume delete <volume_id>` — Delete a volume
- `chvctl storage list-pools` — List storage pools
- `chvctl storage show-pool <pool_id>` — Show pool details

### Images
- `chvctl image list` — List disk images
- `chvctl image show <image_id>` — Show image details
- `chvctl image import` — Import an image
- `chvctl image delete <image_id>` — Delete an image

### Networks
- `chvctl network list` — List networks
- `chvctl network show <network_id>` — Show network details
- `chvctl network create` — Create a network
- `chvctl network delete <network_id>` — Delete a network

### Tasks / Operations
- `chvctl task list` — List tasks/operations
- `chvctl task show <task_id>` — Show task details

### Backups
- `chvctl backup list` — List backup jobs
- `chvctl backup show <backup_id>` — Show backup details
- `chvctl backup create` — Create a backup job
- `chvctl backup restore <backup_id>` — Restore from backup

### Users (Admin)
- `chvctl user list` — List users
- `chvctl user create` — Create a user
- `chvctl user delete <user_id>` — Delete a user

### Live Migration
- `chvctl migrate start` — Start a live migration
- `chvctl migrate status <migration_id>` — Check migration status
- `chvctl migrate cancel <migration_id>` — Cancel a migration

### Upgrades
- `chvctl upgrade start <node_id> --version <ver>` — Start rolling upgrade
- `chvctl upgrade status <node_id>` — Check upgrade status
- `chvctl upgrade list` — List active/past upgrades
- `chvctl upgrade rollback <node_id>` — Rollback a failed upgrade

### Health
- `chvctl health cluster` — Cluster health summary (deep health check)

### Version
- `chvctl version` — Show CLI version, git commit, build date, and release channel

## Global Flags

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--server` | `-s` | `http://localhost:8080` | BFF server URL |
| `--token` | `-t` | (from `~/.chv/credentials`) | Auth token override |
| `--output` | `-o` | `table` | Output format: `table`, `json`, `yaml` |

## Safety Requirements
- local access only unless a future remote operator model is explicitly defined
- mutations must surface confirmation, policy check result, and operation ID
- failures must map to stable error codes
- `resize` and `delete` operations enforce quota checks and ownership validation
- `drain` and `upgrade` operations require `Operator` or `Admin` role
