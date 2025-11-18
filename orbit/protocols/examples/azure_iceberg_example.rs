//! Example: Using Azure Blob Storage with Iceberg Cold Tier
//!
//! This example demonstrates how to configure and use Azure Blob Storage
//! (or Azurite emulator) as the storage backend for Iceberg cold tier.
//!
//! ## Prerequisites
//!
//! 1. Running Azurite emulator:
//!    ```bash
//!    docker run -p 10000:10000 -p 10001:10001 -p 10002:10002 \
//!      mcr.microsoft.com/azure-storage/azurite
//!    ```
//!
//! 2. Running Iceberg REST catalog:
//!    ```bash
//!    docker run -p 8181:8181 \
//!      -e AWS_REGION=us-east-1 \
//!      tabulario/iceberg-rest:latest
//!    ```
//!
//! ## Run the example
//!
//! ```bash
//! cargo run --example azure_iceberg_example --features iceberg-cold
//! ```

#![cfg(feature = "iceberg-cold")]

use std::sync::Arc;

use orbit_protocols::postgres_wire::sql::execution::{
    AzureConfig, StorageBackend, IcebergColdStore,
    create_file_io_for_storage, create_rest_catalog_with_storage,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debugging
    tracing_subscriber::fmt::init();

    println!("=== Azure Blob Storage + Iceberg Example ===\n");

    // Example 1: Using Azurite (local development)
    println!("Example 1: Azurite Configuration");
    example_azurite_config().await?;

    // Example 2: Using Azure Storage (production)
    println!("\nExample 2: Azure Storage Configuration");
    example_azure_storage_config()?;

    // Example 3: Using connection string
    println!("\nExample 3: Connection String Configuration");
    example_connection_string_config()?;

    // Example 4: Creating a cold store with Azure backend
    println!("\nExample 4: Creating Cold Store with Azure");
    println!("  (Requires REST catalog server running)");
    // Uncomment when you have a REST catalog running:
    // example_create_cold_store().await?;

    println!("\n=== Examples Complete ===");
    Ok(())
}

/// Example 1: Configure Azurite for local development
async fn example_azurite_config() -> Result<(), Box<dyn std::error::Error>> {
    // Create Azurite configuration with default development credentials
    let azure_config = AzureConfig::azurite("orbitstore".to_string());

    println!("  Account Name: {}", azure_config.account_name);
    println!("  Container: {}", azure_config.container);
    println!(
        "  Endpoint: {}",
        azure_config.endpoint.as_ref().unwrap_or(&"default".to_string())
    );

    // Create storage backend
    let storage_backend = StorageBackend::Azure(azure_config);
    println!("  Warehouse Path: {}", storage_backend.warehouse_path());

    // Create FileIO (this validates the configuration)
    let _file_io = storage_backend.create_file_io()?;
    println!("  ✓ FileIO created successfully");

    Ok(())
}

/// Example 2: Configure production Azure Storage
fn example_azure_storage_config() -> Result<(), Box<dyn std::error::Error>> {
    // In production, you would get these from environment variables or config
    let account_name = std::env::var("AZURE_STORAGE_ACCOUNT")
        .unwrap_or_else(|_| "mystorageaccount".to_string());
    let account_key = std::env::var("AZURE_STORAGE_KEY")
        .unwrap_or_else(|_| "your-account-key-here".to_string());

    let azure_config = AzureConfig::azure_storage(
        account_name.clone(),
        account_key,
        "orbitstore".to_string(),
    );

    println!("  Account Name: {}", account_name);
    println!("  Container: {}", azure_config.container);
    println!("  Using default Azure endpoints");

    // Create storage backend
    let storage_backend = StorageBackend::Azure(azure_config);
    println!("  Warehouse Path: {}", storage_backend.warehouse_path());

    Ok(())
}

/// Example 3: Configure using connection string
fn example_connection_string_config() -> Result<(), Box<dyn std::error::Error>> {
    // This is the Azurite default connection string
    let connection_string = "AccountName=devstoreaccount1;\
        AccountKey=Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==;\
        DefaultEndpointsProtocol=http;\
        BlobEndpoint=http://127.0.0.1:10000/devstoreaccount1;\
        QueueEndpoint=http://127.0.0.1:10001/devstoreaccount1;\
        TableEndpoint=http://127.0.0.1:10002/devstoreaccount1;";

    let azure_config = AzureConfig::from_connection_string(
        connection_string,
        "orbitstore".to_string(),
    )?;

    println!("  Account Name: {}", azure_config.account_name);
    println!("  Container: {}", azure_config.container);
    println!(
        "  Endpoint: {}",
        azure_config.endpoint.as_ref().unwrap_or(&"default".to_string())
    );

    // Create storage backend and FileIO
    let storage_backend = StorageBackend::Azure(azure_config);
    let _file_io = storage_backend.create_file_io()?;
    println!("  ✓ Configuration parsed and FileIO created");

    Ok(())
}

/// Example 4: Create an Iceberg cold store with Azure backend
#[allow(dead_code)]
async fn example_create_cold_store() -> Result<(), Box<dyn std::error::Error>> {
    // Configure Azure storage
    let azure_config = AzureConfig::azurite("orbitstore".to_string());
    let storage_backend = StorageBackend::Azure(azure_config);

    // Create REST catalog with Azure storage backend
    let catalog = create_rest_catalog_with_storage(
        "http://localhost:8181",  // REST catalog URI
        storage_backend,
    ).await?;

    println!("  ✓ Created REST catalog with Azure backend");

    // Create cold store instance
    let cold_store = IcebergColdStore::new(
        catalog,
        "default",      // namespace
        "user_events",  // table name
    ).await?;

    println!("  ✓ Cold store created successfully");
    println!("  Table: {}", cold_store.table_name());

    // Example: Scan the table
    let batches = cold_store.scan(None).await?;
    println!("  ✓ Scanned {} batches", batches.len());

    Ok(())
}

/// Example 5: Complete workflow with data
#[allow(dead_code)]
async fn example_complete_workflow() -> Result<(), Box<dyn std::error::Error>> {
    println!("Complete Workflow Example:");

    // 1. Configure storage using connection string
    let azure_config = AzureConfig::from_connection_string(
        "AccountName=devstoreaccount1;\
         AccountKey=Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==;\
         BlobEndpoint=http://127.0.0.1:10000/devstoreaccount1;",
        "orbitstore".to_string(),
    )?;

    // 2. Create storage backend
    let storage_backend = StorageBackend::Azure(azure_config);

    // 3. Create REST catalog with Azure backend
    let catalog = create_rest_catalog_with_storage(
        "http://localhost:8181",
        storage_backend,
    ).await?;

    println!("  ✓ Catalog configured with Azure Blob Storage");

    // 4. Create cold store
    let cold_store = IcebergColdStore::new(catalog, "default", "events").await?;
    println!("  ✓ Cold store ready: {}", cold_store.table_name());

    // 5. Query data
    let batches = cold_store.scan(None).await?;
    println!("  ✓ Retrieved {} batches from Azure", batches.len());

    // 6. Process with vectorized execution
    for (i, batch) in batches.iter().enumerate() {
        println!(
            "    Batch {}: {} rows × {} columns",
            i,
            batch.num_rows(),
            batch.num_columns()
        );
    }

    Ok(())
}
