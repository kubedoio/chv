use crate::ebpf::{self, EbpfManager};
use crate::executor::{NetworkExecutor, OverlayStatusInfo, TopologyApplyResult};
use crate::reconcile;
use crate::state::{TopologyState, TopologyTable};
use chv_errors::ChvError;
use chv_nwd_api::chv_nwd_api as proto;
use chv_observability::{operation_span, Metrics};
use dashmap::DashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::info;

pub struct NetworkServiceImpl<E: NetworkExecutor> {
    executor: Arc<E>,
    topologies: Arc<TopologyTable>,
    metrics: Arc<Metrics>,
    security_policies: Arc<DashMap<String, proto::SecurityPolicy>>,
    rate_limit_policies: Arc<DashMap<String, proto::RateLimitPolicy>>,
    ebpf: Arc<dyn EbpfManager>,
    /// Counter tracking eBPF program load failures.
    ebpf_load_failures: Arc<AtomicU32>,
}

impl<E: NetworkExecutor> NetworkServiceImpl<E> {
    pub fn new(executor: Arc<E>, topologies: Arc<TopologyTable>, metrics: Arc<Metrics>) -> Self {
        Self {
            executor,
            topologies,
            metrics,
            security_policies: Arc::new(DashMap::new()),
            rate_limit_policies: Arc::new(DashMap::new()),
            ebpf: Arc::new(ebpf::NoopEbpfManager),
            ebpf_load_failures: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn topologies(&self) -> Arc<TopologyTable> {
        self.topologies.clone()
    }

    fn ok_result() -> proto::Result {
        let (status, error_code, human_summary) = ChvError::ok_result_fields();
        proto::Result {
            status: status.to_string(),
            error_code: error_code.to_string(),
            human_summary,
        }
    }

    fn err_result(e: &ChvError) -> proto::Result {
        let (status, error_code, human_summary) = e.to_result_fields();
        proto::Result {
            status: status.to_string(),
            error_code: error_code.to_string(),
            human_summary,
        }
    }

    fn map_topology_spec(t: Option<proto::TopologySpec>) -> Result<proto::TopologySpec, ChvError> {
        t.ok_or_else(|| ChvError::InvalidArgument {
            field: "topology".to_string(),
            reason: "missing".to_string(),
        })
    }
}

#[tonic::async_trait]
impl<E: NetworkExecutor> proto::network_service_server::NetworkService for NetworkServiceImpl<E> {
    async fn ensure_network_topology(
        &self,
        request: Request<proto::EnsureNetworkTopologyRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        self.metrics
            .increment_counter("nwd_ensure_network_topology_total");
        let req = request.into_inner();
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

        let spec = match Self::map_topology_spec(req.topology) {
            Ok(s) => s,
            Err(e) => return Ok(Response::new(Self::err_result(&e))),
        };

        // IFNAMSIZ limit: Linux interface names must be <= 15 bytes
        if spec.bridge_name.len() > 15 {
            let e = ChvError::InvalidArgument {
                field: "bridge_name".to_string(),
                reason: format!(
                    "exceeds IFNAMSIZ limit (15 chars): '{}' is {} chars",
                    spec.bridge_name,
                    spec.bridge_name.len()
                ),
            };
            return Ok(Response::new(Self::err_result(&e)));
        }

        // VNI range validation: VXLAN VNI is a 24-bit field (max 16777215)
        if spec.vni > 16_777_215 {
            let e = ChvError::InvalidArgument {
                field: "vni".to_string(),
                reason: format!("VNI {} exceeds maximum 16777215", spec.vni),
            };
            return Ok(Response::new(Self::err_result(&e)));
        }

        // Idempotency: if already ensured with same network_id, return OK
        if let Some(existing) = self.topologies.get(&spec.network_id) {
            if existing.bridge_name == spec.bridge_name
                && existing.namespace_name == spec.namespace_name
                && existing.subnet_cidr == spec.subnet_cidr
                && existing.gateway_ip == spec.gateway_ip
            {
                return Ok(Response::new(Self::ok_result()));
            }
        }

        let result = self.executor.ensure_topology(&spec).await;
        match result {
            Ok(TopologyApplyResult {
                namespace_handle: _,
                bridge_handle: _,
            }) => {
                let vni = if spec.vni > 0 { Some(spec.vni) } else { None };
                let peer_vteps: Vec<String> = spec
                    .vtep_endpoints
                    .iter()
                    .map(|e| e.vtep_ip.clone())
                    .collect();
                let state = TopologyState {
                    network_id: spec.network_id.clone(),
                    tenant_id: spec.tenant_id.clone(),
                    bridge_name: spec.bridge_name.clone(),
                    namespace_name: spec.namespace_name.clone(),
                    subnet_cidr: spec.subnet_cidr.clone(),
                    gateway_ip: spec.gateway_ip.clone(),
                    runtime_status: "ensured".to_string(),
                    vni,
                    peer_vteps,
                };
                self.topologies.upsert(state.clone());
                Ok(Response::new(Self::ok_result()))
            }
            Err(e) => Ok(Response::new(Self::err_result(&e))),
        }
    }

    async fn delete_network_topology(
        &self,
        request: Request<proto::DeleteNetworkTopologyRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        self.metrics
            .increment_counter("nwd_delete_network_topology_total");
        let req = request.into_inner();
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

        if let Some(state) = self.topologies.get(&req.network_id) {
            if let Err(e) = self.executor.delete_topology(&req.network_id, &state).await {
                return Ok(Response::new(Self::err_result(&e)));
            }
            self.topologies.remove(&req.network_id);
        }

        Ok(Response::new(Self::ok_result()))
    }

    async fn get_network_health(
        &self,
        request: Request<proto::NetworkHealthRequest>,
    ) -> Result<Response<proto::NetworkHealthResponse>, Status> {
        let req = request.into_inner();

        let (status, last_error) = if let Some(state) = self.topologies.get(&req.network_id) {
            match self.executor.health(&req.network_id, &state).await {
                Ok(s) => {
                    // Downgrade health if eBPF has load failures
                    let ebpf_failures = self.ebpf_load_failures.load(Ordering::Relaxed);
                    if ebpf_failures > 0 && s == "healthy" {
                        (
                            "degraded".to_string(),
                            format!(
                                "eBPF load failures: {}; traffic may pass unfiltered",
                                ebpf_failures
                            ),
                        )
                    } else {
                        (s, String::new())
                    }
                }
                Err(e) => ("unhealthy".to_string(), e.to_string()),
            }
        } else {
            ("unknown".to_string(), String::new())
        };

        Ok(Response::new(proto::NetworkHealthResponse {
            result: Some(Self::ok_result()),
            network_id: req.network_id,
            health_status: status,
            last_error,
        }))
    }

    async fn list_namespace_state(
        &self,
        _request: Request<proto::ListNamespaceStateRequest>,
    ) -> Result<Response<proto::ListNamespaceStateResponse>, Status> {
        let items: Vec<proto::NamespaceState> = self
            .topologies
            .list()
            .into_iter()
            .map(|s| proto::NamespaceState {
                network_id: s.network_id,
                namespace_name: s.namespace_name,
                bridge_name: s.bridge_name,
                runtime_status: s.runtime_status,
            })
            .collect();

        Ok(Response::new(proto::ListNamespaceStateResponse { items }))
    }

    async fn attach_vm_nic(
        &self,
        request: Request<proto::AttachVmNicRequest>,
    ) -> Result<Response<proto::AttachVmNicResponse>, Status> {
        self.metrics.increment_counter("nwd_attach_vm_nic_total");
        let req = request.into_inner();
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

        let nic = req.nic.ok_or_else(|| ChvError::InvalidArgument {
            field: "nic".to_string(),
            reason: "missing".to_string(),
        });
        let nic = match nic {
            Ok(n) => n,
            Err(e) => {
                return Ok(Response::new(proto::AttachVmNicResponse {
                    result: Some(Self::err_result(&e)),
                    namespace_handle: String::new(),
                    tap_handle: String::new(),
                }));
            }
        };

        let state = match self.topologies.get(&nic.network_id) {
            Some(s) => s,
            None => {
                let e = ChvError::NotFound {
                    resource: "topology".to_string(),
                    id: nic.network_id.clone(),
                };
                return Ok(Response::new(proto::AttachVmNicResponse {
                    result: Some(Self::err_result(&e)),
                    namespace_handle: String::new(),
                    tap_handle: String::new(),
                }));
            }
        };

        match self
            .executor
            .attach_vm_nic(
                &nic.network_id,
                &nic.nic_id,
                &nic.vm_id,
                &state.bridge_name,
                &nic.mac_address,
                &nic.ip_address,
            )
            .await
        {
            Ok((namespace_handle, tap_handle)) => {
                // Auto-load eBPF programs on the new tap interface — MANDATORY for tenant isolation
                let bridge_name = format!("br-{}", &nic.network_id[..nic.network_id.len().min(8)]);
                if let Err(e) = self.ebpf.load_policy_program(&tap_handle).await {
                    tracing::error!(tap = %tap_handle, error = %e, "eBPF policy load failed — refusing NIC attach (default-deny)");
                    self.ebpf_load_failures.fetch_add(1, Ordering::Relaxed);
                    // Detach the NIC we just attached to avoid dangling resources
                    let _ = self
                        .executor
                        .detach_vm_nic(
                            &nic.nic_id,
                            chv_common::AttachmentOwnership {
                                vm_id: nic.vm_id.clone(),
                                operation_id: req.meta.as_ref().map(|m| m.operation_id.clone()),
                                requester: None,
                            },
                        )
                        .await;
                    let err = ChvError::Internal {
                        reason: format!(
                            "eBPF policy program failed to load on tap {tap_handle}: {e}"
                        ),
                    };
                    return Ok(Response::new(proto::AttachVmNicResponse {
                        result: Some(Self::err_result(&err)),
                        namespace_handle: String::new(),
                        tap_handle: String::new(),
                    }));
                }
                if let Err(e) = self.ebpf.load_ingress_program(&bridge_name).await {
                    tracing::error!(bridge = %bridge_name, error = %e, "eBPF ingress load failed — refusing NIC attach (default-deny)");
                    self.ebpf_load_failures.fetch_add(1, Ordering::Relaxed);
                    // Detach the NIC we just attached to avoid dangling resources
                    let _ = self
                        .executor
                        .detach_vm_nic(
                            &nic.nic_id,
                            chv_common::AttachmentOwnership {
                                vm_id: nic.vm_id.clone(),
                                operation_id: req.meta.as_ref().map(|m| m.operation_id.clone()),
                                requester: None,
                            },
                        )
                        .await;
                    let err = ChvError::Internal {
                        reason: format!(
                            "eBPF ingress program failed to load on bridge {bridge_name}: {e}"
                        ),
                    };
                    return Ok(Response::new(proto::AttachVmNicResponse {
                        result: Some(Self::err_result(&err)),
                        namespace_handle: String::new(),
                        tap_handle: String::new(),
                    }));
                }

                Ok(Response::new(proto::AttachVmNicResponse {
                    result: Some(Self::ok_result()),
                    namespace_handle,
                    tap_handle,
                }))
            }
            Err(e) => Ok(Response::new(proto::AttachVmNicResponse {
                result: Some(Self::err_result(&e)),
                namespace_handle: String::new(),
                tap_handle: String::new(),
            })),
        }
    }

    async fn detach_vm_nic(
        &self,
        request: Request<proto::DetachVmNicRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        self.metrics.increment_counter("nwd_detach_vm_nic_total");
        let req = request.into_inner();
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

        match self
            .executor
            .detach_vm_nic(
                &req.nic_id,
                chv_common::AttachmentOwnership {
                    vm_id: req.vm_id.clone(),
                    operation_id: req.meta.as_ref().map(|m| m.operation_id.clone()),
                    requester: None,
                },
            )
            .await
        {
            Ok(()) => {
                // Clean up FDB entries for this VM's MAC across peer VTEPs
                if !req.network_id.is_empty() && !req.vm_mac.is_empty() {
                    if let Some(state) = self.topologies.get(&req.network_id) {
                        if let Some(vni) = state.vni {
                            for vtep_ip in &state.peer_vteps {
                                if let Err(e) = self
                                    .executor
                                    .delete_fdb_entry(
                                        &state.namespace_name,
                                        vni,
                                        &req.vm_mac,
                                        vtep_ip,
                                    )
                                    .await
                                {
                                    tracing::warn!(
                                        vm_mac = %req.vm_mac,
                                        vtep_ip = %vtep_ip,
                                        error = %e,
                                        "failed to delete FDB entry on detach"
                                    );
                                }
                            }
                        }
                    }
                }
                Ok(Response::new(Self::ok_result()))
            }
            Err(e) => Ok(Response::new(Self::err_result(&e))),
        }
    }

    async fn set_firewall_policy(
        &self,
        request: Request<proto::SetFirewallPolicyRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        self.metrics
            .increment_counter("nwd_set_firewall_policy_total");
        let req = request.into_inner();
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

        let policy = req.policy.ok_or_else(|| ChvError::InvalidArgument {
            field: "policy".to_string(),
            reason: "missing".to_string(),
        });
        let policy = match policy {
            Ok(p) => p,
            Err(e) => return Ok(Response::new(Self::err_result(&e))),
        };

        match self
            .executor
            .set_firewall_policy(&req.network_id, &policy.policy_version, &policy.policy_json)
            .await
        {
            Ok(()) => Ok(Response::new(Self::ok_result())),
            Err(e) => Ok(Response::new(Self::err_result(&e))),
        }
    }

    async fn set_nat_policy(
        &self,
        request: Request<proto::SetNatPolicyRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        self.metrics.increment_counter("nwd_set_nat_policy_total");
        let req = request.into_inner();
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

        let policy = req.policy.ok_or_else(|| ChvError::InvalidArgument {
            field: "policy".to_string(),
            reason: "missing".to_string(),
        });
        let policy = match policy {
            Ok(p) => p,
            Err(e) => return Ok(Response::new(Self::err_result(&e))),
        };

        match self
            .executor
            .set_nat_policy(&req.network_id, &policy.policy_version, &policy.policy_json)
            .await
        {
            Ok(()) => Ok(Response::new(Self::ok_result())),
            Err(e) => Ok(Response::new(Self::err_result(&e))),
        }
    }

    async fn ensure_dhcp_scope(
        &self,
        request: Request<proto::EnsureDhcpScopeRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        self.metrics
            .increment_counter("nwd_ensure_dhcp_scope_total");
        let req = request.into_inner();
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

        let scope = req.scope.ok_or_else(|| ChvError::InvalidArgument {
            field: "scope".to_string(),
            reason: "missing".to_string(),
        });
        let scope = match scope {
            Ok(s) => s,
            Err(e) => return Ok(Response::new(Self::err_result(&e))),
        };

        match self
            .executor
            .ensure_dhcp_scope(
                &scope.network_id,
                &scope.cidr,
                &scope.range_start,
                &scope.range_end,
                &scope.dns_servers,
            )
            .await
        {
            Ok(()) => Ok(Response::new(Self::ok_result())),
            Err(e) => Ok(Response::new(Self::err_result(&e))),
        }
    }

    async fn ensure_dns_scope(
        &self,
        request: Request<proto::EnsureDnsScopeRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        self.metrics.increment_counter("nwd_ensure_dns_scope_total");
        let req = request.into_inner();
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

        let scope = req.scope.ok_or_else(|| ChvError::InvalidArgument {
            field: "scope".to_string(),
            reason: "missing".to_string(),
        });
        let scope = match scope {
            Ok(s) => s,
            Err(e) => return Ok(Response::new(Self::err_result(&e))),
        };

        let fw: Vec<&str> = scope.forwarders.iter().map(|s| s.as_str()).collect();
        let static_records: std::collections::HashMap<String, String> =
            scope.static_records.into_iter().collect();
        match self
            .executor
            .ensure_dns_scope(&scope.network_id, &fw, &static_records)
            .await
        {
            Ok(()) => Ok(Response::new(Self::ok_result())),
            Err(e) => Ok(Response::new(Self::err_result(&e))),
        }
    }

    async fn expose_service(
        &self,
        request: Request<proto::ExposeServiceRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        self.metrics.increment_counter("nwd_expose_service_total");
        let req = request.into_inner();
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

        let exposure = req.exposure.ok_or_else(|| ChvError::InvalidArgument {
            field: "exposure".to_string(),
            reason: "missing".to_string(),
        });
        let exposure = match exposure {
            Ok(e) => e,
            Err(e) => return Ok(Response::new(Self::err_result(&e))),
        };

        match self
            .executor
            .expose_service(
                &exposure.network_id,
                &exposure.exposure_id,
                &exposure.protocol,
                exposure.external_port,
                &exposure.target_ip,
                exposure.target_port,
                &exposure.mode,
            )
            .await
        {
            Ok(()) => Ok(Response::new(Self::ok_result())),
            Err(e) => Ok(Response::new(Self::err_result(&e))),
        }
    }

    async fn withdraw_service_exposure(
        &self,
        request: Request<proto::WithdrawServiceExposureRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        self.metrics
            .increment_counter("nwd_withdraw_service_exposure_total");
        let req = request.into_inner();
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

        match self
            .executor
            .withdraw_service_exposure(&req.network_id, &req.exposure_id)
            .await
        {
            Ok(()) => Ok(Response::new(Self::ok_result())),
            Err(e) => Ok(Response::new(Self::err_result(&e))),
        }
    }

    async fn update_overlay(
        &self,
        request: Request<proto::UpdateOverlayRequest>,
    ) -> Result<Response<proto::UpdateOverlayResponse>, Status> {
        self.metrics.increment_counter("nwd_update_overlay_total");
        let req = request.into_inner();

        let state = match self.topologies.get(&req.network_id) {
            Some(s) => s,
            None => {
                let e = ChvError::NotFound {
                    resource: "topology".to_string(),
                    id: req.network_id.clone(),
                };
                return Ok(Response::new(proto::UpdateOverlayResponse {
                    result: Some(Self::err_result(&e)),
                }));
            }
        };

        if req.vni == 0 {
            let e = ChvError::InvalidArgument {
                field: "vni".to_string(),
                reason: "VNI must be > 0 for overlay update".to_string(),
            };
            return Ok(Response::new(proto::UpdateOverlayResponse {
                result: Some(Self::err_result(&e)),
            }));
        }

        // VNI range validation: VXLAN VNI is a 24-bit field (max 16777215)
        if req.vni > 16_777_215 {
            let e = ChvError::InvalidArgument {
                field: "vni".to_string(),
                reason: format!("VNI {} exceeds maximum 16777215", req.vni),
            };
            return Ok(Response::new(proto::UpdateOverlayResponse {
                result: Some(Self::err_result(&e)),
            }));
        }

        // Sync explicit FDB entries for peer VTEPs (unicast MAC-to-VTEP mappings)
        for fdb in &req.fdb_entries {
            if let Err(e) = self
                .executor
                .add_fdb_entry(
                    &state.namespace_name,
                    req.vni,
                    &fdb.mac_address,
                    &fdb.vtep_ip,
                )
                .await
            {
                return Ok(Response::new(proto::UpdateOverlayResponse {
                    result: Some(Self::err_result(&e)),
                }));
            }
        }

        // Reconcile BUM traffic FDB entries: compute delta against previously known VTEPs
        let new_vteps: Vec<String> = req
            .vtep_endpoints
            .iter()
            .map(|e| e.vtep_ip.clone())
            .collect();
        let old_vteps = &state.peer_vteps;

        if let Err(e) = reconcile::reconcile_fdb_entries(
            self.executor.as_ref(),
            &state.namespace_name,
            req.vni,
            old_vteps,
            &new_vteps,
        )
        .await
        {
            return Ok(Response::new(proto::UpdateOverlayResponse {
                result: Some(Self::err_result(&e)),
            }));
        }

        // Update tracked peer VTEPs in topology state
        let updated_state = TopologyState {
            peer_vteps: new_vteps,
            ..state.clone()
        };
        self.topologies.upsert(updated_state.clone());

        info!(
            network_id = %req.network_id,
            vni = req.vni,
            fdb_count = req.fdb_entries.len(),
            vtep_count = req.vtep_endpoints.len(),
            "overlay updated with FDB reconciliation"
        );

        Ok(Response::new(proto::UpdateOverlayResponse {
            result: Some(Self::ok_result()),
        }))
    }

    async fn send_gratuitous_arp(
        &self,
        request: Request<proto::SendGratuitousArpRequest>,
    ) -> Result<Response<proto::SendGratuitousArpResponse>, Status> {
        self.metrics
            .increment_counter("nwd_send_gratuitous_arp_total");
        let req = request.into_inner();

        let state = match self.topologies.get(&req.network_id) {
            Some(s) => s,
            None => {
                let e = ChvError::NotFound {
                    resource: "topology".to_string(),
                    id: req.network_id.clone(),
                };
                return Ok(Response::new(proto::SendGratuitousArpResponse {
                    result: Some(Self::err_result(&e)),
                }));
            }
        };

        if let Err(e) = self
            .executor
            .send_gratuitous_arp(&state.namespace_name, &req.bridge_name, &req.vm_ip)
            .await
        {
            return Ok(Response::new(proto::SendGratuitousArpResponse {
                result: Some(Self::err_result(&e)),
            }));
        }

        info!(
            network_id = %req.network_id,
            vm_ip = %req.vm_ip,
            bridge_name = %req.bridge_name,
            "gratuitous ARP sent"
        );

        Ok(Response::new(proto::SendGratuitousArpResponse {
            result: Some(Self::ok_result()),
        }))
    }

    async fn update_security_policy(
        &self,
        request: Request<proto::SecurityPolicy>,
    ) -> Result<Response<proto::UpdateSecurityPolicyResponse>, Status> {
        self.metrics
            .increment_counter("nwd_update_security_policy_total");
        let policy = request.into_inner();

        let key = format!("{}:{}", policy.network_id, policy.vm_id);
        let vm_id = policy.vm_id.clone();
        let default_action = if policy.default_action == proto::PolicyAction::PolicyAllow as i32 {
            1u8
        } else {
            0u8
        };

        // Convert proto rules to eBPF rules
        let ebpf_rules = ebpf::proto_to_ebpf_rules(&vm_id, &policy);

        info!(
            vm_id = %policy.vm_id,
            network_id = %policy.network_id,
            rule_count = policy.rules.len(),
            ebpf_available = self.ebpf.is_available(),
            "security policy stored"
        );
        self.security_policies.insert(key, policy);

        // Push rules to eBPF maps
        if let Err(e) = self.ebpf.update_rules(&vm_id, &ebpf_rules).await {
            tracing::warn!(vm_id = %vm_id, error = %e, "failed to update eBPF rules");
        }
        if let Err(e) = self.ebpf.set_default_action(&vm_id, default_action).await {
            tracing::warn!(vm_id = %vm_id, error = %e, "failed to set eBPF default action");
        }

        Ok(Response::new(proto::UpdateSecurityPolicyResponse {
            result: Some(Self::ok_result()),
        }))
    }

    async fn update_rate_limit(
        &self,
        request: Request<proto::RateLimitPolicy>,
    ) -> Result<Response<proto::UpdateRateLimitResponse>, Status> {
        self.metrics
            .increment_counter("nwd_update_rate_limit_total");
        let policy = request.into_inner();

        let vm_id = policy.vm_id.clone();
        let ebpf_rl = ebpf::proto_to_ebpf_rate_limit(&policy);

        info!(
            vm_id = %policy.vm_id,
            rate_bps = policy.rate_bps,
            burst_bytes = policy.burst_bytes,
            ebpf_available = self.ebpf.is_available(),
            "rate limit policy stored"
        );
        self.rate_limit_policies
            .insert(policy.vm_id.clone(), policy);

        // Push rate limit to eBPF maps
        if let Err(e) = self.ebpf.update_rate_limit(&vm_id, &ebpf_rl).await {
            tracing::warn!(vm_id = %vm_id, error = %e, "failed to update eBPF rate limit");
        }

        Ok(Response::new(proto::UpdateRateLimitResponse {
            result: Some(Self::ok_result()),
        }))
    }

    async fn get_overlay_status(
        &self,
        request: Request<proto::GetOverlayStatusRequest>,
    ) -> Result<Response<proto::OverlayStatus>, Status> {
        let req = request.into_inner();

        let state = match self.topologies.get(&req.network_id) {
            Some(s) => s,
            None => {
                return Ok(Response::new(proto::OverlayStatus {
                    network_id: req.network_id,
                    vni: 0,
                    vxlan_interface_up: false,
                    fdb_entry_count: 0,
                    ebpf_programs_loaded: 0,
                }));
            }
        };

        // Try to determine VNI from topology; for now look it up from state
        // In a full implementation, TopologyState would track VNI.
        // We use a best-effort approach: check if any overlay exists.
        let vni = state.vni.unwrap_or(0);
        if vni == 0 {
            return Ok(Response::new(proto::OverlayStatus {
                network_id: req.network_id,
                vni: 0,
                vxlan_interface_up: false,
                fdb_entry_count: 0,
                ebpf_programs_loaded: 0,
            }));
        }

        let status_info: OverlayStatusInfo = match self
            .executor
            .get_overlay_status(&state.namespace_name, vni)
            .await
        {
            Ok(s) => s,
            Err(_) => OverlayStatusInfo {
                vxlan_interface_up: false,
                fdb_entry_count: 0,
            },
        };

        Ok(Response::new(proto::OverlayStatus {
            network_id: req.network_id,
            vni,
            vxlan_interface_up: status_info.vxlan_interface_up,
            fdb_entry_count: status_info.fdb_entry_count,
            ebpf_programs_loaded: self.ebpf.loaded_program_count(),
        }))
    }
}
