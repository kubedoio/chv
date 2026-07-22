use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[tokio::test]
async fn core_auth_convergence_001() {
    let dir = TempDir::new().unwrap();
    let runtime_dir = dir.path().join("runtime");
    let stord_socket = runtime_dir.join("stord.sock");
    let nwd_socket = runtime_dir.join("nwd.sock");
    let cache_path = runtime_dir.join("node-cache.json");
    let core_store = runtime_dir.join("core.db");
    let core_archive = runtime_dir.join("node-cache.archive");
    let chv_binary = "/bin/true";
    let grpc_socket = runtime_dir.join("agent.sock");
    let http_socket = runtime_dir.join("core.sock");

    std::fs::create_dir_all(&runtime_dir).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&runtime_dir, std::fs::Permissions::from_mode(0o700)).unwrap();

    let config = format!(
        "authority_mode = \"core-managed\"
node_id = \"test-node\"
runtime_dir = \"{}\"
stord_socket = \"{}\"
nwd_socket = \"{}\"
cache_path = \"{}\"
core_store_path = \"{}\"
core_archive_path = \"{}\"
chv_binary_path = \"{}\"
stord_binary_path = \"/bin/true\"
nwd_binary_path = \"/bin/true\"
socket_path = \"{}\"
core_api_socket_path = \"{}\"
storage_base_dir = \"{}\"
tls_cert_path = \"{}\"
tls_key_path = \"{}\"
ca_cert_path = \"{}\"
log_level = \"info\"
control_plane_addr = \"https://localhost:8443\"
console_bind = \"127.0.0.1:0\"
jwt_secret = \"secret\"
",
        runtime_dir.to_str().unwrap(),
        stord_socket.to_str().unwrap(),
        nwd_socket.to_str().unwrap(),
        cache_path.to_str().unwrap(),
        core_store.to_str().unwrap(),
        core_archive.to_str().unwrap(),
        chv_binary,
        grpc_socket.to_str().unwrap(),
        http_socket.to_str().unwrap(),
        runtime_dir.join("storage").to_str().unwrap(),
        dir.path().join("cert.pem").to_str().unwrap(),
        dir.path().join("key.pem").to_str().unwrap(),
        dir.path().join("ca.pem").to_str().unwrap(),
    );
    let config_path = dir.path().join("config.toml");
    std::fs::write(&config_path, config).unwrap();

    let exe = env!("CARGO_BIN_EXE_chv-agent");
    let mut agent = Command::new(exe)
        .arg(config_path)
        .env("CHV_ALLOW_INSECURE", "1")
        .env("RUST_LOG", "debug")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut ok = false;
    for _ in 0..100 {
        if grpc_socket.exists() && http_socket.exists() {
            ok = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    if !ok {
        let mut stdout = String::new();
        let mut stderr = String::new();
        agent.kill().unwrap();
        agent.wait().unwrap();
        agent
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut stdout)
            .unwrap();
        agent
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .unwrap();
        panic!("sockets not found. stdout: {}\nstderr: {}", stdout, stderr);
    }

    let mut http_client = UnixStream::connect(&http_socket).await.unwrap();
    http_client
        .write_all(b"GET /v1/host HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut response = String::new();
    http_client.read_to_string(&mut response).await.unwrap();
    assert!(response.contains("test-node"));

    agent.kill().unwrap();
    agent.wait().unwrap();
}
