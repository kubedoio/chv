# Prompt 07 — Phase F: Controller, O3K, and Core 1.0 Qualification

Complete the active CellHV Core programme by moving CellHV-owned management systems onto the public `chv-agent` Core authority path and qualifying the supported 1.0 scope.

## Preconditions

- Phase C standalone/recovery profile passes.
- Phase D advertised providers pass.
- Phase E OpenStack compatibility reaches at least Preview.
- Use branch `agent/cellhv-core-pf-qualification`.

## Estimated effort

6–8 engineering weeks, divided into separate Controller, O3K, packaging, and release-qualification PRs.

## Goal

Prove that `chv-agent` is a stable self-contained compute runtime, that Controller and O3K are optional ecosystem bridges rather than runtime authorities, and that the published 1.0 support claims are reproducible.

## Required work

### 1. Controller migration

Move Controller operations to the public Core authority path.

Requirements:

- Controller does not access Core database or VMM/provider private APIs;
- Controller stores a projection, not the only VM record;
- retries map to Core idempotency;
- resource-version conflicts are handled explicitly;
- Controller restart and data loss can rebuild projections from Core;
- Controller removal/outage does not stop workloads;
- unsupported Core capabilities are hidden or disabled in the UI/API;
- existing control-plane compatibility path has a documented deprecation timeline.

Do not redesign the full Web UI in this phase.

### 2. O3K integration

Implement O3K as a native consumer of the public Core API.

Requirements:

- O3K-specific project, tenant, image, network, and policy models remain outside Core;
- compute lifecycle maps to durable Core operations;
- identity mapping is deterministic;
- O3K restart/outage does not stop workloads;
- supported/unsupported OpenStack-compatible behavior is explicit;
- no dependency on Controller internals.

O3K is a lightweight OpenStack-compatible control plane, not the same integration as standalone OpenStack Nova.

### 3. Managed endpoint

Add remote management only after local standalone behavior is stable.

Implement and qualify:

- HTTPS/mTLS or the reviewed managed transport;
- node enrollment;
- certificate rotation and expiry;
- explicit client identities and authorization;
- management leases only where required;
- replay/idempotency protection;
- local workload survival after credential or manager failure;
- fail-closed behavior for new unauthorized mutations.

Remote access must not expose privileged helper or VMM sockets.

### 4. Packaging and service lifecycle

Produce supported packages for the qualification matrix.

Requirements:

- install and uninstall procedures;
- service user and filesystem permissions;
- database/runtime/log locations;
- pinned Cloud Hypervisor and firmware dependencies;
- package checksums and signatures;
- SBOM;
- configuration validation;
- one-step documented rollback where feasible;
- no parallel `cellhvd` service;
- upgrade preserves VM identity and running workloads within the published profile.

### 5. Upgrade and rollback qualification

Test:

- previous supported version to candidate;
- candidate rollback before schema migration where supported;
- documented recovery when schema rollback is impossible;
- running VM preservation;
- agent restart and re-adoption;
- Controller and O3K version skew within the published matrix;
- certificate rotation during normal operation;
- package failure midway through upgrade.

### 6. Security qualification

Perform and document:

- threat-model review;
- privilege-boundary review;
- Unix socket and remote authorization tests;
- path, identifier, and payload validation;
- secret redaction;
- dependency advisory checks;
- signed artifacts and provenance;
- denial of direct VMM/provider/private-store access;
- negative tests for unsupported or malformed requests.

### 7. Soak and performance budgets

Define evidence-based budgets for the supported host class and run:

- 24-hour lifecycle/steady-state soak;
- extended soak before Supported status;
- repeated agent restart;
- repeated Controller/O3K restart;
- resource leak and database growth checks;
- idle CPU and memory measurement;
- operation latency and event-delivery measurement;
- log and metric cardinality checks.

Do not choose arbitrary performance numbers solely to pass release gates. Record baseline and justify budgets.

### 8. Release claims

Publish exact claim tuples for:

- standalone Core;
- Cloud Hypervisor VMM;
- each advertised network path;
- each advertised storage path;
- selected OpenStack integration;
- Controller integration;
- O3K integration.

Each claim names versions, workload profile, evidence digest, unsupported features, and maintenance owner.

CloudStack, OpenNebula, Kubernetes, Terraform, Designer, other VMMs, and unqualified providers must not appear as supported 1.0 claims.

## Acceptance criteria

- `AGENT-CORE-005`: manager removal does not stop workloads or erase identity.
- Controller projection rebuild succeeds from Core public APIs.
- O3K lifecycle uses only the public Core authority path.
- managed endpoint authentication and rotation tests pass.
- minimal Core, recovery, Cloud Hypervisor, and advertised provider profiles pass.
- selected OpenStack profile passes at its published level.
- package install/upgrade/rollback evidence exists.
- security and soak profiles pass.
- compatibility claim tuples validate and resolve to evidence.
- no parallel runtime service or authority exists.

## Forbidden outcomes

- moving Controller or O3K models into Core;
- making Controller mandatory for workload survival;
- broad Web UI or Designer implementation;
- claiming CloudStack/OpenNebula/Kubernetes/Terraform support;
- adding another VMM backend;
- exposing private Core or provider internals;
- shipping unsigned or unversioned integration artifacts;
- hiding known limitations from the support matrix.

## Deliverables

- Controller public-API migration;
- O3K integration;
- managed endpoint and authorization;
- packages, signatures, checksums, and SBOM;
- upgrade/rollback and security evidence;
- soak/performance reports;
- operator and troubleshooting documentation;
- Core 1.0 compatibility claims;
- post-1.0 backlog for CloudStack, OpenNebula, and other deferred programmes.

## Exit gate

Phase F passes when the exact Core 1.0 support matrix is reproducible from clean hosts, running VMs survive agent-adjacent management outages and supported upgrades, Controller/O3K remain optional clients, and every published claim resolves to qualification evidence.
