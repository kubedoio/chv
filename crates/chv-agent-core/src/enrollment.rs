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

pub struct RotatedCertificate {
    pub certificate_pem: Vec<u8>,
    pub private_key_pem: Vec<u8>,
    pub ca_pem: Vec<u8>,
}

impl EnrollmentClient {
    pub async fn connect(endpoint: impl Into<String>) -> Result<Self, ChvError> {
        Self::connect_with_tls(endpoint, None, None, None).await
    }

    pub async fn connect_with_tls(
        endpoint: impl Into<String>,
        tls_cert_path: Option<&std::path::Path>,
        tls_key_path: Option<&std::path::Path>,
        ca_cert_path: Option<&std::path::Path>,
    ) -> Result<Self, ChvError> {
        let endpoint = tonic::transport::Endpoint::try_from(endpoint.into()).map_err(|e| {
            ChvError::InvalidArgument {
                field: "enrollment_endpoint".to_string(),
                reason: e.to_string(),
            }
        })?;
        let mut endpoint = endpoint;

        if let (Some(cert), Some(key)) = (tls_cert_path, tls_key_path) {
            let cert_pem = tokio::fs::read(cert).await.map_err(|e| ChvError::Io {
                path: cert.to_string_lossy().to_string(),
                source: e,
            })?;
            let key_pem = tokio::fs::read(key).await.map_err(|e| ChvError::Io {
                path: key.to_string_lossy().to_string(),
                source: e,
            })?;
            let identity = tonic::transport::Identity::from_pem(cert_pem, key_pem);
            let mut tls = tonic::transport::ClientTlsConfig::new().identity(identity);
            if let Some(ca) = ca_cert_path {
                let ca_pem = tokio::fs::read(ca).await.map_err(|e| ChvError::Io {
                    path: ca.to_string_lossy().to_string(),
                    source: e,
                })?;
                tls = tls.ca_certificate(tonic::transport::Certificate::from_pem(ca_pem));
            }
            endpoint = endpoint
                .tls_config(tls)
                .map_err(|e| ChvError::InvalidArgument {
                    field: "tls_config".to_string(),
                    reason: e.to_string(),
                })?;
        }

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| ChvError::ControlPlaneUnavailable {
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
        let resp =
            self.inner
                .enroll_node(req)
                .await
                .map_err(|e| ChvError::ControlPlaneUnavailable {
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

    pub async fn rotate_node_certificate(
        &mut self,
        meta: proto::RequestMeta,
        node_id: &str,
    ) -> Result<RotatedCertificate, ChvError> {
        let req = proto::RotateNodeCertificateRequest {
            meta: Some(meta),
            node_id: node_id.to_string(),
        };
        let resp = self.inner.rotate_node_certificate(req).await.map_err(|e| {
            ChvError::ControlPlaneUnavailable {
                reason: e.to_string(),
            }
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
        Ok(RotatedCertificate {
            certificate_pem: inner.certificate_pem,
            private_key_pem: inner.private_key_pem,
            ca_pem: inner.ca_pem,
        })
    }

    pub async fn report_bootstrap_result(
        &mut self,
        meta: proto::RequestMeta,
        node_id: &str,
        bootstrap_status: &str,
        message: &str,
    ) -> Result<(), ChvError> {
        let req = proto::ReportBootstrapResultRequest {
            meta: Some(meta),
            node_id: node_id.to_string(),
            bootstrap_status: bootstrap_status.to_string(),
            message: message.to_string(),
        };
        let resp = self.inner.report_bootstrap_result(req).await.map_err(|e| {
            ChvError::ControlPlaneUnavailable {
                reason: e.to_string(),
            }
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
        Ok(())
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
            Ok(Response::new(proto::RotateNodeCertificateResponse {
                result: Some(proto::ResultMeta {
                    operation_id: "rotate-op".to_string(),
                    status: "ok".to_string(),
                    node_observed_generation: "".to_string(),
                    error_code: "".to_string(),
                    human_summary: "rotated".to_string(),
                }),
                certificate_pem: b"rotated-cert".to_vec(),
                private_key_pem: b"rotated-key".to_vec(),
                ca_pem: b"rotated-ca".to_vec(),
            }))
        }

        async fn report_bootstrap_result(
            &self,
            _request: Request<proto::ReportBootstrapResultRequest>,
        ) -> Result<Response<proto::AckResponse>, Status> {
            Ok(Response::new(proto::AckResponse {
                result: Some(proto::ResultMeta {
                    operation_id: "bootstrap-op".to_string(),
                    status: "ok".to_string(),
                    node_observed_generation: "".to_string(),
                    error_code: "".to_string(),
                    human_summary: "reported".to_string(),
                }),
            }))
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
                    async {
                        rx.await.ok();
                    },
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

        let resp = client
            .enroll_node("token", inventory, versions)
            .await
            .unwrap();
        assert_eq!(resp.node_id, "node-123");
        assert_eq!(resp.certificate_pem, b"cert-pem");
        assert_eq!(resp.private_key_pem, b"key-pem");
        assert_eq!(resp.ca_pem, b"ca-pem");

        let _ = tx.send(());
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), server).await;
    }

    #[tokio::test]
    async fn mock_rotate_certificate_success() {
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
                    async {
                        rx.await.ok();
                    },
                );
            server.await.unwrap();
        });

        let mut client = EnrollmentClient::connect(format!("http://{}", bound_addr))
            .await
            .unwrap();
        let meta = proto::RequestMeta {
            operation_id: "rotate-op".to_string(),
            requested_by: "agent".to_string(),
            target_node_id: "node-123".to_string(),
            desired_state_version: "".to_string(),
            request_unix_ms: 0,
        };

        let resp = client
            .rotate_node_certificate(meta, "node-123")
            .await
            .unwrap();
        assert_eq!(resp.certificate_pem, b"rotated-cert");
        assert_eq!(resp.private_key_pem, b"rotated-key");
        assert_eq!(resp.ca_pem, b"rotated-ca");

        let _ = tx.send(());
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), server).await;
    }

    #[tokio::test]
    async fn mock_report_bootstrap_result_success() {
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
                    async {
                        rx.await.ok();
                    },
                );
            server.await.unwrap();
        });

        let mut client = EnrollmentClient::connect(format!("http://{}", bound_addr))
            .await
            .unwrap();
        let meta = proto::RequestMeta {
            operation_id: "bootstrap-op".to_string(),
            requested_by: "agent".to_string(),
            target_node_id: "node-123".to_string(),
            desired_state_version: "".to_string(),
            request_unix_ms: 0,
        };

        client
            .report_bootstrap_result(meta, "node-123", "ok", "bootstrap complete")
            .await
            .unwrap();

        let _ = tx.send(());
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), server).await;
    }
}
