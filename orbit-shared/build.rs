fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check if we should skip protobuf compilation
    if std::env::var("SKIP_PROTO_BUILD").is_ok() {
        println!("cargo:warning=Skipping protobuf compilation due to SKIP_PROTO_BUILD environment variable");
        return Ok(());
    }

    // Try to compile protobuf files
    match tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &["proto/transaction.proto", "proto/consensus.proto"],
            &["proto"],
        ) {
        Ok(()) => {
            println!("cargo:warning=Successfully compiled protobuf files");
            Ok(())
        }
        Err(e) => {
            eprintln!("cargo:warning=Failed to compile protobuf files: {}", e);
            eprintln!(
                "cargo:warning=This is likely due to missing protoc (Protocol Buffers compiler)"
            );
            eprintln!(
                "cargo:warning=To fix this, install protobuf-compiler or set SKIP_PROTO_BUILD=1"
            );
            eprintln!("cargo:warning=For CI/CD: apt-get install -y protobuf-compiler or brew install protobuf");

            // In CI environments, we might want to fail the build
            if std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok() {
                return Err(Box::new(e));
            }

            // For local development, continue without protobuf
            println!(
                "cargo:warning=Continuing build without protobuf compilation for local development"
            );
            Ok(())
        }
    }
}
