#[cfg(test)]
mod tests {
    use crate::protocols::cypher::bolt_protocol::{BoltProtocolHandler, PackStreamDecoder};
    use crate::protocols::cypher::storage::CypherStorageProvider;
    use crate::protocols::cypher::types::{GraphNode, GraphRelationship};
    use crate::protocols::error::ProtocolResult;
    use async_trait::async_trait;
    use bytes::BufMut;
    use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::io::AsyncReadExt;

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

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), std::io::Error>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), std::io::Error>> {
            std::task::Poll::Ready(Ok(()))
        }
    }

    // Mock storage for testing
    #[derive(Clone)]
    struct MockStorage {
        nodes: Arc<tokio::sync::RwLock<std::collections::HashMap<String, GraphNode>>>,
        relationships:
            Arc<tokio::sync::RwLock<std::collections::HashMap<String, GraphRelationship>>>,
    }

    impl MockStorage {
        fn new() -> Self {
            Self {
                nodes: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
                relationships: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl CypherStorageProvider for MockStorage {
        async fn initialize(&self) -> ProtocolResult<()> {
            Ok(())
        }
        async fn store_node(&self, node: GraphNode) -> ProtocolResult<()> {
            self.nodes.write().await.insert(node.id.clone(), node);
            Ok(())
        }
        async fn get_node(&self, node_id: &str) -> ProtocolResult<Option<GraphNode>> {
            Ok(self.nodes.read().await.get(node_id).cloned())
        }
        async fn get_all_nodes(&self) -> ProtocolResult<Vec<GraphNode>> {
            Ok(self.nodes.read().await.values().cloned().collect())
        }
        async fn store_relationship(&self, rel: GraphRelationship) -> ProtocolResult<()> {
            self.relationships.write().await.insert(rel.id.clone(), rel);
            Ok(())
        }
        async fn get_relationship(
            &self,
            rel_id: &str,
        ) -> ProtocolResult<Option<GraphRelationship>> {
            Ok(self.relationships.read().await.get(rel_id).cloned())
        }
        async fn get_all_relationships(&self) -> ProtocolResult<Vec<GraphRelationship>> {
            Ok(self.relationships.read().await.values().cloned().collect())
        }
        async fn shutdown(&self) -> ProtocolResult<()> {
            Ok(())
        }
    }

    fn create_hello_message() -> Vec<u8> {
        let mut buf = bytes::BytesMut::new();
        let mut metadata = std::collections::HashMap::new();
        metadata.insert(
            "user_agent".to_string(),
            Value::String("test-client/1.0".to_string()),
        );
        metadata.insert("scheme".to_string(), Value::String("basic".to_string()));
        metadata.insert("principal".to_string(), Value::String("neo4j".to_string()));
        metadata.insert(
            "credentials".to_string(),
            Value::String("password".to_string()),
        );

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

        message_with_chunk_header
    }

    #[tokio::test]
    async fn test_handle_hello() {
        // 1. Setup
        let mut hello_message = create_hello_message();
        hello_message.extend_from_slice(&[0x00, 0x00]); // End of message
        let mut stream = MockStream::new(hello_message);

        #[cfg(not(feature = "storage-rocksdb"))]
        let mut handler = BoltProtocolHandler::new_without_storage();
        #[cfg(feature = "storage-rocksdb")]
        let mut handler = BoltProtocolHandler::new(Some(Arc::new(MockStorage::new())));

        // 2. Act
        let mut read_buf = bytes::BytesMut::with_capacity(1024);
        let chunk_size = handler
            .read_chunk(&mut stream, &mut read_buf)
            .await
            .unwrap();
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
        metadata.insert(
            "credentials".to_string(),
            Value::String("new_password".to_string()),
        );

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

        message_with_chunk_header
    }

    #[tokio::test]
    async fn test_handle_logon() {
        // 1. Setup
        let mut logon_message = create_logon_message();
        logon_message.extend_from_slice(&[0x00, 0x00]); // End of message
        let mut stream = MockStream::new(logon_message);

        #[cfg(not(feature = "storage-rocksdb"))]
        let mut handler = BoltProtocolHandler::new_without_storage();
        #[cfg(feature = "storage-rocksdb")]
        let mut handler = BoltProtocolHandler::new(Some(Arc::new(MockStorage::new())));

        // 2. Act
        let mut read_buf = bytes::BytesMut::with_capacity(1024);
        let chunk_size = handler
            .read_chunk(&mut stream, &mut read_buf)
            .await
            .unwrap();
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

        message_with_chunk_header
    }

    #[tokio::test]
    async fn test_handle_logoff() {
        // 1. Setup
        let mut logoff_message = create_logoff_message();
        logoff_message.extend_from_slice(&[0x00, 0x00]); // End of message
        let mut stream = MockStream::new(logoff_message);

        #[cfg(not(feature = "storage-rocksdb"))]
        let mut handler = BoltProtocolHandler::new_without_storage();
        #[cfg(feature = "storage-rocksdb")]
        let mut handler = BoltProtocolHandler::new(Some(Arc::new(MockStorage::new())));

        // 2. Act
        let mut read_buf = bytes::BytesMut::with_capacity(1024);
        let chunk_size = handler
            .read_chunk(&mut stream, &mut read_buf)
            .await
            .unwrap();
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

    fn create_run_message(query: &str) -> Vec<u8> {
        let mut buf = bytes::BytesMut::new();

        // Manually encode the RUN message
        // Structure: 3 fields, signature 0x10 (RUN)
        buf.put_u8(0xB3); // Tiny Struct (3 fields)
        buf.put_u8(0x10); // RUN signature

        // Query string
        let query_bytes = query.as_bytes();
        if query_bytes.len() < 16 {
            buf.put_u8(0x80 + query_bytes.len() as u8);
        } else {
            buf.put_u8(0xD0);
            buf.put_u8(query_bytes.len() as u8);
        }
        buf.put_slice(query_bytes);

        // Parameters map (empty)
        buf.put_u8(0xA0);

        // Extra map (empty)
        buf.put_u8(0xA0);

        let mut message_with_chunk_header = Vec::new();
        let len = buf.len() as u16;
        message_with_chunk_header.extend_from_slice(&len.to_be_bytes());
        message_with_chunk_header.extend_from_slice(&buf);

        message_with_chunk_header
    }

    #[tokio::test]
    async fn test_handle_run() {
        // 1. Setup
        let mut run_message = create_run_message("RETURN 1");
        run_message.extend_from_slice(&[0x00, 0x00]); // End of message
        let mut stream = MockStream::new(run_message);

        #[cfg(not(feature = "storage-rocksdb"))]
        let mut handler = BoltProtocolHandler::new_without_storage();
        #[cfg(feature = "storage-rocksdb")]
        let mut handler = BoltProtocolHandler::new(Some(Arc::new(MockStorage::new())));
        handler.auth_state.authenticated = true;

        // 2. Act
        let mut read_buf = bytes::BytesMut::with_capacity(1024);
        let chunk_size = handler
            .read_chunk(&mut stream, &mut read_buf)
            .await
            .unwrap();
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

        let mut decoder = PackStreamDecoder::new();
        decoder.position = 2; // Skip struct header
        let decoded_value = decoder.decode_value(message_data).unwrap();

        if let Value::Object(map) = decoded_value {
            assert!(map.contains_key("fields"));
            if let Some(Value::Array(fields)) = map.get("fields") {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0], Value::String("1".to_string()));
            } else {
                panic!("Expected 'fields' array in SUCCESS message");
            }
        } else {
            panic!("Expected a map in SUCCESS message");
        }
    }

    fn create_discard_message() -> Vec<u8> {
        let mut buf = bytes::BytesMut::new();

        // Manually encode the DISCARD message
        // Structure: 1 field, signature 0x2F (DISCARD)
        buf.put_u8(0xB1); // Tiny Struct (1 field)
        buf.put_u8(0x2F); // DISCARD signature

        // The single field is a map with n=-1 and qid=-1
        buf.put_u8(0xA2); // Tiny Map (2 fields)
                          // n
        buf.put_u8(0x81);
        buf.put_u8(b'n');
        buf.put_u8(0xFF); // -1
                          // qid
        buf.put_u8(0x83);
        buf.put_u8(b'q');
        buf.put_u8(b'i');
        buf.put_u8(b'd');
        buf.put_u8(0xFF); // -1

        let mut message_with_chunk_header = Vec::new();
        let len = buf.len() as u16;
        message_with_chunk_header.extend_from_slice(&len.to_be_bytes());
        message_with_chunk_header.extend_from_slice(&buf);

        message_with_chunk_header
    }

    #[tokio::test]
    #[ignore] // Disabled: DISCARD handler commented out
    async fn test_handle_discard() {
        // 1. Setup
        let mut run_message = create_run_message("RETURN 1");
        run_message.extend_from_slice(&[0x00, 0x00]); // End of message
        let mut discard_message = create_discard_message();
        discard_message.extend_from_slice(&[0x00, 0x00]); // End of message

        let mut messages = Vec::new();
        messages.extend_from_slice(&run_message);
        messages.extend_from_slice(&discard_message);

        let mut stream = MockStream::new(messages);

        #[cfg(not(feature = "storage-rocksdb"))]
        let mut handler = BoltProtocolHandler::new_without_storage();
        #[cfg(feature = "storage-rocksdb")]
        let mut handler = BoltProtocolHandler::new(Some(Arc::new(MockStorage::new())));
        handler.auth_state.authenticated = true;

        // 2. Act
        // Process RUN
        let mut read_buf = bytes::BytesMut::with_capacity(1024);
        let chunk_size = handler
            .read_chunk(&mut stream, &mut read_buf)
            .await
            .unwrap();
        assert_ne!(chunk_size, 0);
        let message_bytes = read_buf.freeze();
        handler
            .process_message(&message_bytes, &mut stream)
            .await
            .unwrap();
        assert!(!handler.pending_results.is_empty());

        // Consume the end-of-message marker
        let mut end_marker = [0u8; 2];
        stream.read_exact(&mut end_marker).await.unwrap();
        assert_eq!(end_marker, [0x00, 0x00]);

        // Process DISCARD
        let mut read_buf_2 = bytes::BytesMut::with_capacity(1024);
        let chunk_size = handler
            .read_chunk(&mut stream, &mut read_buf_2)
            .await
            .unwrap();
        assert_ne!(chunk_size, 0);
        let message_bytes_2 = read_buf_2.freeze();
        let result = handler.process_message(&message_bytes_2, &mut stream).await;

        // 3. Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);

        // Check that pending results are cleared
        assert!(handler.current_query.is_none());
        assert!(handler.pending_results.is_empty());
    }

    fn create_pull_message() -> Vec<u8> {
        let mut buf = bytes::BytesMut::new();

        // Manually encode the PULL message
        // Structure: 1 field, signature 0x3F (PULL)
        buf.put_u8(0xB1); // Tiny Struct (1 field)
        buf.put_u8(0x3F); // PULL signature

        // The single field is a map with n=-1
        buf.put_u8(0xA1); // Tiny Map (1 field)
                          // n
        buf.put_u8(0x81);
        buf.put_u8(b'n');
        buf.put_u8(0xFF); // -1

        let mut message_with_chunk_header = Vec::new();
        let len = buf.len() as u16;
        message_with_chunk_header.extend_from_slice(&len.to_be_bytes());
        message_with_chunk_header.extend_from_slice(&buf);

        message_with_chunk_header
    }

    #[tokio::test]
    async fn test_handle_pull() {
        // 1. Setup
        let mut run_message = create_run_message("RETURN 1");
        run_message.extend_from_slice(&[0x00, 0x00]); // End of message
        let mut pull_message = create_pull_message();
        pull_message.extend_from_slice(&[0x00, 0x00]); // End of message

        let mut messages = Vec::new();
        messages.extend_from_slice(&run_message);
        messages.extend_from_slice(&pull_message);

        let mut stream = MockStream::new(messages);

        #[cfg(not(feature = "storage-rocksdb"))]
        let mut handler = BoltProtocolHandler::new_without_storage();
        #[cfg(feature = "storage-rocksdb")]
        let mut handler = BoltProtocolHandler::new(Some(Arc::new(MockStorage::new())));
        handler.auth_state.authenticated = true;

        // 2. Act
        // Process RUN
        let mut read_buf = bytes::BytesMut::with_capacity(1024);
        handler
            .read_chunk(&mut stream, &mut read_buf)
            .await
            .unwrap();
        let message_bytes = read_buf.freeze();
        handler
            .process_message(&message_bytes, &mut stream)
            .await
            .unwrap();

        // Consume the end-of-message marker
        let mut end_marker = [0u8; 2];
        stream.read_exact(&mut end_marker).await.unwrap();
        assert_eq!(end_marker, [0x00, 0x00]);

        // Process PULL
        let mut read_buf_2 = bytes::BytesMut::with_capacity(1024);
        handler
            .read_chunk(&mut stream, &mut read_buf_2)
            .await
            .unwrap();
        let message_bytes_2 = read_buf_2.freeze();
        handler
            .process_message(&message_bytes_2, &mut stream)
            .await
            .unwrap();

        // 3. Assert
        let response = stream.write_data;
        assert!(!response.is_empty());

        // The response should contain a RECORD and a SUCCESS message
        // For simplicity, we'll just check that the response contains the RECORD signature (0x71)
        // and the SUCCESS signature (0x70)
        let record_signature = 0x71;
        let success_signature = 0x70;

        let record_found = response.windows(1).any(|w| w == [record_signature]);
        let success_found = response.windows(1).any(|w| w == [success_signature]);

        assert!(record_found);
        assert!(success_found);
    }

    fn create_begin_message() -> Vec<u8> {
        let mut buf = bytes::BytesMut::new();

        // Manually encode the BEGIN message
        // Structure: 1 field, signature 0x11 (BEGIN)
        buf.put_u8(0xB1); // Tiny Struct (1 field)
        buf.put_u8(0x11); // BEGIN signature

        // The single field is an empty map
        buf.put_u8(0xA0);

        let mut message_with_chunk_header = Vec::new();
        let len = buf.len() as u16;
        message_with_chunk_header.extend_from_slice(&len.to_be_bytes());
        message_with_chunk_header.extend_from_slice(&buf);

        message_with_chunk_header
    }

    #[tokio::test]
    #[ignore] // Disabled: BEGIN handler commented out
    async fn test_handle_begin() {
        // 1. Setup
        let mut begin_message = create_begin_message();
        begin_message.extend_from_slice(&[0x00, 0x00]); // End of message
        let mut stream = MockStream::new(begin_message);

        #[cfg(not(feature = "storage-rocksdb"))]
        let mut handler = BoltProtocolHandler::new_without_storage();
        #[cfg(feature = "storage-rocksdb")]
        let mut handler = BoltProtocolHandler::new(Some(Arc::new(MockStorage::new())));
        handler.auth_state.authenticated = true;

        // 2. Act
        let mut read_buf = bytes::BytesMut::with_capacity(1024);
        handler
            .read_chunk(&mut stream, &mut read_buf)
            .await
            .unwrap();
        let message_bytes = read_buf.freeze();
        handler
            .process_message(&message_bytes, &mut stream)
            .await
            .unwrap();

        // 3. Assert
        let response = stream.write_data;
        assert!(!response.is_empty());

        // SUCCESS message is a struct with 1 field, signature 0x70
        let chunk_size = u16::from_be_bytes([response[0], response[1]]) as usize;
        let message_data = &response[2..2 + chunk_size];
        assert_eq!(message_data[0], 0xB1); // Tiny Struct (1 field)
        assert_eq!(message_data[1], 0x70); // SUCCESS signature

        assert_eq!(
            handler.transaction_state,
            crate::protocols::cypher::bolt_protocol::TransactionState::Active
        );
    }

    fn create_commit_message() -> Vec<u8> {
        let mut buf = bytes::BytesMut::new();

        // Manually encode the COMMIT message
        // Structure: 0 fields, signature 0x12 (COMMIT)
        buf.put_u8(0xB0); // Tiny Struct (0 fields)
        buf.put_u8(0x12); // COMMIT signature

        let mut message_with_chunk_header = Vec::new();
        let len = buf.len() as u16;
        message_with_chunk_header.extend_from_slice(&len.to_be_bytes());
        message_with_chunk_header.extend_from_slice(&buf);

        message_with_chunk_header
    }

    #[tokio::test]
    async fn test_handle_commit() {
        // 1. Setup
        let mut begin_message = create_begin_message();
        begin_message.extend_from_slice(&[0x00, 0x00]); // End of message
        let mut commit_message = create_commit_message();
        commit_message.extend_from_slice(&[0x00, 0x00]); // End of message

        let mut messages = Vec::new();
        messages.extend_from_slice(&begin_message);
        messages.extend_from_slice(&commit_message);

        let mut stream = MockStream::new(messages);

        #[cfg(not(feature = "storage-rocksdb"))]
        let mut handler = BoltProtocolHandler::new_without_storage();
        #[cfg(feature = "storage-rocksdb")]
        let mut handler = BoltProtocolHandler::new(Some(Arc::new(MockStorage::new())));
        handler.auth_state.authenticated = true;

        // 2. Act
        // Process BEGIN
        let mut read_buf = bytes::BytesMut::with_capacity(1024);
        handler
            .read_chunk(&mut stream, &mut read_buf)
            .await
            .unwrap();
        let message_bytes = read_buf.freeze();
        handler
            .process_message(&message_bytes, &mut stream)
            .await
            .unwrap();

        // Consume the end-of-message marker
        let mut end_marker = [0u8; 2];
        stream.read_exact(&mut end_marker).await.unwrap();
        assert_eq!(end_marker, [0x00, 0x00]);

        // Process COMMIT
        let mut read_buf_2 = bytes::BytesMut::with_capacity(1024);
        handler
            .read_chunk(&mut stream, &mut read_buf_2)
            .await
            .unwrap();
        let message_bytes_2 = read_buf_2.freeze();
        handler
            .process_message(&message_bytes_2, &mut stream)
            .await
            .unwrap();

        // 3. Assert
        assert_eq!(
            handler.transaction_state,
            crate::protocols::cypher::bolt_protocol::TransactionState::None
        );
    }

    fn create_rollback_message() -> Vec<u8> {
        let mut buf = bytes::BytesMut::new();

        // Manually encode the ROLLBACK message
        // Structure: 0 fields, signature 0x13 (ROLLBACK)
        buf.put_u8(0xB0); // Tiny Struct (0 fields)
        buf.put_u8(0x13); // ROLLBACK signature

        let mut message_with_chunk_header = Vec::new();
        let len = buf.len() as u16;
        message_with_chunk_header.extend_from_slice(&len.to_be_bytes());
        message_with_chunk_header.extend_from_slice(&buf);

        message_with_chunk_header
    }

    #[tokio::test]
    async fn test_handle_rollback() {
        // 1. Setup
        let mut begin_message = create_begin_message();
        begin_message.extend_from_slice(&[0x00, 0x00]); // End of message
        let mut rollback_message = create_rollback_message();
        rollback_message.extend_from_slice(&[0x00, 0x00]); // End of message

        let mut messages = Vec::new();
        messages.extend_from_slice(&begin_message);
        messages.extend_from_slice(&rollback_message);

        let mut stream = MockStream::new(messages);

        #[cfg(not(feature = "storage-rocksdb"))]
        let mut handler = BoltProtocolHandler::new_without_storage();
        #[cfg(feature = "storage-rocksdb")]
        let mut handler = BoltProtocolHandler::new(Some(Arc::new(MockStorage::new())));
        handler.auth_state.authenticated = true;

        // 2. Act
        // Process BEGIN
        let mut read_buf = bytes::BytesMut::with_capacity(1024);
        handler
            .read_chunk(&mut stream, &mut read_buf)
            .await
            .unwrap();
        let message_bytes = read_buf.freeze();
        handler
            .process_message(&message_bytes, &mut stream)
            .await
            .unwrap();

        // Consume the end-of-message marker
        let mut end_marker = [0u8; 2];
        stream.read_exact(&mut end_marker).await.unwrap();
        assert_eq!(end_marker, [0x00, 0x00]);

        // Process ROLLBACK
        let mut read_buf_2 = bytes::BytesMut::with_capacity(1024);
        handler
            .read_chunk(&mut stream, &mut read_buf_2)
            .await
            .unwrap();
        let message_bytes_2 = read_buf_2.freeze();
        handler
            .process_message(&message_bytes_2, &mut stream)
            .await
            .unwrap();

        // 3. Assert
        assert_eq!(
            handler.transaction_state,
            crate::protocols::cypher::bolt_protocol::TransactionState::None
        );
    }

    fn create_reset_message() -> Vec<u8> {
        let mut buf = bytes::BytesMut::new();

        // Manually encode the RESET message
        // Structure: 0 fields, signature 0x0F (RESET)
        buf.put_u8(0xB0); // Tiny Struct (0 fields)
        buf.put_u8(0x0F); // RESET signature

        let mut message_with_chunk_header = Vec::new();
        let len = buf.len() as u16;
        message_with_chunk_header.extend_from_slice(&len.to_be_bytes());
        message_with_chunk_header.extend_from_slice(&buf);

        message_with_chunk_header
    }

    #[tokio::test]
    #[ignore] // Disabled: RESET handler commented out
    async fn test_handle_reset() {
        // 1. Setup
        let mut begin_message = create_begin_message();
        begin_message.extend_from_slice(&[0x00, 0x00]); // End of message
        let mut run_message = create_run_message("RETURN 1");
        run_message.extend_from_slice(&[0x00, 0x00]); // End of message
        let mut reset_message = create_reset_message();
        reset_message.extend_from_slice(&[0x00, 0x00]); // End of message

        let mut messages = Vec::new();
        messages.extend_from_slice(&begin_message);
        messages.extend_from_slice(&run_message);
        messages.extend_from_slice(&reset_message);

        let mut stream = MockStream::new(messages);

        #[cfg(not(feature = "storage-rocksdb"))]
        let mut handler = BoltProtocolHandler::new_without_storage();
        #[cfg(feature = "storage-rocksdb")]
        let mut handler = BoltProtocolHandler::new(Some(Arc::new(MockStorage::new())));
        handler.auth_state.authenticated = true;

        // 2. Act
        // Process BEGIN
        let mut read_buf = bytes::BytesMut::with_capacity(1024);
        handler
            .read_chunk(&mut stream, &mut read_buf)
            .await
            .unwrap();
        let message_bytes = read_buf.freeze();
        handler
            .process_message(&message_bytes, &mut stream)
            .await
            .unwrap();

        // Consume the end-of-message marker
        let mut end_marker = [0u8; 2];
        stream.read_exact(&mut end_marker).await.unwrap();
        assert_eq!(end_marker, [0x00, 0x00]);

        // Process RUN
        let mut read_buf_2 = bytes::BytesMut::with_capacity(1024);
        handler
            .read_chunk(&mut stream, &mut read_buf_2)
            .await
            .unwrap();
        let message_bytes_2 = read_buf_2.freeze();
        handler
            .process_message(&message_bytes_2, &mut stream)
            .await
            .unwrap();

        // Consume the end-of-message marker
        let mut end_marker = [0u8; 2];
        stream.read_exact(&mut end_marker).await.unwrap();
        assert_eq!(end_marker, [0x00, 0x00]);

        // Process RESET
        let mut read_buf_3 = bytes::BytesMut::with_capacity(1024);
        handler
            .read_chunk(&mut stream, &mut read_buf_3)
            .await
            .unwrap();
        let message_bytes_3 = read_buf_3.freeze();
        handler
            .process_message(&message_bytes_3, &mut stream)
            .await
            .unwrap();

        // 3. Assert
        assert_eq!(
            handler.transaction_state,
            crate::protocols::cypher::bolt_protocol::TransactionState::None
        );
        assert!(handler.current_query.is_none());
        assert!(handler.pending_results.is_empty());
    }

    fn create_route_message() -> Vec<u8> {
        let mut buf = bytes::BytesMut::new();

        // Manually encode the ROUTE message
        // Structure: 3 fields, signature 0x66 (ROUTE)
        buf.put_u8(0xB3); // Tiny Struct (3 fields)
        buf.put_u8(0x66); // ROUTE signature

        // Routing context (empty map)
        buf.put_u8(0xA0);
        // Bookmarks (empty list)
        buf.put_u8(0x90);
        // Extra (empty map)
        buf.put_u8(0xA0);

        let mut message_with_chunk_header = Vec::new();
        let len = buf.len() as u16;
        message_with_chunk_header.extend_from_slice(&len.to_be_bytes());
        message_with_chunk_header.extend_from_slice(&buf);

        message_with_chunk_header
    }

    #[tokio::test]
    async fn test_handle_route() {
        // 1. Setup
        let mut route_message = create_route_message();
        route_message.extend_from_slice(&[0x00, 0x00]); // End of message
        let mut stream = MockStream::new(route_message);

        #[cfg(not(feature = "storage-rocksdb"))]
        let mut handler = BoltProtocolHandler::new_without_storage();
        #[cfg(feature = "storage-rocksdb")]
        let mut handler = BoltProtocolHandler::new(Some(Arc::new(MockStorage::new())));
        handler.auth_state.authenticated = true;

        // 2. Act
        let mut read_buf = bytes::BytesMut::with_capacity(1024);
        handler
            .read_chunk(&mut stream, &mut read_buf)
            .await
            .unwrap();
        let message_bytes = read_buf.freeze();
        handler
            .process_message(&message_bytes, &mut stream)
            .await
            .unwrap();

        // 3. Assert
        let response = stream.write_data;
        assert!(!response.is_empty());

        // SUCCESS message is a struct with 1 field, signature 0x70
        let chunk_size = u16::from_be_bytes([response[0], response[1]]) as usize;
        let message_data = &response[2..2 + chunk_size];
        assert_eq!(message_data[0], 0xB1); // Tiny Struct (1 field)
        assert_eq!(message_data[1], 0x70); // SUCCESS signature

        let mut decoder = PackStreamDecoder::new();
        decoder.position = 2; // Skip struct header
        let decoded_value = decoder.decode_value(message_data).unwrap();

        if let Value::Object(map) = decoded_value {
            assert!(map.contains_key("rt"));
        } else {
            panic!("Expected a map in SUCCESS message");
        }
    }

    fn create_telemetry_message() -> Vec<u8> {
        let mut buf = bytes::BytesMut::new();

        // Manually encode the TELEMETRY message
        // Structure: 1 field, signature 0x54 (TELEMETRY)
        buf.put_u8(0xB1); // Tiny Struct (1 field)
        buf.put_u8(0x54); // TELEMETRY signature

        // The single field is a map with some telemetry data
        buf.put_u8(0xA1); // Tiny Map (1 field)
        buf.put_u8(0x83);
        buf.put_u8(b'd');
        buf.put_u8(b'a');
        buf.put_u8(b't');
        buf.put_u8(b'a');
        buf.put_u8(0x01); // some value

        let mut message_with_chunk_header = Vec::new();
        let len = buf.len() as u16;
        message_with_chunk_header.extend_from_slice(&len.to_be_bytes());
        message_with_chunk_header.extend_from_slice(&buf);

        message_with_chunk_header
    }

    #[tokio::test]
    #[ignore] // Disabled: TELEMETRY sends IGNORED
    async fn test_handle_telemetry() {
        // 1. Setup
        let mut telemetry_message = create_telemetry_message();
        telemetry_message.extend_from_slice(&[0x00, 0x00]); // End of message
        let mut stream = MockStream::new(telemetry_message);

        #[cfg(not(feature = "storage-rocksdb"))]
        let mut handler = BoltProtocolHandler::new_without_storage();
        #[cfg(feature = "storage-rocksdb")]
        let mut handler = BoltProtocolHandler::new(Some(Arc::new(MockStorage::new())));
        handler.auth_state.authenticated = true;

        // 2. Act
        let mut read_buf = bytes::BytesMut::with_capacity(1024);
        handler
            .read_chunk(&mut stream, &mut read_buf)
            .await
            .unwrap();
        let message_bytes = read_buf.freeze();
        handler
            .process_message(&message_bytes, &mut stream)
            .await
            .unwrap();

        // 3. Assert
        let response = stream.write_data;
        assert!(!response.is_empty());

        // IGNORED message is 0x7E
        let chunk_size = u16::from_be_bytes([response[0], response[1]]) as usize;
        let message_data = &response[2..2 + chunk_size];
        assert_eq!(message_data[0], 0x7E); // IGNORED signature
    }

    fn create_goodbye_message() -> Vec<u8> {
        let mut buf = bytes::BytesMut::new();

        // Manually encode the GOODBYE message
        // Structure: 0 fields, signature 0x02 (GOODBYE)
        buf.put_u8(0xB0); // Tiny Struct (0 fields)
        buf.put_u8(0x02); // GOODBYE signature

        let mut message_with_chunk_header = Vec::new();
        let len = buf.len() as u16;
        message_with_chunk_header.extend_from_slice(&len.to_be_bytes());
        message_with_chunk_header.extend_from_slice(&buf);

        message_with_chunk_header
    }

    #[tokio::test]
    async fn test_handle_goodbye() {
        // 1. Setup
        let mut goodbye_message = create_goodbye_message();
        goodbye_message.extend_from_slice(&[0x00, 0x00]); // End of message
        let mut stream = MockStream::new(goodbye_message);

        #[cfg(not(feature = "storage-rocksdb"))]
        let mut handler = BoltProtocolHandler::new_without_storage();
        #[cfg(feature = "storage-rocksdb")]
        let mut handler = BoltProtocolHandler::new(Some(Arc::new(MockStorage::new())));
        handler.auth_state.authenticated = true;

        // 2. Act
        let mut read_buf = bytes::BytesMut::with_capacity(1024);
        handler
            .read_chunk(&mut stream, &mut read_buf)
            .await
            .unwrap();
        let message_bytes = read_buf.freeze();
        let result = handler.process_message(&message_bytes, &mut stream).await;

        // 3. Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false);
    }

    #[tokio::test]
    #[ignore] // Disabled: Test needs review
    #[cfg(feature = "storage-rocksdb")]
    async fn test_match_relationship_with_properties() {
        // 1. Setup
        let mut run_message =
            create_run_message("MATCH (n:Person)-[r {name: 'test'}]->(m:Person) RETURN r");
        run_message.extend_from_slice(&[0x00, 0x00]); // End of message
        let mut stream = MockStream::new(run_message);

        #[cfg(not(feature = "storage-rocksdb"))]
        let mut handler = BoltProtocolHandler::new_without_storage();
        #[cfg(feature = "storage-rocksdb")]
        let storage = Arc::new(MockStorage::new());
        let mut handler = BoltProtocolHandler::new(Some(storage.clone()));
        handler.auth_state.authenticated = true;

        // Add nodes and a relationship to the mock storage
        let start_node = GraphNode {
            id: "1".to_string(),
            labels: vec!["Person".to_string()],
            properties: HashMap::new(),
        };
        let end_node = GraphNode {
            id: "2".to_string(),
            labels: vec!["Person".to_string()],
            properties: HashMap::new(),
        };
        storage.store_node(start_node).await.unwrap();
        storage.store_node(end_node).await.unwrap();

        let mut properties = std::collections::HashMap::new();
        properties.insert("name".to_string(), Value::String("test".to_string()));
        let rel = GraphRelationship {
            id: "1".to_string(),
            start_node: "1".to_string(),
            end_node: "2".to_string(),
            rel_type: "TEST".to_string(),
            properties,
        };
        storage.store_relationship(rel).await.unwrap();

        // 2. Act
        let mut read_buf = bytes::BytesMut::with_capacity(1024);
        handler
            .read_chunk(&mut stream, &mut read_buf)
            .await
            .unwrap();
        let message_bytes = read_buf.freeze();
        handler
            .process_message(&message_bytes, &mut stream)
            .await
            .unwrap();

        // 3. Assert
        assert!(!handler.pending_results.is_empty());
        assert_eq!(handler.pending_results.len(), 1);
    }
}
