//! Logging in: who may run queries, and over what.

use super::auth::{AuthMethod, UserStore};
use super::protocol::PostgresWireProtocol;
use postgres_protocol::authentication::sasl::{ChannelBinding, ScramSha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

struct Wire(DuplexStream);

impl Wire {
    async fn send(&mut self, tag: Option<u8>, body: &[u8]) {
        let mut m = Vec::new();
        if let Some(t) = tag {
            m.push(t);
        }
        m.extend_from_slice(&((body.len() as u32 + 4).to_be_bytes()));
        m.extend_from_slice(body);
        self.0.write_all(&m).await.unwrap();
    }

    async fn startup(&mut self, user: &str) {
        let mut body = 196_608u32.to_be_bytes().to_vec();
        body.extend_from_slice(format!("user\0{user}\0database\0orbit\0\0").as_bytes());
        self.send(None, &body).await;
    }

    /// The next message, or `None` when the server has closed the connection.
    async fn recv(&mut self) -> Option<(u8, Vec<u8>)> {
        let mut tag = [0u8; 1];
        self.0.read_exact(&mut tag).await.ok()?;
        let mut len = [0u8; 4];
        self.0.read_exact(&mut len).await.ok()?;
        let mut body = vec![0u8; u32::from_be_bytes(len) as usize - 4];
        self.0.read_exact(&mut body).await.ok()?;
        Some((tag[0], body))
    }

    /// A SCRAM-SHA-256 login. Returns the SQLSTATE of the error the server answers with, or `None` on success.
    async fn scram(&mut self, password: &str) -> Option<String> {
        let (tag, body) = self
            .recv()
            .await
            .expect("the server asks for authentication");
        assert_eq!(tag, b'R');
        assert_eq!(
            i32::from_be_bytes(body[..4].try_into().unwrap()),
            10,
            "SASL"
        );
        let mut scram = ScramSha256::new(password.as_bytes(), ChannelBinding::unrequested());
        let first = scram.message().to_vec();
        let mut init = b"SCRAM-SHA-256\0".to_vec();
        init.extend_from_slice(&(first.len() as i32).to_be_bytes());
        init.extend_from_slice(&first);
        self.send(Some(b'p'), &init).await;
        let (tag, body) = self
            .recv()
            .await
            .expect("a reply to the first SCRAM message");
        if tag == b'E' {
            return Some(sqlstate(&body));
        }
        assert_eq!(
            i32::from_be_bytes(body[..4].try_into().unwrap()),
            11,
            "SASLContinue"
        );
        scram.update(&body[4..]).unwrap();
        let fin = scram.message().to_vec();
        self.send(Some(b'p'), &fin).await;
        let (tag, body) = self
            .recv()
            .await
            .expect("a reply to the final SCRAM message");
        if tag == b'E' {
            return Some(sqlstate(&body));
        }
        assert_eq!(
            i32::from_be_bytes(body[..4].try_into().unwrap()),
            12,
            "SASLFinal"
        );
        scram
            .finish(&body[4..])
            .expect("the server proves it knows the password");
        None
    }
}

fn sqlstate(error_body: &[u8]) -> String {
    let mut i = 0;
    while i < error_body.len() && error_body[i] != 0 {
        let tag = error_body[i];
        let end = error_body[i + 1..].iter().position(|b| *b == 0).unwrap() + i + 1;
        if tag == b'C' {
            return String::from_utf8_lossy(&error_body[i + 1..end]).to_string();
        }
        i = end + 1;
    }
    String::new()
}

async fn server_with(users: &[(&str, &str)], auto_register: bool) -> Wire {
    let store = UserStore::new();
    for (u, p) in users {
        store
            .add_user(u.to_string(), p.to_string(), &AuthMethod::ScramSha256)
            .await;
    }
    let (client, server) = tokio::io::duplex(1 << 16);
    tokio::spawn(async move {
        let mut protocol = PostgresWireProtocol::new().with_auth(store, auto_register);
        let _ = protocol.handle_connection(server).await;
    });
    Wire(client)
}

#[tokio::test]
async fn a_client_that_skips_authentication_cannot_run_queries() {
    let mut w = server_with(&[("alice", "secret")], false).await;
    w.startup("alice").await;
    let (tag, _) = w.recv().await.unwrap();
    assert_eq!(tag, b'R', "the server asks for a login");
    // answer with a query instead of a password
    w.send(Some(b'Q'), b"SELECT 1\0").await;
    let (tag, body) = w.recv().await.expect("an error");
    assert_eq!(tag, b'E');
    assert_eq!(sqlstate(&body), "28000");
    assert!(
        w.recv().await.is_none(),
        "the connection is closed and no result was sent"
    );
}

#[tokio::test]
async fn a_wrong_password_and_an_unknown_user_are_refused_the_same_way() {
    let mut w = server_with(&[("alice", "secret")], false).await;
    w.startup("alice").await;
    assert_eq!(w.scram("not-the-password").await.as_deref(), Some("28P01"));
    assert!(
        w.recv().await.is_none(),
        "the connection is closed after a failed login"
    );

    let mut w = server_with(&[("alice", "secret")], false).await;
    w.startup("mallory").await;
    assert_eq!(
        w.scram("mallory").await.as_deref(),
        Some("28P01"),
        "a name equal to its password does not create a user"
    );
}

#[tokio::test]
async fn the_right_password_logs_in_and_queries_run() {
    let mut w = server_with(&[("alice", "secret")], false).await;
    w.startup("alice").await;
    assert_eq!(w.scram("secret").await, None);
    // AuthenticationOk, parameter statuses, key data, ReadyForQuery
    loop {
        let (tag, _) = w.recv().await.expect("startup completes");
        if tag == b'Z' {
            break;
        }
    }
    w.send(Some(b'Q'), b"SELECT 1\0").await;
    let mut got_rows = false;
    while let Some((tag, _)) = w.recv().await {
        assert_ne!(tag, b'E', "the query is accepted");
        got_rows |= tag == b'T' || tag == b'C';
        if tag == b'Z' {
            break;
        }
    }
    assert!(got_rows);
}

#[tokio::test]
async fn development_mode_still_creates_users_on_first_login() {
    let mut w = server_with(&[], true).await;
    w.startup("devuser").await;
    assert_eq!(w.scram("devuser").await, None);
}

#[tokio::test]
async fn a_server_with_no_users_and_no_development_switch_refuses_to_start() {
    let server =
        crate::protocols::PostgresServer::new("127.0.0.1:0").with_auth(UserStore::new(), false);
    let err = server.run().await.unwrap_err().to_string();
    assert!(err.contains("ORBIT_PG_USERS"), "{err}");
}

mod tls {
    use crate::config::TlsConfig;
    use crate::protocols::postgres_wire::auth::UserStore;
    use crate::protocols::PostgresServer;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio_rustls::rustls::client::danger::{
        HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
    };
    use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
    use tokio_rustls::rustls::{self, DigitallySignedStruct, SignatureScheme};

    /// Accepts any certificate: the test checks the negotiation, not the PKI.
    #[derive(Debug)]
    struct AcceptAny;

    impl ServerCertVerifier for AcceptAny {
        fn verify_server_cert(
            &self,
            _: &CertificateDer<'_>,
            _: &[CertificateDer<'_>],
            _: &ServerName<'_>,
            _: &[u8],
            _: UnixTime,
        ) -> Result<ServerCertVerified, rustls::Error> {
            Ok(ServerCertVerified::assertion())
        }
        fn verify_tls12_signature(
            &self,
            _: &[u8],
            _: &CertificateDer<'_>,
            _: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }
        fn verify_tls13_signature(
            &self,
            _: &[u8],
            _: &CertificateDer<'_>,
            _: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }
        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            vec![
                SignatureScheme::ECDSA_NISTP256_SHA256,
                SignatureScheme::ECDSA_NISTP384_SHA384,
                SignatureScheme::RSA_PSS_SHA256,
                SignatureScheme::RSA_PSS_SHA384,
                SignatureScheme::RSA_PKCS1_SHA256,
                SignatureScheme::ED25519,
            ]
        }
    }

    async fn start_tls_server() -> u16 {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let certs = concat!(env!("CARGO_MANIFEST_DIR"), "/../../config/certs");
        let tls = TlsConfig {
            enabled: true,
            cert_file: format!("{certs}/server_cert.pem").into(),
            key_file: format!("{certs}/server_key.pem").into(),
            ca_cert_file: None,
            require_client_cert: false,
        };
        let server = PostgresServer::new(format!("127.0.0.1:{port}"))
            .with_tls_config(Some(tls))
            .with_auth(UserStore::new(), true);
        tokio::spawn(async move {
            let _ = server.run().await;
        });
        for _ in 0..100 {
            if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        port
    }

    const SSL_REQUEST: [u8; 8] = [0, 0, 0, 8, 0x04, 0xd2, 0x16, 0x2f];

    #[tokio::test]
    async fn an_ssl_request_is_answered_and_the_connection_upgrades() {
        let port = start_tls_server().await;
        let mut tcp = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        tcp.write_all(&SSL_REQUEST).await.unwrap();
        let mut answer = [0u8; 1];
        tcp.read_exact(&mut answer).await.unwrap();
        assert_eq!(answer[0], b'S', "the server agrees to TLS");
        let config = rustls::ClientConfig::builder_with_provider(crate::protocols::tls::provider())
            .with_safe_default_protocol_versions()
            .unwrap()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAny))
            .with_no_client_auth();
        let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
        let mut tls = connector
            .connect(ServerName::try_from("localhost").unwrap(), tcp)
            .await
            .expect("the TLS handshake completes");
        // the startup message now travels encrypted and gets an authentication request back
        let mut body = 196_608u32.to_be_bytes().to_vec();
        body.extend_from_slice(b"user\0alice\0database\0orbit\0\0");
        let mut m = ((body.len() as u32 + 4).to_be_bytes()).to_vec();
        m.extend_from_slice(&body);
        tls.write_all(&m).await.unwrap();
        let mut tag = [0u8; 1];
        tls.read_exact(&mut tag).await.unwrap();
        assert_eq!(tag[0], b'R');
    }

    #[tokio::test]
    async fn plaintext_is_refused_when_tls_is_on() {
        let port = start_tls_server().await;
        let mut tcp = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let mut body = 196_608u32.to_be_bytes().to_vec();
        body.extend_from_slice(b"user\0alice\0database\0orbit\0\0");
        let mut m = ((body.len() as u32 + 4).to_be_bytes()).to_vec();
        m.extend_from_slice(&body);
        tcp.write_all(&m).await.unwrap();
        let mut reply = Vec::new();
        tcp.read_to_end(&mut reply).await.unwrap();
        assert_eq!(reply[0], b'E');
        assert!(String::from_utf8_lossy(&reply).contains("SSL connection is required"));
    }

    #[tokio::test]
    async fn without_tls_an_ssl_request_gets_n_and_the_client_may_continue_in_plaintext() {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let server =
            PostgresServer::new(format!("127.0.0.1:{port}")).with_auth(UserStore::new(), true);
        tokio::spawn(async move {
            let _ = server.run().await;
        });
        let mut tcp = loop {
            if let Ok(t) = TcpStream::connect(("127.0.0.1", port)).await {
                break t;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        };
        tcp.write_all(&SSL_REQUEST).await.unwrap();
        let mut answer = [0u8; 1];
        tcp.read_exact(&mut answer).await.unwrap();
        assert_eq!(answer[0], b'N');
    }
}
