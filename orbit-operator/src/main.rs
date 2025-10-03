use anyhow::Result;
use clap::Parser;
use kube::Client;
use std::sync::Arc;
use tokio::signal;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod actor_controller;
mod actor_crd;
mod cluster_controller;
mod crd;
mod transaction_controller;
mod transaction_crd;

use actor_controller::ActorController;
use cluster_controller::ClusterController;
use transaction_controller::TransactionController;

#[derive(Parser)]
#[command(name = "orbit-operator")]
#[command(about = "Kubernetes operator for Orbit-RS distributed actor system")]
struct Args {
    /// Kubernetes namespace to watch (empty = all namespaces)
    #[arg(long, env = "WATCH_NAMESPACE")]
    namespace: Option<String>,

    /// Metrics bind address
    #[arg(long, env = "METRICS_ADDR", default_value = "0.0.0.0:8080")]
    metrics_addr: String,

    /// Health probe bind address
    #[arg(long, env = "HEALTH_ADDR", default_value = "0.0.0.0:8081")]
    health_addr: String,

    /// Log level
    #[arg(long, env = "LOG_LEVEL", default_value = "info")]
    log_level: String,

    /// Enable development mode (more verbose logging)
    #[arg(long, env = "DEV_MODE")]
    dev_mode: bool,

    /// Leader election namespace
    #[arg(long, env = "LEADER_ELECTION_NAMESPACE")]
    leader_election_namespace: Option<String>,

    /// Leader election lease name
    #[arg(
        long,
        env = "LEADER_ELECTION_NAME",
        default_value = "orbit-operator-leader-election"
    )]
    leader_election_name: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing
    let log_level = match args.log_level.as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };

    let subscriber = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(args.dev_mode)
                .with_level(true)
                .with_ansi(!std::env::var("NO_COLOR").is_ok()),
        );

    subscriber.init();

    info!("Starting Orbit-RS Kubernetes Operator");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));
    info!("Watch namespace: {:?}", args.namespace);
    info!("Metrics address: {}", args.metrics_addr);
    info!("Health address: {}", args.health_addr);

    // Initialize Kubernetes client
    let client = Client::try_default().await?;
    info!("Kubernetes client initialized");

    // Start health and metrics servers
    let health_server = start_health_server(args.health_addr.clone());
    let metrics_server = start_metrics_server(args.metrics_addr.clone());

    // Start controllers
    info!("Starting controllers...");

    let cluster_controller = ClusterController::new(client.clone());
    let actor_controller = ActorController::new(client.clone());
    let transaction_controller = TransactionController::new(client.clone());

    let cluster_task = tokio::spawn(async move {
        cluster_controller.run().await;
    });

    let actor_task = tokio::spawn(async move {
        actor_controller.run().await;
    });

    let transaction_task = tokio::spawn(async move {
        transaction_controller.run().await;
    });

    info!("All controllers started successfully");

    // Wait for shutdown signal
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Received SIGINT, shutting down gracefully...");
        }
        _ = cluster_task => {
            warn!("Cluster controller exited unexpectedly");
        }
        _ = actor_task => {
            warn!("Actor controller exited unexpectedly");
        }
        _ = transaction_task => {
            warn!("Transaction controller exited unexpectedly");
        }
        _ = health_server => {
            warn!("Health server exited unexpectedly");
        }
        _ = metrics_server => {
            warn!("Metrics server exited unexpectedly");
        }
    }

    info!("Orbit-RS operator shutdown complete");
    Ok(())
}

async fn start_health_server(addr: String) -> Result<()> {
    use hyper::service::{make_service_fn, service_fn};
    use hyper::{Body, Request, Response, Server, StatusCode};
    use std::convert::Infallible;
    use std::net::SocketAddr;

    async fn health_handler(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(r#"{"status":"healthy"}"#))
            .unwrap())
    }

    async fn readiness_handler(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
        // TODO: Add actual readiness checks (e.g., can connect to Kubernetes API)
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(r#"{"status":"ready"}"#))
            .unwrap())
    }

    async fn handle_request(req: Request<Body>) -> Result<Response<Body>, Infallible> {
        match req.uri().path() {
            "/healthz" => health_handler(req).await,
            "/readyz" => readiness_handler(req).await,
            _ => Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not Found"))
                .unwrap()),
        }
    }

    let make_svc =
        make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(handle_request)) });

    let addr: SocketAddr = addr.parse()?;
    let server = Server::bind(&addr).serve(make_svc);

    info!("Health server listening on {}", addr);

    if let Err(e) = server.await {
        eprintln!("Health server error: {}", e);
    }

    Ok(())
}

async fn start_metrics_server(addr: String) -> Result<()> {
    use hyper::service::{make_service_fn, service_fn};
    use hyper::{Body, Request, Response, Server, StatusCode};
    use std::convert::Infallible;
    use std::net::SocketAddr;

    async fn metrics_handler(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
        // TODO: Implement actual Prometheus metrics collection
        let metrics = r#"
# HELP orbit_operator_info Information about the Orbit operator
# TYPE orbit_operator_info gauge
orbit_operator_info{version="0.1.0"} 1

# HELP orbit_clusters_total Total number of OrbitClusters
# TYPE orbit_clusters_total gauge
orbit_clusters_total 0

# HELP orbit_actors_total Total number of OrbitActors
# TYPE orbit_actors_total gauge
orbit_actors_total 0

# HELP orbit_transactions_total Total number of OrbitTransactions
# TYPE orbit_transactions_total gauge
orbit_transactions_total 0
"#;

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/plain; version=0.0.4")
            .body(Body::from(metrics))
            .unwrap())
    }

    async fn handle_request(req: Request<Body>) -> Result<Response<Body>, Infallible> {
        match req.uri().path() {
            "/metrics" => metrics_handler(req).await,
            _ => Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not Found"))
                .unwrap()),
        }
    }

    let make_svc =
        make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(handle_request)) });

    let addr: SocketAddr = addr.parse()?;
    let server = Server::bind(&addr).serve(make_svc);

    info!("Metrics server listening on {}", addr);

    if let Err(e) = server.await {
        eprintln!("Metrics server error: {}", e);
    }

    Ok(())
}
