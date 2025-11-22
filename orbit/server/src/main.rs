//! Orbit Server - Distributed Actor System Runtime
//!
//! The main Orbit server binary that provides:
//! - gRPC API for actor management and messaging
//! - Cluster membership and node discovery
//! - Load balancing and routing
//! - Health monitoring and metrics
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
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use orbit_server::{OrbitServerBuilder, OrbitServerConfig};
use orbit_protocols::cql::{CqlAdapter, CqlConfig};
use orbit_protocols::mysql::{MySqlAdapter, MySqlConfig};
use tokio::task::JoinHandle;

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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.dev_mode {
        "debug,orbit_server=trace,orbit_shared=debug,orbit_proto=debug".to_string()
    } else {
        format!("{},orbit_server=info,orbit_shared=info", args.log_level)
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
    info!("[Orbit Server] Orbit Server Starting...");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));
    if args.dev_mode {
        warn!("[Dev Mode] Development mode enabled");
    }

    // Load configuration
    let mut config = load_config(&args).await?;

    // Override with command line arguments
    apply_cli_overrides(&mut config, &args);

    info!("[Configuration] Configuration loaded and CLI overrides applied");

    info!(
        "[gRPC] gRPC server will listen on {}:{}",
        args.bind, args.grpc_port
    );
    info!(
        "[HTTP API] HTTP API will listen on {}:{}",
        args.bind, args.http_port
    );
    info!(
        "[Metrics] Metrics will be exposed on {}:{}",
        args.bind, args.metrics_port
    );

    // Log protocol adapters
    info!(
        "[MySQL] MySQL protocol will listen on {}:{}",
        args.bind, args.mysql_port
    );
    info!(
        "[CQL] CQL protocol will listen on {}:{}",
        args.bind, args.cql_port
    );

    // Log seed nodes
    if !args.seed_nodes.is_empty() {
        info!("[Seed Nodes] Seed nodes: {:?}", args.seed_nodes);
    } else {
        info!("[Seed Nodes] Starting as seed node (no seed nodes configured)");
    }

    // Build and start server
    let mut server = OrbitServerBuilder::new()
        .with_bind_address(&args.bind)
        .with_port(args.grpc_port)
        .build()
        .await?;

    info!("[Orbit Server] Orbit Server initialized successfully");

    // Start protocol adapters
    let mut protocol_handles: Vec<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>> = Vec::new();

    // Start MySQL protocol adapter
    let mysql_config = MySqlConfig {
        listen_addr: format!("{}:{}", args.bind, args.mysql_port).parse()?,
        max_connections: 1000,
        authentication_enabled: false,
        server_version: format!("Orbit-DB {} (MySQL-compatible)", env!("CARGO_PKG_VERSION")),
        username: None,
        password: None,
    };

    let mysql_adapter = MySqlAdapter::new(mysql_config).await?;
    let mysql_handle = tokio::spawn(async move {
        info!("[MySQL] MySQL protocol adapter started on port 3306");
        mysql_adapter.start().await.map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
    });
    protocol_handles.push(mysql_handle);

    // Start CQL protocol adapter
    let cql_config = CqlConfig {
        listen_addr: format!("{}:{}", args.bind, args.cql_port).parse()?,
        max_connections: 1000,
        authentication_enabled: false,
        protocol_version: 4,
        username: None,
        password: None,
    };

    let cql_adapter = CqlAdapter::new(cql_config).await?;
    let cql_handle = tokio::spawn(async move {
        info!("[CQL] CQL protocol adapter started on port 9042");
        cql_adapter.start().await.map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
    });
    protocol_handles.push(cql_handle);

    info!("[Orbit Server] Ready to serve actor workloads");

    // Start the gRPC server in a separate task
    let grpc_handle = tokio::spawn(async move {
        server.start().await.map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
    });

    // Wait for any server to exit (all run indefinitely until interrupted)
    tokio::select! {
        result = grpc_handle => {
            match result {
                Ok(Ok(())) => info!("gRPC server exited successfully"),
                Ok(Err(e)) => warn!("gRPC server error: {}", e),
                Err(e) => warn!("gRPC server panic: {}", e),
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
                Ok(Ok(())) => info!("Protocol adapter exited successfully"),
                Ok(Err(e)) => warn!("Protocol adapter error: {}", e),
                Err(e) => warn!("Protocol adapter panic: {}", e),
            }
        }
    }

    info!("[Orbit Server] Orbit Server shutdown complete");
    Ok(())
}

/// Load configuration from file or use defaults
async fn load_config(args: &Args) -> Result<OrbitServerConfig, Box<dyn Error>> {
    // For now, always use default configuration
    // TODO: Implement TOML config file loading
    if args.config.exists() {
        info!(
            "[Config] Config file found: {:?} (using defaults for now)",
            args.config
        );
    } else {
        info!(
            "[Config] Config file not found: {:?}. Using defaults.",
            args.config
        );
    }

    let mut config = OrbitServerConfig::default();
    config.bind_address = args.bind.clone();
    config.port = args.grpc_port;

    Ok(config)
}

/// Apply command line argument overrides to configuration
fn apply_cli_overrides(config: &mut OrbitServerConfig, args: &Args) {
    apply_network_overrides(config, args);
    apply_mode_overrides(config, args);

    info!("[CLI Overrides] Configuration applied with CLI overrides");
}

/// Apply network-related CLI overrides
fn apply_network_overrides(config: &mut OrbitServerConfig, args: &Args) {
    config.bind_address = args.bind.clone();
    config.port = args.grpc_port;
}

/// Apply mode-specific CLI overrides
fn apply_mode_overrides(_config: &mut OrbitServerConfig, args: &Args) {
    if args.dev_mode {
        info!("[Dev Mode] Applying development mode configuration overrides");
        // Set development-friendly defaults here
    }
}
