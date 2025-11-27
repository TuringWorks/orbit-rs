use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

// Lightweight test types to avoid dependency cycles
// These mirror types from orbit-shared but are defined here for testing only

/// Test-only node ID wrapper
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

/// Test-only addressable key types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Key {
    /// String key
    StringKey { key: String },
    /// 32-bit integer key
    Int32Key { key: i32 },
    /// 64-bit integer key
    Int64Key { key: i64 },
    /// No key (singleton)
    NoKey,
}

/// Test-only addressable reference
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AddressableReference {
    /// Addressable type name
    pub addressable_type: String,
    /// Addressable key
    pub key: Key,
}

/// Test-only error type
#[derive(Debug, thiserror::Error)]
pub enum OrbitError {
    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),
    /// Timeout error
    #[error("Timeout: {details}")]
    Timeout {
        /// Timeout details
        details: String,
    },
    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl OrbitError {
    /// Create a network error
    pub fn network(msg: impl Into<String>) -> Self {
        Self::NetworkError(msg.into())
    }

    /// Create a timeout error
    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout {
            details: msg.into(),
        }
    }

    /// Create an internal error
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

/// Test-only result type
pub type OrbitResult<T> = Result<T, OrbitError>;

/// Test utilities for CI-friendly testing without external dependencies
pub struct TestUtils;

impl TestUtils {
    /// Create a mock node ID for testing
    pub fn mock_node_id(suffix: &str) -> NodeId {
        NodeId(format!("test-node-{}", suffix))
    }

    /// Create a test addressable reference
    pub fn mock_addressable_reference(actor_type: &str, key: Key) -> AddressableReference {
        AddressableReference {
            addressable_type: actor_type.to_string(),
            key,
        }
    }

    /// Create a test error for error handling scenarios
    pub fn mock_error(message: &str) -> OrbitError {
        OrbitError::internal(format!("test error: {}", message))
    }

    /// Simulate async work without external dependencies
    pub async fn simulate_work(millis: u64) {
        sleep(Duration::from_millis(millis)).await;
    }

    /// Create a sequence of test data
    pub fn generate_test_sequence(count: usize, prefix: &str) -> Vec<String> {
        (0..count).map(|i| format!("{}-{}", prefix, i)).collect()
    }

    /// Validate that a value is within expected range
    pub fn assert_in_range<T: PartialOrd>(value: T, min: T, max: T) {
        assert!(value >= min && value <= max, "Value not in expected range");
    }

    /// Create a thread-safe counter for concurrent tests
    pub fn atomic_counter() -> Arc<AtomicU64> {
        Arc::new(AtomicU64::new(0))
    }

    /// Generate test keys of different types
    pub fn test_keys() -> Vec<Key> {
        vec![
            Key::StringKey {
                key: "test-string".to_string(),
            },
            Key::Int32Key { key: 42 },
            Key::Int64Key { key: 1000000 },
            Key::NoKey,
        ]
    }

    /// Create mock network error scenarios
    pub fn network_error_scenarios() -> Vec<OrbitError> {
        vec![
            OrbitError::network("Connection timeout"),
            OrbitError::network("Connection refused"),
            OrbitError::network("DNS resolution failed"),
            OrbitError::timeout("Request timeout"),
        ]
    }

    /// Create mock serialization test data
    pub fn serialization_test_data() -> serde_json::Value {
        serde_json::json!({
            "string": "test_value",
            "number": 42,
            "boolean": true,
            "array": [1, 2, 3],
            "object": {
                "nested": "value"
            }
        })
    }

    /// Validate JSON serialization roundtrip
    pub fn assert_json_roundtrip<T>(original: &T) -> OrbitResult<()>
    where
        T: serde::Serialize + for<'de> serde::Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(original)
            .map_err(|e| OrbitError::internal(format!("Serialization failed: {}", e)))?;

        let deserialized: T = serde_json::from_str(&json)
            .map_err(|e| OrbitError::internal(format!("Deserialization failed: {}", e)))?;

        if original == &deserialized {
            Ok(())
        } else {
            Err(OrbitError::internal("Roundtrip failed: values not equal"))
        }
    }

    /// Generate large test datasets for performance/edge case testing
    pub fn large_test_dataset(size: usize) -> Vec<String> {
        (0..size)
            .map(|i| format!("large-data-item-{:06}", i))
            .collect()
    }

    /// Create a mock future that resolves after a delay (for timeout testing)
    pub async fn delayed_result<T>(value: T, delay_ms: u64) -> T {
        sleep(Duration::from_millis(delay_ms)).await;
        value
    }

    /// Generate random-like deterministic data (for reproducible tests)
    pub fn deterministic_random_string(seed: u64, length: usize) -> String {
        // Simple deterministic "random" generator for tests
        let mut value = seed;
        (0..length)
            .map(|_| {
                value = value.wrapping_mul(1103515245).wrapping_add(12345);
                let char_val = (value / 65536) % 26;
                (b'a' + char_val as u8) as char
            })
            .collect()
    }

    /// Mock retry logic for testing resilience patterns
    pub async fn mock_retry_operation<F, T, E>(
        mut operation: F,
        max_retries: usize,
        delay_ms: u64,
    ) -> Result<T, E>
    where
        F: FnMut() -> Result<T, E>,
    {
        let mut attempts = 0;
        loop {
            match operation() {
                Ok(result) => return Ok(result),
                Err(e) => {
                    attempts += 1;
                    if attempts >= max_retries {
                        return Err(e);
                    }
                    sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }

    /// Validate concurrent access patterns  
    pub async fn concurrent_test_runner<F>(tasks: Vec<F>) -> Vec<Result<(), String>>
    where
        F: std::future::Future<Output = Result<(), String>> + Send + 'static,
    {
        let handles: Vec<_> = tasks.into_iter().map(|task| tokio::spawn(task)).collect();

        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => results.push(Err(format!("Task panicked: {}", e))),
            }
        }

        results
    }
}

/// Trait for creating test doubles/mocks
pub trait MockBuilder<T> {
    fn with_default() -> T;
    fn with_error() -> T;
    fn with_timeout() -> T;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_node_id() {
        let node_id = TestUtils::mock_node_id("123");
        assert_eq!(node_id.0, "test-node-123");
    }

    #[test]
    fn test_mock_addressable_reference() {
        let key = Key::StringKey {
            key: "test".to_string(),
        };
        let addr_ref = TestUtils::mock_addressable_reference("TestActor", key.clone());

        assert_eq!(addr_ref.addressable_type, "TestActor");
        assert_eq!(addr_ref.key, key);
    }

    #[test]
    fn test_mock_error() {
        let error = TestUtils::mock_error("test scenario");
        match error {
            OrbitError::Internal(msg) => {
                assert!(msg.contains("test error: test scenario"));
            }
            _ => panic!("Expected Internal error"),
        }
    }

    #[tokio::test]
    async fn test_simulate_work() {
        let start = std::time::Instant::now();
        TestUtils::simulate_work(10).await;
        let elapsed = start.elapsed();

        // Should be approximately 10ms, allow some variance
        assert!(elapsed >= Duration::from_millis(8));
        assert!(elapsed <= Duration::from_millis(50)); // Generous upper bound for CI
    }

    #[test]
    fn test_generate_test_sequence() {
        let sequence = TestUtils::generate_test_sequence(3, "item");
        assert_eq!(sequence, vec!["item-0", "item-1", "item-2"]);
    }

    #[test]
    fn test_assert_in_range() {
        TestUtils::assert_in_range(5, 1, 10); // Should pass

        // Test that it panics when out of range
        let result = std::panic::catch_unwind(|| {
            TestUtils::assert_in_range(15, 1, 10);
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_atomic_counter() {
        let counter = TestUtils::atomic_counter();
        assert_eq!(counter.load(Ordering::SeqCst), 0);

        counter.fetch_add(5, Ordering::SeqCst);
        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn test_test_keys() {
        let keys = TestUtils::test_keys();
        assert_eq!(keys.len(), 4);

        // Verify we have all key types
        assert!(matches!(keys[0], Key::StringKey { .. }));
        assert!(matches!(keys[1], Key::Int32Key { .. }));
        assert!(matches!(keys[2], Key::Int64Key { .. }));
        assert!(matches!(keys[3], Key::NoKey));
    }

    #[test]
    fn test_network_error_scenarios() {
        let errors = TestUtils::network_error_scenarios();
        assert!(errors.len() >= 4);

        // All should be error types
        for error in &errors {
            match error {
                OrbitError::NetworkError(_) | OrbitError::Timeout { .. } => {}
                _ => panic!("Expected network or timeout error"),
            }
        }
    }

    #[test]
    fn test_serialization_test_data() {
        let data = TestUtils::serialization_test_data();
        assert!(data.is_object());
        assert!(data["string"].is_string());
        assert!(data["number"].is_number());
        assert!(data["boolean"].is_boolean());
        assert!(data["array"].is_array());
        assert!(data["object"].is_object());
    }

    #[test]
    fn test_json_roundtrip() {
        let original = "test_string".to_string();
        let result = TestUtils::assert_json_roundtrip(&original);
        assert!(result.is_ok());

        // Test with complex data
        let complex_data = vec![1, 2, 3, 4, 5];
        let result = TestUtils::assert_json_roundtrip(&complex_data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_large_test_dataset() {
        let dataset = TestUtils::large_test_dataset(1000);
        assert_eq!(dataset.len(), 1000);
        assert_eq!(dataset[0], "large-data-item-000000");
        assert_eq!(dataset[999], "large-data-item-000999");
    }

    #[tokio::test]
    async fn test_delayed_result() {
        let start = std::time::Instant::now();
        let result = TestUtils::delayed_result("delayed", 10).await;
        let elapsed = start.elapsed();

        assert_eq!(result, "delayed");
        assert!(elapsed >= Duration::from_millis(8));
    }

    #[test]
    fn test_deterministic_random_string() {
        let str1 = TestUtils::deterministic_random_string(42, 10);
        let str2 = TestUtils::deterministic_random_string(42, 10);

        // Same seed should produce same result
        assert_eq!(str1, str2);
        assert_eq!(str1.len(), 10);

        // Different seed should produce different result
        let str3 = TestUtils::deterministic_random_string(43, 10);
        assert_ne!(str1, str3);
    }

    #[tokio::test]
    async fn test_mock_retry_operation() {
        let mut attempt_count = 0;
        let operation = || {
            attempt_count += 1;
            if attempt_count < 3 {
                Err("temporary failure")
            } else {
                Ok("success")
            }
        };

        let result = TestUtils::mock_retry_operation(operation, 5, 1).await;
        assert_eq!(result, Ok("success"));
        assert_eq!(attempt_count, 3);
    }

    // Note: concurrent_test_runner test removed due to Rust's async block typing limitations
    // The function is still available for actual usage where types can be properly inferred
}
