use crate::executor::{NetworkExecutor, TopologyApplyResult};
use crate::state::{TopologyState, TopologyTable};
use chv_errors::ChvError;
use chv_nwd_api::chv_nwd_api as proto;
use chv_observability::{operation_span, Metrics};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct NetworkServiceImpl<E: NetworkExecutor> {
    executor: Arc<E>,
    topologies: Arc<TopologyTable>,
    metrics: Arc<Metrics>,
}

impl<E: NetworkExecutor> NetworkServiceImpl<E> {
    pub fn new(
        executor: Arc<E>,
        topologies: Arc<TopologyTable>,
        metrics: Arc<Metrics>,
    ) -> Self {
        Self {
            executor,
            topologies,
            metrics,
        }
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

    fn map_topology_spec(
        t: Option<proto::TopologySpec>,
    ) -> Result<proto::TopologySpec, ChvError> {
        t.ok_or_else(|| ChvError::InvalidArgument {
            field: "topology".to_string(),
            reason: "missing".to_string(),
        })
    }
}

#[tonic::async_trait]
impl<E: NetworkExecutor> proto::network_service_server::NetworkService
    for NetworkServiceImpl<E>
{
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
        let _enter = _span.enter();

        let spec = match Self::map_topology_spec(req.topology) {
            Ok(s) => s,
            Err(e) => return Ok(Response::new(Self::err_result(&e))),
        };

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
                let state = TopologyState {
                    network_id: spec.network_id.clone(),
                    tenant_id: spec.tenant_id.clone(),
                    bridge_name: spec.bridge_name.clone(),
                    namespace_name: spec.namespace_name.clone(),
                    subnet_cidr: spec.subnet_cidr.clone(),
                    gateway_ip: spec.gateway_ip.clone(),
                    runtime_status: "ensured".to_string(),
                };
                self.topologies.upsert(state);
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
        let _enter = _span.enter();

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

        let (status, last_error) =
            if let Some(state) = self.topologies.get(&req.network_id) {
                match self.executor.health(&req.network_id, &state).await {
                    Ok(s) => (s, String::new()),
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
        _request: Request<proto::AttachVmNicRequest>,
    ) -> Result<Response<proto::AttachVmNicResponse>, Status> {
        Err(Status::unimplemented("attach_vm_nic not yet implemented"))
    }

    async fn detach_vm_nic(
        &self,
        _request: Request<proto::DetachVmNicRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        Err(Status::unimplemented("detach_vm_nic not yet implemented"))
    }

    async fn set_firewall_policy(
        &self,
        _request: Request<proto::SetFirewallPolicyRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        Err(Status::unimplemented(
            "set_firewall_policy not yet implemented",
        ))
    }

    async fn set_nat_policy(
        &self,
        _request: Request<proto::SetNatPolicyRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        Err(Status::unimplemented(
            "set_nat_policy not yet implemented",
        ))
    }

    async fn ensure_dhcp_scope(
        &self,
        _request: Request<proto::EnsureDhcpScopeRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        Err(Status::unimplemented(
            "ensure_dhcp_scope not yet implemented",
        ))
    }

    async fn ensure_dns_scope(
        &self,
        _request: Request<proto::EnsureDnsScopeRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        Err(Status::unimplemented(
            "ensure_dns_scope not yet implemented",
        ))
    }

    async fn expose_service(
        &self,
        _request: Request<proto::ExposeServiceRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        Err(Status::unimplemented(
            "expose_service not yet implemented",
        ))
    }

    async fn withdraw_service_exposure(
        &self,
        _request: Request<proto::WithdrawServiceExposureRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        Err(Status::unimplemented(
            "withdraw_service_exposure not yet implemented",
        ))
    }
}
