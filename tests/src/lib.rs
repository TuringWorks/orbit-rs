//! Orbit-RS Integration Tests Library
//!
//! This crate provides integration tests for the Orbit-RS distributed database system.
//! Tests are organized by feature and can be enabled via feature flags.
//!
//! ## Test Modules
//!
//! The following test modules are available as standalone test files in the tests/ directory:
//! - `neo4j/` - Neo4j Bolt protocol and Cypher compatibility tests (enable with `neo4j-features`)
//! - `graphml/` - Graph Machine Learning tests (enable with `graphml-features`)
//! - `bdd/` - BDD/Cucumber tests for scenario validation
//! - `integration/` - Python integration tests for protocol verification
//! - `orbit-protocols/` - Protocol-specific Rust integration tests
//! - `orbit-engine/` - Storage engine tests

// Re-export common test utilities
pub mod utils {
    //! Common test utilities and helpers.

    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// Test configuration for integration tests
    #[derive(Debug, Clone)]
    pub struct TestConfig {
        /// Host address for the Orbit server
        pub host: String,
        /// RESP protocol port
        pub resp_port: u16,
        /// PostgreSQL wire protocol port
        pub postgres_port: u16,
        /// Vector store port
        pub vector_port: u16,
    }

    impl Default for TestConfig {
        fn default() -> Self {
            Self {
                host: "localhost".to_string(),
                resp_port: 6379,
                postgres_port: 5432,
                vector_port: 6381,
            }
        }
    }

    /// Create a test configuration from environment variables
    pub fn config_from_env() -> TestConfig {
        TestConfig {
            host: std::env::var("ORBIT_TEST_HOST").unwrap_or_else(|_| "localhost".to_string()),
            resp_port: std::env::var("ORBIT_TEST_RESP_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(6379),
            postgres_port: std::env::var("ORBIT_TEST_POSTGRES_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5432),
            vector_port: std::env::var("ORBIT_TEST_VECTOR_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(6381),
        }
    }

    /// Shared test state wrapper for concurrent tests
    pub type SharedState<T> = Arc<RwLock<T>>;

    /// Create a new shared state wrapper
    pub fn shared<T>(value: T) -> SharedState<T> {
        Arc::new(RwLock::new(value))
    }
}

#[cfg(test)]
mod tests {
    use super::utils::*;

    #[test]
    fn test_default_config() {
        let config = TestConfig::default();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.resp_port, 6379);
        assert_eq!(config.postgres_port, 5432);
        assert_eq!(config.vector_port, 6381);
    }

    #[test]
    fn test_shared_state() {
        let state = shared(42);
        assert!(std::sync::Arc::strong_count(&state) == 1);
    }
}
