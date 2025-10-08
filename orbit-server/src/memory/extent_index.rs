use anyhow::{anyhow, Result};
use bitflags::bitflags;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtentRef {
    pub file_id: u32,
    pub offset: u64,
    pub len: u32,
    pub flags: ExtentFlags,
}

impl ExtentRef {
    pub fn as_pin_key(&self) -> u64 {
        // Create a unique key from file_id and offset
        ((self.file_id as u64) << 32) | (self.offset >> 12) // Shift offset by page size
    }
}

#[bitflags]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExtentFlags {
    const HOT         = 0b00000001;  // Frequently accessed
    const COMPRESSED  = 0b00000010;  // Uses compression
    const HUGEPAGE    = 0b00000100;  // Prefers huge pages
    const NUMA_LOCAL  = 0b00001000;  // NUMA-local placement
    const PREFETCH    = 0b00010000;  // Prefetch adjacent extents
}

#[derive(Debug, Clone)]
pub struct ExtentConnections {
    pub forward_extents: Vec<ExtentRef>,   // Next extents in sequence
    pub back_extents: Vec<ExtentRef>,      // Previous extents  
    pub hot_neighbors: Vec<ExtentRef>,     // Frequently co-accessed
    pub prefetch_weight: f32,              // 0.0-1.0 prefetch priority
}

#[derive(Debug, Clone, Copy)]
pub enum AccessType {
    SequentialRead,
    RandomRead, 
    Write,
    Scan,
}

#[derive(Debug)]
struct ExtentMetadata {
    extent: ExtentRef,
    connections: ExtentConnections,
    access_count: AtomicU64,
    last_access: std::sync::RwLock<Instant>,
    hotness_score: std::sync::atomic::AtomicU32, // f32 as u32 bits
}

pub trait ExtentIndex: Send + Sync {
    fn lookup_rowgroup(&self, table_id: u64, rg_id: u64) -> &[ExtentRef];
    fn lookup_graph_partition(&self, part_id: u64) -> &[ExtentRef];
    fn lookup_vector_list(&self, list_id: u64) -> &[ExtentRef];
    fn get_connections(&self, extent: &ExtentRef) -> Option<&ExtentConnections>;
    fn record_access(&self, extent: &ExtentRef, access_type: AccessType);
    fn get_hot_extents(&self, limit: usize) -> Vec<(ExtentRef, f32)>;
}

pub struct DefaultExtentIndex {
    // Table mappings: table_id -> rowgroup_id -> extents
    table_extents: DashMap<u64, DashMap<u64, Vec<ExtentRef>>>,
    
    // Graph mappings: partition_id -> extents
    graph_extents: DashMap<u64, Vec<ExtentRef>>,
    
    // Vector mappings: list_id -> extents
    vector_extents: DashMap<u64, Vec<ExtentRef>>,
    
    // Extent metadata and connections
    extent_metadata: DashMap<u64, Arc<ExtentMetadata>>, // keyed by extent hash
    
    // Access pattern learning
    hotness_decay_factor: f32,
    access_weights: HashMap<AccessType, f32>,
}

impl DefaultExtentIndex {
    pub fn new() -> Self {
        let mut access_weights = HashMap::new();
        access_weights.insert(AccessType::SequentialRead, 1.0);
        access_weights.insert(AccessType::RandomRead, 2.0);
        access_weights.insert(AccessType::Write, 1.5);
        access_weights.insert(AccessType::Scan, 0.5);

        Self {
            table_extents: DashMap::new(),
            graph_extents: DashMap::new(),
            vector_extents: DashMap::new(),
            extent_metadata: DashMap::new(),
            hotness_decay_factor: 0.95,
            access_weights,
        }
    }

    pub fn add_table_extent(&self, table_id: u64, rg_id: u64, extent: ExtentRef) {
        let table_map = self.table_extents.entry(table_id).or_insert_with(DashMap::new);
        let rg_extents = table_map.entry(rg_id).or_insert_with(Vec::new);
        rg_extents.push(extent);

        // Add metadata
        self.add_extent_metadata(extent);
    }

    pub fn add_graph_extent(&self, part_id: u64, extent: ExtentRef) {
        let mut extents = self.graph_extents.entry(part_id).or_insert_with(Vec::new);
        extents.push(extent);

        // Add metadata
        self.add_extent_metadata(extent);
    }

    pub fn add_vector_extent(&self, list_id: u64, extent: ExtentRef) {
        let mut extents = self.vector_extents.entry(list_id).or_insert_with(Vec::new);
        extents.push(extent);

        // Add metadata
        self.add_extent_metadata(extent);
    }

    fn add_extent_metadata(&self, extent: ExtentRef) {
        let extent_key = self.extent_hash(&extent);
        
        if !self.extent_metadata.contains_key(&extent_key) {
            let metadata = Arc::new(ExtentMetadata {
                extent,
                connections: ExtentConnections {
                    forward_extents: Vec::new(),
                    back_extents: Vec::new(),
                    hot_neighbors: Vec::new(),
                    prefetch_weight: 0.0,
                },
                access_count: AtomicU64::new(0),
                last_access: std::sync::RwLock::new(Instant::now()),
                hotness_score: std::sync::atomic::AtomicU32::new(0),
            });
            
            self.extent_metadata.insert(extent_key, metadata);
        }
    }

    pub fn set_connections(&self, extent: &ExtentRef, connections: ExtentConnections) {
        let extent_key = self.extent_hash(extent);
        if let Some(mut metadata) = self.extent_metadata.get_mut(&extent_key) {
            // We need to replace the entire Arc because ExtentConnections doesn't have interior mutability
            let old_metadata = metadata.value();
            let new_metadata = Arc::new(ExtentMetadata {
                extent: old_metadata.extent,
                connections,
                access_count: AtomicU64::new(old_metadata.access_count.load(Ordering::Relaxed)),
                last_access: std::sync::RwLock::new(*old_metadata.last_access.read().unwrap()),
                hotness_score: std::sync::atomic::AtomicU32::new(old_metadata.hotness_score.load(Ordering::Relaxed)),
            });
            *metadata.value_mut() = new_metadata;
        }
    }

    fn extent_hash(&self, extent: &ExtentRef) -> u64 {
        // Simple hash based on file_id and offset
        ((extent.file_id as u64) << 32) | (extent.offset >> 12)
    }

    fn update_hotness_score(&self, extent: &ExtentRef, access_type: AccessType) {
        let extent_key = self.extent_hash(extent);
        if let Some(metadata) = self.extent_metadata.get(&extent_key) {
            let weight = self.access_weights.get(&access_type).copied().unwrap_or(1.0);
            
            // Load current hotness score (stored as f32 bits in u32)
            let current_bits = metadata.hotness_score.load(Ordering::Relaxed);
            let current_score = f32::from_bits(current_bits);
            
            // Apply decay and add new access weight
            let new_score = (current_score * self.hotness_decay_factor + weight).min(100.0);
            let new_bits = new_score.to_bits();
            
            metadata.hotness_score.store(new_bits, Ordering::Relaxed);
            
            // Update last access time
            if let Ok(mut last_access) = metadata.last_access.write() {
                *last_access = Instant::now();
            }
            
            // Increment access count
            metadata.access_count.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl ExtentIndex for DefaultExtentIndex {
    fn lookup_rowgroup(&self, table_id: u64, rg_id: u64) -> &[ExtentRef] {
        if let Some(table_map) = self.table_extents.get(&table_id) {
            if let Some(extents) = table_map.get(&rg_id) {
                return extents.as_slice();
            }
        }
        &[]
    }

    fn lookup_graph_partition(&self, part_id: u64) -> &[ExtentRef] {
        if let Some(extents) = self.graph_extents.get(&part_id) {
            extents.as_slice()
        } else {
            &[]
        }
    }

    fn lookup_vector_list(&self, list_id: u64) -> &[ExtentRef] {
        if let Some(extents) = self.vector_extents.get(&list_id) {
            extents.as_slice()
        } else {
            &[]
        }
    }

    fn get_connections(&self, extent: &ExtentRef) -> Option<&ExtentConnections> {
        let extent_key = self.extent_hash(extent);
        self.extent_metadata
            .get(&extent_key)
            .map(|metadata| &metadata.connections)
    }

    fn record_access(&self, extent: &ExtentRef, access_type: AccessType) {
        self.update_hotness_score(extent, access_type);
    }

    fn get_hot_extents(&self, limit: usize) -> Vec<(ExtentRef, f32)> {
        let mut hot_extents: Vec<_> = self.extent_metadata
            .iter()
            .map(|entry| {
                let metadata = entry.value();
                let hotness_bits = metadata.hotness_score.load(Ordering::Relaxed);
                let hotness = f32::from_bits(hotness_bits);
                (metadata.extent, hotness)
            })
            .collect();

        // Sort by hotness score (descending)
        hot_extents.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        // Take top N
        hot_extents.truncate(limit);
        
        hot_extents
    }
}

impl Default for DefaultExtentIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extent_ref_pin_key() {
        let extent = ExtentRef {
            file_id: 1,
            offset: 4096,
            len: 1024,
            flags: ExtentFlags::HOT,
        };
        
        let pin_key = extent.as_pin_key();
        assert_eq!(pin_key, (1u64 << 32) | 1); // file_id in high bits, offset/4096 in low bits
    }

    #[test]
    fn test_extent_flags() {
        let flags = ExtentFlags::HOT | ExtentFlags::HUGEPAGE;
        assert!(flags.contains(ExtentFlags::HOT));
        assert!(flags.contains(ExtentFlags::HUGEPAGE));
        assert!(!flags.contains(ExtentFlags::COMPRESSED));
    }

    #[test]
    fn test_extent_index_table_operations() {
        let index = DefaultExtentIndex::new();
        
        let extent = ExtentRef {
            file_id: 1,
            offset: 0,
            len: 1024,
            flags: ExtentFlags::HOT,
        };
        
        index.add_table_extent(100, 1, extent);
        
        let extents = index.lookup_rowgroup(100, 1);
        assert_eq!(extents.len(), 1);
        assert_eq!(extents[0].file_id, 1);
        
        // Test non-existent rowgroup
        let empty_extents = index.lookup_rowgroup(100, 999);
        assert_eq!(empty_extents.len(), 0);
    }

    #[test]
    fn test_hotness_tracking() {
        let index = DefaultExtentIndex::new();
        
        let extent = ExtentRef {
            file_id: 1,
            offset: 0,
            len: 1024,
            flags: ExtentFlags::HOT,
        };
        
        index.add_table_extent(100, 1, extent);
        
        // Record some accesses
        index.record_access(&extent, AccessType::RandomRead);
        index.record_access(&extent, AccessType::SequentialRead);
        
        let hot_extents = index.get_hot_extents(10);
        assert_eq!(hot_extents.len(), 1);
        assert!(hot_extents[0].1 > 0.0); // Should have some hotness score
    }

    #[test]
    fn test_connections() {
        let index = DefaultExtentIndex::new();
        
        let extent = ExtentRef {
            file_id: 1,
            offset: 0,
            len: 1024,
            flags: ExtentFlags::HOT,
        };
        
        index.add_table_extent(100, 1, extent);
        
        let connections = ExtentConnections {
            forward_extents: vec![extent],
            back_extents: vec![],
            hot_neighbors: vec![],
            prefetch_weight: 0.8,
        };
        
        index.set_connections(&extent, connections);
        
        let retrieved_connections = index.get_connections(&extent);
        assert!(retrieved_connections.is_some());
        assert_eq!(retrieved_connections.unwrap().prefetch_weight, 0.8);
    }
}