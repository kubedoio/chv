use async_trait::async_trait;
use chv_errors::ChvError;
use chv_nwd_api::chv_nwd_api::{
    network_service_client::NetworkServiceClient, AttachVmNicRequest, DeleteNetworkTopologyRequest,
    DetachVmNicRequest, DhcpScope, DnsScope, EnsureDhcpScopeRequest, EnsureDnsScopeRequest,
    EnsureNetworkTopologyRequest, ExposeServiceRequest, ExposureSpec, FirewallPolicy,
    ListNamespaceStateRequest, NatPolicy, NetworkHealthRequest, NicSpec, SetFirewallPolicyRequest,
    TopologySpec, WithdrawServiceExposureRequest,
};
use chv_nwd_core::executor::{NetworkExecutor, TopologyApplyResult};
use chv_nwd_core::{NetworkServer, TopologyState};
use chv_observability::Metrics;
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::UnixStream;
use tonic::transport::{Endpoint, Uri};
use tower::service_fn;

struct MockExecutor;

#[async_trait]
impl NetworkExecutor for MockExecutor {
    async fn ensure_topology(
        &self,
        spec: &chv_nwd_api::chv_nwd_api::TopologySpec,
    ) -> Result<TopologyApplyResult, ChvError> {
        Ok(TopologyApplyResult {
            namespace_handle: spec.namespace_name.clone(),
            bridge_handle: spec.bridge_name.clone(),
        })
    }

    async fn delete_topology(
        &self,
        _network_id: &str,
        _state: &TopologyState,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn health(
        &self,
        _network_id: &str,
        _state: &TopologyState,
    ) -> Result<String, ChvError> {
        Ok("healthy".to_string())
    }

    async fn attach_vm_nic(
        &self,
        network_id: &str,
        nic_id: &str,
        _vm_id: &str,
        _bridge_name: &str,
        _mac_address: &str,
        _ip_address: &str,
    ) -> Result<(String, String), ChvError> {
        Ok((format!("ns-{}", network_id), format!("tap-{}", nic_id)))
    }

    async fn detach_vm_nic(
        &self,
        _nic_id: &str,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn set_firewall_policy(
        &self,
        _network_id: &str,
        _policy_version: &str,
        _policy_json: &[u8],
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn set_nat_policy(
        &self,
        _network_id: &str,
        _policy_version: &str,
        _policy_json: &[u8],
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn ensure_dhcp_scope(
        &self,
        _network_id: &str,
        _cidr: &str,
        _range_start: &str,
        _range_end: &str,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn ensure_dns_scope(
        &self,
        _network_id: &str,
        _forwarders: &[&str],
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn expose_service(
        &self,
        _network_id: &str,
        _exposure_id: &str,
        _protocol: &str,
        _external_port: u32,
        _target_ip: &str,
        _target_port: u32,
        _mode: &str,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn withdraw_service_exposure(
        &self,
        _network_id: &str,
        _exposure_id: &str,
    ) -> Result<(), ChvError> {
        Ok(())
    }
}

async fn make_client(socket: PathBuf) -> NetworkServiceClient<tonic::transport::Channel> {
    let channel = Endpoint::try_from("http://[::]:50051")
        .unwrap()
        .connect_with_connector(service_fn(move |_: Uri| {
            let s = socket.clone();
            async move {
                let stream = UnixStream::connect(s).await?;
                Ok::<_, std::io::Error>(hyper_util::rt::tokio::TokioIo::new(stream))
            }
        }))
        .await
        .unwrap();
    NetworkServiceClient::new(channel)
}

#[tokio::test]
async fn ensure_and_delete_topology_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nwd.sock");

    let server = NetworkServer::new(MockExecutor, Metrics::new());
    let socket_clone = socket.clone();
    tokio::spawn(async move {
        server.serve(&socket_clone).await.ok();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut client = make_client(socket).await;

    let req = EnsureNetworkTopologyRequest {
        meta: None,
        topology: Some(TopologySpec {
            network_id: "net-1".to_string(),
            tenant_id: "t1".to_string(),
            bridge_name: "br-net1".to_string(),
            namespace_name: "ns-net1".to_string(),
            subnet_cidr: "10.0.1.0/24".to_string(),
            gateway_ip: "10.0.1.1".to_string(),
            options: Default::default(),
        }),
    };

    // First ensure
    let resp1 = client
        .ensure_network_topology(req.clone())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(resp1.status, "OK");

    // Idempotent ensure
    let resp2 = client
        .ensure_network_topology(req.clone())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(resp2.status, "OK");

    // Health
    let health = client
        .get_network_health(NetworkHealthRequest {
            network_id: "net-1".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(health.network_id, "net-1");
    assert_eq!(health.health_status, "healthy");

    // List
    let list = client
        .list_namespace_state(ListNamespaceStateRequest {})
        .await
        .unwrap()
        .into_inner();
    assert_eq!(list.items.len(), 1);
    assert_eq!(list.items[0].network_id, "net-1");

    // Delete
    let del1 = client
        .delete_network_topology(DeleteNetworkTopologyRequest {
            meta: None,
            network_id: "net-1".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(del1.status, "OK");

    // Idempotent delete
    let del2 = client
        .delete_network_topology(DeleteNetworkTopologyRequest {
            meta: None,
            network_id: "net-1".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(del2.status, "OK");

    // Health after delete
    let health2 = client
        .get_network_health(NetworkHealthRequest {
            network_id: "net-1".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(health2.health_status, "unknown");
}

#[tokio::test]
async fn all_network_handlers_smoke() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nwd.sock");

    let server = NetworkServer::new(MockExecutor, Metrics::new());
    let socket_clone = socket.clone();
    tokio::spawn(async move {
        server.serve(&socket_clone).await.ok();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut client = make_client(socket).await;

    // Ensure topology first
    let ensure = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(TopologySpec {
                network_id: "net-1".to_string(),
                tenant_id: "t1".to_string(),
                bridge_name: "br-net1".to_string(),
                namespace_name: "ns-net1".to_string(),
                subnet_cidr: "10.0.1.0/24".to_string(),
                gateway_ip: "10.0.1.1".to_string(),
                options: Default::default(),
            }),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(ensure.status, "OK");

    // attach_vm_nic
    let attach = client
        .attach_vm_nic(AttachVmNicRequest {
            meta: None,
            nic: Some(NicSpec {
                nic_id: "nic-1".to_string(),
                vm_id: "vm-1".to_string(),
                network_id: "net-1".to_string(),
                mac_address: "02:00:00:00:00:01".to_string(),
                tap_name: "tap-nic-1".to_string(),
                ip_address: "10.0.1.10".to_string(),
            }),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(attach.result.as_ref().unwrap().status, "OK");
    assert_eq!(attach.namespace_handle, "ns-net-1");
    assert_eq!(attach.tap_handle, "tap-nic-1");

    // detach_vm_nic
    let detach = client
        .detach_vm_nic(DetachVmNicRequest {
            meta: None,
            vm_id: "vm-1".to_string(),
            nic_id: "nic-1".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(detach.status, "OK");

    // set_firewall_policy
    let fw = client
        .set_firewall_policy(SetFirewallPolicyRequest {
            meta: None,
            network_id: "net-1".to_string(),
            policy: Some(FirewallPolicy {
                policy_version: "v1".to_string(),
                policy_json: b"{}".to_vec(),
            }),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(fw.status, "OK");

    // set_nat_policy
    let nat = client
        .set_nat_policy(chv_nwd_api::chv_nwd_api::SetNatPolicyRequest {
            meta: None,
            network_id: "net-1".to_string(),
            policy: Some(NatPolicy {
                policy_version: "v1".to_string(),
                policy_json: b"{}".to_vec(),
            }),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(nat.status, "OK");

    // ensure_dhcp_scope
    let dhcp = client
        .ensure_dhcp_scope(EnsureDhcpScopeRequest {
            meta: None,
            scope: Some(DhcpScope {
                network_id: "net-1".to_string(),
                cidr: "10.0.1.0/24".to_string(),
                range_start: "10.0.1.50".to_string(),
                range_end: "10.0.1.100".to_string(),
                dns_servers: vec!["10.0.1.1".to_string()],
            }),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(dhcp.status, "OK");

    // ensure_dns_scope
    let dns = client
        .ensure_dns_scope(EnsureDnsScopeRequest {
            meta: None,
            scope: Some(DnsScope {
                network_id: "net-1".to_string(),
                forwarders: vec!["8.8.8.8".to_string()],
                static_records: Default::default(),
            }),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(dns.status, "OK");

    // expose_service
    let expose = client
        .expose_service(ExposeServiceRequest {
            meta: None,
            exposure: Some(ExposureSpec {
                network_id: "net-1".to_string(),
                exposure_id: "exp-1".to_string(),
                protocol: "tcp".to_string(),
                external_port: 8080,
                target_ip: "10.0.1.10".to_string(),
                target_port: 80,
                mode: "dnat".to_string(),
            }),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(expose.status, "OK");

    // withdraw_service_exposure
    let withdraw = client
        .withdraw_service_exposure(WithdrawServiceExposureRequest {
            meta: None,
            exposure_id: "exp-1".to_string(),
            network_id: "net-1".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(withdraw.status, "OK");

    // Cleanup
    let del = client
        .delete_network_topology(DeleteNetworkTopologyRequest {
            meta: None,
            network_id: "net-1".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(del.status, "OK");
}

#[tokio::test]
async fn attach_vm_nic_missing_topology_returns_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nwd.sock");

    let server = NetworkServer::new(MockExecutor, Metrics::new());
    let socket_clone = socket.clone();
    tokio::spawn(async move {
        server.serve(&socket_clone).await.ok();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut client = make_client(socket).await;

    let result = client
        .attach_vm_nic(AttachVmNicRequest {
            meta: None,
            nic: Some(NicSpec {
                nic_id: "nic-1".to_string(),
                vm_id: "vm-1".to_string(),
                network_id: "net-not-ensured".to_string(),
                mac_address: "02:00:00:00:00:01".to_string(),
                tap_name: "tap-nic-1".to_string(),
                ip_address: "10.0.1.10".to_string(),
            }),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(result.result.as_ref().unwrap().status, "error");
    assert_eq!(result.result.as_ref().unwrap().error_code, "NOT_FOUND");
}
