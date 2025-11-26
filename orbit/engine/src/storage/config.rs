//! Storage Configuration for Iceberg Cold Tier
//!
//! Provides unified configuration for multiple storage backends:
//! - S3 (MinIO, AWS S3)
//! - Azure Blob Storage (Azurite, Azure Storage)
//!
//! This module creates properly configured FileIO instances for Iceberg
//! based on the storage backend type and credentials.

#[cfg(feature = "iceberg-cold")]
use iceberg::io::{FileIO, FileIOBuilder};

use crate::error::{EngineError, EngineResult};

/// Storage backend type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageBackend {
    /// S3-compatible storage (MinIO, AWS S3)
    S3(S3Config),
    /// Azure Blob Storage
    Azure(AzureConfig),
}

/// S3 storage configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3Config {
    /// S3 endpoint URL (e.g., "http://localhost:9000" for MinIO)
    pub endpoint: String,

    /// AWS access key ID
    pub access_key_id: String,

    /// AWS secret access key
    pub secret_access_key: String,

    /// AWS region (e.g., "us-east-1")
    pub region: String,

    /// S3 bucket name
    pub bucket: String,

    /// Enable path-style access (required for MinIO)
    pub path_style_access: bool,
}

/// Azure Blob Storage configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AzureConfig {
    /// Azure storage account name
    pub account_name: String,

    /// Azure storage account key
    pub account_key: String,

    /// Azure Blob Storage endpoint (optional, for Azurite)
    /// Example: "http://127.0.0.1:10000/devstoreaccount1"
    pub endpoint: Option<String>,

    /// Azure container name
    pub container: String,
}

impl S3Config {
    /// Create a new S3 configuration
    pub fn new(
        endpoint: String,
        access_key_id: String,
        secret_access_key: String,
        region: String,
        bucket: String,
        path_style_access: bool,
    ) -> Self {
        Self {
            endpoint,
            access_key_id,
            secret_access_key,
            region,
            bucket,
            path_style_access,
        }
    }

    /// Create MinIO configuration with default settings
    pub fn minio(endpoint: String, access_key: String, secret_key: String, bucket: String) -> Self {
        Self {
            endpoint,
            access_key_id: access_key,
            secret_access_key: secret_key,
            region: "us-east-1".to_string(),
            bucket,
            path_style_access: true, // MinIO requires path-style
        }
    }

    /// Create AWS S3 configuration
    pub fn aws_s3(
        access_key_id: String,
        secret_access_key: String,
        region: String,
        bucket: String,
    ) -> Self {
        Self {
            endpoint: format!("https://s3.{}.amazonaws.com", region),
            access_key_id,
            secret_access_key,
            region,
            bucket,
            path_style_access: false,
        }
    }
}

impl AzureConfig {
    /// Create a new Azure Blob Storage configuration
    pub fn new(
        account_name: String,
        account_key: String,
        endpoint: Option<String>,
        container: String,
    ) -> Self {
        Self {
            account_name,
            account_key,
            endpoint,
            container,
        }
    }

    /// Create Azurite configuration with default development settings
    pub fn azurite(container: String) -> Self {
        Self {
            account_name: "devstoreaccount1".to_string(),
            account_key: "Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==".to_string(),
            endpoint: Some("http://127.0.0.1:10000/devstoreaccount1".to_string()),
            container,
        }
    }

    /// Create Azure Storage configuration (production)
    pub fn azure_storage(account_name: String, account_key: String, container: String) -> Self {
        Self {
            account_name,
            account_key,
            endpoint: None, // Use default Azure endpoints
            container,
        }
    }

    /// Parse from Azure connection string
    ///
    /// Example connection string:
    /// "AccountName=devstoreaccount1;AccountKey=...;DefaultEndpointsProtocol=http;BlobEndpoint=http://127.0.0.1:10000/devstoreaccount1;..."
    pub fn from_connection_string(
        connection_string: &str,
        container: String,
    ) -> EngineResult<Self> {
        let mut account_name = None;
        let mut account_key = None;
        let mut blob_endpoint = None;

        // Parse key=value pairs separated by semicolons
        for pair in connection_string.split(';') {
            let parts: Vec<&str> = pair.splitn(2, '=').collect();
            if parts.len() != 2 {
                continue;
            }

            match parts[0] {
                "AccountName" => account_name = Some(parts[1].to_string()),
                "AccountKey" => account_key = Some(parts[1].to_string()),
                "BlobEndpoint" => blob_endpoint = Some(parts[1].to_string()),
                _ => {} // Ignore other fields
            }
        }

        let account_name = account_name.ok_or_else(|| {
            EngineError::storage("Missing AccountName in connection string".to_string())
        })?;
        let account_key = account_key.ok_or_else(|| {
            EngineError::storage("Missing AccountKey in connection string".to_string())
        })?;

        Ok(Self {
            account_name,
            account_key,
            endpoint: blob_endpoint,
            container,
        })
    }
}

impl StorageBackend {
    /// Create a FileIO instance for this storage backend
    #[cfg(feature = "iceberg-cold")]
    pub fn create_file_io(&self) -> EngineResult<FileIO> {
        match self {
            StorageBackend::S3(config) => create_s3_file_io(config),
            StorageBackend::Azure(config) => create_azure_file_io(config),
        }
    }

    /// Get the warehouse path for this storage backend
    pub fn warehouse_path(&self) -> String {
        match self {
            StorageBackend::S3(config) => {
                format!("s3://{}/warehouse", config.bucket)
            }
            StorageBackend::Azure(config) => {
                format!("az://{}/warehouse", config.container)
            }
        }
    }
}

/// Create a FileIO for S3-compatible storage
#[cfg(feature = "iceberg-cold")]
fn create_s3_file_io(config: &S3Config) -> EngineResult<FileIO> {
    let mut builder = FileIOBuilder::new("s3");

    builder = builder
        .with_prop("s3.endpoint", &config.endpoint)
        .with_prop("s3.access_key_id", &config.access_key_id)
        .with_prop("s3.secret_access_key", &config.secret_access_key)
        .with_prop("s3.region", &config.region);

    if config.path_style_access {
        builder = builder.with_prop("s3.path_style_access", "true");
    }

    builder
        .build()
        .map_err(|e| EngineError::storage(format!("Failed to create S3 FileIO: {}", e)))
}

/// Create a FileIO for Azure Blob Storage
#[cfg(feature = "iceberg-cold")]
fn create_azure_file_io(config: &AzureConfig) -> EngineResult<FileIO> {
    let mut builder = FileIOBuilder::new("azblob");

    builder = builder
        .with_prop("azblob.account_name", &config.account_name)
        .with_prop("azblob.account_key", &config.account_key)
        .with_prop("azblob.container", &config.container);

    if let Some(endpoint) = &config.endpoint {
        builder = builder.with_prop("azblob.endpoint", endpoint);
    }

    builder
        .build()
        .map_err(|e| EngineError::storage(format!("Failed to create Azure FileIO: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s3_config_minio() {
        let config = S3Config::minio(
            "http://localhost:9000".to_string(),
            "minioadmin".to_string(),
            "minioadmin".to_string(),
            "orbitstore".to_string(),
        );

        assert_eq!(config.endpoint, "http://localhost:9000");
        assert_eq!(config.region, "us-east-1");
        assert!(config.path_style_access);
    }

    #[test]
    fn test_azure_config_azurite() {
        let config = AzureConfig::azurite("orbitstore".to_string());

        assert_eq!(config.account_name, "devstoreaccount1");
        assert_eq!(config.container, "orbitstore");
        assert!(config.endpoint.is_some());
    }

    #[test]
    fn test_azure_from_connection_string() {
        let conn_str = "AccountName=devstoreaccount1;AccountKey=Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==;DefaultEndpointsProtocol=http;BlobEndpoint=http://127.0.0.1:10000/devstoreaccount1;";

        let config = AzureConfig::from_connection_string(conn_str, "orbitstore".to_string())
            .expect("Failed to parse connection string");

        assert_eq!(config.account_name, "devstoreaccount1");
        assert_eq!(config.container, "orbitstore");
        assert!(config.endpoint.is_some());
        assert_eq!(
            config.endpoint.unwrap(),
            "http://127.0.0.1:10000/devstoreaccount1"
        );
    }

    #[test]
    fn test_storage_backend_warehouse_path() {
        let s3_backend = StorageBackend::S3(S3Config::minio(
            "http://localhost:9000".to_string(),
            "minioadmin".to_string(),
            "minioadmin".to_string(),
            "orbitstore".to_string(),
        ));

        assert_eq!(s3_backend.warehouse_path(), "s3://orbitstore/warehouse");

        let azure_backend = StorageBackend::Azure(AzureConfig::azurite("orbitstore".to_string()));
        assert_eq!(azure_backend.warehouse_path(), "az://orbitstore/warehouse");
    }
}
