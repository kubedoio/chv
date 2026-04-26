# ADR-003 WebUI Navigation Model

## Status
Accepted

## Date
2026-04-15

## Decision
The primary navigation model is:

- Overview
- Datacenters / Clusters
- Nodes
- Virtual Machines
- Volumes
- Networks
- Images / Templates
- Tasks
- Events / Alerts
- Maintenance / Upgrades
- Settings / Access

## Detail navigation pattern
Every major resource gets:
- Summary
- Configuration
- Tasks
- Events
- Related resources

## Reasoning
This model is more legible for operators than purely technical left-tree structures or raw inventory lists.

## Consequences
Pros:
- easier operator orientation
- scalable for future cluster growth
- easier to map to audits, troubleshooting, and task flow

Cons:
- requires careful cross-linking between related resources
