//! Strategy pattern for runtime polymorphism
//!
//! Allows selecting algorithm behavior at runtime while maintaining type safety.

use crate::error::{OrbitError, OrbitResult};
use async_trait::async_trait;
use std::sync::Arc;

// ===== Retry Strategy =====

/// Trait for retry strategies
#[async_trait]
pub trait RetryStrategy: Send + Sync {
    /// Calculate delay before next retry
    fn calculate_delay(&self, attempt: u32) -> std::time::Duration;

    /// Check if should retry
    fn should_retry(&self, attempt: u32, error: &OrbitError) -> bool;

    /// Maximum number of retries
    fn max_retries(&self) -> u32;
}

/// Exponential backoff strategy
#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub max_retries: u32,
    pub multiplier: f64,
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self {
            base_delay_ms: 100,
            max_delay_ms: 30000,
            max_retries: 3,
            multiplier: 2.0,
        }
    }
}

#[async_trait]
impl RetryStrategy for ExponentialBackoff {
    fn calculate_delay(&self, attempt: u32) -> std::time::Duration {
        let delay_ms = (self.base_delay_ms as f64 * self.multiplier.powi(attempt as i32)) as u64;
        std::time::Duration::from_millis(delay_ms.min(self.max_delay_ms))
    }

    fn should_retry(&self, attempt: u32, error: &OrbitError) -> bool {
        attempt < self.max_retries && matches!(error, OrbitError::NetworkError(_) | OrbitError::Timeout { .. })
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

/// Fixed delay strategy
#[derive(Debug, Clone)]
pub struct FixedDelay {
    pub delay_ms: u64,
    pub max_retries: u32,
}

#[async_trait]
impl RetryStrategy for FixedDelay {
    fn calculate_delay(&self, _attempt: u32) -> std::time::Duration {
        std::time::Duration::from_millis(self.delay_ms)
    }

    fn should_retry(&self, attempt: u32, _error: &OrbitError) -> bool {
        attempt < self.max_retries
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

/// No retry strategy
pub struct NoRetry;

#[async_trait]
impl RetryStrategy for NoRetry {
    fn calculate_delay(&self, _attempt: u32) -> std::time::Duration {
        std::time::Duration::from_secs(0)
    }

    fn should_retry(&self, _attempt: u32, _error: &OrbitError) -> bool {
        false
    }

    fn max_retries(&self) -> u32 {
        0
    }
}

/// Execute operation with retry strategy
pub async fn with_retry<F, T, Fut>(
    strategy: Arc<dyn RetryStrategy>,
    mut operation: F,
) -> OrbitResult<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = OrbitResult<T>>,
{
    let mut attempt = 0;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(error) => {
                if !strategy.should_retry(attempt, &error) {
                    return Err(error);
                }

                let delay = strategy.calculate_delay(attempt);
                tracing::warn!(
                    attempt = attempt,
                    delay_ms = delay.as_millis(),
                    error = %error,
                    "Retrying operation"
                );

                tokio::time::sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

// ===== Serialization Strategy =====

/// Trait for serialization strategies
pub trait SerializationStrategy: Send + Sync {
    /// Serialize data to bytes
    fn serialize<T: serde::Serialize>(&self, data: &T) -> OrbitResult<Vec<u8>>;

    /// Deserialize bytes to data
    fn deserialize<T: serde::de::DeserializeOwned>(&self, bytes: &[u8]) -> OrbitResult<T>;

    /// Get content type
    fn content_type(&self) -> &str;
}

/// JSON serialization
pub struct JsonSerialization;

impl SerializationStrategy for JsonSerialization {
    fn serialize<T: serde::Serialize>(&self, data: &T) -> OrbitResult<Vec<u8>> {
        serde_json::to_vec(data).map_err(|e| OrbitError::internal(format!("JSON serialization failed: {}", e)))
    }

    fn deserialize<T: serde::de::DeserializeOwned>(&self, bytes: &[u8]) -> OrbitResult<T> {
        serde_json::from_slice(bytes).map_err(|e| OrbitError::internal(format!("JSON deserialization failed: {}", e)))
    }

    fn content_type(&self) -> &str {
        "application/json"
    }
}

/// Bincode serialization (more efficient)
pub struct BincodeSerialization;

impl SerializationStrategy for BincodeSerialization {
    fn serialize<T: serde::Serialize>(&self, data: &T) -> OrbitResult<Vec<u8>> {
        bincode::serialize(data).map_err(|e| OrbitError::internal(format!("Bincode serialization failed: {}", e)))
    }

    fn deserialize<T: serde::de::DeserializeOwned>(&self, bytes: &[u8]) -> OrbitResult<T> {
        bincode::deserialize(bytes).map_err(|e| OrbitError::internal(format!("Bincode deserialization failed: {}", e)))
    }

    fn content_type(&self) -> &str {
        "application/octet-stream"
    }
}

// ===== Compression Strategy =====

use std::io::{Read, Write};

/// Trait for compression strategies
pub trait CompressionStrategy: Send + Sync {
    /// Compress data
    fn compress(&self, data: &[u8]) -> OrbitResult<Vec<u8>>;

    /// Decompress data
    fn decompress(&self, data: &[u8]) -> OrbitResult<Vec<u8>>;

    /// Get compression algorithm name
    fn algorithm(&self) -> &str;
}

/// No compression
pub struct NoCompression;

impl CompressionStrategy for NoCompression {
    fn compress(&self, data: &[u8]) -> OrbitResult<Vec<u8>> {
        Ok(data.to_vec())
    }

    fn decompress(&self, data: &[u8]) -> OrbitResult<Vec<u8>> {
        Ok(data.to_vec())
    }

    fn algorithm(&self) -> &str {
        "none"
    }
}

/// Gzip compression
pub struct GzipCompression {
    pub level: u32,
}

impl Default for GzipCompression {
    fn default() -> Self {
        Self { level: 6 }
    }
}

impl CompressionStrategy for GzipCompression {
    fn compress(&self, data: &[u8]) -> OrbitResult<Vec<u8>> {
        use flate2::write::GzEncoder;
        use flate2::Compression;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::new(self.level));
        encoder.write_all(data).map_err(|e| OrbitError::internal(format!("Gzip compression failed: {}", e)))?;
        encoder.finish().map_err(|e| OrbitError::internal(format!("Gzip finalization failed: {}", e)))
    }

    fn decompress(&self, data: &[u8]) -> OrbitResult<Vec<u8>> {
        use flate2::read::GzDecoder;

        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).map_err(|e| OrbitError::internal(format!("Gzip decompression failed: {}", e)))?;
        Ok(decompressed)
    }

    fn algorithm(&self) -> &str {
        "gzip"
    }
}

// ===== Strategy Context =====

/// Context that uses strategies
pub struct DataProcessor {
    serialization: Arc<dyn SerializationStrategy>,
    compression: Arc<dyn CompressionStrategy>,
}

impl DataProcessor {
    pub fn new(
        serialization: Arc<dyn SerializationStrategy>,
        compression: Arc<dyn CompressionStrategy>,
    ) -> Self {
        Self {
            serialization,
            compression,
        }
    }

    pub fn process<T: serde::Serialize>(&self, data: &T) -> OrbitResult<Vec<u8>> {
        let serialized = self.serialization.serialize(data)?;
        let compressed = self.compression.compress(&serialized)?;
        Ok(compressed)
    }

    pub fn unprocess<T: serde::de::DeserializeOwned>(&self, data: &[u8]) -> OrbitResult<T> {
        let decompressed = self.compression.decompress(data)?;
        let deserialized = self.serialization.deserialize(&decompressed)?;
        Ok(deserialized)
    }

    /// Change strategies at runtime
    pub fn set_serialization(&mut self, strategy: Arc<dyn SerializationStrategy>) {
        self.serialization = strategy;
    }

    pub fn set_compression(&mut self, strategy: Arc<dyn CompressionStrategy>) {
        self.compression = strategy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_exponential_backoff() {
        let strategy = ExponentialBackoff::default();

        let delay0 = strategy.calculate_delay(0);
        let delay1 = strategy.calculate_delay(1);
        let delay2 = strategy.calculate_delay(2);

        assert_eq!(delay0.as_millis(), 100);
        assert_eq!(delay1.as_millis(), 200);
        assert_eq!(delay2.as_millis(), 400);
    }

    #[tokio::test]
    async fn test_with_retry_success() {
        let strategy = Arc::new(ExponentialBackoff::default());
        let mut attempts = 0;

        let result = with_retry(strategy, || async {
            attempts += 1;
            if attempts < 3 {
                Err(OrbitError::network("temporary error"))
            } else {
                Ok(42)
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts, 3);
    }

    #[test]
    fn test_serialization_strategies() {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct TestData {
            name: String,
            value: i32,
        }

        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        // JSON
        let json_strategy = JsonSerialization;
        let serialized = json_strategy.serialize(&data).unwrap();
        let deserialized: TestData = json_strategy.deserialize(&serialized).unwrap();
        assert_eq!(data, deserialized);

        // Bincode
        let bincode_strategy = BincodeSerialization;
        let serialized = bincode_strategy.serialize(&data).unwrap();
        let deserialized: TestData = bincode_strategy.deserialize(&serialized).unwrap();
        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_compression_strategies() {
        let data = b"Hello, World! This is a test string that should compress well.";

        // No compression
        let no_compression = NoCompression;
        let compressed = no_compression.compress(data).unwrap();
        assert_eq!(compressed, data);

        // Gzip
        let gzip = GzipCompression::default();
        let compressed = gzip.compress(data).unwrap();
        assert!(compressed.len() < data.len()); // Should be smaller
        let decompressed = gzip.decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_data_processor() {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct TestData {
            value: String,
        }

        let data = TestData {
            value: "test data".to_string(),
        };

        let processor = DataProcessor::new(
            Arc::new(JsonSerialization),
            Arc::new(NoCompression),
        );

        let processed = processor.process(&data).unwrap();
        let unprocessed: TestData = processor.unprocess(&processed).unwrap();

        assert_eq!(data, unprocessed);
    }
}
