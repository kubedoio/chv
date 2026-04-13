fn main() {
    let proto_dir = std::path::PathBuf::from(std::env!("CARGO_MANIFEST_DIR"))
        .join("../../../proto");

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(
            &[proto_dir.join("controlplane/control-plane-node.proto")],
            &[proto_dir],
        )
        .expect("Failed to compile protos");
}
