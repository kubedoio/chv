# Practical LLM Workflow Guide for CHV

## Goal
Use one or more LLMs without letting them collapse the architecture or drift away from the contracts.

## Recommended execution order
1. `chv-stord`
2. `chv-nwd`
3. `chv-agent`
4. reviewer/audit pass
5. integration pass

This order works well because:
- `chv-stord` is the narrowest bounded component
- `chv-nwd` is more stateful, but still bounded
- `chv-agent` depends on the other two boundaries being clear

## Use separate focused chats
Do not use one giant chat for the entire system.
Open separate chats for:
- `chv-stord`
- `chv-nwd`
- `chv-agent`
- reviewer/audit
- integration

## Always give documents in this order
1. relevant ADRs
2. relevant proto contract
3. relevant component spec
4. relevant ops docs
5. repository layout spec
6. the task prompt

## Stage-by-stage interaction pattern

### Stage 1 — analysis only
Ask the model to:
- summarize component boundary
- list assumptions
- propose file map
- propose phased plan
- propose tests

Do not allow code yet.

### Stage 2 — exact file map
Ask the model to produce:
- file paths
- purpose of each file
- public types/functions
- dependencies
- test file mapping

Approve or correct the plan.

### Stage 3 — code in small vertical slices
Ask for the first runnable slice only.
Examples:
- server bootstrap + 2–4 handlers
- state machine skeleton + cache bootstrap
- basic health endpoints + tests

Do not ask for the whole component in one go.

### Stage 4 — reviewer pass
Open a second chat.
Paste:
- original spec
- generated code/files
- reviewer-audit-prompt.md

Use the reviewer model to identify:
- contract drift
- architecture violations
- missing tests
- idempotency mistakes

### Stage 5 — fix round
Return to the implementation chat with the review findings.
Ask for a concrete patch, not a rewrite.

## Prompting rules that help a lot
Always restate these constraints:
- do not merge daemons
- do not expose Cloud Hypervisor remotely
- preserve typed proto contracts
- preserve node state machine
- preserve thin-host principle
- reject stale generations cleanly
- keep ensure/apply operations idempotent

## What to paste into each component chat

### For `chv-stord`
Paste:
- `001-node-runtime-split.md`
- `003-node-state-machine.md`
- `004-storage-datapath.md`
- `chv-stord-api.proto`
- `chv-stord-spec.md`
- storage-related rows from `failure-matrix.md`
- `runtime-sequences.md`
- `repository-layout-spec.md`
- `prompts/chv-stord-implementation-prompt.md`

### For `chv-nwd`
Paste:
- `001-node-runtime-split.md`
- `003-node-state-machine.md`
- `005-network-service-model.md`
- `chv-nwd-api.proto`
- `chv-nwd-spec.md`
- network-related rows from `failure-matrix.md`
- `runtime-sequences.md`
- `repository-layout-spec.md`
- `prompts/chv-nwd-implementation-prompt.md`

### For `chv-agent`
Paste:
- `001-node-runtime-split.md`
- `002-control-plane-boundary.md`
- `003-node-state-machine.md`
- `control-plane-node.proto`
- `chv-agent-spec.md`
- `failure-matrix.md`
- `runtime-sequences.md`
- `repository-layout-spec.md`
- `prompts/chv-agent-implementation-prompt.md`

## Model behavior to watch for
Stop or correct the model if it does any of these:
- merges `chv-agent` and `chv-stord`
- merges `chv-agent` and `chv-nwd`
- replaces protobuf contracts with ad hoc JSON REST
- weakens the node state machine
- introduces service VMs for MVP-1
- puts Cloud Hypervisor access outside `chv-agent`
- invents features that are clearly deferred in the specs

## Best review method
Use one model to implement and another to review.
If using one model only, at least use separate chats:
- one for implementation
- one for review

## Minimal success criteria per component

### `chv-stord`
- local Unix-socket server
- typed handlers
- typed errors
- minimal health model
- session tracking
- tests

### `chv-nwd`
- local Unix-socket server
- deterministic topology ensure/delete behavior
- health reporting
- namespace state listing
- tests

### `chv-agent`
- local durable cache bootstrap
- state machine skeleton
- control-plane client skeleton
- local daemon client skeletons
- runtime adapter boundary for Cloud Hypervisor
- tests

## Final recommendation
Do not rush to a giant implementation prompt.
Treat the LLM like a strong junior-to-mid engineer with speed, not like an infallible architect.
The documents are there to constrain it.
