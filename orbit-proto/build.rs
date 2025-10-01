use std::io::Result;

fn main() -> Result<()> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
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