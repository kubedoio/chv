# `chv-agent` Component Spec

## Purpose
`chv-agent` is the sole node-side orchestrator. It is the only process that communicates with the control plane and the only process that communicates with Cloud Hypervisor.

## Responsibilities
- node enrollment and certificate lifecycle
- fetch and reconcile desired state
- maintain the local durable cache
- enforce the node state machine
- supervise `chv-stord` and `chv-nwd`
- translate VM specs into Cloud Hypervisor configuration
- launch and control Cloud Hypervisor processes
- report inventory, health, events, and observed state
- provide local operator-safe debug hooks via `chvctl`

## Inputs
- desired state from control plane
- local durable cache
- host inventory
- health from `chv-stord` and `chv-nwd`
- Cloud Hypervisor API responses

## Outputs
- VM lifecycle actions
- service supervision intents
- observed state and telemetry reports
- structured events with operation IDs

## Hard rules
- no storage datapath
- no network datapath
- no remote Cloud Hypervisor exposure
- no silent divergence from control-plane desired state
- stale desired-state generations must be rejected

## Required capabilities
- idempotent reconcile loop
- local state recovery after restart/reboot
- per-VM API socket path handling
- operation correlation IDs across all downstream actions
- clean degraded-mode handling
- safe retry behavior

## Failure behavior
- if `chv-stord` or `chv-nwd` becomes unhealthy, mark node `Degraded`
- existing workloads may continue when safe
- no new placements when node is not `TenantReady`

## Security
- mTLS with control plane
- least-privilege local access
- strict filesystem and socket ownership
