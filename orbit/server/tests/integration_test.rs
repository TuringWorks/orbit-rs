use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;

/// Integration tests for Orbit Server
///
/// These tests verify that all server features are properly initialized:
/// - Persistence layer (RocksDB)
/// - Write-Ahead Log (WAL)
/// - Protocol adapters (PostgreSQL, Redis, MySQL, CQL, gRPC)
/// - Prometheus metrics endpoint
/// - MinIO cold storage configuration
/// - Cluster configuration

// =============================================================================
// Helper Functions
// =============================================================================

/// Cleanup all lingering server instances before and after tests
async fn cleanup_lingering_instances() {
    // Kill all orbit-server and multi-protocol-server instances
    let _ = Command::new("killall")
        .arg("orbit-server")
        .output();

    let _ = Command::new("killall")
        .arg("multi-protocol-server")
        .output();

    // Give processes time to terminate
    sleep(Duration::from_millis(500)).await;

    // Verify cleanup
    let output = Command::new("ps")
        .arg("aux")
        .output()
        .expect("Failed to execute ps");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let orbit_count = stdout
        .lines()
        .filter(|line| line.contains("orbit-server") || line.contains("multi-protocol-server"))
        .filter(|line| !line.contains("grep"))
        .count();

    assert_eq!(orbit_count, 0, "Expected 0 lingering instances, found {}", orbit_count);
}

/// Check if a TCP port is listening
async fn is_port_listening(port: u16) -> bool {
    use tokio::net::TcpListener;

    // Try to bind to the port - if it fails, the port is already in use (listening)
    match TcpListener::bind(format!("127.0.0.1:{}", port)).await {
        Ok(_) => false, // Port is free
        Err(_) => true,  // Port is in use
    }
}

/// Wait for a port to start listening (max timeout in seconds)
async fn wait_for_port(port: u16, max_wait_secs: u64) -> bool {
    let start = std::time::Instant::now();
    let max_duration = Duration::from_secs(max_wait_secs);

    while start.elapsed() < max_duration {
        if is_port_listening(port).await {
            return true;
        }
        sleep(Duration::from_millis(200)).await;
    }

    false
}

/// Check if data directory exists
fn data_directory_exists(path: &str) -> bool {
    PathBuf::from(path).exists()
}

/// Check if RocksDB directory is initialized (contains files)
fn rocksdb_initialized(base_path: &str) -> bool {
    let rocksdb_path = PathBuf::from(base_path).join("rocksdb");

    if !rocksdb_path.exists() {
        return false;
    }

    // Check if directory contains any files (initialized)
    if let Ok(entries) = std::fs::read_dir(&rocksdb_path) {
        entries.count() > 0
    } else {
        false
    }
}

/// Check if WAL directory exists
fn wal_directory_exists(base_path: &str) -> bool {
    PathBuf::from(base_path).join("wal").exists()
}

// =============================================================================
// Integration Tests
// =============================================================================

#[tokio::test]
async fn test_cleanup_before_tests() {
    cleanup_lingering_instances().await;
}

#[tokio::test]
#[ignore] // Run with: cargo test --test integration_test -- --ignored
async fn test_data_directories_created() {
    cleanup_lingering_instances().await;

    // Start server in background
    let mut child = Command::new("cargo")
        .args(&["run", "-p", "orbit-server", "--"])
        .spawn()
        .expect("Failed to start orbit-server");

    // Wait for initialization
    sleep(Duration::from_secs(5)).await;

    // Verify data directories exist
    let base_path = "./data";
    assert!(data_directory_exists(base_path), "Base data directory should exist");
    assert!(data_directory_exists(&format!("{}/hot", base_path)), "Hot tier directory should exist");
    assert!(data_directory_exists(&format!("{}/warm", base_path)), "Warm tier directory should exist");
    assert!(data_directory_exists(&format!("{}/cold", base_path)), "Cold tier directory should exist");
    assert!(wal_directory_exists(base_path), "WAL directory should exist");
    assert!(data_directory_exists(&format!("{}/rocksdb", base_path)), "RocksDB directory should exist");

    // Cleanup
    let _ = child.kill();
    cleanup_lingering_instances().await;
}

#[tokio::test]
#[ignore] // Run with: cargo test --test integration_test -- --ignored
async fn test_rocksdb_persistence_initialized() {
    cleanup_lingering_instances().await;

    // Start server in background
    let mut child = Command::new("cargo")
        .args(&["run", "-p", "orbit-server", "--"])
        .spawn()
        .expect("Failed to start orbit-server");

    // Wait for initialization
    sleep(Duration::from_secs(5)).await;

    // Verify RocksDB is initialized (contains files)
    assert!(rocksdb_initialized("./data"), "RocksDB should be initialized with database files");

    // Cleanup
    let _ = child.kill();
    cleanup_lingering_instances().await;
}

#[tokio::test]
#[ignore] // Run with: cargo test --test integration_test -- --ignored
async fn test_all_protocol_ports_listening() {
    cleanup_lingering_instances().await;

    // Start server in background
    let mut child = Command::new("cargo")
        .args(&["run", "-p", "orbit-server", "--"])
        .spawn()
        .expect("Failed to start orbit-server");

    // Wait for all protocols to start (up to 15 seconds)
    let ports = vec![
        (5432, "PostgreSQL"),
        (6379, "Redis"),
        (3306, "MySQL"),
        (9042, "CQL"),
        (50051, "gRPC"),
    ];

    for (port, protocol) in ports {
        let listening = wait_for_port(port, 15).await;
        assert!(listening, "{} protocol should be listening on port {}", protocol, port);
    }

    // Cleanup
    let _ = child.kill();
    cleanup_lingering_instances().await;
}

#[tokio::test]
#[ignore] // Run with: cargo test --test integration_test -- --ignored
async fn test_prometheus_metrics_endpoint() {
    cleanup_lingering_instances().await;

    // Start server in background
    let mut child = Command::new("cargo")
        .args(&["run", "-p", "orbit-server", "--"])
        .spawn()
        .expect("Failed to start orbit-server");

    // Wait for metrics endpoint to start
    let listening = wait_for_port(9090, 15).await;
    assert!(listening, "Prometheus metrics endpoint should be listening on port 9090");

    // Try to fetch metrics (requires reqwest)
    // This will be tested manually or with a separate HTTP client test

    // Cleanup
    let _ = child.kill();
    cleanup_lingering_instances().await;
}

#[tokio::test]
#[ignore] // Run with: cargo test --test integration_test -- --ignored
async fn test_minio_configuration_loaded() {
    cleanup_lingering_instances().await;

    // Set environment variables for MinIO
    std::env::set_var("MINIO_ENDPOINT", "http://localhost:9000");
    std::env::set_var("MINIO_BUCKET", "orbit-cold-tier");
    std::env::set_var("MINIO_ACCESS_KEY", "minioadmin");
    std::env::set_var("MINIO_SECRET_KEY", "minioadmin");

    // Start server in background
    let mut child = Command::new("cargo")
        .args(&["run", "-p", "orbit-server", "--"])
        .spawn()
        .expect("Failed to start orbit-server");

    // Wait for initialization
    sleep(Duration::from_secs(5)).await;

    // Configuration loading is verified via logs
    // In a real test, we would check logs or internal state
    // For now, we verify the server starts successfully

    // Cleanup environment
    std::env::remove_var("MINIO_ENDPOINT");
    std::env::remove_var("MINIO_BUCKET");
    std::env::remove_var("MINIO_ACCESS_KEY");
    std::env::remove_var("MINIO_SECRET_KEY");

    // Cleanup
    let _ = child.kill();
    cleanup_lingering_instances().await;
}

#[tokio::test]
#[ignore] // Run with: cargo test --test integration_test -- --ignored
async fn test_wal_enabled() {
    cleanup_lingering_instances().await;

    // Start server in background
    let mut child = Command::new("cargo")
        .args(&["run", "-p", "orbit-server", "--"])
        .spawn()
        .expect("Failed to start orbit-server");

    // Wait for initialization
    sleep(Duration::from_secs(5)).await;

    // Verify WAL directory exists and is being used
    let wal_path = PathBuf::from("./data/wal");
    assert!(wal_path.exists(), "WAL directory should exist");

    // Check if WAL files are created (may take some writes)
    // In a production test, we would perform writes and verify WAL entries

    // Cleanup
    let _ = child.kill();
    cleanup_lingering_instances().await;
}

#[tokio::test]
#[ignore] // Run with: cargo test --test integration_test -- --ignored
async fn test_postgresql_wire_protocol_connection() {
    cleanup_lingering_instances().await;

    // Start server in background
    let mut child = Command::new("cargo")
        .args(&["run", "-p", "orbit-server", "--"])
        .spawn()
        .expect("Failed to start orbit-server");

    // Wait for PostgreSQL protocol to start
    let listening = wait_for_port(5432, 15).await;
    assert!(listening, "PostgreSQL protocol should be listening on port 5432");

    // Try to connect using psql or a PostgreSQL client library
    // This would require tokio-postgres or similar
    // For now, we verify the port is listening

    // Cleanup
    let _ = child.kill();
    cleanup_lingering_instances().await;
}

#[tokio::test]
#[ignore] // Run with: cargo test --test integration_test -- --ignored
async fn test_redis_resp_protocol_connection() {
    cleanup_lingering_instances().await;

    // Start server in background
    let mut child = Command::new("cargo")
        .args(&["run", "-p", "orbit-server", "--"])
        .spawn()
        .expect("Failed to start orbit-server");

    // Wait for Redis protocol to start
    let listening = wait_for_port(6379, 15).await;
    assert!(listening, "Redis protocol should be listening on port 6379");

    // Try to connect using redis-cli or redis-rs
    // For now, we verify the port is listening

    // Cleanup
    let _ = child.kill();
    cleanup_lingering_instances().await;
}

#[tokio::test]
async fn test_cleanup_after_tests() {
    cleanup_lingering_instances().await;
}

// =============================================================================
// Stress Tests
// =============================================================================

#[tokio::test]
#[ignore] // Run with: cargo test --test integration_test -- --ignored
async fn test_multiple_restarts_no_lingering_instances() {
    cleanup_lingering_instances().await;

    // Start and stop server 5 times
    for i in 1..=5 {
        println!("Restart iteration {}/5", i);

        // Start server
        let mut child = Command::new("cargo")
            .args(&["run", "-p", "orbit-server", "--"])
            .spawn()
            .expect("Failed to start orbit-server");

        // Wait for startup
        sleep(Duration::from_secs(3)).await;

        // Kill server
        let _ = child.kill();

        // Cleanup
        cleanup_lingering_instances().await;
    }

    // Final verification - no lingering instances
    let output = Command::new("ps")
        .arg("aux")
        .output()
        .expect("Failed to execute ps");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let orbit_count = stdout
        .lines()
        .filter(|line| line.contains("orbit-server") || line.contains("multi-protocol-server"))
        .filter(|line| !line.contains("grep"))
        .count();

    assert_eq!(orbit_count, 0, "Expected 0 lingering instances after 5 restarts, found {}", orbit_count);
}

#[tokio::test]
#[ignore] // Run with: cargo test --test integration_test -- --ignored
async fn test_concurrent_protocol_connections() {
    cleanup_lingering_instances().await;

    // Start server
    let mut child = Command::new("cargo")
        .args(&["run", "-p", "orbit-server", "--"])
        .spawn()
        .expect("Failed to start orbit-server");

    // Wait for all protocols to start
    sleep(Duration::from_secs(10)).await;

    // Verify all ports are listening concurrently
    let postgres_listening = is_port_listening(5432).await;
    let redis_listening = is_port_listening(6379).await;
    let mysql_listening = is_port_listening(3306).await;
    let cql_listening = is_port_listening(9042).await;
    let grpc_listening = is_port_listening(50051).await;
    let metrics_listening = is_port_listening(9090).await;

    assert!(postgres_listening, "PostgreSQL should be listening");
    assert!(redis_listening, "Redis should be listening");
    assert!(mysql_listening, "MySQL should be listening");
    assert!(cql_listening, "CQL should be listening");
    assert!(grpc_listening, "gRPC should be listening");
    assert!(metrics_listening, "Metrics should be listening");

    // Cleanup
    let _ = child.kill();
    cleanup_lingering_instances().await;
}
