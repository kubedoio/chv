# ADR-001 WebUI Product Principles

## Status
Accepted

## Date
2026-04-15

## Context
CHV needs a WebUI that competes with established virtualization platforms while matching CHV's architecture:
- remote control plane
- node-side `chv-agent`
- host-side `chv-stord`
- host-side `chv-nwd`
- Cloud Hypervisor as the VMM

The UI must not become a thin wrapper around low-level node internals. It must present a clear operator product.

## Decision
The WebUI is a product-facing control surface built around:
- clarity over density
- task transparency over hidden background work
- cluster-first navigation
- fast access to VM lifecycle actions
- explicit health/state visibility
- typed backend contracts through a control-plane-facing BFF layer

## Principles
1. **Cluster and fleet first**
   - the primary mental model is datacenter / cluster / node / workload
2. **Tasks are first-class**
   - every operator action must produce a visible task or operation record
3. **State must be legible**
   - node state, VM state, storage health, and network health must be easy to read
4. **No browser-to-node direct coupling**
   - the browser never talks directly to node daemons or Cloud Hypervisor
5. **Progressive depth**
   - overview first, details second, expert controls third
6. **Predictable mutation UX**
   - dangerous actions are explicit, reversible when possible, and auditable
7. **Private-cloud-first usability**
   - optimize first for operators managing real infrastructure, not hobby dashboards

## Consequences
Pros:
- aligns UI with the CHV architecture
- keeps backend contracts clean
- enables better operator trust and auditability

Cons:
- requires a deliberate BFF/API layer
- pushes more view-model shaping into backend services
