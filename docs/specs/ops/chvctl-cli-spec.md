# `chvctl` CLI Spec

## Purpose
Provide local operator-safe inspection and limited recovery workflows for the CHV platform.

## Principles
- read-first by default
- mutation commands gated by maintenance mode or explicit force policy
- output aligned with node state machine and operation IDs
- structured output via `--output json|yaml` for automation

## Implemented Commands

This section documents the binary as built (`cmd/chvctl`); it was
realigned to the real command surface in the #372 live-path-fixes pass —
the previous revision documented commands that never existed and
spellings the binary never accepted.

### Authentication
- `chvctl login --username <username> --password <password>` — authenticate against the backend-for-frontend (BFF) and store the token locally

### Virtual Machines
- `chvctl vm list` — List all VMs
- `chvctl vm get <vm_id>` — Show VM details
- `chvctl vm create <name> [--cpu <n>] [--memory <size>] [--image <ref>] [--network <id>] [--node <node_id>] [--storage-class <local|iscsi|ceph|lvm>] [--disk-size-gb <n>] [--cloud-init <userdata|@file>]` — Create a new VM. `--memory` takes a size string ("512M", "2G"); `--node` overrides default placement; `--storage-class` is validated against the same vocabulary the BFF enforces (the create is rejected if the node does not offer the class); `--cloud-init` takes the userdata inline or reads it from a file with the `@file` form
- `chvctl vm start <vm_id>` — Start a VM
- `chvctl vm stop <vm_id>` — Stop a VM
- `chvctl vm reboot <vm_id>` — Reboot a VM
- `chvctl vm delete <vm_id>` — Delete a VM
- `chvctl vm migrate <vm_id> --to <node_id>` — Live-migrate a VM
- `chvctl vm resize <vm_id> [--cpu <n>] [--memory <size>]` — Resize VM resources

### Nodes
- `chvctl node list` — List compute nodes
- `chvctl node get <node_id>` — Show node details
- `chvctl node drain <node_id>` — Drain node (evacuate VMs via live migration)
- `chvctl node maintenance <node_id> --enable` — Enter (`--enable`) or exit (omitted) maintenance mode

### Volumes
- `chvctl volume list` — List volumes
- `chvctl volume snapshot <volume_id> --name <snapshot>` — Create a volume snapshot
- `chvctl volume clone <volume_id> --name <target_volume_id>` — Clone a volume

Volume create/show/delete are not implemented; the volume-create API is
tracked in #513.

### Images
- `chvctl image list` — List disk images
- `chvctl image import <name> --url <source_url> [--format <qcow2|raw>]` — Import an image
- `chvctl image delete <image_id>` — Delete an image

### Networks
- `chvctl network list` — List networks
- `chvctl network create <name> --cidr <cidr>` — Create a network
- `chvctl network delete <network_id>` — Delete a network

There is no `--vlan` flag: VLAN tagging does not exist at any layer of
the platform today and the flag was silently ignored when it existed
(#372); #517 tracks the real implementation.

### Tasks / Operations
- `chvctl task list` — List tasks/operations
- `chvctl task watch <task_id> [--timeout <seconds>]` — Poll a task until it reaches a terminal status (`Succeeded` exits 0; `Failed`/`Cancelled`/`Rejected`/`Stale`/`Conflict` exit non-zero). The watch is bounded: it gives up after `--timeout` seconds (default 900 = 15 minutes)

### Users (Admin)
- `chvctl user list` — List users
- `chvctl user create <username> --password <password> [--role <admin|operator|viewer>]` — Create a user
- `chvctl user delete <user_id>` — Delete a user (takes the user id from `user list`, not the username)

### Health
- `chvctl health check` — Quick control-plane health check
- `chvctl health report <node_id>` — Per-node health report
- `chvctl health cluster` — Cluster-wide health summary

### Version
- `chvctl version` — Show CLI version, git commit, build date, and release channel

## Known-broken command groups (pending removal)

The `storage`, `migrate`, and `backup` groups target BFF routes that do
not exist — every invocation 404s (#372 §2.2–2.4). They are scheduled
for removal; do not script against them:

- `chvctl storage list|show|create|delete`
- `chvctl migrate start|status|cancel|list` (use `chvctl vm migrate` to start a migration)
- `chvctl backup list|run`

## Global Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--server` | `http://localhost:8080` | BFF server URL |
| `--token` | (from `~/.config/chvctl/credentials`) | Auth token override |
| `--output` | `table` | Output format: `table`, `json`, `yaml` |

The credentials written by `chvctl login` live under the platform config
dir (`~/.config/chvctl/credentials` on Linux; `XDG_CONFIG_HOME` is
honored), not `~/.chv/`.

## Safety Requirements
- local access only unless a future remote operator model is explicitly defined
- mutations must surface confirmation, policy check result, and operation ID
- failures must map to stable error codes
- `resize` and `delete` operations enforce quota checks and ownership validation
- `drain` operations require `Operator` or `Admin` role
