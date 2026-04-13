use crate::cache::NodeCache;
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
            telemetry: proto::telemetry_service_client::TelemetryServiceClient::new(channel.clone()),
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
        if cache.is_stale(kind, id, incoming) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn non_numeric_generation_accepted() {
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
            result.is_ok(),
            "non-numeric generations should not be treated as stale"
        );
    }
}
