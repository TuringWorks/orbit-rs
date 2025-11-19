fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile protobuf files for Raft consensus gRPC services
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/consensus.proto"], &["proto"])?;

    Ok(())
}
