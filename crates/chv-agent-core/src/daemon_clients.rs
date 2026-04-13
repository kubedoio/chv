use chv_errors::ChvError;
use chv_nwd_api::chv_nwd_api::{
    network_service_client::NetworkServiceClient, ListNamespaceStateRequest,
};
use chv_stord_api::chv_stord_api::{
    storage_service_client::StorageServiceClient, ListVolumeSessionsRequest,
};
use std::path::Path;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;

pub struct StordClient {
    inner: StorageServiceClient<Channel>,
}

impl StordClient {
    pub async fn connect(socket_path: &Path) -> Result<Self, ChvError> {
        let path = socket_path.to_path_buf();
        let channel = Endpoint::try_from("http://[::]:50051")
            .map_err(|e| ChvError::InvalidArgument {
                field: "stord_socket".to_string(),
                reason: e.to_string(),
            })?
            .connect_with_connector(service_fn(move |_: Uri| {
                let p = path.clone();
                async move {
                    let stream = UnixStream::connect(p).await?;
                    Ok::<_, std::io::Error>(hyper_util::rt::tokio::TokioIo::new(stream))
                }
            }))
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "stord".to_string(),
                reason: e.to_string(),
            })?;
        Ok(Self {
            inner: StorageServiceClient::new(channel),
        })
    }

    pub async fn health_probe(&mut self) -> Result<bool, ChvError> {
        let _ = self
            .inner
            .list_volume_sessions(ListVolumeSessionsRequest {})
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "stord".to_string(),
                reason: e.to_string(),
            })?;
        Ok(true)
    }
}

pub struct NwdClient {
    inner: NetworkServiceClient<Channel>,
}

impl NwdClient {
    pub async fn connect(socket_path: &Path) -> Result<Self, ChvError> {
        let path = socket_path.to_path_buf();
        let channel = Endpoint::try_from("http://[::]:50051")
            .map_err(|e| ChvError::InvalidArgument {
                field: "nwd_socket".to_string(),
                reason: e.to_string(),
            })?
            .connect_with_connector(service_fn(move |_: Uri| {
                let p = path.clone();
                async move {
                    let stream = UnixStream::connect(p).await?;
                    Ok::<_, std::io::Error>(hyper_util::rt::tokio::TokioIo::new(stream))
                }
            }))
            .await
            .map_err(|e| ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: e.to_string(),
            })?;
        Ok(Self {
            inner: NetworkServiceClient::new(channel),
        })
    }

    pub async fn health_probe(&mut self) -> Result<bool, ChvError> {
        let _ = self
            .inner
            .list_namespace_state(ListNamespaceStateRequest {})
            .await
            .map_err(|e| ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: e.to_string(),
            })?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chv_nwd_api::chv_nwd_api::network_service_server::NetworkService;
    use chv_stord_api::chv_stord_api::storage_service_server::StorageService;
    use std::time::Duration;
    use tonic::{Request, Response, Status};

    struct MockStord;
    #[tonic::async_trait]
    impl StorageService for MockStord {
        async fn list_volume_sessions(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::ListVolumeSessionsRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::ListVolumeSessionsResponse>, Status> {
            Ok(Response::new(
                chv_stord_api::chv_stord_api::ListVolumeSessionsResponse {
                    sessions: vec![],
                },
            ))
        }
        // Stub remaining methods
        async fn open_volume(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::OpenVolumeRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::OpenVolumeResponse>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn close_volume(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::CloseVolumeRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn get_volume_health(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::VolumeHealthRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::VolumeHealthResponse>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn attach_volume_to_vm(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::AttachVolumeToVmRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::AttachVolumeToVmResponse>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn detach_volume_from_vm(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::DetachVolumeFromVmRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn resize_volume(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::ResizeVolumeRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn prepare_snapshot(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::PrepareSnapshotRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn prepare_clone(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::PrepareCloneRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn set_device_policy(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::SetDevicePolicyRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
    }

    struct MockNwd;
    #[tonic::async_trait]
    impl NetworkService for MockNwd {
        async fn list_namespace_state(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::ListNamespaceStateRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::ListNamespaceStateResponse>, Status> {
            Ok(Response::new(
                chv_nwd_api::chv_nwd_api::ListNamespaceStateResponse { items: vec![] },
            ))
        }
        // Stub remaining methods
        async fn ensure_network_topology(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::EnsureNetworkTopologyRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn delete_network_topology(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::DeleteNetworkTopologyRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn get_network_health(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::NetworkHealthRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::NetworkHealthResponse>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn attach_vm_nic(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::AttachVmNicRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::AttachVmNicResponse>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn detach_vm_nic(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::DetachVmNicRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn set_firewall_policy(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::SetFirewallPolicyRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn set_nat_policy(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::SetNatPolicyRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn ensure_dhcp_scope(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::EnsureDhcpScopeRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn ensure_dns_scope(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::EnsureDnsScopeRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn expose_service(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::ExposeServiceRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn withdraw_service_exposure(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::WithdrawServiceExposureRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
    }

    #[tokio::test]
    async fn stord_health_probe_mock() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("stord.sock");

        let uds = tokio::net::UnixListener::bind(&socket).unwrap();
        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(chv_stord_api::chv_stord_api::storage_service_server::StorageServiceServer::new(MockStord))
                .serve_with_incoming(tokio_stream::wrappers::UnixListenerStream::new(uds))
                .await
                .ok();
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut client = StordClient::connect(&socket).await.unwrap();
        assert!(client.health_probe().await.unwrap());
    }

    #[tokio::test]
    async fn nwd_health_probe_mock() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("nwd.sock");

        let uds = tokio::net::UnixListener::bind(&socket).unwrap();
        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(chv_nwd_api::chv_nwd_api::network_service_server::NetworkServiceServer::new(MockNwd))
                .serve_with_incoming(tokio_stream::wrappers::UnixListenerStream::new(uds))
                .await
                .ok();
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut client = NwdClient::connect(&socket).await.unwrap();
        assert!(client.health_probe().await.unwrap());
    }
}
