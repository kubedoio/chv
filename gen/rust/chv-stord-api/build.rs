fn main() {
    let proto_dir =
        std::path::PathBuf::from(std::env!("CARGO_MANIFEST_DIR")).join("../../../proto");

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[
                proto_dir.join("node/chv-stord-api.proto"),
                proto_dir.join("node/chv-stord-migration.proto"),
            ],
            std::slice::from_ref(&proto_dir),
        )
        .expect("Failed to compile protos");
}
