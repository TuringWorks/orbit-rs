use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_rustls::rustls::{ClientConfig, RootCertStore, pki_types::CertificateDer};
use tokio_rustls::TlsConnector;
use orbit_server::protocols::PostgresServer;
use orbit_server::config::TlsConfig;

#[tokio::test]
async fn test_postgres_server_tls_connection() {
    // Install crypto provider
    tokio_rustls::rustls::crypto::ring::default_provider().install_default().ok();

    // 1. Setup paths
    let certs_dir = PathBuf::from("/Users/ravindraboddipalli/.gemini/certs");
    let ca_cert_path = certs_dir.join("ca_cert.pem");
    let server_cert_path = certs_dir.join("server_cert.pem");
    let server_key_path = certs_dir.join("server_key.pem");
    let client_cert_path = certs_dir.join("client_cert.pem");
    let client_key_path = certs_dir.join("client_key.pem");

    // Ensure certs exist (assuming they were created by previous tool calls)
    if !ca_cert_path.exists() {
        eprintln!("SKIPPING TEST: Certs not found at {:?}", certs_dir);
        return;
    }

    // 2. Configure Server with mTLS
    let tls_config = TlsConfig {
        enabled: true,
        cert_file: server_cert_path.clone(),
        key_file: server_key_path.clone(),
        ca_cert_file: Some(ca_cert_path.clone()),
        require_client_cert: true,
    };

    // 3. Start Server
    let port = 54443; // Random port
    let bind_addr = format!("127.0.0.1:{}", port);
    
    let server = PostgresServer::new(bind_addr.clone())
        .with_tls_config(Some(tls_config));

    let server_handle = tokio::spawn(async move {
        server.run().await.expect("Server failed");
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_secs(1)).await;

    // 4. Client Connection
    // Load CA cert
    let mut root_store = RootCertStore::empty();
    let ca_cert_pem = std::fs::read_to_string(&ca_cert_path).unwrap();
    for cert in rustls_pemfile::certs(&mut ca_cert_pem.as_bytes()) {
        root_store.add(cert.unwrap()).unwrap();
    }

    // Load Client cert/key for mTLS
    let client_cert_pem = std::fs::read_to_string(&client_cert_path).unwrap();
    let client_key_pem = std::fs::read_to_string(&client_key_path).unwrap();
    
    let client_certs: Vec<CertificateDer> = rustls_pemfile::certs(&mut client_cert_pem.as_bytes())
        .collect::<Result<_, _>>().unwrap();
    let client_key = rustls_pemfile::private_key(&mut client_key_pem.as_bytes())
        .unwrap().unwrap();

    let client_config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_client_auth_cert(client_certs, client_key)
        .unwrap();
    
    let connector = TlsConnector::from(Arc::new(client_config));

    // Connect
    let stream = TcpStream::connect(&bind_addr).await.expect("Failed to connect via TCP");
    let domain = "localhost".to_string().try_into().unwrap();
    
    let tls_stream = connector.connect(domain, stream).await;
    
    match tls_stream {
        Ok(_) => println!("TLS Handshake Success!"),
        Err(e) => panic!("TLS Handshake Failed: {}", e),
    }

    // Cleanup
    server_handle.abort();
}
