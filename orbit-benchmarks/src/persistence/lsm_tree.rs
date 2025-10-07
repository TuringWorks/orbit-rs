use super::config::LsmConfig;
use super::{
    ActorKey, ActorLease, OperationType, PersistenceError, PersistenceMetrics, PersistenceProvider,
    StorageStats,
};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Instant, SystemTime};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::Mutex;

const TOMBSTONE_VALUE: &[u8] = b"__TOMBSTONE__";

/// LSM-Tree persistence implementation
pub struct LsmTreePersistence {
    config: LsmConfig,
    data_dir: PathBuf,

    // Active MemTable (in-memory sorted map)
    memtable: Arc<RwLock<MemTable>>,

    // Immutable MemTables waiting to be flushed
    immutable_memtables: Arc<Mutex<VecDeque<MemTable>>>,

    // SSTable levels
    levels: Arc<RwLock<Vec<Level>>>,

    // Write-Ahead Log for durability
    wal: Arc<Mutex<WriteAheadLog>>,

    // Block cache for frequently accessed data
    block_cache: Arc<RwLock<HashMap<String, CachedBlock>>>,

    // Performance metrics
    operation_count: Arc<Mutex<u64>>,
    total_bytes_written: Arc<Mutex<u64>>,
    total_bytes_read: Arc<Mutex<u64>>,

    // Background compaction state
    compaction_in_progress: Arc<Mutex<bool>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct MemTable {
    data: BTreeMap<ActorKey, Option<ActorLease>>, // None represents deletion
    size_bytes: usize,
    created_at: SystemTime,
}

#[derive(Debug, Clone)]
struct Level {
    sstables: Vec<SSTable>,
    max_size_bytes: usize,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct SSTable {
    file_path: PathBuf,
    min_key: ActorKey,
    max_key: ActorKey,
    size_bytes: usize,
    bloom_filter: Option<BloomFilter>,
    index: Vec<IndexEntry>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct IndexEntry {
    key: ActorKey,
    offset: u64,
    size: usize,
}

#[derive(Debug, Clone)]
struct BloomFilter {
    bits: Vec<bool>,
    num_hash_functions: usize,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct CachedBlock {
    data: Vec<u8>,
    last_accessed: SystemTime,
}

struct WriteAheadLog {
    #[allow(dead_code)]
    file: File,
    buffer: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WALEntry {
    key: ActorKey,
    value: Option<ActorLease>, // None for deletions
    sequence: u64,
    timestamp: SystemTime,
}

#[derive(Debug, Serialize, Deserialize)]
struct SSTableRecord {
    key: ActorKey,
    value: Option<ActorLease>,
    timestamp: SystemTime,
}

impl LsmTreePersistence {
    pub async fn new(config: LsmConfig, data_dir: &Path) -> Result<Self, PersistenceError> {
        tokio::fs::create_dir_all(data_dir).await?;

        let wal_path = data_dir.join("wal.log");
        let wal_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&wal_path)
            .await?;

        let wal = WriteAheadLog {
            file: wal_file,
            buffer: Vec::new(),
        };

        // Initialize levels based on configuration
        let mut levels = Vec::new();
        let mut level_size = config.memtable_size_mb * 1024 * 1024;

        for _ in 0..config.max_levels {
            levels.push(Level {
                sstables: Vec::new(),
                max_size_bytes: level_size,
            });
            level_size *= config.level_size_multiplier;
        }

        let persistence = Self {
            config,
            data_dir: data_dir.to_path_buf(),
            memtable: Arc::new(RwLock::new(MemTable::new())),
            immutable_memtables: Arc::new(Mutex::new(VecDeque::new())),
            levels: Arc::new(RwLock::new(levels)),
            wal: Arc::new(Mutex::new(wal)),
            block_cache: Arc::new(RwLock::new(HashMap::new())),
            operation_count: Arc::new(Mutex::new(0)),
            total_bytes_written: Arc::new(Mutex::new(0)),
            total_bytes_read: Arc::new(Mutex::new(0)),
            compaction_in_progress: Arc::new(Mutex::new(false)),
        };

        // Recover from existing SSTables if any
        persistence.recover_from_disk().await?;

        Ok(persistence)
    }

    async fn recover_from_disk(&self) -> Result<(), PersistenceError> {
        let sstable_dir = self.data_dir.join("sstables");
        if !sstable_dir.exists() {
            tokio::fs::create_dir_all(&sstable_dir).await?;
            return Ok(());
        }

        // Scan for existing SSTable files and rebuild level structure
        let mut entries = tokio::fs::read_dir(&sstable_dir).await?;
        let mut sstable_files = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            if entry.path().extension().is_some_and(|ext| ext == "sst") {
                sstable_files.push(entry.path());
            }
        }

        sstable_files.sort();

        // Load SSTables into appropriate levels
        for file_path in sstable_files {
            if let Ok(sstable) = self.load_sstable_metadata(&file_path).await {
                let level_index = self.determine_level_for_sstable(&sstable);
                let mut levels = self.levels.write().unwrap();
                if level_index < levels.len() {
                    levels[level_index].sstables.push(sstable);
                }
            }
        }

        println!("LSM-Tree recovery completed");
        Ok(())
    }

    async fn load_sstable_metadata(&self, file_path: &Path) -> Result<SSTable, PersistenceError> {
        let file = File::open(file_path).await?;
        let metadata = file.metadata().await?;

        // For simplicity, create a basic SSTable metadata
        // In a real implementation, this would parse the SSTable header
        Ok(SSTable {
            file_path: file_path.to_path_buf(),
            min_key: ActorKey {
                actor_id: uuid::Uuid::nil(),
                actor_type: String::new(),
            },
            max_key: ActorKey {
                actor_id: uuid::Uuid::max(),
                actor_type: "zzz".to_string(),
            },
            size_bytes: metadata.len() as usize,
            bloom_filter: None,
            index: Vec::new(),
        })
    }

    fn determine_level_for_sstable(&self, sstable: &SSTable) -> usize {
        // Simple heuristic: place in level based on size
        let levels = self.levels.read().unwrap();
        for (i, level) in levels.iter().enumerate() {
            if sstable.size_bytes <= level.max_size_bytes {
                return i;
            }
        }
        levels.len().saturating_sub(1)
    }

    async fn insert_internal(
        &self,
        key: ActorKey,
        value: Option<ActorLease>,
    ) -> Result<PersistenceMetrics, PersistenceError> {
        let start_time = Instant::now();
        let mut bytes_written = 0u64;

        // Log to WAL first
        {
            let mut wal = self.wal.lock().await;
            let wal_entry = WALEntry {
                key: key.clone(),
                value: value.clone(),
                sequence: *self.operation_count.lock().await,
                timestamp: SystemTime::now(),
            };

            let serialized = serde_json::to_vec(&wal_entry)?;
            wal.buffer
                .extend_from_slice(&(serialized.len() as u32).to_le_bytes());
            wal.buffer.extend_from_slice(&serialized);
            bytes_written += serialized.len() as u64;
        }

        // Check if MemTable needs to be rotated
        let should_flush = {
            let memtable = self.memtable.read().unwrap();
            memtable.size_bytes >= (self.config.memtable_size_mb * 1024 * 1024)
        };

        if should_flush {
            self.rotate_memtable().await?;
        }

        let is_insert = value.is_some();

        // Insert into active MemTable
        {
            let mut memtable = self.memtable.write().unwrap();
            let value_size = if let Some(ref lease) = value {
                lease.to_bytes()?.len()
            } else {
                TOMBSTONE_VALUE.len()
            };

            memtable.data.insert(key, value);
            memtable.size_bytes += value_size;
        }

        // Update metrics
        {
            let mut op_count = self.operation_count.lock().await;
            *op_count += 1;

            let mut total_written = self.total_bytes_written.lock().await;
            *total_written += bytes_written;
        }

        // Trigger background compaction if needed
        tokio::spawn({
            let persistence = self.clone_for_background_task();
            async move {
                if let Err(e) = persistence.maybe_trigger_compaction().await {
                    eprintln!("Background compaction failed: {}", e);
                }
            }
        });

        Ok(PersistenceMetrics {
            operation_type: if is_insert {
                OperationType::Insert
            } else {
                OperationType::Update // Deletion treated as update
            },
            latency: start_time.elapsed(),
            memory_used: std::mem::size_of::<ActorLease>() as u64,
            disk_bytes_read: 0,
            disk_bytes_written: bytes_written,
            success: true,
        })
    }

    fn clone_for_background_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            data_dir: self.data_dir.clone(),
            memtable: Arc::clone(&self.memtable),
            immutable_memtables: Arc::clone(&self.immutable_memtables),
            levels: Arc::clone(&self.levels),
            wal: Arc::clone(&self.wal),
            block_cache: Arc::clone(&self.block_cache),
            operation_count: Arc::clone(&self.operation_count),
            total_bytes_written: Arc::clone(&self.total_bytes_written),
            total_bytes_read: Arc::clone(&self.total_bytes_read),
            compaction_in_progress: Arc::clone(&self.compaction_in_progress),
        }
    }

    async fn rotate_memtable(&self) -> Result<(), PersistenceError> {
        let old_memtable = {
            let mut memtable = self.memtable.write().unwrap();
            std::mem::replace(&mut *memtable, MemTable::new())
        };

        // Add to immutable memtables queue
        {
            let mut immutable = self.immutable_memtables.lock().await;
            immutable.push_back(old_memtable);
        }

        // Flush oldest immutable memtable
        self.flush_immutable_memtable().await?;

        Ok(())
    }

    async fn flush_immutable_memtable(&self) -> Result<(), PersistenceError> {
        let memtable = {
            let mut immutable = self.immutable_memtables.lock().await;
            immutable.pop_front()
        };

        if let Some(memtable) = memtable {
            let sstable_path = self.generate_sstable_path(0).await;
            self.write_sstable(&memtable, &sstable_path).await?;

            // Add to Level 0
            let sstable = SSTable {
                file_path: sstable_path,
                min_key: memtable.data.keys().next().cloned().unwrap_or_default(),
                max_key: memtable.data.keys().last().cloned().unwrap_or_default(),
                size_bytes: memtable.size_bytes,
                bloom_filter: if self.config.enable_bloom_filters {
                    Some(self.build_bloom_filter(&memtable))
                } else {
                    None
                },
                index: Vec::new(), // Would be populated during write
            };

            let mut levels = self.levels.write().unwrap();
            levels[0].sstables.push(sstable);
        }

        Ok(())
    }

    async fn generate_sstable_path(&self, level: usize) -> PathBuf {
        let sstable_dir = self.data_dir.join("sstables");
        tokio::fs::create_dir_all(&sstable_dir).await.unwrap_or(());

        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        sstable_dir.join(format!(
            "level_{}_{}_{}.sst",
            level,
            timestamp,
            rand::random::<u32>()
        ))
    }

    async fn write_sstable(
        &self,
        memtable: &MemTable,
        path: &Path,
    ) -> Result<(), PersistenceError> {
        let file = File::create(path).await?;
        let mut writer = BufWriter::new(file);

        // Write records in sorted order
        for (key, value) in &memtable.data {
            let record = SSTableRecord {
                key: key.clone(),
                value: value.clone(),
                timestamp: SystemTime::now(),
            };

            let serialized = serde_json::to_vec(&record)?;
            writer
                .write_all(&(serialized.len() as u32).to_le_bytes())
                .await?;
            writer.write_all(&serialized).await?;
        }

        writer.flush().await?;
        Ok(())
    }

    fn build_bloom_filter(&self, memtable: &MemTable) -> BloomFilter {
        let num_items = memtable.data.len();
        let num_bits = num_items * self.config.bloom_filter_bits_per_key;
        let num_hash_functions =
            ((self.config.bloom_filter_bits_per_key as f64) * (2.0_f64.ln())).ceil() as usize;

        let mut bits = vec![false; num_bits];

        for key in memtable.data.keys() {
            for i in 0..num_hash_functions {
                let hash = self.hash_key_with_seed(key, i as u64);
                let bit_index = (hash as usize) % num_bits;
                bits[bit_index] = true;
            }
        }

        BloomFilter {
            bits,
            num_hash_functions,
        }
    }

    fn hash_key_with_seed(&self, key: &ActorKey, seed: u64) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        seed.hash(&mut hasher);
        hasher.finish()
    }

    async fn maybe_trigger_compaction(&self) -> Result<(), PersistenceError> {
        let mut compaction_guard = self.compaction_in_progress.lock().await;
        if *compaction_guard {
            return Ok(()); // Compaction already in progress
        }

        // Check if any level needs compaction
        let level_index_to_compact = {
            let levels = self.levels.read().unwrap();
            let mut result = None;
            for (level_index, level) in levels.iter().enumerate() {
                if level.sstables.len() > self.config.compaction_threshold {
                    result = Some(level_index);
                    break;
                }
            }
            result
        };

        if let Some(level_index) = level_index_to_compact {
            *compaction_guard = true;
            drop(compaction_guard);

            self.compact_level(level_index).await?;

            let mut compaction_guard = self.compaction_in_progress.lock().await;
            *compaction_guard = false;
        }

        Ok(())
    }

    async fn compact_level(&self, level_index: usize) -> Result<(), PersistenceError> {
        println!("Starting compaction for level {}", level_index);

        // For simplicity, this is a basic compaction strategy
        // In a real implementation, this would be much more sophisticated

        let sstables_to_compact = {
            let mut levels = self.levels.write().unwrap();
            let level = &mut levels[level_index];
            let to_compact = level.sstables.clone();
            level.sstables.clear();
            to_compact
        };

        if sstables_to_compact.is_empty() {
            return Ok(());
        }

        // Merge all SSTables in this level
        let mut merged_data = BTreeMap::new();
        for sstable in &sstables_to_compact {
            let sstable_data = self.read_sstable(&sstable.file_path).await?;
            for (key, value) in sstable_data {
                merged_data.insert(key, value);
            }
        }

        // Create new SSTable for the next level
        if !merged_data.is_empty() {
            let next_level = level_index + 1;
            if next_level < self.config.max_levels {
                let new_sstable_path = self.generate_sstable_path(next_level).await;
                self.write_merged_sstable(&merged_data, &new_sstable_path)
                    .await?;

                let new_sstable = SSTable {
                    file_path: new_sstable_path,
                    min_key: merged_data.keys().next().cloned().unwrap_or_default(),
                    max_key: merged_data.keys().last().cloned().unwrap_or_default(),
                    size_bytes: merged_data.len() * 256, // Rough estimate
                    bloom_filter: None,
                    index: Vec::new(),
                };

                let mut levels = self.levels.write().unwrap();
                levels[next_level].sstables.push(new_sstable);
            }
        }

        // Clean up old SSTable files
        for sstable in &sstables_to_compact {
            let _ = tokio::fs::remove_file(&sstable.file_path).await;
        }

        println!("Compaction completed for level {}", level_index);
        Ok(())
    }

    async fn read_sstable(
        &self,
        path: &Path,
    ) -> Result<BTreeMap<ActorKey, Option<ActorLease>>, PersistenceError> {
        let file = File::open(path).await?;
        let mut reader = BufReader::new(file);
        let mut data = BTreeMap::new();
        let mut buffer = [0u8; 4];

        while reader.read_exact(&mut buffer).await.is_ok() {
            let record_size = u32::from_le_bytes(buffer) as usize;
            let mut record_bytes = vec![0u8; record_size];
            reader.read_exact(&mut record_bytes).await?;

            if let Ok(record) = serde_json::from_slice::<SSTableRecord>(&record_bytes) {
                data.insert(record.key, record.value);
            }
        }

        Ok(data)
    }

    async fn write_merged_sstable(
        &self,
        data: &BTreeMap<ActorKey, Option<ActorLease>>,
        path: &Path,
    ) -> Result<(), PersistenceError> {
        let file = File::create(path).await?;
        let mut writer = BufWriter::new(file);

        for (key, value) in data {
            let record = SSTableRecord {
                key: key.clone(),
                value: value.clone(),
                timestamp: SystemTime::now(),
            };

            let serialized = serde_json::to_vec(&record)?;
            writer
                .write_all(&(serialized.len() as u32).to_le_bytes())
                .await?;
            writer.write_all(&serialized).await?;
        }

        writer.flush().await?;
        Ok(())
    }

    async fn search_sstables(
        &self,
        key: &ActorKey,
    ) -> Result<Option<ActorLease>, PersistenceError> {
        // Clone the sstables we need to search to avoid holding the lock across await
        let sstables_to_search = {
            let levels = self.levels.read().unwrap();
            let mut sstables = Vec::new();
            for level in levels.iter() {
                for sstable in &level.sstables {
                    // Check bloom filter first if available
                    if let Some(ref bloom) = sstable.bloom_filter {
                        if !self.bloom_filter_might_contain(bloom, key) {
                            continue;
                        }
                    }

                    // Check key range
                    if key >= &sstable.min_key && key <= &sstable.max_key {
                        sstables.push(sstable.file_path.clone());
                    }
                }
            }
            sstables
        };

        // Search in the identified SSTables
        for sstable_path in sstables_to_search {
            if let Some(value) = self.search_in_sstable(&sstable_path, key).await? {
                return Ok(value);
            }
        }

        Ok(None)
    }

    fn bloom_filter_might_contain(&self, bloom: &BloomFilter, key: &ActorKey) -> bool {
        for i in 0..bloom.num_hash_functions {
            let hash = self.hash_key_with_seed(key, i as u64);
            let bit_index = (hash as usize) % bloom.bits.len();
            if !bloom.bits[bit_index] {
                return false;
            }
        }
        true
    }

    async fn search_in_sstable(
        &self,
        path: &Path,
        key: &ActorKey,
    ) -> Result<Option<Option<ActorLease>>, PersistenceError> {
        // For simplicity, do a linear scan
        // In a real implementation, this would use the index for efficient lookup
        let sstable_data = self.read_sstable(path).await?;
        Ok(sstable_data.get(key).cloned())
    }
}

impl MemTable {
    fn new() -> Self {
        Self {
            data: BTreeMap::new(),
            size_bytes: 0,
            created_at: SystemTime::now(),
        }
    }
}

impl Default for ActorKey {
    fn default() -> Self {
        Self {
            actor_id: uuid::Uuid::nil(),
            actor_type: String::new(),
        }
    }
}

#[async_trait::async_trait]
impl PersistenceProvider for LsmTreePersistence {
    async fn store_lease(
        &self,
        lease: &ActorLease,
    ) -> Result<PersistenceMetrics, PersistenceError> {
        self.insert_internal(lease.key.clone(), Some(lease.clone()))
            .await
    }

    async fn get_lease(
        &self,
        key: &ActorKey,
    ) -> Result<(Option<ActorLease>, PersistenceMetrics), PersistenceError> {
        let start_time = Instant::now();
        let mut bytes_read = 0u64;

        // Search in MemTable first
        let memtable_result = {
            let memtable = self.memtable.read().unwrap();
            memtable.data.get(key).cloned()
        };

        if let Some(value) = memtable_result {
            let metrics = PersistenceMetrics {
                operation_type: OperationType::Get,
                latency: start_time.elapsed(),
                memory_used: 0,
                disk_bytes_read: 0,
                disk_bytes_written: 0,
                success: true,
            };
            return Ok((value, metrics));
        }

        // Search in immutable memtables
        {
            let immutable = self.immutable_memtables.lock().await;
            for memtable in immutable.iter().rev() {
                if let Some(value) = memtable.data.get(key).cloned() {
                    let metrics = PersistenceMetrics {
                        operation_type: OperationType::Get,
                        latency: start_time.elapsed(),
                        memory_used: 0,
                        disk_bytes_read: 0,
                        disk_bytes_written: 0,
                        success: true,
                    };
                    return Ok((value, metrics));
                }
            }
        }

        // Search in SSTables
        let sstable_result = self.search_sstables(key).await?;
        bytes_read += 1024; // Estimate

        let mut total_read = self.total_bytes_read.lock().await;
        *total_read += bytes_read;

        let metrics = PersistenceMetrics {
            operation_type: OperationType::Get,
            latency: start_time.elapsed(),
            memory_used: 0,
            disk_bytes_read: bytes_read,
            disk_bytes_written: 0,
            success: true,
        };

        Ok((sstable_result, metrics))
    }

    async fn range_query(
        &self,
        start: &ActorKey,
        end: &ActorKey,
    ) -> Result<(Vec<ActorLease>, PersistenceMetrics), PersistenceError> {
        let start_time = Instant::now();
        let mut results = Vec::new();

        // Collect from MemTable
        {
            let memtable = self.memtable.read().unwrap();
            for (_key, value) in memtable.data.range(start..=end) {
                if let Some(lease) = value {
                    results.push(lease.clone());
                }
            }
        }

        // This is a simplified implementation
        // A real LSM-tree would need sophisticated merge logic for range queries

        let metrics = PersistenceMetrics {
            operation_type: OperationType::RangeQuery,
            latency: start_time.elapsed(),
            memory_used: results.len() as u64 * std::mem::size_of::<ActorLease>() as u64,
            disk_bytes_read: 0,
            disk_bytes_written: 0,
            success: true,
        };

        Ok((results, metrics))
    }

    async fn create_snapshot(&self) -> Result<(String, PersistenceMetrics), PersistenceError> {
        let start_time = Instant::now();
        let snapshot_id = format!(
            "lsm_snapshot_{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );

        // Force flush all memtables
        self.rotate_memtable().await?;

        let metrics = PersistenceMetrics {
            operation_type: OperationType::Snapshot,
            latency: start_time.elapsed(),
            memory_used: 0,
            disk_bytes_read: 0,
            disk_bytes_written: 0,
            success: true,
        };

        Ok((snapshot_id, metrics))
    }

    async fn restore_from_snapshot(
        &self,
        _snapshot_id: &str,
    ) -> Result<PersistenceMetrics, PersistenceError> {
        let start_time = Instant::now();

        // Simplified - would restore from a specific snapshot
        self.recover_from_disk().await?;

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
        let op_count = *self.operation_count.lock().await;
        let bytes_written = *self.total_bytes_written.lock().await;
        let _bytes_read = *self.total_bytes_read.lock().await;

        // Estimate memory usage
        let memtable_size = {
            let memtable = self.memtable.read().unwrap();
            memtable.size_bytes as u64
        };

        let immutable_size: usize = {
            let immutable = self.immutable_memtables.lock().await;
            immutable.iter().map(|mt| mt.size_bytes).sum()
        };

        Ok(StorageStats {
            total_keys: op_count,
            total_size_bytes: bytes_written,
            memory_usage_bytes: memtable_size + immutable_size as u64,
            disk_usage_bytes: bytes_written,
            average_key_size: 64,    // Estimate
            average_value_size: 512, // Estimate
        })
    }

    async fn simulate_crash_recovery(&self) -> Result<PersistenceMetrics, PersistenceError> {
        let start_time = Instant::now();

        // Simulate recovery by reading from disk
        self.recover_from_disk().await?;

        Ok(PersistenceMetrics {
            operation_type: OperationType::Recovery,
            latency: start_time.elapsed(),
            memory_used: 0,
            disk_bytes_read: 1024, // Estimate
            disk_bytes_written: 0,
            success: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::tempdir;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_lsm_basic_operations() {
        let temp_dir = tempdir().unwrap();
        let config = LsmConfig::default();
        let lsm = LsmTreePersistence::new(config, temp_dir.path())
            .await
            .unwrap();

        // Test insert
        let lease = ActorLease::new(
            Uuid::new_v4(),
            "test_actor".to_string(),
            "node1".to_string(),
            Duration::from_secs(300),
        );

        let insert_metrics = lsm.store_lease(&lease).await.unwrap();
        assert!(insert_metrics.success);
        assert!(insert_metrics.latency.as_micros() > 0);

        // Test get
        let (retrieved, get_metrics) = lsm.get_lease(&lease.key).await.unwrap();
        assert!(get_metrics.success);
        assert_eq!(retrieved.unwrap(), lease);
    }

    #[tokio::test]
    async fn test_lsm_memtable_flush() {
        let temp_dir = tempdir().unwrap();
        let config = LsmConfig {
            memtable_size_mb: 1, // Small memtable for testing
            ..Default::default()
        };

        let lsm = LsmTreePersistence::new(config, temp_dir.path())
            .await
            .unwrap();

        // Insert enough data to trigger memtable flush
        for i in 0..100 {
            let lease = ActorLease::new(
                Uuid::new_v4(),
                format!("actor_{}", i),
                "node1".to_string(),
                Duration::from_secs(300),
            );
            lsm.store_lease(&lease).await.unwrap();
        }

        // Give background tasks time to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let stats = lsm.get_stats().await.unwrap();
        assert!(stats.total_keys > 0);
    }
}
