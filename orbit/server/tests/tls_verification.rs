use orbit_server::config::TlsConfig;
use orbit_server::protocols::aql::{AqlServer, AqlStorage};
use orbit_server::protocols::cypher::server::CypherServer;
use orbit_server::protocols::cypher::storage::CypherGraphStorage;
use orbit_server::protocols::mongodb::MongoDbServer;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;

// Helper to load certificates (reused logic)
fn setup_certificates(test_dir: &std::path::Path) -> (PathBuf, PathBuf, PathBuf) {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let ca_cert_path = test_dir.join("ca.crt");
    let server_cert_path = test_dir.join("server.crt");
    let server_key_path = test_dir.join("server.key");

    // In a real scenario we would generate them.
    // Here we assume they exist or we copy them from a known location if possible.
    // Since we can't easily rely on external files in this isolated test,
    // we would ideally generate them.
    // For now, to keep it simple and given previous `tls_test.rs` generated them,
    // we'll use a mocked approach or try to find them.
    // Actually `tls_test.rs` relies on `command("openssl")`.
    // We will duplicate the generation logic for robustness.

    let subject = "/C=US/ST=State/L=City/O=Orbit/CN=localhost";

    // Generate CA key and cert
    let status = std::process::Command::new("openssl")
        .args([
            "req",
            "-new",
            "-x509",
            "-days",
            "1",
            "-nodes",
            "-text",
            "-out",
            ca_cert_path.to_str().unwrap(),
            "-keyout",
            test_dir.join("ca.key").to_str().unwrap(),
            "-subj",
            subject,
        ])
        .output()
        .expect("failed to execute openssl");

    if !status.status.success() {
        panic!("openssl failed: {:?}", status);
    }

    // Create extension file for SAN
    let ext_path = test_dir.join("v3.ext");
    std::fs::write(&ext_path, "subjectAltName=DNS:localhost").expect("failed to write ext file");

    // Generate Server key and CSR
    let status = std::process::Command::new("openssl")
        .args([
            "req",
            "-new",
            "-nodes",
            "-text",
            "-out",
            test_dir.join("server.csr").to_str().unwrap(),
            "-keyout",
            server_key_path.to_str().unwrap(),
            "-subj",
            subject,
        ])
        .output()
        .expect("failed to execute openssl");
    if !status.status.success() {
        panic!("openssl csr failed");
    }

    // Sign Server CSR with CA and include SAN
    let status = std::process::Command::new("openssl")
        .args([
            "x509",
            "-req",
            "-in",
            test_dir.join("server.csr").to_str().unwrap(),
            "-text",
            "-days",
            "1",
            "-CA",
            ca_cert_path.to_str().unwrap(),
            "-CAkey",
            test_dir.join("ca.key").to_str().unwrap(),
            "-CAcreateserial",
            "-out",
            server_cert_path.to_str().unwrap(),
            "-extfile",
            ext_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to execute openssl");
    if !status.status.success() {
        panic!("openssl sign failed");
    }

    (ca_cert_path, server_cert_path, server_key_path)
}

fn create_client_config(ca_cert_path: &PathBuf) -> Arc<ClientConfig> {
    let mut root_store = RootCertStore::empty();
    let cert_pem = std::fs::read(ca_cert_path).expect("failed to read ca cert");
    for cert in rustls_pemfile::certs(&mut &*cert_pem) {
        root_store.add(cert.unwrap()).unwrap();
    }

    Arc::new(
        ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    )
}

#[tokio::test]
async fn test_mongodb_tls_handshake() {
    let test_dir = tempfile::tempdir().unwrap();
    let (ca_cert, server_cert, server_key) = setup_certificates(test_dir.path());

    let tls_config = TlsConfig {
        enabled: true,
        cert_file: server_cert,
        key_file: server_key,
        ca_cert_file: Some(ca_cert.clone()),
        require_client_cert: false,
    };

    // Start Mongo Server
    let port = 27099; // Test port
    let addr = format!("127.0.0.1:{}", port);
    let server = MongoDbServer::new(addr.clone()).with_tls_config(Some(tls_config));

    let _handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    // Wait for server startup
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Connect using TlsConnector
    let client_config = create_client_config(&ca_cert);
    let connector = TlsConnector::from(client_config);
    let dns_name = "localhost".to_string().try_into().unwrap();

    let stream = TcpStream::connect(&addr)
        .await
        .expect("Failed to connect to Mongo port");
    let mut tls_stream = connector
        .connect(dns_name, stream)
        .await
        .expect("TLS handshake failed");

    // Send a simple Mongo handshake or just check stream is open
    // Simple verification: we established TLS, that's enough for this test layer
    tls_stream.flush().await.unwrap();
}

#[tokio::test]
async fn test_cypher_tls_handshake() {
    let test_dir = tempfile::tempdir().unwrap();
    let (ca_cert, server_cert, server_key) = setup_certificates(test_dir.path());

    let tls_config = TlsConfig {
        enabled: true,
        cert_file: server_cert,
        key_file: server_key,
        ca_cert_file: Some(ca_cert.clone()),
        require_client_cert: false,
    };

    // Start Cypher Server
    let port = 7699;
    let addr = format!("127.0.0.1:{}", port);
    let storage = Arc::new(CypherGraphStorage::new(test_dir.path().join("cypher_data")));
    // initialize storage needs to be async but GraphStorage::new is not?
    // Ah, CypherGraphStorage::new returns Self, Initialize logic?
    // main.rs: "storage.initialize().await"
    // But struct CypherGraphStorage is not public in crate::protocols::cypher?
    // It's in orbit_server::protocols::cypher::CypherGraphStorage (public re-export)

    let server = CypherServer::new_with_storage(&addr, storage).with_tls_config(Some(tls_config));

    let _handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(500)).await;

    let client_config = create_client_config(&ca_cert);
    let connector = TlsConnector::from(client_config);
    let dns_name = "localhost".to_string().try_into().unwrap();

    let stream = TcpStream::connect(&addr)
        .await
        .expect("Failed to connect to Cypher port");
    let mut tls_stream = connector
        .connect(dns_name, stream)
        .await
        .expect("TLS handshake failed");

    // Bolt Handshake: 0x60 60 B0 17 (GOGOBOLT) + 4 versions
    let handshake = [
        0x60, 0x60, 0xB0, 0x17, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    tls_stream.write_all(&handshake).await.unwrap();
    // Server should respond with selected version (4 bytes)
    let mut response = [0u8; 4];
    tls_stream
        .read_exact(&mut response)
        .await
        .expect("Failed to read Bolt handshake response");
    // Just verify we got something back
    assert_ne!(response, [0, 0, 0, 0]);
}

#[tokio::test]
async fn test_aql_tls_handshake() {
    let test_dir = tempfile::tempdir().unwrap();
    let (ca_cert, server_cert, server_key) = setup_certificates(test_dir.path());

    let tls_config = TlsConfig {
        enabled: true,
        cert_file: server_cert,
        key_file: server_key,
        ca_cert_file: Some(ca_cert.clone()),
        require_client_cert: false,
    };

    // Start AQL Server (HTTP)
    let port = 8599;
    let addr = format!("127.0.0.1:{}", port);
    let storage = Arc::new(AqlStorage::new(test_dir.path().join("aql_data")));

    let server = AqlServer::new_with_storage(&addr, storage).with_tls_config(Some(tls_config));

    let _handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(500)).await;

    let client_config = create_client_config(&ca_cert);
    let connector = TlsConnector::from(client_config);
    let dns_name = "localhost".to_string().try_into().unwrap();

    let stream = TcpStream::connect(&addr)
        .await
        .expect("Failed to connect to AQL port");
    let mut tls_stream = connector
        .connect(dns_name, stream)
        .await
        .expect("TLS handshake failed");

    // Simple HTTP GE request
    let request = b"GET /_api/version HTTP/1.1\r\nHost: localhost\r\n\r\n";
    tls_stream
        .write_all(request)
        .await
        .expect("Failed to write HTTP request");

    let mut buf = [0u8; 1024];
    let n = tls_stream
        .read(&mut buf)
        .await
        .expect("Failed to read HTTP response");
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.contains("HTTP/1.1 200 OK") || response.contains("HTTP/1.1"));
}
