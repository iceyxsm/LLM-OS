fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("failed to find vendored protoc");
    std::env::set_var("PROTOC", protoc);
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR");
    let proto_root = std::path::Path::new(&manifest_dir).join("../../contracts/proto");
    let proto_file = proto_root.join("controlplane/v1/llmos.proto");

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(&[proto_file], &[proto_root])
        .expect("failed to compile control plane proto");
}
