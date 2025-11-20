//! Orbit Server - Multi-Protocol Distributed Actor System Runtime
//!
//! The enhanced Orbit server that natively provides:
//! - gRPC API for actor management and messaging
//! - PostgreSQL wire protocol for SQL access
//! - Redis RESP protocol for key-value and vector operations
//! - HTTP REST API for web access
//! - CQL (Cassandra Query Language) protocol for wide-column access
//! - MySQL wire protocol for MySQL-compatible SQL access
//! - Cluster membership and node discovery
//! - Load balancing and routing
//! - Health monitoring and metrics
//!
//! This is a single binary that replaces separate database servers (PostgreSQL, Redis, Cassandra, MySQL)
//! while providing full compatibility with their respective protocols.

use clap::Parser;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Import orbit components
use orbit_server::{OrbitServerBuilder};
use orbit_client::OrbitClient;
use orbit_shared::{
    ActorSystemConfig,
    pooling::{AdvancedConnectionPool, AdvancedPoolConfig, CircuitBreakerConfig, LoadBalancingStrategy, PoolTier}
};

// Import protocol servers
use orbit_protocols::{
    postgres_wire::{PostgresServer, query_engine::QueryEngine},
    resp::RespServer,
    rest::RestServer,
    cql::{CqlAdapter, CqlConfig},
    mysql::{MySqlAdapter, MySqlConfig},
};

// Import configuration
mod config;
use config::OrbitServerConfig;

/// Orbit Server - Multi-Protocol Distributed Actor System Runtime
#[derive(Parser)]
#[command(
    name = "orbit-server",
    version = env!("CARGO_PKG_VERSION"),
    about = "Orbit Server - Multi-Protocol Distributed Actor System",
    long_about = "The Orbit server provides a unified runtime that natively supports multiple protocols:\n\
    • PostgreSQL wire protocol (port 5432) - Full SQL compatibility with pgvector support\n\
    • MySQL wire protocol (port 3306) - MySQL-compatible SQL interface\n\
    • CQL protocol (port 9042) - Cassandra Query Language for wide-column access\n\
    • Redis RESP protocol (port 6379) - Key-value storage with vector operations\n\
    • gRPC API (port 50051) - Actor system management\n\
    • HTTP REST API (port 8080) - Web-friendly interface\n\
    \n\
    This single server replaces the need for separate PostgreSQL, MySQL, Cassandra, and Redis instances."
)]
struct Args {
    /// Configuration file path
    #[arg(
        short,
        long,
        value_name = "FILE",
        help = "Path to configuration file",
        default_value = "./config/orbit-server.toml"
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

    /// Enable PostgreSQL server
    #[arg(
        long,
        help = "Enable PostgreSQL wire protocol server"
    )]
    enable_postgresql: bool,

    /// PostgreSQL port
    #[arg(
        long,
        value_name = "PORT",
        help = "PostgreSQL server port (default: 5432)"
    )]
    postgres_port: Option<u16>,

    /// Enable Redis server
    #[arg(
        long,
        help = "Enable Redis RESP protocol server"
    )]
    enable_redis: bool,

    /// Redis port
    #[arg(
        long,
        value_name = "PORT", 
        help = "Redis server port (default: 6379)"
    )]
    redis_port: Option<u16>,

    /// Enable REST API server
    #[arg(
        long,
        help = "Enable HTTP REST API server"
    )]
    enable_rest: bool,

    /// REST API port
    #[arg(
        long,
        value_name = "PORT",
        help = "REST API server port (default: 8080)"
    )]
    rest_port: Option<u16>,

    /// Enable CQL server
    #[arg(
        long,
        help = "Enable CQL (Cassandra Query Language) protocol server"
    )]
    enable_cql: bool,

    /// CQL port
    #[arg(
        long,
        value_name = "PORT",
        help = "CQL server port (default: 9042)"
    )]
    cql_port: Option<u16>,

    /// Enable MySQL server
    #[arg(
        long,
        help = "Enable MySQL wire protocol server"
    )]
    enable_mysql: bool,

    /// MySQL port
    #[arg(
        long,
        value_name = "PORT",
        help = "MySQL server port (default: 3306)"
    )]
    mysql_port: Option<u16>,

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
        help = "Enable development mode (verbose logging, all protocols enabled)"
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

    /// Generate example configuration file
    #[arg(
        long,
        help = "Generate example configuration file and exit"
    )]
    generate_config: bool,
}

/// Connection type for pooling
#[derive(Debug, Clone)]
struct ProtocolConnection {
    protocol: String,
    client: OrbitClient,
}

/// Multi-protocol server manager with advanced connection pooling
struct MultiProtocolServer {
    config: OrbitServerConfig,
    orbit_client: OrbitClient,
    
    // Advanced connection pools per protocol
    postgres_pool: Option<AdvancedConnectionPool<ProtocolConnection>>,
    redis_pool: Option<AdvancedConnectionPool<ProtocolConnection>>,
    rest_pool: Option<AdvancedConnectionPool<ProtocolConnection>>,
    
    // Protocol servers
    postgres_server: Option<PostgresServer>,
    redis_server: Option<RespServer>,
    rest_server: Option<RestServer>,
    cql_server: Option<CqlAdapter>,
    mysql_server: Option<MySqlAdapter>,
}

impl MultiProtocolServer {
    /// Create a new multi-protocol server
    async fn new(config: OrbitServerConfig) -> Result<Self, Box<dyn Error>> {
        // Initialize actor system
        let actor_config = ActorSystemConfig {
            max_actors: config.actor_system.max_actors,
            mailbox_size: config.actor_system.mailbox_size,
            ..Default::default()
        };
        
        let orbit_client = OrbitClient::new(actor_config).await?;
        
        Ok(Self {
            config,
            orbit_client: orbit_client.clone(),
            postgres_pool: None,
            redis_pool: None,
            rest_pool: None,
            postgres_server: None,
            redis_server: None,
            rest_server: None,
            cql_server: None,
            mysql_server: None,
        })
    }
    
    /// Initialize connection pools for all enabled protocols
    async fn initialize_connection_pools(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.config.pooling.enabled {
            info!("🔗 Connection pooling disabled, using direct connections");
            return Ok(());
        }
        
        info!("🔗 Initializing advanced connection pools...");
        
        // Initialize PostgreSQL connection pool
        if let Some(pg_config) = &self.config.protocols.postgresql {
            if pg_config.enabled {
                info!("  🐘 Creating PostgreSQL connection pool...");
                let pool_config = self.create_pool_config("postgresql");
                let postgres_pool = self.create_connection_pool("postgresql", pool_config).await?;
                self.postgres_pool = Some(postgres_pool);
                info!("    ✅ PostgreSQL connection pool initialized");
            }
        }
        
        // Initialize Redis connection pool
        if let Some(redis_config) = &self.config.protocols.redis {
            if redis_config.enabled {
                info!("  🔴 Creating Redis connection pool...");
                let pool_config = self.create_pool_config("redis");
                let redis_pool = self.create_connection_pool("redis", pool_config).await?;
                self.redis_pool = Some(redis_pool);
                info!("    ✅ Redis connection pool initialized");
            }
        }
        
        // Initialize REST connection pool
        if let Some(rest_config) = &self.config.protocols.rest {
            if rest_config.enabled {
                info!("  🌍 Creating REST connection pool...");
                let pool_config = self.create_pool_config("rest");
                let rest_pool = self.create_connection_pool("rest", pool_config).await?;
                self.rest_pool = Some(rest_pool);
                info!("    ✅ REST connection pool initialized");
            }
        }
        
        info!("✅ All connection pools initialized");
        Ok(())
    }
    
    /// Create pool configuration for a specific protocol
    fn create_pool_config(&self, protocol: &str) -> AdvancedPoolConfig {
        let base_config = &self.config.pooling;
        
        // Check for protocol-specific overrides
        let protocol_override = base_config.protocol_overrides.get(protocol);
        
        AdvancedPoolConfig {
            min_connections: protocol_override
                .and_then(|o| o.min_connections)
                .unwrap_or(base_config.min_connections),
            max_connections: protocol_override
                .and_then(|o| o.max_connections)
                .unwrap_or(base_config.max_connections),
            connection_timeout: std::time::Duration::from_secs(base_config.connection_timeout_secs),
            idle_timeout: std::time::Duration::from_secs(base_config.idle_timeout_secs),
            max_lifetime: std::time::Duration::from_secs(base_config.max_lifetime_secs),
            health_check_interval: std::time::Duration::from_secs(base_config.health_check_interval_secs),
            load_balancing_strategy: protocol_override
                .and_then(|o| o.load_balancing_strategy)
                .unwrap_or(base_config.load_balancing_strategy),
            circuit_breaker: CircuitBreakerConfig {
                failure_threshold: base_config.circuit_breaker.failure_threshold,
                failure_window: std::time::Duration::from_secs(base_config.circuit_breaker.failure_window_secs),
                recovery_timeout: std::time::Duration::from_secs(base_config.circuit_breaker.recovery_timeout_secs),
                success_threshold: base_config.circuit_breaker.success_threshold,
                half_open_max_calls: base_config.circuit_breaker.half_open_max_calls,
            },
            enable_dynamic_sizing: base_config.enable_dynamic_sizing,
            target_utilization: base_config.target_utilization,
            tier: base_config.tier,
        }
    }
    
    /// Create a connection pool for a specific protocol
    async fn create_connection_pool(
        &self,
        protocol: &str,
        config: AdvancedPoolConfig,
    ) -> Result<AdvancedConnectionPool<ProtocolConnection>, Box<dyn Error>> {
        let orbit_client = self.orbit_client.clone();
        let protocol_name = protocol.to_string();
        
        let pool = AdvancedConnectionPool::new(config, move |node_id| {
            let client = orbit_client.clone();
            let protocol = protocol_name.clone();
            Box::pin(async move {
                // Create protocol-specific connection
                Ok(ProtocolConnection {
                    protocol: format!("{}-{}", protocol, node_id),
                    client,
                })
            })
        });
        
        // Add default nodes based on configuration
        let base_config = &self.config.pooling;
        if let Some(protocol_override) = base_config.protocol_overrides.get(protocol) {
            for node in &protocol_override.nodes {
                pool.add_node(node.node_id.clone(), node.max_connections).await;
            }
        } else {
            // Add default single node
            pool.add_node(format!("{}-primary", protocol), config.max_connections / 2).await;
        }
        
        // Start pool maintenance
        pool.start_maintenance().await;
        
        Ok(pool)
    }
    
    /// Initialize all configured protocol servers
    async fn initialize_servers(&mut self) -> Result<(), Box<dyn Error>> {
        info!("🔧 Initializing protocol servers...");
        
        // Initialize PostgreSQL server
        if let Some(pg_config) = &self.config.protocols.postgresql {
            if pg_config.enabled {
                info!("  🐘 Setting up PostgreSQL wire protocol server...");
                
                // Create query engine with orbit client
                let query_engine = QueryEngine::new(self.orbit_client.clone());
                
                let postgres_server = PostgresServer::new_with_query_engine(
                    format!("{}:{}", self.config.server.bind_address, pg_config.port),
                    query_engine,
                );
                
                self.postgres_server = Some(postgres_server);
                info!("    ✅ PostgreSQL server initialized on port {}", pg_config.port);
            }
        }
        
        // Initialize Redis server
        if let Some(redis_config) = &self.config.protocols.redis {
            if redis_config.enabled {
                info!("  🔴 Setting up Redis RESP protocol server...");
                
                let redis_server = RespServer::new(
                    format!("{}:{}", self.config.server.bind_address, redis_config.port),
                    self.orbit_client.clone(),
                );
                
                self.redis_server = Some(redis_server);
                info!("    ✅ Redis server initialized on port {}", redis_config.port);
            }
        }
        
        // Initialize REST server
        if let Some(rest_config) = &self.config.protocols.rest {
            if rest_config.enabled {
                info!("  🌍 Setting up HTTP REST API server...");

                let rest_server = RestServer::new(
                    format!("{}:{}", self.config.server.bind_address, rest_config.port),
                    self.orbit_client.clone(),
                );

                self.rest_server = Some(rest_server);
                info!("    ✅ REST server initialized on port {}", rest_config.port);
            }
        }

        // Initialize CQL server
        if let Some(cql_config) = &self.config.protocols.cql {
            if cql_config.enabled {
                info!("  🗂️  Setting up CQL (Cassandra) protocol server...");

                let cql_adapter_config = CqlConfig {
                    listen_addr: format!("{}:{}", self.config.server.bind_address, cql_config.port).parse()?,
                    max_connections: cql_config.max_connections,
                    authentication_enabled: cql_config.authentication_enabled,
                    protocol_version: cql_config.protocol_version,
                };

                let cql_server = CqlAdapter::new(cql_adapter_config).await?;

                self.cql_server = Some(cql_server);
                info!("    ✅ CQL server initialized on port {}", cql_config.port);
            }
        }

        // Initialize MySQL server
        if let Some(mysql_config) = &self.config.protocols.mysql {
            if mysql_config.enabled {
                info!("  🐬 Setting up MySQL wire protocol server...");

                let mysql_adapter_config = MySqlConfig {
                    listen_addr: format!("{}:{}", self.config.server.bind_address, mysql_config.port).parse()?,
                    max_connections: mysql_config.max_connections,
                    authentication_enabled: mysql_config.authentication_enabled,
                    server_version: mysql_config.server_version.clone(),
                };

                let mysql_server = MySqlAdapter::new(mysql_adapter_config).await?;

                self.mysql_server = Some(mysql_server);
                info!("    ✅ MySQL server initialized on port {}", mysql_config.port);
            }
        }

        info!("✅ All protocol servers initialized");
        Ok(())
    }
    
    /// Start all configured servers
    async fn start_servers(self) -> Result<(), Box<dyn Error>> {
        info!("🚀 Starting all protocol servers...");
        
        let mut handles = vec![];
        
        // Start PostgreSQL server
        if let Some(postgres_server) = self.postgres_server {
            let handle = tokio::spawn(async move {
                info!("🐘 PostgreSQL server starting...");
                if let Err(e) = postgres_server.run().await {
                    error!("PostgreSQL server error: {}", e);
                }
            });
            handles.push(handle);
        }
        
        // Start Redis server
        if let Some(redis_server) = self.redis_server {
            let handle = tokio::spawn(async move {
                info!("🔴 Redis server starting...");
                if let Err(e) = redis_server.run().await {
                    error!("Redis server error: {}", e);
                }
            });
            handles.push(handle);
        }
        
        // Start REST server
        if let Some(rest_server) = self.rest_server {
            let handle = tokio::spawn(async move {
                info!("🌍 REST server starting...");
                if let Err(e) = rest_server.run().await {
                    error!("REST server error: {}", e);
                }
            });
            handles.push(handle);
        }

        // Start CQL server
        if let Some(cql_server) = self.cql_server {
            let handle = tokio::spawn(async move {
                info!("🗂️  CQL server starting...");
                if let Err(e) = cql_server.start().await {
                    error!("CQL server error: {}", e);
                }
            });
            handles.push(handle);
        }

        // Start MySQL server
        if let Some(mysql_server) = self.mysql_server {
            let handle = tokio::spawn(async move {
                info!("🐬 MySQL server starting...");
                if let Err(e) = mysql_server.start().await {
                    error!("MySQL server error: {}", e);
                }
            });
            handles.push(handle);
        }

        // Start gRPC actor system server
        let grpc_config = self.config.protocols.grpc.as_ref().unwrap();
        if grpc_config.enabled {
            info!("📡 Starting gRPC actor system server...");
            let mut orbit_server = OrbitServerBuilder::new()
                .with_bind_address(&self.config.server.bind_address)
                .with_port(grpc_config.port)
                .build()
                .await?;
                
            let grpc_handle = tokio::spawn(async move {
                if let Err(e) = orbit_server.start().await {
                    error!("gRPC server error: {}", e);
                }
            });
            handles.push(grpc_handle);
        }
        
        info!("🎯 All servers started successfully!");
        info!("🔄 Server is ready to handle multi-protocol requests");
        
        // Wait for shutdown signal
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("🛑 Received shutdown signal...");
            }
            result = futures::future::try_join_all(handles) => {
                if let Err(e) = result {
                    error!("Server task error: {}", e);
                }
            }
        }
        
        info!("👋 Orbit multi-protocol server shutdown complete");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Handle config generation
    if args.generate_config {
        let example_config = OrbitServerConfig::generate_example_config();
        println!("{}", example_config);
        return Ok(());
    }

    // Initialize logging
    let log_level = if args.dev_mode {
        "debug,orbit_server=trace,orbit_shared=debug,orbit_protocols=debug".to_string()
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
    info!("🚀 Orbit Multi-Protocol Server Starting...");
    info!("Version: {}", env!(("CARGO_PKG_VERSION")));
    info!("🎯 Single server providing PostgreSQL, Redis, gRPC, and REST APIs");
    
    if args.dev_mode {
        warn!("🔧 Development mode enabled - all protocols active");
    }

    // Load configuration
    let mut config = load_config(&args).await?;

    // Apply command line overrides
    apply_cli_overrides(&mut config, &args);

    // Validate configuration
    config.validate()?;
    
    info!("📖 Configuration loaded and validated");

    // Print enabled protocols
    print_enabled_protocols(&config, &args);

    // Create and initialize multi-protocol server
    let mut server = MultiProtocolServer::new(config).await?;
    
    // Initialize connection pools first
    server.initialize_connection_pools().await?;
    
    // Then initialize protocol servers
    server.initialize_servers().await?;
    
    // Start all servers (this blocks until shutdown)
    server.start_servers().await?;

    Ok(())
}

/// Load configuration from file or use defaults
async fn load_config(args: &Args) -> Result<OrbitServerConfig, Box<dyn Error>> {
    if args.config.exists() {
        info!("📖 Loading config from: {:?}", args.config);
        OrbitServerConfig::load_from_file(&args.config).await
    } else {
        info!("📄 Config file not found: {:?}. Using defaults.", args.config);
        Ok(OrbitServerConfig::default())
    }
}

/// Apply command line argument overrides to configuration
fn apply_cli_overrides(config: &mut OrbitServerConfig, args: &Args) {
    // Server configuration overrides
    config.server.bind_address = args.bind.clone();
    if let Some(node_id) = &args.node_id {
        config.server.node_id = Some(node_id.clone());
    }

    // Protocol overrides
    if let Some(grpc_config) = &mut config.protocols.grpc {
        grpc_config.port = args.grpc_port;
    }

    // PostgreSQL overrides
    if args.enable_postgresql {
        if let Some(pg_config) = &mut config.protocols.postgresql {
            pg_config.enabled = true;
            if let Some(port) = args.postgres_port {
                pg_config.port = port;
            }
        } else {
            config.protocols.postgresql = Some(config::PostgresqlConfig {
                enabled: true,
                port: args.postgres_port.unwrap_or(5432),
                ..Default::default()
            });
        }
    }

    // Redis overrides
    if args.enable_redis {
        if let Some(redis_config) = &mut config.protocols.redis {
            redis_config.enabled = true;
            if let Some(port) = args.redis_port {
                redis_config.port = port;
            }
        } else {
            config.protocols.redis = Some(config::RedisConfig {
                enabled: true,
                port: args.redis_port.unwrap_or(6379),
                ..Default::default()
            });
        }
    }

    // REST overrides
    if args.enable_rest {
        if let Some(rest_config) = &mut config.protocols.rest {
            rest_config.enabled = true;
            if let Some(port) = args.rest_port {
                rest_config.port = port;
            }
        } else {
            config.protocols.rest = Some(config::RestConfig {
                enabled: true,
                port: args.rest_port.unwrap_or(8080),
                ..Default::default()
            });
        }
    }

    // CQL overrides
    if args.enable_cql {
        if let Some(cql_config) = &mut config.protocols.cql {
            cql_config.enabled = true;
            if let Some(port) = args.cql_port {
                cql_config.port = port;
            }
        } else {
            config.protocols.cql = Some(config::CqlConfig {
                enabled: true,
                port: args.cql_port.unwrap_or(9042),
                ..Default::default()
            });
        }
    }

    // MySQL overrides
    if args.enable_mysql {
        if let Some(mysql_config) = &mut config.protocols.mysql {
            mysql_config.enabled = true;
            if let Some(port) = args.mysql_port {
                mysql_config.port = port;
            }
        } else {
            config.protocols.mysql = Some(config::MySqlConfig {
                enabled: true,
                port: args.mysql_port.unwrap_or(3306),
                ..Default::default()
            });
        }
    }

    // Development mode overrides - enable all protocols
    if args.dev_mode {
        if let Some(grpc_config) = &mut config.protocols.grpc {
            grpc_config.enabled = true;
        }
        if let Some(pg_config) = &mut config.protocols.postgresql {
            pg_config.enabled = true;
        } else {
            config.protocols.postgresql = Some(config::PostgresqlConfig::default());
        }
        if let Some(redis_config) = &mut config.protocols.redis {
            redis_config.enabled = true;
        } else {
            config.protocols.redis = Some(config::RedisConfig::default());
        }
        if let Some(rest_config) = &mut config.protocols.rest {
            rest_config.enabled = true;
        } else {
            config.protocols.rest = Some(config::RestConfig::default());
        }
        if let Some(cql_config) = &mut config.protocols.cql {
            cql_config.enabled = true;
        } else {
            config.protocols.cql = Some(config::CqlConfig::default());
        }
        if let Some(mysql_config) = &mut config.protocols.mysql {
            mysql_config.enabled = true;
        } else {
            config.protocols.mysql = Some(config::MySqlConfig::default());
        }

        info!("🔧 Development mode: All protocols enabled");
    }

    // Clustering overrides
    if !args.seed_nodes.is_empty() {
        if config.actor_system.cluster.is_none() {
            config.actor_system.cluster = Some(config::ClusterConfig {
                seed_nodes: args.seed_nodes.clone(),
                gossip_port: 7946,
                heartbeat_interval_ms: 5000,
                failure_timeout_ms: 30000,
            });
        } else if let Some(cluster_config) = &mut config.actor_system.cluster {
            cluster_config.seed_nodes = args.seed_nodes.clone();
        }
    }

    info!("⚙️ Configuration applied with CLI overrides");
}

/// Print information about enabled protocols
fn print_enabled_protocols(config: &OrbitServerConfig, args: &Args) {
    let enabled = config.enabled_protocols();
    
    info!("🎯 Enabled Protocol Servers:");
    
    if enabled.contains(&"grpc") {
        let port = config.protocols.grpc.as_ref().unwrap().port;
        info!("  📡 gRPC Actor API:         {}:{}", args.bind, port);
    }
    
    if enabled.contains(&"postgresql") {
        let port = config.protocols.postgresql.as_ref().unwrap().port;
        info!("  🐘 PostgreSQL Wire:       {}:{} (psql compatible + pgvector)", args.bind, port);
    }
    
    if enabled.contains(&"redis") {
        let port = config.protocols.redis.as_ref().unwrap().port;
        info!("  🔴 Redis RESP:            {}:{} (redis-cli compatible + vectors)", args.bind, port);
    }
    
    if enabled.contains(&"rest") {
        let port = config.protocols.rest.as_ref().unwrap().port;
        info!("  🌍 HTTP REST API:         {}:{}", args.bind, port);
    }

    if enabled.contains(&"cql") {
        let port = config.protocols.cql.as_ref().unwrap().port;
        info!("  🗂️  CQL (Cassandra):      {}:{} (cqlsh compatible)", args.bind, port);
    }

    if enabled.contains(&"mysql") {
        let port = config.protocols.mysql.as_ref().unwrap().port;
        info!("  🐬 MySQL Wire:            {}:{} (mysql compatible)", args.bind, port);
    }

    info!("  📊 Prometheus Metrics:    {}:{}", args.bind, args.metrics_port);
    
    if !args.seed_nodes.is_empty() {
        info!("🌱 Cluster Mode: Seed nodes: {:?}", args.seed_nodes);
    } else {
        info!("🏝️  Standalone Mode: No clustering configured");
    }
    
    info!(""); // Empty line for readability
    info!("🎉 Ready to serve multi-protocol database workloads!");
    info!("   • Connect with psql for SQL queries and pgvector operations");
    info!("   • Connect with mysql for MySQL-compatible SQL queries");
    info!("   • Connect with cqlsh for Cassandra Query Language");
    info!("   • Connect with redis-cli for key-value and vector operations");
    info!("   • Use gRPC clients for actor system management");
    info!("   • Use HTTP REST for web applications");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_multi_protocol_server_creation() {
        let config = OrbitServerConfig::default();
        let server = MultiProtocolServer::new(config).await;
        assert!(server.is_ok());
    }
    
    #[test]
    fn test_cli_overrides() {
        let mut config = OrbitServerConfig::default();
        let args = Args {
            config: PathBuf::from("test.toml"),
            bind: "127.0.0.1".to_string(),
            grpc_port: 50052,
            enable_postgresql: true,
            postgres_port: Some(5433),
            enable_redis: true,
            redis_port: Some(6380),
            enable_rest: false,
            rest_port: None,
            metrics_port: 9091,
            node_id: Some("test-node".to_string()),
            seed_nodes: vec!["node1:7946".to_string()],
            dev_mode: false,
            log_level: "info".to_string(),
            generate_config: false,
        };
        
        apply_cli_overrides(&mut config, &args);
        
        assert_eq!(config.server.bind_address, "127.0.0.1");
        assert_eq!(config.server.node_id, Some("test-node".to_string()));
        assert_eq!(config.protocols.grpc.as_ref().unwrap().port, 50052);
        assert!(config.protocols.postgresql.as_ref().unwrap().enabled);
        assert_eq!(config.protocols.postgresql.as_ref().unwrap().port, 5433);
        assert!(config.protocols.redis.as_ref().unwrap().enabled);
        assert_eq!(config.protocols.redis.as_ref().unwrap().port, 6380);
    }
}