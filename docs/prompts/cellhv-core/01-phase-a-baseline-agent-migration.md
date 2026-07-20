# Prompt 01 — Phase A1: Baseline and `chv-agent` Migration Lock

Implement the first narrow slice of Phase A.

## Goal

Produce a factual migration map and enforce the decision that the existing `chv-agent` evolves into CellHV Core. This slice must not change VM runtime behavior.

## Branch

`agent/cellhv-core-pa-baseline`

## Estimated effort

3–5 engineering days for one senior Rust/Linux engineer, plus architecture review.

## Required work

### 1. Current-state inventory

Create a versioned analysis document with file/type/function references for:

- current `chv-agent` binary and crate graph;
- VM identity and lifecycle ownership;
- desired, observed, cached, and persistent state;
- operation/idempotency handling;
- Cloud Hypervisor launch, sockets, processes, console, and recovery;
- current `chv-stord` and `chv-nwd` boundaries;
- control-plane gRPC dependencies;
- privilege boundaries;
- installation, package, and service names;
- current tests and real-KVM gaps;
- code to reuse, refactor, deprecate, or leave untouched.

Do not describe intended architecture as current fact.

### 2. Enforce ADR-016

Add static or repository checks that fail if an active implementation introduces:

- a new `cellhvd` binary or systemd service;
- a second VM runtime database or operation engine;
- independent old/new VM process ownership;
- a Core dependency on Controller, UI, Designer, or cloud-platform models;
- Cloud Hypervisor advertised as QEMU.

Allow documentation references explaining why `cellhvd` is not created.

### 3. Define migration seams

Document exact seams for incremental work:

- where durable local state will sit beneath `chv-agent`;
- how current gRPC requests will enter the future operation engine;
- where the native local API will enter the same engine;
- how NodeCache data may be migrated;
- how current Cloud Hypervisor runtime code is reused;
- how ownership conflicts between old and new paths are prevented;
- how rollback restores current behavior without losing VM identity.

### 4. Qualification tuple

Check in a machine-readable proposed tuple:

```yaml
host_distribution: Ubuntu Server 24.04 LTS
architecture: x86_64
kernel: <QUALIFIED_KERNEL_RANGE>
vmm:
  backend: cloud-hypervisor
  version: <PINNED_CLOUD_HYPERVISOR_VERSION>
firmware: <PINNED_OVMF_BUILD>
guest: <PINNED_UBUNTU_LTS_CLOUD_IMAGE>
network: existing-linux-bridge-or-tap
storage: existing-raw-file-or-block-device
runtime_service: chv-agent
manager: absent
```

Placeholders are allowed only with an owner and deadline for resolution.

### 5. Minimal acceptance registry

Create only the schema/validator required to register acceptance IDs, tiers, prerequisites, required evidence, forbidden outcomes, cleanup, and timeout.

Do not build a comprehensive test harness.

## Acceptance criteria

- `AGENT-CORE-001`: no parallel `cellhvd` implementation exists.
- `AGENT-CORE-006`: packaging guards prevent two runtime services.
- `VMM-ID-001`: static guard prevents Cloud Hypervisor QEMU identity.
- `CLAIM-001`: compatibility-claim schema validates.
- Current-state analysis is reviewed for factual accuracy.
- Existing workspace format, build, Clippy, tests, and policy checks pass.

## Explicit non-scope

- SQLite authority implementation;
- native API implementation;
- VM runtime changes;
- provider redesign;
- OpenStack integration implementation;
- cloud-platform support claims.

## Deliverables

- current-state inventory;
- migration-seam document;
- qualification tuple;
- ADR/dependency/identity checks;
- minimal acceptance registry;
- effort estimate for Phase B based on actual code;
- rollback instructions.

## Exit gate

Reviewers must be able to answer:

1. Which existing `chv-agent` code becomes Core?
2. Where will local authority be inserted?
3. How will old and new API paths share one operation engine?
4. How is a second runtime technically prevented?
5. Which exact host/VMM/guest tuple will be used for real-KVM testing?
