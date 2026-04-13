use chv_common::types::DevicePolicy;
use chv_observability::Metrics;
use chv_stord_api::chv_stord_api::{
    storage_service_client::StorageServiceClient, BackendLocator, CloseVolumeRequest,
    ListVolumeSessionsRequest, OpenVolumeRequest, VolumeHealthRequest,
};
use chv_stord_backends::LocalFileBackend;
use chv_stord_core::StorageServer;
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

#[tokio::test]
async fn open_close_health_list_smoke() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("stord.sock");

    let backend = LocalFileBackend::new(dir.path().to_path_buf());
    let server = StorageServer::new(backend, Metrics::new());

    let socket_clone = socket.clone();
    tokio::spawn(async move {
        server.serve(&socket_clone).await.ok();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut client = make_client(socket).await;

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

    let open_resp = client.open_volume(open_req.clone()).await.unwrap().into_inner();
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
    assert_eq!(health_resp.health_status, "healthy");
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
