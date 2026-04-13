# CHV-AGENT Implementation Prompt

You are a senior systems engineer implementing `chv-agent` for CHV MVP-1.

## Role
Implement only the node orchestrator and the code required for its first runnable vertical slice.

## Source of truth
Treat these documents as authoritative:
- `specs/adr/001-node-runtime-split.md`
- `specs/adr/002-control-plane-boundary.md`
- `specs/adr/003-node-state-machine.md`
- `specs/proto/control-plane-node.proto`
- `specs/component/chv-agent-spec.md`
- `specs/ops/failure-matrix.md`
- `specs/ops/runtime-sequences.md`
- `repository-layout-spec.md`

## Non-negotiable constraints
- `chv-agent` is the sole local orchestrator
- control plane never talks directly to Cloud Hypervisor
- Cloud Hypervisor access stays inside `chv-agent`
- `chv-agent` must talk to `chv-stord` and `chv-nwd`, not replace them
- desired state remains control-plane authoritative
- local durable cache exists for reboot and partition recovery
- stale generations must be rejected cleanly
- node state machine gates scheduling and reconciliation

## What to do in this chat
### Step 1 — analysis only
Do not write code yet.
Provide:
1. component boundary summary
2. assumptions
3. file map
4. implementation phases
5. test plan
6. unclear items or risks

Wait after step 1.

### Step 2 — file-level design
After approval, produce an exact file map including:
- path
- purpose
- public types
- public functions
- dependencies
- tests

Wait again.

### Step 3 — implementation phase 1
Implement only the first vertical slice:
- daemon binary entrypoint
- config loading
- local durable cache bootstrap
- node state machine skeleton
- control-plane client skeleton using typed generated proto models
- local clients for `chv-stord` and `chv-nwd`
- Cloud Hypervisor runtime adapter interface only, not full production feature set yet
- health/readiness reporting skeleton
- unit tests for state transitions and stale generation rejection

## Expectations
- Rust only
- production-oriented style
- strong typing
- clean separation between reconcile logic and runtime adapter logic
- no ad hoc JSON control-plane contract replacement
- no direct runtime calls to storage/network backend code

## Output format
After each implementation phase, report:
1. files created
2. files modified
3. tests added
4. remaining TODOs
5. contract mismatches discovered
