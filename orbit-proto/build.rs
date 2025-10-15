use std::io::Result;

fn main() -> Result<()> {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let descriptor_path = std::path::Path::new(&out_dir).join("proto_descriptor.bin");

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(&descriptor_path)
        .compile_protos(
            &[
                "proto/messages.proto",
                "proto/node.proto",
                "proto/addressable.proto",
                "proto/connection.proto",
                "proto/health.proto",
            ],
            &["proto"],
        )?;
    Ok(())
}
