//! Orbit Server - Distributed Actor System Runtime
//!
//! The main Orbit server binary that provides:
//! - gRPC API for actor management and messaging
//! - PostgreSQL Wire Protocol (port 5432)
//! - Redis RESP Protocol (port 6379)
//! - MySQL Wire Protocol (port 3306)
//! - CQL Protocol (port 9042)
//! - Cluster membership and node discovery
//! - Load balancing and routing
//! - Health monitoring and metrics
//! - RocksDB persistence with WAL
//! - Tiered storage (hot/warm/cold)
//! - MinIO/S3 cold tier integration
//!
//! Usage:
//!   orbit-server [OPTIONS]
//!
//! Configuration:
//!   Uses TOML configuration files with environment variable overrides.
//!   Default config locations:
//!   - ./config/orbit-server.toml
//!   - /app/config/orbit-server.toml (in Docker)
//!   - /etc/orbit/orbit-server.toml (system-wide)

use clap::Parser;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{info, warn, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use orbit_server::OrbitServerBuilder;
use orbit_server::protocols::{RespServer, PostgresServer, MySqlServer, CqlServer};
use orbit_server::protocols::cql::CqlConfig;
use orbit_server::protocols::mysql::MySqlConfig;
use orbit_server::protocols::postgres_wire::{QueryEngine, RocksDbTableStorage};
use orbit_server::protocols::common::storage::tiered::TieredTableStorage;
use orbit_server::protocols::cypher::{CypherServer, CypherGraphStorage};
use orbit_server::protocols::aql::{AqlServer, AqlStorage};
use orbit_server::protocols::common::storage::TableStorage;
use orbit_server::protocols::postgres_wire::sql::execution::hybrid::HybridStorageConfig;

/// Orbit Server - Distributed Actor System Runtime
#[derive(Parser)]
#[command(
    name = "orbit-server",
    version = env!("CARGO_PKG_VERSION"),
    about = "Orbit Server - Distributed Actor System Runtime",
    long_about = "The Orbit server provides a runtime for distributed actor systems with \
clustering, load balancing, and fault tolerance capabilities."
)]
struct Args {
    /// Configuration file path
    #[arg(
        short,
        long,
        value_name = "FILE",
        help = "Path to configuration file",
        default_value = "/app/config/orbit-server.toml"
    )]
    config: PathBuf,

    /// Server bind address
    #[arg(
        short,
        long,
        value_name = "ADDR",
        help = "Server bind address",
        default_value = "0.0.0.0"
    )]
    bind: String,

    /// gRPC port
    #[arg(
        short = 'p',
        long,
        value_name = "PORT",
        help = "gRPC server port",
        default_value = "50051"
    )]
    grpc_port: u16,

    /// HTTP/REST API port
    #[arg(
        short = 'H',
        long,
        value_name = "PORT",
        help = "HTTP/REST API port",
        default_value = "8080"
    )]
    http_port: u16,

    /// Metrics port
    #[arg(
        short = 'm',
        long,
        value_name = "PORT",
        help = "Prometheus metrics port",
        default_value = "9090"
    )]
    metrics_port: u16,

    /// Node ID (auto-generated if not provided)
    #[arg(short = 'n', long, value_name = "ID", help = "Unique node identifier")]
    node_id: Option<String>,

    /// Cluster seed nodes
    #[arg(
        short = 's',
        long,
        value_name = "URLS",
        help = "Comma-separated list of seed node URLs for cluster joining",
        value_delimiter = ','
    )]
    seed_nodes: Vec<String>,

    /// Enable development mode
    #[arg(
        short = 'd',
        long,
        help = "Enable development mode (verbose logging, relaxed security)"
    )]
    dev_mode: bool,

    /// Log level
    #[arg(
        short = 'l',
        long,
        value_name = "LEVEL",
        help = "Log level (trace, debug, info, warn, error)",
        default_value = "info"
    )]
    log_level: String,

    /// PostgreSQL port
    #[arg(
        long,
        value_name = "PORT",
        help = "PostgreSQL server port",
        default_value = "5432"
    )]
    postgres_port: u16,

    /// Redis port
    #[arg(
        long,
        value_name = "PORT",
        help = "Redis server port",
        default_value = "6379"
    )]
    redis_port: u16,

    /// MySQL port
    #[arg(
        long,
        value_name = "PORT",
        help = "MySQL server port",
        default_value = "3306"
    )]
    mysql_port: u16,

    /// CQL port
    #[arg(
        long,
        value_name = "PORT",
        help = "CQL server port",
        default_value = "9042"
    )]
    cql_port: u16,

    /// Data directory
    #[arg(
        long,
        value_name = "DIR",
        help = "Data directory for persistence",
        default_value = "./data"
    )]
    data_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.dev_mode {
        "debug,orbit_server=trace,orbit_shared=debug,orbit_proto=debug,orbit_protocols=debug".to_string()
    } else {
        format!("{},orbit_server=info,orbit_shared=info,orbit_protocols=info", args.log_level)
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or(log_level),
        ))
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_thread_ids(args.dev_mode)
                .with_file(args.dev_mode)
                .with_line_number(args.dev_mode),
        )
        .init();

    // Print startup banner
    info!("=========================================");
    info!("    Orbit Server v{}", env!("CARGO_PKG_VERSION"));
    info!("    Multi-Protocol Distributed Database");
    info!("=========================================");

    if args.dev_mode {
        warn!("[Dev Mode] Development mode enabled - NOT FOR PRODUCTION");
    }

    // Load configuration from TOML file
    info!("[Configuration] Loading configuration from: {:?}", args.config);
    let mut toml_config = load_toml_config(&args).await?;

    // Override with command line arguments
    apply_cli_overrides(&mut toml_config, &args);

    info!("[Configuration] Configuration loaded successfully");

    // Initialize data directories
    info!("[Storage] Initializing storage directories...");
    initialize_data_directories(&args.data_dir).await?;
    info!("[Storage] Data directories created at: {:?}", args.data_dir);

    // Initialize RocksDB storage with WAL
    info!("[Persistence] Initializing RocksDB with Write-Ahead Logging...");
    let rocksdb_storage = initialize_rocksdb_storage(&args.data_dir, &toml_config).await?;
    info!("[Persistence] RocksDB initialized successfully");

    // Configure tiered storage (hot/warm/cold)
    info!("[Tiered Storage] Configuring tiered storage strategy...");
    let tiered_config = configure_tiered_storage(&toml_config);
    info!(
        "[Tiered Storage] Hot→Warm: {:?}, Warm→Cold: {:?}",
        tiered_config.hot_to_warm_threshold,
        tiered_config.warm_to_cold_threshold
    );

    // Initialize MinIO cold storage if configured
    let cold_storage_config = initialize_minio_cold_storage(&toml_config)?;
    if let Some(ref config) = cold_storage_config {
        info!("[Cold Storage] MinIO cold tier configured: {}", config.endpoint);
    }

    // Start Prometheus metrics exporter
    info!("[Metrics] Starting Prometheus metrics exporter on port {}...", args.metrics_port);
    if let Err(e) = start_prometheus_metrics_exporter(args.metrics_port).await {
        warn!("[Metrics] Failed to start metrics server: {}. Continuing without metrics endpoint.", e);
    } else {
        info!("[Metrics] Metrics endpoint available at http://{}:{}/metrics", args.bind, args.metrics_port);
    }

    // Log all enabled protocol servers
    log_protocol_configuration(&args);

    // Initialize cluster/Raft if seed nodes are provided
    if !args.seed_nodes.is_empty() {
        info!("[Cluster] Initializing cluster with seed nodes: {:?}", args.seed_nodes);
        initialize_cluster(&args).await?;
    } else {
        info!("[Cluster] Starting as standalone node (no seed nodes configured)");
    }

    // Build and start gRPC server
    let mut server = OrbitServerBuilder::new()
        .with_bind_address(&args.bind)
        .with_port(args.grpc_port)
        .build()
        .await?;

    info!("[gRPC] gRPC server initialized on {}:{}", args.bind, args.grpc_port);

    // Create independent tiered storage for each protocol with protocol-specific data directories
    let postgres_data_dir = args.data_dir.join("postgresql");
    let redis_data_dir = args.data_dir.join("redis");
    let mysql_data_dir = args.data_dir.join("mysql");
    let cql_data_dir = args.data_dir.join("cql");

    let postgres_storage = Arc::new(TieredTableStorage::with_data_dir(
        postgres_data_dir,
        tiered_config.clone(),
    ));
    let redis_storage = Arc::new(TieredTableStorage::with_data_dir(
        redis_data_dir,
        tiered_config.clone(),
    ));
    let mysql_storage = Arc::new(TieredTableStorage::with_data_dir(
        mysql_data_dir,
        tiered_config.clone(),
    ));
    let cql_storage = Arc::new(TieredTableStorage::with_data_dir(
        cql_data_dir,
        tiered_config,
    ));

    info!("[Protocol Storage] Independent tiered storage created for all protocols");

    // Initialize storage instances (ensure .initialize() is called)
    postgres_storage.initialize().await?;
    redis_storage.initialize().await?;
    mysql_storage.initialize().await?;
    cql_storage.initialize().await?;

    info!("[Protocol Storage] All storage instances initialized");

    // Start protocol adapters
    let mut protocol_handles: Vec<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>> = Vec::new();

    // Start PostgreSQL Wire Protocol server (port 5432)
    if toml_config.protocols.postgresql.as_ref().map_or(true, |c| c.enabled) {
        let postgres_handle = start_postgresql_server(
            &args,
            postgres_storage.clone(),
            rocksdb_storage.clone(),
        ).await?;
        protocol_handles.push(postgres_handle);
        info!("[PostgreSQL] PostgreSQL wire protocol server started on port {}", args.postgres_port);
    }

    // Start Redis RESP Protocol server (port 6379)
    if toml_config.protocols.redis.as_ref().map_or(true, |c| c.enabled) {
        let redis_handle = start_redis_server(
            &args,
            redis_storage.clone(),
        ).await?;
        protocol_handles.push(redis_handle);
        info!("[Redis] Redis RESP protocol server started on port {}", args.redis_port);
    }

    // Start MySQL protocol adapter (port 3306)
    if toml_config.protocols.mysql.as_ref().map_or(true, |c| c.enabled) {
        let mysql_config = MySqlConfig {
            listen_addr: format!("{}:{}", args.bind, args.mysql_port).parse()?,
            max_connections: 1000,
            authentication_enabled: false,
            server_version: format!("Orbit-DB {} (MySQL-compatible)", env!("CARGO_PKG_VERSION")),
            username: None,
            password: None,
        };

        let mysql_server = MySqlServer::new_with_storage(mysql_config, mysql_storage).await?;
        let mysql_handle = tokio::spawn(async move {
            mysql_server.start().await.map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
        });
        protocol_handles.push(mysql_handle);
        info!("[MySQL] MySQL protocol adapter started on port {}", args.mysql_port);
    }

    // Start CQL protocol adapter (port 9042)
    if toml_config.protocols.cql.as_ref().map_or(true, |c| c.enabled) {
        let cql_config = CqlConfig {
            listen_addr: format!("{}:{}", args.bind, args.cql_port).parse()?,
            max_connections: 1000,
            authentication_enabled: false,
            protocol_version: 4,
            username: None,
            password: None,
        };

        let cql_server = CqlServer::new_with_storage(cql_config, cql_storage).await?;
        let cql_handle = tokio::spawn(async move {
            cql_server.start().await.map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
        });
        protocol_handles.push(cql_handle);
        info!("[CQL] CQL protocol adapter started on port {}", args.cql_port);
    }

    // Start Cypher/Neo4j protocol adapter (port 7687)
    let cypher_data_dir = args.data_dir.join("cypher");
    let cypher_storage = Arc::new(CypherGraphStorage::new(cypher_data_dir));
    cypher_storage.initialize().await?;
    
    let cypher_bind_addr = format!("{}:7687", args.bind);
    let cypher_server = CypherServer::new_with_storage(cypher_bind_addr, cypher_storage);
    let cypher_handle = tokio::spawn(async move {
        cypher_server.run().await.map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
    });
    protocol_handles.push(cypher_handle);
    info!("[Cypher] Cypher/Neo4j protocol adapter started on port 7687");

    // Start AQL/ArangoDB protocol adapter (port 8529)
    let aql_data_dir = args.data_dir.join("aql");
    let aql_storage = Arc::new(AqlStorage::new(aql_data_dir));
    aql_storage.initialize().await?;
    
    let aql_bind_addr = format!("{}:8529", args.bind);
    let aql_server = AqlServer::new_with_storage(aql_bind_addr, aql_storage);
    let aql_handle = tokio::spawn(async move {
        aql_server.run().await.map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
    });
    protocol_handles.push(aql_handle);
    info!("[AQL] AQL/ArangoDB protocol adapter started on port 8529");

    info!("=========================================");
    info!("    Orbit Server Ready!");
    info!("=========================================");
    info!("  gRPC:       {}:{}", args.bind, args.grpc_port);
    info!("  PostgreSQL: {}:{}", args.bind, args.postgres_port);
    info!("  Redis:      {}:{}", args.bind, args.redis_port);
    info!("  MySQL:      {}:{}", args.bind, args.mysql_port);
    info!("  CQL:        {}:{}", args.bind, args.cql_port);
    info!("  Cypher:     {}:7687", args.bind);
    info!("  AQL:        {}:8529", args.bind);
    info!("  Metrics:    {}:{}/metrics", args.bind, args.metrics_port);
    
    // Initialize MCP server if enabled
    if toml_config.protocols.mcp.as_ref().map_or(false, |c| c.enabled) {
        let mcp_handle = start_mcp_server(
            &args,
            postgres_storage.clone(),
            rocksdb_storage.clone(),
        ).await?;
        protocol_handles.push(mcp_handle);
        info!("[MCP] Model Context Protocol server initialized");
    }
    
    info!("=========================================");

    // Start the gRPC server in a separate task
    let grpc_handle = tokio::spawn(async move {
        server.start().await.map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
    });

    // Wait for any server to exit (all run indefinitely until interrupted)
    tokio::select! {
        result = grpc_handle => {
            match result {
                Ok(Ok(())) => info!("[gRPC] gRPC server exited successfully"),
                Ok(Err(e)) => error!("[gRPC] gRPC server error: {}", e),
                Err(e) => error!("[gRPC] gRPC server panic: {}", e),
            }
        }
        result = async {
            if protocol_handles.is_empty() {
                std::future::pending::<()>().await;
                Ok(Ok(()))
            } else {
                futures::future::select_all(protocol_handles).await.0
            }
        } => {
            match result {
                Ok(Ok(())) => info!("[Protocol] Protocol adapter exited successfully"),
                Ok(Err(e)) => error!("[Protocol] Protocol adapter error: {}", e),
                Err(e) => error!("[Protocol] Protocol adapter panic: {}", e),
            }
        }
    }

    info!("[Shutdown] Orbit Server shutdown complete");
    Ok(())
}

/// Load configuration from TOML file
async fn load_toml_config(args: &Args) -> Result<orbit_server::config::OrbitServerConfig, Box<dyn Error>> {
    use orbit_server::config::OrbitServerConfig;

    if args.config.exists() {
        info!("[Config] Loading configuration from: {:?}", args.config);
        let config = OrbitServerConfig::load_from_file(&args.config).await?;
        info!("[Config] Configuration loaded and validated");
        Ok(config)
    } else {
        warn!("[Config] Config file not found: {:?}. Using defaults.", args.config);
        Ok(OrbitServerConfig::default())
    }
}

/// Apply command line argument overrides to configuration
fn apply_cli_overrides(config: &mut orbit_server::config::OrbitServerConfig, args: &Args) {
    // Override server configuration
    config.server.bind_address = args.bind.clone();

    if let Some(ref node_id) = args.node_id {
        config.server.node_id = Some(node_id.clone());
    }

    // Override protocol ports
    if let Some(ref mut grpc) = config.protocols.grpc {
        grpc.port = args.grpc_port;
    }

    if let Some(ref mut postgres) = config.protocols.postgresql {
        postgres.port = args.postgres_port;
    }

    if let Some(ref mut redis) = config.protocols.redis {
        redis.port = args.redis_port;
    }

    if let Some(ref mut mysql) = config.protocols.mysql {
        mysql.port = args.mysql_port;
    }

    if let Some(ref mut cql) = config.protocols.cql {
        cql.port = args.cql_port;
    }

    // Override metrics port
    config.monitoring.metrics.port = args.metrics_port;

    // Override data directory
    config.server.data_dir = args.data_dir.clone();
    if let Some(ref mut persistence) = config.persistence {
        persistence.data_dir = args.data_dir.clone();
    }

    info!("[CLI Overrides] Configuration overridden with command-line arguments");
}

/// Initialize data directories
async fn initialize_data_directories(data_dir: &PathBuf) -> Result<(), Box<dyn Error>> {
    // Create main data directory
    tokio::fs::create_dir_all(data_dir).await?;

    // Create subdirectories for different storage tiers
    let hot_dir = data_dir.join("hot");
    let warm_dir = data_dir.join("warm");
    let cold_dir = data_dir.join("cold");
    let wal_dir = data_dir.join("wal");
    let rocksdb_dir = data_dir.join("rocksdb");
    let redis_dir = data_dir.join("redis");

    // Create protocol-specific persistence directories
    let postgresql_dir = data_dir.join("postgresql");
    let mysql_dir = data_dir.join("mysql");
    let cql_dir = data_dir.join("cql");
    let cypher_dir = data_dir.join("cypher");
    let aql_dir = data_dir.join("aql");
    let graphrag_dir = data_dir.join("graphrag");

    tokio::fs::create_dir_all(&hot_dir).await?;
    tokio::fs::create_dir_all(&warm_dir).await?;
    tokio::fs::create_dir_all(&cold_dir).await?;
    tokio::fs::create_dir_all(&wal_dir).await?;
    tokio::fs::create_dir_all(&rocksdb_dir).await?;
    tokio::fs::create_dir_all(&redis_dir).await?;
    tokio::fs::create_dir_all(&postgresql_dir).await?;
    tokio::fs::create_dir_all(&mysql_dir).await?;
    tokio::fs::create_dir_all(&cql_dir).await?;
    tokio::fs::create_dir_all(&cypher_dir).await?;
    tokio::fs::create_dir_all(&aql_dir).await?;
    tokio::fs::create_dir_all(&graphrag_dir).await?;

    info!("[Storage Dirs] Created: hot, warm, cold, wal, rocksdb, redis, postgresql, mysql, cql, cypher, aql, graphrag");

    Ok(())
}

/// Initialize RocksDB storage with WAL enabled
async fn initialize_rocksdb_storage(
    data_dir: &PathBuf,
    config: &orbit_server::config::OrbitServerConfig,
) -> Result<Arc<RocksDbTableStorage>, Box<dyn Error>> {
    let rocksdb_path = data_dir.join("rocksdb");

    // Extract RocksDB configuration from TOML
    let enable_wal = config
        .persistence
        .as_ref()
        .and_then(|p| p.rocksdb.as_ref())
        .map(|r| r.enable_wal)
        .unwrap_or(true);

    info!("[RocksDB] Opening RocksDB at: {:?} (WAL: {})", rocksdb_path, enable_wal);

    let storage = RocksDbTableStorage::new(rocksdb_path.to_str().unwrap())?;

    info!("[RocksDB] Write-Ahead Logging: {}", if enable_wal { "ENABLED" } else { "DISABLED" });

    Ok(Arc::new(storage))
}

/// Configure tiered storage from TOML config
fn configure_tiered_storage(
    config: &orbit_server::config::OrbitServerConfig,
) -> HybridStorageConfig {
    use std::time::Duration;

    let storage_config = config.storage.as_ref();
    let tiered_config = storage_config.and_then(|s| Some(&s.tiered));

    let hot_to_warm_hours = tiered_config
        .map(|t| t.hot_to_warm_threshold_hours)
        .unwrap_or(48);

    let warm_to_cold_days = tiered_config
        .map(|t| t.warm_to_cold_threshold_days)
        .unwrap_or(30);

    let auto_tiering = tiered_config
        .map(|t| t.auto_tiering)
        .unwrap_or(true);

    let background_migration = tiered_config
        .map(|t| t.background_migration)
        .unwrap_or(true);

    HybridStorageConfig {
        hot_to_warm_threshold: Duration::from_secs(hot_to_warm_hours * 60 * 60),
        warm_to_cold_threshold: Duration::from_secs(warm_to_cold_days * 24 * 60 * 60),
        auto_tiering,
        background_migration,
    }
}

/// Configuration for MinIO cold storage
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct MinioConfig {
    endpoint: String,
    bucket: String,
    access_key: String,
    secret_key: String,
    use_ssl: bool,
}

/// Initialize MinIO cold storage from environment variables
fn initialize_minio_cold_storage(
    config: &orbit_server::config::OrbitServerConfig,
) -> Result<Option<MinioConfig>, Box<dyn Error>> {
    // Try to get from TOML config first
    if let Some(storage_config) = &config.storage {
        if let Some(cold_tier) = &storage_config.cold_tier {
            if cold_tier.enabled && cold_tier.backend == "minio" {
                if let Some(minio) = &cold_tier.minio {
                    return Ok(Some(MinioConfig {
                        endpoint: minio.endpoint.clone(),
                        bucket: minio.bucket.clone(),
                        access_key: minio.access_key.clone(),
                        secret_key: minio.secret_key.clone(),
                        use_ssl: minio.use_ssl,
                    }));
                }
            }
        }
    }

    // Fall back to environment variables
    if let (Ok(endpoint), Ok(access_key), Ok(secret_key), Ok(bucket)) = (
        std::env::var("MINIO_ENDPOINT"),
        std::env::var("MINIO_ACCESS_KEY"),
        std::env::var("MINIO_SECRET_KEY"),
        std::env::var("MINIO_BUCKET"),
    ) {
        info!("[MinIO] Using MinIO configuration from environment variables");
        Ok(Some(MinioConfig {
            endpoint,
            bucket,
            access_key,
            secret_key,
            use_ssl: std::env::var("MINIO_USE_SSL").unwrap_or_else(|_| "true".to_string()) == "true",
        }))
    } else {
        Ok(None)
    }
}

/// Start Prometheus metrics exporter
async fn start_prometheus_metrics_exporter(port: u16) -> Result<(), Box<dyn Error>> {
    use metrics_exporter_prometheus::PrometheusBuilder;

    let builder = PrometheusBuilder::new();
    let handle = builder
        .install_recorder()
        .map_err(|e| format!("Failed to install Prometheus recorder: {}", e))?;

    // Start HTTP server for metrics endpoint
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    // Bind the listener before spawning to catch port binding errors early
    use tokio::net::TcpListener;
    let listener = TcpListener::bind(addr).await
        .map_err(|e| format!("Failed to bind metrics server to {}: {}. Port may already be in use.", addr, e))?;

    tokio::spawn(async move {
        use hyper::server::conn::http1;
        use hyper::service::service_fn;
        use hyper::{Request, Response};
        use http_body_util::Full;
        use hyper::body::Bytes;
        use hyper_util::rt::TokioIo;

        loop {
            let (stream, _) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    error!("[Metrics] Accept error: {}", e);
                    continue;
                }
            };

            let handle_clone = handle.clone();

            tokio::spawn(async move {
                let io = TokioIo::new(stream);

                let service = service_fn(move |_req: Request<hyper::body::Incoming>| {
                    let handle = handle_clone.clone();
                    async move {
                        let metrics = handle.render();
                        Ok::<_, std::convert::Infallible>(Response::new(Full::new(Bytes::from(metrics))))
                    }
                });

                if let Err(e) = http1::Builder::new()
                    .serve_connection(io, service)
                    .await
                {
                    error!("[Metrics] Connection error: {}", e);
                }
            });
        }
    });

    Ok(())
}

/// Log protocol configuration
fn log_protocol_configuration(args: &Args) {
    info!("[Protocols] Enabled protocol servers:");
    info!("  - gRPC (Actor Management): {}:{}", args.bind, args.grpc_port);
    info!("  - PostgreSQL (Wire Protocol): {}:{}", args.bind, args.postgres_port);
    info!("  - Redis (RESP Protocol): {}:{}", args.bind, args.redis_port);
    info!("  - MySQL (Wire Protocol): {}:{}", args.bind, args.mysql_port);
    info!("  - CQL (Cassandra): {}:{}", args.bind, args.cql_port);
}

/// Initialize cluster with Raft consensus
async fn initialize_cluster(args: &Args) -> Result<(), Box<dyn Error>> {
    use orbit_shared::consensus::RaftConfig;
    use orbit_shared::cluster_manager::{EnhancedClusterManager, QuorumConfig};
    use orbit_shared::raft_transport::GrpcRaftTransport;
    use orbit_shared::NodeId;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    info!("[Cluster] Initializing cluster with Raft consensus");
    info!("[Cluster] Seed nodes: {:?}", args.seed_nodes);

    // Generate or use provided node ID
    let node_id = if let Some(id_str) = &args.node_id {
        NodeId::from_string(id_str)
    } else {
        NodeId::generate("default".to_string())
    };

    info!("[Cluster] Node ID: {}", node_id);

    // Parse seed nodes into node addresses
    // Seed nodes format: "http://host:port" or "node-id@http://host:port"
    let mut node_addresses: HashMap<NodeId, String> = HashMap::new();
    let mut cluster_nodes: Vec<NodeId> = Vec::new();

    // Add this node's address
    let this_node_address = format!("http://{}:{}", args.bind, args.grpc_port);
    node_addresses.insert(node_id.clone(), this_node_address.clone());
    cluster_nodes.push(node_id.clone());

    // Parse seed nodes
    for seed in &args.seed_nodes {
        // Try to parse as "node-id@http://host:port" or just "http://host:port"
        if let Some(at_pos) = seed.find('@') {
            let (seed_node_id_str, address) = seed.split_at(at_pos);
            let address = &address[1..]; // Remove '@'
            let seed_node_id = NodeId::from_string(seed_node_id_str);
            
            node_addresses.insert(seed_node_id.clone(), address.to_string());
            cluster_nodes.push(seed_node_id);
        } else {
            // Just an address, generate a node ID from it
            let seed_node_id = NodeId::from_string(&format!("node-{}", seed.replace("http://", "").replace(":", "-")));
            node_addresses.insert(seed_node_id.clone(), seed.clone());
            cluster_nodes.push(seed_node_id);
        }
    }

    info!("[Cluster] Cluster nodes: {:?}", cluster_nodes);
    info!("[Cluster] Node addresses: {:?}", node_addresses);

    // Create Raft configuration
    let raft_config = RaftConfig {
        election_timeout_min: Duration::from_millis(150),
        election_timeout_max: Duration::from_millis(300),
        heartbeat_interval: Duration::from_millis(50),
        max_entries_per_request: 100,
        log_compaction_threshold: 1000,
    };

    // Create Quorum configuration
    let quorum_config = QuorumConfig {
        min_quorum_size: if cluster_nodes.len() >= 3 { 3 } else { cluster_nodes.len() },
        max_failures: (cluster_nodes.len().saturating_sub(1)) / 2,
        quorum_timeout: Duration::from_secs(10),
        dynamic_quorum: false,
    };

    // Create gRPC Raft transport
    let transport = Arc::new(GrpcRaftTransport::new(
        node_id.clone(),
        node_addresses,
        Duration::from_secs(5),  // connection timeout
        Duration::from_secs(10), // request timeout
    ));

    // Create enhanced cluster manager with Raft consensus
    let cluster_manager = Arc::new(EnhancedClusterManager::new(
        node_id.clone(),
        cluster_nodes,
        quorum_config,
        raft_config,
    ));

    // Start the cluster manager (this starts Raft consensus)
    cluster_manager.start(transport).await.map_err(|e| {
        Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to start cluster manager: {}", e),
        ))
    })?;

    info!("[Cluster] Cluster manager started successfully with Raft consensus");
    info!("[Cluster] Node {} is ready for cluster operations", node_id);

    // Store cluster manager in a global context if needed
    // For now, we'll just log that it's initialized
    // TODO: Store cluster_manager in server context for use by other components

    Ok(())
}

/// Start PostgreSQL Wire Protocol server
async fn start_postgresql_server(
    args: &Args,
    _storage: Arc<TieredTableStorage>,
    rocksdb: Arc<RocksDbTableStorage>,
) -> Result<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>, Box<dyn Error>> {
    let bind_addr = format!("{}:{}", args.bind, args.postgres_port);

    // Create QueryEngine with RocksDB persistence
    let query_engine = QueryEngine::new_with_persistent_storage(rocksdb);

    // Create PostgreSQL server with query engine
    let postgres_server = PostgresServer::new_with_query_engine(bind_addr, query_engine);

    let handle = tokio::spawn(async move {
        postgres_server.run()
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
    });

    Ok(handle)
}

/// Start MCP (Model Context Protocol) server
async fn start_mcp_server(
    _args: &Args,
    storage: Arc<TieredTableStorage>,
    rocksdb: Arc<RocksDbTableStorage>,
) -> Result<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>, Box<dyn Error>> {
    use orbit_server::protocols::mcp::{McpConfig, McpCapabilities, McpServer};
    use orbit_server::protocols::mcp::integration::OrbitMcpIntegration;
    
    // Create QueryEngine with RocksDB persistence
    let query_engine = QueryEngine::new_with_persistent_storage(rocksdb.clone());
    let query_engine_arc = Arc::new(query_engine);
    
    // Create MCP integration layer with RocksDB storage (implements PersistentTableStorage)
    let integration = Arc::new(OrbitMcpIntegration::with_storage(
        query_engine_arc.clone(),
        rocksdb.clone(),
    ));
    
    // Create MCP server with storage-backed schema analyzer (uses TieredTableStorage for schema discovery)
    let mcp_config = McpConfig::default();
    let mcp_capabilities = McpCapabilities::default();
    let storage_for_schema = storage.clone();
    let mcp_server = McpServer::with_storage_and_integration(
        mcp_config,
        mcp_capabilities,
        integration.clone(),
        storage_for_schema,
    );
    
    // MCP is integrated into REST API, so we just initialize it here
    // The REST server will use it if available
    let _mcp_server_arc = Arc::new(mcp_server);
    
    // Start background schema discovery
    // This will periodically refresh the schema cache
    use orbit_server::protocols::mcp::schema_discovery::SchemaDiscoveryManager;
    use orbit_server::protocols::mcp::schema::SchemaAnalyzer;
    
    // Create schema analyzer with storage for discovery
    let schema_analyzer_for_discovery = Arc::new(SchemaAnalyzer::with_storage(storage.clone()));
    let schema_discovery = SchemaDiscoveryManager::new(
        schema_analyzer_for_discovery,
        integration,
        300, // Refresh every 5 minutes
    );
    if let Err(e) = schema_discovery.start_discovery().await {
        warn!("[MCP] Failed to start schema discovery: {}", e);
    } else {
        info!("[MCP] Schema discovery started");
    }
    
    // For now, MCP runs as part of REST API or can be accessed programmatically
    // Create a handle that keeps the server alive
    let handle = tokio::spawn(async move {
        // MCP server is ready and can be used by REST API handlers
        // Keep this task alive
        std::future::pending::<()>().await;
        Ok(())
    });
    
    Ok(handle)
}

/// Start Redis RESP Protocol server
async fn start_redis_server(
    args: &Args,
    _storage: Arc<TieredTableStorage>,
) -> Result<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>, Box<dyn Error>> {
    let bind_addr = format!("{}:{}", args.bind, args.redis_port);

    // Create RocksDB storage for Redis persistence
    let redis_data_path = args.data_dir.join("redis").join("rocksdb");
    let redis_provider: Option<Arc<dyn orbit_server::protocols::persistence::redis_data::RedisDataProvider>> = 
        match orbit_server::protocols::persistence::rocksdb_redis_provider::RocksDbRedisDataProvider::new(
            redis_data_path.to_str().unwrap(),
            orbit_server::protocols::persistence::redis_data::RedisDataConfig::default(),
        ) {
            Ok(provider) => {
                info!("[Redis] Using persistent RocksDB storage at: {}", redis_data_path.display());
                let provider_arc: Arc<dyn orbit_server::protocols::persistence::redis_data::RedisDataProvider> = Arc::new(provider);
                // Initialize the provider
                if let Err(e) = orbit_server::protocols::persistence::redis_data::RedisDataProvider::initialize(&*provider_arc).await {
                    warn!("[Redis] Failed to initialize Redis persistent storage: {}. Using in-memory storage.", e);
                    None
                } else {
                    Some(provider_arc)
                }
            }
            Err(e) => {
                warn!("[Redis] Failed to create Redis persistent storage: {}. Using in-memory storage.", e);
                None
            }
        };

    let redis_server = RespServer::new_with_persistence(bind_addr, redis_provider);

    let handle = tokio::spawn(async move {
        redis_server.run()
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
    });

    Ok(handle)
}
