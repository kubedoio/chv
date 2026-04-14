use crate::error::ControlPlaneServiceError;
use async_trait::async_trait;
use chv_controlplane_store::{NodeInventoryInput, NodeRepository, NodeVersionInput};
use chv_controlplane_types::domain::NodeId;
use control_plane_node_api::control_plane_node_api as proto;
use std::time::{SystemTime, UNIX_EPOCH};

#[async_trait]
pub trait InventoryService: Send + Sync {
    async fn report_node_inventory(
        &self,
        request: proto::ReportNodeInventoryRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn report_service_versions(
        &self,
        request: proto::ReportServiceVersionsRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;
}

#[derive(Clone)]
pub struct InventoryServiceImplementation {
    node_repo: NodeRepository,
}

impl InventoryServiceImplementation {
    pub fn new(node_repo: NodeRepository) -> Self {
        Self { node_repo }
    }

    fn now_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }
}

use chv_controlplane_types::constants::{
    COMPONENT_AGENT, COMPONENT_CHV, COMPONENT_HOST, COMPONENT_NWD, COMPONENT_STORD,
    SOURCE_PERIODIC, STATUS_OK, SUMMARY_INVENTORY_REPORTED, SUMMARY_VERSIONS_REPORTED,
};

#[async_trait]
impl InventoryService for InventoryServiceImplementation {
    async fn report_node_inventory(
        &self,
        request: proto::ReportNodeInventoryRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let inventory = request
            .inventory
            .ok_or_else(|| ControlPlaneServiceError::InvalidArgument("missing inventory".into()))?;

        let meta = request
            .meta
            .ok_or_else(|| ControlPlaneServiceError::InvalidArgument("missing meta".into()))?;

        let node_id = NodeId::new(inventory.node_id.clone()).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid node_id: {}", e))
        })?;

        let now = self.now_ms();

        // Convert lists to JSONB
        let storage_classes = if inventory.storage_classes.is_empty() {
            None
        } else {
            Some(
                serde_json::to_value(&inventory.storage_classes).map_err(|e| {
                    ControlPlaneServiceError::Internal(format!(
                        "failed to serialize storage_classes: {}",
                        e
                    ))
                })?,
            )
        };

        let network_capabilities = if inventory.network_capabilities.is_empty() {
            None
        } else {
            Some(
                serde_json::to_value(&inventory.network_capabilities).map_err(|e| {
                    ControlPlaneServiceError::Internal(format!(
                        "failed to serialize network_capabilities: {}",
                        e
                    ))
                })?,
            )
        };

        let labels = if inventory.labels.is_empty() {
            None
        } else {
            Some(serde_json::to_value(&inventory.labels).map_err(|e| {
                ControlPlaneServiceError::Internal(format!("failed to serialize labels: {}", e))
            })?)
        };

        self.node_repo
            .upsert_inventory(&NodeInventoryInput {
                node_id: node_id.clone(),
                architecture: inventory.architecture.clone(),
                kernel_version: None,
                os_release: None,
                cpu_count: inventory.cpu_threads as i32,
                memory_bytes: inventory.memory_bytes as i64,
                disk_bytes: None,
                cloud_hypervisor_version: None,
                chv_agent_version: None,
                chv_stord_version: None,
                chv_nwd_version: None,
                host_bundle_version: None,
                inventory_status: Some(SOURCE_PERIODIC.into()),
                storage_classes,
                network_capabilities,
                labels,
                reported_unix_ms: now,
            })
            .await?;

        Ok(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id,
                status: STATUS_OK.into(),
                node_observed_generation: "".into(),
                error_code: "".into(),
                human_summary: SUMMARY_INVENTORY_REPORTED.into(),
            }),
        })
    }

    async fn report_service_versions(
        &self,
        request: proto::ReportServiceVersionsRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let versions = request
            .versions
            .ok_or_else(|| ControlPlaneServiceError::InvalidArgument("missing versions".into()))?;

        let meta = request
            .meta
            .ok_or_else(|| ControlPlaneServiceError::InvalidArgument("missing meta".into()))?;

        let node_id = NodeId::new(versions.node_id.clone()).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid node_id: {}", e))
        })?;

        let now = self.now_ms();

        let components = [
            (COMPONENT_AGENT, versions.chv_agent_version),
            (COMPONENT_STORD, versions.chv_stord_version),
            (COMPONENT_NWD, versions.chv_nwd_version),
            (COMPONENT_CHV, versions.cloud_hypervisor_version),
            (COMPONENT_HOST, versions.host_bundle_version),
        ];

        for (name, version) in components {
            if !version.is_empty() {
                self.node_repo
                    .append_version(&NodeVersionInput {
                        node_id: node_id.clone(),
                        component_name: name.into(),
                        version,
                        source: Some(SOURCE_PERIODIC.into()),
                        reported_unix_ms: now,
                    })
                    .await?;
            }
        }

        Ok(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id,
                status: STATUS_OK.into(),
                node_observed_generation: "".into(),
                error_code: "".into(),
                human_summary: SUMMARY_VERSIONS_REPORTED.into(),
            }),
        })
    }
}
