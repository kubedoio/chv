# Prompt 01 — Phase 0: Repository Baseline and Architecture Guards

Implement Phase 0 of the CellHV Core programme.

## Goal

Create a factual repository baseline and machine-enforced architecture guards before extracting Core. This phase must not change VM runtime behavior.

## Preconditions

- Read and apply `00-execution-policy.md`.
- Confirm ADR-015 and all Core contracts exist on the target branch.
- Start from current `main` in `agent/cellhv-core-p00-baseline`.

## Required work

### 1. Produce a repository architecture inventory

Create a versioned document under `docs/analysis/` or `docs/plans/` that records:

- current crate and binary graph;
- current owners of VM identity, desired state, observed state, and operations;
- current Cloud Hypervisor launch, socket, process, and recovery paths;
- current network and storage mutation paths;
- current persistence formats and locations;
- current public and private APIs;
- current privilege boundaries;
- current test tiers and missing real-KVM evidence;
- reusable code versus code that must be isolated or retired.

Every statement must reference concrete files, types, or functions.

### 2. Define the first qualification tuple

Add a checked-in proposed matrix containing placeholders only where selection genuinely remains open:

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
api: local-unix-http
store: sqlite
manager: absent
```

Add validation that rejects an incomplete matrix when a release claim is generated. Do not invent exact versions without repository evidence.

### 3. Add dependency and identity guards

Implement CI/static checks that fail when:

- a Core crate depends on Controller, Web UI, Designer, OpenStack, CloudStack, OpenNebula, Kubernetes, or cloud-specific crates;
- an integration crate accesses Core database modules;
- Cloud Hypervisor code advertises `qemu:///system`, QMP, or QEMU capabilities;
- platform-specific state types enter Core domain modules;
- privileged helper APIs expose arbitrary command, path, systemd, or nftables execution;
- compatibility claims omit required tuple fields.

Prefer a small repository script or Rust test with deterministic output. Document how to extend the guard.

### 4. Define crate-boundary targets

Add or update documentation for the intended boundaries:

- Core domain and operation crates;
- Core store;
- native API;
- VMM adapter interface;
- Cloud Hypervisor adapter;
- network provider interface;
- storage provider interface;
- privileged helper;
- integrations outside Core.

Do not create empty placeholder crates unless they are needed to enforce dependency direction now.

### 5. Establish test-harness skeleton

Create the smallest scenario-registry schema and validator necessary to register acceptance IDs. It must support:

- scenario ID;
- tier;
- profile;
- prerequisites;
- required evidence;
- forbidden outcomes;
- cleanup assertion;
- timeout.

Do not implement a large test framework in this phase.

## Acceptance criteria

- `CLAIM-001` compatibility-claim schema validation exists.
- `VMM-ID-001` has a static guard preventing Cloud Hypervisor from being reported as QEMU.
- Core dependency-direction violations produce a failing CI test.
- The current architecture inventory is reviewed for factual accuracy.
- The qualification tuple is checked in and machine-readable.
- Existing workspace formatting, build, Clippy, unit tests, and dependency-policy checks pass.

## Forbidden outcomes

- changing current production runtime behavior;
- renaming all existing crates or binaries;
- adding new cloud-platform adapters;
- adding fake Core implementation placeholders that compile but have no contract;
- marking ADR-015 accepted without its acceptance conditions;
- choosing `ch:///system` or a native adapter as the final cloud strategy without evidence.

## Deliverables

- architecture inventory;
- proposed qualification tuple;
- dependency/identity guard implementation;
- minimal scenario registry and validator;
- CI integration;
- phase evidence report;
- rollback instructions.

## Exit gate

Do not begin Phase 1 until reviewers can answer, from committed evidence:

1. Which component owns each piece of VM state today?
2. Which existing code will be reused by Core?
3. Which dependency directions are forbidden and machine-enforced?
4. Which exact initial host/VMM/guest tuple will Phase 2 qualify?
