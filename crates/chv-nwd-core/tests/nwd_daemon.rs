use async_trait::async_trait;
use chv_errors::ChvError;
use chv_nwd_api::chv_nwd_api::{
    network_service_client::NetworkServiceClient, DeleteNetworkTopologyRequest,
    EnsureNetworkTopologyRequest, ListNamespaceStateRequest, NetworkHealthRequest,
    TopologySpec,
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
