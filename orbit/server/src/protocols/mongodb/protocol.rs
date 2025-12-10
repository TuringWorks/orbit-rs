use bytes::{Buf, BufMut, BytesMut};
use orbit_shared::OrbitError;
use std::io::Cursor;
use tokio_util::codec::{Decoder, Encoder};

// MongoDB Wire Protocol Constants
pub const OP_REPLY: i32 = 1;
pub const OP_UPDATE: i32 = 2001;
pub const OP_INSERT: i32 = 2002;
pub const OP_QUERY: i32 = 2004;
pub const OP_GET_MORE: i32 = 2005;
pub const OP_DELETE: i32 = 2006;
pub const OP_KILL_CURSORS: i32 = 2007;
pub const OP_COMPRESSED: i32 = 2012;
pub const OP_MSG: i32 = 2013;

// Compressor IDs
pub const COMPRESSOR_NOOP: u8 = 0;
pub const COMPRESSOR_SNAPPY: u8 = 1;
pub const COMPRESSOR_ZLIB: u8 = 2;
pub const COMPRESSOR_ZSTD: u8 = 3;

// OP_MSG Flags
pub const MSG_CHECKSUM_PRESENT: u32 = 1 << 0;
pub const MSG_MORE_TO_COME: u32 = 1 << 1;
pub const MSG_EXHAUST_ALLOWED: u32 = 1 << 16;

// OP_MSG Section Kinds
pub const KIND_BODY: u8 = 0;
pub const KIND_DOCUMENT_SEQUENCE: u8 = 1;

#[derive(Debug, Clone)]
pub struct MongoHeader {
    pub message_length: i32,
    pub request_id: i32,
    pub response_to: i32,
    pub op_code: i32,
}

#[derive(Debug, Clone)]
pub enum MongoMessage {
    Query {
        header: MongoHeader,
        flags: i32,
        full_collection_name: String,
        number_to_skip: i32,
        number_to_return: i32,
        query: bson::Document,
        return_fields_selector: Option<bson::Document>,
    },
    Msg {
        header: MongoHeader,
        flag_bits: u32,
        sections: Vec<MsgSection>,
        checksum: Option<u32>,
    },
    Reply {
        header: MongoHeader,
        response_flags: i32,
        cursor_id: i64,
        starting_from: i32,
        number_returned: i32,
        documents: Vec<bson::Document>,
    },
    // Other opcodes can be added as needed
    Unknown {
        header: MongoHeader,
        body: Vec<u8>,
    },
}

#[derive(Debug, Clone)]
pub enum MsgSection {
    Body(bson::Document),
    DocumentSequence {
        identifier: String,
        documents: Vec<bson::Document>,
    },
}

pub struct MongoCodec;

impl Default for MongoCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl MongoCodec {
    pub fn new() -> Self {
        Self
    }
}

impl Decoder for MongoCodec {
    type Item = MongoMessage;
    type Error = OrbitError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 16 {
            return Ok(None);
        }

        let mut cursor = Cursor::new(&src[..]);
        let message_length = cursor.get_i32_le();

        if src.len() < message_length as usize {
            src.reserve(message_length as usize - src.len());
            return Ok(None);
        }

        let request_id = cursor.get_i32_le();
        let response_to = cursor.get_i32_le();
        let op_code = cursor.get_i32_le();

        let header = MongoHeader {
            message_length,
            request_id,
            response_to,
            op_code,
        };

        // Advance buffer to body
        cursor.set_position(16);
        let body_len = message_length as usize - 16;
        let mut body_slice = vec![0u8; body_len];
        std::io::Read::read_exact(&mut cursor, &mut body_slice)
            .map_err(|e| OrbitError::network(format!("IO error: {}", e)))?;

        // Consume bytes from src
        src.advance(message_length as usize);

        match op_code {
            OP_QUERY => {
                let mut body_cursor = Cursor::new(body_slice);
                let flags = body_cursor.get_i32_le();

                // Read CString for collection name
                let mut full_collection_name = String::new();
                loop {
                    let byte = body_cursor.get_u8();
                    if byte == 0 {
                        break;
                    }
                    full_collection_name.push(byte as char);
                }

                let number_to_skip = body_cursor.get_i32_le();
                let number_to_return = body_cursor.get_i32_le();

                // Read query document
                let pos = body_cursor.position();
                let mut reader = &body_cursor.get_ref()[pos as usize..];
                let query = bson::Document::from_reader(&mut reader)
                    .map_err(|e| OrbitError::network(format!("Invalid BSON query: {}", e)))?;

                // Update cursor position after reading BSON
                // BSON document starts with length (i32)
                let query_size_bytes = u32::from_le_bytes(
                    body_cursor.get_ref()[pos as usize..pos as usize + 4]
                        .try_into()
                        .unwrap(),
                ) as u64;

                body_cursor.set_position(pos + query_size_bytes);

                let return_fields_selector =
                    if body_cursor.position() < body_cursor.get_ref().len() as u64 {
                        let pos = body_cursor.position();
                        let mut reader = &body_cursor.get_ref()[pos as usize..];
                        Some(bson::Document::from_reader(&mut reader).map_err(|e| {
                            OrbitError::network(format!("Invalid BSON selector: {}", e))
                        })?)
                    } else {
                        None
                    };

                Ok(Some(MongoMessage::Query {
                    header,
                    flags,
                    full_collection_name,
                    number_to_skip,
                    number_to_return,
                    query,
                    return_fields_selector,
                }))
            }
            OP_MSG => {
                let mut body_cursor = Cursor::new(body_slice);
                let flag_bits = body_cursor.get_u32_le();

                let mut sections = Vec::new();
                
                // Check if checksum is present (bit 0)
                let checksum_present = (flag_bits & MSG_CHECKSUM_PRESENT) != 0;
                
                // Calculate where the sections end
                // If checksum present, last 4 bytes are checksum
                let total_len = body_cursor.get_ref().len() as u64;
                let sections_end = if checksum_present {
                    total_len - 4
                } else {
                    total_len
                };

                while body_cursor.position() < sections_end {
                    let kind = body_cursor.get_u8();
                    match kind {
                        KIND_BODY => {
                            let pos = body_cursor.position();
                            let mut reader = &body_cursor.get_ref()[pos as usize..];
                            let doc = bson::Document::from_reader(&mut reader).map_err(|e| {
                                OrbitError::network(format!("Invalid BSON body: {}", e))
                            })?;

                            let doc_size = u32::from_le_bytes(
                                body_cursor.get_ref()[pos as usize..pos as usize + 4]
                                    .try_into()
                                    .unwrap(),
                            ) as u64;
                            body_cursor.set_position(pos + doc_size);

                            sections.push(MsgSection::Body(doc));
                        }
                        KIND_DOCUMENT_SEQUENCE => {
                            let section_size = body_cursor.get_i32_le();
                            let end_pos = body_cursor.position() + section_size as u64 - 4; // -4 because size includes itself

                            let mut identifier = String::new();
                            loop {
                                let byte = body_cursor.get_u8();
                                if byte == 0 {
                                    break;
                                }
                                identifier.push(byte as char);
                            }

                            let mut documents = Vec::new();
                            while body_cursor.position() < end_pos {
                                let pos = body_cursor.position();
                                let mut reader = &body_cursor.get_ref()[pos as usize..];
                                let doc =
                                    bson::Document::from_reader(&mut reader).map_err(|e| {
                                        OrbitError::network(format!(
                                            "Invalid BSON sequence doc: {}",
                                            e
                                        ))
                                    })?;

                                let doc_size = u32::from_le_bytes(
                                    body_cursor.get_ref()[pos as usize..pos as usize + 4]
                                        .try_into()
                                        .unwrap(),
                                ) as u64;
                                body_cursor.set_position(pos + doc_size);

                                documents.push(doc);
                            }

                            sections.push(MsgSection::DocumentSequence {
                                identifier,
                                documents,
                            });
                        }
                        _ => {
                            return Err(OrbitError::network(format!("Unknown OP_MSG section kind: {}", kind)));
                        }
                    }
                }
                
                let checksum = if checksum_present {
                    body_cursor.set_position(total_len - 4);
                    Some(body_cursor.get_u32_le())
                } else {
                    None
                };

                Ok(Some(MongoMessage::Msg {
                    header,
                    flag_bits,
                    sections,
                    checksum,
                }))
            }
            OP_COMPRESSED => {
                let mut body_cursor = Cursor::new(body_slice);
                let original_opcode = body_cursor.get_i32_le();
                let uncompressed_size = body_cursor.get_i32_le();
                let compressor_id = body_cursor.get_u8();
                
                // Read remaining bytes as compressed data
                let pos = body_cursor.position();
                let compressed_data = &body_cursor.get_ref()[pos as usize..];
                
                let decompressed_data = match compressor_id {
                    COMPRESSOR_NOOP => compressed_data.to_vec(),
                    COMPRESSOR_ZLIB => {
                        use std::io::Read;
                        let mut decoder = flate2::read::ZlibDecoder::new(compressed_data);
                        let mut buf = Vec::with_capacity(uncompressed_size as usize);
                        decoder.read_to_end(&mut buf)
                            .map_err(|e| OrbitError::network(format!("Zlib decompression failed: {}", e)))?;
                        buf
                    }
                    COMPRESSOR_SNAPPY => {
                         // Snappy not yet supported
                         return Err(OrbitError::network("Snappy compression not supported"));
                    }
                     COMPRESSOR_ZSTD => {
                         // Zstd not yet supported
                         return Err(OrbitError::network("Zstd compression not supported"));
                    }
                    _ => return Err(OrbitError::network(format!("Unknown compressor ID: {}", compressor_id))),
                };
                
                if decompressed_data.len() != uncompressed_size as usize {
                     return Err(OrbitError::network(format!("Decompressed size mismatch. Expected {}, got {}", uncompressed_size, decompressed_data.len())));
                }
                
                // Rekindle decoding for the inner message
                // We construct a synthetic buffer with the original header fields but inner body
                // Actually, our Decoder expects the FULL message including header (16 bytes).
                // But we don't have the original header bytes easily available to reconstruct exactly as they were (since length changes).
                // However, our logic separates Header parsing from Body parsing.
                // We can reuse the body parsing logic if we extract it to a helper method.
                // For now, let's just recursively call a helper that processes the body given an opcode.
                
                // Refactoring note: The current structure matches on op_code inside the decode function.
                // We should ideally split this. But for this specific case, we can verify that
                // the inner message structure for specific opcodes works with our parser.
                // Example: OP_QUERY expects body_slice to start with flags.
                // OP_MSG expects body_slice to start with flag_bits.
                // decompressed_data IS that body slice.
                
                // So we can just recursivelly call a body parser.
                // But we can't easily change the structure of `decode` without a big diff.
                // Instead, let's restart the loop? No, decode parses one item.
                
                // Let's create a synthetic BytesMut with a FAKE header around the decompressed body
                // matching `original_opcode` and correct length.
                // Then call `decode` on it.
                
                let mut inner_src = BytesMut::with_capacity(16 + decompressed_data.len());
                // New Length
                inner_src.put_i32_le((16 + decompressed_data.len()) as i32);
                // Original Request ID (from wrapper? or is it in compressed? Spec says wrapper usually used)
                inner_src.put_i32_le(header.request_id);
                inner_src.put_i32_le(header.response_to);
                inner_src.put_i32_le(original_opcode);
                inner_src.extend_from_slice(&decompressed_data);
                
                // Recurse
                // Note: This relies on `self` not having state that breaks on recursion, which is true (stateless decoder).
                let mut inner_decoder = MongoCodec::new(); // Stateless
                inner_decoder.decode(&mut inner_src)
            }
            _ => Ok(Some(MongoMessage::Unknown {
                header,
                body: body_slice,
            })),
        }
    }
}

impl Encoder<MongoMessage> for MongoCodec {
    type Error = OrbitError;

    fn encode(&mut self, item: MongoMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match item {
            MongoMessage::Reply {
                header,
                response_flags,
                cursor_id,
                starting_from,
                number_returned,
                documents,
            } => {
                // Calculate length
                let mut body_len = 4 + 8 + 4 + 4; // flags, cursor_id, starting_from, number_returned
                let mut doc_bytes = Vec::new();
                for doc in &documents {
                    let mut buf = Vec::new();
                    doc.to_writer(&mut buf)
                        .map_err(|e| OrbitError::network(format!("BSON encode error: {}", e)))?;
                    doc_bytes.extend_from_slice(&buf);
                }
                body_len += doc_bytes.len();

                let total_len = 16 + body_len;

                dst.reserve(total_len);
                dst.put_i32_le(total_len as i32);
                dst.put_i32_le(header.request_id);
                dst.put_i32_le(header.response_to);
                dst.put_i32_le(OP_REPLY);

                dst.put_i32_le(response_flags);
                dst.put_i64_le(cursor_id);
                dst.put_i32_le(starting_from);
                dst.put_i32_le(number_returned);
                dst.put_slice(&doc_bytes);
            }
            MongoMessage::Msg {
                header,
                flag_bits,
                sections,
                checksum,
            } => {
                let mut body_buf = BytesMut::new();
                body_buf.put_u32_le(flag_bits);

                for section in sections {
                    match section {
                        MsgSection::Body(doc) => {
                            body_buf.put_u8(KIND_BODY);
                            let mut buf = Vec::new();
                            doc.to_writer(&mut buf).map_err(|e| {
                                OrbitError::network(format!("BSON encode error: {}", e))
                            })?;
                            body_buf.put_slice(&buf);
                        }
                        MsgSection::DocumentSequence {
                            identifier,
                            documents,
                        } => {
                            body_buf.put_u8(KIND_DOCUMENT_SEQUENCE);

                            // We need to calculate size first
                            let mut seq_buf = BytesMut::new();
                            seq_buf.put_slice(identifier.as_bytes());
                            seq_buf.put_u8(0); // CString null terminator

                            for doc in documents {
                                let mut buf = Vec::new();
                                doc.to_writer(&mut buf).map_err(|e| {
                                    OrbitError::network(format!("BSON encode error: {}", e))
                                })?;
                                seq_buf.put_slice(&buf);
                            }

                            let size = 4 + seq_buf.len() as i32;
                            body_buf.put_i32_le(size);
                            body_buf.put_slice(&seq_buf);
                        }
                    }
                }

                if let Some(crc) = checksum {
                    body_buf.put_u32_le(crc);
                }

                let total_len = 16 + body_buf.len();

                dst.reserve(total_len);
                dst.put_i32_le(total_len as i32);
                dst.put_i32_le(header.request_id);
                dst.put_i32_le(header.response_to);
                dst.put_i32_le(OP_MSG);
                dst.put_slice(&body_buf);
            }
            _ => return Err(OrbitError::network("Unsupported message type for encoding")),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============ Constants Tests ============

    #[test]
    fn test_opcode_constants() {
        assert_eq!(OP_REPLY, 1);
        assert_eq!(OP_UPDATE, 2001);
        assert_eq!(OP_INSERT, 2002);
        assert_eq!(OP_QUERY, 2004);
        assert_eq!(OP_GET_MORE, 2005);
        assert_eq!(OP_DELETE, 2006);
        assert_eq!(OP_KILL_CURSORS, 2007);
        assert_eq!(OP_MSG, 2013);
    }

    #[test]
    fn test_section_kind_constants() {
        assert_eq!(KIND_BODY, 0);
        assert_eq!(KIND_DOCUMENT_SEQUENCE, 1);
    }

    // ============ MongoHeader Tests ============

    #[test]
    fn test_mongo_header_creation() {
        let header = MongoHeader {
            message_length: 100,
            request_id: 1,
            response_to: 0,
            op_code: OP_MSG,
        };
        assert_eq!(header.message_length, 100);
        assert_eq!(header.request_id, 1);
        assert_eq!(header.response_to, 0);
        assert_eq!(header.op_code, OP_MSG);
    }

    #[test]
    fn test_mongo_header_clone() {
        let header = MongoHeader {
            message_length: 50,
            request_id: 123,
            response_to: 456,
            op_code: OP_QUERY,
        };
        let cloned = header.clone();
        assert_eq!(header.message_length, cloned.message_length);
        assert_eq!(header.request_id, cloned.request_id);
        assert_eq!(header.response_to, cloned.response_to);
        assert_eq!(header.op_code, cloned.op_code);
    }

    // ============ MsgSection Tests ============

    #[test]
    fn test_msg_section_body() {
        let doc = bson::doc! { "test": "value" };
        let section = MsgSection::Body(doc.clone());
        match section {
            MsgSection::Body(d) => assert_eq!(d.get_str("test").unwrap(), "value"),
            _ => panic!("Expected Body section"),
        }
    }

    #[test]
    fn test_msg_section_document_sequence() {
        let docs = vec![bson::doc! { "a": 1 }, bson::doc! { "b": 2 }];
        let section = MsgSection::DocumentSequence {
            identifier: "documents".to_string(),
            documents: docs.clone(),
        };
        match section {
            MsgSection::DocumentSequence {
                identifier,
                documents,
            } => {
                assert_eq!(identifier, "documents");
                assert_eq!(documents.len(), 2);
            }
            _ => panic!("Expected DocumentSequence section"),
        }
    }

    // ============ MongoMessage Tests ============

    #[test]
    fn test_mongo_message_reply() {
        let header = MongoHeader {
            message_length: 100,
            request_id: 1,
            response_to: 2,
            op_code: OP_REPLY,
        };
        let msg = MongoMessage::Reply {
            header: header.clone(),
            response_flags: 0,
            cursor_id: 0,
            starting_from: 0,
            number_returned: 1,
            documents: vec![bson::doc! { "ok": 1 }],
        };

        match msg {
            MongoMessage::Reply {
                number_returned,
                documents,
                ..
            } => {
                assert_eq!(number_returned, 1);
                assert_eq!(documents.len(), 1);
            }
            _ => panic!("Expected Reply message"),
        }
    }

    #[test]
    fn test_mongo_message_msg() {
        let header = MongoHeader {
            message_length: 100,
            request_id: 1,
            response_to: 0,
            op_code: OP_MSG,
        };
        let msg = MongoMessage::Msg {
            header,
            flag_bits: 0,
            sections: vec![MsgSection::Body(bson::doc! { "find": "test" })],
            checksum: None,
        };

        match msg {
            MongoMessage::Msg {
                sections, checksum, ..
            } => {
                assert_eq!(sections.len(), 1);
                assert!(checksum.is_none());
            }
            _ => panic!("Expected Msg message"),
        }
    }

    #[test]
    fn test_mongo_message_query() {
        let header = MongoHeader {
            message_length: 100,
            request_id: 1,
            response_to: 0,
            op_code: OP_QUERY,
        };
        let msg = MongoMessage::Query {
            header,
            flags: 0,
            full_collection_name: "test.collection".to_string(),
            number_to_skip: 0,
            number_to_return: 10,
            query: bson::doc! { "field": "value" },
            return_fields_selector: None,
        };

        match msg {
            MongoMessage::Query {
                full_collection_name,
                number_to_return,
                query,
                ..
            } => {
                assert_eq!(full_collection_name, "test.collection");
                assert_eq!(number_to_return, 10);
                assert_eq!(query.get_str("field").unwrap(), "value");
            }
            _ => panic!("Expected Query message"),
        }
    }

    #[test]
    fn test_mongo_message_unknown() {
        let header = MongoHeader {
            message_length: 20,
            request_id: 1,
            response_to: 0,
            op_code: 9999, // Unknown opcode
        };
        let msg = MongoMessage::Unknown {
            header,
            body: vec![1, 2, 3, 4],
        };

        match msg {
            MongoMessage::Unknown { body, .. } => {
                assert_eq!(body, vec![1, 2, 3, 4]);
            }
            _ => panic!("Expected Unknown message"),
        }
    }

    // ============ MongoCodec Tests ============

    #[test]
    fn test_mongo_codec_new() {
        let _codec = MongoCodec::new();
    }

    #[test]
    fn test_mongo_codec_default() {
        let _codec = MongoCodec::default();
    }

    #[test]
    fn test_decode_incomplete_header() {
        let mut codec = MongoCodec::new();
        let mut buf = BytesMut::from(&[0u8; 10][..]); // Less than 16 bytes
        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_decode_incomplete_message() {
        let mut codec = MongoCodec::new();
        let mut buf = BytesMut::new();
        // Write header indicating 100 byte message, but only provide 16
        buf.put_i32_le(100); // message_length
        buf.put_i32_le(1); // request_id
        buf.put_i32_le(0); // response_to
        buf.put_i32_le(OP_MSG); // op_code

        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_encode_reply() {
        let mut codec = MongoCodec::new();
        let header = MongoHeader {
            message_length: 0, // Will be calculated
            request_id: 1,
            response_to: 2,
            op_code: OP_REPLY,
        };
        let msg = MongoMessage::Reply {
            header,
            response_flags: 8, // AWAIT_CAPABLE
            cursor_id: 0,
            starting_from: 0,
            number_returned: 1,
            documents: vec![bson::doc! { "ok": 1 }],
        };

        let mut dst = BytesMut::new();
        codec.encode(msg, &mut dst).unwrap();

        // Verify header
        let mut cursor = Cursor::new(&dst[..]);
        let msg_len = cursor.get_i32_le();
        assert!(msg_len > 16); // At least header size

        let req_id = cursor.get_i32_le();
        assert_eq!(req_id, 1);

        let resp_to = cursor.get_i32_le();
        assert_eq!(resp_to, 2);

        let op_code = cursor.get_i32_le();
        assert_eq!(op_code, OP_REPLY);
    }

    #[test]
    fn test_encode_msg_with_body() {
        let mut codec = MongoCodec::new();
        let header = MongoHeader {
            message_length: 0,
            request_id: 10,
            response_to: 5,
            op_code: OP_MSG,
        };
        let msg = MongoMessage::Msg {
            header,
            flag_bits: 0,
            sections: vec![MsgSection::Body(bson::doc! {
                "ok": 1,
                "cursor": {
                    "firstBatch": [],
                    "id": 0_i64,
                    "ns": "test.collection"
                }
            })],
            checksum: None,
        };

        let mut dst = BytesMut::new();
        codec.encode(msg, &mut dst).unwrap();

        // Verify it's a valid message
        let mut cursor = Cursor::new(&dst[..]);
        let msg_len = cursor.get_i32_le();
        assert!(msg_len > 16);

        cursor.get_i32_le(); // request_id
        cursor.get_i32_le(); // response_to
        let op_code = cursor.get_i32_le();
        assert_eq!(op_code, OP_MSG);

        // Verify flag_bits
        let flag_bits = cursor.get_u32_le();
        assert_eq!(flag_bits, 0);

        // Verify section kind
        let kind = cursor.get_u8();
        assert_eq!(kind, KIND_BODY);
    }

    #[test]
    fn test_encode_msg_with_document_sequence() {
        let mut codec = MongoCodec::new();
        let header = MongoHeader {
            message_length: 0,
            request_id: 20,
            response_to: 10,
            op_code: OP_MSG,
        };
        let msg = MongoMessage::Msg {
            header,
            flag_bits: 0,
            sections: vec![MsgSection::DocumentSequence {
                identifier: "documents".to_string(),
                documents: vec![bson::doc! { "a": 1 }, bson::doc! { "b": 2 }],
            }],
            checksum: None,
        };

        let mut dst = BytesMut::new();
        codec.encode(msg, &mut dst).unwrap();

        // Just verify it encodes without error
        assert!(dst.len() > 16);
    }

    #[test]
    fn test_encode_msg_with_checksum() {
        let mut codec = MongoCodec::new();
        let header = MongoHeader {
            message_length: 0,
            request_id: 30,
            response_to: 15,
            op_code: OP_MSG,
        };
        let msg = MongoMessage::Msg {
            header,
            flag_bits: 1, // checksumPresent flag
            sections: vec![MsgSection::Body(bson::doc! { "ping": 1 })],
            checksum: Some(0xDEADBEEF),
        };

        let mut dst = BytesMut::new();
        codec.encode(msg, &mut dst).unwrap();

        // Verify checksum is at the end
        assert!(dst.len() > 20);
    }

    #[test]
    fn test_encode_unsupported_message_type() {
        let mut codec = MongoCodec::new();
        let header = MongoHeader {
            message_length: 0,
            request_id: 1,
            response_to: 0,
            op_code: OP_QUERY,
        };
        let msg = MongoMessage::Query {
            header,
            flags: 0,
            full_collection_name: "test.col".to_string(),
            number_to_skip: 0,
            number_to_return: 10,
            query: bson::doc! {},
            return_fields_selector: None,
        };

        let mut dst = BytesMut::new();
        let result = codec.encode(msg, &mut dst);
        assert!(result.is_err());
    }

    // ============ Roundtrip Tests ============

    #[test]
    fn test_roundtrip_op_msg() {
        let mut codec = MongoCodec::new();

        // Create and encode a message
        let original_header = MongoHeader {
            message_length: 0,
            request_id: 42,
            response_to: 21,
            op_code: OP_MSG,
        };
        let original = MongoMessage::Msg {
            header: original_header.clone(),
            flag_bits: 0,
            sections: vec![MsgSection::Body(bson::doc! {
                "ismaster": 1
            })],
            checksum: None,
        };

        let mut encoded = BytesMut::new();
        codec.encode(original, &mut encoded).unwrap();

        // Decode it back
        let decoded = codec.decode(&mut encoded).unwrap().unwrap();

        match decoded {
            MongoMessage::Msg {
                header,
                flag_bits,
                sections,
                ..
            } => {
                assert_eq!(header.request_id, 42);
                assert_eq!(header.response_to, 21);
                assert_eq!(header.op_code, OP_MSG);
                assert_eq!(flag_bits, 0);
                assert_eq!(sections.len(), 1);
                match &sections[0] {
                    MsgSection::Body(doc) => {
                        assert_eq!(doc.get_i32("ismaster").unwrap(), 1);
                    }
                    _ => panic!("Expected Body section"),
                }
            }
            _ => panic!("Expected Msg message"),
        }
    }

    // ============ Edge Case Tests ============

    #[test]
    fn test_empty_documents_reply() {
        let mut codec = MongoCodec::new();
        let header = MongoHeader {
            message_length: 0,
            request_id: 1,
            response_to: 0,
            op_code: OP_REPLY,
        };
        let msg = MongoMessage::Reply {
            header,
            response_flags: 0,
            cursor_id: 0,
            starting_from: 0,
            number_returned: 0,
            documents: vec![],
        };

        let mut dst = BytesMut::new();
        codec.encode(msg, &mut dst).unwrap();

        // Header + response_flags(4) + cursor_id(8) + starting_from(4) + number_returned(4)
        // = 16 + 20 = 36 bytes
        assert_eq!(dst.len(), 36);
    }

    #[test]
    fn test_multiple_documents_reply() {
        let mut codec = MongoCodec::new();
        let header = MongoHeader {
            message_length: 0,
            request_id: 1,
            response_to: 0,
            op_code: OP_REPLY,
        };
        let msg = MongoMessage::Reply {
            header,
            response_flags: 0,
            cursor_id: 12345,
            starting_from: 0,
            number_returned: 3,
            documents: vec![
                bson::doc! { "a": 1 },
                bson::doc! { "b": 2 },
                bson::doc! { "c": 3 },
            ],
        };

        let mut dst = BytesMut::new();
        codec.encode(msg, &mut dst).unwrap();
        assert!(dst.len() > 36);
    }

    #[test]
    fn test_large_document() {
        let mut codec = MongoCodec::new();
        let header = MongoHeader {
            message_length: 0,
            request_id: 1,
            response_to: 0,
            op_code: OP_MSG,
        };

        // Create a document with a large string
        let large_string = "x".repeat(10000);
        let msg = MongoMessage::Msg {
            header,
            flag_bits: 0,
            sections: vec![MsgSection::Body(bson::doc! {
                "data": large_string
            })],
            checksum: None,
        };

        let mut dst = BytesMut::new();
        codec.encode(msg, &mut dst).unwrap();
        assert!(dst.len() > 10000);
    }

    #[test]
    fn test_nested_document() {
        let mut codec = MongoCodec::new();
        let header = MongoHeader {
            message_length: 0,
            request_id: 1,
            response_to: 0,
            op_code: OP_MSG,
        };

        let nested = bson::doc! {
            "level1": {
                "level2": {
                    "level3": {
                        "value": 42
                    }
                }
            }
        };

        let msg = MongoMessage::Msg {
            header,
            flag_bits: 0,
            sections: vec![MsgSection::Body(nested)],
            checksum: None,
        };

        let mut dst = BytesMut::new();
        codec.encode(msg, &mut dst).unwrap();

        // Decode and verify structure
        let decoded = codec.decode(&mut dst).unwrap().unwrap();
        match decoded {
            MongoMessage::Msg { sections, .. } => match &sections[0] {
                MsgSection::Body(doc) => {
                    let level1 = doc.get_document("level1").unwrap();
                    let level2 = level1.get_document("level2").unwrap();
                    let level3 = level2.get_document("level3").unwrap();
                    assert_eq!(level3.get_i32("value").unwrap(), 42);
                }
                _ => panic!("Expected Body section"),
            },
            _ => panic!("Expected Msg message"),
        }
    }
}
