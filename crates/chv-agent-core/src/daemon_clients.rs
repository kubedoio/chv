use chv_errors::ChvError;
use chv_nwd_api::chv_nwd_api::{
    network_service_client::NetworkServiceClient, AttachVmNicRequest, DeleteNetworkTopologyRequest,
    DetachVmNicRequest, DhcpScope, DnsScope, EnsureDhcpScopeRequest, EnsureDnsScopeRequest,
    EnsureNetworkTopologyRequest, ExposeServiceRequest, ListNamespaceStateRequest,
    NetworkHealthRequest, SendGratuitousArpRequest, SetFirewallPolicyRequest, SetNatPolicyRequest,
    UpdateOverlayRequest, WithdrawServiceExposureRequest,
};
use chv_stord_api::chv_stord_api::{
    storage_service_client::StorageServiceClient, AttachVolumeToVmRequest, CloseVolumeRequest,
    DeleteSnapshotRequest, DetachVolumeFromVmRequest, DevicePolicy, GetDiskMigrationStatusRequest,
    ListVolumeSessionsRequest, OpenVolumeRequest, PrepareCloneRequest, PrepareSnapshotRequest,
    ResizeVolumeRequest, RestoreSnapshotRequest, ResumeDiskMigrationRequest,
    SetDevicePolicyRequest, TriggerDiskMigrationRequest, VolumeHealthRequest,
};
use std::path::Path;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;
use tracing::Instrument;

fn with_operation_id<T>(req: T, operation_id: Option<&str>) -> tonic::Request<T> {
    let mut grpc_req = tonic::Request::new(req);
    if let Some(op_id) = operation_id {
        if let Ok(val) = tonic::metadata::MetadataValue::try_from(op_id) {
            grpc_req
                .metadata_mut()
                .insert(chv_common::OPERATION_ID_METADATA_KEY, val);
        }
    }
    grpc_req
}

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
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(5))
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
        let span = tracing::info_span!("stord_health_probe");
        let _ = self
            .inner
            .list_volume_sessions(ListVolumeSessionsRequest {})
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "stord".to_string(),
                reason: e.to_string(),
            })?;
        Ok(true)
    }

    pub async fn open_volume(
        &mut self,
        volume_id: &str,
        backend_class: &str,
        locator: &str,
        operation_id: Option<&str>,
    ) -> Result<(String, String, String), ChvError> {
        self.open_volume_with_options(
            volume_id,
            backend_class,
            locator,
            std::collections::HashMap::new(),
            operation_id,
        )
        .await
    }

    pub async fn open_volume_with_options(
        &mut self,
        volume_id: &str,
        backend_class: &str,
        locator: &str,
        mut options: std::collections::HashMap<String, String>,
        operation_id: Option<&str>,
    ) -> Result<(String, String, String), ChvError> {
        options
            .entry("volume_id".to_string())
            .or_insert_with(|| volume_id.to_string());
        let req = OpenVolumeRequest {
            meta: Some(chv_stord_api::chv_stord_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            volume_id: volume_id.to_string(),
            backend: Some(chv_stord_api::chv_stord_api::BackendLocator {
                backend_class: backend_class.to_string(),
                locator: locator.to_string(),
                options,
            }),
            policy: None,
        };
        let span = tracing::info_span!("open_volume", operation_id = operation_id.unwrap_or(""));
        let resp = self
            .inner
            .open_volume(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "stord".to_string(),
                reason: e.to_string(),
            })?
            .into_inner();
        if let Some(ref res) = resp.result {
            if !res.status.eq_ignore_ascii_case("ok")
                && !res.status.eq_ignore_ascii_case("0")
                && !res.status.is_empty()
            {
                return Err(ChvError::BackendUnavailable {
                    backend: "stord".to_string(),
                    reason: format!(
                        "stord open_volume failed (code {}): {}",
                        res.error_code, res.human_summary
                    ),
                });
            }
        }
        Ok((resp.volume_id, resp.attachment_handle, resp.export_path))
    }

    pub async fn attach_volume_to_vm(
        &mut self,
        volume_id: &str,
        vm_id: &str,
        attachment_handle: &str,
        operation_id: Option<&str>,
    ) -> Result<(String, String), ChvError> {
        let req = AttachVolumeToVmRequest {
            meta: Some(chv_stord_api::chv_stord_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            volume_id: volume_id.to_string(),
            vm_id: vm_id.to_string(),
            attachment_handle: attachment_handle.to_string(),
        };
        let span = tracing::info_span!(
            "attach_volume_to_vm",
            operation_id = operation_id.unwrap_or("")
        );
        let resp = self
            .inner
            .attach_volume_to_vm(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "stord".to_string(),
                reason: e.to_string(),
            })?
            .into_inner();
        Ok((resp.export_kind, resp.export_path))
    }

    pub async fn detach_volume_from_vm(
        &mut self,
        volume_id: &str,
        vm_id: &str,
        force: bool,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = DetachVolumeFromVmRequest {
            meta: Some(chv_stord_api::chv_stord_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            volume_id: volume_id.to_string(),
            vm_id: vm_id.to_string(),
            force,
        };
        let span = tracing::info_span!(
            "detach_volume_from_vm",
            operation_id = operation_id.unwrap_or("")
        );
        self.inner
            .detach_volume_from_vm(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "stord".to_string(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    pub async fn close_volume(
        &mut self,
        volume_id: &str,
        attachment_handle: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = CloseVolumeRequest {
            meta: Some(chv_stord_api::chv_stord_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            volume_id: volume_id.to_string(),
            attachment_handle: attachment_handle.to_string(),
        };
        let span = tracing::info_span!("close_volume", operation_id = operation_id.unwrap_or(""));
        self.inner
            .close_volume(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "stord".to_string(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    pub async fn resize_volume(
        &mut self,
        volume_id: &str,
        new_size_bytes: u64,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = ResizeVolumeRequest {
            meta: Some(chv_stord_api::chv_stord_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            volume_id: volume_id.to_string(),
            new_size_bytes,
        };
        let span = tracing::info_span!("resize_volume", operation_id = operation_id.unwrap_or(""));
        self.inner
            .resize_volume(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "stord".to_string(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    pub async fn get_volume_health(
        &mut self,
        volume_id: &str,
    ) -> Result<chv_stord_api::chv_stord_api::VolumeHealthResponse, ChvError> {
        let req = VolumeHealthRequest {
            volume_id: volume_id.to_string(),
        };
        let span = tracing::info_span!("get_volume_health");
        let resp = self
            .inner
            .get_volume_health(req)
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "stord".to_string(),
                reason: e.to_string(),
            })?
            .into_inner();
        Ok(resp)
    }

    pub async fn prepare_snapshot(
        &mut self,
        volume_id: &str,
        snapshot_name: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = PrepareSnapshotRequest {
            meta: Some(chv_stord_api::chv_stord_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            volume_id: volume_id.to_string(),
            snapshot_name: snapshot_name.to_string(),
        };
        let span = tracing::info_span!(
            "prepare_snapshot",
            operation_id = operation_id.unwrap_or("")
        );
        self.inner
            .prepare_snapshot(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "stord".to_string(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    pub async fn prepare_clone(
        &mut self,
        volume_id: &str,
        clone_name: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = PrepareCloneRequest {
            meta: Some(chv_stord_api::chv_stord_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            volume_id: volume_id.to_string(),
            clone_name: clone_name.to_string(),
        };
        let span = tracing::info_span!("prepare_clone", operation_id = operation_id.unwrap_or(""));
        self.inner
            .prepare_clone(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "stord".to_string(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    pub async fn restore_snapshot(
        &mut self,
        volume_id: &str,
        snapshot_name: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = RestoreSnapshotRequest {
            meta: Some(chv_stord_api::chv_stord_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            volume_id: volume_id.to_string(),
            snapshot_name: snapshot_name.to_string(),
        };
        let span = tracing::info_span!(
            "restore_snapshot",
            operation_id = operation_id.unwrap_or("")
        );
        self.inner
            .restore_snapshot(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "stord".to_string(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    pub async fn delete_snapshot(
        &mut self,
        volume_id: &str,
        snapshot_name: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = DeleteSnapshotRequest {
            meta: Some(chv_stord_api::chv_stord_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            volume_id: volume_id.to_string(),
            snapshot_name: snapshot_name.to_string(),
        };
        let span =
            tracing::info_span!("delete_snapshot", operation_id = operation_id.unwrap_or(""));
        self.inner
            .delete_snapshot(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "stord".to_string(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    /// Trigger disk pre-copy migration for a volume to a remote stord peer.
    ///
    /// Returns the migration_id assigned by the local stord.
    pub async fn trigger_disk_migration(
        &mut self,
        volume_id: &str,
        attachment_handle: &str,
        dest_endpoint: &str,
        operation_id: Option<&str>,
    ) -> Result<String, ChvError> {
        let req = TriggerDiskMigrationRequest {
            meta: Some(chv_stord_api::chv_stord_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            volume_id: volume_id.to_string(),
            attachment_handle: attachment_handle.to_string(),
            dest_endpoint: dest_endpoint.to_string(),
        };
        let span = tracing::info_span!(
            "trigger_disk_migration",
            volume_id = %volume_id,
            dest_endpoint = %dest_endpoint,
            operation_id = operation_id.unwrap_or("")
        );
        let resp = self
            .inner
            .trigger_disk_migration(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "stord".to_string(),
                reason: e.to_string(),
            })?
            .into_inner();
        if let Some(ref result) = resp.result {
            if !result.status.eq_ignore_ascii_case("ok") {
                return Err(ChvError::BackendUnavailable {
                    backend: "stord".to_string(),
                    reason: format!(
                        "trigger_disk_migration failed: {} ({})",
                        result.human_summary, result.error_code
                    ),
                });
            }
        }
        Ok(resp.migration_id)
    }

    /// Query the status of an active disk migration from the local stord.
    pub async fn get_disk_migration_status(
        &mut self,
        migration_id: &str,
    ) -> Result<chv_stord_api::chv_stord_api::GetDiskMigrationStatusResponse, ChvError> {
        let req = GetDiskMigrationStatusRequest {
            migration_id: migration_id.to_string(),
        };
        let span = tracing::info_span!("get_disk_migration_status", migration_id = %migration_id);
        let resp = self
            .inner
            .get_disk_migration_status(req)
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "stord".to_string(),
                reason: e.to_string(),
            })?
            .into_inner();
        Ok(resp)
    }

    /// Resume a disk migration after the VM has been paused (or unpaused).
    pub async fn resume_disk_migration(
        &mut self,
        migration_id: &str,
        vm_paused: bool,
    ) -> Result<(), ChvError> {
        let req = ResumeDiskMigrationRequest {
            migration_id: migration_id.to_string(),
            vm_paused,
        };
        let span = tracing::info_span!(
            "resume_disk_migration",
            migration_id = %migration_id,
            vm_paused = vm_paused
        );
        let resp = self
            .inner
            .resume_disk_migration(req)
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "stord".to_string(),
                reason: e.to_string(),
            })?
            .into_inner();
        if let Some(ref result) = resp.result {
            if !result.status.eq_ignore_ascii_case("ok") {
                return Err(ChvError::BackendUnavailable {
                    backend: "stord".to_string(),
                    reason: format!(
                        "resume_disk_migration failed: {} ({})",
                        result.human_summary, result.error_code
                    ),
                });
            }
        }
        Ok(())
    }

    pub async fn set_device_policy(
        &mut self,
        volume_id: &str,
        policy: DevicePolicy,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = SetDevicePolicyRequest {
            meta: Some(chv_stord_api::chv_stord_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            volume_id: volume_id.to_string(),
            policy: Some(policy),
        };
        let span = tracing::info_span!(
            "set_device_policy",
            operation_id = operation_id.unwrap_or("")
        );
        self.inner
            .set_device_policy(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "stord".to_string(),
                reason: e.to_string(),
            })?;
        Ok(())
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
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(5))
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
        let span = tracing::info_span!("nwd_health_probe");
        let _ = self
            .inner
            .list_namespace_state(ListNamespaceStateRequest {})
            .instrument(span)
            .await
            .map_err(|e| ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: e.to_string(),
            })?;
        Ok(true)
    }

    pub async fn ensure_network_topology(
        &mut self,
        network_id: &str,
        bridge_name: &str,
        subnet_cidr: &str,
        gateway_ip: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = EnsureNetworkTopologyRequest {
            meta: Some(chv_nwd_api::chv_nwd_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            topology: Some(chv_nwd_api::chv_nwd_api::TopologySpec {
                network_id: network_id.to_string(),
                tenant_id: "".to_string(),
                bridge_name: bridge_name.to_string(),
                namespace_name: format!("ns-{}", network_id),
                subnet_cidr: subnet_cidr.to_string(),
                gateway_ip: gateway_ip.to_string(),
                options: std::collections::HashMap::new(),
                vni: 0,
                vtep_endpoints: vec![],
                overlay_type: 0,
            }),
        };
        let span = tracing::info_span!(
            "ensure_network_topology",
            operation_id = operation_id.unwrap_or("")
        );
        let resp = self
            .inner
            .ensure_network_topology(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: e.to_string(),
            })?
            .into_inner();
        if !resp.status.eq_ignore_ascii_case("ok") {
            return Err(ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: format!(
                    "ensure_network_topology failed: {} ({})",
                    resp.human_summary, resp.error_code
                ),
            });
        }
        Ok(())
    }

    pub async fn attach_vm_nic(
        &mut self,
        nic_id: &str,
        vm_id: &str,
        network_id: &str,
        mac_address: &str,
        ip_address: &str,
        operation_id: Option<&str>,
    ) -> Result<(String, String), ChvError> {
        let req = AttachVmNicRequest {
            meta: Some(chv_nwd_api::chv_nwd_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            nic: Some(chv_nwd_api::chv_nwd_api::NicSpec {
                nic_id: nic_id.to_string(),
                vm_id: vm_id.to_string(),
                network_id: network_id.to_string(),
                mac_address: mac_address.to_string(),
                tap_name: "".to_string(),
                ip_address: ip_address.to_string(),
            }),
        };
        let span = tracing::info_span!("attach_vm_nic", operation_id = operation_id.unwrap_or(""));
        let resp = self
            .inner
            .attach_vm_nic(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: e.to_string(),
            })?
            .into_inner();
        if let Some(ref result) = resp.result {
            if !result.status.eq_ignore_ascii_case("ok") {
                return Err(ChvError::NetworkUnavailable {
                    resource: "nwd".to_string(),
                    reason: format!(
                        "attach_vm_nic failed: {} ({})",
                        result.human_summary, result.error_code
                    ),
                });
            }
        }
        if resp.tap_handle.is_empty() {
            return Err(ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: "attach_vm_nic returned empty tap_handle".to_string(),
            });
        }
        Ok((resp.namespace_handle, resp.tap_handle))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn expose_service(
        &mut self,
        network_id: &str,
        exposure_id: &str,
        protocol: &str,
        external_port: u32,
        target_ip: &str,
        target_port: u32,
        mode: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = ExposeServiceRequest {
            meta: Some(chv_nwd_api::chv_nwd_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            exposure: Some(chv_nwd_api::chv_nwd_api::ExposureSpec {
                network_id: network_id.to_string(),
                exposure_id: exposure_id.to_string(),
                protocol: protocol.to_string(),
                external_port,
                target_ip: target_ip.to_string(),
                target_port,
                mode: mode.to_string(),
            }),
        };
        let span = tracing::info_span!("expose_service", operation_id = operation_id.unwrap_or(""));
        self.inner
            .expose_service(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    pub async fn withdraw_service_exposure(
        &mut self,
        exposure_id: &str,
        network_id: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = WithdrawServiceExposureRequest {
            meta: Some(chv_nwd_api::chv_nwd_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            exposure_id: exposure_id.to_string(),
            network_id: network_id.to_string(),
        };
        let span = tracing::info_span!(
            "withdraw_service_exposure",
            operation_id = operation_id.unwrap_or("")
        );
        self.inner
            .withdraw_service_exposure(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    pub async fn list_namespace_state(
        &mut self,
    ) -> Result<chv_nwd_api::chv_nwd_api::ListNamespaceStateResponse, ChvError> {
        let span = tracing::info_span!("list_namespace_state");
        let resp = self
            .inner
            .list_namespace_state(ListNamespaceStateRequest {})
            .instrument(span)
            .await
            .map_err(|e| ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: e.to_string(),
            })?
            .into_inner();
        Ok(resp)
    }

    pub async fn delete_network_topology(
        &mut self,
        network_id: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = DeleteNetworkTopologyRequest {
            meta: Some(chv_nwd_api::chv_nwd_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            network_id: network_id.to_string(),
        };
        let span = tracing::info_span!(
            "delete_network_topology",
            operation_id = operation_id.unwrap_or("")
        );
        self.inner
            .delete_network_topology(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    pub async fn set_firewall_policy(
        &mut self,
        network_id: &str,
        policy_version: &str,
        policy_json: Vec<u8>,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = SetFirewallPolicyRequest {
            meta: Some(chv_nwd_api::chv_nwd_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            network_id: network_id.to_string(),
            policy: Some(chv_nwd_api::chv_nwd_api::FirewallPolicy {
                policy_version: policy_version.to_string(),
                policy_json,
            }),
        };
        let span = tracing::info_span!(
            "set_firewall_policy",
            operation_id = operation_id.unwrap_or("")
        );
        self.inner
            .set_firewall_policy(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    pub async fn set_nat_policy(
        &mut self,
        network_id: &str,
        policy_version: &str,
        policy_json: Vec<u8>,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = SetNatPolicyRequest {
            meta: Some(chv_nwd_api::chv_nwd_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            network_id: network_id.to_string(),
            policy: Some(chv_nwd_api::chv_nwd_api::NatPolicy {
                policy_version: policy_version.to_string(),
                policy_json,
            }),
        };
        let span = tracing::info_span!("set_nat_policy", operation_id = operation_id.unwrap_or(""));
        self.inner
            .set_nat_policy(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    pub async fn detach_vm_nic(
        &mut self,
        nic_id: &str,
        vm_id: &str,
        _network_id: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = DetachVmNicRequest {
            meta: Some(chv_nwd_api::chv_nwd_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            nic_id: nic_id.to_string(),
            vm_id: vm_id.to_string(),
            network_id: _network_id.to_string(),
            vm_mac: String::new(),
        };
        let span = tracing::info_span!("detach_vm_nic", operation_id = operation_id.unwrap_or(""));
        self.inner
            .detach_vm_nic(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    pub async fn get_network_health(
        &mut self,
        network_id: &str,
    ) -> Result<chv_nwd_api::chv_nwd_api::NetworkHealthResponse, ChvError> {
        let req = NetworkHealthRequest {
            network_id: network_id.to_string(),
        };
        let span = tracing::info_span!("get_network_health");
        let resp = self
            .inner
            .get_network_health(req)
            .instrument(span)
            .await
            .map_err(|e| ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: e.to_string(),
            })?
            .into_inner();
        Ok(resp)
    }

    pub async fn ensure_dhcp_scope(
        &mut self,
        network_id: &str,
        cidr: &str,
        range_start: &str,
        range_end: &str,
        dns_servers: Vec<String>,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = EnsureDhcpScopeRequest {
            meta: Some(chv_nwd_api::chv_nwd_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            scope: Some(DhcpScope {
                network_id: network_id.to_string(),
                cidr: cidr.to_string(),
                range_start: range_start.to_string(),
                range_end: range_end.to_string(),
                dns_servers,
            }),
        };
        let span = tracing::info_span!(
            "ensure_dhcp_scope",
            operation_id = operation_id.unwrap_or("")
        );
        self.inner
            .ensure_dhcp_scope(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    pub async fn ensure_dns_scope(
        &mut self,
        network_id: &str,
        forwarders: Vec<String>,
        static_records: std::collections::HashMap<String, String>,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = EnsureDnsScopeRequest {
            meta: Some(chv_nwd_api::chv_nwd_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            scope: Some(DnsScope {
                network_id: network_id.to_string(),
                forwarders,
                static_records,
            }),
        };
        let span = tracing::info_span!(
            "ensure_dns_scope",
            operation_id = operation_id.unwrap_or("")
        );
        self.inner
            .ensure_dns_scope(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    pub async fn update_overlay(
        &mut self,
        network_id: &str,
        vni: u32,
        vtep_endpoints: Vec<chv_nwd_api::chv_nwd_api::VtepEndpoint>,
        fdb_entries: Vec<chv_nwd_api::chv_nwd_api::FdbEntry>,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = UpdateOverlayRequest {
            network_id: network_id.to_string(),
            vni,
            vtep_endpoints,
            fdb_entries,
        };
        let span = tracing::info_span!("update_overlay", operation_id = operation_id.unwrap_or(""));
        let resp = self
            .inner
            .update_overlay(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: e.to_string(),
            })?
            .into_inner();
        if let Some(ref result) = resp.result {
            if !result.status.eq_ignore_ascii_case("ok") {
                return Err(ChvError::NetworkUnavailable {
                    resource: "nwd".to_string(),
                    reason: format!(
                        "update_overlay failed: {} ({})",
                        result.human_summary, result.error_code
                    ),
                });
            }
        }
        Ok(())
    }

    pub async fn send_gratuitous_arp(
        &mut self,
        network_id: &str,
        vm_ip: &str,
        bridge_name: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let req = SendGratuitousArpRequest {
            meta: Some(chv_nwd_api::chv_nwd_api::Meta {
                operation_id: operation_id.unwrap_or("").to_string(),
                request_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            }),
            network_id: network_id.to_string(),
            vm_ip: vm_ip.to_string(),
            bridge_name: bridge_name.to_string(),
        };
        let span = tracing::info_span!(
            "send_gratuitous_arp",
            operation_id = operation_id.unwrap_or("")
        );
        let resp = self
            .inner
            .send_gratuitous_arp(with_operation_id(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::NetworkUnavailable {
                resource: "nwd".to_string(),
                reason: e.to_string(),
            })?
            .into_inner();
        if let Some(ref result) = resp.result {
            if !result.status.eq_ignore_ascii_case("ok") {
                return Err(ChvError::NetworkUnavailable {
                    resource: "nwd".to_string(),
                    reason: format!(
                        "send_gratuitous_arp failed: {} ({})",
                        result.human_summary, result.error_code
                    ),
                });
            }
        }
        Ok(())
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
        ) -> Result<Response<chv_stord_api::chv_stord_api::ListVolumeSessionsResponse>, Status>
        {
            Ok(Response::new(
                chv_stord_api::chv_stord_api::ListVolumeSessionsResponse { sessions: vec![] },
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
        ) -> Result<Response<chv_stord_api::chv_stord_api::AttachVolumeToVmResponse>, Status>
        {
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
        async fn restore_snapshot(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::RestoreSnapshotRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn delete_snapshot(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::DeleteSnapshotRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn set_device_policy(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::SetDevicePolicyRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn trigger_disk_migration(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::TriggerDiskMigrationRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::TriggerDiskMigrationResponse>, Status>
        {
            Err(Status::unimplemented(""))
        }
        async fn get_disk_migration_status(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::GetDiskMigrationStatusRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::GetDiskMigrationStatusResponse>, Status>
        {
            Err(Status::unimplemented(""))
        }
        async fn resume_disk_migration(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::ResumeDiskMigrationRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::ResumeDiskMigrationResponse>, Status>
        {
            Err(Status::unimplemented(""))
        }
    }

    struct MockNwd;
    #[tonic::async_trait]
    impl NetworkService for MockNwd {
        async fn list_namespace_state(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::ListNamespaceStateRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::ListNamespaceStateResponse>, Status>
        {
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
            Ok(Response::new(chv_nwd_api::chv_nwd_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
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

        async fn update_overlay(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::UpdateOverlayRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::UpdateOverlayResponse>, Status> {
            Err(Status::unimplemented(""))
        }

        async fn send_gratuitous_arp(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::SendGratuitousArpRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::SendGratuitousArpResponse>, Status> {
            Err(Status::unimplemented(""))
        }

        async fn update_security_policy(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::SecurityPolicy>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::UpdateSecurityPolicyResponse>, Status>
        {
            Err(Status::unimplemented(""))
        }

        async fn update_rate_limit(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::RateLimitPolicy>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::UpdateRateLimitResponse>, Status> {
            Err(Status::unimplemented(""))
        }

        async fn get_overlay_status(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::GetOverlayStatusRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::OverlayStatus>, Status> {
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
                .add_service(
                    chv_stord_api::chv_stord_api::storage_service_server::StorageServiceServer::new(
                        MockStord,
                    ),
                )
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
                .add_service(
                    chv_nwd_api::chv_nwd_api::network_service_server::NetworkServiceServer::new(
                        MockNwd,
                    ),
                )
                .serve_with_incoming(tokio_stream::wrappers::UnixListenerStream::new(uds))
                .await
                .ok();
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut client = NwdClient::connect(&socket).await.unwrap();
        assert!(client.health_probe().await.unwrap());
    }

    #[tokio::test]
    async fn stord_resize_volume_propagates_backend_error() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("stord.sock");

        let uds = tokio::net::UnixListener::bind(&socket).unwrap();
        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(
                    chv_stord_api::chv_stord_api::storage_service_server::StorageServiceServer::new(
                        MockStord,
                    ),
                )
                .serve_with_incoming(tokio_stream::wrappers::UnixListenerStream::new(uds))
                .await
                .ok();
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut client = StordClient::connect(&socket).await.unwrap();
        let result = client.resize_volume("vol-1", 1024, Some("op-1")).await;
        assert!(matches!(result, Err(ChvError::BackendUnavailable { .. })));
    }

    #[tokio::test]
    async fn nwd_detach_vm_nic_rpc_ok() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("nwd.sock");

        let uds = tokio::net::UnixListener::bind(&socket).unwrap();
        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(
                    chv_nwd_api::chv_nwd_api::network_service_server::NetworkServiceServer::new(
                        MockNwd,
                    ),
                )
                .serve_with_incoming(tokio_stream::wrappers::UnixListenerStream::new(uds))
                .await
                .ok();
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut client = NwdClient::connect(&socket).await.unwrap();
        let result = client
            .detach_vm_nic("nic-1", "vm-1", "net-1", Some("op-1"))
            .await;
        assert!(result.is_ok());
    }
}
