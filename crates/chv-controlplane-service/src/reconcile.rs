use crate::error::ControlPlaneServiceError;
use async_trait::async_trait;
use chv_controlplane_store::{
    DesiredStateRepository, EventAppendInput, EventRepository, NetworkExposureInput,
    NetworkExposureRepository, NodeRepository, NodeStateInput, VmDesiredStateInput,
    VolumeDesiredStateInput, NetworkDesiredStateInput,
};
use chv_controlplane_types::constants::STATUS_OK;
use chv_controlplane_types::domain::{
    EventSeverity, EventType, Generation, NodeId, OperationId, ResourceId, ResourceKind,
    NodeState,
};
use chv_controlplane_types::fragment::{NetworkSpec, NodeSpec, VmSpec, VolumeSpec};
use control_plane_node_api::control_plane_node_api as proto;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

#[async_trait]
pub trait ReconcileService: Send + Sync {
    async fn apply_node_desired_state(
        &self,
        request: proto::ApplyNodeDesiredStateRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn apply_vm_desired_state(
        &self,
        request: proto::ApplyVmDesiredStateRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn apply_volume_desired_state(
        &self,
        request: proto::ApplyVolumeDesiredStateRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn apply_network_desired_state(
        &self,
        request: proto::ApplyNetworkDesiredStateRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn acknowledge_desired_state_version(
        &self,
        request: proto::AcknowledgeDesiredStateVersionRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;
}

#[derive(Clone)]
pub struct ReconcileServiceImplementation {
    node_repo: NodeRepository,
    desired_state_repo: DesiredStateRepository,
    network_exposure_repo: NetworkExposureRepository,
    event_repo: EventRepository,
}

impl ReconcileServiceImplementation {
    pub fn new(
        node_repo: NodeRepository,
        desired_state_repo: DesiredStateRepository,
        network_exposure_repo: NetworkExposureRepository,
        event_repo: EventRepository,
    ) -> Self {
        Self {
            node_repo,
            desired_state_repo,
            network_exposure_repo,
            event_repo,
        }
    }

    fn now_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

    fn meta_from_request(
        &self,
        meta: Option<proto::RequestMeta>,
    ) -> Result<proto::RequestMeta, ControlPlaneServiceError> {
        meta.ok_or_else(|| ControlPlaneServiceError::InvalidArgument("missing meta".into()))
    }

    async fn emit_event(
        &self,
        event_type: EventType,
        node_id: Option<NodeId>,
        resource_kind: Option<ResourceKind>,
        resource_id: Option<ResourceId>,
        operation_id: Option<OperationId>,
        message: String,
        occurred_unix_ms: i64,
        requested_by: Option<String>,
    ) -> Result<(), ControlPlaneServiceError> {
        self.event_repo
            .append(&EventAppendInput {
                occurred_unix_ms,
                event_type,
                severity: if event_type == EventType::DesiredStateRejected {
                    EventSeverity::Warning
                } else {
                    EventSeverity::Info
                },
                resource_kind,
                resource_id,
                node_id,
                operation_id,
                actor_id: None,
                requested_by,
                correlation_id: None,
                message,
                details: None,
            })
            .await?;
        Ok(())
    }
}

#[async_trait]
impl ReconcileService for ReconcileServiceImplementation {
    async fn apply_node_desired_state(
        &self,
        request: proto::ApplyNodeDesiredStateRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = NodeId::new(request.node_id).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid node_id: {}", e))
        })?;

        let fragment = request
            .fragment
            .ok_or_else(|| ControlPlaneServiceError::InvalidArgument("missing fragment".into()))?;

        let generation = Generation::from_str(&fragment.generation).map_err(|_| {
            ControlPlaneServiceError::InvalidArgument("generation must be numeric".into())
        })?;

        let spec: NodeSpec = serde_json::from_slice(&fragment.spec_json).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid spec_json: {}", e))
        })?;

        let desired_state = NodeState::from_str(&spec.desired_state).map_err(|_| {
            ControlPlaneServiceError::InvalidArgument(format!(
                "invalid node state: {}",
                spec.desired_state
            ))
        })?;

        let now = self.now_ms();

        if let Err(e) = self
            .node_repo
            .upsert_state(&NodeStateInput {
                node_id: node_id.clone(),
                desired_state,
                desired_generation: generation,
                requested_by: if meta.requested_by.is_empty() {
                    None
                } else {
                    Some(meta.requested_by.clone())
                },
                updated_by: if fragment.updated_by.is_empty() {
                    None
                } else {
                    Some(fragment.updated_by)
                },
                state_reason: spec.state_reason,
                requested_unix_ms: now,
            })
            .await
        {
            let op_id = if meta.operation_id.is_empty() {
                None
            } else {
                Some(OperationId::new(meta.operation_id.clone()).map_err(|e| {
                    ControlPlaneServiceError::InvalidArgument(format!("invalid operation_id: {}", e))
                })?)
            };
            self.emit_event(
                EventType::DesiredStateRejected,
                Some(node_id.clone()),
                Some(ResourceKind::Node),
                Some(ResourceId::new(node_id.as_str()).unwrap()),
                op_id,
                format!("failed to apply node desired state: {}", e),
                now,
                if meta.requested_by.is_empty() {
                    None
                } else {
                    Some(meta.requested_by.clone())
                },
            )
            .await?;
            return Err(e.into());
        }

        let op_id = if meta.operation_id.is_empty() {
            None
        } else {
            Some(OperationId::new(meta.operation_id.clone()).map_err(|e| {
                ControlPlaneServiceError::InvalidArgument(format!("invalid operation_id: {}", e))
            })?)
        };
        self.emit_event(
            EventType::DesiredStateApplied,
            Some(node_id.clone()),
            Some(ResourceKind::Node),
            Some(ResourceId::new(node_id.as_str()).unwrap()),
            op_id,
            "node desired state applied".into(),
            now,
            if meta.requested_by.is_empty() {
                None
            } else {
                Some(meta.requested_by.clone())
            },
        )
        .await?;

        Ok(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id,
                status: STATUS_OK.into(),
                node_observed_generation: "".into(),
                error_code: "".into(),
                human_summary: "node desired state applied".into(),
            }),
        })
    }

    async fn apply_vm_desired_state(
        &self,
        request: proto::ApplyVmDesiredStateRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = NodeId::new(request.node_id).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid node_id: {}", e))
        })?;
        let vm_id = ResourceId::new(request.vm_id).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid vm_id: {}", e))
        })?;

        let fragment = request
            .fragment
            .ok_or_else(|| ControlPlaneServiceError::InvalidArgument("missing fragment".into()))?;

        let generation = Generation::from_str(&fragment.generation).map_err(|_| {
            ControlPlaneServiceError::InvalidArgument("generation must be numeric".into())
        })?;

        let spec: VmSpec = serde_json::from_slice(&fragment.spec_json).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid spec_json: {}", e))
        })?;

        let now = self.now_ms();

        if let Err(e) = self
            .desired_state_repo
            .upsert_vm(&VmDesiredStateInput {
                vm_id: vm_id.clone(),
                node_id: Some(node_id.clone()),
                display_name: fragment.id.clone(),
                tenant_id: None,
                placement_policy: None,
                desired_generation: generation,
                desired_status: STATUS_OK.into(),
                requested_by: if meta.requested_by.is_empty() {
                    None
                } else {
                    Some(meta.requested_by.clone())
                },
                updated_by: if fragment.updated_by.is_empty() {
                    None
                } else {
                    Some(fragment.updated_by)
                },
                target_node_id: Some(node_id.clone()),
                cpu_count: spec.cpu_count,
                memory_bytes: spec.memory_bytes,
                image_ref: spec.image_ref,
                boot_mode: spec.boot_mode,
                desired_power_state: spec.desired_power_state,
                requested_unix_ms: now,
            })
            .await
        {
            let op_id = if meta.operation_id.is_empty() {
                None
            } else {
                Some(OperationId::new(meta.operation_id.clone()).map_err(|e| {
                    ControlPlaneServiceError::InvalidArgument(format!("invalid operation_id: {}", e))
                })?)
            };
            self.emit_event(
                EventType::DesiredStateRejected,
                Some(node_id.clone()),
                Some(ResourceKind::Vm),
                Some(vm_id.clone()),
                op_id,
                format!("failed to apply vm desired state: {}", e),
                now,
                if meta.requested_by.is_empty() {
                    None
                } else {
                    Some(meta.requested_by.clone())
                },
            )
            .await?;
            return Err(e.into());
        }

        let op_id = if meta.operation_id.is_empty() {
            None
        } else {
            Some(OperationId::new(meta.operation_id.clone()).map_err(|e| {
                ControlPlaneServiceError::InvalidArgument(format!("invalid operation_id: {}", e))
            })?)
        };
        self.emit_event(
            EventType::DesiredStateApplied,
            Some(node_id.clone()),
            Some(ResourceKind::Vm),
            Some(vm_id.clone()),
            op_id,
            "vm desired state applied".into(),
            now,
            if meta.requested_by.is_empty() {
                None
            } else {
                Some(meta.requested_by.clone())
            },
        )
        .await?;

        Ok(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id,
                status: STATUS_OK.into(),
                node_observed_generation: "".into(),
                error_code: "".into(),
                human_summary: "vm desired state applied".into(),
            }),
        })
    }

    async fn apply_volume_desired_state(
        &self,
        request: proto::ApplyVolumeDesiredStateRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = NodeId::new(request.node_id).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid node_id: {}", e))
        })?;
        let volume_id = ResourceId::new(request.volume_id).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid volume_id: {}", e))
        })?;

        let fragment = request
            .fragment
            .ok_or_else(|| ControlPlaneServiceError::InvalidArgument("missing fragment".into()))?;

        let generation = Generation::from_str(&fragment.generation).map_err(|_| {
            ControlPlaneServiceError::InvalidArgument("generation must be numeric".into())
        })?;

        let spec: VolumeSpec = serde_json::from_slice(&fragment.spec_json).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid spec_json: {}", e))
        })?;

        let attached_vm_id = spec
            .attached_vm_id
            .as_ref()
            .map(|id| ResourceId::new(id.clone()))
            .transpose()
            .map_err(|e| {
                ControlPlaneServiceError::InvalidArgument(format!("invalid attached_vm_id: {}", e))
            })?;

        let now = self.now_ms();

        if let Err(e) = self
            .desired_state_repo
            .upsert_volume(&VolumeDesiredStateInput {
                volume_id: volume_id.clone(),
                node_id: Some(node_id.clone()),
                display_name: fragment.id.clone(),
                capacity_bytes: spec.capacity_bytes,
                volume_kind: spec.volume_kind,
                storage_class: spec.storage_class,
                desired_generation: generation,
                desired_status: STATUS_OK.into(),
                requested_by: if meta.requested_by.is_empty() {
                    None
                } else {
                    Some(meta.requested_by.clone())
                },
                updated_by: if fragment.updated_by.is_empty() {
                    None
                } else {
                    Some(fragment.updated_by)
                },
                attached_vm_id,
                attachment_mode: spec.attachment_mode,
                device_name: spec.device_name,
                read_only: spec.read_only,
                requested_unix_ms: now,
            })
            .await
        {
            let op_id = if meta.operation_id.is_empty() {
                None
            } else {
                Some(OperationId::new(meta.operation_id.clone()).map_err(|e| {
                    ControlPlaneServiceError::InvalidArgument(format!("invalid operation_id: {}", e))
                })?)
            };
            self.emit_event(
                EventType::DesiredStateRejected,
                Some(node_id.clone()),
                Some(ResourceKind::Volume),
                Some(volume_id.clone()),
                op_id,
                format!("failed to apply volume desired state: {}", e),
                now,
                if meta.requested_by.is_empty() {
                    None
                } else {
                    Some(meta.requested_by.clone())
                },
            )
            .await?;
            return Err(e.into());
        }

        let op_id = if meta.operation_id.is_empty() {
            None
        } else {
            Some(OperationId::new(meta.operation_id.clone()).map_err(|e| {
                ControlPlaneServiceError::InvalidArgument(format!("invalid operation_id: {}", e))
            })?)
        };
        self.emit_event(
            EventType::DesiredStateApplied,
            Some(node_id.clone()),
            Some(ResourceKind::Volume),
            Some(volume_id.clone()),
            op_id,
            "volume desired state applied".into(),
            now,
            if meta.requested_by.is_empty() {
                None
            } else {
                Some(meta.requested_by.clone())
            },
        )
        .await?;

        Ok(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id,
                status: STATUS_OK.into(),
                node_observed_generation: "".into(),
                error_code: "".into(),
                human_summary: "volume desired state applied".into(),
            }),
        })
    }

    async fn apply_network_desired_state(
        &self,
        request: proto::ApplyNetworkDesiredStateRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = NodeId::new(request.node_id).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid node_id: {}", e))
        })?;
        let network_id = ResourceId::new(request.network_id).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid network_id: {}", e))
        })?;

        let fragment = request
            .fragment
            .ok_or_else(|| ControlPlaneServiceError::InvalidArgument("missing fragment".into()))?;

        let generation = Generation::from_str(&fragment.generation).map_err(|_| {
            ControlPlaneServiceError::InvalidArgument("generation must be numeric".into())
        })?;

        let spec: NetworkSpec = serde_json::from_slice(&fragment.spec_json).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid spec_json: {}", e))
        })?;

        let now = self.now_ms();

        if let Err(e) = self
            .desired_state_repo
            .upsert_network(&NetworkDesiredStateInput {
                network_id: network_id.clone(),
                node_id: Some(node_id.clone()),
                display_name: fragment.id.clone(),
                network_class: spec.network_class,
                desired_generation: generation,
                desired_status: STATUS_OK.into(),
                requested_by: if meta.requested_by.is_empty() {
                    None
                } else {
                    Some(meta.requested_by.clone())
                },
                updated_by: if fragment.updated_by.is_empty() {
                    None
                } else {
                    Some(fragment.updated_by)
                },
                requested_unix_ms: now,
            })
            .await
        {
            let op_id = if meta.operation_id.is_empty() {
                None
            } else {
                Some(OperationId::new(meta.operation_id.clone()).map_err(|e| {
                    ControlPlaneServiceError::InvalidArgument(format!("invalid operation_id: {}", e))
                })?)
            };
            self.emit_event(
                EventType::DesiredStateRejected,
                Some(node_id.clone()),
                Some(ResourceKind::Network),
                Some(network_id.clone()),
                op_id,
                format!("failed to apply network desired state: {}", e),
                now,
                if meta.requested_by.is_empty() {
                    None
                } else {
                    Some(meta.requested_by.clone())
                },
            )
            .await?;
            return Err(e.into());
        }

        if let Some(exposures) = spec.exposures {
            for exposure in exposures {
                if let Err(e) = self
                    .network_exposure_repo
                    .upsert(&NetworkExposureInput {
                        network_id: network_id.clone(),
                        service_name: exposure.service_name,
                        protocol: exposure.protocol,
                        listen_address: exposure.listen_address,
                        listen_port: exposure.listen_port,
                        target_address: exposure.target_address,
                        target_port: exposure.target_port,
                        exposure_policy: exposure.exposure_policy,
                        updated_unix_ms: now,
                    })
                    .await
                {
                    let op_id = if meta.operation_id.is_empty() {
                        None
                    } else {
                        Some(OperationId::new(meta.operation_id.clone()).map_err(|e| {
                            ControlPlaneServiceError::InvalidArgument(format!("invalid operation_id: {}", e))
                        })?)
                    };
                    self.emit_event(
                        EventType::DesiredStateRejected,
                        Some(node_id.clone()),
                        Some(ResourceKind::Network),
                        Some(network_id.clone()),
                        op_id,
                        format!("failed to apply network exposure: {}", e),
                        now,
                        if meta.requested_by.is_empty() {
                            None
                        } else {
                            Some(meta.requested_by.clone())
                        },
                    )
                    .await?;
                    return Err(e.into());
                }
            }
        }

        let op_id = if meta.operation_id.is_empty() {
            None
        } else {
            Some(OperationId::new(meta.operation_id.clone()).map_err(|e| {
                ControlPlaneServiceError::InvalidArgument(format!("invalid operation_id: {}", e))
            })?)
        };
        self.emit_event(
            EventType::DesiredStateApplied,
            Some(node_id.clone()),
            Some(ResourceKind::Network),
            Some(network_id.clone()),
            op_id,
            "network desired state applied".into(),
            now,
            if meta.requested_by.is_empty() {
                None
            } else {
                Some(meta.requested_by.clone())
            },
        )
        .await?;

        Ok(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id,
                status: STATUS_OK.into(),
                node_observed_generation: "".into(),
                error_code: "".into(),
                human_summary: "network desired state applied".into(),
            }),
        })
    }

    async fn acknowledge_desired_state_version(
        &self,
        request: proto::AcknowledgeDesiredStateVersionRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = NodeId::new(request.node_id).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid node_id: {}", e))
        })?;
        let resource_id = ResourceId::new(request.fragment_id).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid fragment_id: {}", e))
        })?;
        let resource_kind = ResourceKind::from_str(&request.fragment_kind).map_err(|_| {
            ControlPlaneServiceError::InvalidArgument(format!(
                "invalid fragment_kind: {}",
                request.fragment_kind
            ))
        })?;

        let now = self.now_ms();

        let op_id = if meta.operation_id.is_empty() {
            None
        } else {
            Some(OperationId::new(meta.operation_id.clone()).map_err(|e| {
                ControlPlaneServiceError::InvalidArgument(format!("invalid operation_id: {}", e))
            })?)
        };

        self.emit_event(
            EventType::DesiredStateApplied,
            Some(node_id.clone()),
            Some(resource_kind),
            Some(resource_id),
            op_id,
            format!(
                "desired state version {} acknowledged",
                request.observed_generation
            ),
            now,
            if meta.requested_by.is_empty() {
                None
            } else {
                Some(meta.requested_by.clone())
            },
        )
        .await?;

        Ok(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id,
                status: STATUS_OK.into(),
                node_observed_generation: request.observed_generation,
                error_code: "".into(),
                human_summary: "desired state version acknowledged".into(),
            }),
        })
    }
}
