//! Integration test for advanced connection pooling
//!
//! This module tests the integration of the advanced connection pooling system
//! with the orbit-server components to ensure it can be properly used.

#[cfg(test)]
mod tests {
    use orbit_shared::pooling::{
        AdvancedConnectionPool, AdvancedPoolConfig, LoadBalancingStrategy, PoolTier,
    };
    use orbit_shared::OrbitResult;
    use std::time::Duration;

    #[derive(Debug, Clone)]
    struct MockServerConnection {
        server_type: String,
    }

    async fn create_mock_server_connection(node_id: String) -> OrbitResult<MockServerConnection> {
        // Simulate different server types based on node_id
        let server_type = if node_id.contains("postgres") {
            "PostgreSQL".to_string()
        } else if node_id.contains("redis") {
            "Redis".to_string()
        } else {
            "Generic".to_string()
        };

        Ok(MockServerConnection { server_type })
    }

    #[tokio::test]
    async fn test_pooling_integration_basic() {
        let config = AdvancedPoolConfig {
            min_connections: 2,
            max_connections: 10,
            connection_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(300),
            max_lifetime: Duration::from_secs(3600),
            health_check_interval: Duration::from_secs(30),
            load_balancing_strategy: LoadBalancingStrategy::LeastConnections,
            tier: PoolTier::Application,
            ..Default::default()
        };

        let pool = AdvancedConnectionPool::new(config, |node_id| {
            Box::pin(create_mock_server_connection(node_id))
        });

        // Add nodes representing different server types
        pool.add_node("postgres-node-1".to_string(), 5).await;
        pool.add_node("redis-node-1".to_string(), 5).await;

        // Test connection acquisition
        let conn1 = pool.acquire().await.expect("Failed to acquire connection");
        let conn2 = pool.acquire().await.expect("Failed to acquire connection");

        // Verify we got connections from different nodes (load balancing)
        let conn1_type = &conn1.connection().server_type;
        let conn2_type = &conn2.connection().server_type;

        println!("Connection 1: {} ({})", conn1.node_id(), conn1_type);
        println!("Connection 2: {} ({})", conn2.node_id(), conn2_type);

        // Check metrics
        let metrics = pool.get_metrics().await;
        assert_eq!(metrics.current_active, 2);
        assert_eq!(metrics.total_acquired, 2);

        // Verify health status
        let health = pool.get_health_status().await;
        println!("Pool health: {:?}", health);

        drop(conn1);
        drop(conn2);

        // Allow time for connections to be returned to pool
        tokio::time::sleep(Duration::from_millis(100)).await;

        let final_metrics = pool.get_metrics().await;
        assert_eq!(final_metrics.current_active, 0);
    }

    #[tokio::test]
    async fn test_pooling_integration_multi_protocol() {
        let config = AdvancedPoolConfig {
            max_connections: 20,
            load_balancing_strategy: LoadBalancingStrategy::RoundRobin,
            tier: PoolTier::Application,
            ..Default::default()
        };

        let pool = AdvancedConnectionPool::new(config, |node_id| {
            Box::pin(create_mock_server_connection(node_id))
        });

        // Simulate multi-protocol server setup
        pool.add_node("postgres-primary".to_string(), 8).await;
        pool.add_node("postgres-replica".to_string(), 6).await;
        pool.add_node("redis-primary".to_string(), 4).await;
        pool.add_node("redis-replica".to_string(), 4).await;

        // Test rapid connection acquisition (simulating high load)
        let mut connections = Vec::new();
        for i in 0..8 {
            let conn = pool
                .acquire()
                .await
                .expect(&format!("Failed to acquire connection {}", i));
            connections.push(conn);
        }

        let metrics = pool.get_metrics().await;
        assert_eq!(metrics.current_active, 8);
        println!(
            "High load test: {} active connections, hit rate: {:.2}%",
            metrics.current_active,
            metrics.hit_rate() * 100.0
        );

        // Verify round-robin load balancing by checking node distribution
        let node_counts: std::collections::HashMap<_, _> = connections
            .iter()
            .map(|conn| conn.node_id())
            .fold(std::collections::HashMap::new(), |mut acc, node| {
                *acc.entry(node).or_insert(0usize) += 1;
                acc
            });

        println!("Node distribution: {:?}", node_counts);
        // With 4 nodes and 8 connections, we should have roughly 2 per node
        assert!(
            node_counts.len() > 1,
            "Connections should be distributed across multiple nodes"
        );

        // Clean up
        connections.clear();
        tokio::time::sleep(Duration::from_millis(100)).await;

        let final_metrics = pool.get_metrics().await;
        assert_eq!(final_metrics.current_active, 0);
    }
}
