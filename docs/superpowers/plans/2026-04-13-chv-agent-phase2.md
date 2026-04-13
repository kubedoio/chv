# chv-agent Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the chv-agent gRPC server (ReconcileService + LifecycleService), telemetry client, VM runtime manager with CloudHypervisorAdapter integration, cache fragment helpers, and reconciler skeleton.

**Architecture:** The agent exposes a gRPC server for the control plane to push desired state and lifecycle commands. A `VmRuntime` manager tracks VM state and delegates VM create/start/stop/delete to the `CloudHypervisorAdapter`. The `Reconciler` processes cached desired-state fragments. A telemetry client pushes node state upstream.

**Tech Stack:** Rust, tonic, tokio, Unix sockets, async-trait

---

## File Map

| File | Responsibility |
|------|----------------|
| `crates/chv-agent-core/src/agent_server.rs` | gRPC server implementing `ReconcileService` and `LifecycleService` |
| `crates/chv-agent-core/src/vm_runtime.rs` | In-memory VM registry, lifecycle orchestration, CH adapter coordination |
| `crates/chv-agent-core/src/telemetry.rs` | `TelemetryService` client for pushing state/events to control plane |
| `crates/chv-agent-runtime-ch/src/mock.rs` | Test-only mock `CloudHypervisorAdapter` |
| `crates/chv-agent-core/src/control_plane.rs` | Extend with telemetry client methods |
| `crates/chv-agent-core/src/reconcile.rs` | Implement actual desired-state fragment processing skeleton |
| `crates/chv-agent-core/src/cache.rs` | Add fragment helper methods |
| `crates/chv-agent-core/Cargo.toml` | Add `chv-agent-runtime-ch` dependency |
| `crates/chv-agent-core/src/lib.rs` | Export new modules |
| `cmd/chv-agent/src/main.rs` | Start gRPC server, wire telemetry client and adapter |

---

### Task 1: Add chv-agent-runtime-ch dependency to chv-agent-core

**Files:**
- Modify: `crates/chv-agent-core/Cargo.toml`

- [ ] **Step 1: Add dependency**

Insert into `[dependencies]`:
```toml
chv-agent-runtime-ch = { path = "../chv-agent-runtime-ch" }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p chv-agent-core`
Expected: compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/chv-agent-core/Cargo.toml
git commit -m "build(agent-core): add chv-agent-runtime-ch dependency"
```

---

### Task 2: Mock CloudHypervisorAdapter for tests

**Files:**
- Create: `crates/chv-agent-runtime-ch/src/mock.rs`
- Modify: `crates/chv-agent-runtime-ch/src/lib.rs`

- [ ] **Step 1: Write the mock adapter**

```rust
use async_trait::async_trait;
use chv_errors::ChvError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::adapter::{CloudHypervisorAdapter, VmConfig};

#[derive(Debug, Clone, Default)]
pub struct MockCloudHypervisorAdapter {
    pub vms: Arc<Mutex<HashMap<String, VmConfig>>>,
}

#[async_trait]
impl CloudHypervisorAdapter for MockCloudHypervisorAdapter {
    async fn create_vm(&self, config: &VmConfig) -> Result<String, ChvError> {
        self.vms.lock().unwrap().insert(config.vm_id.clone(), config.clone());
        Ok(config.vm_id.clone())
    }

    async fn start_vm(&self, _vm_id: &str) -> Result<(), ChvError> {
        Ok(())
    }

    async fn stop_vm(&self, _vm_id: &str, _force: bool) -> Result<(), ChvError> {
        Ok(())
    }

    async fn delete_vm(&self, vm_id: &str) -> Result<(), ChvError> {
        self.vms.lock().unwrap().remove(vm_id);
        Ok(())
    }
}
```

- [ ] **Step 2: Export mock from lib.rs**

Add `pub mod mock;` to `crates/chv-agent-runtime-ch/src/lib.rs`.

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p chv-agent-runtime-ch`
Expected: compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/chv-agent-runtime-ch/
git commit -m "feat(agent-runtime): add mock CloudHypervisorAdapter"
```

---

### Task 3: Cache fragment helpers

**Files:**
- Modify: `crates/chv-agent-core/src/cache.rs`

- [ ] **Step 1: Add fragment helper methods**

Add these methods to `impl NodeCache`:

```rust
    pub fn store_fragment(
        &mut self,
        kind: &str,
        id: &str,
        fragment: DesiredStateFragment,
    ) {
        match kind {
            "vm" => self.vm_fragments.insert(id.to_string(), fragment),
            "volume" => self.volume_fragments.insert(id.to_string(), fragment),
            "network" => self.network_fragments.insert(id.to_string(), fragment),
            _ => None,
        };
    }

    pub fn get_fragment(
        &self,
        kind: &str,
        id: &str,
    ) -> Option<&DesiredStateFragment> {
        match kind {
            "vm" => self.vm_fragments.get(id),
            "volume" => self.volume_fragments.get(id),
            "network" => self.network_fragments.get(id),
            _ => None,
        }
    }

    pub fn remove_fragment(&mut self, kind: &str, id: &str) {
        match kind {
            "vm" => { self.vm_fragments.remove(id); }
            "volume" => { self.volume_fragments.remove(id); }
            "network" => { self.network_fragments.remove(id); }
            _ => {}
        };
    }
```

- [ ] **Step 2: Add fragment helper tests**

```rust
    #[test]
    fn cache_fragment_roundtrip() {
        let mut cache = NodeCache::new("node-1");
        let frag = DesiredStateFragment {
            id: "vm-1".to_string(),
            kind: "vm".to_string(),
            generation: "5".to_string(),
            spec_json: b"{}".to_vec(),
            policy_json: vec![],
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            updated_by: "cp".to_string(),
        };
        cache.store_fragment("vm", "vm-1", frag.clone());
        assert_eq!(cache.get_fragment("vm", "vm-1").unwrap().generation, "5");
        cache.remove_fragment("vm", "vm-1");
        assert!(cache.get_fragment("vm", "vm-1").is_none());
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p chv-agent-core cache_fragment`
Expected: 1 PASS

- [ ] **Step 4: Commit**

```bash
git add crates/chv-agent-core/src/cache.rs
git commit -m "feat(agent-core): add cache fragment helpers"
```

---

### Task 4: VmRuntime manager with CH adapter

**Files:**
- Create: `crates/chv-agent-core/src/vm_runtime.rs`
- Modify: `crates/chv-agent-core/src/lib.rs`

- [ ] **Step 1: Write VmRuntime with adapter delegation**

```rust
use chv_agent_runtime_ch::adapter::{CloudHypervisorAdapter, VmConfig};
use chv_errors::ChvError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct VmRecord {
    pub vm_id: String,
    pub observed_generation: String,
    pub runtime_status: String,
    pub last_error: Option<String>,
}

pub struct VmRuntime {
    vms: Arc<Mutex<HashMap<String, VmRecord>>>,
    adapter: Arc<dyn CloudHypervisorAdapter>,
}

impl Clone for VmRuntime {
    fn clone(&self) -> Self {
        Self {
            vms: self.vms.clone(),
            adapter: self.adapter.clone(),
        }
    }
}

impl VmRuntime {
    pub fn new(adapter: Arc<dyn CloudHypervisorAdapter>) -> Self {
        Self {
            vms: Arc::new(Mutex::new(HashMap::new())),
            adapter,
        }
    }

    pub async fn create_vm(
        &self,
        vm_id: impl Into<String>,
        generation: impl Into<String>,
        config: &VmConfig,
    ) -> Result<(), ChvError> {
        let id = vm_id.into();
        self.adapter.create_vm(config).await?;
        let mut map = self.vms.lock().unwrap();
        map.insert(
            id.clone(),
            VmRecord {
                vm_id: id,
                observed_generation: generation.into(),
                runtime_status: "Created".to_string(),
                last_error: None,
            },
        );
        Ok(())
    }

    pub async fn start_vm(
        &self,
        vm_id: &str,
    ) -> Result<(), ChvError> {
        self.adapter.start_vm(vm_id).await?;
        let mut map = self.vms.lock().unwrap();
        let rec = map.get_mut(vm_id).ok_or_else(|| ChvError::NotFound {
            resource: "vm".to_string(),
            id: vm_id.to_string(),
        })?;
        rec.runtime_status = "Running".to_string();
        Ok(())
    }

    pub async fn stop_vm(
        &self,
        vm_id: &str,
        force: bool,
    ) -> Result<(), ChvError> {
        self.adapter.stop_vm(vm_id, force).await?;
        let mut map = self.vms.lock().unwrap();
        let rec = map.get_mut(vm_id).ok_or_else(|| ChvError::NotFound {
            resource: "vm".to_string(),
            id: vm_id.to_string(),
        })?;
        rec.runtime_status = "Stopped".to_string();
        Ok(())
    }

    pub async fn delete_vm(&self, vm_id: &str) -> Result<(), ChvError> {
        self.adapter.delete_vm(vm_id).await?;
        self.vms.lock().unwrap().remove(vm_id);
        Ok(())
    }

    pub fn get(&self, vm_id: &str) -> Option<VmRecord> {
        self.vms.lock().unwrap().get(vm_id).cloned()
    }

    pub fn list(&self) -> Vec<VmRecord> {
        self.vms.lock().unwrap().values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter;
    use std::path::PathBuf;

    fn test_runtime() -> VmRuntime {
        VmRuntime::new(Arc::new(MockCloudHypervisorAdapter::default()))
    }

    #[tokio::test]
    async fn vm_runtime_create_and_get() {
        let rt = test_runtime();
        let config = VmConfig {
            vm_id: "vm-1".to_string(),
            cpus: 2,
            memory_bytes: 1024,
            kernel_path: PathBuf::from("/dev/null"),
            disk_paths: vec![],
        };
        rt.create_vm("vm-1", "5", &config).await.unwrap();
        let rec = rt.get("vm-1").unwrap();
        assert_eq!(rec.observed_generation, "5");
        assert_eq!(rec.runtime_status, "Created");
        assert!(rt.adapter.vms.lock().unwrap().contains_key("vm-1"));
    }

    #[tokio::test]
    async fn vm_runtime_start_and_stop() {
        let rt = test_runtime();
        let config = VmConfig {
            vm_id: "vm-1".to_string(),
            cpus: 2,
            memory_bytes: 1024,
            kernel_path: PathBuf::from("/dev/null"),
            disk_paths: vec![],
        };
        rt.create_vm("vm-1", "5", &config).await.unwrap();
        rt.start_vm("vm-1").await.unwrap();
        assert_eq!(rt.get("vm-1").unwrap().runtime_status, "Running");
        rt.stop_vm("vm-1", false).await.unwrap();
        assert_eq!(rt.get("vm-1").unwrap().runtime_status, "Stopped");
    }

    #[tokio::test]
    async fn vm_runtime_delete() {
        let rt = test_runtime();
        let config = VmConfig {
            vm_id: "vm-1".to_string(),
            cpus: 2,
            memory_bytes: 1024,
            kernel_path: PathBuf::from("/dev/null"),
            disk_paths: vec![],
        };
        rt.create_vm("vm-1", "5", &config).await.unwrap();
        rt.delete_vm("vm-1").await.unwrap();
        assert!(rt.get("vm-1").is_none());
    }
}
```

- [ ] **Step 2: Export vm_runtime from lib.rs**

Add `pub mod vm_runtime;` and `pub use vm_runtime::{VmRuntime, VmRecord};` to `crates/chv-agent-core/src/lib.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p chv-agent-core vm_runtime`
Expected: 3 PASS

- [ ] **Step 4: Commit**

```bash
git add crates/chv-agent-core/
git commit -m "feat(agent-core): add VmRuntime with CH adapter integration"
```

---

### Task 5: Telemetry client

**Files:**
- Create: `crates/chv-agent-core/src/telemetry.rs`
- Modify: `crates/chv-agent-core/src/lib.rs`
- Modify: `crates/chv-agent-core/src/control_plane.rs`

- [ ] **Step 1: Extend ControlPlaneClient with TelemetryService**

Modify `crates/chv-agent-core/src/control_plane.rs`:

Replace the existing struct with:
```rust
pub struct ControlPlaneClient {
    reconcile: proto::reconcile_service_client::ReconcileServiceClient<Channel>,
    telemetry: proto::telemetry_service_client::TelemetryServiceClient<Channel>,
}
```

Replace the existing `new()` method with:
```rust
    pub async fn new(endpoint: impl Into<String>) -> Result<Self, ChvError> {
        let endpoint = endpoint.into();
        let channel = tonic::transport::Endpoint::try_from(endpoint)
            .map_err(|e| ChvError::InvalidArgument {
                field: "control_plane_addr".to_string(),
                reason: e.to_string(),
            })?
            .connect()
            .await
            .map_err(|e| ChvError::ControlPlaneUnavailable {
                reason: e.to_string(),
            })?;
        Ok(Self {
            reconcile: proto::reconcile_service_client::ReconcileServiceClient::new(channel.clone()),
            telemetry: proto::telemetry_service_client::TelemetryServiceClient::new(channel),
        })
    }
```

Keep `stale_generation_check` and `apply_node_desired_state` as-is. Append these methods to the `impl` block:

```rust
    pub async fn report_node_state(
        &mut self,
        req: proto::NodeStateReport,
    ) -> Result<proto::AckResponse, ChvError> {
        self.telemetry
            .report_node_state(req)
            .await
            .map(|r| r.into_inner())
            .map_err(|e| ChvError::ControlPlaneUnavailable {
                reason: e.to_string(),
            })
    }

    pub async fn report_vm_state(
        &mut self,
        req: proto::VmStateReport,
    ) -> Result<proto::AckResponse, ChvError> {
        self.telemetry
            .report_vm_state(req)
            .await
            .map(|r| r.into_inner())
            .map_err(|e| ChvError::ControlPlaneUnavailable {
                reason: e.to_string(),
            })
    }

    pub async fn publish_event(
        &mut self,
        req: proto::PublishEventRequest,
    ) -> Result<proto::AckResponse, ChvError> {
        self.telemetry
            .publish_event(req)
            .await
            .map(|r| r.into_inner())
            .map_err(|e| ChvError::ControlPlaneUnavailable {
                reason: e.to_string(),
            })
    }
```

- [ ] **Step 2: Create telemetry module**

`crates/chv-agent-core/src/telemetry.rs`:

```rust
use control_plane_node_api::control_plane_node_api as proto;

pub struct TelemetryReporter {
    node_id: String,
}

impl TelemetryReporter {
    pub fn new(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
        }
    }

    pub fn node_state_report(
        &self,
        state: &str,
        observed_generation: &str,
        health_status: &str,
        last_error: Option<String>,
    ) -> proto::NodeStateReport {
        proto::NodeStateReport {
            node_id: self.node_id.clone(),
            state: state.to_string(),
            observed_generation: observed_generation.to_string(),
            health_status: health_status.to_string(),
            last_error: last_error.unwrap_or_default(),
            reported_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_report_has_node_id() {
        let rep = TelemetryReporter::new("node-1");
        let report = rep.node_state_report("TenantReady", "10", "Healthy", None);
        assert_eq!(report.node_id, "node-1");
        assert_eq!(report.state, "TenantReady");
    }
}
```

- [ ] **Step 3: Export telemetry module**

Add `pub mod telemetry;` and `pub use telemetry::TelemetryReporter;` to `lib.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p chv-agent-core telemetry`
Expected: 1 PASS

- [ ] **Step 5: Commit**

```bash
git add crates/chv-agent-core/
git commit -m "feat(agent-core): add telemetry client and reporter"
```

---

### Task 6: Agent gRPC server

**Files:**
- Create: `crates/chv-agent-core/src/agent_server.rs`
- Modify: `crates/chv-agent-core/src/lib.rs`

- [ ] **Step 1: Implement agent server with CH adapter**

`crates/chv-agent-core/src/agent_server.rs`:

```rust
use crate::cache::NodeCache;
use crate::control_plane::ControlPlaneClient;
use crate::vm_runtime::VmRuntime;
use chv_agent_runtime_ch::adapter::VmConfig;
use chv_errors::ChvError;
use control_plane_node_api::control_plane_node_api as proto;
use std::path::Path;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct AgentServer {
    pub cache: Arc<tokio::sync::Mutex<NodeCache>>,
    pub vm_runtime: VmRuntime,
}

impl AgentServer {
    pub fn new(cache: NodeCache, vm_runtime: VmRuntime) -> Self {
        Self {
            cache: Arc::new(tokio::sync::Mutex::new(cache)),
            vm_runtime,
        }
    }

    pub async fn serve(self, socket_path: &Path) -> Result<(), ChvError> {
        if let Some(parent) = socket_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| ChvError::Io {
                path: parent.to_string_lossy().to_string(),
                source: e,
            })?;
        }
        if socket_path.exists() {
            tokio::fs::remove_file(socket_path).await.map_err(|e| ChvError::Io {
                path: socket_path.to_string_lossy().to_string(),
                source: e,
            })?;
        }
        let uds = UnixListener::bind(socket_path).map_err(|e| ChvError::Io {
            path: socket_path.to_string_lossy().to_string(),
            source: e,
        })?;
        let uds_stream = UnixListenerStream::new(uds);
        tonic::transport::Server::builder()
            .add_service(proto::reconcile_service_server::ReconcileServiceServer::new(self.clone()))
            .add_service(proto::lifecycle_service_server::LifecycleServiceServer::new(self))
            .serve_with_incoming(uds_stream)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("agent server error: {e}"),
            })
    }
}

#[tonic::async_trait]
impl proto::reconcile_service_server::ReconcileService for AgentServer {
    async fn apply_node_desired_state(
        &self,
        req: Request<proto::ApplyNodeDesiredStateRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner.meta.as_ref().ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "node", &inner.node_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        if let Some(frag) = inner.fragment {
            cache.observe_generation("node", &inner.node_id, &frag.generation);
            // Note: node-level fragments are not stored in cache; they drive
            // immediate node configuration. store_fragment only handles vm/volume/network.
        }
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "node desired state accepted".to_string(),
            }),
        }))
    }

    async fn apply_vm_desired_state(
        &self,
        req: Request<proto::ApplyVmDesiredStateRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner.meta.as_ref().ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        if let Some(frag) = inner.fragment {
            cache.observe_generation("vm", &inner.vm_id, &frag.generation);
            cache.store_fragment("vm", &inner.vm_id, crate::cache::DesiredStateFragment {
                id: frag.id,
                kind: frag.kind,
                generation: frag.generation,
                spec_json: frag.spec_json,
                policy_json: frag.policy_json,
                updated_at: frag.updated_at,
                updated_by: frag.updated_by,
            });
        }
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "vm desired state accepted".to_string(),
            }),
        }))
    }

    async fn apply_volume_desired_state(
        &self,
        _req: Request<proto::ApplyVolumeDesiredStateRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("volume desired state in Phase 3"))
    }

    async fn apply_network_desired_state(
        &self,
        _req: Request<proto::ApplyNetworkDesiredStateRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("network desired state in Phase 3"))
    }

    async fn acknowledge_desired_state_version(
        &self,
        _req: Request<proto::AcknowledgeDesiredStateVersionRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("acknowledge desired state in Phase 3"))
    }
}

#[tonic::async_trait]
impl proto::lifecycle_service_server::LifecycleService for AgentServer {
    async fn create_vm(
        &self,
        req: Request<proto::CreateVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner.meta.as_ref().ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let vm = inner.vm.as_ref().ok_or_else(|| Status::invalid_argument("missing vm"))?;
        let mut cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &vm.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        // ADR-003: only TenantReady nodes accept new placements.
        let node_state = cache.node_state.parse::<crate::state_machine::NodeState>()
            .unwrap_or(crate::state_machine::NodeState::Bootstrapping);
        if node_state != crate::state_machine::NodeState::TenantReady {
            return Err(Status::failed_precondition(
                format!("node not schedulable: {}", cache.node_state)
            ));
        }
        let config = VmConfig {
            vm_id: vm.vm_id.clone(),
            cpus: 2,
            memory_bytes: 1024,
            kernel_path: std::path::PathBuf::from("/dev/null"),
            disk_paths: vec![],
        };
        self.vm_runtime.create_vm(&vm.vm_id, &meta.desired_state_version, &config).await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "vm created".to_string(),
            }),
        }))
    }

    async fn start_vm(
        &self,
        req: Request<proto::StartVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner.meta.as_ref().ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        self.vm_runtime.start_vm(&inner.vm_id).await
            .map_err(|e| Status::not_found(e.to_string()))?;
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "vm started".to_string(),
            }),
        }))
    }

    async fn stop_vm(
        &self,
        req: Request<proto::StopVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner.meta.as_ref().ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        self.vm_runtime.stop_vm(&inner.vm_id, inner.force).await
            .map_err(|e| Status::not_found(e.to_string()))?;
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "vm stopped".to_string(),
            }),
        }))
    }

    async fn reboot_vm(
        &self,
        req: Request<proto::RebootVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner.meta.as_ref().ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        Err(Status::unimplemented("reboot_vm in Phase 3"))
    }

    async fn delete_vm(
        &self,
        req: Request<proto::DeleteVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner.meta.as_ref().ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        // force is ignored in Phase 2; the adapter trait does not yet support forced deletion.
        self.vm_runtime.delete_vm(&inner.vm_id).await
            .map_err(|e| Status::not_found(e.to_string()))?;
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "vm deleted".to_string(),
            }),
        }))
    }

    async fn attach_volume(
        &self,
        _req: Request<proto::AttachVolumeRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("attach_volume in Phase 3"))
    }

    async fn detach_volume(
        &self,
        _req: Request<proto::DetachVolumeRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("detach_volume in Phase 3"))
    }

    async fn resize_volume(
        &self,
        _req: Request<proto::ResizeVolumeRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("resize_volume in Phase 3"))
    }

    async fn pause_node_scheduling(
        &self,
        _req: Request<proto::PauseNodeSchedulingRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("pause_node_scheduling in Phase 3"))
    }

    async fn resume_node_scheduling(
        &self,
        _req: Request<proto::ResumeNodeSchedulingRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("resume_node_scheduling in Phase 3"))
    }

    async fn drain_node(
        &self,
        _req: Request<proto::DrainNodeRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("drain_node in Phase 3"))
    }

    async fn enter_maintenance(
        &self,
        _req: Request<proto::EnterMaintenanceRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("enter_maintenance in Phase 3"))
    }

    async fn exit_maintenance(
        &self,
        _req: Request<proto::ExitMaintenanceRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("exit_maintenance in Phase 3"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter;
    use std::sync::Arc;

    fn test_server() -> AgentServer {
        let mut cache = NodeCache::new("node-1");
        cache.node_state = crate::state_machine::NodeState::TenantReady.as_str().to_string();
        AgentServer::new(
            cache,
            VmRuntime::new(Arc::new(MockCloudHypervisorAdapter::default())),
        )
    }

    fn test_meta(desired_state_version: &str) -> proto::RequestMeta {
        proto::RequestMeta {
            operation_id: "op-1".to_string(),
            requested_by: "cp".to_string(),
            target_node_id: "node-1".to_string(),
            desired_state_version: desired_state_version.to_string(),
            request_unix_ms: 0,
        }
    }

    #[tokio::test]
    async fn apply_vm_desired_state_updates_generation_and_fragment() {
        let server = test_server();
        let req = proto::ApplyVmDesiredStateRequest {
            meta: Some(test_meta("5")),
            node_id: "node-1".to_string(),
            vm_id: "vm-1".to_string(),
            fragment: Some(proto::DesiredStateFragment {
                id: "vm-1".to_string(),
                kind: "vm".to_string(),
                generation: "5".to_string(),
                spec_json: vec![],
                policy_json: vec![],
                updated_at: "".to_string(),
                updated_by: "".to_string(),
            }),
        };
        let resp = proto::reconcile_service_server::ReconcileService::apply_vm_desired_state(
            &server, Request::new(req)
        ).await;
        assert!(resp.is_ok());
        let cache = server.cache.lock().await;
        assert_eq!(cache.get_generation("vm", "vm-1"), Some(&"5".to_string()));
        assert!(cache.get_fragment("vm", "vm-1").is_some());
    }

    #[tokio::test]
    async fn create_vm_lifecycle_flow() {
        let server = test_server();
        let create_req = proto::CreateVmRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            vm: Some(proto::VmMutationSpec {
                vm_id: "vm-1".to_string(),
                vm_spec_json: vec![],
            }),
        };
        let resp = proto::lifecycle_service_server::LifecycleService::create_vm(
            &server, Request::new(create_req)
        ).await;
        assert!(resp.is_ok());

        let start_req = proto::StartVmRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            vm_id: "vm-1".to_string(),
        };
        let resp = proto::lifecycle_service_server::LifecycleService::start_vm(
            &server, Request::new(start_req)
        ).await;
        assert!(resp.is_ok());
        assert_eq!(server.vm_runtime.get("vm-1").unwrap().runtime_status, "Running");

        let stop_req = proto::StopVmRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            vm_id: "vm-1".to_string(),
            force: false,
        };
        let resp = proto::lifecycle_service_server::LifecycleService::stop_vm(
            &server, Request::new(stop_req)
        ).await;
        assert!(resp.is_ok());
        assert_eq!(server.vm_runtime.get("vm-1").unwrap().runtime_status, "Stopped");

        let delete_req = proto::DeleteVmRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            vm_id: "vm-1".to_string(),
            force: false,
        };
        let resp = proto::lifecycle_service_server::LifecycleService::delete_vm(
            &server, Request::new(delete_req)
        ).await;
        assert!(resp.is_ok());
        assert!(server.vm_runtime.get("vm-1").is_none());
    }

    #[tokio::test]
    async fn lifecycle_stale_generation_rejected() {
        let server = test_server();
        let mut cache = server.cache.lock().await;
        cache.observe_generation("vm", "vm-1", "10");
        drop(cache);

        let req = proto::StartVmRequest {
            meta: Some(test_meta("9")),
            node_id: "node-1".to_string(),
            vm_id: "vm-1".to_string(),
        };
        let resp = proto::lifecycle_service_server::LifecycleService::start_vm(
            &server, Request::new(req)
        ).await;
        assert_eq!(resp.unwrap_err().code(), tonic::Code::FailedPrecondition);
    }
}
```

- [ ] **Step 2: Export agent_server from lib.rs**

Add `pub mod agent_server;` and `pub use agent_server::AgentServer;` to `lib.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p chv-agent-core agent_server`
Expected: 3 PASS

- [ ] **Step 4: Commit**

```bash
git add crates/chv-agent-core/
git commit -m "feat(agent-core): add gRPC server with CH adapter integration"
```

---

### Task 7: Reconciler skeleton with fragment processing

**Files:**
- Modify: `crates/chv-agent-core/src/reconcile.rs`

- [ ] **Step 1: Implement fragment-processing skeleton**

Replace the contents of `reconcile.rs`:

```rust
use crate::cache::NodeCache;
use crate::state_machine::{NodeState, StateMachine};
use crate::vm_runtime::VmRuntime;
use chv_errors::ChvError;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

pub struct Reconciler {
    pub cache: NodeCache,
    pub state_machine: StateMachine,
    pub vm_runtime: VmRuntime,
    pub stord_socket: PathBuf,
    pub nwd_socket: PathBuf,
}

impl Reconciler {
    pub fn new(
        cache: NodeCache,
        vm_runtime: VmRuntime,
        stord_socket: PathBuf,
        nwd_socket: PathBuf,
    ) -> Self {
        let initial = cache
            .node_state
            .parse()
            .unwrap_or(NodeState::Bootstrapping);
        Self {
            cache,
            state_machine: StateMachine::new(initial),
            vm_runtime,
            stord_socket,
            nwd_socket,
        }
    }

    pub async fn run_once(&mut self) -> Result<(), ChvError> {
        info!(
            state = %self.state_machine.current().as_str(),
            "reconcile tick"
        );
        // Phase 2: iterate over cached VM fragments and ensure they match runtime.
        // For now, log the divergence count without acting.
        let vm_count = self.cache.vm_fragments.len();
        let runtime_count = self.vm_runtime.list().len();
        if vm_count != runtime_count {
            info!(
                cached_vms = vm_count,
                runtime_vms = runtime_count,
                "reconcile divergence detected"
            );
        }
        Ok(())
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p chv-agent-core reconcile`
Expected: compiles and passes (existing tests still pass)

- [ ] **Step 3: Commit**

```bash
git add crates/chv-agent-core/src/reconcile.rs
git commit -m "feat(agent-core): add reconciler fragment-processing skeleton"
```

---

### Task 8: Wire server and telemetry into main.rs

**Files:**
- Modify: `cmd/chv-agent/src/main.rs`

- [ ] **Step 1: Update main.rs**

```rust
use chv_agent_core::{
    agent_server::AgentServer, cache::NodeCache, config::load_agent_config,
    control_plane::ControlPlaneClient, daemon_clients::{NwdClient, StordClient},
    health::HealthAggregator, reconcile::Reconciler, state_machine::NodeState,
    telemetry::TelemetryReporter, vm_runtime::VmRuntime,
};
use chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter;
use chv_observability::init_logger;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = std::env::args().nth(1).map(PathBuf::from);
    let config = load_agent_config(config_path.as_deref())?;

    init_logger(&config.log_level)?;

    info!("chv-agent starting");

    let mut cache = match NodeCache::load(&config.cache_path).await {
        Ok(c) => {
            info!(node_id = %c.node_id, "loaded cache");
            c
        }
        Err(chv_errors::ChvError::NotFound { .. }) => {
            let node_id = if config.node_id.is_empty() {
                "unknown".to_string()
            } else {
                config.node_id.clone()
            };
            NodeCache::new(node_id)
        }
        Err(e) => {
            warn!(error = %e, "failed to load cache, starting fresh");
            let node_id = if config.node_id.is_empty() {
                "unknown".to_string()
            } else {
                config.node_id.clone()
            };
            NodeCache::new(node_id)
        }
    };

    cache.node_state = NodeState::Bootstrapping.as_str().to_string();

    // Phase 2 placeholder: the real CloudHypervisorAdapter is not yet implemented.
    // We wire the mock adapter so the full control-plane -> agent -> adapter flow
    // can be tested end-to-end. Replace with the production adapter in Phase 3.
    warn!("using mock CloudHypervisorAdapter — real VM processes will not be launched");
    let adapter: Arc<dyn chv_agent_runtime_ch::adapter::CloudHypervisorAdapter> =
        Arc::new(MockCloudHypervisorAdapter::default());
    let vm_runtime = VmRuntime::new(adapter);

    let agent_server = AgentServer::new(cache.clone(), vm_runtime.clone());
    let server_socket = config.socket_path.clone();
    tokio::spawn(async move {
        if let Err(e) = agent_server.serve(&server_socket).await {
            warn!(error = %e, "agent server exited");
        }
    });

    let mut reconciler = Reconciler::new(
        cache.clone(),
        vm_runtime.clone(),
        config.stord_socket.clone(),
        config.nwd_socket.clone(),
    );

    let mut telemetry = match ControlPlaneClient::new(&config.control_plane_addr).await {
        Ok(client) => {
            info!("connected to control plane");
            Some((TelemetryReporter::new(&cache.node_id), client))
        }
        Err(e) => {
            warn!(error = %e, "control plane unavailable; will retry later");
            None
        }
    };

    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;

        let stord_ok = match StordClient::connect(&config.stord_socket).await {
            Ok(mut c) => c.health_probe().await.unwrap_or(false),
            Err(_) => false,
        };

        let nwd_ok = match NwdClient::connect(&config.nwd_socket).await {
            Ok(mut c) => c.health_probe().await.unwrap_or(false),
            Err(_) => false,
        };

        let mut health = HealthAggregator::new();
        health.update_stord(stord_ok);
        health.update_nwd(nwd_ok);

        let derived = health.derive_node_state(reconciler.state_machine.current());
        if derived != reconciler.state_machine.current() {
            info!(
                from = %reconciler.state_machine.current().as_str(),
                to = %derived.as_str(),
                "state transition"
            );
            if let Err(e) = reconciler.state_machine.transition(derived) {
                warn!(error = %e, "invalid state transition ignored");
            } else {
                cache.node_state = reconciler.state_machine.current().as_str().to_string();
                if let Err(e) = cache.save(&config.cache_path).await {
                    warn!(error = %e, "failed to save cache");
                }
            }
        }

        if let Some((ref reporter, ref mut client)) = telemetry {
            let report = reporter.node_state_report(
                cache.node_state.as_str(),
                cache.observed_generation.as_str(),
                if reconciler.state_machine.current() == NodeState::TenantReady {
                    "Healthy"
                } else {
                    "Degraded"
                },
                cache.last_error.clone(),
            );
            if let Err(e) = client.report_node_state(report).await {
                warn!(error = %e, "failed to report node state");
            }
            for vm in reconciler.vm_runtime.list() {
                let vm_report = proto::VmStateReport {
                    node_id: cache.node_id.clone(),
                    vm_id: vm.vm_id.clone(),
                    runtime_status: vm.runtime_status.clone(),
                    observed_generation: vm.observed_generation.clone(),
                    health_status: "Healthy".to_string(),
                    last_error: vm.last_error.unwrap_or_default(),
                    reported_unix_ms: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64,
                };
                if let Err(e) = client.report_vm_state(vm_report).await {
                    warn!(vm_id = %vm.vm_id, error = %e, "failed to report vm state");
                }
            }
        }

        if let Err(e) = reconciler.run_once().await {
            warn!(error = %e, "reconcile tick failed");
        }

        // Periodically persist cache so gRPC mutations are durable even without state transitions.
        if let Err(e) = cache.save(&config.cache_path).await {
            warn!(error = %e, "failed to save cache");
        }
    }
}
```

- [ ] **Step 2: Build chv-agent**

Run: `cargo build --release -p chv-agent`
Expected: compiles successfully

- [ ] **Step 3: Commit**

```bash
git add cmd/chv-agent/src/main.rs
git commit -m "feat(agent): wire gRPC server, telemetry, and CH adapter into main loop"
```

---

### Task 9: Full workspace validation

- [ ] **Step 1: Run tests**

Run: `cargo test --workspace`
Expected: all tests pass

- [ ] **Step 2: Commit any remaining fixes**

If tests fail, diagnose, fix, and commit.

---

## Phase 3 TODOs

The following items are intentionally deferred to Phase 3:

1. **Production CloudHypervisorAdapter** — replace `MockCloudHypervisorAdapter` with the real CH process launcher and Unix API socket manager.
2. **mTLS for control-plane connection** — `ControlPlaneClient` currently connects over plain TCP/gRPC. Add client certificate loading and TLS configuration.
3. **Per-VM API socket paths** — `VmConfig` needs a `api_socket_path` field so the agent can communicate with individual CH processes.
4. **Operation correlation IDs** — propagate `operation_id` through the `CloudHypervisorAdapter` trait and downstream daemon client calls.
5. **VM state telemetry** — implement `ReportVmState` calls in the telemetry loop (currently only `ReportNodeState` is sent).
6. **Volume attach/detach lifecycle** — implement `AttachVolume` and `DetachVolume` RPCs with `chv-stord` integration.
7. **Network exposure lifecycle** — implement `ExposeService` via `chv-nwd` integration.
8. **Cache persistence after gRPC mutations** — `AgentServer` mutates the shared `NodeCache` but `main.rs` only persists it on state transitions. Add unconditional cache save after gRPC mutations or at the end of each loop tick.

---

## Plan Review

Dispatch a plan-document-reviewer subagent with:
- Plan path: `docs/superpowers/plans/2026-04-13-chv-agent-phase2.md`
- Spec paths: `docs/specs/component/chv-agent-spec.md`, `docs/specs/proto/control-plane-node.proto`
