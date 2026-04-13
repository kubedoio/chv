use chv_errors::ChvError;
use control_plane_node_api::control_plane_node_api as proto;
use tonic::transport::Channel;

pub struct EnrollmentClient {
    inner: proto::enrollment_service_client::EnrollmentServiceClient<Channel>,
}

pub struct BootstrapResponse {
    pub node_id: String,
    pub certificate_pem: Vec<u8>,
    pub private_key_pem: Vec<u8>,
    pub ca_pem: Vec<u8>,
}

impl EnrollmentClient {
    pub async fn connect(endpoint: impl Into<String>) -> Result<Self, ChvError> {
        let endpoint = tonic::transport::Endpoint::try_from(endpoint.into())
            .map_err(|e| ChvError::InvalidArgument {
                field: "enrollment_endpoint".to_string(),
                reason: e.to_string(),
            })?;
        let channel = endpoint.connect().await.map_err(|e| ChvError::ControlPlaneUnavailable {
            reason: e.to_string(),
        })?;
        Ok(Self {
            inner: proto::enrollment_service_client::EnrollmentServiceClient::new(channel),
        })
    }

    pub async fn enroll_node(
        &mut self,
        bootstrap_token: &str,
        inventory: proto::NodeInventory,
        versions: proto::ServiceVersions,
    ) -> Result<BootstrapResponse, ChvError> {
        let req = proto::EnrollmentRequest {
            bootstrap_token: bootstrap_token.to_string(),
            inventory: Some(inventory),
            versions: Some(versions),
        };
        let resp = self.inner.enroll_node(req).await.map_err(|e| ChvError::ControlPlaneUnavailable {
            reason: e.to_string(),
        })?;
        let inner = resp.into_inner();
        let result = inner.result.ok_or_else(|| ChvError::Internal {
            reason: "missing result meta".to_string(),
        })?;
        if result.status != "ok" {
            return Err(ChvError::ControlPlaneUnavailable {
                reason: result.human_summary,
            });
        }
        Ok(BootstrapResponse {
            node_id: inner.issued_node_id,
            certificate_pem: inner.certificate_pem,
            private_key_pem: inner.private_key_pem,
            ca_pem: inner.ca_pem,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proto::enrollment_service_server::{EnrollmentService, EnrollmentServiceServer};
    use std::net::SocketAddr;
    use tokio::sync::oneshot;
    use tonic::{Request, Response, Status};

    struct MockEnrollmentService;

    #[tonic::async_trait]
    impl EnrollmentService for MockEnrollmentService {
        async fn enroll_node(
            &self,
            _request: Request<proto::EnrollmentRequest>,
        ) -> Result<Response<proto::EnrollmentResponse>, Status> {
            Ok(Response::new(proto::EnrollmentResponse {
                result: Some(proto::ResultMeta {
                    operation_id: "".to_string(),
                    status: "ok".to_string(),
                    node_observed_generation: "".to_string(),
                    error_code: "".to_string(),
                    human_summary: "enrolled".to_string(),
                }),
                issued_node_id: "node-123".to_string(),
                certificate_pem: b"cert-pem".to_vec(),
                private_key_pem: b"key-pem".to_vec(),
                ca_pem: b"ca-pem".to_vec(),
            }))
        }

        async fn rotate_node_certificate(
            &self,
            _request: Request<proto::RotateNodeCertificateRequest>,
        ) -> Result<Response<proto::RotateNodeCertificateResponse>, Status> {
            Err(Status::unimplemented(""))
        }

        async fn report_bootstrap_result(
            &self,
            _request: Request<proto::ReportBootstrapResultRequest>,
        ) -> Result<Response<proto::AckResponse>, Status> {
            Err(Status::unimplemented(""))
        }
    }

    #[tokio::test]
    async fn mock_enroll_success() {
        let service = MockEnrollmentService;
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        let bound_addr = listener.local_addr().unwrap();

        let (tx, rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let server = tonic::transport::Server::builder()
                .add_service(EnrollmentServiceServer::new(service))
                .serve_with_incoming_shutdown(
                    tokio_stream::wrappers::TcpListenerStream::new(listener),
                    async { rx.await.ok(); },
                );
            server.await.unwrap();
        });

        let mut client = EnrollmentClient::connect(format!("http://{}", bound_addr))
            .await
            .unwrap();

        let inventory = proto::NodeInventory {
            node_id: "node-123".to_string(),
            hostname: "test-host".to_string(),
            architecture: "x86_64".to_string(),
            cpu_threads: 4,
            memory_bytes: 8589934592,
            storage_classes: vec![],
            network_capabilities: vec![],
            labels: std::collections::HashMap::new(),
        };
        let versions = proto::ServiceVersions {
            node_id: "node-123".to_string(),
            chv_agent_version: env!("CARGO_PKG_VERSION").to_string(),
            chv_stord_version: "".to_string(),
            chv_nwd_version: "".to_string(),
            cloud_hypervisor_version: "".to_string(),
            host_bundle_version: "".to_string(),
        };

        let resp = client.enroll_node("token", inventory, versions).await.unwrap();
        assert_eq!(resp.node_id, "node-123");
        assert_eq!(resp.certificate_pem, b"cert-pem");
        assert_eq!(resp.private_key_pem, b"key-pem");
        assert_eq!(resp.ca_pem, b"ca-pem");

        let _ = tx.send(());
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), server).await;
    }
}
