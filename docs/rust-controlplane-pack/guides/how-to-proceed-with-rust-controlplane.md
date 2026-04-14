# How to Use the Rust Control Plane Pack with an LLM

## Recommended order

1. clean legacy Go ambiguity first
2. then start the Rust control-plane implementation
3. only after the backend contract is stable, continue with deeper WebUI binding

## Step 1 — repository cleanup chat

In a fresh chat, paste:
- your current repository tree or relevant files
- the legacy Go cleanup strategy
- the legacy Go cleanup prompt

Ask the model to do audit only first.

Do not let it delete or rewrite immediately.
Make it classify:
- keep
- archive
- delete
- document

Then approve the cleanup plan.
Then let it apply the cleanup in small patches.

## Step 2 — Rust control-plane analysis chat

In a new chat, paste in this order:
1. relevant ADRs
2. `control-plane-node.proto`
3. Rust control plane implementation spec
4. failure/runtime docs
5. Rust control plane implementation prompt

Again: analysis first, not code.

The model should first give:
- understanding
- workspace structure
- phases
- risks
- tests

Only after you approve that should it write code.

## Step 3 — reviewer chat

Use the review prompt in a separate chat.
Paste:
- the generated Rust code
- the spec
- the proto contracts

Ask the reviewer model to find:
- contract violations
- state-machine problems
- bad ownership boundaries
- leftover Go assumptions
- missing tests

## Practical advice

### Keep the chats narrow
Do not ask one model to:
- clean the repo
- redesign the architecture
- implement the entire control plane
- build the UI

Split these tasks.

### Start with these Rust control-plane slices
Recommended order:
1. workspace skeleton
2. proto generation crate
3. config + tracing bootstrap
4. enrollment service
5. persistence models and migrations
6. node inventory/state ingestion
7. desired-state persistence
8. lifecycle service client flow

### Enforce boundaries
Remind the model every time:
- control plane does not talk to Cloud Hypervisor
- nodes are managed through `chv-agent`
- desired and observed state are different things
- Rust is the active backend direction

## Best next move

Use the cleanup prompt first.
Then begin the Rust control-plane workspace skeleton.
