//! OrbitWire frame structure
//!
//! Defines the frame format for OrbitWire protocol messages

use super::CompressionType;
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// Frame header size: 14 bytes
/// - Flags: 1 byte
/// - Stream ID: 4 bytes
/// - Message Type: 2 bytes
/// - Payload Length: 4 bytes
/// - Reserved: 3 bytes
pub const FRAME_HEADER_SIZE: usize = 14;

/// Maximum payload size (16MB - header)
pub const MAX_PAYLOAD_SIZE: usize = 16 * 1024 * 1024 - FRAME_HEADER_SIZE;

/// Frame flags
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameFlags {
    /// End of stream marker
    pub end_stream: bool,
    /// Frame is compressed
    pub compressed: bool,
    /// Frame requires acknowledgment
    pub ack_required: bool,
    /// Frame is a continuation of previous frame
    pub continuation: bool,
    /// Compression type (2 bits)
    pub compression_type: CompressionType,
}

impl FrameFlags {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_end_stream(mut self) -> Self {
        self.end_stream = true;
        self
    }

    pub fn with_compression(mut self, compression: CompressionType) -> Self {
        self.compressed = compression != CompressionType::None;
        self.compression_type = compression;
        self
    }

    pub fn with_ack_required(mut self) -> Self {
        self.ack_required = true;
        self
    }

    pub fn with_continuation(mut self) -> Self {
        self.continuation = true;
        self
    }

    /// Encode flags to a single byte
    pub fn to_byte(&self) -> u8 {
        let mut flags = 0u8;
        if self.end_stream {
            flags |= 0x01;
        }
        if self.compressed {
            flags |= 0x02;
        }
        if self.ack_required {
            flags |= 0x04;
        }
        if self.continuation {
            flags |= 0x08;
        }
        // Compression type in bits 4-5
        flags |= (self.compression_type.to_byte() & 0x03) << 4;
        flags
    }

    /// Decode flags from a byte
    pub fn from_byte(b: u8) -> Self {
        Self {
            end_stream: (b & 0x01) != 0,
            compressed: (b & 0x02) != 0,
            ack_required: (b & 0x04) != 0,
            continuation: (b & 0x08) != 0,
            compression_type: CompressionType::from_byte((b >> 4) & 0x03),
        }
    }
}

/// Message type codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MessageType {
    // Connection lifecycle (0x00xx)
    Hello = 0x0001,
    HelloAck = 0x0002,
    Authenticate = 0x0003,
    AuthenticateOk = 0x0004,
    AuthenticateFail = 0x0005,
    Goodbye = 0x0006,
    GoodbyeAck = 0x0007,
    Ping = 0x0008,
    Pong = 0x0009,

    // Query execution (0x01xx)
    Query = 0x0101,
    QueryOk = 0x0102,
    QueryError = 0x0103,
    QueryCancel = 0x0104,
    QueryCancelled = 0x0105,

    // Result streaming (0x02xx)
    RowDescription = 0x0201,
    RowData = 0x0202,
    RowsComplete = 0x0203,
    CommandComplete = 0x0204,

    // Prepared statements (0x03xx)
    Prepare = 0x0301,
    PrepareOk = 0x0302,
    PrepareError = 0x0303,
    Execute = 0x0304,
    ExecuteOk = 0x0305,
    BindParameters = 0x0306,
    BindOk = 0x0307,
    ClosePrepared = 0x0308,
    ClosePreparedOk = 0x0309,

    // Transactions (0x04xx)
    Begin = 0x0401,
    BeginOk = 0x0402,
    Commit = 0x0403,
    CommitOk = 0x0404,
    Rollback = 0x0405,
    RollbackOk = 0x0406,
    Savepoint = 0x0407,
    SavepointOk = 0x0408,
    ReleaseSavepoint = 0x0409,
    ReleaseSavepointOk = 0x040A,
    RollbackToSavepoint = 0x040B,
    RollbackToSavepointOk = 0x040C,

    // LIVE queries (0x05xx)
    LiveSubscribe = 0x0501,
    LiveSubscribeOk = 0x0502,
    LiveEvent = 0x0503,
    LiveKill = 0x0504,
    LiveKillOk = 0x0505,
    LiveDiff = 0x0506,

    // Graph operations (0x06xx)
    GraphNode = 0x0601,
    GraphEdge = 0x0602,
    GraphPath = 0x0603,
    TraversalStart = 0x0604,
    TraversalStep = 0x0605,
    TraversalEnd = 0x0606,

    // Vector operations (0x07xx)
    VectorData = 0x0701,
    VectorSearchResult = 0x0702,
    VectorSimilarity = 0x0703,

    // Spatial operations (0x08xx)
    SpatialPoint = 0x0801,
    SpatialGeometry = 0x0802,
    SpatialDistance = 0x0803,

    // Metadata (0x09xx)
    InfoRequest = 0x0901,
    InfoResponse = 0x0902,
    CatalogRequest = 0x0903,
    CatalogResponse = 0x0904,

    // Errors (0x0Fxx)
    Error = 0x0F01,
    Warning = 0x0F02,
    Notice = 0x0F03,

    // Unknown
    Unknown = 0xFFFF,
}

impl MessageType {
    pub fn from_u16(value: u16) -> Self {
        match value {
            0x0001 => Self::Hello,
            0x0002 => Self::HelloAck,
            0x0003 => Self::Authenticate,
            0x0004 => Self::AuthenticateOk,
            0x0005 => Self::AuthenticateFail,
            0x0006 => Self::Goodbye,
            0x0007 => Self::GoodbyeAck,
            0x0008 => Self::Ping,
            0x0009 => Self::Pong,
            0x0101 => Self::Query,
            0x0102 => Self::QueryOk,
            0x0103 => Self::QueryError,
            0x0104 => Self::QueryCancel,
            0x0105 => Self::QueryCancelled,
            0x0201 => Self::RowDescription,
            0x0202 => Self::RowData,
            0x0203 => Self::RowsComplete,
            0x0204 => Self::CommandComplete,
            0x0301 => Self::Prepare,
            0x0302 => Self::PrepareOk,
            0x0303 => Self::PrepareError,
            0x0304 => Self::Execute,
            0x0305 => Self::ExecuteOk,
            0x0306 => Self::BindParameters,
            0x0307 => Self::BindOk,
            0x0308 => Self::ClosePrepared,
            0x0309 => Self::ClosePreparedOk,
            0x0401 => Self::Begin,
            0x0402 => Self::BeginOk,
            0x0403 => Self::Commit,
            0x0404 => Self::CommitOk,
            0x0405 => Self::Rollback,
            0x0406 => Self::RollbackOk,
            0x0407 => Self::Savepoint,
            0x0408 => Self::SavepointOk,
            0x0409 => Self::ReleaseSavepoint,
            0x040A => Self::ReleaseSavepointOk,
            0x040B => Self::RollbackToSavepoint,
            0x040C => Self::RollbackToSavepointOk,
            0x0501 => Self::LiveSubscribe,
            0x0502 => Self::LiveSubscribeOk,
            0x0503 => Self::LiveEvent,
            0x0504 => Self::LiveKill,
            0x0505 => Self::LiveKillOk,
            0x0506 => Self::LiveDiff,
            0x0601 => Self::GraphNode,
            0x0602 => Self::GraphEdge,
            0x0603 => Self::GraphPath,
            0x0604 => Self::TraversalStart,
            0x0605 => Self::TraversalStep,
            0x0606 => Self::TraversalEnd,
            0x0701 => Self::VectorData,
            0x0702 => Self::VectorSearchResult,
            0x0703 => Self::VectorSimilarity,
            0x0801 => Self::SpatialPoint,
            0x0802 => Self::SpatialGeometry,
            0x0803 => Self::SpatialDistance,
            0x0901 => Self::InfoRequest,
            0x0902 => Self::InfoResponse,
            0x0903 => Self::CatalogRequest,
            0x0904 => Self::CatalogResponse,
            0x0F01 => Self::Error,
            0x0F02 => Self::Warning,
            0x0F03 => Self::Notice,
            _ => Self::Unknown,
        }
    }

    pub fn to_u16(self) -> u16 {
        self as u16
    }
}

/// OrbitWire frame
#[derive(Debug, Clone)]
pub struct Frame {
    /// Frame flags
    pub flags: FrameFlags,
    /// Stream ID (0 for connection-level messages)
    pub stream_id: u32,
    /// Message type
    pub message_type: MessageType,
    /// Payload data
    pub payload: Bytes,
}

impl Frame {
    /// Create a new frame
    pub fn new(stream_id: u32, message_type: MessageType, payload: Bytes) -> Self {
        Self {
            flags: FrameFlags::default(),
            stream_id,
            message_type,
            payload,
        }
    }

    /// Create a frame with flags
    pub fn with_flags(
        flags: FrameFlags,
        stream_id: u32,
        message_type: MessageType,
        payload: Bytes,
    ) -> Self {
        Self {
            flags,
            stream_id,
            message_type,
            payload,
        }
    }

    /// Create a connection-level frame (stream_id = 0)
    pub fn connection_frame(message_type: MessageType, payload: Bytes) -> Self {
        Self::new(0, message_type, payload)
    }

    /// Encode frame to bytes
    pub fn encode(&self) -> Bytes {
        let payload_len = self.payload.len();
        let mut buf = BytesMut::with_capacity(FRAME_HEADER_SIZE + payload_len);

        // Flags (1 byte)
        buf.put_u8(self.flags.to_byte());
        // Stream ID (4 bytes, big-endian)
        buf.put_u32(self.stream_id);
        // Message Type (2 bytes, big-endian)
        buf.put_u16(self.message_type.to_u16());
        // Payload Length (4 bytes, big-endian)
        buf.put_u32(payload_len as u32);
        // Reserved (3 bytes)
        buf.put_u8(0);
        buf.put_u8(0);
        buf.put_u8(0);
        // Payload
        buf.put_slice(&self.payload);

        buf.freeze()
    }

    /// Decode frame from bytes
    pub fn decode(data: &mut Bytes) -> Result<Self, FrameError> {
        if data.remaining() < FRAME_HEADER_SIZE {
            return Err(FrameError::InsufficientData);
        }

        let flags = FrameFlags::from_byte(data.get_u8());
        let stream_id = data.get_u32();
        let message_type = MessageType::from_u16(data.get_u16());
        let payload_len = data.get_u32() as usize;

        // Skip reserved bytes
        data.advance(3);

        if payload_len > MAX_PAYLOAD_SIZE {
            return Err(FrameError::PayloadTooLarge);
        }

        if data.remaining() < payload_len {
            return Err(FrameError::InsufficientData);
        }

        let payload = data.copy_to_bytes(payload_len);

        Ok(Self {
            flags,
            stream_id,
            message_type,
            payload,
        })
    }

    /// Check if this is the end of stream
    pub fn is_end_stream(&self) -> bool {
        self.flags.end_stream
    }

    /// Check if payload is compressed
    pub fn is_compressed(&self) -> bool {
        self.flags.compressed
    }

    /// Get total frame size
    pub fn total_size(&self) -> usize {
        FRAME_HEADER_SIZE + self.payload.len()
    }
}

/// Frame encoding/decoding errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// Not enough data to decode frame
    InsufficientData,
    /// Payload exceeds maximum size
    PayloadTooLarge,
    /// Invalid message type
    InvalidMessageType,
    /// Compression error
    CompressionError,
    /// Decompression error
    DecompressionError,
    /// Invalid frame structure
    InvalidFrame,
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::InsufficientData => write!(f, "Insufficient data to decode frame"),
            FrameError::PayloadTooLarge => write!(f, "Payload exceeds maximum size"),
            FrameError::InvalidMessageType => write!(f, "Invalid message type"),
            FrameError::CompressionError => write!(f, "Compression error"),
            FrameError::DecompressionError => write!(f, "Decompression error"),
            FrameError::InvalidFrame => write!(f, "Invalid frame structure"),
        }
    }
}

impl std::error::Error for FrameError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_flags() {
        let flags = FrameFlags::new()
            .with_end_stream()
            .with_compression(CompressionType::Lz4)
            .with_ack_required();

        let byte = flags.to_byte();
        let decoded = FrameFlags::from_byte(byte);

        assert!(decoded.end_stream);
        assert!(decoded.compressed);
        assert!(decoded.ack_required);
        assert!(!decoded.continuation);
        assert_eq!(decoded.compression_type, CompressionType::Lz4);
    }

    #[test]
    fn test_frame_encode_decode() {
        let payload = Bytes::from("SELECT * FROM users");
        let frame = Frame::new(1, MessageType::Query, payload.clone());

        let encoded = frame.encode();
        let mut data = encoded;
        let decoded = Frame::decode(&mut data).unwrap();

        assert_eq!(decoded.stream_id, 1);
        assert_eq!(decoded.message_type, MessageType::Query);
        assert_eq!(decoded.payload, payload);
    }

    #[test]
    fn test_connection_frame() {
        let frame = Frame::connection_frame(MessageType::Hello, Bytes::from("hello"));

        assert_eq!(frame.stream_id, 0);
        assert_eq!(frame.message_type, MessageType::Hello);
    }

    #[test]
    fn test_message_type_roundtrip() {
        let types = vec![
            MessageType::Hello,
            MessageType::Query,
            MessageType::RowData,
            MessageType::LiveSubscribe,
            MessageType::GraphPath,
            MessageType::Error,
        ];

        for msg_type in types {
            let encoded = msg_type.to_u16();
            let decoded = MessageType::from_u16(encoded);
            assert_eq!(msg_type, decoded);
        }
    }

    #[test]
    fn test_insufficient_data() {
        let mut data = Bytes::from(vec![0u8; 5]); // Less than header size
        let result = Frame::decode(&mut data);
        assert_eq!(result, Err(FrameError::InsufficientData));
    }
}
