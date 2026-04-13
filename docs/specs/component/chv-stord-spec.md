# `chv-stord` Component Spec

## Purpose
`chv-stord` is the host-side storage service boundary for volume attach/open/serve and lifecycle hooks.

## MVP-1 storage classes
- local qcow2/raw file
- local block/LVM
- iSCSI block
- Ceph RBD

## Responsibilities
- open and close volumes
- attach and detach volumes for VMs
- expose block endpoints for Cloud Hypervisor integration
- resize hooks
- snapshot preparation hooks
- clone preparation hooks
- publish volume health
- publish storage metrics
- apply per-device runtime policy

## Non-goals
- no global storage scheduler
- no distributed replication controller
- no control-plane client responsibilities
- no VM lifecycle management

## Required properties
- typed local API over Unix socket
- idempotent open/close behavior for repeated desired attachment intent
- reconstructable runtime state
- explicit error codes
- structured logging
- per-volume health visibility

## Security requirements
- dedicated service account
- restricted Unix socket permissions
- minimal filesystem visibility
- explicit device/path allowlists
- capability drop and sandboxing where possible

## Recovery model
- existing VMs should continue where safe if service restarts
- session/runtime state should be rebuilt from durable metadata plus local cache
