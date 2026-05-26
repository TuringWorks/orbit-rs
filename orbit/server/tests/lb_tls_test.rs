use orbit_server::config::TlsConfig;
use orbit_server::protocols::PostgresServer;
use orbit_server::tcp_proxy::{run_proxy, ProxyConfig};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_rustls::rustls::{pki_types::CertificateDer, ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;

/// Integration test for TLS passthrough through the load balancer proxy.
///
/// This test requires:
///   - Self-signed test certificates in `config/certs/`
///   - Ports 25432 and 26432 to be available
///
/// Run with: cargo test -p orbit-server --test lb_tls_test -- --ignored
#[tokio::test]
#[ignore = "requires test certificates and available ports; run with --ignored"]
async fn test_lb_tls_passthrough() {
    // Install crypto provider
    tokio_rustls::rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    // 1. Setup paths (relative to workspace root)
    let certs_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/certs");
    let ca_cert_path = certs_dir.join("ca_cert.pem");
    let server_cert_path = certs_dir.join("server_cert.pem");
    let server_key_path = certs_dir.join("server_key.pem");
    let client_cert_path = certs_dir.join("client_cert.pem");
    let client_key_path = certs_dir.join("client_key.pem");

    // Verify cert files exist
    for path in [&ca_cert_path, &server_cert_path, &server_key_path, &client_cert_path, &client_key_path] {
        assert!(path.exists(), "Missing cert file: {}", path.display());
    }

    // 2. Configure Server (Backend)
    let server_port = 25432;
    let tls_config = TlsConfig {
        enabled: true,
        cert_file: server_cert_path,
        key_file: server_key_path,
        ca_cert_file: Some(ca_cert_path.clone()),
        require_client_cert: true,
    };

    let server =
        PostgresServer::new(format!("127.0.0.1:{}", server_port)).with_tls_config(Some(tls_config));

    // Spawn Backend Server
    tokio::spawn(async move {
        let _ = server.run().await;
    });

    // Wait for server to be up
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 3. Configure Load Balancer (Proxy)
    let proxy_port = 26432;
    let backend_addr = format!("127.0.0.1:{}", server_port).parse().unwrap();
    let proxy_config = Arc::new(ProxyConfig::new("test-lb", proxy_port, vec![backend_addr]));

    // Spawn Proxy
    tokio::spawn(async move {
        let _ = run_proxy(proxy_config, "127.0.0.1", true).await;
    });

    // Wait for proxy to be up
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 4. Client Connection to PROXY
    // Load CA
    let mut root_cert_store = RootCertStore::empty();
    let ca_cert_bytes = std::fs::read(&ca_cert_path).expect("Failed to read CA cert");
    let ca_cert = rustls_pemfile::certs(&mut &ca_cert_bytes[..])
        .next()
        .unwrap()
        .unwrap();
    root_cert_store.add(ca_cert).unwrap();

    // Load Client Cert/Key
    let client_cert_bytes = std::fs::read(&client_cert_path).expect("Failed to read client cert");
    let client_key_bytes = std::fs::read(&client_key_path).expect("Failed to read client key");

    let client_certs: Vec<CertificateDer> = rustls_pemfile::certs(&mut &client_cert_bytes[..])
        .collect::<Result<_, _>>()
        .unwrap();
    let client_key = rustls_pemfile::private_key(&mut &client_key_bytes[..])
        .unwrap()
        .unwrap();

    let client_config = ClientConfig::builder()
        .with_root_certificates(root_cert_store)
        .with_client_auth_cert(client_certs, client_key)
        .unwrap();

    let connector = TlsConnector::from(Arc::new(client_config));

    // Connect to PROXY PORT
    let stream = TcpStream::connect(format!("127.0.0.1:{}", proxy_port))
        .await
        .expect("Failed to connect to proxy");

    // Perform TLS Handshake (domain must match cert SAN)
    let domain = "localhost".to_string().try_into().unwrap();
    match connector.connect(domain, stream).await {
        Ok(_) => {
            println!("TLS Handshake via Proxy Successful!");
        }
        Err(e) => {
            panic!("TLS Handshake Failed via Proxy: {}", e);
        }
    }
}
