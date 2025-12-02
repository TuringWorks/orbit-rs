//! S3-compatible storage backend (supports AWS S3, MinIO, etc.)
//!
//! This module provides an S3-compatible storage backend that can be used
//! for cold tier storage in the tiered storage system.

use super::storage::{
    UnifiedStorageBackend, UnifiedStorageError, UnifiedStorageMetrics, UnifiedStorageResult,
};
use async_trait::async_trait;
use opendal::services::S3;
use opendal::Operator;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Configuration for S3-compatible storage
#[derive(Debug, Clone)]
pub struct S3BackendConfig {
    /// S3 endpoint URL (e.g., "http://localhost:9000" for MinIO)
    pub endpoint: String,
    /// Access key ID
    pub access_key_id: String,
    /// Secret access key
    pub secret_access_key: String,
    /// Region
    pub region: String,
    /// Bucket name
    pub bucket: String,
    /// Path prefix for all keys
    pub prefix: String,
    /// Enable path-style access (required for MinIO)
    pub path_style_access: bool,
}

impl Default for S3BackendConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:9000".to_string(),
            access_key_id: "minioadmin".to_string(),
            secret_access_key: "minioadmin".to_string(),
            region: "us-east-1".to_string(),
            bucket: "orbit-cold-storage".to_string(),
            prefix: "data/".to_string(),
            path_style_access: true,
        }
    }
}

impl S3BackendConfig {
    /// Create a MinIO configuration
    pub fn minio(endpoint: &str, access_key: &str, secret_key: &str, bucket: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            access_key_id: access_key.to_string(),
            secret_access_key: secret_key.to_string(),
            region: "us-east-1".to_string(),
            bucket: bucket.to_string(),
            prefix: "data/".to_string(),
            path_style_access: true,
        }
    }

    /// Create an AWS S3 configuration
    pub fn aws_s3(access_key: &str, secret_key: &str, region: &str, bucket: &str) -> Self {
        Self {
            endpoint: format!("https://s3.{}.amazonaws.com", region),
            access_key_id: access_key.to_string(),
            secret_access_key: secret_key.to_string(),
            region: region.to_string(),
            bucket: bucket.to_string(),
            prefix: "data/".to_string(),
            path_style_access: false,
        }
    }
}

/// S3-compatible storage backend
pub struct S3Backend {
    /// Configuration
    config: S3BackendConfig,
    /// OpenDAL operator (initialized lazily)
    operator: Arc<RwLock<Option<Operator>>>,
    /// Read operations counter
    read_ops: AtomicU64,
    /// Write operations counter
    write_ops: AtomicU64,
    /// Delete operations counter
    delete_ops: AtomicU64,
    /// Error counter
    error_count: AtomicU64,
}

impl S3Backend {
    /// Create a new S3 backend with the given configuration
    pub fn new(config: S3BackendConfig) -> Self {
        Self {
            config,
            operator: Arc::new(RwLock::new(None)),
            read_ops: AtomicU64::new(0),
            write_ops: AtomicU64::new(0),
            delete_ops: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
        }
    }

    /// Create a MinIO backend for testing
    pub fn minio(endpoint: &str, access_key: &str, secret_key: &str, bucket: &str) -> Self {
        Self::new(S3BackendConfig::minio(
            endpoint, access_key, secret_key, bucket,
        ))
    }

    /// Create with default MinIO settings (localhost:9000, minioadmin/minioadmin)
    pub fn minio_default(bucket: &str) -> Self {
        Self::minio("http://localhost:9000", "minioadmin", "minioadmin", bucket)
    }

    /// Get the full key path with prefix
    fn full_key(&self, key: &str) -> String {
        format!("{}{}", self.config.prefix, key)
    }

    /// Get the operator, initializing if necessary
    async fn get_operator(&self) -> UnifiedStorageResult<Operator> {
        let guard = self.operator.read().await;
        if let Some(ref op) = *guard {
            return Ok(op.clone());
        }
        drop(guard);

        // Initialize operator
        let mut guard = self.operator.write().await;
        if let Some(ref op) = *guard {
            return Ok(op.clone());
        }

        let mut builder = S3::default()
            .endpoint(&self.config.endpoint)
            .access_key_id(&self.config.access_key_id)
            .secret_access_key(&self.config.secret_access_key)
            .region(&self.config.region)
            .bucket(&self.config.bucket);

        // Note: OpenDAL uses virtual host style by default for S3
        // For MinIO and path-style access, we need to disable virtual host style
        if !self.config.path_style_access {
            builder = builder.enable_virtual_host_style();
        }

        let op = Operator::new(builder)
            .map_err(|e| {
                UnifiedStorageError::Backend(format!("Failed to create S3 operator: {}", e))
            })?
            .finish();

        *guard = Some(op.clone());
        Ok(op)
    }
}

#[async_trait]
impl UnifiedStorageBackend for S3Backend {
    async fn initialize(&self) -> UnifiedStorageResult<()> {
        info!(
            "[S3Backend] Initializing S3 backend: endpoint={}, bucket={}",
            self.config.endpoint, self.config.bucket
        );

        // Try to get the operator to verify connection
        let op = self.get_operator().await?;

        // Try a simple operation to verify bucket access
        // We'll try to check if the prefix exists
        match op.exists(&self.config.prefix).await {
            Ok(_) => {
                info!(
                    "[S3Backend] Successfully connected to S3: bucket={}",
                    self.config.bucket
                );
            }
            Err(e) => {
                // Check if this is a "bucket not found" error
                let error_str = e.to_string();
                if error_str.contains("NoSuchBucket") || error_str.contains("404") {
                    warn!(
                        "[S3Backend] Bucket {} does not exist, attempting to create it",
                        self.config.bucket
                    );

                    // Try to create the bucket by writing a marker file
                    // OpenDAL doesn't have direct bucket creation, but writing a file
                    // to a non-existent bucket in MinIO will auto-create it if configured
                    match op.write("_bucket_marker", Vec::<u8>::new()).await {
                        Ok(_) => {
                            info!(
                                "[S3Backend] Bucket {} created successfully",
                                self.config.bucket
                            );
                            // Clean up marker file
                            let _ = op.delete("_bucket_marker").await;
                        }
                        Err(create_err) => {
                            error!(
                                "[S3Backend] Failed to create bucket {}: {}",
                                self.config.bucket, create_err
                            );
                            return Err(UnifiedStorageError::Backend(format!(
                                "Bucket {} does not exist and could not be created. \
                                 Please create it manually using MinIO console or mc tool: \
                                 mc mb local/{}",
                                self.config.bucket, self.config.bucket
                            )));
                        }
                    }
                } else {
                    warn!(
                        "[S3Backend] Could not verify bucket access (this may be OK): {}",
                        e
                    );
                }
            }
        }

        Ok(())
    }

    async fn shutdown(&self) -> UnifiedStorageResult<()> {
        info!("[S3Backend] Shutting down S3 backend");
        let mut guard = self.operator.write().await;
        *guard = None;
        Ok(())
    }

    async fn get(&self, key: &str) -> UnifiedStorageResult<Option<Vec<u8>>> {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
        let op = self.get_operator().await?;
        let full_key = self.full_key(key);

        match op.read(&full_key).await {
            Ok(data) => {
                debug!("[S3Backend] GET {}: {} bytes", key, data.len());
                Ok(Some(data.to_vec()))
            }
            Err(e) if e.kind() == opendal::ErrorKind::NotFound => {
                debug!("[S3Backend] GET {}: not found", key);
                Ok(None)
            }
            Err(e) => {
                self.error_count.fetch_add(1, Ordering::Relaxed);
                error!("[S3Backend] GET {} failed: {}", key, e);
                Err(UnifiedStorageError::Backend(format!(
                    "S3 get failed: {}",
                    e
                )))
            }
        }
    }

    async fn put(&self, key: &str, value: &[u8]) -> UnifiedStorageResult<()> {
        self.write_ops.fetch_add(1, Ordering::Relaxed);
        let op = self.get_operator().await?;
        let full_key = self.full_key(key);

        op.write(&full_key, value.to_vec()).await.map_err(|e| {
            self.error_count.fetch_add(1, Ordering::Relaxed);
            error!("[S3Backend] PUT {} failed: {}", key, e);
            UnifiedStorageError::Backend(format!("S3 put failed: {}", e))
        })?;

        debug!("[S3Backend] PUT {}: {} bytes", key, value.len());
        Ok(())
    }

    async fn delete(&self, key: &str) -> UnifiedStorageResult<bool> {
        self.delete_ops.fetch_add(1, Ordering::Relaxed);
        let op = self.get_operator().await?;
        let full_key = self.full_key(key);

        // Check if exists first
        let key_exists = op
            .exists(&full_key)
            .await
            .map_err(|e| UnifiedStorageError::Backend(format!("S3 exists check failed: {}", e)))?;

        if !key_exists {
            debug!("[S3Backend] DELETE {}: not found", key);
            return Ok(false);
        }

        op.delete(&full_key).await.map_err(|e| {
            self.error_count.fetch_add(1, Ordering::Relaxed);
            error!("[S3Backend] DELETE {} failed: {}", key, e);
            UnifiedStorageError::Backend(format!("S3 delete failed: {}", e))
        })?;

        debug!("[S3Backend] DELETE {}: success", key);
        Ok(true)
    }

    async fn exists(&self, key: &str) -> UnifiedStorageResult<bool> {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
        let op = self.get_operator().await?;
        let full_key = self.full_key(key);

        let key_exists = op
            .exists(&full_key)
            .await
            .map_err(|e| UnifiedStorageError::Backend(format!("S3 exists check failed: {}", e)))?;

        debug!("[S3Backend] EXISTS {}: {}", key, key_exists);
        Ok(key_exists)
    }

    async fn scan_prefix(
        &self,
        prefix: &str,
        limit: Option<usize>,
    ) -> UnifiedStorageResult<Vec<(String, Vec<u8>)>> {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
        let op = self.get_operator().await?;
        let full_prefix = self.full_key(prefix);

        let mut results = Vec::new();
        let max_results = limit.unwrap_or(10000);

        // List objects with prefix
        let entries = op
            .list(&full_prefix)
            .await
            .map_err(|e| UnifiedStorageError::Backend(format!("S3 list failed: {}", e)))?;

        for entry in entries {
            if results.len() >= max_results {
                break;
            }

            let path = entry.path();
            // Skip directories
            if path.ends_with('/') {
                continue;
            }

            // Get the value
            match op.read(path).await {
                Ok(data) => {
                    // Remove the prefix from the key
                    let key = path
                        .strip_prefix(&self.config.prefix)
                        .unwrap_or(path)
                        .to_string();
                    results.push((key, data.to_vec()));
                }
                Err(e) => {
                    warn!("[S3Backend] Failed to read {} during scan: {}", path, e);
                }
            }
        }

        debug!(
            "[S3Backend] SCAN prefix={}: {} results",
            prefix,
            results.len()
        );
        Ok(results)
    }

    async fn put_batch(&self, entries: Vec<(String, Vec<u8>)>) -> UnifiedStorageResult<()> {
        for (key, value) in entries {
            self.put(&key, &value).await?;
        }
        Ok(())
    }

    async fn delete_batch(&self, keys: Vec<String>) -> UnifiedStorageResult<u64> {
        let mut count = 0u64;
        for key in keys {
            if self.delete(&key).await? {
                count += 1;
            }
        }
        Ok(count)
    }

    async fn metrics(&self) -> UnifiedStorageMetrics {
        UnifiedStorageMetrics {
            read_operations: self.read_ops.load(Ordering::Relaxed),
            write_operations: self.write_ops.load(Ordering::Relaxed),
            delete_operations: self.delete_ops.load(Ordering::Relaxed),
            error_count: self.error_count.load(Ordering::Relaxed),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that requires MinIO running at localhost:9000
    /// Run with: docker run -p 9000:9000 -p 9001:9001 minio/minio server /data --console-address ":9001"
    /// Create bucket: docker exec -it <container> mc mb /data/orbit-cold-storage
    #[tokio::test]
    #[ignore = "Requires MinIO running at localhost:9000"]
    async fn test_s3_backend_with_minio() {
        let backend = S3Backend::minio_default("orbit-cold-storage");
        backend.initialize().await.unwrap();

        // Test put
        backend.put("test-key", b"test-value").await.unwrap();

        // Test get
        let value = backend.get("test-key").await.unwrap();
        assert_eq!(value, Some(b"test-value".to_vec()));

        // Test exists
        assert!(backend.exists("test-key").await.unwrap());

        // Test delete
        assert!(backend.delete("test-key").await.unwrap());
        assert!(!backend.exists("test-key").await.unwrap());

        backend.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "Requires MinIO running at localhost:9000"]
    async fn test_s3_backend_scan_prefix() {
        let backend = S3Backend::minio_default("orbit-cold-storage");
        backend.initialize().await.unwrap();

        // Put some test data
        backend.put("scan-test:1", b"value1").await.unwrap();
        backend.put("scan-test:2", b"value2").await.unwrap();
        backend.put("scan-test:3", b"value3").await.unwrap();
        backend.put("other:1", b"other").await.unwrap();

        // Scan with prefix
        let results = backend.scan_prefix("scan-test:", None).await.unwrap();
        assert_eq!(results.len(), 3);

        // Cleanup
        backend.delete("scan-test:1").await.unwrap();
        backend.delete("scan-test:2").await.unwrap();
        backend.delete("scan-test:3").await.unwrap();
        backend.delete("other:1").await.unwrap();

        backend.shutdown().await.unwrap();
    }
}
