//! Additional comprehensive tests for config utilities

#[cfg(test)]
mod extended_tests {
    use super::super::config_utils::*;
    use std::time::Duration;
    
    #[test]
    fn test_timeout_config_edge_cases() {
        let mut config = TimeoutConfig::default();
        
        // Test zero timeout validation
        config.connection_timeout = Duration::from_millis(0);
        assert!(config.validate().is_err(), "Zero connection timeout should be invalid");
        
        // Test very large timeouts
        config.connection_timeout = Duration::from_secs(3600); // 1 hour
        config.request_timeout = Duration::from_secs(7200); // 2 hours
        assert!(config.validate().is_ok(), "Large timeouts should be valid");
        
        // Test edge case where timeouts are equal
        config.request_timeout = Duration::from_secs(3600);
        assert!(config.validate().is_ok(), "Equal timeouts should be valid");
    }
    
    #[test]
    fn test_retry_config_edge_cases() {
        let mut config = RetryConfig::default();
        
        // Test with backoff multiplier exactly 1.0
        config.backoff_multiplier = 1.0;
        assert!(config.validate().is_ok(), "Backoff multiplier of 1.0 should be valid");
        
        // Test with very high max attempts
        config.max_attempts = 1000;
        assert!(config.validate().is_ok(), "High max attempts should be valid");
        
        // Test with fractional backoff multiplier
        config.backoff_multiplier = 0.5;
        assert!(config.validate().is_err(), "Backoff multiplier < 1.0 should be invalid");
    }
    
    #[test]
    fn test_resource_config_edge_cases() {
        // Test with very large buffer size
        let config = ResourceConfig {
            buffer_size: usize::MAX,
            ..Default::default()
        };
        assert!(config.validate().is_ok(), "Large buffer size should be valid");
        
        // Test with zero memory limit (None should be valid)
        let config = ResourceConfig {
            max_memory_mb: None,
            ..Default::default()
        };
        assert!(config.validate().is_ok(), "No memory limit should be valid");
        
        // Test with minimum valid values
        let config = ResourceConfig {
            max_memory_mb: Some(1),
            max_connections: 1,
            buffer_size: 1,
            ..Default::default()
        };
        assert!(config.validate().is_ok(), "Minimum valid values should be accepted");
    }
    
    #[test]
    fn test_config_defaults_trait() {
        let timeout_config = TimeoutConfig::default();
        assert_eq!(timeout_config.config_source(), "default");
        
        // Test merge functionality (default implementation)
        let config1 = TimeoutConfig::default();
        let mut config2 = TimeoutConfig::default();
        config2.connection_timeout = Duration::from_millis(1000);
        
        let merged = config1.merge(config2.clone());
        assert_eq!(merged.connection_timeout, config2.connection_timeout);
    }
    
    #[test]
    fn test_get_env_or_default() {
        // Test with non-existent environment variable
        let result: i32 = get_env_or_default("NONEXISTENT_TEST_VAR", 42);
        assert_eq!(result, 42);
        
        // Test with different types
        let result: String = get_env_or_default("NONEXISTENT_STRING_VAR", "default".to_string());
        assert_eq!(result, "default");
        
        let result: bool = get_env_or_default("NONEXISTENT_BOOL_VAR", true);
        assert!(result);
    }
    
    #[test]
    fn test_config_builder_macro() {
        // This would test the config_builder! macro if it were available
        // For now, we'll create a simple test config to verify the pattern
        
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        struct TestConfig {
            pub name: String,
            pub port: u16,
            pub enabled: bool,
        }
        
        impl Default for TestConfig {
            fn default() -> Self {
                Self {
                    name: "test".to_string(),
                    port: 8080,
                    enabled: true,
                }
            }
        }
        
        impl ConfigDefaults for TestConfig {}
        
        let config = TestConfig::default();
        assert_eq!(config.name, "test");
        assert_eq!(config.port, 8080);
        assert!(config.enabled);
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_timeout_config_serialization() {
        let config = TimeoutConfig::default();
        
        // Test JSON serialization
        let json = serde_json::to_string(&config).expect("Should serialize to JSON");
        let deserialized: TimeoutConfig = serde_json::from_str(&json).expect("Should deserialize from JSON");
        
        assert_eq!(config.connection_timeout, deserialized.connection_timeout);
        assert_eq!(config.request_timeout, deserialized.request_timeout);
        assert_eq!(config.health_check_interval, deserialized.health_check_interval);
    }
    
    #[test]
    fn test_retry_config_serialization() {
        let config = RetryConfig::default();
        
        let json = serde_json::to_string(&config).expect("Should serialize to JSON");
        let deserialized: RetryConfig = serde_json::from_str(&json).expect("Should deserialize from JSON");
        
        assert_eq!(config.max_attempts, deserialized.max_attempts);
        assert_eq!(config.initial_delay, deserialized.initial_delay);
        assert_eq!(config.max_delay, deserialized.max_delay);
        assert_eq!(config.backoff_multiplier, deserialized.backoff_multiplier);
    }
    
    #[test]
    fn test_resource_config_serialization() {
        let config = ResourceConfig {
            max_memory_mb: Some(1024),
            max_connections: 50,
            thread_pool_size: Some(4),
            buffer_size: 4096,
        };
        
        let json = serde_json::to_string(&config).expect("Should serialize to JSON");
        let deserialized: ResourceConfig = serde_json::from_str(&json).expect("Should deserialize from JSON");
        
        assert_eq!(config.max_memory_mb, deserialized.max_memory_mb);
        assert_eq!(config.max_connections, deserialized.max_connections);
        assert_eq!(config.thread_pool_size, deserialized.thread_pool_size);
        assert_eq!(config.buffer_size, deserialized.buffer_size);
    }
    
    #[test]
    fn test_validation_error_messages() {
        let mut timeout_config = TimeoutConfig::default();
        timeout_config.connection_timeout = Duration::from_millis(0);
        
        let error = timeout_config.validate().unwrap_err();
        assert!(error.contains("Connection timeout must be greater than 0"));
        
        let mut retry_config = RetryConfig::default();
        retry_config.max_attempts = 0;
        
        let error = retry_config.validate().unwrap_err();
        assert!(error.contains("Max attempts must be greater than 0"));
        
        let resource_config = ResourceConfig {
            max_connections: 0,
            ..Default::default()
        };
        
        let error = resource_config.validate().unwrap_err();
        assert!(error.contains("Max connections must be greater than 0"));
    }
    
    #[test]
    fn test_config_cloning() {
        let original = TimeoutConfig::default();
        let cloned = original.clone();
        
        assert_eq!(original.connection_timeout, cloned.connection_timeout);
        assert_eq!(original.request_timeout, cloned.request_timeout);
        assert_eq!(original.health_check_interval, cloned.health_check_interval);
    }
    
    #[test]
    fn test_config_debug_formatting() {
        let config = TimeoutConfig::default();
        let debug_str = format!("{:?}", config);
        
        assert!(debug_str.contains("TimeoutConfig"));
        assert!(debug_str.contains("connection_timeout"));
        assert!(debug_str.contains("request_timeout"));
        assert!(debug_str.contains("health_check_interval"));
    }
}