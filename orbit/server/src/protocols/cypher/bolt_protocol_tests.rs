
#[cfg(test)]
mod tests {
    use crate::protocols::cypher::bolt_protocol::{BoltProtocolHandler, PackStreamDecoder};
    use crate::protocols::cypher::storage::CypherStorageProvider;
    use crate::protocols::cypher::types::{GraphNode, GraphRelationship};
    use crate::protocols::error::ProtocolResult;
    use async_trait::async_trait;
    use bytes::BufMut;
    use serde_json::Value;
    use std::sync::Arc;

    // Mock stream for testing
    struct MockStream {
        read_data: Vec<u8>,
        write_data: Vec<u8>,
    }

    impl MockStream {
        fn new(read_data: Vec<u8>) -> Self {
            Self {
                read_data,
                write_data: Vec::new(),
            }
        }
    }

    impl tokio::io::AsyncRead for MockStream {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            let self_mut = self.get_mut();
            let len = std::cmp::min(buf.remaining(), self_mut.read_data.len());
            buf.put_slice(&self_mut.read_data[..len]);
            self_mut.read_data = self_mut.read_data[len..].to_vec();
            std::task::Poll::Ready(Ok(()))
        }
    }

    impl tokio::io::AsyncWrite for MockStream {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> std::task::Poll<Result<usize, std::io::Error>> {
            self.get_mut().write_data.extend_from_slice(buf);
            std::task::Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), std::io::Error>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), std::io::Error>> {
            std::task::Poll::Ready(Ok(()))
        }
    }

    // Mock storage for testing
    struct MockStorage;

    #[async_trait]
    impl CypherStorageProvider for MockStorage {
        async fn initialize(&self) -> ProtocolResult<()> {
            Ok(())
        }
        async fn store_node(&self, _node: GraphNode) -> ProtocolResult<()> {
            Ok(())
        }
        async fn get_node(&self, _node_id: &str) -> ProtocolResult<Option<GraphNode>> {
            Ok(None)
        }
        async fn get_all_nodes(&self) -> ProtocolResult<Vec<GraphNode>> {
            Ok(vec![])
        }
        async fn store_relationship(&self, _rel: GraphRelationship) -> ProtocolResult<()> {
            Ok(())
        }
        async fn get_relationship(&self, _rel_id: &str) -> ProtocolResult<Option<GraphRelationship>> {
            Ok(None)
        }
        async fn get_all_relationships(&self) -> ProtocolResult<Vec<GraphRelationship>> {
            Ok(vec![])
        }
        async fn shutdown(&self) -> ProtocolResult<()> {
            Ok(())
        }
    }


    fn create_hello_message() -> Vec<u8> {
        let mut buf = bytes::BytesMut::new();
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("user_agent".to_string(), Value::String("test-client/1.0".to_string()));
        metadata.insert("scheme".to_string(), Value::String("basic".to_string()));
        metadata.insert("principal".to_string(), Value::String("neo4j".to_string()));
        metadata.insert("credentials".to_string(), Value::String("password".to_string()));

        let mut routing = std::collections::HashMap::new();
        routing.insert("policy".to_string(), Value::String("leader".to_string()));
        let mut routing_map = serde_json::Map::new();
        routing_map.insert("policy".to_string(), Value::String("leader".to_string()));
        metadata.insert("routing".to_string(), Value::Object(routing_map));

        // Manually encode the HELLO message
        // Structure: 1 field, signature 0x01 (HELLO)
        buf.put_u8(0xB1); // Tiny Struct (1 field)
        buf.put_u8(0x01); // HELLO signature

        // The single field is a map
        let map_len = metadata.len();
        if map_len < 16 {
            buf.put_u8(0xA0 + map_len as u8); // Tiny Map
        } else {
            // Handle larger maps if needed
        }

        for (key, value) in &metadata {
            // Encode key (string)
            let key_bytes = key.as_bytes();
            if key_bytes.len() < 16 {
                buf.put_u8(0x80 + key_bytes.len() as u8);
            } else {
                // Handle larger strings if needed
            }
            buf.put_slice(key_bytes);

            // Encode value
            match value {
                Value::String(s) => {
                    let s_bytes = s.as_bytes();
                    if s_bytes.len() < 16 {
                        buf.put_u8(0x80 + s_bytes.len() as u8);
                    } else {
                        // Handle larger strings
                    }
                    buf.put_slice(s_bytes);
                }
                Value::Object(obj) => {
                    let obj_len = obj.len();
                     if obj_len < 16 {
                        buf.put_u8(0xA0 + obj_len as u8); // Tiny Map
                    } else {
                        // Handle larger maps if needed
                    }
                    for (k, v) in obj {
                         let k_bytes = k.as_bytes();
                        if k_bytes.len() < 16 {
                            buf.put_u8(0x80 + k_bytes.len() as u8);
                        } else {
                            // Handle larger strings if needed
                        }
                        buf.put_slice(k_bytes);

                        if let Value::String(s) = v {
                            let s_bytes = s.as_bytes();
                            if s_bytes.len() < 16 {
                                buf.put_u8(0x80 + s_bytes.len() as u8);
                            } else {
                                // Handle larger strings
                            }
                            buf.put_slice(s_bytes);
                        }
                    }
                }
                _ => {}
            }
        }

        let mut message_with_chunk_header = Vec::new();
        let len = buf.len() as u16;
        message_with_chunk_header.extend_from_slice(&len.to_be_bytes());
        message_with_chunk_header.extend_from_slice(&buf);
        // End of message marker
        message_with_chunk_header.extend_from_slice(&[0x00, 0x00]);

        message_with_chunk_header
    }


    #[tokio::test]
    async fn test_handle_hello() {
        // 1. Setup
        let hello_message = create_hello_message();
        let mut stream = MockStream::new(hello_message);

        #[cfg(not(feature = "storage-rocksdb"))]
        let mut handler = BoltProtocolHandler::new_without_storage();
        #[cfg(feature = "storage-rocksdb")]
        let mut handler = BoltProtocolHandler::new(Arc::new(MockStorage));


        // 2. Act
        let mut read_buf = bytes::BytesMut::with_capacity(1024);
        let chunk_size = handler.read_chunk(&mut stream, &mut read_buf).await.unwrap();
        assert_ne!(chunk_size, 0);

        let message_bytes = read_buf.freeze();
        let result = handler.process_message(&message_bytes, &mut stream).await;

        // 3. Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);

        // Check the response written to the stream
        let response = stream.write_data;
        assert!(!response.is_empty());

        // Decode the response to verify it's a SUCCESS message
        // The response is chunked, so first 2 bytes are chunk size
        let chunk_size = u16::from_be_bytes([response[0], response[1]]) as usize;
        let message_data = &response[2..2 + chunk_size];

        // SUCCESS message is a struct with 1 field, signature 0x70
        assert_eq!(message_data[0], 0xB1); // Tiny Struct (1 field)
        assert_eq!(message_data[1], 0x70); // SUCCESS signature

        // The field is a map, let's decode it
        let mut decoder = PackStreamDecoder::new();
        decoder.position = 2; // Skip struct header
        let decoded_value = decoder.decode_value(message_data).unwrap();

        if let Value::Object(map) = decoded_value {
            assert!(map.contains_key("server"));
            assert!(map.contains_key("connection_id"));
            if let Some(Value::String(server_val)) = map.get("server") {
                assert!(server_val.contains("Orbit-RS"));
            }
        } else {
            panic!("Expected a map in SUCCESS message");
        }
    }

    fn create_logon_message() -> Vec<u8> {
        let mut buf = bytes::BytesMut::new();
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("scheme".to_string(), Value::String("basic".to_string()));
        metadata.insert("principal".to_string(), Value::String("neo4j".to_string()));
        metadata.insert("credentials".to_string(), Value::String("new_password".to_string()));

        // Manually encode the LOGON message
        // Structure: 1 field, signature 0x6A (LOGON)
        buf.put_u8(0xB1); // Tiny Struct (1 field)
        buf.put_u8(0x6A); // LOGON signature

        // The single field is a map
        let map_len = metadata.len();
        if map_len < 16 {
            buf.put_u8(0xA0 + map_len as u8); // Tiny Map
        }

        for (key, value) in &metadata {
            // Encode key (string)
            let key_bytes = key.as_bytes();
            if key_bytes.len() < 16 {
                buf.put_u8(0x80 + key_bytes.len() as u8);
            }
            buf.put_slice(key_bytes);

            // Encode value
            if let Value::String(s) = value {
                let s_bytes = s.as_bytes();
                if s_bytes.len() < 16 {
                    buf.put_u8(0x80 + s_bytes.len() as u8);
                }
                buf.put_slice(s_bytes);
            }
        }

        let mut message_with_chunk_header = Vec::new();
        let len = buf.len() as u16;
        message_with_chunk_header.extend_from_slice(&len.to_be_bytes());
        message_with_chunk_header.extend_from_slice(&buf);
        // End of message marker
        message_with_chunk_header.extend_from_slice(&[0x00, 0x00]);

        message_with_chunk_header
    }

    #[tokio::test]
    async fn test_handle_logon() {
        // 1. Setup
        let logon_message = create_logon_message();
        let mut stream = MockStream::new(logon_message);

        #[cfg(not(feature = "storage-rocksdb"))]
        let mut handler = BoltProtocolHandler::new_without_storage();
        #[cfg(feature = "storage-rocksdb")]
        let mut handler = BoltProtocolHandler::new(Arc::new(MockStorage));

        // 2. Act
        let mut read_buf = bytes::BytesMut::with_capacity(1024);
        let chunk_size = handler.read_chunk(&mut stream, &mut read_buf).await.unwrap();
        assert_ne!(chunk_size, 0);

        let message_bytes = read_buf.freeze();
        let result = handler.process_message(&message_bytes, &mut stream).await;

        // 3. Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);

        // Check the response written to the stream
        let response = stream.write_data;
        assert!(!response.is_empty());

        // Decode the response to verify it's a SUCCESS message
        let chunk_size = u16::from_be_bytes([response[0], response[1]]) as usize;
        let message_data = &response[2..2 + chunk_size];

        // SUCCESS message is a struct with 1 field, signature 0x70
        assert_eq!(message_data[0], 0xB1); // Tiny Struct (1 field)
        assert_eq!(message_data[1], 0x70); // SUCCESS signature
    }

    fn create_logoff_message() -> Vec<u8> {
        let mut buf = bytes::BytesMut::new();

        // Manually encode the LOGOFF message
        // Structure: 0 fields, signature 0x6B (LOGOFF)
        buf.put_u8(0xB0); // Tiny Struct (0 fields)
        buf.put_u8(0x6B); // LOGOFF signature

        let mut message_with_chunk_header = Vec::new();
        let len = buf.len() as u16;
        message_with_chunk_header.extend_from_slice(&len.to_be_bytes());
        message_with_chunk_header.extend_from_slice(&buf);
        // End of message marker
        message_with_chunk_header.extend_from_slice(&[0x00, 0x00]);

        message_with_chunk_header
    }

    #[tokio::test]
    async fn test_handle_logoff() {
        // 1. Setup
        let logoff_message = create_logoff_message();
        let mut stream = MockStream::new(logoff_message);

        #[cfg(not(feature = "storage-rocksdb"))]
        let mut handler = BoltProtocolHandler::new_without_storage();
        #[cfg(feature = "storage-rocksdb")]
        let mut handler = BoltProtocolHandler::new(Arc::new(MockStorage));

        // 2. Act
        let mut read_buf = bytes::BytesMut::with_capacity(1024);
        let chunk_size = handler.read_chunk(&mut stream, &mut read_buf).await.unwrap();
        assert_ne!(chunk_size, 0);

        let message_bytes = read_buf.freeze();
        let result = handler.process_message(&message_bytes, &mut stream).await;

        // 3. Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);

        // Check the response written to the stream
        let response = stream.write_data;
        assert!(!response.is_empty());

        // Decode the response to verify it's a SUCCESS message
        let chunk_size = u16::from_be_bytes([response[0], response[1]]) as usize;
        let message_data = &response[2..2 + chunk_size];

        // SUCCESS message is a struct with 1 field, signature 0x70
        assert_eq!(message_data[0], 0xB1); // Tiny Struct (1 field)
        assert_eq!(message_data[1], 0x70); // SUCCESS signature
    }
}
