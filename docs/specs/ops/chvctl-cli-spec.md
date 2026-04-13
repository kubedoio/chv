# `chvctl` CLI Spec

## Purpose
Provide local operator-safe inspection and limited recovery workflows.

## Principles
- read-first by default
- mutation commands gated by maintenance mode or explicit force policy
- output aligned with node state machine and operation IDs

## Initial commands
- `chvctl node status`
- `chvctl node inventory`
- `chvctl vm list`
- `chvctl vm show <vm_id>`
- `chvctl stor sessions`
- `chvctl stor volume <volume_id>`
- `chvctl nw status`
- `chvctl nw namespaces`
- `chvctl ops tail`

## Later commands
- `chvctl node drain`
- `chvctl node maintenance enter`
- `chvctl node maintenance exit`
- `chvctl vm reboot <vm_id>`

## Safety requirements
- local access only unless a future remote operator model is explicitly defined
- mutations must surface confirmation, policy check result, and operation ID
- failures must map to stable error codes
