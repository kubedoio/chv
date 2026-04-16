fn main() {
    let proto_dir =
        std::path::PathBuf::from(std::env!("CARGO_MANIFEST_DIR")).join("../../../proto");

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[
                proto_dir.join("webui/webui-bff.proto"),
                proto_dir.join("webui/webui-tasks.proto"),
                proto_dir.join("webui/webui-viewmodels.proto"),
            ],
            &[proto_dir],
        )
        .expect("Failed to compile protos");
}
