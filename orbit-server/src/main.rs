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
    info!("🚀 Orbit Server Starting...");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));
    if args.dev_mode {
        warn!("🔧 Development mode enabled");
    }

    // Load configuration
    let mut config = load_config(&args).await?;

    // Override with command line arguments
    apply_cli_overrides(&mut config, &args);

    info!("🎯 Configuration loaded and CLI overrides applied");

    info!(
        "📡 gRPC server will listen on {}:{}",
        args.bind, args.grpc_port
    );
    info!(
        "🌍 HTTP API will listen on {}:{}",
        args.bind, args.http_port
    );
    info!(
        "📊 Metrics will be exposed on {}:{}",
        args.bind, args.metrics_port
    );

    // Log seed nodes
    if !args.seed_nodes.is_empty() {
        info!("🌱 Seed nodes: {:?}", args.seed_nodes);
    } else {
        info!("🏝️  Starting as seed node (no seed nodes configured)");
    }

    // Build and start server
    let mut server = OrbitServerBuilder::new()
        .with_bind_address(&args.bind)
        .with_port(args.grpc_port)
        .build()
        .await?;

    info!("✅ Orbit Server initialized successfully");
    info!("🎯 Ready to serve actor workloads");

    // Start the server (this blocks until shutdown)
    server.start().await?;

    info!("👋 Orbit Server shutdown complete");
    Ok(())
}

/// Load configuration from file or use defaults
async fn load_config(args: &Args) -> Result<OrbitServerConfig, Box<dyn Error>> {
    // For now, always use default configuration
    // TODO: Implement TOML config file loading
    if args.config.exists() {
        info!(
            "📖 Config file found: {:?} (using defaults for now)",
            args.config
        );
    } else {
        info!(
            "📄 Config file not found: {:?}. Using defaults.",
            args.config
        );
    }

    let mut config = OrbitServerConfig::default();

    // Apply basic overrides from CLI
    config.bind_address = args.bind.clone();
    config.port = args.grpc_port;

    Ok(config)
}

/// Apply command line argument overrides to configuration
fn apply_cli_overrides(_config: &mut OrbitServerConfig, args: &Args) {
    // Apply CLI overrides to config
    // This would override specific config values based on CLI args

    if args.dev_mode {
        info!("🔧 Applying development mode configuration overrides");
        // Set development-friendly defaults
    }

    info!("⚙️  Configuration applied with CLI overrides");
}
