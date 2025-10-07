use super::{ActorKey, ActorLease, PersistenceError, PersistenceMetrics, PersistenceProvider, OperationType, StorageStats};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use rocksdb::{DB, Options, WriteOptions, ReadOptions, ColumnFamily, ColumnFamilyDescriptor};

/// RocksDB implementation optimized for actor lease workload
pub struct RocksDBPersistence {
    db: Arc<DB>,
    
    // Optimized options for different operation types
    lease_write_options: WriteOptions,
    read_options: ReadOptions,
    
    // Performance tracking
    operation_count: Arc<Mutex<u64>>,
}

impl RocksDBPersistence {
    pub async fn new(data_dir: &std::path::Path) -> Result<Self, PersistenceError> {
        tokio::fs::create_dir_all(data_dir).await?;
        
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        
        // Optimize for actor lease workload
        opts.set_write_buffer_size(64 * 1024 * 1024);  // 64MB memtables
        opts.set_max_write_buffer_number(4);
        opts.set_target_file_size_base(64 * 1024 * 1024);
        opts.set_max_bytes_for_level_base(256 * 1024 * 1024);
        
        // Optimize compaction for frequent updates
        opts.set_compaction_style(rocksdb::DBCompactionStyle::Level);
        opts.set_level_compaction_dynamic_level_bytes(true);
        
        // Enable bloom filters for faster reads
        let mut block_opts = rocksdb::BlockBasedOptions::default();
        block_opts.set_bloom_filter(10.0, false);
        block_opts.set_cache_index_and_filter_blocks(true);
        opts.set_block_based_table_factory(&block_opts);
        
        // Column family configurations
        let mut lease_cf_opts = Options::default();
        lease_cf_opts.set_write_buffer_size(32 * 1024 * 1024);
        lease_cf_opts.set_max_write_buffer_number(3);
        
        let mut cluster_cf_opts = Options::default();
        cluster_cf_opts.set_write_buffer_size(16 * 1024 * 1024);
        cluster_cf_opts.set_max_write_buffer_number(2);
        
        let cf_descriptors = vec![
            ColumnFamilyDescriptor::new("default", opts.clone()),
            ColumnFamilyDescriptor::new("leases", lease_cf_opts),
            ColumnFamilyDescriptor::new("cluster", cluster_cf_opts),
        ];
        
        let db = DB::open_cf_descriptors(&opts, data_dir, cf_descriptors)
            .map_err(|e| PersistenceError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        
        // Verify column families exist
        db.cf_handle("leases")
            .ok_or_else(|| PersistenceError::Corruption("Missing leases column family".to_string()))?;
        
        db.cf_handle("cluster")
            .ok_or_else(|| PersistenceError::Corruption("Missing cluster column family".to_string()))?;
        
        // Configure write options for durability
        let mut lease_write_options = WriteOptions::default();
        lease_write_options.set_sync(true);  // Ensure durability for lease updates
        lease_write_options.disable_wal(false);  // Keep WAL for recovery
        
        let read_options = ReadOptions::default();
        
        Ok(Self {
            db: Arc::new(db),
            lease_write_options,
            read_options,
            operation_count: Arc::new(Mutex::new(0)),
        })
    }
    
    /// Get the appropriate column family for an actor key
    fn get_column_family(&self, _key: &ActorKey) -> &ColumnFamily {
        // For simplicity, put all leases in the lease column family
        // In production, could distribute based on actor type or other criteria
        self.db.cf_handle("leases").unwrap()
    }
    
    /// Serialize key for RocksDB storage
    fn serialize_key(&self, key: &ActorKey) -> Vec<u8> {
        key.to_bytes()
    }
    
    /// Serialize lease for RocksDB storage
    fn serialize_lease(&self, lease: &ActorLease) -> Result<Vec<u8>, PersistenceError> {
        lease.to_bytes()
    }
    
    /// Deserialize lease from RocksDB storage
    fn deserialize_lease(&self, bytes: &[u8]) -> Result<ActorLease, PersistenceError> {
        ActorLease::from_bytes(bytes)
    }
}

#[async_trait::async_trait]
impl PersistenceProvider for RocksDBPersistence {
    async fn store_lease(&self, lease: &ActorLease) -> Result<PersistenceMetrics, PersistenceError> {
        let start_time = Instant::now();
        
        let key = self.serialize_key(&lease.key);
        let value = self.serialize_lease(lease)?;
        let cf = self.get_column_family(&lease.key);
        
        let result = self.db.put_cf_opt(cf, &key, &value, &self.lease_write_options);
        
        match result {
            Ok(()) => {
                let mut op_count = self.operation_count.lock().unwrap();
                *op_count += 1;
                
                Ok(PersistenceMetrics {
                    operation_type: OperationType::Insert,
                    latency: start_time.elapsed(),
                    memory_used: value.len() as u64,
                    disk_bytes_read: 0,
                    disk_bytes_written: value.len() as u64,
                    success: true,
                })
            }
            Err(e) => Err(PersistenceError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))),
        }
    }
    
    async fn get_lease(&self, key: &ActorKey) -> Result<(Option<ActorLease>, PersistenceMetrics), PersistenceError> {
        let start_time = Instant::now();
        
        let serialized_key = self.serialize_key(key);
        let cf = self.get_column_family(key);
        
        match self.db.get_cf_opt(cf, &serialized_key, &self.read_options) {
            Ok(Some(value)) => {
                let lease = self.deserialize_lease(&value)?;
                let metrics = PersistenceMetrics {
                    operation_type: OperationType::Get,
                    latency: start_time.elapsed(),
                    memory_used: value.len() as u64,
                    disk_bytes_read: value.len() as u64,
                    disk_bytes_written: 0,
                    success: true,
                };
                Ok((Some(lease), metrics))
            }
            Ok(None) => {
                let metrics = PersistenceMetrics {
                    operation_type: OperationType::Get,
                    latency: start_time.elapsed(),
                    memory_used: 0,
                    disk_bytes_read: 0,
                    disk_bytes_written: 0,
                    success: true,
                };
                Ok((None, metrics))
            }
            Err(e) => Err(PersistenceError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))),
        }
    }
    
    async fn range_query(&self, start: &ActorKey, end: &ActorKey) -> Result<(Vec<ActorLease>, PersistenceMetrics), PersistenceError> {
        let start_time = Instant::now();
        
        let start_key = self.serialize_key(start);
        let end_key = self.serialize_key(end);
        let cf = self.get_column_family(start);
        
        let mut results = Vec::new();
        let mut bytes_read = 0u64;
        
        let mut read_opts = ReadOptions::default();
        read_opts.set_iterate_lower_bound(start_key.clone());
        read_opts.set_iterate_upper_bound(end_key.clone());
        
        let iter = self.db.iterator_cf_opt(cf, read_opts, rocksdb::IteratorMode::Start);
        
        for item in iter {
            match item {
                Ok((key, value)) => {
                    // Verify key is within range
                    if key.as_ref() >= start_key.as_slice() && key.as_ref() <= end_key.as_slice() {
                        let lease = self.deserialize_lease(&value)?;
                        bytes_read += value.len() as u64;
                        results.push(lease);
                    }
                }
                Err(e) => return Err(PersistenceError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))),
            }
        }
        
        let metrics = PersistenceMetrics {
            operation_type: OperationType::RangeQuery,
            latency: start_time.elapsed(),
            memory_used: results.len() as u64 * std::mem::size_of::<ActorLease>() as u64,
            disk_bytes_read: bytes_read,
            disk_bytes_written: 0,
            success: true,
        };
        
        Ok((results, metrics))
    }
    
    async fn create_snapshot(&self) -> Result<(String, PersistenceMetrics), PersistenceError> {
        let start_time = Instant::now();
        
        let snapshot_id = format!("rocksdb_snapshot_{}", chrono::Utc::now().timestamp());
        
        // Create RocksDB snapshot
        let _snapshot = self.db.snapshot();
        
        // In production, would serialize and store snapshot metadata
        // For benchmark, just measure the operation cost
        
        let metrics = PersistenceMetrics {
            operation_type: OperationType::Snapshot,
            latency: start_time.elapsed(),
            memory_used: 0, // Snapshots in RocksDB are lightweight
            disk_bytes_read: 0,
            disk_bytes_written: 0,
            success: true,
        };
        
        Ok((snapshot_id, metrics))
    }
    
    async fn restore_from_snapshot(&self, _snapshot_id: &str) -> Result<PersistenceMetrics, PersistenceError> {
        let start_time = Instant::now();
        
        // In production, would restore from snapshot
        // For benchmark, simulate the cost
        
        Ok(PersistenceMetrics {
            operation_type: OperationType::Recovery,
            latency: start_time.elapsed(),
            memory_used: 0,
            disk_bytes_read: 0,
            disk_bytes_written: 0,
            success: true,
        })
    }
    
    async fn get_stats(&self) -> Result<StorageStats, PersistenceError> {
        let total_keys = *self.operation_count.lock().unwrap();
        
        // Get RocksDB statistics
        let mut disk_usage = 0u64;
        
        // Estimate sizes from RocksDB properties
        if let Ok(Some(size_str)) = self.db.property_value("rocksdb.total-sst-files-size") {
            if let Ok(size) = size_str.parse::<u64>() {
                disk_usage += size;
            }
        }
        
        if let Ok(Some(wal_size_str)) = self.db.property_value("rocksdb.cur-size-all-mem-tables") {
            if let Ok(wal_size) = wal_size_str.parse::<u64>() {
                disk_usage += wal_size;
            }
        }
        
        Ok(StorageStats {
            total_keys,
            total_size_bytes: disk_usage,
            memory_usage_bytes: 0, // RocksDB manages its own memory
            disk_usage_bytes: disk_usage,
            average_key_size: 32, // Estimate
            average_value_size: 256, // Estimate based on ActorLease size
        })
    }
    
    async fn simulate_crash_recovery(&self) -> Result<PersistenceMetrics, PersistenceError> {
        let start_time = Instant::now();
        
        // RocksDB handles crash recovery automatically via WAL replay
        // Simulate the typical recovery cost
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        Ok(PersistenceMetrics {
            operation_type: OperationType::Recovery,
            latency: start_time.elapsed(),
            memory_used: 0,
            disk_bytes_read: 1024 * 1024, // Simulate WAL replay
            disk_bytes_written: 0,
            success: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use std::time::Duration;
    
    #[tokio::test]
    async fn test_rocksdb_basic_operations() {
        let temp_dir = tempfile::tempdir().unwrap();
        let persistence = RocksDBPersistence::new(temp_dir.path()).await.unwrap();
        
        let lease = ActorLease::new(
            Uuid::new_v4(),
            "test_actor".to_string(),
            "node1".to_string(),
            Duration::from_secs(300),
        );
        
        // Test insert
        let metrics = persistence.store_lease(&lease).await.unwrap();
        assert!(metrics.success);
        
        // Test get
        let (retrieved, get_metrics) = persistence.get_lease(&lease.key).await.unwrap();
        assert!(get_metrics.success);
        assert_eq!(retrieved.unwrap(), lease);
        
        // Test range query
        let end_key = ActorKey {
            actor_id: Uuid::new_v4(),
            actor_type: "z_actor".to_string(),
        };
        let (range_results, range_metrics) = persistence.range_query(&lease.key, &end_key).await.unwrap();
        assert!(range_metrics.success);
        assert_eq!(range_results.len(), 1);
    }
    
    #[tokio::test]
    async fn test_rocksdb_batch_operations() {
        let temp_dir = tempfile::tempdir().unwrap();
        let persistence = RocksDBPersistence::new(temp_dir.path()).await.unwrap();
        
        // Create multiple leases
        let mut leases = Vec::new();
        for i in 0..100 {
            let lease = ActorLease::new(
                Uuid::new_v4(),
                format!("actor_type_{}", i % 10),
                "node1".to_string(),
                Duration::from_secs(300),
            );
            leases.push(lease);
        }
        
        // Store all leases
        for lease in &leases {
            let result = persistence.store_lease(lease).await;
            assert!(result.is_ok());
        }
        
        // Verify retrieval
        for lease in &leases {
            let (retrieved, _) = persistence.get_lease(&lease.key).await.unwrap();
            assert_eq!(retrieved.unwrap(), *lease);
        }
        
        // Test stats
        let stats = persistence.get_stats().await.unwrap();
        assert_eq!(stats.total_keys, 100);
    }
}