# ADR-004 WebUI Task and State Model

## Status
Accepted

## Date
2026-04-15

## Context
Virtualization UIs become hard to trust when actions disappear into background processes or when state is ambiguous.

## Decision
The WebUI treats tasks and state as first-class UI objects.

### Rules
- every mutating operator action creates a task record
- task progress is visible at global and resource scope
- resource pages show filtered tasks relevant to that resource
- node and VM states must reflect the backend state machine, not ad hoc front-end labels
- degraded states are visible and actionable

## Required task states
- queued
- running
- succeeded
- failed
- cancelled
- awaiting-operator-input (reserved for later)

## Required resource health states
- healthy
- warning
- degraded
- failed
- unknown

## Consequences
Pros:
- better operator trust
- better troubleshooting
- cleaner alignment with auditability

Cons:
- requires consistent backend task persistence
- UI must handle eventual consistency carefully
