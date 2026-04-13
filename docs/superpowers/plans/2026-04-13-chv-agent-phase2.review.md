# Plan Review: 2026-04-13-chv-agent-phase2.md

**Verdict: NEEDS_FIXES**

The plan has compilation errors, logic bugs, architectural boundary violations, and spec gaps that must be resolved before implementation.

---

## Critical Issues (Must Fix)

### 1. Compilation Error: `inner.vm` is `Option<VmMutationSpec>`
**File/Line:** `crates/chv-agent-core/src/agent_server.rs` (Task 6, `create_vm` handler, lines using `inner.vm.vm_id`)

The plan writes:
```rust
ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm.vm_id)
// and
let config = VmConfig { vm_id: inner.vm.vm_id.clone(), ... };
```

`CreateVmRequest.vm` is a protobuf message field, so in generated Rust it is `Option<VmMutationSpec>`. Direct field access on `inner.vm.vm_id` will not compile.

**Fix:** Extract the VM spec first:
```rust
let vm = inner.vm.as_ref().ok_or_else(|| Status::invalid_argument("missing vm"))?;
ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &vm.vm_id)
    .map_err(|e| Status::failed_precondition(e.to_string()))?;
let config = VmConfig {
    vm_id: vm.vm_id.clone(),
    cpus: 2,
    memory_bytes: 1024,
    kernel_path: std::path::PathBuf::from("/dev/null"),
    disk_paths: vec![],
};
self.vm_runtime.create_vm(&vm.vm_id, &meta.desired_state_version, &config).await
    .map_err(|e| Status::internal(e.to_string()))?;
```

---

### 2. Logic Bug: Node Fragment Silently Dropped
**File/Line:** `crates/chv-agent-core/src/cache.rs` (Task 3, `store_fragment`) and `crates/chv-agent-core/src/agent_server.rs` (Task 6, `apply_node_desired_state`)

`store_fragment` only handles `"vm"`, `"volume"`, and `"network"`. The `apply_node_desired_state` handler calls:
```rust
cache.store_fragment("node", &inner.node_id, crate::cache::DesiredStateFragment { ... });
```

This falls through to `_ => None` and the fragment is silently discarded. `NodeCache` also has no `node_fragments` field.

**Fix:** In `apply_node_desired_state`, remove the `store_fragment("node", ...)` call. Node desired state should only update `observed_generation` (which the code already does). The node-level fragment storage is unnecessary because the node has a single state entry.

```rust
if let Some(frag) = inner.fragment {
    cache.observe_generation("node", &inner.node_id, &frag.generation);
    // REMOVE: cache.store_fragment("node", ...)
}
```

---

### 3. Incomplete `control_plane.rs` Replacement Instructions
**File/Line:** Task 5, Step 1

The plan says "Replace the struct and `new()`" and shows only the struct, `new()`, and two new telemetry methods, with a comment `// stale_generation_check and apply_node_desired_state remain unchanged...`. An implementing agent may overwrite the entire file and lose those methods.

**Fix:** Change the instruction to "Append the two telemetry methods to the existing `impl ControlPlaneClient` block, and update the struct and `new()` signature in place. Do NOT delete `stale_generation_check` or `apply_node_desired_state`." Show the full final file or at minimum include the retained methods in the snippet.

---

### 4. Wrong Expected Test Count
**File/Line:** Task 6, Step 3

The plan says `Expected: 4 PASS`, but only 3 tests are defined in the `agent_server.rs` test module:
- `apply_vm_desired_state_updates_generation_and_fragment`
- `create_vm_lifecycle_flow`
- `lifecycle_stale_generation_rejected`

**Fix:** Change expected count to `3 PASS`, or add a fourth test (e.g., a test for `apply_node_desired_state`).

---

## Architectural / Spec Violations

### 5. Mock Adapter Used in Production Binary
**File/Line:** `cmd/chv-agent/src/main.rs` (Task 8)

The plan explicitly wires `MockCloudHypervisorAdapter` into `main.rs`:
```rust
let adapter: Arc<dyn chv_agent_runtime_ch::adapter::CloudHypervisorAdapter> =
    Arc::new(MockCloudHypervisorAdapter::default());
```

Per `chv-agent-spec.md`: "launch and control Cloud Hypervisor processes" is a core responsibility. Using a test mock in the production binary is an architectural boundary violation.

**Fix:** Create a minimal real adapter skeleton in `crates/chv-agent-runtime-ch/src/adapter.rs` (or a new file) that implements `CloudHypervisorAdapter` with stub methods returning `Err(ChvError::Internal { reason: "not yet implemented".to_string() })`. Use that skeleton in `main.rs` instead of the mock. Keep `MockCloudHypervisorAdapter` for tests only (wrap its module in `#[cfg(test)]` or a `test-utils` feature).

---

### 6. Lifecycle Commands Ignore Node State Machine
**File/Line:** `crates/chv-agent-core/src/agent_server.rs` (Task 6, all `LifecycleService` methods)

Per `adr/003-node-state-machine.md`: "only `TenantReady` nodes are schedulable for new tenant workloads" and "no new placements when node is not `TenantReady`". The `create_vm` handler does not check the node's current state before creating a VM.

**Fix:** In `create_vm`, acquire the cache lock, check `cache.node_state`, and reject with `Status::failed_precondition` if the node is not `TenantReady`:
```rust
if cache.node_state != NodeState::TenantReady.as_str() {
    return Err(Status::failed_precondition(
        format!("node not ready for new VMs: {}", cache.node_state)
    ));
}
```

(Other lifecycle methods like start/stop/delete may be allowed in degraded states; document the intended behavior in the plan.)

---

### 7. No mTLS for Control Plane Connection
**File/Line:** `crates/chv-agent-core/src/control_plane.rs` (Task 5)

Per `chv-agent-spec.md` and `adr/002-control-plane-boundary.md`: "mTLS with control plane". `ControlPlaneClient::new` uses a plain `tonic::transport::Endpoint` with no TLS configuration.

**Fix:** Add a `// TODO(Phase 3): configure mTLS using config.tls_ca_path, config.tls_cert_path, config.tls_key_path` comment in `ControlPlaneClient::new`, and update the plan to note that mTLS is deferred to Phase 3 with a clear TODO.

---

### 8. Missing Required Capabilities: Per-VM Socket Paths and Operation Correlation IDs
**File/Line:** `crates/chv-agent-runtime-ch/src/adapter.rs` and `crates/chv-agent-core/src/vm_runtime.rs`

Per `chv-agent-spec.md`: "per-VM API socket path handling" and "operation correlation IDs across all downstream actions" are required capabilities. Neither is present in the adapter trait or `VmRuntime`.

**Fix:** Add `socket_path: PathBuf` to `VmConfig`, and add an `operation_id: String` parameter to all `CloudHypervisorAdapter` trait methods. Update the mock and all call sites accordingly. If this is intentionally deferred, document it explicitly as a Phase 3 TODO in the plan.

---

## Additional Issues

### 9. Hardcoded VM Config Ignores `vm_spec_json`
**File/Line:** `crates/chv-agent-core/src/agent_server.rs` (Task 6, `create_vm`)

The `create_vm` handler hardcodes `cpus: 2, memory_bytes: 1024, kernel_path: "/dev/null"` and ignores `inner.vm.vm_spec_json`.

**Fix:** Add a `// TODO(Phase 3): parse vm_spec_json into VmConfig` comment in the handler code.

---

### 10. `VmRuntime` Uses `std::sync::Mutex` in Async Context
**File/Line:** `crates/chv-agent-core/src/vm_runtime.rs` (Task 4)

`VmRuntime` uses `std::sync::Mutex` inside async methods. While the critical sections are short, mutex poisoning could panic the async task.

**Fix:** Either switch to `tokio::sync::Mutex` (and keep locks brief), or use `parking_lot::Mutex` (no poisoning, already common in the ecosystem). At minimum, handle poisoning gracefully instead of `.unwrap()`.

---

### 11. Telemetry Client Never Reconnects
**File/Line:** `cmd/chv-agent/src/main.rs` (Task 8)

If `ControlPlaneClient::new` fails on startup, `telemetry` is `None` forever. If a later `report_node_state` fails, the error is only logged.

**Fix:** Add a `// TODO(Phase 3): implement retry with exponential backoff and reconnection` comment in the telemetry block of `main.rs`.

---

### 12. No VM State Telemetry
**File/Line:** Plan overall

Per `chv-agent-spec.md`: "report inventory, health, events, and observed state". The plan only reports node state; it never sends `ReportVmState`.

**Fix:** Add a TODO in Task 8 or Task 5 noting that VM state telemetry (`ReportVmState`) is deferred to Phase 3.

---

## Summary

The plan is well-structured in terms of task ordering, but it contains **compilation-blocking errors** (`inner.vm.vm_id`, `store_fragment("node")`), **architectural violations** (mock in production, no node-state checks for VM creation), and **significant spec gaps** (no mTLS, no per-VM sockets, no operation IDs, no VM telemetry). Fix the critical issues above before approving for implementation.
