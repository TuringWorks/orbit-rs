//! Sealed trait pattern for preventing external trait implementations
//!
//! The sealed trait pattern prevents external crates from implementing a trait
//! while still allowing the trait to be publicly used. This gives library authors
//! control over which types can implement a trait.

use crate::error::{OrbitError, OrbitResult};

// ===== Basic Sealed Trait Pattern =====

mod private {
    /// Private module prevents external implementation
    pub trait Sealed {}
}

/// Public trait that can only be implemented by types that also implement the sealed trait
pub trait ProtocolVersion: private::Sealed {
    fn version_number(&self) -> u32;
    fn protocol_name(&self) -> &'static str;
}

/// V1 protocol
pub struct ProtocolV1;
impl private::Sealed for ProtocolV1 {}
impl ProtocolVersion for ProtocolV1 {
    fn version_number(&self) -> u32 {
        1
    }

    fn protocol_name(&self) -> &'static str {
        "orbit-v1"
    }
}

/// V2 protocol
pub struct ProtocolV2;
impl private::Sealed for ProtocolV2 {}
impl ProtocolVersion for ProtocolV2 {
    fn version_number(&self) -> u32 {
        2
    }

    fn protocol_name(&self) -> &'static str {
        "orbit-v2"
    }
}

/// V3 protocol with enhanced features
pub struct ProtocolV3;
impl private::Sealed for ProtocolV3 {}
impl ProtocolVersion for ProtocolV3 {
    fn version_number(&self) -> u32 {
        3
    }

    fn protocol_name(&self) -> &'static str {
        "orbit-v3-enhanced"
    }
}

// ===== Sealed Trait for Storage Backends =====

mod storage_sealed {
    pub trait Sealed {}
}

/// Sealed trait for storage backend implementations
/// External crates cannot implement this trait
pub trait StorageBackend: storage_sealed::Sealed {
    fn backend_type(&self) -> &'static str;
    fn supports_transactions(&self) -> bool;
    fn max_key_size(&self) -> usize;
}

pub struct MemoryBackend;
impl storage_sealed::Sealed for MemoryBackend {}
impl StorageBackend for MemoryBackend {
    fn backend_type(&self) -> &'static str {
        "memory"
    }

    fn supports_transactions(&self) -> bool {
        true
    }

    fn max_key_size(&self) -> usize {
        1024
    }
}

pub struct RocksDbBackend;
impl storage_sealed::Sealed for RocksDbBackend {}
impl StorageBackend for RocksDbBackend {
    fn backend_type(&self) -> &'static str {
        "rocksdb"
    }

    fn supports_transactions(&self) -> bool {
        true
    }

    fn max_key_size(&self) -> usize {
        8 * 1024 * 1024 // 8MB
    }
}

pub struct RedisBackend;
impl storage_sealed::Sealed for RedisBackend {}
impl StorageBackend for RedisBackend {
    fn backend_type(&self) -> &'static str {
        "redis"
    }

    fn supports_transactions(&self) -> bool {
        true
    }

    fn max_key_size(&self) -> usize {
        512 * 1024 * 1024 // 512MB
    }
}

// ===== Sealed Trait with Exhaustive Matching =====

mod serialization_sealed {
    pub trait Sealed {}
}

/// Sealed serialization format trait
/// Allows exhaustive matching since all implementations are known
pub trait SerializationFormat: serialization_sealed::Sealed {
    fn format_name(&self) -> &'static str;
    fn file_extension(&self) -> &'static str;
    fn is_binary(&self) -> bool;
}

#[derive(Debug, Clone, Copy)]
pub struct JsonFormat;
impl serialization_sealed::Sealed for JsonFormat {}
impl SerializationFormat for JsonFormat {
    fn format_name(&self) -> &'static str {
        "JSON"
    }

    fn file_extension(&self) -> &'static str {
        ".json"
    }

    fn is_binary(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BincodeFormat;
impl serialization_sealed::Sealed for BincodeFormat {}
impl SerializationFormat for BincodeFormat {
    fn format_name(&self) -> &'static str {
        "Bincode"
    }

    fn file_extension(&self) -> &'static str {
        ".bin"
    }

    fn is_binary(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MessagePackFormat;
impl serialization_sealed::Sealed for MessagePackFormat {}
impl SerializationFormat for MessagePackFormat {
    fn format_name(&self) -> &'static str {
        "MessagePack"
    }

    fn file_extension(&self) -> &'static str {
        ".msgpack"
    }

    fn is_binary(&self) -> bool {
        true
    }
}

/// Enum wrapper for exhaustive matching
#[derive(Debug, Clone, Copy)]
pub enum Format {
    Json(JsonFormat),
    Bincode(BincodeFormat),
    MessagePack(MessagePackFormat),
}

impl Format {
    pub fn as_trait(&self) -> &dyn SerializationFormat {
        match self {
            Format::Json(f) => f,
            Format::Bincode(f) => f,
            Format::MessagePack(f) => f,
        }
    }

    /// Exhaustive matching is possible because trait is sealed
    pub fn process(&self, data: &[u8]) -> String {
        match self {
            Format::Json(_) => format!("Processing JSON data: {} bytes", data.len()),
            Format::Bincode(_) => format!("Processing Bincode data: {} bytes", data.len()),
            Format::MessagePack(_) => {
                format!("Processing MessagePack data: {} bytes", data.len())
            }
        }
    }
}

// ===== Sealed Trait for Permission Levels =====

mod permission_sealed {
    pub trait Sealed {}
}

/// Sealed permission level trait
/// Ensures only predefined permission levels can exist
pub trait PermissionLevel: permission_sealed::Sealed {
    fn level_name(&self) -> &'static str;
    fn level_value(&self) -> u8;
    fn can_read(&self) -> bool;
    fn can_write(&self) -> bool;
    fn can_delete(&self) -> bool;
    fn can_admin(&self) -> bool;
}

pub struct ReadOnly;
impl permission_sealed::Sealed for ReadOnly {}
impl PermissionLevel for ReadOnly {
    fn level_name(&self) -> &'static str {
        "ReadOnly"
    }

    fn level_value(&self) -> u8 {
        1
    }

    fn can_read(&self) -> bool {
        true
    }

    fn can_write(&self) -> bool {
        false
    }

    fn can_delete(&self) -> bool {
        false
    }

    fn can_admin(&self) -> bool {
        false
    }
}

pub struct ReadWrite;
impl permission_sealed::Sealed for ReadWrite {}
impl PermissionLevel for ReadWrite {
    fn level_name(&self) -> &'static str {
        "ReadWrite"
    }

    fn level_value(&self) -> u8 {
        2
    }

    fn can_read(&self) -> bool {
        true
    }

    fn can_write(&self) -> bool {
        true
    }

    fn can_delete(&self) -> bool {
        false
    }

    fn can_admin(&self) -> bool {
        false
    }
}

pub struct Admin;
impl permission_sealed::Sealed for Admin {}
impl PermissionLevel for Admin {
    fn level_name(&self) -> &'static str {
        "Admin"
    }

    fn level_value(&self) -> u8 {
        3
    }

    fn can_read(&self) -> bool {
        true
    }

    fn can_write(&self) -> bool {
        true
    }

    fn can_delete(&self) -> bool {
        true
    }

    fn can_admin(&self) -> bool {
        true
    }
}

// ===== Sealed Trait for Index Types =====

mod index_sealed {
    pub trait Sealed {}
}

/// Sealed trait for index types
/// Only the library can define valid index implementations
pub trait IndexType: index_sealed::Sealed {
    fn index_name(&self) -> &'static str;
    fn supports_range_queries(&self) -> bool;
    fn supports_prefix_queries(&self) -> bool;
    fn supports_full_text_search(&self) -> bool;
}

pub struct BTreeIndex;
impl index_sealed::Sealed for BTreeIndex {}
impl IndexType for BTreeIndex {
    fn index_name(&self) -> &'static str {
        "BTree"
    }

    fn supports_range_queries(&self) -> bool {
        true
    }

    fn supports_prefix_queries(&self) -> bool {
        true
    }

    fn supports_full_text_search(&self) -> bool {
        false
    }
}

pub struct HashIndex;
impl index_sealed::Sealed for HashIndex {}
impl IndexType for HashIndex {
    fn index_name(&self) -> &'static str {
        "Hash"
    }

    fn supports_range_queries(&self) -> bool {
        false
    }

    fn supports_prefix_queries(&self) -> bool {
        false
    }

    fn supports_full_text_search(&self) -> bool {
        false
    }
}

pub struct FullTextIndex;
impl index_sealed::Sealed for FullTextIndex {}
impl IndexType for FullTextIndex {
    fn index_name(&self) -> &'static str {
        "FullText"
    }

    fn supports_range_queries(&self) -> bool {
        false
    }

    fn supports_prefix_queries(&self) -> bool {
        true
    }

    fn supports_full_text_search(&self) -> bool {
        true
    }
}

// ===== Helper Functions Using Sealed Traits =====

/// Function that works with any protocol version
/// External code cannot break this by implementing custom versions
pub fn get_protocol_info<P: ProtocolVersion>(protocol: &P) -> String {
    format!(
        "{} (v{})",
        protocol.protocol_name(),
        protocol.version_number()
    )
}

/// Select storage backend based on requirements
pub fn select_backend(
    needs_transactions: bool,
    max_key_size: usize,
) -> Box<dyn StorageBackend> {
    if max_key_size > 8 * 1024 * 1024 {
        Box::new(RedisBackend)
    } else if needs_transactions {
        Box::new(RocksDbBackend)
    } else {
        Box::new(MemoryBackend)
    }
}

/// Format file name based on serialization format
pub fn format_filename(base: &str, format: &dyn SerializationFormat) -> String {
    format!("{}{}", base, format.file_extension())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_versions() {
        let v1 = ProtocolV1;
        let v2 = ProtocolV2;
        let v3 = ProtocolV3;

        assert_eq!(v1.version_number(), 1);
        assert_eq!(v2.version_number(), 2);
        assert_eq!(v3.version_number(), 3);

        assert_eq!(get_protocol_info(&v1), "orbit-v1 (v1)");
        assert_eq!(get_protocol_info(&v2), "orbit-v2 (v2)");
        assert_eq!(get_protocol_info(&v3), "orbit-v3-enhanced (v3)");
    }

    #[test]
    fn test_storage_backends() {
        let memory = MemoryBackend;
        let rocksdb = RocksDbBackend;
        let redis = RedisBackend;

        assert_eq!(memory.backend_type(), "memory");
        assert_eq!(rocksdb.backend_type(), "rocksdb");
        assert_eq!(redis.backend_type(), "redis");

        assert!(memory.supports_transactions());
        assert!(rocksdb.supports_transactions());
        assert!(redis.supports_transactions());

        assert_eq!(memory.max_key_size(), 1024);
        assert_eq!(rocksdb.max_key_size(), 8 * 1024 * 1024);
        assert_eq!(redis.max_key_size(), 512 * 1024 * 1024);
    }

    #[test]
    fn test_serialization_formats() {
        let json = JsonFormat;
        let bincode = BincodeFormat;
        let msgpack = MessagePackFormat;

        assert_eq!(json.format_name(), "JSON");
        assert_eq!(bincode.format_name(), "Bincode");
        assert_eq!(msgpack.format_name(), "MessagePack");

        assert!(!json.is_binary());
        assert!(bincode.is_binary());
        assert!(msgpack.is_binary());
    }

    #[test]
    fn test_exhaustive_format_matching() {
        let formats = vec![
            Format::Json(JsonFormat),
            Format::Bincode(BincodeFormat),
            Format::MessagePack(MessagePackFormat),
        ];

        for format in formats {
            let result = format.process(b"test data");
            assert!(result.contains("bytes"));
        }
    }

    #[test]
    fn test_permission_levels() {
        let readonly = ReadOnly;
        let readwrite = ReadWrite;
        let admin = Admin;

        assert!(readonly.can_read());
        assert!(!readonly.can_write());
        assert!(!readonly.can_delete());
        assert!(!readonly.can_admin());

        assert!(readwrite.can_read());
        assert!(readwrite.can_write());
        assert!(!readwrite.can_delete());
        assert!(!readwrite.can_admin());

        assert!(admin.can_read());
        assert!(admin.can_write());
        assert!(admin.can_delete());
        assert!(admin.can_admin());
    }

    #[test]
    fn test_index_types() {
        let btree = BTreeIndex;
        let hash = HashIndex;
        let fulltext = FullTextIndex;

        assert!(btree.supports_range_queries());
        assert!(!hash.supports_range_queries());
        assert!(!fulltext.supports_range_queries());

        assert!(!btree.supports_full_text_search());
        assert!(!hash.supports_full_text_search());
        assert!(fulltext.supports_full_text_search());
    }

    #[test]
    fn test_backend_selection() {
        let backend = select_backend(true, 1024);
        assert_eq!(backend.backend_type(), "rocksdb");

        let backend = select_backend(true, 10 * 1024 * 1024);
        assert_eq!(backend.backend_type(), "redis");

        let backend = select_backend(false, 512);
        assert_eq!(backend.backend_type(), "memory");
    }

    #[test]
    fn test_format_filename() {
        let json = JsonFormat;
        let bincode = BincodeFormat;

        assert_eq!(format_filename("data", &json), "data.json");
        assert_eq!(format_filename("data", &bincode), "data.bin");
    }
}
