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
        attempt < self.max_retries
            && matches!(
                error,
                OrbitError::NetworkError(_) | OrbitError::Timeout { .. }
            )
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

/// Serialization formats, as a closed set.
///
/// A strategy whose operations are generic (`serialize<T>`) cannot be a trait
/// object: there is no vtable slot for a method that is monomorphized per type.
/// Rust's answer is a sum type. Dispatch is still chosen at runtime, the methods
/// stay generic, and adding a format turns every `match` into a compile error
/// instead of a silently missing case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Serialization {
    /// Self-describing and human-readable.
    Json,
    /// Compact binary. Not self-describing — decoding requires the target type,
    /// which is precisely why an erased `serde_json::Value` cannot stand in for
    /// `T` here.
    Bincode,
}

impl Serialization {
    /// Serialize a value in this format.
    ///
    /// # Errors
    /// Returns an error if the value cannot be encoded in this format.
    pub fn serialize<T: serde::Serialize>(self, data: &T) -> OrbitResult<Vec<u8>> {
        match self {
            Self::Json => serde_json::to_vec(data)
                .map_err(|e| OrbitError::internal(format!("JSON serialization failed: {e}"))),
            Self::Bincode => bincode::serialize(data)
                .map_err(|e| OrbitError::internal(format!("Bincode serialization failed: {e}"))),
        }
    }

    /// Deserialize a value from this format.
    ///
    /// # Errors
    /// Returns an error if the bytes are malformed or do not match `T`.
    pub fn deserialize<T: serde::de::DeserializeOwned>(self, bytes: &[u8]) -> OrbitResult<T> {
        match self {
            Self::Json => serde_json::from_slice(bytes)
                .map_err(|e| OrbitError::internal(format!("JSON deserialization failed: {e}"))),
            Self::Bincode => bincode::deserialize(bytes)
                .map_err(|e| OrbitError::internal(format!("Bincode deserialization failed: {e}"))),
        }
    }

    /// MIME type produced by this format.
    #[must_use]
    pub const fn content_type(self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Bincode => "application/octet-stream",
        }
    }
}

// ===== Compression Strategy =====

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

/// Run-length compression: `(count, byte)` pairs, counts capped at 255.
///
/// Named for what it does. A second strategy is needed to show the context
/// swapping algorithms at runtime, and run-length encoding earns that role
/// without pulling a compression crate into the dependency graph — real
/// deployments should reach for the codecs in `timeseries::compression` or a
/// dedicated crate instead.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunLengthCompression;

impl CompressionStrategy for RunLengthCompression {
    fn compress(&self, data: &[u8]) -> OrbitResult<Vec<u8>> {
        let runs = data.iter().fold(Vec::<(u8, u8)>::new(), |mut runs, &byte| {
            match runs.last_mut() {
                Some((value, count)) if *value == byte && *count < u8::MAX => *count += 1,
                _ => runs.push((byte, 1)),
            }
            runs
        });

        Ok(runs
            .into_iter()
            .flat_map(|(value, count)| [count, value])
            .collect())
    }

    fn decompress(&self, data: &[u8]) -> OrbitResult<Vec<u8>> {
        if !data.len().is_multiple_of(2) {
            return Err(OrbitError::internal(format!(
                "Run-length payload must be (count, byte) pairs, got {} bytes",
                data.len()
            )));
        }

        Ok(data
            .chunks_exact(2)
            .flat_map(|pair| std::iter::repeat_n(pair[1], usize::from(pair[0])))
            .collect())
    }

    fn algorithm(&self) -> &str {
        "run-length"
    }
}

// ===== Strategy Context =====

/// Context that combines both dispatch styles: a sum type where the operations
/// are generic, and a trait object where they are not.
pub struct DataProcessor {
    serialization: Serialization,
    compression: Arc<dyn CompressionStrategy>,
}

impl DataProcessor {
    pub fn new(serialization: Serialization, compression: Arc<dyn CompressionStrategy>) -> Self {
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
    pub fn set_serialization(&mut self, strategy: Serialization) {
        self.serialization = strategy;
    }

    pub fn set_compression(&mut self, strategy: Arc<dyn CompressionStrategy>) {
        self.compression = strategy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

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
        // The operation returns a future that outlives each closure call, so the
        // attempt counter is shared rather than mutably borrowed by the closure.
        let attempts = Arc::new(AtomicU32::new(0));
        let counter = Arc::clone(&attempts);

        let result = with_retry(strategy, move || {
            let counter = Arc::clone(&counter);
            async move {
                let attempt = counter.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt < 3 {
                    Err(OrbitError::network("temporary error"))
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
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

        // Every format round-trips, and each reports its own content type.
        for format in [Serialization::Json, Serialization::Bincode] {
            let serialized = format.serialize(&data).unwrap();
            let deserialized: TestData = format.deserialize(&serialized).unwrap();
            assert_eq!(data, deserialized, "{format:?} did not round-trip");
            assert!(!format.content_type().is_empty());
        }

        // Bincode is the more compact of the two for this payload.
        assert!(
            Serialization::Bincode.serialize(&data).unwrap().len()
                < Serialization::Json.serialize(&data).unwrap().len()
        );
    }

    #[test]
    fn test_compression_strategies() {
        let data = b"Hello, World! This is a test string that should compress well.";

        // No compression
        let no_compression = NoCompression;
        let compressed = no_compression.compress(data).unwrap();
        assert_eq!(compressed, data);

        // Run-length: round-trips, and shrinks input that actually has runs.
        let rle = RunLengthCompression;
        let decompressed = rle.decompress(&rle.compress(data).unwrap()).unwrap();
        assert_eq!(decompressed, data);

        let runs = vec![b'a'; 300];
        let compressed = rle.compress(&runs).unwrap();
        assert!(compressed.len() < runs.len());
        assert_eq!(rle.decompress(&compressed).unwrap(), runs);

        // A truncated payload is rejected, not silently half-decoded.
        assert!(rle.decompress(&[3]).is_err());
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

        let mut processor = DataProcessor::new(Serialization::Json, Arc::new(NoCompression));

        let processed = processor.process(&data).unwrap();
        let unprocessed: TestData = processor.unprocess(&processed).unwrap();
        assert_eq!(data, unprocessed);

        // Both strategies are swappable at runtime.
        processor.set_serialization(Serialization::Bincode);
        processor.set_compression(Arc::new(RunLengthCompression));
        let processed = processor.process(&data).unwrap();
        let unprocessed: TestData = processor.unprocess(&processed).unwrap();
        assert_eq!(data, unprocessed);
    }
}
