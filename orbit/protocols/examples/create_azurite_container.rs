//! Utility to create the Azurite container for testing
//!
//! Run this before running the integration tests:
//! ```bash
//! cargo run --example create_azurite_container --features iceberg-cold
//! ```

#[cfg(feature = "iceberg-cold")]
use opendal::Operator;

#[cfg(feature = "iceberg-cold")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Creating Azurite container 'orbitstore'...");

    let builder = opendal::services::Azblob::default()
        .account_name("devstoreaccount1")
        .account_key("Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==")
        .endpoint("http://127.0.0.1:10000/devstoreaccount1")
        .container("orbitstore");

    let op = Operator::new(builder)?.finish();

    // Try to write a marker file to trigger container creation
    println!("Attempting to write marker file...");

    match op.write(".container_marker", "created").await {
        Ok(_) => {
            println!("✓ Container 'orbitstore' created successfully!");
            println!("You can now run the integration tests:");
            println!("  cargo test --test iceberg_azurite_integration --features iceberg-cold -- --ignored");
            Ok(())
        }
        Err(e) => {
            if e.to_string().contains("ContainerNotFound") {
                eprintln!("\n✗ Container creation failed.");
                eprintln!("\nPlease create the container manually using Azure Storage Explorer:");
                eprintln!("1. Download: https://azure.microsoft.com/products/storage/storage-explorer/");
                eprintln!("2. Connect to: Local & Attached > Storage Accounts > (Emulator - Default Ports)");
                eprintln!("3. Right-click 'Blob Containers' and create 'orbitstore'");
                eprintln!("\nOr use Azure CLI:");
                eprintln!("  az storage container create --name orbitstore \\");
                eprintln!("    --connection-string \"DefaultEndpointsProtocol=http;AccountName=devstoreaccount1;AccountKey=Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==;BlobEndpoint=http://127.0.0.1:10000/devstoreaccount1;\"");
                Err(e.into())
            } else {
                Err(e.into())
            }
        }
    }
}

#[cfg(not(feature = "iceberg-cold"))]
fn main() {
    eprintln!("This example requires the 'iceberg-cold' feature to be enabled.");
    eprintln!("Run with: cargo run --example create_azurite_container --features iceberg-cold");
    std::process::exit(1);
}
