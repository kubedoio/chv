# ADR-0001: MVP-1 is Linux-first and cloud-image-first

## Status
Accepted

## Context
Broad virtualization compatibility would significantly increase scope, testing burden, and operational ambiguity.

## Decision
MVP-1 supports Linux cloud-image workloads only.
ISO installation, appliance-style guests, and Windows-first support are deferred.

## Consequences
### Positive
- tighter validation matrix
- simpler provisioning model
- better fit with Cloud Hypervisor strengths
- faster MVP delivery

### Negative
- narrower market
- no legacy migration story in MVP-1
