# ADR-005 WebUI Design System Direction

## Status
Accepted

## Date
2026-04-15

## Decision
The CHV WebUI should not copy Proxmox or Xen Orchestra visually. It should exceed them in legibility and workflow quality.

### Direction
- modern but restrained
- enterprise-serious, not startup-like
- light mode first
- dark mode later
- high information density where needed, but never visually noisy
- border-first surfaces, minimal glow effects
- strong typography hierarchy
- tables and detail panels are core patterns
- command/result visibility is prioritized over decorative visuals

### UI quality bar
The UI should feel:
- faster to scan than Proxmox
- cleaner and more task-transparent than older virtualization UIs
- more operationally trustworthy than marketing-heavy admin panels

## Consequences
Pros:
- distinct product identity
- better operator trust
- strong fit for technical buyers

Cons:
- demands more design discipline
- requires careful component consistency
