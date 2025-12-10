//! OrbitWire codec for tokio
//!
//! Implements tokio_util::codec traits for frame encoding/decoding

use super::frame::{Frame, FrameError, FRAME_HEADER_SIZE, MAX_PAYLOAD_SIZE};
use super::{CompressionType, MAGIC_BYTES, PROTOCOL_VERSION};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

/// OrbitWire frame codec
pub struct OrbitWireCodec {
    /// Maximum frame size
    max_frame_size: usize,
    /// Default compression for outgoing frames
    default_compression: CompressionType,
    /// State of codec
    state: CodecState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodecState {
    /// Waiting for magic bytes (handshake)
    WaitingForMagic,
    /// Normal operation
    Active,
}

impl OrbitWireCodec {
    /// Create a new codec
    pub fn new() -> Self {
        Self {
            max_frame_size: MAX_PAYLOAD_SIZE + FRAME_HEADER_SIZE,
            default_compression: CompressionType::None,
            state: CodecState::WaitingForMagic,
        }
    }

    /// Create a codec with custom max frame size
    pub fn with_max_frame_size(mut self, size: usize) -> Self {
        self.max_frame_size = size;
        self
    }

    /// Create a codec with default compression
    pub fn with_compression(mut self, compression: CompressionType) -> Self {
        self.default_compression = compression;
        self
    }

    /// Create a server codec (already in active state)
    pub fn server() -> Self {
        Self {
            max_frame_size: MAX_PAYLOAD_SIZE + FRAME_HEADER_SIZE,
            default_compression: CompressionType::None,
            state: CodecState::WaitingForMagic,
        }
    }

    /// Create a client codec
    pub fn client() -> Self {
        Self {
            max_frame_size: MAX_PAYLOAD_SIZE + FRAME_HEADER_SIZE,
            default_compression: CompressionType::None,
            state: CodecState::Active, // Client starts active
        }
    }

    /// Check if handshake is complete
    pub fn is_active(&self) -> bool {
        self.state == CodecState::Active
    }

    /// Compress payload if needed
    fn compress(&self, data: &[u8], compression: CompressionType) -> Result<Bytes, CodecError> {
        match compression {
            CompressionType::None => Ok(Bytes::copy_from_slice(data)),
            CompressionType::Lz4 => {
                // LZ4 compression
                let compressed = lz4_flex::compress_prepend_size(data);
                Ok(Bytes::from(compressed))
            }
            CompressionType::Zstd => {
                // Zstd compression - would need zstd crate
                // For now, return uncompressed
                Ok(Bytes::copy_from_slice(data))
            }
            CompressionType::Snappy => {
                // Snappy compression - would need snap crate
                // For now, return uncompressed
                Ok(Bytes::copy_from_slice(data))
            }
        }
    }

    /// Decompress payload if needed
    fn decompress(&self, data: &[u8], compression: CompressionType) -> Result<Bytes, CodecError> {
        match compression {
            CompressionType::None => Ok(Bytes::copy_from_slice(data)),
            CompressionType::Lz4 => {
                let decompressed = lz4_flex::decompress_size_prepended(data)
                    .map_err(|_| CodecError::DecompressionError)?;
                Ok(Bytes::from(decompressed))
            }
            CompressionType::Zstd => {
                // Zstd decompression
                Ok(Bytes::copy_from_slice(data))
            }
            CompressionType::Snappy => {
                // Snappy decompression
                Ok(Bytes::copy_from_slice(data))
            }
        }
    }
}

impl Default for OrbitWireCodec {
    fn default() -> Self {
        Self::new()
    }
}

/// Codec errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// Invalid magic bytes
    InvalidMagic,
    /// Protocol version mismatch
    VersionMismatch { expected: u8, got: u8 },
    /// Frame too large
    FrameTooLarge,
    /// Decompression failed
    DecompressionError,
    /// Compression failed
    CompressionError,
    /// IO error
    IoError(String),
    /// Frame error
    FrameError(FrameError),
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecError::InvalidMagic => write!(f, "Invalid magic bytes"),
            CodecError::VersionMismatch { expected, got } => {
                write!(f, "Version mismatch: expected {}, got {}", expected, got)
            }
            CodecError::FrameTooLarge => write!(f, "Frame too large"),
            CodecError::DecompressionError => write!(f, "Decompression error"),
            CodecError::CompressionError => write!(f, "Compression error"),
            CodecError::IoError(e) => write!(f, "IO error: {}", e),
            CodecError::FrameError(e) => write!(f, "Frame error: {}", e),
        }
    }
}

impl std::error::Error for CodecError {}

impl From<std::io::Error> for CodecError {
    fn from(err: std::io::Error) -> Self {
        CodecError::IoError(err.to_string())
    }
}

impl From<FrameError> for CodecError {
    fn from(err: FrameError) -> Self {
        CodecError::FrameError(err)
    }
}

impl Decoder for OrbitWireCodec {
    type Item = Frame;
    type Error = CodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Handle handshake state
        if self.state == CodecState::WaitingForMagic {
            if src.len() < 5 {
                return Ok(None); // Need more data
            }

            // Check magic bytes
            if src[..4] != MAGIC_BYTES {
                return Err(CodecError::InvalidMagic);
            }

            // Check protocol version
            let version = src[4];
            if version != PROTOCOL_VERSION {
                return Err(CodecError::VersionMismatch {
                    expected: PROTOCOL_VERSION,
                    got: version,
                });
            }

            // Consume handshake bytes
            src.advance(5);
            self.state = CodecState::Active;
        }

        // Need at least header size
        if src.len() < FRAME_HEADER_SIZE {
            return Ok(None);
        }

        // Peek at payload length (bytes 7-10)
        let payload_len = u32::from_be_bytes([src[7], src[8], src[9], src[10]]) as usize;

        // Check frame size
        let total_len = FRAME_HEADER_SIZE + payload_len;
        if total_len > self.max_frame_size {
            return Err(CodecError::FrameTooLarge);
        }

        // Need complete frame
        if src.len() < total_len {
            return Ok(None);
        }

        // Decode frame
        let frame_bytes = src.split_to(total_len);
        let mut data = frame_bytes.freeze();
        let frame = Frame::decode(&mut data)?;

        // Decompress if needed
        if frame.flags.compressed {
            let decompressed = self.decompress(&frame.payload, frame.flags.compression_type)?;
            return Ok(Some(Frame {
                flags: frame.flags,
                stream_id: frame.stream_id,
                message_type: frame.message_type,
                payload: decompressed,
            }));
        }

        Ok(Some(frame))
    }
}

impl Encoder<Frame> for OrbitWireCodec {
    type Error = CodecError;

    fn encode(&mut self, frame: Frame, dst: &mut BytesMut) -> Result<(), Self::Error> {
        // For first frame on client, send magic bytes
        if self.state == CodecState::WaitingForMagic {
            dst.put_slice(&MAGIC_BYTES);
            dst.put_u8(PROTOCOL_VERSION);
            self.state = CodecState::Active;
        }

        // Optionally compress payload
        let (payload, flags) = if self.default_compression != CompressionType::None
            && frame.payload.len() > 1024
        // Only compress if > 1KB
        {
            let compressed = self.compress(&frame.payload, self.default_compression)?;
            if compressed.len() < frame.payload.len() {
                // Compression helped
                let mut new_flags = frame.flags;
                new_flags.compressed = true;
                new_flags.compression_type = self.default_compression;
                (compressed, new_flags)
            } else {
                // Compression didn't help, use original
                (frame.payload.clone(), frame.flags)
            }
        } else {
            (frame.payload.clone(), frame.flags)
        };

        // Check size
        if FRAME_HEADER_SIZE + payload.len() > self.max_frame_size {
            return Err(CodecError::FrameTooLarge);
        }

        // Create frame with potentially compressed payload
        let output_frame = Frame {
            flags,
            stream_id: frame.stream_id,
            message_type: frame.message_type,
            payload,
        };

        // Encode frame
        let encoded = output_frame.encode();
        dst.put_slice(&encoded);

        Ok(())
    }
}

/// Handshake message for initial connection
#[derive(Debug, Clone)]
pub struct Handshake {
    pub magic: [u8; 4],
    pub version: u8,
}

impl Handshake {
    /// Create a new handshake
    pub fn new() -> Self {
        Self {
            magic: MAGIC_BYTES,
            version: PROTOCOL_VERSION,
        }
    }

    /// Encode handshake to bytes
    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(5);
        buf.put_slice(&self.magic);
        buf.put_u8(self.version);
        buf.freeze()
    }

    /// Decode handshake from bytes
    pub fn decode(data: &mut Bytes) -> Result<Self, CodecError> {
        if data.remaining() < 5 {
            return Err(CodecError::IoError("Insufficient data".to_string()));
        }

        let mut magic = [0u8; 4];
        data.copy_to_slice(&mut magic);

        if magic != MAGIC_BYTES {
            return Err(CodecError::InvalidMagic);
        }

        let version = data.get_u8();
        if version != PROTOCOL_VERSION {
            return Err(CodecError::VersionMismatch {
                expected: PROTOCOL_VERSION,
                got: version,
            });
        }

        Ok(Self { magic, version })
    }
}

impl Default for Handshake {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::orbitwire::frame::MessageType;

    #[test]
    fn test_handshake() {
        let handshake = Handshake::new();
        let encoded = handshake.encode();
        let mut data = encoded;
        let decoded = Handshake::decode(&mut data).unwrap();
        assert_eq!(decoded.magic, MAGIC_BYTES);
        assert_eq!(decoded.version, PROTOCOL_VERSION);
    }

    #[test]
    fn test_codec_encode_decode() {
        let mut codec = OrbitWireCodec::client();
        let mut buf = BytesMut::new();

        // Add handshake first (client sends magic bytes on connect)
        let handshake = Handshake::new();
        buf.put_slice(&handshake.encode());

        let frame = Frame::new(1, MessageType::Query, Bytes::from("SELECT 1"));

        // Encode frame after handshake
        codec.encode(frame.clone(), &mut buf).unwrap();

        // Should have magic bytes (5) + frame
        let frame = Frame::new(1, MessageType::Query, Bytes::from("SELECT 1"));

        // Encode
        codec.encode(frame.clone(), &mut buf).unwrap();

        // Should have magic bytes + frame
        assert!(buf.len() > 5);

        // Create server codec and decode
        let mut server_codec = OrbitWireCodec::server();
        let decoded = server_codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(decoded.stream_id, 1);
        assert_eq!(decoded.message_type, MessageType::Query);
        assert_eq!(decoded.payload, Bytes::from("SELECT 1"));
    }

    #[test]
    fn test_invalid_magic() {
        let mut codec = OrbitWireCodec::server();
        let mut buf = BytesMut::from(&[0x00, 0x00, 0x00, 0x00, 0x01][..]);

        let result = codec.decode(&mut buf);
        assert!(matches!(result, Err(CodecError::InvalidMagic)));
    }

    #[test]
    fn test_version_mismatch() {
        let mut codec = OrbitWireCodec::server();
        let mut buf = BytesMut::new();
        buf.put_slice(&MAGIC_BYTES);
        buf.put_u8(99); // Invalid version

        let result = codec.decode(&mut buf);
        assert!(matches!(
            result,
            Err(CodecError::VersionMismatch {
                expected: 1,
                got: 99
            })
        ));
    }
}
