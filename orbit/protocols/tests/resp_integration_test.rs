//! RESP Server Integration Tests
//!
//! End-to-end tests for the Redis Protocol (RESP) server functionality

#![cfg(feature = "resp")]

use bytes::BytesMut;
use orbit_protocols::resp::{RespCodec, RespValue};
use tokio_util::codec::{Decoder, Encoder};

#[tokio::test]
async fn test_resp_codec_basic_operations() {
    let mut codec = RespCodec::new();

    // Test encoding and decoding simple string
    let mut buf = BytesMut::new();
    let simple_string = RespValue::simple_string("OK");

    // Encode
    codec.encode(simple_string.clone(), &mut buf).unwrap();
    assert_eq!(buf, BytesMut::from("+OK\r\n"));

    // Decode
    let decoded = codec.decode(&mut buf).unwrap();
    assert_eq!(decoded, Some(simple_string));
    assert!(buf.is_empty());
}

#[tokio::test]
async fn test_resp_codec_bulk_string() {
    let mut codec = RespCodec::new();

    // Test encoding and decoding bulk string
    let mut buf = BytesMut::new();
    let bulk_string = RespValue::bulk_string_from_str("hello world");

    // Encode
    codec.encode(bulk_string.clone(), &mut buf).unwrap();
    assert_eq!(buf, BytesMut::from("$11\r\nhello world\r\n"));

    // Decode
    let decoded = codec.decode(&mut buf).unwrap();
    assert_eq!(decoded, Some(bulk_string));
    assert!(buf.is_empty());
}

#[tokio::test]
async fn test_resp_codec_integer() {
    let mut codec = RespCodec::new();

    // Test encoding and decoding integer
    let mut buf = BytesMut::new();
    let integer_val = RespValue::integer(12345);

    // Encode
    codec.encode(integer_val.clone(), &mut buf).unwrap();
    assert_eq!(buf, BytesMut::from(":12345\r\n"));

    // Decode
    let decoded = codec.decode(&mut buf).unwrap();
    assert_eq!(decoded, Some(integer_val));
    assert!(buf.is_empty());
}

#[tokio::test]
async fn test_resp_codec_array() {
    let mut codec = RespCodec::new();

    // Test encoding and decoding array
    let mut buf = BytesMut::new();
    let array_val = RespValue::Array(vec![
        RespValue::simple_string("PING"),
        RespValue::bulk_string_from_str("hello"),
    ]);

    // Encode
    codec.encode(array_val.clone(), &mut buf).unwrap();
    assert_eq!(buf, BytesMut::from("*2\r\n+PING\r\n$5\r\nhello\r\n"));

    // Decode
    let decoded = codec.decode(&mut buf).unwrap();
    assert_eq!(decoded, Some(array_val));
    assert!(buf.is_empty());
}

#[tokio::test]
async fn test_resp_codec_error_handling() {
    let mut codec = RespCodec::new();

    // Test invalid message
    let mut buf = BytesMut::from("@invalid\r\n");
    let result = codec.decode(&mut buf);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_resp_codec_incomplete_messages() {
    let mut codec = RespCodec::new();

    // Test incomplete simple string
    let mut buf = BytesMut::from("+HELLO");
    let result = codec.decode(&mut buf).unwrap();
    assert_eq!(result, None); // Not enough data

    // Complete the message
    buf.extend_from_slice(b"\r\n");
    let result = codec.decode(&mut buf).unwrap();
    assert_eq!(result, Some(RespValue::simple_string("HELLO")));
    assert!(buf.is_empty());
}

#[tokio::test]
async fn test_resp_redis_command_format() {
    let mut codec = RespCodec::new();

    // Test Redis command format: ["SET", "key", "value"]
    let mut buf = BytesMut::new();
    let redis_command = RespValue::Array(vec![
        RespValue::bulk_string_from_str("SET"),
        RespValue::bulk_string_from_str("mykey"),
        RespValue::bulk_string_from_str("myvalue"),
    ]);

    // Encode
    codec.encode(redis_command.clone(), &mut buf).unwrap();

    // Should match Redis protocol format
    let expected = "*3\r\n$3\r\nSET\r\n$5\r\nmykey\r\n$7\r\nmyvalue\r\n";
    assert_eq!(buf, BytesMut::from(expected));

    // Decode back
    let decoded = codec.decode(&mut buf).unwrap();
    assert_eq!(decoded, Some(redis_command));
}

#[tokio::test]
async fn test_resp_null_values() {
    let mut codec = RespCodec::new();

    // Test null bulk string
    let mut buf = BytesMut::new();
    let null_bulk = RespValue::NullBulkString;

    codec.encode(null_bulk.clone(), &mut buf).unwrap();
    assert_eq!(buf, BytesMut::from("$-1\r\n"));

    let decoded = codec.decode(&mut buf).unwrap();
    assert_eq!(decoded, Some(null_bulk));
    assert!(buf.is_empty());

    // Test null array
    let mut buf = BytesMut::new();
    let null_array = RespValue::NullArray;

    codec.encode(null_array.clone(), &mut buf).unwrap();
    assert_eq!(buf, BytesMut::from("*-1\r\n"));

    let decoded = codec.decode(&mut buf).unwrap();
    assert_eq!(decoded, Some(null_array));
    assert!(buf.is_empty());
}
