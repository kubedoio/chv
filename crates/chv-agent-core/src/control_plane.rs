use crate::cache::NodeCache;
use chv_errors::ChvError;
use control_plane_node_api::control_plane_node_api as proto;
use tonic::transport::Channel;

pub struct ControlPlaneClient {
    reconcile: proto::reconcile_service_client::ReconcileServiceClient<Channel>,
}

impl ControlPlaneClient {
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
            reconcile: proto::reconcile_service_client::ReconcileServiceClient::new(channel),
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
            let current = cache
                .get_generation(kind, id)
                .cloned()
                .unwrap_or_default();
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
        assert!(result.is_ok(), "non-numeric generations should not be treated as stale");
    }
}
