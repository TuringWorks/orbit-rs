//! OrbitWire Server Implementation
//!
//! TCP server for the OrbitWire protocol

use super::codec::{CodecError, OrbitWireCodec};
use super::frame::{Frame, FrameFlags, MessageType};
use super::messages::*;
use super::session::{OrbitWireSession, OrbitWireSessionManager, SessionError};
use super::values::WireValue;
use super::OrbitWireConfig;

use bytes::Bytes;
use futures::stream::StreamExt;
use futures::SinkExt;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio_util::codec::Framed;
use tracing::{debug, error, info, instrument};

/// OrbitWire Server
pub struct OrbitWireServer {
    config: OrbitWireConfig,
    session_manager: Arc<OrbitWireSessionManager>,
}

impl OrbitWireServer {
    /// Create a new OrbitWire server
    pub fn new(config: OrbitWireConfig) -> Self {
        Self {
            config,
            session_manager: Arc::new(OrbitWireSessionManager::default()),
        }
    }

    /// Get the session manager
    pub fn session_manager(&self) -> Arc<OrbitWireSessionManager> {
        self.session_manager.clone()
    }

    /// Start the OrbitWire server
    #[instrument(skip(self))]
    pub async fn start(&self) -> Result<(), ServerError> {
        let addr: SocketAddr = format!("{}:{}", self.config.bind_address, self.config.port)
            .parse()
            .map_err(|e| ServerError::BindError(format!("Invalid address: {}", e)))?;

        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| ServerError::BindError(e.to_string()))?;

        info!("OrbitWire server listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    info!("New connection from {}", peer_addr);
                    let session_manager = self.session_manager.clone();
                    let config = self.config.clone();

                    tokio::spawn(async move {
                        if let Err(e) =
                            handle_connection(stream, peer_addr, session_manager, config).await
                        {
                            error!("Connection error from {}: {}", peer_addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Accept error: {}", e);
                }
            }
        }
    }
}

/// Handle a single client connection
async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    session_manager: Arc<OrbitWireSessionManager>,
    config: OrbitWireConfig,
) -> Result<(), ServerError> {
    // Set up codec
    let codec = OrbitWireCodec::server()
        .with_max_frame_size(config.max_frame_size)
        .with_compression(config.default_compression);

    let mut framed = Framed::new(stream, codec);

    // Create session
    let session = session_manager.create_session().await;
    let session_id = session.read().await.session_id.clone();

    debug!("Session {} created for {}", session_id, peer_addr);

    // Connection handler
    let handler = ConnectionHandler::new(session.clone());

    // Main message loop
    while let Some(result) = framed.next().await {
        match result {
            Ok(frame) => {
                debug!(
                    "Received frame: type={:?}, stream={}",
                    frame.message_type, frame.stream_id
                );

                match handler.handle_frame(frame).await {
                    Ok(responses) => {
                        for response in responses {
                            if let Err(e) = framed.send(response).await {
                                error!("Failed to send response: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        // Send error frame
                        let error_msg = ErrorMessage::new("INTERNAL", e.to_string());
                        let error_frame = error_msg.to_frame(0);
                        let _ = framed.send(error_frame).await;

                        if matches!(e, HandlerError::Fatal(_)) {
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                error!("Frame decode error: {}", e);
                if matches!(
                    e,
                    CodecError::InvalidMagic | CodecError::VersionMismatch { .. }
                ) {
                    break;
                }
            }
        }
    }

    // Clean up session
    session_manager.remove_session(&session_id).await;
    info!(
        "Connection closed for {} (session {})",
        peer_addr, session_id
    );

    Ok(())
}

/// Connection handler for processing frames
struct ConnectionHandler {
    session: Arc<RwLock<OrbitWireSession>>,
}

impl ConnectionHandler {
    fn new(session: Arc<RwLock<OrbitWireSession>>) -> Self {
        Self { session }
    }

    /// Handle an incoming frame
    async fn handle_frame(&self, frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        match frame.message_type {
            MessageType::Hello => self.handle_hello(frame).await,
            MessageType::Authenticate => self.handle_authenticate(frame).await,
            MessageType::Goodbye => self.handle_goodbye(frame).await,
            MessageType::Ping => self.handle_ping(frame).await,
            MessageType::Query => self.handle_query(frame).await,
            MessageType::QueryCancel => self.handle_query_cancel(frame).await,
            MessageType::Prepare => self.handle_prepare(frame).await,
            MessageType::Execute => self.handle_execute(frame).await,
            MessageType::ClosePrepared => self.handle_close_prepared(frame).await,
            MessageType::Begin => self.handle_begin(frame).await,
            MessageType::Commit => self.handle_commit(frame).await,
            MessageType::Rollback => self.handle_rollback(frame).await,
            MessageType::Savepoint => self.handle_savepoint(frame).await,
            MessageType::ReleaseSavepoint => self.handle_release_savepoint(frame).await,
            MessageType::RollbackToSavepoint => self.handle_rollback_to_savepoint(frame).await,
            MessageType::LiveSubscribe => self.handle_live_subscribe(frame).await,
            MessageType::LiveKill => self.handle_live_kill(frame).await,
            _ => Err(HandlerError::UnsupportedMessageType(frame.message_type)),
        }
    }

    /// Handle Hello message
    async fn handle_hello(&self, frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        let mut payload = frame.payload;
        let hello = HelloMessage::decode(&mut payload)
            .map_err(|e| HandlerError::InvalidMessage(e.to_string()))?;

        debug!("Hello from {} v{}", hello.client_name, hello.client_version);

        // Set capabilities
        let mut session = self.session.write().await;
        session.set_capabilities(hello.capabilities);

        // Send HelloAck
        let ack = HelloAckMessage::new(&session.session_id);
        Ok(vec![ack.to_frame()])
    }

    /// Handle Authenticate message
    async fn handle_authenticate(&self, _frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        // In a real implementation, this would validate credentials
        let mut session = self.session.write().await;
        session.set_user("authenticated_user");

        let ack = Frame::connection_frame(MessageType::AuthenticateOk, Bytes::new());
        Ok(vec![ack])
    }

    /// Handle Goodbye message
    async fn handle_goodbye(&self, _frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        let ack = Frame::connection_frame(MessageType::GoodbyeAck, Bytes::new());
        Ok(vec![ack])
    }

    /// Handle Ping message
    async fn handle_ping(&self, frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        let pong = Frame::connection_frame(MessageType::Pong, frame.payload);
        Ok(vec![pong])
    }

    /// Handle Query message
    async fn handle_query(&self, frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        let stream_id = frame.stream_id;

        // In a real implementation, this would:
        // 1. Parse the query
        // 2. Execute it
        // 3. Stream results

        // Simulate query execution
        let schema = RowDescriptionMessage::new(vec![
            ColumnDescription {
                name: "id".to_string(),
                type_tag: 0x05, // Int64
                nullable: false,
                precision: None,
                scale: None,
            },
            ColumnDescription {
                name: "name".to_string(),
                type_tag: 0x10, // String
                nullable: true,
                precision: None,
                scale: None,
            },
        ]);

        // Example row
        let row = RowDataMessage::new(vec![
            WireValue::Int64(1),
            WireValue::String("Example".to_string()),
        ]);

        let complete = CommandCompleteMessage::new("SELECT", 1);

        Ok(vec![
            schema.to_frame(stream_id),
            row.to_frame(stream_id),
            complete.to_frame(stream_id),
        ])
    }

    /// Handle QueryCancel message
    async fn handle_query_cancel(&self, frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        let stream_id = frame.stream_id;
        let mut session = self.session.write().await;
        session.complete_stream(stream_id);

        let cancelled = Frame::with_flags(
            FrameFlags::new().with_end_stream(),
            stream_id,
            MessageType::QueryCancelled,
            Bytes::new(),
        );
        Ok(vec![cancelled])
    }

    /// Handle Prepare message
    async fn handle_prepare(&self, frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        let stream_id = frame.stream_id;
        // In real implementation, parse the prepare message and create prepared statement

        let ok = Frame::new(stream_id, MessageType::PrepareOk, Bytes::new());
        Ok(vec![ok])
    }

    /// Handle Execute message
    async fn handle_execute(&self, frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        let stream_id = frame.stream_id;
        // In real implementation, execute the prepared statement

        let complete = CommandCompleteMessage::new("EXECUTE", 1);
        Ok(vec![complete.to_frame(stream_id)])
    }

    /// Handle ClosePrepared message
    async fn handle_close_prepared(&self, frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        let stream_id = frame.stream_id;
        let ok = Frame::new(stream_id, MessageType::ClosePreparedOk, Bytes::new());
        Ok(vec![ok])
    }

    /// Handle Begin message
    async fn handle_begin(&self, frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        let stream_id = frame.stream_id;
        let payload = frame.payload;
        let mut payload = frame.payload;

        // Parse begin options
        let isolation = if payload.len() >= 1 {
            match payload[0] {
                1 => IsolationLevel::ReadUncommitted,
                2 => IsolationLevel::ReadCommitted,
                3 => IsolationLevel::RepeatableRead,
                4 => IsolationLevel::Serializable,
                5 => IsolationLevel::Snapshot,
                _ => IsolationLevel::ReadCommitted,
            }
        } else {
            IsolationLevel::ReadCommitted
        };

        let read_only = payload.len() >= 2 && payload[1] != 0;

        let mut session = self.session.write().await;
        let tx_id = session
            .begin_transaction(isolation, read_only)
            .map_err(|e| HandlerError::SessionError(e))?;

        let ack = BeginOkMessage::new(tx_id);
        Ok(vec![ack.to_frame(stream_id)])
    }

    /// Handle Commit message
    async fn handle_commit(&self, frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        let stream_id = frame.stream_id;
        let mut session = self.session.write().await;
        let tx_id = session
            .commit_transaction()
            .map_err(|e| HandlerError::SessionError(e))?;

        let ack = Frame::with_flags(
            FrameFlags::new().with_end_stream(),
            stream_id,
            MessageType::CommitOk,
            Bytes::copy_from_slice(&tx_id),
        );
        Ok(vec![ack])
    }

    /// Handle Rollback message
    async fn handle_rollback(&self, frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        let stream_id = frame.stream_id;
        let mut session = self.session.write().await;
        let tx_id = session
            .rollback_transaction()
            .map_err(|e| HandlerError::SessionError(e))?;

        let ack = Frame::with_flags(
            FrameFlags::new().with_end_stream(),
            stream_id,
            MessageType::RollbackOk,
            Bytes::copy_from_slice(&tx_id),
        );
        Ok(vec![ack])
    }

    /// Handle Savepoint message
    async fn handle_savepoint(&self, frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        let stream_id = frame.stream_id;
        // In real implementation, parse savepoint name from payload
        let name = "savepoint";

        let mut session = self.session.write().await;
        let sp_id = session
            .create_savepoint(name)
            .map_err(|e| HandlerError::SessionError(e))?;

        let ack = Frame::new(
            stream_id,
            MessageType::SavepointOk,
            Bytes::copy_from_slice(&sp_id),
        );
        Ok(vec![ack])
    }

    /// Handle ReleaseSavepoint message
    async fn handle_release_savepoint(&self, frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        let stream_id = frame.stream_id;
        // In real implementation, parse savepoint name
        let name = "savepoint";

        let mut session = self.session.write().await;
        let sp_id = session
            .release_savepoint(name)
            .map_err(|e| HandlerError::SessionError(e))?;

        let ack = Frame::new(
            stream_id,
            MessageType::ReleaseSavepointOk,
            Bytes::copy_from_slice(&sp_id),
        );
        Ok(vec![ack])
    }

    /// Handle RollbackToSavepoint message
    async fn handle_rollback_to_savepoint(&self, frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        let stream_id = frame.stream_id;
        // In real implementation, parse savepoint name
        let name = "savepoint";

        let mut session = self.session.write().await;
        let sp_id = session
            .rollback_to_savepoint(name)
            .map_err(|e| HandlerError::SessionError(e))?;

        let ack = Frame::new(
            stream_id,
            MessageType::RollbackToSavepointOk,
            Bytes::copy_from_slice(&sp_id),
        );
        Ok(vec![ack])
    }

    /// Handle LiveSubscribe message
    async fn handle_live_subscribe(&self, frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        let stream_id = frame.stream_id;
        let mut payload = frame.payload;

        let msg = LiveSubscribeMessage::decode(&mut payload)
            .map_err(|e| HandlerError::InvalidMessage(e.to_string()))?;

        let mut session = self.session.write().await;
        let subscription = session.subscribe_live_query(&msg.query, msg.diff_mode);

        // Create schema for subscription
        let schema = RowDescriptionMessage::new(vec![ColumnDescription {
            name: "data".to_string(),
            type_tag: 0x10, // String
            nullable: true,
            precision: None,
            scale: None,
        }]);

        let ack = LiveSubscribeOkMessage::new(subscription.subscription_id, schema);
        Ok(vec![ack.to_frame(stream_id)])
    }

    /// Handle LiveKill message
    async fn handle_live_kill(&self, frame: Frame) -> Result<Vec<Frame>, HandlerError> {
        let stream_id = frame.stream_id;

        if frame.payload.len() < 16 {
            return Err(HandlerError::InvalidMessage(
                "Invalid subscription ID".to_string(),
            ));
        }

        let mut subscription_id = [0u8; 16];
        subscription_id.copy_from_slice(&frame.payload[..16]);

        let mut session = self.session.write().await;
        session.unsubscribe_live_query(&subscription_id);

        let ack = Frame::with_flags(
            FrameFlags::new().with_end_stream(),
            stream_id,
            MessageType::LiveKillOk,
            Bytes::copy_from_slice(&subscription_id),
        );
        Ok(vec![ack])
    }
}

impl LiveSubscribeMessage {
    fn decode(data: &mut Bytes) -> Result<Self, MessageError> {
        use bytes::Buf;

        if data.remaining() < 4 {
            return Err(MessageError::InsufficientData);
        }

        let query_len = data.get_u32() as usize;
        if data.remaining() < query_len + 1 {
            return Err(MessageError::InsufficientData);
        }

        let query = String::from_utf8(data.copy_to_bytes(query_len).to_vec())
            .map_err(|_| MessageError::InvalidUtf8)?;
        let diff_mode = data.get_u8() != 0;

        Ok(Self { query, diff_mode })
    }
}

/// Server errors
#[derive(Debug)]
pub enum ServerError {
    BindError(String),
    IoError(std::io::Error),
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerError::BindError(e) => write!(f, "Bind error: {}", e),
            ServerError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for ServerError {}

/// Handler errors
#[derive(Debug)]
pub enum HandlerError {
    UnsupportedMessageType(MessageType),
    InvalidMessage(String),
    SessionError(SessionError),
    #[allow(dead_code)]
    Fatal(String),
}

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandlerError::UnsupportedMessageType(t) => {
                write!(f, "Unsupported message type: {:?}", t)
            }
            HandlerError::InvalidMessage(e) => write!(f, "Invalid message: {}", e),
            HandlerError::SessionError(e) => write!(f, "Session error: {}", e),
            HandlerError::Fatal(e) => write!(f, "Fatal error: {}", e),
        }
    }
}

impl std::error::Error for HandlerError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        let config = OrbitWireConfig::default();
        let server = OrbitWireServer::new(config);
        assert_eq!(server.config.port, 50053);
    }

    #[tokio::test]
    async fn test_connection_handler_ping() {
        let session = Arc::new(RwLock::new(OrbitWireSession::new()));
        let handler = ConnectionHandler::new(session);

        let ping_frame = Frame::connection_frame(MessageType::Ping, Bytes::from("ping_data"));

        let responses = handler.handle_frame(ping_frame).await.unwrap();
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].message_type, MessageType::Pong);
        assert_eq!(responses[0].payload, Bytes::from("ping_data"));
    }

    #[tokio::test]
    async fn test_connection_handler_hello() {
        let session = Arc::new(RwLock::new(OrbitWireSession::new()));
        let handler = ConnectionHandler::new(session.clone());

        let hello = HelloMessage::new("test-client", "1.0.0");
        let hello_frame = hello.to_frame();

        let responses = handler.handle_frame(hello_frame).await.unwrap();
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].message_type, MessageType::HelloAck);
    }

    #[tokio::test]
    async fn test_connection_handler_transaction() {
        let session = Arc::new(RwLock::new(OrbitWireSession::new()));
        let handler = ConnectionHandler::new(session.clone());

        // Begin transaction
        let begin = BeginMessage::new().with_isolation(IsolationLevel::ReadCommitted);
        let begin_frame = begin.to_frame(1);

        let responses = handler.handle_frame(begin_frame).await.unwrap();
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].message_type, MessageType::BeginOk);

        // Verify transaction is active
        assert!(session.read().await.has_transaction());

        // Commit
        let commit_frame = Frame::new(1, MessageType::Commit, Bytes::new());
        let responses = handler.handle_frame(commit_frame).await.unwrap();
        assert_eq!(responses[0].message_type, MessageType::CommitOk);

        // Verify transaction is ended
        assert!(!session.read().await.has_transaction());
    }
}
