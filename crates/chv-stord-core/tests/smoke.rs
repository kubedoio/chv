use chv_observability::Metrics;
use chv_stord_core::store::SessionStore;
use chv_stord_api::chv_stord_api::{
    storage_service_client::StorageServiceClient, AttachVolumeToVmRequest, BackendLocator,
    CloseVolumeRequest, DetachVolumeFromVmRequest, DevicePolicy, ListVolumeSessionsRequest,
    OpenVolumeRequest, PrepareCloneRequest, PrepareSnapshotRequest, ResizeVolumeRequest,
    SetDevicePolicyRequest, VolumeHealthRequest,
};
use chv_stord_backends::LocalFileBackend;
use chv_stord_core::StorageServer;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::UnixStream;
use tonic::transport::{Endpoint, Uri};
use tower::service_fn;

async fn make_client(socket: PathBuf) -> StorageServiceClient<tonic::transport::Channel> {
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
    StorageServiceClient::new(channel)
}

async fn setup_server() -> (
    tempfile::TempDir,
    PathBuf,
    StorageServiceClient<tonic::transport::Channel>,
) {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("stord.sock");

    let backend = LocalFileBackend::new(dir.path().to_path_buf());
    let server = StorageServer::new(backend, Metrics::new(), vec!["local".to_string()], None);

    let socket_clone = socket.clone();
    tokio::spawn(async move {
        server.serve(&socket_clone, None).await.ok();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let client = make_client(socket.clone()).await;
    (dir, socket, client)
}

#[tokio::test]
async fn open_close_health_list_smoke() {
    let (_dir, _socket, mut client) = setup_server().await;

    // OpenVolume
    let open_req = OpenVolumeRequest {
        meta: None,
        volume_id: "vol-1".to_string(),
        backend: Some(BackendLocator {
            backend_class: "local".to_string(),
            locator: "vol-1.img".to_string(),
            options: Default::default(),
        }),
        policy: None,
    };

    let open_resp = client
        .open_volume(open_req.clone())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(open_resp.volume_id, "vol-1");
    assert_eq!(open_resp.export_kind, "raw");
    let handle = open_resp.attachment_handle;

    // Idempotent open returns same handle
    let open_resp2 = client.open_volume(open_req).await.unwrap().into_inner();
    assert_eq!(open_resp2.attachment_handle, handle);

    // GetVolumeHealth
    let health_resp = client
        .get_volume_health(VolumeHealthRequest {
            volume_id: "vol-1".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(health_resp.volume_id, "vol-1");
    assert_eq!(health_resp.health_status, "unhealthy"); // file does not exist yet
    assert_eq!(health_resp.backend_state, "open");

    // ListVolumeSessions
    let list_resp = client
        .list_volume_sessions(ListVolumeSessionsRequest {})
        .await
        .unwrap()
        .into_inner();
    assert_eq!(list_resp.sessions.len(), 1);
    assert_eq!(list_resp.sessions[0].volume_id, "vol-1");
    assert_eq!(list_resp.sessions[0].attachment_handle, handle);

    // CloseVolume
    let close_resp = client
        .close_volume(CloseVolumeRequest {
            meta: None,
            volume_id: "vol-1".to_string(),
            attachment_handle: handle.clone(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(close_resp.status, "OK");

    // Idempotent close: second close succeeds
    let close_resp2 = client
        .close_volume(CloseVolumeRequest {
            meta: None,
            volume_id: "vol-1".to_string(),
            attachment_handle: handle,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(close_resp2.status, "OK");

    // Health after close = unknown/closed
    let health_resp2 = client
        .get_volume_health(VolumeHealthRequest {
            volume_id: "vol-1".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(health_resp2.health_status, "unknown");
    assert_eq!(health_resp2.backend_state, "closed");
}

#[tokio::test]
async fn attach_and_detach_volume_smoke() {
    let (_dir, _socket, mut client) = setup_server().await;

    // Open volume
    let open_resp = client
        .open_volume(OpenVolumeRequest {
            meta: None,
            volume_id: "vol-1".to_string(),
            backend: Some(BackendLocator {
                backend_class: "local".to_string(),
                locator: "vol-1.img".to_string(),
                options: Default::default(),
            }),
            policy: None,
        })
        .await
        .unwrap()
        .into_inner();
    let handle = open_resp.attachment_handle;

    // Attach volume to VM
    let attach_resp = client
        .attach_volume_to_vm(AttachVolumeToVmRequest {
            meta: None,
            volume_id: "vol-1".to_string(),
            vm_id: "vm-1".to_string(),
            attachment_handle: handle.clone(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(attach_resp.volume_id, "vol-1");
    assert_eq!(attach_resp.vm_id, "vm-1");
    assert_eq!(attach_resp.result.as_ref().unwrap().status, "OK");

    // List sessions to verify attachment state
    let list_resp = client
        .list_volume_sessions(ListVolumeSessionsRequest {})
        .await
        .unwrap()
        .into_inner();
    assert_eq!(list_resp.sessions.len(), 1);
    assert_eq!(list_resp.sessions[0].vm_id, "vm-1");
    assert_eq!(list_resp.sessions[0].runtime_status, "attached");

    // Detach volume from VM
    let detach_resp = client
        .detach_volume_from_vm(DetachVolumeFromVmRequest {
            meta: None,
            volume_id: "vol-1".to_string(),
            vm_id: "vm-1".to_string(),
            force: false,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(detach_resp.status, "OK");

    // Verify session is back to open
    let list_resp2 = client
        .list_volume_sessions(ListVolumeSessionsRequest {})
        .await
        .unwrap()
        .into_inner();
    assert_eq!(list_resp2.sessions[0].vm_id, "");
    assert_eq!(list_resp2.sessions[0].runtime_status, "open");

    // Idempotent detach: no session with vm_id should still return OK
    let detach_resp2 = client
        .detach_volume_from_vm(DetachVolumeFromVmRequest {
            meta: None,
            volume_id: "vol-1".to_string(),
            vm_id: "vm-1".to_string(),
            force: false,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(detach_resp2.status, "OK");

    // Close volume
    let _ = client
        .close_volume(CloseVolumeRequest {
            meta: None,
            volume_id: "vol-1".to_string(),
            attachment_handle: handle,
        })
        .await
        .unwrap()
        .into_inner();
}

#[tokio::test]
async fn attach_volume_missing_session_returns_not_found() {
    let (_dir, _socket, mut client) = setup_server().await;

    let attach_resp = client
        .attach_volume_to_vm(AttachVolumeToVmRequest {
            meta: None,
            volume_id: "vol-missing".to_string(),
            vm_id: "vm-1".to_string(),
            attachment_handle: "no-handle".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    let result = attach_resp.result.unwrap();
    assert_eq!(result.status, "error");
    assert_eq!(result.error_code, "NOT_FOUND");
}

#[tokio::test]
async fn allowlist_rejects_unknown_backend() {
    let (_dir, _socket, mut client) = setup_server().await;

    let resp = client
        .open_volume(OpenVolumeRequest {
            meta: None,
            volume_id: "vol-1".to_string(),
            backend: Some(BackendLocator {
                backend_class: "iscsi".to_string(),
                locator: "tgt".to_string(),
                options: Default::default(),
            }),
            policy: None,
        })
        .await
        .unwrap()
        .into_inner();

    let result = resp.result.unwrap();
    assert_eq!(result.status, "error");
    assert_eq!(result.error_code, "BACKEND_UNAVAILABLE");
}

#[tokio::test]
async fn resize_volume_smoke() {
    let (dir, _socket, mut client) = setup_server().await;

    let locator = "vol-resize.img".to_string();
    let path = dir.path().join(&locator);
    {
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(&[0u8; 512]).unwrap();
    }

    let open_resp = client
        .open_volume(OpenVolumeRequest {
            meta: None,
            volume_id: "vol-resize".to_string(),
            backend: Some(BackendLocator {
                backend_class: "local".to_string(),
                locator,
                options: Default::default(),
            }),
            policy: None,
        })
        .await
        .unwrap()
        .into_inner();
    let handle = open_resp.attachment_handle;

    let resize_resp = client
        .resize_volume(ResizeVolumeRequest {
            meta: None,
            volume_id: "vol-resize".to_string(),
            new_size_bytes: 1024,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(resize_resp.status, "OK");

    let meta = std::fs::metadata(&path).unwrap();
    assert_eq!(meta.len(), 1024);

    client
        .close_volume(CloseVolumeRequest {
            meta: None,
            volume_id: "vol-resize".to_string(),
            attachment_handle: handle,
        })
        .await
        .unwrap()
        .into_inner();
}

#[tokio::test]
async fn set_device_policy_smoke() {
    let (_dir, _socket, mut client) = setup_server().await;

    let open_resp = client
        .open_volume(OpenVolumeRequest {
            meta: None,
            volume_id: "vol-policy".to_string(),
            backend: Some(BackendLocator {
                backend_class: "local".to_string(),
                locator: "vol-policy.img".to_string(),
                options: Default::default(),
            }),
            policy: None,
        })
        .await
        .unwrap()
        .into_inner();
    let handle = open_resp.attachment_handle;

    let policy_resp = client
        .set_device_policy(SetDevicePolicyRequest {
            meta: None,
            volume_id: "vol-policy".to_string(),
            policy: Some(DevicePolicy {
                read_bps: 1000,
                write_bps: 2000,
                read_iops: 100,
                write_iops: 100,
                burst_allowed: false,
            }),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(policy_resp.status, "OK");

    client
        .close_volume(CloseVolumeRequest {
            meta: None,
            volume_id: "vol-policy".to_string(),
            attachment_handle: handle,
        })
        .await
        .unwrap()
        .into_inner();
}

#[tokio::test]
async fn set_device_policy_missing_session_returns_not_found() {
    let (_dir, _socket, mut client) = setup_server().await;

    let policy_resp = client
        .set_device_policy(SetDevicePolicyRequest {
            meta: None,
            volume_id: "vol-missing".to_string(),
            policy: Some(DevicePolicy {
                read_bps: 1000,
                write_bps: 2000,
                read_iops: 100,
                write_iops: 100,
                burst_allowed: false,
            }),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(policy_resp.status, "error");
    assert_eq!(policy_resp.error_code, "NOT_FOUND");
}

#[tokio::test]
async fn resize_volume_missing_session_returns_not_found() {
    let (_dir, _socket, mut client) = setup_server().await;

    let resize_resp = client
        .resize_volume(ResizeVolumeRequest {
            meta: None,
            volume_id: "vol-missing".to_string(),
            new_size_bytes: 1024,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(resize_resp.status, "error");
    assert_eq!(resize_resp.error_code, "NOT_FOUND");
}

#[tokio::test]
async fn prepare_snapshot_smoke() {
    let (dir, _socket, mut client) = setup_server().await;

    let locator = "vol-snap.img".to_string();
    let path = dir.path().join(&locator);
    {
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(&[0u8; 512]).unwrap();
    }

    let open_resp = client
        .open_volume(OpenVolumeRequest {
            meta: None,
            volume_id: "vol-snap".to_string(),
            backend: Some(BackendLocator {
                backend_class: "local".to_string(),
                locator,
                options: Default::default(),
            }),
            policy: None,
        })
        .await
        .unwrap()
        .into_inner();
    let handle = open_resp.attachment_handle;

    let resp = client
        .prepare_snapshot(PrepareSnapshotRequest {
            meta: None,
            volume_id: "vol-snap".to_string(),
            snapshot_name: "snap1".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(resp.status, "OK");

    let snap_path = dir.path().join("vol-snap-snap1.img");
    assert!(snap_path.exists());

    client
        .close_volume(CloseVolumeRequest {
            meta: None,
            volume_id: "vol-snap".to_string(),
            attachment_handle: handle,
        })
        .await
        .unwrap()
        .into_inner();
}

#[tokio::test]
async fn prepare_clone_smoke() {
    let (dir, _socket, mut client) = setup_server().await;

    let locator = "vol-clone.img".to_string();
    let path = dir.path().join(&locator);
    {
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(&[0u8; 512]).unwrap();
    }

    let open_resp = client
        .open_volume(OpenVolumeRequest {
            meta: None,
            volume_id: "vol-clone".to_string(),
            backend: Some(BackendLocator {
                backend_class: "local".to_string(),
                locator,
                options: Default::default(),
            }),
            policy: None,
        })
        .await
        .unwrap()
        .into_inner();
    let handle = open_resp.attachment_handle;

    let resp = client
        .prepare_clone(PrepareCloneRequest {
            meta: None,
            volume_id: "vol-clone".to_string(),
            clone_name: "clone1".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(resp.status, "OK");

    let clone_path = dir.path().join("vol-clone-clone1.img");
    assert!(clone_path.exists());

    client
        .close_volume(CloseVolumeRequest {
            meta: None,
            volume_id: "vol-clone".to_string(),
            attachment_handle: handle,
        })
        .await
        .unwrap()
        .into_inner();
}

#[tokio::test]
async fn prepare_snapshot_missing_session_returns_not_found() {
    let (_dir, _socket, mut client) = setup_server().await;

    let resp = client
        .prepare_snapshot(PrepareSnapshotRequest {
            meta: None,
            volume_id: "vol-missing".to_string(),
            snapshot_name: "snap1".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(resp.status, "error");
    assert_eq!(resp.error_code, "NOT_FOUND");
}

#[tokio::test]
async fn prepare_clone_missing_session_returns_not_found() {
    let (_dir, _socket, mut client) = setup_server().await;

    let resp = client
        .prepare_clone(PrepareCloneRequest {
            meta: None,
            volume_id: "vol-missing".to_string(),
            clone_name: "clone1".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(resp.status, "error");
    assert_eq!(resp.error_code, "NOT_FOUND");
}

#[tokio::test]
async fn sqlite_persistence_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("stord-persist.sock");
    let db_path = dir.path().join("stord.db");

    let backend = LocalFileBackend::new(dir.path().to_path_buf());
    let store = SessionStore::new(&db_path).unwrap();
    let server = StorageServer::new(backend, Metrics::new(), vec!["local".to_string()], Some(store));
    let socket_clone = socket.clone();
    let db_path_clone = db_path.clone();
    tokio::spawn(async move {
        server.serve(&socket_clone, Some(&db_path_clone)).await.ok();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut client = make_client(socket).await;

    // Open volume
    let open_resp = client
        .open_volume(OpenVolumeRequest {
            meta: None,
            volume_id: "vol-persist".to_string(),
            backend: Some(BackendLocator {
                backend_class: "local".to_string(),
                locator: "vol-persist.img".to_string(),
                options: Default::default(),
            }),
            policy: None,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(open_resp.volume_id, "vol-persist");

    // Verify persisted to SQLite by opening a new store connection
    let store2 = SessionStore::new(&db_path).unwrap();
    let sessions = store2.list().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].volume_id, "vol-persist");

    // Close volume
    client
        .close_volume(CloseVolumeRequest {
            meta: None,
            volume_id: "vol-persist".to_string(),
            attachment_handle: open_resp.attachment_handle,
        })
        .await
        .unwrap()
        .into_inner();

    // Verify removed from SQLite
    let store3 = SessionStore::new(&db_path).unwrap();
    let sessions = store3.list().unwrap();
    assert!(sessions.is_empty());
}
