//! PostgreSQL TCP server

use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info};

use crate::protocols::error::ProtocolResult;
use crate::protocols::postgres_wire::{protocol::PostgresWireProtocol, query_engine::QueryEngine};
use crate::protocols::ProtocolError;

use crate::config::TlsConfig;
use crate::protocols::postgres_wire::notifications::NotificationHub;
use crate::protocols::tls::OrbitTlsAcceptor;

/// PostgreSQL wire protocol server
pub struct PostgresServer {
    bind_addr: String,
    query_engine: Option<Arc<QueryEngine>>,
    tls_config: Option<TlsConfig>,
}

impl PostgresServer {
    /// Create a new PostgreSQL server
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            query_engine: None,
            tls_config: None,
        }
    }

    /// Create a new PostgreSQL server with custom query engine
    pub fn new_with_query_engine(bind_addr: impl Into<String>, query_engine: QueryEngine) -> Self {
        Self::new_with_query_engine_arc(bind_addr, Arc::new(query_engine))
    }

    /// Create a server sharing an engine with something else — the autovacuum
    /// worker, which has to run against the same storage the sessions use.
    pub fn new_with_query_engine_arc(
        bind_addr: impl Into<String>,
        query_engine: Arc<QueryEngine>,
    ) -> Self {
        tracing::debug!("PostgreSQL server created with a custom query engine");
        Self {
            bind_addr: bind_addr.into(),
            query_engine: Some(query_engine),
            tls_config: None,
        }
    }

    /// Set TLS configuration
    pub fn with_tls_config(mut self, tls_config: Option<TlsConfig>) -> Self {
        self.tls_config = tls_config;
        self
    }

    /// Start the server
    pub async fn run(&self) -> ProtocolResult<()> {
        let listener = TcpListener::bind(&self.bind_addr).await?;
        // One registry for the whole server: a NOTIFY on one connection has to
        // reach a LISTEN on another.
        let notifications = NotificationHub::new();
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
                    let notifications = Arc::clone(&notifications);

                    tokio::spawn(async move {
                        let mut protocol = if let Some(engine) = query_engine {
                            PostgresWireProtocol::new_with_query_engine(engine)
                        } else {
                            PostgresWireProtocol::new()
                        }
                        .with_notification_hub(notifications);

                        // PostgreSQL negotiates TLS explicitly: the client sends
                        // an SSLRequest in the clear and the server answers
                        // before any handshake. Handing the raw socket straight
                        // to the TLS acceptor — as this used to — never matches
                        // what a conforming client sends.
                        match negotiate_tls(stream, &tls_acceptor).await {
                            Ok((stream, prefix)) => {
                                if let Err(e) =
                                    protocol.handle_connection_with_buffer(stream, prefix).await
                                {
                                    error!("Connection error: {}", e);
                                }
                            }
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

impl Default for PostgresServer {
    fn default() -> Self {
        Self::new("127.0.0.1:5432")
    }
}

/// Request codes a client may send before the startup message.
///
/// These are sent as a bare `length + code` pair with no message-type byte.
pub mod pre_startup {
    /// `SSLRequest`: asks whether the server will speak TLS.
    pub const SSL_REQUEST: i32 = 80_877_103;
    /// `GSSENCRequest`: asks for GSSAPI encryption, which is not supported.
    pub const GSSENC_REQUEST: i32 = 80_877_104;
    /// A request to cancel the query running on another connection.
    pub const CANCEL_REQUEST: i32 = 80_877_102;
}

/// Answer any pre-startup requests, upgrading to TLS if one is asked for and
/// available.
///
/// Returns the stream to speak the rest of the protocol over, plus any bytes
/// already read that belong to the startup message and must not be lost.
async fn negotiate_tls(
    mut stream: TcpStream,
    tls_acceptor: &OrbitTlsAcceptor,
) -> std::io::Result<(crate::protocols::tls::TlsStreamOrPlain, bytes::BytesMut)> {
    use bytes::{Buf, BytesMut};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut prefix = BytesMut::with_capacity(8);

    loop {
        // Every pre-startup request is exactly 8 bytes: length, then code.
        while prefix.len() < 8 {
            if stream.read_buf(&mut prefix).await? == 0 {
                // Client hung up before saying anything.
                return Ok((
                    crate::protocols::tls::TlsStreamOrPlain::Plain(stream),
                    prefix,
                ));
            }
        }

        let code = (&prefix[4..8]).get_i32();

        match code {
            pre_startup::SSL_REQUEST => {
                prefix.advance(8);
                if tls_acceptor.is_enabled() {
                    stream.write_all(b"S").await?;
                    stream.flush().await?;
                    let stream = tls_acceptor.accept(stream).await?;
                    // The startup message arrives inside the TLS session, so
                    // nothing is carried over.
                    return Ok((stream, BytesMut::new()));
                }
                // No TLS configured: say so and continue in the clear. It is
                // then the client's choice whether that is acceptable.
                stream.write_all(b"N").await?;
                stream.flush().await?;
            }
            pre_startup::GSSENC_REQUEST => {
                prefix.advance(8);
                stream.write_all(b"N").await?;
                stream.flush().await?;
            }
            _ => {
                // A startup or cancel message: hand the bytes back unread.
                return Ok((
                    crate::protocols::tls::TlsStreamOrPlain::Plain(stream),
                    prefix,
                ));
            }
        }
    }
}
