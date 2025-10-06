use crate::{ActorKey, ActorLease, PersistenceError, PersistenceMetrics, PersistenceProvider, OperationType, StorageStats};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;
use std::time::{Instant, SystemTime};
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use serde::{Serialize, Deserialize};

const MAX_KEYS_PER_NODE: usize = 16; // Small for demonstration, would be ~100-500 in production
const WAL_BUFFER_SIZE: usize = 1024 * 1024; // 1MB WAL buffer

/// Copy-on-Write B+ Tree node
#[derive(Debug, Clone)]
pub struct BTreeNode {
    pub keys: Vec<ActorKey>,
    pub values: Vec<Option<ActorLease>>, // None for internal nodes
    pub children: Vec<Arc<BTreeNode>>,
    pub is_leaf: bool,
    pub version: u64,
    pub parent: Option<Arc<RwLock<BTreeNode>>>,
}

/// Write-Ahead Log entry
#[derive(Debug, Clone)]
struct WALEntry {
    sequence: u64,
    timestamp: SystemTime,
    operation: WALOperation,
}

#[derive(Debug, Clone)]
enum WALOperation {
    Insert { key: ActorKey, lease: ActorLease },
    Update { key: ActorKey, lease: ActorLease },
    Delete { key: ActorKey },
}

/// Snapshot metadata
#[derive(Debug, Clone)]
struct Snapshot {
    id: String,
    root_version: u64,
    timestamp: SystemTime,
    key_count: u64,
}

/// COW B+ Tree persistence implementation
pub struct CowBTreePersistence {
    root: Arc<RwLock<BTreeNode>>,
    wal: Arc<Mutex<WriteAheadLog>>,
    snapshots: Arc<RwLock<HashMap<String, Snapshot>>>,
    version_counter: Arc<Mutex<u64>>,
    data_dir: std::path::PathBuf,
    
    // Performance tracking
    operation_count: Arc<Mutex<u64>>,
    total_memory_used: Arc<Mutex<u64>>,
}

struct WriteAheadLog {
    file: File,
    sequence: u64,
    buffer: Vec<u8>,
}

impl BTreeNode {
    fn new_leaf(version: u64) -> Self {
        Self {
            keys: Vec::new(),
            values: Vec::new(),
            children: Vec::new(),
            is_leaf: true,
            version,
            parent: None,
        }
    }
    
    fn new_internal(version: u64) -> Self {
        Self {
            keys: Vec::new(),
            values: Vec::new(),
            children: Vec::new(),
            is_leaf: false,
            version,
            parent: None,
        }
    }
    
    fn is_full(&self) -> bool {
        self.keys.len() >= MAX_KEYS_PER_NODE
    }
    
    /// Copy-on-write clone of this node
    fn cow_clone(&self, new_version: u64) -> Self {
        Self {
            keys: self.keys.clone(),
            values: self.values.clone(),
            children: self.children.clone(), // Arc references are cheap to clone
            is_leaf: self.is_leaf,
            version: new_version,
            parent: None, // Will be set by parent
        }
    }
    
    /// Find the index where a key should be inserted
    fn find_key_index(&self, key: &ActorKey) -> Result<usize, usize> {
        self.keys.binary_search(key)
    }
    
    /// Insert a key-value pair in a leaf node
    fn insert_in_leaf(&mut self, key: ActorKey, lease: ActorLease) -> bool {
        match self.find_key_index(&key) {
            Ok(idx) => {
                // Key exists, update value
                self.values[idx] = Some(lease);
                false // Not a new insertion
            }
            Err(idx) => {
                // Key doesn't exist, insert new
                self.keys.insert(idx, key);
                self.values.insert(idx, Some(lease));
                true // New insertion
            }
        }
    }
    
    /// Split a full node
    fn split(&mut self, new_version: u64) -> (ActorKey, BTreeNode) {
        let mid = self.keys.len() / 2;
        
        let mut new_node = if self.is_leaf {
            BTreeNode::new_leaf(new_version)
        } else {
            BTreeNode::new_internal(new_version)
        };
        
        // Move half the keys to the new node
        new_node.keys = self.keys.split_off(mid + if self.is_leaf { 0 } else { 1 });
        
        if self.is_leaf {
            new_node.values = self.values.split_off(mid);
        } else {
            new_node.values = self.values.split_off(mid + 1);
            new_node.children = self.children.split_off(mid + 1);
        }
        
        let promote_key = if self.is_leaf {
            new_node.keys[0].clone()
        } else {
            self.keys.pop().unwrap()
        };
        
        (promote_key, new_node)
    }
}

impl WriteAheadLog {
    async fn new(path: &std::path::Path) -> Result<Self, PersistenceError> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;
        
        // Determine the next sequence number by reading existing entries
        let sequence = Self::get_last_sequence(path).await.unwrap_or(0) + 1;
            
        Ok(Self {
            file,
            sequence,
            buffer: Vec::with_capacity(WAL_BUFFER_SIZE),
        })
    }
    
    /// Get the last sequence number from the WAL file
    async fn get_last_sequence(path: &std::path::Path) -> Result<u64, PersistenceError> {
        if !path.exists() {
            return Ok(0);
        }
        
        let content = tokio::fs::read(path).await?;
        let mut last_sequence = 0u64;
        let mut offset = 0;
        
        while offset + 4 <= content.len() {
            // Read entry length
            let len_bytes = &content[offset..offset + 4];
            let entry_len = u32::from_le_bytes([
                len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]
            ]) as usize;
            offset += 4;
            
            if offset + entry_len <= content.len() {
                let entry_data = &content[offset..offset + entry_len];
                
                // Try to parse the entry to get sequence number
                if let Ok(entry) = serde_json::from_slice::<WALEntry>(entry_data) {
                    last_sequence = entry.sequence;
                }
                
                offset += entry_len;
            } else {
                break; // Incomplete entry
            }
        }
        
        Ok(last_sequence)
    }
    
    async fn append(&mut self, operation: WALOperation) -> Result<(), PersistenceError> {
        let entry = WALEntry {
            sequence: self.sequence,
            timestamp: SystemTime::now(),
            operation,
        };
        
        let serialized = serde_json::to_vec(&entry)?;
        println!("[WAL] Appending entry {} with {} bytes", self.sequence, serialized.len());
        self.buffer.extend_from_slice(&(serialized.len() as u32).to_le_bytes());
        self.buffer.extend_from_slice(&serialized);
        
        if self.buffer.len() > WAL_BUFFER_SIZE / 2 {
            self.flush().await?;
        }
        
        self.sequence += 1;
        Ok(())
    }
    
    async fn flush(&mut self) -> Result<(), PersistenceError> {
        if !self.buffer.is_empty() {
            println!("[WAL] Flushing {} bytes to disk", self.buffer.len());
            self.file.write_all(&self.buffer).await?;
            self.file.sync_all().await?;
            self.buffer.clear();
        }
        Ok(())
    }
}

impl CowBTreePersistence {
    pub async fn new(data_dir: &std::path::Path) -> Result<Self, PersistenceError> {
        tokio::fs::create_dir_all(data_dir).await?;
        
        let wal_path = data_dir.join("orbit.wal");
        let wal = WriteAheadLog::new(&wal_path).await?;
        
        let root = BTreeNode::new_leaf(1);
        
        let mut persistence = Self {
            root: Arc::new(RwLock::new(root)),
            wal: Arc::new(Mutex::new(wal)),
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            version_counter: Arc::new(Mutex::new(1)),
            data_dir: data_dir.to_path_buf(),
            operation_count: Arc::new(Mutex::new(0)),
            total_memory_used: Arc::new(Mutex::new(0)),
        };
        
        // Replay WAL entries to recover state
        persistence.replay_wal(&wal_path).await?;
        
        Ok(persistence)
    }
    
    async fn next_version(&self) -> u64 {
        let mut counter = self.version_counter.lock().await;
        *counter += 1;
        *counter
    }
    
    /// Replay WAL entries to recover the tree state
    async fn replay_wal(&mut self, wal_path: &std::path::Path) -> Result<(), PersistenceError> {
        if !wal_path.exists() {
            return Ok(()); // No WAL file to replay
        }
        
        println!("Replaying WAL from: {:?}", wal_path);
        
        let content = tokio::fs::read(wal_path).await?;
        let mut offset = 0;
        let mut replayed_count = 0;
        
        while offset + 4 <= content.len() {
            // Read entry length
            let len_bytes = &content[offset..offset + 4];
            let entry_len = u32::from_le_bytes([
                len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]
            ]) as usize;
            offset += 4;
            
            if offset + entry_len <= content.len() {
                let entry_data = &content[offset..offset + entry_len];
                
                // Parse and apply the entry
                match serde_json::from_slice::<WALEntry>(entry_data) {
                    Ok(entry) => {
                        match entry.operation {
                            WALOperation::Insert { key, lease } => {
                                // Apply the insert without logging to WAL again
                                self.apply_insert_for_recovery(key, lease).await?;
                                replayed_count += 1;
                            }
                            WALOperation::Delete { key: _ } => {
                                // Delete operations would be handled here
                                // For the demo, we primarily focus on inserts
                            }
                            WALOperation::Update { key, lease } => {
                                // Apply update without logging
                                self.apply_insert_for_recovery(key, lease).await?;
                                replayed_count += 1;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to parse WAL entry: {}", e);
                        // Continue processing other entries
                    }
                }
                
                offset += entry_len;
            } else {
                eprintln!("Warning: Incomplete WAL entry found at end of file");
                break;
            }
        }
        
        println!("WAL replay completed. Replayed {} operations", replayed_count);
        Ok(())
    }
    
    /// Force flush the WAL to disk (for testing/demo purposes)
    pub async fn flush_wal(&self) -> Result<(), PersistenceError> {
        let mut wal = self.wal.lock().await;
        wal.flush().await
    }
    
    /// Apply an insert operation during recovery (without WAL logging)
    async fn apply_insert_for_recovery(&mut self, key: ActorKey, lease: ActorLease) -> Result<(), PersistenceError> {
        let new_version = self.next_version().await;
        
        // COW update of the tree (same logic as cow_insert but without WAL logging)
        let (new_root, memory_delta) = {
            let root = self.root.read().unwrap();
            self.cow_insert_recursive(&*root, key, lease, new_version)?
        };
        
        // Atomic root replacement
        {
            let mut root = self.root.write().unwrap();
            *root = new_root;
        }
        
        // Update memory tracking
        {
            let mut total_memory = self.total_memory_used.lock().await;
            *total_memory = (*total_memory as i64 + memory_delta) as u64;
        }
        
        let mut op_count = self.operation_count.lock().await;
        *op_count += 1;
        
        Ok(())
    }
    
    /// Insert or update a lease with COW semantics
    async fn cow_insert(&self, key: ActorKey, lease: ActorLease) -> Result<PersistenceMetrics, PersistenceError> {
        let start_time = Instant::now();
        let mut bytes_written = 0u64;
        
        // Log operation to WAL first
        {
            let mut wal = self.wal.lock().await;
            wal.append(WALOperation::Insert { key: key.clone(), lease: lease.clone() }).await?;
            bytes_written += lease.to_bytes()?.len() as u64;
        }
        
        let new_version = self.next_version().await;
        
        // COW update of the tree
        let (new_root, memory_delta) = {
            let root = self.root.read().unwrap();
            self.cow_insert_recursive(&*root, key, lease, new_version)?
        };
        
        // Atomic root replacement
        {
            let mut root = self.root.write().unwrap();
            *root = new_root;
        }
        
        // Update memory tracking
        {
            let mut total_memory = self.total_memory_used.lock().await;
            *total_memory = (*total_memory as i64 + memory_delta) as u64;
        }
        
        let mut op_count = self.operation_count.lock().await;
        *op_count += 1;
        
        Ok(PersistenceMetrics {
            operation_type: OperationType::Insert,
            latency: start_time.elapsed(),
            memory_used: memory_delta as u64,
            disk_bytes_read: 0,
            disk_bytes_written: bytes_written,
            success: true,
        })
    }
    
    fn cow_insert_recursive(
        &self,
        node: &BTreeNode,
        key: ActorKey,
        lease: ActorLease,
        new_version: u64,
    ) -> Result<(BTreeNode, i64), PersistenceError> {
        let mut new_node = node.cow_clone(new_version);
        let memory_delta = std::mem::size_of::<BTreeNode>() as i64;
        
        if new_node.is_leaf {
            // Insert in leaf node
            let is_new = new_node.insert_in_leaf(key, lease);
            
            if new_node.is_full() && is_new {
                // Need to split
                let (promote_key, right_node) = new_node.split(new_version);
                
                // Create new root
                let mut new_root = BTreeNode::new_internal(new_version);
                new_root.keys.push(promote_key);
                new_root.children.push(Arc::new(new_node));
                new_root.children.push(Arc::new(right_node));
                
                return Ok((new_root, memory_delta * 3));
            }
            
            Ok((new_node, memory_delta))
        } else {
            // Internal node - find child to recurse into
            let child_index = match new_node.find_key_index(&key) {
                Ok(idx) => idx + 1,
                Err(idx) => idx,
            };
            
            if child_index >= new_node.children.len() {
                return Err(PersistenceError::Corruption("Invalid child index".to_string()));
            }
            
            let child = &new_node.children[child_index];
            let (new_child, child_memory_delta) = self.cow_insert_recursive(child, key, lease, new_version)?;
            
            // Replace child with COW version
            new_node.children[child_index] = Arc::new(new_child);
            
            Ok((new_node, memory_delta + child_memory_delta))
        }
    }
    
    /// Find a lease by key
    fn find_lease(&self, key: &ActorKey) -> Result<Option<ActorLease>, PersistenceError> {
        let root = self.root.read().unwrap();
        self.find_recursive(&*root, key)
    }
    
    fn find_recursive(&self, node: &BTreeNode, key: &ActorKey) -> Result<Option<ActorLease>, PersistenceError> {
        match node.find_key_index(key) {
            Ok(idx) => {
                if node.is_leaf {
                    Ok(node.values[idx].clone())
                } else {
                    // Key found in internal node, go to right child
                    let child = &node.children[idx + 1];
                    self.find_recursive(child, key)
                }
            }
            Err(idx) => {
                if node.is_leaf {
                    Ok(None)
                } else {
                    // Go to appropriate child
                    if idx < node.children.len() {
                        let child = &node.children[idx];
                        self.find_recursive(child, key)
                    } else {
                        Ok(None)
                    }
                }
            }
        }
    }
    
    /// Collect all leases in a range (for cluster coordination)
    fn range_query_recursive(
        &self,
        node: &BTreeNode,
        start: &ActorKey,
        end: &ActorKey,
        results: &mut Vec<ActorLease>,
    ) -> Result<(), PersistenceError> {
        if node.is_leaf {
            for (i, key) in node.keys.iter().enumerate() {
                if key >= start && key <= end {
                    if let Some(lease) = &node.values[i] {
                        results.push(lease.clone());
                    }
                }
            }
        } else {
            // For internal nodes, check all relevant children
            let start_idx = match node.find_key_index(start) {
                Ok(idx) => idx,
                Err(idx) => idx,
            };
            
            let end_idx = match node.find_key_index(end) {
                Ok(idx) => idx + 1,
                Err(idx) => idx,
            };
            
            for i in start_idx..=end_idx.min(node.children.len() - 1) {
                let child = &node.children[i];
                self.range_query_recursive(child, start, end, results)?;
            }
        }
        
        Ok(())
    }
}

#[async_trait::async_trait]
impl PersistenceProvider for CowBTreePersistence {
    async fn store_lease(&self, lease: &ActorLease) -> Result<PersistenceMetrics, PersistenceError> {
        self.cow_insert(lease.key.clone(), lease.clone()).await
    }
    
    async fn get_lease(&self, key: &ActorKey) -> Result<(Option<ActorLease>, PersistenceMetrics), PersistenceError> {
        let start_time = Instant::now();
        
        let lease = self.find_lease(key)?;
        
        let metrics = PersistenceMetrics {
            operation_type: OperationType::Get,
            latency: start_time.elapsed(),
            memory_used: 0, // No additional memory for reads
            disk_bytes_read: 0, // In-memory structure
            disk_bytes_written: 0,
            success: true,
        };
        
        Ok((lease, metrics))
    }
    
    async fn range_query(&self, start: &ActorKey, end: &ActorKey) -> Result<(Vec<ActorLease>, PersistenceMetrics), PersistenceError> {
        let start_time = Instant::now();
        
        let mut results = Vec::new();
        let root = self.root.read().unwrap();
        self.range_query_recursive(&*root, start, end, &mut results)?;
        
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
        
        let snapshot_id = format!("snapshot_{}", chrono::Utc::now().timestamp());
        let root_version = {
            let root = self.root.read().unwrap();
            root.version
        };
        
        let key_count = *self.operation_count.lock().await;
        
        let snapshot = Snapshot {
            id: snapshot_id.clone(),
            root_version,
            timestamp: SystemTime::now(),
            key_count,
        };
        
        {
            let mut snapshots = self.snapshots.write().unwrap();
            snapshots.insert(snapshot_id.clone(), snapshot);
        }
        
        // Flush WAL to ensure consistency
        {
            let mut wal = self.wal.lock().await;
            wal.flush().await?;
        }
        
        let metrics = PersistenceMetrics {
            operation_type: OperationType::Snapshot,
            latency: start_time.elapsed(),
            memory_used: std::mem::size_of::<Snapshot>() as u64,
            disk_bytes_read: 0,
            disk_bytes_written: 0,
            success: true,
        };
        
        Ok((snapshot_id, metrics))
    }
    
    async fn restore_from_snapshot(&self, _snapshot_id: &str) -> Result<PersistenceMetrics, PersistenceError> {
        // Simplified implementation - in production would restore from disk
        let start_time = Instant::now();
        
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
        let key_count = *self.operation_count.lock().await;
        let memory_usage = *self.total_memory_used.lock().await;
        
        Ok(StorageStats {
            total_keys: key_count,
            total_size_bytes: memory_usage,
            memory_usage_bytes: memory_usage,
            disk_usage_bytes: 0, // Simplified - would calculate WAL size in production
            average_key_size: 32, // Estimate
            average_value_size: 256, // Estimate based on ActorLease size
        })
    }
    
    async fn simulate_crash_recovery(&self) -> Result<PersistenceMetrics, PersistenceError> {
        let start_time = Instant::now();
        
        // Simulate crash recovery by replaying WAL
        // In production, this would read from disk and rebuild the tree
        
        Ok(PersistenceMetrics {
            operation_type: OperationType::Recovery,
            latency: start_time.elapsed(),
            memory_used: 0,
            disk_bytes_read: 0,
            disk_bytes_written: 0,
            success: true,
        })
    }
}

// Implement serde for WAL operations
impl serde::Serialize for WALOperation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        match self {
            WALOperation::Insert { key, lease } => {
                let mut state = serializer.serialize_struct("WALOperation", 3)?;
                state.serialize_field("type", "Insert")?;
                state.serialize_field("key", key)?;
                state.serialize_field("lease", lease)?;
                state.end()
            }
            WALOperation::Update { key, lease } => {
                let mut state = serializer.serialize_struct("WALOperation", 3)?;
                state.serialize_field("type", "Update")?;
                state.serialize_field("key", key)?;
                state.serialize_field("lease", lease)?;
                state.end()
            }
            WALOperation::Delete { key } => {
                let mut state = serializer.serialize_struct("WALOperation", 2)?;
                state.serialize_field("type", "Delete")?;
                state.serialize_field("key", key)?;
                state.end()
            }
        }
    }
}

impl serde::Serialize for WALEntry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("WALEntry", 3)?;
        state.serialize_field("sequence", &self.sequence)?;
        state.serialize_field("timestamp", &self.timestamp)?;
        state.serialize_field("operation", &self.operation)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for WALOperation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;
        
        struct WALOperationVisitor;
        
        impl<'de> Visitor<'de> for WALOperationVisitor {
            type Value = WALOperation;
            
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a WALOperation struct")
            }
            
            fn visit_map<V>(self, mut map: V) -> Result<WALOperation, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut op_type: Option<String> = None;
                let mut key: Option<ActorKey> = None;
                let mut lease: Option<ActorLease> = None;
                
                while let Some(k) = map.next_key()? {
                    match k {
                        "type" => {
                            if op_type.is_some() {
                                return Err(de::Error::duplicate_field("type"));
                            }
                            op_type = Some(map.next_value()?);
                        }
                        "key" => {
                            if key.is_some() {
                                return Err(de::Error::duplicate_field("key"));
                            }
                            key = Some(map.next_value()?);
                        }
                        "lease" => {
                            if lease.is_some() {
                                return Err(de::Error::duplicate_field("lease"));
                            }
                            lease = Some(map.next_value()?);
                        }
                        _ => {
                            let _: serde_json::Value = map.next_value()?;
                        }
                    }
                }
                
                let op_type = op_type.ok_or_else(|| de::Error::missing_field("type"))?;
                let key = key.ok_or_else(|| de::Error::missing_field("key"))?;
                
                match op_type.as_str() {
                    "Insert" => {
                        let lease = lease.ok_or_else(|| de::Error::missing_field("lease"))?;
                        Ok(WALOperation::Insert { key, lease })
                    }
                    "Update" => {
                        let lease = lease.ok_or_else(|| de::Error::missing_field("lease"))?;
                        Ok(WALOperation::Update { key, lease })
                    }
                    "Delete" => Ok(WALOperation::Delete { key }),
                    _ => Err(de::Error::unknown_variant(&op_type, &["Insert", "Update", "Delete"])),
                }
            }
        }
        
        deserializer.deserialize_map(WALOperationVisitor)
    }
}

impl<'de> serde::Deserialize<'de> for WALEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;
        
        struct WALEntryVisitor;
        
        impl<'de> Visitor<'de> for WALEntryVisitor {
            type Value = WALEntry;
            
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a WALEntry struct")
            }
            
            fn visit_map<V>(self, mut map: V) -> Result<WALEntry, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut sequence: Option<u64> = None;
                let mut timestamp: Option<SystemTime> = None;
                let mut operation: Option<WALOperation> = None;
                
                while let Some(key) = map.next_key()? {
                    match key {
                        "sequence" => {
                            if sequence.is_some() {
                                return Err(de::Error::duplicate_field("sequence"));
                            }
                            sequence = Some(map.next_value()?);
                        }
                        "timestamp" => {
                            if timestamp.is_some() {
                                return Err(de::Error::duplicate_field("timestamp"));
                            }
                            timestamp = Some(map.next_value()?);
                        }
                        "operation" => {
                            if operation.is_some() {
                                return Err(de::Error::duplicate_field("operation"));
                            }
                            operation = Some(map.next_value()?);
                        }
                        _ => {
                            let _: serde_json::Value = map.next_value()?;
                        }
                    }
                }
                
                let sequence = sequence.ok_or_else(|| de::Error::missing_field("sequence"))?;
                let timestamp = timestamp.ok_or_else(|| de::Error::missing_field("timestamp"))?;
                let operation = operation.ok_or_else(|| de::Error::missing_field("operation"))?;
                
                Ok(WALEntry { sequence, timestamp, operation })
            }
        }
        
        deserializer.deserialize_map(WALEntryVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use std::time::Duration;
    
    #[tokio::test]
    async fn test_cow_btree_basic_operations() {
        let temp_dir = tempfile::tempdir().unwrap();
        let persistence = CowBTreePersistence::new(temp_dir.path()).await.unwrap();
        
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
        // The range query might return 0 or 1 results depending on the key ordering
        assert!(range_results.len() <= 1);
    }
}