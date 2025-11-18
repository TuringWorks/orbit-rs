//! Test configuration utilities for choosing between mock and real services
//!
//! This module provides utilities to determine whether tests should use mock
//! implementations or attempt to connect to real services based on environment
//! variables, features, and other configuration.

use std::env;
use std::time::Duration;

/// Test execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TestMode {
    /// Use mock implementations (default for CI/unit tests)
    #[default]
    Mock,
    /// Use real services (for integration tests with running services)
    Real,
    /// Skip tests that require external services
    Skip,
}

/// Test configuration for Orbit components
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// Test execution mode
    pub mode: TestMode,
    /// Timeout for connecting to real services
    pub connection_timeout: Duration,
    /// Whether to enable verbose logging during tests
    pub verbose_logging: bool,
    /// Base URLs for real services (when using Real mode)
    pub service_urls: TestServiceUrls,
    /// Mock configuration (when using Mock mode)
    pub mock_config: crate::mocks::MockConfig,
}

#[derive(Debug, Clone)]
pub struct TestServiceUrls {
    /// OrbitServer gRPC URL
    pub orbit_server: String,
    /// Redis server URL for RESP tests
    pub redis_server: String,
    /// PostgreSQL server URL for postgres wire tests
    pub postgres_server: String,
}

impl Default for TestServiceUrls {
    fn default() -> Self {
        Self {
            orbit_server: "http://localhost:50051".to_string(),
            redis_server: "redis://localhost:6379".to_string(),
            postgres_server: "postgres://test:test@localhost:5432/test".to_string(),
        }
    }
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            mode: TestMode::default(),
            connection_timeout: Duration::from_secs(5),
            verbose_logging: false,
            service_urls: TestServiceUrls::default(),
            mock_config: crate::mocks::MockConfig::default(),
        }
    }
}

impl TestConfig {
    /// Create test configuration based on environment variables
    pub fn from_env() -> Self {
        // Determine test mode
        let mode = if env::var("ORBIT_TEST_MODE").as_deref() == Ok("real") {
            TestMode::Real
        } else if env::var("ORBIT_TEST_MODE").as_deref() == Ok("skip") {
            TestMode::Skip
        } else if env::var("CI").is_ok() || env::var("GITHUB_ACTIONS").is_ok() {
            // Default to Mock in CI environments
            TestMode::Mock
        } else {
            // Allow real services in development
            TestMode::Mock
        };

        // Configure verbose logging
        let verbose_logging =
            env::var("RUST_LOG").is_ok() || env::var("ORBIT_TEST_VERBOSE").as_deref() == Ok("true");

        // Configure connection timeout
        let connection_timeout = if let Ok(timeout_str) = env::var("ORBIT_TEST_TIMEOUT") {
            if let Ok(timeout_secs) = timeout_str.parse::<u64>() {
                Duration::from_secs(timeout_secs)
            } else {
                Duration::from_secs(5)
            }
        } else {
            Duration::from_secs(5)
        };

        // Configure service URLs
        let mut service_urls = TestServiceUrls::default();
        if let Ok(url) = env::var("ORBIT_SERVER_URL") {
            service_urls.orbit_server = url;
        }
        if let Ok(url) = env::var("REDIS_URL") {
            service_urls.redis_server = url;
        }
        if let Ok(url) = env::var("DATABASE_URL") {
            service_urls.postgres_server = url;
        }

        // Configure mock behavior
        let failure_rate = if let Ok(rate_str) = env::var("ORBIT_TEST_FAILURE_RATE") {
            if let Ok(rate) = rate_str.parse::<f64>() {
                rate.clamp(0.0, 1.0)
            } else {
                0.0
            }
        } else {
            0.0
        };

        let (simulate_network_delay, network_delay) =
            if env::var("ORBIT_TEST_NETWORK_DELAY").as_deref() == Ok("true") {
                (true, Duration::from_millis(10))
            } else {
                (false, Duration::from_millis(10))
            };

        let mock_config = crate::mocks::MockConfig {
            simulate_network_delay,
            network_delay,
            failure_rate,
            max_actors: 1000,
            verbose_logging,
        };

        Self {
            mode,
            connection_timeout,
            verbose_logging,
            service_urls,
            mock_config,
        }
    }

    /// Check if tests should use mock implementations
    pub fn use_mocks(&self) -> bool {
        matches!(self.mode, TestMode::Mock)
    }

    /// Check if tests should use real services
    pub fn use_real_services(&self) -> bool {
        matches!(self.mode, TestMode::Real)
    }

    /// Check if tests should be skipped
    pub fn should_skip(&self) -> bool {
        matches!(self.mode, TestMode::Skip)
    }

    /// Get a description of the current test configuration
    pub fn description(&self) -> String {
        match self.mode {
            TestMode::Mock => "Using mock implementations".to_string(),
            TestMode::Real => format!(
                "Using real services: orbit={}",
                self.service_urls.orbit_server
            ),
            TestMode::Skip => "Skipping tests that require external services".to_string(),
        }
    }
}

/// Macro to skip tests based on configuration
#[macro_export]
macro_rules! skip_test_if_configured {
    () => {
        let test_config = $crate::test_config::TestConfig::from_env();
        if test_config.should_skip() {
            eprintln!("⏭️  {}", test_config.description());
            return;
        }
    };
}

/// Macro to get test configuration and print it
#[macro_export]
macro_rules! test_config {
    () => {{
        let config = $crate::test_config::TestConfig::from_env();
        if config.verbose_logging {
            eprintln!("🔧 Test config: {}", config.description());
        }
        config
    }};
}

/// Test setup helper that works with both mock and real services
pub struct UniversalTestSetup {
    config: TestConfig,
    mock_setup: Option<crate::mocks::MockTestSetup>,
}

impl UniversalTestSetup {
    /// Create a new universal test setup
    pub async fn new() -> anyhow::Result<Self> {
        let config = TestConfig::from_env();

        if config.verbose_logging {
            eprintln!("🔧 {}", config.description());
        }

        let mock_setup = if config.use_mocks() {
            Some(
                crate::mocks::MockTestSetup::builder()
                    .with_namespace("universal-test")
                    .with_client_config(config.mock_config.clone())
                    .with_server_config(config.mock_config.clone())
                    .build()
                    .await,
            )
        } else {
            None
        };

        Ok(Self { config, mock_setup })
    }

    /// Start the test setup
    pub async fn start(&self) -> anyhow::Result<()> {
        if let Some(mock_setup) = &self.mock_setup {
            mock_setup.start().await.map_err(|e| anyhow::anyhow!(e))?;
        }
        // For real services, we assume they're already running
        Ok(())
    }

    /// Register actor types (only relevant for mock mode)
    pub async fn register_actor_types(&self, types: &[&str]) -> anyhow::Result<()> {
        if let Some(mock_setup) = &self.mock_setup {
            mock_setup
                .register_actor_types(types)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
        }
        Ok(())
    }

    /// Clean up resources
    pub async fn cleanup(&self) -> anyhow::Result<()> {
        if let Some(mock_setup) = &self.mock_setup {
            mock_setup.cleanup().await.map_err(|e| anyhow::anyhow!(e))?;
        }
        Ok(())
    }

    /// Get test mode
    pub fn mode(&self) -> TestMode {
        self.config.mode
    }

    /// Check if using mocks
    pub fn is_using_mocks(&self) -> bool {
        self.config.use_mocks()
    }

    /// Get mock setup (if in mock mode)
    pub fn mock_setup(&self) -> Option<&crate::mocks::MockTestSetup> {
        self.mock_setup.as_ref()
    }

    /// Get service URLs (if in real mode)
    pub fn service_urls(&self) -> &TestServiceUrls {
        &self.config.service_urls
    }
}

/// Print test configuration information
pub fn print_test_info() {
    let config = TestConfig::from_env();
    println!("🔧 Orbit Test Configuration:");
    println!("   Mode: {:?}", config.mode);
    println!("   Description: {}", config.description());
    println!("   Verbose logging: {}", config.verbose_logging);
    println!("   Connection timeout: {:?}", config.connection_timeout);

    if config.use_real_services() {
        println!("   Service URLs:");
        println!("     OrbitServer: {}", config.service_urls.orbit_server);
        println!("     Redis: {}", config.service_urls.redis_server);
        println!("     PostgreSQL: {}", config.service_urls.postgres_server);
    }

    if config.use_mocks() {
        println!("   Mock configuration:");
        println!("     Failure rate: {}", config.mock_config.failure_rate);
        println!(
            "     Network delay: {}",
            config.mock_config.simulate_network_delay
        );
        println!("     Max actors: {}", config.mock_config.max_actors);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_default_config() {
        let config = TestConfig::default();
        assert_eq!(config.mode, TestMode::Mock);
        assert!(!config.verbose_logging);
        assert_eq!(config.connection_timeout, Duration::from_secs(5));
    }

    #[test]
    fn test_env_config() {
        // Set environment variable
        env::set_var("ORBIT_TEST_MODE", "real");
        env::set_var("ORBIT_TEST_VERBOSE", "true");

        let config = TestConfig::from_env();
        assert_eq!(config.mode, TestMode::Real);
        assert!(config.verbose_logging);

        // Clean up
        env::remove_var("ORBIT_TEST_MODE");
        env::remove_var("ORBIT_TEST_VERBOSE");
    }

    #[test]
    fn test_ci_detection() {
        // Store original values
        let original_orbit_test_mode = env::var("ORBIT_TEST_MODE").ok();
        let original_ci = env::var("CI").ok();
        let original_github_actions = env::var("GITHUB_ACTIONS").ok();

        // Ensure ORBIT_TEST_MODE is not set (so CI detection logic is used)
        env::remove_var("ORBIT_TEST_MODE");

        // Set CI environment variable
        env::set_var("CI", "true");

        let config = TestConfig::from_env();
        assert_eq!(config.mode, TestMode::Mock);

        // Clean up - restore original values
        env::remove_var("CI");
        if let Some(val) = original_orbit_test_mode {
            env::set_var("ORBIT_TEST_MODE", val);
        }
        if let Some(val) = original_ci {
            env::set_var("CI", val);
        }
        if let Some(val) = original_github_actions {
            env::set_var("GITHUB_ACTIONS", val);
        }
    }

    #[test]
    fn test_ci_detection_with_github_actions() {
        // Store original values
        let original_orbit_test_mode = env::var("ORBIT_TEST_MODE").ok();
        let original_ci = env::var("CI").ok();
        let original_github_actions = env::var("GITHUB_ACTIONS").ok();

        // Ensure ORBIT_TEST_MODE is not set (so CI detection logic is used)
        env::remove_var("ORBIT_TEST_MODE");

        // Set GITHUB_ACTIONS environment variable
        env::set_var("GITHUB_ACTIONS", "true");

        let config = TestConfig::from_env();
        assert_eq!(config.mode, TestMode::Mock);

        // Clean up - restore original values
        env::remove_var("GITHUB_ACTIONS");
        if let Some(val) = original_orbit_test_mode {
            env::set_var("ORBIT_TEST_MODE", val);
        }
        if let Some(val) = original_ci {
            env::set_var("CI", val);
        }
        if let Some(val) = original_github_actions {
            env::set_var("GITHUB_ACTIONS", val);
        }
    }

    #[tokio::test]
    async fn test_universal_setup() {
        let setup = UniversalTestSetup::new().await.unwrap();
        setup.start().await.unwrap();
        setup.register_actor_types(&["TestActor"]).await.unwrap();
        setup.cleanup().await.unwrap();
    }
}
