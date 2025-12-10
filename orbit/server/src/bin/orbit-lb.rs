//! Lightweight TCP Load Balancer for Orbit-RS Cluster Testing
//!
//! A simple round-robin TCP proxy for testing multi-node clusters.
//!
//! Usage:
//!   orbit-lb --config lb-config.toml
//!   orbit-lb --redis 6379:16379,16380,16381
//!   orbit-lb --postgres 5432:15432,15433,15434 --redis 6379:16379,16380,16381

use clap::Parser;
use orbit_server::tcp_proxy::{run_proxy, ProxyConfig};
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(
    name = "orbit-lb",
    about = "Lightweight TCP Load Balancer for Orbit-RS",
    long_about = "A simple round-robin TCP proxy for cluster testing.\n\n\
                  Example:\n  \
                  orbit-lb --redis 6379:16379,16380,16381 --postgres 5432:15432,15433,15434"
)]
struct Args {
    /// Redis proxy: listen_port:backend_port1,backend_port2,...
    #[arg(long, value_name = "SPEC")]
    redis: Option<String>,

    /// PostgreSQL proxy: listen_port:backend_port1,backend_port2,...
    #[arg(long, value_name = "SPEC")]
    postgres: Option<String>,

    /// MySQL proxy: listen_port:backend_port1,backend_port2,...
    #[arg(long, value_name = "SPEC")]
    mysql: Option<String>,

    /// CQL proxy: listen_port:backend_port1,backend_port2,...
    #[arg(long, value_name = "SPEC")]
    cql: Option<String>,

    /// HTTP proxy: listen_port:backend_port1,backend_port2,...
    #[arg(long, value_name = "SPEC")]
    http: Option<String>,

    /// gRPC proxy: listen_port:backend_port1,backend_port2,...
    #[arg(long, value_name = "SPEC")]
    grpc: Option<String>,

    /// Backend host (default: 127.0.0.1)
    #[arg(long, default_value = "127.0.0.1")]
    backend_host: String,

    /// Bind address (default: 0.0.0.0)
    #[arg(long, default_value = "0.0.0.0")]
    bind: String,

    /// Number of cluster nodes (auto-configures all protocols)
    #[arg(long, short = 'n', value_name = "COUNT")]
    nodes: Option<usize>,

    /// Verbose output
    #[arg(long, short)]
    verbose: bool,
}

fn parse_proxy_spec(spec: &str, backend_host: &str) -> Result<(u16, Vec<SocketAddr>), String> {
    let parts: Vec<&str> = spec.split(':').collect();
    if parts.len() != 2 {
        return Err(format!(
            "Invalid spec '{}': expected listen_port:backend_ports",
            spec
        ));
    }

    let listen_port: u16 = parts[0]
        .parse()
        .map_err(|_| format!("Invalid listen port: {}", parts[0]))?;

    let backends: Result<Vec<SocketAddr>, String> = parts[1]
        .split(',')
        .map(|p| {
            let port: u16 = p
                .trim()
                .parse()
                .map_err(|_| format!("Invalid port: {}", p))?;
            Ok(format!("{}:{}", backend_host, port).parse().unwrap())
        })
        .collect();

    Ok((listen_port, backends?))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut proxies: Vec<Arc<ProxyConfig>> = Vec::new();

    // Auto-configure for N nodes
    if let Some(nodes) = args.nodes {
        println!("Configuring load balancer for {}-node cluster", nodes);

        let protocols = [
            ("Redis", 6379u16, 16379u16),
            ("PostgreSQL", 5432, 15432),
            ("MySQL", 3306, 13306),
            ("CQL", 9042, 19042),
            ("HTTP", 8080, 18080),
            ("gRPC", 50051, 60051),
        ];

        for (name, lb_port, base_port) in protocols {
            let backends: Vec<SocketAddr> = (0..nodes)
                .map(|i| {
                    format!("{}:{}", args.backend_host, base_port + i as u16)
                        .parse()
                        .unwrap()
                })
                .collect();
            proxies.push(Arc::new(ProxyConfig::new(name, lb_port, backends)));
        }
    } else {
        // Manual configuration
        let specs = [
            ("Redis", &args.redis),
            ("PostgreSQL", &args.postgres),
            ("MySQL", &args.mysql),
            ("CQL", &args.cql),
            ("HTTP", &args.http),
            ("gRPC", &args.grpc),
        ];

        for (name, spec) in specs {
            if let Some(s) = spec {
                let (listen_port, backends) = parse_proxy_spec(s, &args.backend_host)?;
                proxies.push(Arc::new(ProxyConfig::new(name, listen_port, backends)));
            }
        }
    }

    if proxies.is_empty() {
        eprintln!("No proxies configured. Use --nodes N or specify individual protocols.");
        eprintln!("Examples:");
        eprintln!("  orbit-lb --nodes 3");
        eprintln!("  orbit-lb --redis 6379:16379,16380,16381");
        std::process::exit(1);
    }

    println!("Starting Orbit Load Balancer");
    println!("============================");

    let mut handles = Vec::new();
    for proxy in proxies {
        let bind = args.bind.clone();
        let verbose = args.verbose;
        handles.push(tokio::spawn(async move {
            run_proxy(proxy, &bind, verbose).await.unwrap();
        }));
    }

    println!("============================");
    println!("Load balancer running. Press Ctrl+C to stop.");

    // Wait for all proxies
    for handle in handles {
        handle.await?;
    }

    Ok(())
}
