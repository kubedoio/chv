# Phase B NodeCache Authority Facade Evidence

## Implemented Boundary

`crates/chv-agent-core/src/cache_authority.rs` provides an unwired owning
facade with legacy, Core, and blocked modes. Core mode freezes cache version,
host identity, node state, VM/volume/network generations and exact fragment
bytes, VM attachment observations, and volume handles.

The facade guards `observe_generation`, `store_fragment`, `remove_fragment`,
`observe_vm_attachment`, `remove_vm_state`, and `update_vm_desired_state`.
Node-state transitions are also rejected in Core mode. Dedicated compatibility
methods cover only node generation, enrollment/certificate metadata, pending
messages, last error, and connectivity; reads return a detached DTO with no VM
state. A second projection check at whole-cache save is covered.
Construction validates and canonicalizes one cache path; persistence exposes
only pathless `save()`. Node-generation writes require the exact nonempty
frozen host identity.

## Executable Evidence

The focused tests prove:

- all current VM/resource mutators are rejected in Core mode;
- compatibility-only state and unchanged persistence remain available through
  narrow methods and a detached snapshot;
- Draining/Maintenance-capable node-state transitions are denied in Core mode;
- save rejects independently introduced projection drift;
- blocked mode rejects mutation and persistence;
- legacy mode preserves the existing behavior.

Architecture tests inject forbidden public raw-cache and `FnOnce` signatures
plus indirect field, alias, conversion, dereference, serialization, and clone
escapes and prove that they fail the machine guard.

Run:

```text
cargo test -p chv-agent-core cache_authority
cargo clippy -p chv-agent-core --all-targets -- -D warnings
python3 -B scripts/check-cellhv-core-architecture.py
python3 -B -m unittest tests/test_cellhv_core_architecture.py
```

## Nonclaims

The production agent is not wired to this facade. Existing handlers and the
reconciler still hold direct mutable NodeCache references. Consequently this
is evidence for a machine-enforced library boundary, not production cutover,
single-authority acceptance, restart recovery, or changed VM runtime behavior.
