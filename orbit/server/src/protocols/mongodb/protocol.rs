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
pub const OP_MSG: i32 = 2013;

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
                let checksum: Option<u32> = None;

                while body_cursor.position() < body_cursor.get_ref().len() as u64 {
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
                            // Assume checksum if it's the last 4 bytes?
                            // Or just break if unknown kind
                            if body_cursor.get_ref().len() as u64 - body_cursor.position() == 4 {
                                // Rewind 1 byte (kind) and read checksum?
                                // Actually checksum is a section with kind? No, it's optional at the end.
                                // But the spec says sections are Type (1 byte) + Payload.
                                // If we encountered a byte that is not 0 or 1, and we are at the end...
                                // Let's just ignore for now.
                                break;
                            }
                            break;
                        }
                    }
                }

                Ok(Some(MongoMessage::Msg {
                    header,
                    flag_bits,
                    sections,
                    checksum,
                }))
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
