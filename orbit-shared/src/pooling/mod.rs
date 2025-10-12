//! Advanced connection pooling for production-scale deployments
//!
//! This module provides enterprise-grade connection management with:
//! - Multi-tier connection pooling (client, application, database)
//! - Connection health monitoring and automatic recovery
//! - Dynamic pool sizing based on load
//! - Connection timeout and idle connection management
//! - Load balancing across multiple database nodes
//! - Comprehensive connection pooling metrics and monitoring

pub mod advanced_pool;
pub mod circuit_breaker;
pub mod health_monitor;
pub mod load_balancer;

pub use advanced_pool::{
    AdvancedConnectionPool, AdvancedPoolConfig, ConnectionPoolMetrics, PoolTier,
};
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerState};
pub use health_monitor::{
    ConnectionHealth, ConnectionHealthMonitor, HealthCheck, HealthStatus,
};
pub use load_balancer::{ConnectionLoadBalancer, LoadBalancingStrategy, NodeHealth};
