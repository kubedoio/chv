use crate::cache::{NodeCache, PendingControlPlaneMessage, PendingControlPlaneMessageKind};
use chv_errors::ChvError;
use control_plane_node_api::control_plane_node_api as proto;
use tonic::transport::Channel;

pub struct ControlPlaneClient {
    reconcile: proto::reconcile_service_client::ReconcileServiceClient<Channel>,
    telemetry: proto::telemetry_service_client::TelemetryServiceClient<Channel>,
    inventory: proto::inventory_service_client::InventoryServiceClient<Channel>,
}

impl ControlPlaneClient {
    pub async fn new(
        endpoint: impl Into<String>,
        tls_cert_path: Option<&std::path::Path>,
        tls_key_path: Option<&std::path::Path>,
        ca_cert_path: Option<&std::path::Path>,
    ) -> Result<Self, ChvError> {
        let endpoint = endpoint.into();
        let mut endpoint = tonic::transport::Endpoint::try_from(endpoint).map_err(|e| {
            ChvError::InvalidArgument {
                field: "control_plane_addr".to_string(),
                reason: e.to_string(),
            }
        })?;

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
            reconcile: proto::reconcile_service_client::ReconcileServiceClient::new(
                channel.clone(),
            ),
            telemetry: proto::telemetry_service_client::TelemetryServiceClient::new(
                channel.clone(),
            ),
            inventory: proto::inventory_service_client::InventoryServiceClient::new(channel),
        })
    }

    pub fn stale_generation_check(
        meta: &proto::RequestMeta,
        cache: &NodeCache,
        kind: &str,
        id: &str,
    ) -> Result<(), ChvError> {
        let incoming = &meta.desired_state_version;
        if cache.is_stale(kind, id, incoming)? {
            let current = cache.get_generation(kind, id).cloned().unwrap_or_default();
            return Err(ChvError::StaleGeneration {
                resource: kind.to_string(),
                id: id.to_string(),
                expected: current,
                got: incoming.clone(),
            });
        }
        Ok(())
    }

    pub async fn apply_node_desired_state(
        &mut self,
        req: proto::ApplyNodeDesiredStateRequest,
    ) -> Result<proto::AckResponse, ChvError> {
        self.reconcile
            .apply_node_desired_state(req)
            .await
            .map(|r| r.into_inner())
            .map_err(|e| ChvError::ControlPlaneUnavailable {
                reason: e.to_string(),
            })
    }

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

    pub async fn report_volume_state(
        &mut self,
        req: proto::VolumeStateReport,
    ) -> Result<proto::AckResponse, ChvError> {
        self.telemetry
            .report_volume_state(req)
            .await
            .map(|r| r.into_inner())
            .map_err(|e| ChvError::ControlPlaneUnavailable {
                reason: e.to_string(),
            })
    }

    pub async fn report_network_state(
        &mut self,
        req: proto::NetworkStateReport,
    ) -> Result<proto::AckResponse, ChvError> {
        self.telemetry
            .report_network_state(req)
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

    pub async fn publish_alert(
        &mut self,
        req: proto::PublishAlertRequest,
    ) -> Result<proto::AckResponse, ChvError> {
        self.telemetry
            .publish_alert(req)
            .await
            .map(|r| r.into_inner())
            .map_err(|e| ChvError::ControlPlaneUnavailable {
                reason: e.to_string(),
            })
    }

    pub async fn report_node_inventory(
        &mut self,
        req: proto::ReportNodeInventoryRequest,
    ) -> Result<proto::AckResponse, ChvError> {
        self.inventory
            .report_node_inventory(req)
            .await
            .map(|r| r.into_inner())
            .map_err(|e| ChvError::ControlPlaneUnavailable {
                reason: e.to_string(),
            })
    }

    pub async fn report_service_versions(
        &mut self,
        req: proto::ReportServiceVersionsRequest,
    ) -> Result<proto::AckResponse, ChvError> {
        self.inventory
            .report_service_versions(req)
            .await
            .map(|r| r.into_inner())
            .map_err(|e| ChvError::ControlPlaneUnavailable {
                reason: e.to_string(),
            })
    }

    pub async fn dispatch_pending_message(
        &mut self,
        message: &PendingControlPlaneMessage,
    ) -> Result<(), ChvError> {
        match message.kind {
            PendingControlPlaneMessageKind::NodeStateReport => {
                self.report_node_state(message.decode_node_state()?).await?;
            }
            PendingControlPlaneMessageKind::VmStateReport => {
                self.report_vm_state(message.decode_vm_state()?).await?;
            }
            PendingControlPlaneMessageKind::VolumeStateReport => {
                self.report_volume_state(message.decode_volume_state()?)
                    .await?;
            }
            PendingControlPlaneMessageKind::NetworkStateReport => {
                self.report_network_state(message.decode_network_state()?)
                    .await?;
            }
            PendingControlPlaneMessageKind::PublishEvent => {
                self.publish_event(message.decode_event()?).await?;
            }
            PendingControlPlaneMessageKind::PublishAlert => {
                self.publish_alert(message.decode_alert()?).await?;
            }
            PendingControlPlaneMessageKind::ReportNodeInventory => {
                self.report_node_inventory(message.decode_node_inventory()?)
                    .await?;
            }
            PendingControlPlaneMessageKind::ReportServiceVersions => {
                self.report_service_versions(message.decode_service_versions()?)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn flush_pending_messages(&mut self, cache: &mut NodeCache) -> Result<(), ChvError> {
        let pending = cache.pending_control_plane_messages().to_vec();
        if pending.is_empty() {
            return Ok(());
        }

        let mut unsent = Vec::new();
        for message in pending {
            if let Err(e) = self.dispatch_pending_message(&message).await {
                unsent.push(message);
                cache.replace_pending_control_plane_messages(unsent);
                return Err(e);
            }
        }

        cache.replace_pending_control_plane_messages(Vec::new());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::PendingControlPlaneMessage;
    use proto::{
        inventory_service_server::{InventoryService, InventoryServiceServer},
        telemetry_service_server::{TelemetryService, TelemetryServiceServer},
    };
    use std::net::SocketAddr;
    use std::sync::{Arc, Mutex};
    use tokio::sync::oneshot;
    use tonic::{Request, Response, Status};

    #[test]
    fn stale_generation_rejected() {
        let mut cache = NodeCache::new("node-1");
        cache.observe_generation("node", "node-1", "10");

        let meta = proto::RequestMeta {
            operation_id: "op-1".to_string(),
            requested_by: "cp".to_string(),
            target_node_id: "node-1".to_string(),
            desired_state_version: "9".to_string(),
            request_unix_ms: 0,
        };

        let result = ControlPlaneClient::stale_generation_check(&meta, &cache, "node", "node-1");
        assert!(matches!(result, Err(ChvError::StaleGeneration { .. })));
    }

    #[test]
    fn fresh_generation_accepted() {
        let mut cache = NodeCache::new("node-1");
        cache.observe_generation("node", "node-1", "10");

        let meta = proto::RequestMeta {
            operation_id: "op-1".to_string(),
            requested_by: "cp".to_string(),
            target_node_id: "node-1".to_string(),
            desired_state_version: "11".to_string(),
            request_unix_ms: 0,
        };

        let result = ControlPlaneClient::stale_generation_check(&meta, &cache, "node", "node-1");
        assert!(result.is_ok());
    }

    #[test]
    fn non_numeric_generation_rejected() {
        let mut cache = NodeCache::new("node-1");
        cache.observe_generation("node", "node-1", "v2");

        let meta = proto::RequestMeta {
            operation_id: "op-1".to_string(),
            requested_by: "cp".to_string(),
            target_node_id: "node-1".to_string(),
            desired_state_version: "v3".to_string(),
            request_unix_ms: 0,
        };

        let result = ControlPlaneClient::stale_generation_check(&meta, &cache, "node", "node-1");
        assert!(
            matches!(result, Err(ChvError::InvalidArgument { .. })),
            "non-numeric generations should be rejected cleanly"
        );
    }

    #[derive(Default)]
    struct MockTelemetryService {
        node_reports: Arc<Mutex<Vec<proto::NodeStateReport>>>,
        events: Arc<Mutex<Vec<proto::PublishEventRequest>>>,
    }

    #[tonic::async_trait]
    impl TelemetryService for MockTelemetryService {
        async fn report_node_state(
            &self,
            request: Request<proto::NodeStateReport>,
        ) -> Result<Response<proto::AckResponse>, Status> {
            self.node_reports.lock().unwrap().push(request.into_inner());
            Ok(Response::new(proto::AckResponse {
                result: Some(proto::ResultMeta {
                    operation_id: "node-report".to_string(),
                    status: "ok".to_string(),
                    node_observed_generation: String::new(),
                    error_code: String::new(),
                    human_summary: "ok".to_string(),
                }),
            }))
        }

        async fn report_vm_state(
            &self,
            _request: Request<proto::VmStateReport>,
        ) -> Result<Response<proto::AckResponse>, Status> {
            Ok(Response::new(proto::AckResponse {
                result: Some(proto::ResultMeta {
                    operation_id: "vm-report".to_string(),
                    status: "ok".to_string(),
                    node_observed_generation: String::new(),
                    error_code: String::new(),
                    human_summary: "ok".to_string(),
                }),
            }))
        }

        async fn report_volume_state(
            &self,
            _request: Request<proto::VolumeStateReport>,
        ) -> Result<Response<proto::AckResponse>, Status> {
            Ok(Response::new(proto::AckResponse {
                result: Some(proto::ResultMeta {
                    operation_id: "volume-report".to_string(),
                    status: "ok".to_string(),
                    node_observed_generation: String::new(),
                    error_code: String::new(),
                    human_summary: "ok".to_string(),
                }),
            }))
        }

        async fn report_network_state(
            &self,
            _request: Request<proto::NetworkStateReport>,
        ) -> Result<Response<proto::AckResponse>, Status> {
            Ok(Response::new(proto::AckResponse {
                result: Some(proto::ResultMeta {
                    operation_id: "network-report".to_string(),
                    status: "ok".to_string(),
                    node_observed_generation: String::new(),
                    error_code: String::new(),
                    human_summary: "ok".to_string(),
                }),
            }))
        }

        async fn publish_event(
            &self,
            request: Request<proto::PublishEventRequest>,
        ) -> Result<Response<proto::AckResponse>, Status> {
            self.events.lock().unwrap().push(request.into_inner());
            Ok(Response::new(proto::AckResponse {
                result: Some(proto::ResultMeta {
                    operation_id: "event-report".to_string(),
                    status: "ok".to_string(),
                    node_observed_generation: String::new(),
                    error_code: String::new(),
                    human_summary: "ok".to_string(),
                }),
            }))
        }

        async fn publish_alert(
            &self,
            _request: Request<proto::PublishAlertRequest>,
        ) -> Result<Response<proto::AckResponse>, Status> {
            Ok(Response::new(proto::AckResponse {
                result: Some(proto::ResultMeta {
                    operation_id: "alert-report".to_string(),
                    status: "ok".to_string(),
                    node_observed_generation: String::new(),
                    error_code: String::new(),
                    human_summary: "ok".to_string(),
                }),
            }))
        }
    }

    #[derive(Default)]
    struct MockInventoryService;

    #[tonic::async_trait]
    impl InventoryService for MockInventoryService {
        async fn report_node_inventory(
            &self,
            _request: Request<proto::ReportNodeInventoryRequest>,
        ) -> Result<Response<proto::AckResponse>, Status> {
            Ok(Response::new(proto::AckResponse {
                result: Some(proto::ResultMeta {
                    operation_id: "inventory-report".to_string(),
                    status: "ok".to_string(),
                    node_observed_generation: String::new(),
                    error_code: String::new(),
                    human_summary: "ok".to_string(),
                }),
            }))
        }

        async fn report_service_versions(
            &self,
            _request: Request<proto::ReportServiceVersionsRequest>,
        ) -> Result<Response<proto::AckResponse>, Status> {
            Ok(Response::new(proto::AckResponse {
                result: Some(proto::ResultMeta {
                    operation_id: "versions-report".to_string(),
                    status: "ok".to_string(),
                    node_observed_generation: String::new(),
                    error_code: String::new(),
                    human_summary: "ok".to_string(),
                }),
            }))
        }
    }

    #[tokio::test]
    async fn flush_pending_messages_drains_cache_outbox() {
        let telemetry = MockTelemetryService::default();
        let node_reports = telemetry.node_reports.clone();
        let events = telemetry.events.clone();
        let inventory = MockInventoryService;

        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        let bound_addr = listener.local_addr().unwrap();

        let (tx, rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let server = tonic::transport::Server::builder()
                .add_service(TelemetryServiceServer::new(telemetry))
                .add_service(InventoryServiceServer::new(inventory))
                .serve_with_incoming_shutdown(
                    tokio_stream::wrappers::TcpListenerStream::new(listener),
                    async {
                        rx.await.ok();
                    },
                );
            server.await.unwrap();
        });

        let mut client =
            ControlPlaneClient::new(format!("http://{}", bound_addr), None, None, None)
                .await
                .unwrap();

        let mut cache = NodeCache::new("node-1");
        cache.enqueue_pending_message(PendingControlPlaneMessage::node_state(
            proto::NodeStateReport {
                node_id: "node-1".to_string(),
                state: "TenantReady".to_string(),
                observed_generation: "7".to_string(),
                health_status: "Healthy".to_string(),
                last_error: String::new(),
                reported_unix_ms: 0,
            },
        ));
        cache.enqueue_pending_message(PendingControlPlaneMessage::event(
            proto::PublishEventRequest {
                meta: Some(proto::RequestMeta {
                    operation_id: "op-1".to_string(),
                    requested_by: "agent".to_string(),
                    target_node_id: "node-1".to_string(),
                    desired_state_version: "7".to_string(),
                    request_unix_ms: 0,
                }),
                node_id: "node-1".to_string(),
                severity: "warning".to_string(),
                event_type: "NodeStateTransition".to_string(),
                summary: "deferred".to_string(),
                details_json: vec![],
            },
        ));

        client.flush_pending_messages(&mut cache).await.unwrap();

        assert!(cache.pending_control_plane_messages().is_empty());
        assert_eq!(node_reports.lock().unwrap().len(), 1);
        assert_eq!(events.lock().unwrap().len(), 1);

        let _ = tx.send(());
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), server).await;
    }
}
