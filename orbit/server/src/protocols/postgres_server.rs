//! PostgreSQL TCP server

use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

use crate::protocols::error::ProtocolResult;
use crate::protocols::postgres_wire::auth::UserStore;
use crate::protocols::postgres_wire::{protocol::PostgresWireProtocol, query_engine::QueryEngine};
use crate::protocols::ProtocolError;

use crate::config::TlsConfig;
use crate::protocols::tls::{OrbitTlsAcceptor, TlsStreamOrPlain};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// PostgreSQL wire protocol server
pub struct PostgresServer {
    bind_addr: String,
    query_engine: Option<Arc<QueryEngine>>,
    tls_config: Option<TlsConfig>,
    users: Option<UserStore>,
    auto_register: Option<bool>,
}

/// Users provisioned through the environment: `ORBIT_PG_USERS=name:password,...` and/or
/// `ORBIT_PG_USERS_FILE=path` with one `name:password` per line.
pub async fn users_from_env() -> Result<UserStore, String> {
    use crate::protocols::postgres_wire::auth::AuthMethod;
    let store = UserStore::new();
    let mut entries: Vec<String> = Vec::new();
    if let Ok(v) = std::env::var("ORBIT_PG_USERS") {
        entries.extend(v.split(',').map(str::to_string));
    }
    if let Ok(path) = std::env::var("ORBIT_PG_USERS_FILE") {
        let text = std::fs::read_to_string(&path).map_err(|e| format!("read {path}: {e}"))?;
        entries.extend(text.lines().map(str::to_string));
    }
    for e in entries {
        let e = e.trim();
        if e.is_empty() || e.starts_with('#') {
            continue;
        }
        let (user, pass) = e
            .split_once(':')
            .ok_or_else(|| format!("a PostgreSQL user entry must be name:password, got `{e}`"))?;
        store
            .add_user(
                user.trim().to_string(),
                pass.to_string(),
                &AuthMethod::ScramSha256,
            )
            .await;
    }
    Ok(store)
}

impl PostgresServer {
    /// Create a new PostgreSQL server
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            query_engine: None,
            tls_config: None,
            users: None,
            auto_register: None,
        }
    }

    /// Create a new PostgreSQL server with custom query engine
    pub fn new_with_query_engine(bind_addr: impl Into<String>, query_engine: QueryEngine) -> Self {
        println!("DEBUG: PostgresServer initialized with custom query engine");
        Self {
            bind_addr: bind_addr.into(),
            query_engine: Some(Arc::new(query_engine)),
            tls_config: None,
            users: None,
            auto_register: None,
        }
    }

    /// Set TLS configuration
    pub fn with_tls_config(mut self, tls_config: Option<TlsConfig>) -> Self {
        self.tls_config = tls_config;
        self
    }

    /// Authenticate against these users. With `auto_register`, unknown users are created on first login
    /// with their name as the password: development and tests only.
    #[must_use]
    pub fn with_auth(mut self, users: UserStore, auto_register: bool) -> Self {
        self.users = Some(users);
        self.auto_register = Some(auto_register);
        self
    }

    /// Start the server
    pub async fn run(&self) -> ProtocolResult<()> {
        // users come from the caller, or else from the environment; with neither, and no explicit development
        // switch (ORBIT_PG_AUTO_REGISTER=1), nobody could log in, so refuse to start rather than run open
        let auto_register = self.auto_register.unwrap_or_else(|| {
            std::env::var("ORBIT_PG_AUTO_REGISTER")
                .map(|v| v == "1")
                .unwrap_or(false)
        });
        let users = match &self.users {
            Some(u) => u.clone(),
            None => users_from_env().await.map_err(ProtocolError::IoError)?,
        };
        if !auto_register && !users.has_users().await {
            return Err(ProtocolError::IoError(
                "no PostgreSQL users are configured: set ORBIT_PG_USERS=name:password (or ORBIT_PG_USERS_FILE), \
                 or ORBIT_PG_AUTO_REGISTER=1 for development"
                    .to_string(),
            ));
        }
        if auto_register {
            tracing::warn!(
                "PostgreSQL logins create users with their name as the password: development only"
            );
        }
        let listener = TcpListener::bind(&self.bind_addr).await?;
        let tls_acceptor = OrbitTlsAcceptor::new(&self.tls_config)
            .map_err(|e| ProtocolError::IoError(e.to_string()))?;

        info!("PostgreSQL server listening on {}", self.bind_addr);
        if tls_acceptor.is_enabled() {
            info!("PostgreSQL TLS enabled");
        }

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New connection from {}", addr);
                    let query_engine = self.query_engine.clone();
                    let tls_acceptor = tls_acceptor.clone();
                    let users = users.clone();

                    tokio::spawn(async move {
                        let mut protocol = if let Some(engine) = query_engine {
                            PostgresWireProtocol::new_with_query_engine(engine)
                        } else {
                            PostgresWireProtocol::new()
                        }
                        .with_auth(users, auto_register);

                        // PostgreSQL clients ask for TLS with an SSLRequest before the startup message
                        match negotiate(stream, &tls_acceptor).await {
                            Ok(Some(stream)) => {
                                if let Err(e) = protocol.handle_connection(stream).await {
                                    error!("Connection error: {}", e);
                                }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                error!("TLS negotiation error: {}", e);
                            }
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }
}

const SSL_REQUEST: u32 = 80_877_103;
const GSS_ENC_REQUEST: u32 = 80_877_104;
const CANCEL_REQUEST: u32 = 80_877_102;

/// A fatal ErrorResponse, written before the connection is closed.
fn fatal(code: &str, message: &str) -> Vec<u8> {
    let mut body = Vec::new();
    for (tag, text) in [
        (b'S', "FATAL"),
        (b'V', "FATAL"),
        (b'C', code),
        (b'M', message),
    ] {
        body.push(tag);
        body.extend_from_slice(text.as_bytes());
        body.push(0);
    }
    body.push(0);
    let mut out = vec![b'E'];
    out.extend_from_slice(&((body.len() as u32 + 4).to_be_bytes()));
    out.extend_from_slice(&body);
    out
}

/// Look at the first message a client sends. An `SSLRequest` is answered `S` and the connection is
/// upgraded to TLS (or `N` when TLS is off); a request for GSS encryption is declined. When TLS is on, a
/// client that goes on without it is refused. Returns `None` when the connection has been dealt with.
async fn negotiate(
    mut stream: TcpStream,
    tls: &OrbitTlsAcceptor,
) -> std::io::Result<Option<TlsStreamOrPlain>> {
    for _ in 0..3 {
        let mut head = [0u8; 8];
        // the first 8 bytes of any startup-phase message: length, then request code or protocol version
        let mut seen = 0;
        while seen < 8 {
            seen = stream.peek(&mut head).await?;
            if seen == 0 {
                return Ok(None);
            }
            if seen < 8 {
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            }
        }
        let code = u32::from_be_bytes([head[4], head[5], head[6], head[7]]);
        match code {
            SSL_REQUEST => {
                stream.read_exact(&mut head).await?;
                if tls.is_enabled() {
                    stream.write_all(b"S").await?;
                    return Ok(Some(tls.accept(stream).await?));
                }
                stream.write_all(b"N").await?;
            }
            GSS_ENC_REQUEST => {
                stream.read_exact(&mut head).await?;
                stream.write_all(b"N").await?;
            }
            CANCEL_REQUEST => return Ok(Some(TlsStreamOrPlain::Plain(stream))),
            _ => {
                if tls.is_enabled() {
                    stream
                        .write_all(&fatal("28000", "SSL connection is required"))
                        .await?;
                    // close politely: unread input at close time would reset the connection and lose the error
                    let _ = stream.shutdown().await;
                    let mut sink = [0u8; 1024];
                    let _ = tokio::time::timeout(
                        std::time::Duration::from_millis(300),
                        stream.read(&mut sink),
                    )
                    .await;
                    return Ok(None);
                }
                return Ok(Some(TlsStreamOrPlain::Plain(stream)));
            }
        }
    }
    Ok(None)
}

impl Default for PostgresServer {
    fn default() -> Self {
        Self::new("127.0.0.1:5432")
    }
}
