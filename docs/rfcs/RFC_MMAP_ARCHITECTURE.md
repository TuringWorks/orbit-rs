---
layout: default
title: RFC: Memory-Mapped File Architecture for Orbit-RS
category: rfcs
---

## RFC: Memory-Mapped File Architecture for Orbit-RS

**RFC ID**: RFC-2024-001  
**Title**: Memory-Mapped File Persistence Architecture  
**Author**: Orbit-RS Team  
**Status**: Draft  
**Created**: 2025-01-08  
**Updated**: 2025-01-08  

## Abstract

This RFC proposes implementing a memory-mapped file (mmap) based persistence architecture for Orbit-RS to dramatically reduce memory requirements and infrastructure costs while maintaining high performance. The proposed architecture enables handling petabyte-scale data with 50-75% fewer resources compared to traditional approaches.

## Table of Contents

- [Motivation](#motivation)
- [Design Overview](#design-overview)
- [Technical Specification](#technical-specification)
- [Implementation Plan](#implementation-plan)
- [Deployment Strategies](#deployment-strategies)
- [Performance Analysis](#performance-analysis)
- [Security Considerations](#security-considerations)
- [Testing Strategy](#testing-strategy)
- [Migration Path](#migration-path)
- [Open Questions](#open-questions)

## Motivation

### Current Limitations

The existing persistence architecture in orbit-rs has several limitations for large-scale deployments:

1. **High Memory Requirements**: Traditional approaches require 32-128GB RAM per node
2. **Node Count**: Need 100-200 nodes for petabyte-scale data
3. **Cost**: Infrastructure costs of ~$100K/month for 1PB deployments
4. **Complexity**: Manual cache management and data tiering

### Proposed Benefits

Memory-mapped files offer significant advantages:

1. **Reduced Infrastructure**: 30-50 nodes instead of 100-200 (3x reduction)
2. **Lower Memory**: 16-48GB RAM per node (50-75% reduction)
3. **Cost Savings**: ~$27K/month savings vs traditional approach
4. **Automatic Tiering**: OS handles hot/cold data management
5. **Near-RAM Performance**: Modern NVMe provides <10μs latency

## Design Overview

### Architecture Components

```text
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                        │
├─────────────────────────────────────────────────────────────┤
│           Orbit-RS Actor System & Virtual Actors            │
├─────────────────────────────────────────────────────────────┤
│              Memory-Mapped Persistence Layer                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ MMap        │  │ Page        │  │ NUMA & THP          │  │
│  │ Provider    │  │ Manager     │  │ Optimization        │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                  Operating System                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ Virtual     │  │ Page        │  │ I/O Subsystem       │  │
│  │ Memory      │  │ Cache       │  │ (io_uring)          │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                    Storage Layer                            │
│          High-Performance NVMe SSDs (1-10TB per node)       │
└─────────────────────────────────────────────────────────────┘
```

### Key Design Principles

1. **Zero-Copy Operations**: Direct memory access to storage
2. **OS Integration**: Leverage kernel page cache and virtual memory
3. **NUMA Awareness**: Optimize for multi-socket server architectures
4. **Huge Page Support**: Use 2MB/1GB pages for better TLB efficiency
5. **Configurable**: Support various workload patterns and deployment types

## Technical Specification

### Core Components

#### 1. MMap Persistence Provider

```rust
/// Primary memory-mapped file persistence provider
pub struct MMapPersistenceProvider {
    /// Configuration for the mmap provider
    config: MMapConfig,
    /// Memory-mapped regions for different data types
    regions: HashMap<DataType, MMapRegion>,
    /// Page access tracking for optimization
    page_tracker: PageAccessTracker,
    /// NUMA topology information
    numa_topology: NumaTopology,
    /// I/O ring for async operations
    io_ring: Option<IoUring>,
    /// Performance metrics
    metrics: Arc<RwLock<MMapMetrics>>,
}

/// Configuration for memory-mapped file provider

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MMapConfig {
    /// Base directory for memory-mapped files
    pub data_dir: PathBuf,
    /// Size of each memory-mapped file in GB
    pub file_size_gb: usize,
    /// Maximum total mapped size per node in GB
    pub max_mapped_size_gb: usize,
    /// Enable transparent huge pages
    pub enable_huge_pages: bool,
    /// Huge page size (2MB or 1GB)
    pub huge_page_size: HugePageSize,
    /// Prefault pages on mapping
    pub prefault_pages: bool,
    /// Memory advice for access patterns
    pub memory_advice: MemoryAdvice,
    /// NUMA policy for memory allocation
    pub numa_policy: NumaPolicy,
    /// Enable io_uring for async I/O
    pub enable_io_uring: bool,
    /// I/O queue depth for io_uring
    pub io_queue_depth: u32,
    /// Sync mode for durability
    pub sync_mode: SyncMode,
}
```

#### 2. Memory-Mapped Region Management

```rust
/// Represents a memory-mapped file region
pub struct MMapRegion {
    /// File descriptor
    file: File,
    /// Memory mapping
    mmap: MmapMut,
    /// Region size in bytes
    size: usize,
    /// Base address of the mapping
    base_addr: *mut u8,
    /// NUMA node where this region is allocated
    numa_node: Option<u32>,
    /// Access pattern statistics
    access_stats: AccessStats,
}

impl MMapRegion {
    /// Create a new memory-mapped region
    pub async fn new(
        path: &Path, 
        size: usize, 
        config: &MMapConfig
    ) -> OrbitResult<Self> {
        let file = Self::create_file(path, size).await?;
        let mmap = Self::create_mapping(&file, size, config).await?;
        
        Ok(Self {
            file,
            mmap,
            size,
            base_addr: mmap.as_mut_ptr(),
            numa_node: Self::detect_numa_node(),
            access_stats: AccessStats::new(),
        })
    }
    
    /// Perform zero-copy read
    pub unsafe fn read_at<T>(&self, offset: usize) -> OrbitResult<T> 
    where 
        T: Copy + 'static,
    {
        if offset + std::mem::size_of::<T>() > self.size {
            return Err(OrbitError::invalid_argument("Offset out of bounds"));
        }
        
        let ptr = self.base_addr.add(offset) as *const T;
        Ok(ptr.read_volatile())
    }
    
    /// Perform zero-copy write
    pub unsafe fn write_at<T>(&mut self, offset: usize, value: T) -> OrbitResult<()>
    where 
        T: Copy + 'static,
    {
        if offset + std::mem::size_of::<T>() > self.size {
            return Err(OrbitError::invalid_argument("Offset out of bounds"));
        }
        
        let ptr = self.base_addr.add(offset) as *mut T;
        ptr.write_volatile(value);
        
        // Update access statistics
        self.access_stats.record_write(offset);
        
        Ok(())
    }
}
```

#### 3. Transparent Huge Page Management

```rust
/// Transparent Huge Page (THP) configuration

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HugePageSize {
    /// 2MB huge pages (default)
    Size2MB,
    /// 1GB huge pages (for very large datasets)
    Size1GB,
    /// System default
    SystemDefault,
}

/// Huge page manager
pub struct HugePageManager {
    /// Current huge page configuration
    config: HugePageConfig,
    /// Available huge page sizes
    available_sizes: Vec<HugePageSize>,
    /// Huge page statistics
    stats: HugePageStats,
}

impl HugePageManager {
    /// Enable transparent huge pages for a memory region
    pub fn enable_thp(&self, addr: *mut u8, size: usize) -> OrbitResult<()> {
        unsafe {
            let result = libc::madvise(
                addr as *mut libc::c_void,
                size,
                libc::MADV_HUGEPAGE
            );
            
            if result != 0 {
                return Err(OrbitError::system(
                    format!("Failed to enable THP: {}", errno::errno())
                ));
            }
        }
        
        self.stats.thp_enabled_regions.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    
    /// Configure transparent huge page behavior
    pub fn configure_thp(&self, config: &MMapConfig) -> OrbitResult<()> {
        match config.huge_page_size {
            HugePageSize::Size2MB => {
                self.write_sys_file("/sys/kernel/mm/transparent_hugepage/enabled", "madvise")?;
                self.write_sys_file("/sys/kernel/mm/transparent_hugepage/defrag", "defer")?;
            },
            HugePageSize::Size1GB => {
                // Configure 1GB huge pages (requires kernel support)
                self.configure_1gb_pages()?;
            },
            HugePageSize::SystemDefault => {
                // Use system defaults
            }
        }
        
        Ok(())
    }
}
```

#### 4. NUMA Optimization

```rust
/// NUMA topology and optimization
pub struct NumaTopology {
    /// Number of NUMA nodes
    node_count: u32,
    /// CPU to NUMA node mapping
    cpu_to_node: HashMap<u32, u32>,
    /// Memory information per NUMA node
    node_memory: HashMap<u32, NumaNodeMemory>,
    /// Distance matrix between NUMA nodes
    distance_matrix: Vec<Vec<u32>>,
}

/// NUMA-aware memory allocation policy

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NumaPolicy {
    /// Allocate memory on the local NUMA node
    Local,
    /// Interleave memory across all NUMA nodes
    Interleave,
    /// Bind memory to specific NUMA nodes
    Bind(Vec<u32>),
    /// Prefer specific NUMA nodes but allow fallback
    Preferred(Vec<u32>),
}

impl NumaTopology {
    /// Detect NUMA topology
    pub fn detect() -> OrbitResult<Self> {
        let node_count = Self::get_numa_node_count()?;
        let cpu_to_node = Self::build_cpu_mapping()?;
        let node_memory = Self::get_node_memory_info()?;
        let distance_matrix = Self::build_distance_matrix(node_count)?;
        
        Ok(Self {
            node_count,
            cpu_to_node,
            node_memory,
            distance_matrix,
        })
    }
    
    /// Allocate memory with NUMA awareness
    pub fn numa_alloc(
        &self, 
        size: usize, 
        policy: &NumaPolicy
    ) -> OrbitResult<*mut u8> {
        match policy {
            NumaPolicy::Local => {
                let current_node = self.get_current_numa_node()?;
                self.alloc_on_node(size, current_node)
            },
            NumaPolicy::Interleave => {
                self.alloc_interleaved(size)
            },
            NumaPolicy::Bind(nodes) => {
                self.alloc_on_nodes(size, nodes)
            },
            NumaPolicy::Preferred(nodes) => {
                self.alloc_preferred(size, nodes)
            }
        }
    }
}
```

#### 5. I/O Ring Integration

```rust
/// I/O ring for high-performance async I/O
pub struct IoRingManager {
    /// io_uring instance
    ring: IoUring,
    /// Submission queue entries
    sq_entries: u32,
    /// Completion queue entries
    cq_entries: u32,
    /// Features supported by the kernel
    features: IoUringFeatures,
    /// Performance counters
    counters: IoRingCounters,
}

impl IoRingManager {
    /// Initialize io_uring with optimal settings
    pub fn new(config: &MMapConfig) -> OrbitResult<Self> {
        let ring = IoUring::builder()
            .setup_cqsize(config.io_queue_depth * 2)
            .build(config.io_queue_depth)?;
            
        let features = Self::detect_features(&ring)?;
        
        Ok(Self {
            ring,
            sq_entries: config.io_queue_depth,
            cq_entries: config.io_queue_depth * 2,
            features,
            counters: IoRingCounters::new(),
        })
    }
    
    /// Submit async read operation
    pub async fn async_read(
        &mut self,
        fd: RawFd,
        buf: &mut [u8],
        offset: u64,
    ) -> OrbitResult<usize> {
        let read_e = opcode::Read::new(types::Fd(fd), buf.as_mut_ptr(), buf.len() as u32)
            .offset(offset);
            
        unsafe {
            self.ring.submission()
                .push(&read_e.build())
                .map_err(|e| OrbitError::io(e.to_string()))?;
        }
        
        self.ring.submit_and_wait(1)?;
        
        let cqe = self.ring.completion().next()
            .ok_or_else(|| OrbitError::io("No completion event"))?;
            
        let result = cqe.result();
        if result < 0 {
            return Err(OrbitError::io(format!("Read failed: {}", result)));
        }
        
        self.counters.reads_completed.fetch_add(1, Ordering::SeqCst);
        Ok(result as usize)
    }
    
    /// Submit async write operation
    pub async fn async_write(
        &mut self,
        fd: RawFd,
        buf: &[u8],
        offset: u64,
    ) -> OrbitResult<usize> {
        let write_e = opcode::Write::new(types::Fd(fd), buf.as_ptr(), buf.len() as u32)
            .offset(offset);
            
        unsafe {
            self.ring.submission()
                .push(&write_e.build())
                .map_err(|e| OrbitError::io(e.to_string()))?;
        }
        
        self.ring.submit_and_wait(1)?;
        
        let cqe = self.ring.completion().next()
            .ok_or_else(|| OrbitError::io("No completion event"))?;
            
        let result = cqe.result();
        if result < 0 {
            return Err(OrbitError::io(format!("Write failed: {}", result)));
        }
        
        self.counters.writes_completed.fetch_add(1, Ordering::SeqCst);
        Ok(result as usize)
    }
}
```

### Configuration System

#### Complete Configuration Structure

```rust
/// Complete configuration for memory-mapped file architecture

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrbitMMapConfig {
    /// Memory-mapped file provider configuration
    pub mmap: MMapConfig,
    /// Huge page configuration
    pub huge_pages: HugePageConfig,
    /// NUMA optimization configuration
    pub numa: NumaConfig,
    /// I/O ring configuration
    pub io_ring: IoRingConfig,
    /// Performance tuning
    pub performance: PerformanceConfig,
    /// Monitoring configuration
    pub monitoring: MonitoringConfig,
}

/// Huge page specific configuration

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HugePageConfig {
    /// Enable transparent huge pages
    pub enabled: bool,
    /// Huge page size preference
    pub size: HugePageSize,
    /// Defragmentation strategy
    pub defrag: DefragStrategy,
    /// Reserve huge pages at startup
    pub reserve_pages: Option<u32>,
}

/// NUMA configuration

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumaConfig {
    /// Enable NUMA optimizations
    pub enabled: bool,
    /// Memory allocation policy
    pub policy: NumaPolicy,
    /// CPU affinity settings
    pub cpu_affinity: CpuAffinityConfig,
    /// Memory migration policy
    pub memory_migration: MemoryMigrationPolicy,
}

/// I/O ring configuration

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IoRingConfig {
    /// Enable io_uring
    pub enabled: bool,
    /// Submission queue depth
    pub sq_entries: u32,
    /// Completion queue depth multiplier
    pub cq_multiplier: f32,
    /// I/O polling mode
    pub polling_mode: IoPollingMode,
    /// Kernel submission queue polling
    pub sq_polling: bool,
}
```

#### Default Configuration Files

**Complete TOML Configuration Example:**

```toml

# Orbit-RS Memory-Mapped File Configuration

[server]
persistence_backend = "memory_mapped"
bind_address = "0.0.0.0:50051"

[mmap]
data_dir = "/mnt/nvme/orbit-data"
file_size_gb = 1000
max_mapped_size_gb = 2000
enable_huge_pages = true
huge_page_size = "2MB"
prefault_pages = false
memory_advice = "random"
numa_policy = "local"
enable_io_uring = true
io_queue_depth = 128
sync_mode = "async"

[huge_pages]
enabled = true
size = "2MB"  # Options: "2MB", "1GB", "system_default"
defrag = "defer"  # Options: "always", "defer", "never"
reserve_pages = 1024  # Reserve 1024 * 2MB = 2GB huge pages

[numa]
enabled = true
policy = "local"  # Options: "local", "interleave", "bind", "preferred"
cpu_affinity = "numa_local"  # Bind threads to local NUMA node CPUs
memory_migration = "on_fault"  # Options: "disabled", "on_fault", "proactive"

[io_ring]
enabled = true
sq_entries = 256
cq_multiplier = 2.0  # CQ size = SQ size * multiplier
polling_mode = "interrupt"  # Options: "interrupt", "polling", "hybrid"
sq_polling = false  # Kernel-side SQ polling

[performance]
worker_threads = 0  # Auto-detect based on CPU cores
enable_cpu_affinity = true
memory_prefetch = true
page_access_tracking = true
adaptive_readahead = true

[monitoring]
enabled = true
page_fault_tracking = true
memory_usage_reporting = true
io_latency_histogram = true
numa_statistics = true
```

**Kubernetes ConfigMap Integration:**

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: orbit-mmap-config
data:
  orbit-server.toml: |
    # Include the complete TOML configuration above
    
  # Environment-specific overrides
  production.toml: |
    [mmap]
    file_size_gb = 2000
    sync_mode = "sync"
    
    [monitoring]
    enabled = true
    metrics_export_interval = 15
    
  development.toml: |
    [mmap]
    file_size_gb = 10
    enable_huge_pages = false
    
    [io_ring]
    enabled = false  # May not be available in dev environments
```

## Implementation Plan

### Phase 1: Core Infrastructure (Weeks 1-4)

#### Week 1-2: Basic MMap Provider

- [ ] Implement `MMapPersistenceProvider` struct
- [ ] Basic memory mapping functionality
- [ ] File creation and management
- [ ] Zero-copy read/write operations
- [ ] Integration with existing persistence traits

#### Week 3-4: Configuration System

- [ ] Complete `MMapConfig` structure
- [ ] TOML configuration parsing
- [ ] Environment variable overrides
- [ ] Configuration validation
- [ ] Default configuration profiles

### Phase 2: Advanced Features (Weeks 5-8)

#### Week 5-6: Transparent Huge Page Support

- [ ] `HugePageManager` implementation
- [ ] THP detection and configuration
- [ ] 2MB huge page support
- [ ] 1GB huge page support (where available)
- [ ] THP monitoring and statistics

#### Week 7-8: NUMA Optimization

- [ ] `NumaTopology` detection
- [ ] NUMA-aware memory allocation
- [ ] CPU affinity management
- [ ] Memory migration policies
- [ ] NUMA performance monitoring

### Phase 3: I/O Optimization (Weeks 9-12)

#### Week 9-10: I/O Ring Integration

- [ ] `IoRingManager` implementation
- [ ] Async read/write operations
- [ ] Polling mode support
- [ ] Error handling and fallbacks
- [ ] Performance benchmarking

#### Week 11-12: Performance Tuning

- [ ] Page access tracking
- [ ] Adaptive prefetching
- [ ] Memory pressure handling
- [ ] Hot/cold data management
- [ ] Performance optimization algorithms

### Phase 4: Deployment & Operations (Weeks 13-16)

#### Week 13-14: Kubernetes Integration

- [ ] Optimized StatefulSet configurations
- [ ] Node preparation scripts
- [ ] Storage class definitions
- [ ] Monitoring and alerting
- [ ] Auto-scaling configurations

#### Week 15-16: Production Readiness

- [ ] Comprehensive testing
- [ ] Performance benchmarks
- [ ] Documentation
- [ ] Migration tools
- [ ] Operational runbooks

## Deployment Strategies

### 1. Kubernetes Deployment

**Node Requirements:**

```yaml
nodeRequirements:
  cpu: "16-32 cores"
  memory: "32-96 GB"
  storage: "2-10 TB NVMe SSD"
  network: "25+ Gbps"
  kernel: "Linux 5.4+ (io_uring support)"
```

**Example Deployment:**

```bash

# Label nodes for mmap optimization
kubectl label nodes <node-name> orbit-rs/mmap-optimized=true

# Deploy storage classes
kubectl apply -f k8s/mmap-optimized/01-storage-mmap.yaml

# Deploy configurations
kubectl apply -f k8s/mmap-optimized/02-configmap-mmap.yaml

# Deploy the optimized StatefulSet
kubectl apply -f k8s/mmap-optimized/03-statefulset-mmap.yaml

# Deploy monitoring and node preparation
kubectl apply -f k8s/mmap-optimized/04-node-prep-monitoring.yaml
```

### 2. Single Node Deployment

For development and small-scale deployments:

```toml

# single-node-config.toml
[mmap]
data_dir = "/data/orbit-mmap"
file_size_gb = 100
max_mapped_size_gb = 200
enable_huge_pages = true
huge_page_size = "2MB"

[numa]
enabled = false  # Single socket system

[performance]
worker_threads = 8  # Match available cores
```

**Setup Commands:**

```bash

# Prepare the system
sudo sysctl -w vm.max_map_count=2097152
sudo echo madvise > /sys/kernel/mm/transparent_hugepage/enabled

# Create data directory
sudo mkdir -p /data/orbit-mmap
sudo chown $USER:$USER /data/orbit-mmap

# Run orbit-rs
./orbit-server --config single-node-config.toml
```

### 3. VM-Based Cluster Deployment

For traditional VM deployments (AWS EC2, Azure VMs, GCP Compute):

**Terraform Configuration:**

```hcl
resource "aws_instance" "orbit_mmap_nodes" {
  count = 30  # Reduced from traditional 100+ nodes
  
  # High-performance instance types with NVMe
  instance_type = "i3en.4xlarge"  # 16 vCPU, 128GB RAM, 2x1.9TB NVMe
  
  # Optimized AMI with kernel 5.4+
  ami = var.optimized_ami_id
  
  # Enhanced networking
  enhanced_networking = true
  sriov_net_support = "simple"
  
  # Instance storage configuration
  ephemeral_block_device {
    device_name = "/dev/xvdb"
    virtual_name = "ephemeral0"
  }
  
  # User data for system optimization
  user_data = base64encode(templatefile("user-data.sh", {
    node_id = count.index
  }))
  
  tags = {
    Name = "orbit-mmap-node-${count.index}"
    Type = "orbit-rs-mmap"
  }
}
```

**VM Initialization Script:**

```bash

#!/bin/bash
# user-data.sh - VM initialization for mmap optimization

# Update system
yum update -y

# Install dependencies
yum install -y nvme-cli fio

# Configure transparent huge pages
echo madvise > /sys/kernel/mm/transparent_hugepage/enabled
echo defer > /sys/kernel/mm/transparent_hugepage/defrag

# Increase mmap limits
echo "vm.max_map_count=2097152" >> /etc/sysctl.conf
echo "vm.mmap_min_addr=4096" >> /etc/sysctl.conf
echo "kernel.shmmax=68719476736" >> /etc/sysctl.conf

# Apply sysctl changes
sysctl -p

# Format and mount NVMe drives
mkfs.ext4 -F /dev/nvme1n1
mkdir -p /mnt/orbit-data
mount -o noatime,nodiratime /dev/nvme1n1 /mnt/orbit-data

# Add to fstab
echo "/dev/nvme1n1 /mnt/orbit-data ext4 noatime,nodiratime 0 2" >> /etc/fstab

# Download and start orbit-rs
wget https://releases.orbit-rs.com/latest/orbit-server
chmod +x orbit-server
./orbit-server --config /etc/orbit-rs/mmap-cluster.toml
```

**Cluster Configuration:**

```toml

# mmap-cluster.toml
[server]
persistence_backend = "memory_mapped"
node_id = "${NODE_ID}"
cluster_discovery = "consul"  # or "etcd", "kubernetes"

[mmap]
data_dir = "/mnt/orbit-data"
file_size_gb = 1900  # Match instance storage capacity
max_mapped_size_gb = 3800
enable_huge_pages = true

[cluster]
discovery_endpoints = [
  "consul.orbit-cluster.internal:8500"
]
```

### 4. Ultra-Dense 3-Node Deployment (1PB with 3 Nodes)

**Challenge**: Handle 1PB of data with only 3 dedicated nodes

**Solution**: Ultra-high density configuration with memory-mapped files

#### Resource Requirements Per Node

```yaml
ultra_dense_node_specs:
  data_per_node: "334TB (1PB ÷ 3 nodes)"
  storage_with_replication: "800TB per node (2x replication + 20% buffer)"
  
  hardware:
    cpu: "32-64 cores"
    memory: "256-512 GB RAM" 
    storage: "800TB NVMe (RAID 0 across multiple drives)"
    network: "100 Gbps"
    instance_type: "AWS i4i.24xlarge or bare metal"
    
  storage_calculation:
    raw_data: "1 PB"
    replication: "2x = 2 PB total"
    safety_buffer: "20% = 2.4 PB required"
    per_node: "2.4 PB ÷ 3 = 800 TB per node"
    
  memory_calculation:
    working_set: "10% of 334TB = 33TB"
    page_cache: "64-128 GB per node"
    application: "64-128 GB per node"
    total_memory: "256-512 GB per node"
```

#### Configuration Files

**Storage Configuration (`k8s/3-node-petabyte/01-storage-ultra-dense.yaml`)**

```yaml

# Ultra-high density storage class
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: orbit-ultra-dense-nvme
provisioner: ebs.csi.aws.com
parameters:
  type: gp3
  iops: "64000"                # Maximum IOPS
  throughput: "4000"           # 4 GB/s throughput
  encrypted: "true"
mountOptions:
  - "noatime,nodiratime"
  - "data=writeback"
  - "commit=60"              # Optimized for ultra-dense
---

# Node-specific 800TB volumes
apiVersion: v1
kind: PersistentVolume
metadata:
  name: orbit-ultra-node1-primary
spec:
  capacity:
    storage: 800Ti  # 800TB primary storage
  storageClassName: orbit-ultra-dense-nvme
  local:
    path: /mnt/orbit-ultra-storage
```

**Application Configuration (`k8s/3-node-petabyte/02-configmap-3node.yaml`)**

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: orbit-3node-config
data:
  orbit-server.toml: |
    [mmap]
    data_dir = "/mnt/orbit-ultra-storage/data"
    file_size_gb = 50000               # 50TB files
    max_mapped_size_gb = 350000        # 350TB mapped per node
    huge_page_size = "1GB"             # Use 1GB pages
    
    # Replication for 3-node cluster
    replication_factor = 2             # 2x replication
    consistency_level = "quorum"       # Require 2/3 nodes
    
    [memory]
    page_cache_limit_gb = 128          # Large page cache
    memory_pressure_threshold = 0.90   # High threshold
    
    [cluster]
    expected_nodes = 3
    
    # System optimizations for ultra-dense
    vm.max_map_count = 10485760         # 10M mmap regions
    vm.nr_hugepages = 128               # 128GB of 1GB pages
```

#### Deployment Commands

```bash

# Label nodes for ultra-dense deployment
kubectl label nodes node1 orbit-rs.io/node-id=node1
kubectl label nodes node1 orbit-rs.io/storage-tier=ultra-dense
kubectl label nodes node2 orbit-rs.io/node-id=node2  
kubectl label nodes node3 orbit-rs.io/node-id=node3

# Deploy ultra-dense storage
kubectl apply -f k8s/3-node-petabyte/01-storage-ultra-dense.yaml

# Deploy 3-node configuration
kubectl apply -f k8s/3-node-petabyte/02-configmap-3node.yaml

# Deploy ultra-dense StatefulSet
kubectl apply -f k8s/3-node-petabyte/03-statefulset-3node.yaml
```

**StatefulSet Configuration (`k8s/3-node-petabyte/03-statefulset-3node.yaml`)**

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: orbit-rs-ultra-dense
  labels:
    app: orbit-rs
    tier: ultra-dense
spec:
  replicas: 3
  serviceName: orbit-rs-ultra
  selector:
    matchLabels:
      app: orbit-rs
      tier: ultra-dense
  template:
    metadata:
      labels:
        app: orbit-rs
        tier: ultra-dense
    spec:
      affinity:
        podAntiAffinity:
          requiredDuringSchedulingIgnoredDuringExecution:
          - labelSelector:
              matchExpressions:
              - key: app
                operator: In
                values:
                - orbit-rs
            topologyKey: kubernetes.io/hostname
      initContainers:
      - name: system-tuning
        image: busybox:1.35
        command: ["sh", "-c"]
        args:
        - |
          echo "Configuring ultra-dense system settings..."
          # Configure huge pages
          echo 128 > /sys/kernel/mm/hugepages/hugepages-1048576kB/nr_hugepages
          
          # Configure memory mapping limits
          echo 10485760 > /proc/sys/vm/max_map_count
          
          # Configure file handle limits
          echo 16777216 > /proc/sys/fs/file-max
          
          # Configure network buffer sizes
          echo 134217728 > /proc/sys/net/core/rmem_max
          echo 134217728 > /proc/sys/net/core/wmem_max
          
          echo "System tuning complete for ultra-dense deployment"
        securityContext:
          privileged: true
        volumeMounts:
        - name: sys
          mountPath: /sys
        - name: proc
          mountPath: /proc
      containers:
      - name: orbit-rs
        image: orbit-rs:ultra-dense
        ports:
        - containerPort: 8080
          name: http
        - containerPort: 9090
          name: metrics
        - containerPort: 8000
          name: cluster
        env:
        - name: POD_NAME
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        - name: NODE_ID
          valueFrom:
            fieldRef:
              fieldPath: spec.nodeName
        - name: TOTAL_MEMORY
          valueFrom:
            resourceFieldRef:
              resource: limits.memory
        resources:
          requests:
            cpu: "16"
            memory: "256Gi"
            hugepages-1Gi: "128Gi"  # 128GB huge pages
          limits:
            cpu: "32"  
            memory: "512Gi"
            hugepages-1Gi: "128Gi"
        volumeMounts:
        - name: orbit-ultra-storage
          mountPath: /mnt/orbit-ultra-storage
        - name: config
          mountPath: /etc/orbit-rs
        - name: hugepage-1gi
          mountPath: /hugepages-1Gi
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 60
          periodSeconds: 30
          timeoutSeconds: 10
          failureThreshold: 5
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 15
          timeoutSeconds: 5
          failureThreshold: 3
      volumes:
      - name: config
        configMap:
          name: orbit-3node-config
      - name: sys
        hostPath:
          path: /sys
      - name: proc
        hostPath:
          path: /proc
      - name: hugepage-1gi
        emptyDir:
          medium: HugePages-1Gi
  volumeClaimTemplates:
  - metadata:
      name: orbit-ultra-storage
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: orbit-ultra-dense-nvme
      resources:
        requests:
          storage: 800Ti  # 800TB per replica
```

**Service Configuration (`k8s/3-node-petabyte/04-services.yaml`)**

```yaml
---

# Load balancer service for external access
apiVersion: v1
kind: Service
metadata:
  name: orbit-rs-ultra-lb
  labels:
    app: orbit-rs
    tier: ultra-dense
spec:
  type: LoadBalancer
  ports:
  - port: 8080
    targetPort: 8080
    name: http
  selector:
    app: orbit-rs
    tier: ultra-dense
---

# Headless service for StatefulSet
apiVersion: v1
kind: Service
metadata:
  name: orbit-rs-ultra
  labels:
    app: orbit-rs
    tier: ultra-dense
spec:
  clusterIP: None
  ports:
  - port: 8080
    name: http
  - port: 9090
    name: metrics
  - port: 8000
    name: cluster
  selector:
    app: orbit-rs
    tier: ultra-dense
---

# Monitoring service
apiVersion: v1
kind: Service
metadata:
  name: orbit-rs-ultra-metrics
  labels:
    app: orbit-rs
    tier: ultra-dense
  annotations:
    prometheus.io/scrape: "true"
    prometheus.io/port: "9090"
    prometheus.io/path: "/metrics"
spec:
  ports:
  - port: 9090
    targetPort: 9090
    name: metrics
  selector:
    app: orbit-rs
    tier: ultra-dense
```

**Health Check Script (`scripts/ultra-dense-health-check.sh`)**

```bash

#!/bin/bash
# Ultra-dense deployment health checker

set -euo pipefail

echo "=== Orbit-RS Ultra-Dense Health Check ==="
echo "Timestamp: $(date)"

# Check cluster status
echo "\n1. Checking cluster status..."
kubectl get pods -l app=orbit-rs,tier=ultra-dense -o wide

# Check storage usage
echo "\n2. Checking storage usage per node..."
for i in {0..2}; do
    echo "Node $i storage:"
    kubectl exec orbit-rs-ultra-dense-$i -- df -h /mnt/orbit-ultra-storage | tail -1
done

# Check memory usage
echo "\n3. Checking memory usage..."
for i in {0..2}; do
    echo "Node $i memory:"
    kubectl exec orbit-rs-ultra-dense-$i -- free -h | head -2 | tail -1
done

# Check huge page usage
echo "\n4. Checking huge page utilization..."
for i in {0..2}; do
    echo "Node $i huge pages:"
    kubectl exec orbit-rs-ultra-dense-$i -- cat /proc/meminfo | grep -i hugepage
done

# Check mmap region count
echo "\n5. Checking mmap regions..."
for i in {0..2}; do
    echo "Node $i mmap regions:"
    kubectl exec orbit-rs-ultra-dense-$i -- wc -l /proc/self/maps
done

# Check application health
echo "\n6. Checking application health endpoints..."
for i in {0..2}; do
    echo "Node $i health:"
    kubectl exec orbit-rs-ultra-dense-$i -- curl -s http://localhost:8080/health || echo "FAILED"
done

# Check performance metrics
echo "\n7. Checking performance metrics..."
echo "Total throughput across cluster:"
kubectl exec orbit-rs-ultra-dense-0 -- curl -s http://localhost:9090/metrics | \
    grep 'orbit_throughput_bytes_per_sec' | head -1

echo "\n=== Health Check Complete ==="
```

## 7. Advanced Memory Management

### 7.1 Overview

For ultra-low latency data operations, orbit-rs implements advanced memory management techniques including **object pinning**, **fragmentation strategies**, and **lifetime-aware garbage collection**. These optimizations are critical for maintaining sub-10μs tail latencies in petabyte-scale deployments.

**Core Concepts:**

- **Hot Object Pinning**: Keep frequently accessed tables, graph nodes, and virtual objects resident in memory
- **Intelligent Fragmentation**: Break large objects into connected slices with independent lifetime management
- **NUMA-Aware Placement**: Optimize memory placement based on CPU topology and access patterns
- **Predictive Prefetching**: Use query plans and access patterns to warm caches proactively

### 7.2 Memory Pinning Strategies

#### 7.2.1 Pin Budget and Priority System

```yaml
pin_management:
  global_budget:
    max_pinned_percentage: 40          # 40% of total RAM
    numa_local_preference: 80          # 80% prefer local NUMA node
    huge_page_allocation: 60           # 60% of pins use huge pages
    
  priority_classes:
    tail_latency_critical: 
      budget_share: 50                 # 50% of pin budget
      max_pin_time_ms: 5000           # 5 second maximum pin time
      
    query_critical:
      budget_share: 30                 # 30% of pin budget  
      max_pin_time_ms: 30000          # 30 second maximum
      
    background_optimization:
      budget_share: 20                 # 20% of pin budget
      max_pin_time_ms: 300000         # 5 minute maximum
```

#### 7.2.2 Pinning Techniques

**Page-Level Pinning:**

```rust
// Linux: mlock2(MLOCK_ONFAULT) - pin on first access
// macOS: mlock() fallback for development
pub fn pin_slice_lazy(addr: *mut u8, len: usize) -> Result<(), PinError> {
    #[cfg(target_os = "linux")]
    {
        unsafe { libc::mlock2(addr as *mut c_void, len, libc::MLOCK_ONFAULT) }
    }
    #[cfg(not(target_os = "linux"))]
    {
        unsafe { libc::mlock(addr as *mut c_void, len) }
    }
}
```

**NUMA-Aware Placement:**

```yaml
numa_policies:
  hot_tables: "bind_local"           # Bind to local NUMA node
  cold_indices: "interleave"         # Spread across NUMA nodes  
  shared_metadata: "preferred_local" # Prefer local, allow remote
  
  migration_triggers:
    cpu_imbalance_threshold: 0.3     # Migrate if 30%+ imbalanced
    memory_pressure_threshold: 0.85  # Migrate if 85%+ memory used
    cross_numa_latency_penalty: 2.5  # 2.5x latency penalty factor
```

**Huge Page Optimization:**

```yaml
huge_pages:
  transparent_huge_pages: "madvise" # THP only on explicit hint
  explicit_1gb_pages: true         # Use 1GB pages for large scans
  explicit_2mb_pages: true         # Use 2MB pages for indices
  
  allocation_strategy:
    large_sequential_scans: "1GB"   # 1GB pages for > 100MB scans
    btree_indices: "2MB"           # 2MB pages for B-tree nodes
    random_access_metadata: "4KB"  # Standard pages for metadata
```

### 7.3 Object Fragmentation and Slicing

#### 7.3.1 Extent-Based Storage

**Tables - Column-Oriented Slicing:**

```yaml
table_slicing:
  extent_size: "64MB"              # Base extent size
  hot_column_extent_size: "16MB"   # Smaller extents for hot columns
  rowgroup_size: "1M"              # 1M rows per group
  
  slicing_policies:
    by_access_pattern:
      hot_columns: ["user_id", "timestamp", "amount"]
      cold_columns: ["description", "metadata", "audit_log"]
    
    by_query_selectivity:
      high_selectivity: "16MB"       # Small extents for filters
      scan_heavy: "256MB"            # Large extents for scans
```

**Graphs - Community-Based Partitioning:**

```yaml
graph_slicing:
  partition_strategy: "community_detection"
  max_partition_size: "128MB"
  min_partition_size: "4MB"
  
  edge_handling:
    intra_partition_edges: "csr_blocks"    # Compressed sparse row
    cross_partition_edges: "portal_nodes"  # Lightweight connectors
    
  property_storage:
    node_properties: "separate_extents"    # Independent lifecycle
    edge_properties: "inline_with_csr"     # Co-located with edges
```

#### 7.3.2 Extent Indices and Connection Graph

**Extent Mapping Structure:**

```rust

#[derive(Clone, Copy)]
pub struct ExtentRef {
    pub file_id: u32,          // Which file contains this extent
    pub offset: u64,           // Byte offset within file
    pub len: u32,              // Length in bytes
    pub flags: ExtentFlags,    // HOT, COMPRESSED, HUGEPAGE, etc.
}

#[bitflags]
#[repr(u32)]
pub enum ExtentFlags {
    HOT         = 0b00000001,  // Frequently accessed
    COMPRESSED  = 0b00000010,  // Uses compression
    HUGEPAGE    = 0b00000100,  // Prefers huge pages
    NUMA_LOCAL  = 0b00001000,  // NUMA-local placement
    PREFETCH    = 0b00010000,  // Prefetch adjacent extents
}
```

**Connection and Prefetch Hints:**

```rust
pub struct ExtentConnections {
    pub forward_extents: Vec<ExtentRef>,   // Next extents in sequence
    pub back_extents: Vec<ExtentRef>,      // Previous extents  
    pub hot_neighbors: Vec<ExtentRef>,     // Frequently co-accessed
    pub prefetch_weight: f32,              // 0.0-1.0 prefetch priority
}
```

### 7.4 Lifetime Management and Garbage Collection

#### 7.4.1 Lifetime Classes

```rust

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LifetimeClass {
    Ephemeral,    // Per-operator scratch, dropped at operator end
    Session,      // Query/session caches, TTL-based expiration
    Task,         // Analytical job windows, phase-based lifecycle  
    LongLived,    // Base data and indices, manual management
}

pub struct LifetimePolicy {
    pub class: LifetimeClass,
    pub ttl_ms: Option<u64>,           // Time-to-live in milliseconds
    pub max_memory_mb: Option<u64>,    // Memory limit for this class
    pub gc_trigger: GcTrigger,         // When to start garbage collection
}

pub enum GcTrigger {
    TimeBasedTtl,                      // TTL expiration
    MemoryPressure(f32),               // Memory usage threshold (0.0-1.0)
    OperatorCompletion,                // End of query operator
    ExplicitDrop,                      // Manual deallocation
}
```

#### 7.4.2 Epoch-Based Reclamation

```rust
pub struct EpochManager {
    current_epoch: AtomicU64,
    retire_lists: Vec<Mutex<Vec<RetiredSlice>>>,  // Per-epoch retire list
    reader_epochs: DashMap<ThreadId, u64>,        // Active reader epochs
}

pub struct RetiredSlice {
    pub slice_key: PinKey,
    pub retire_epoch: u64,
    pub unmap_fn: Box<dyn FnOnce() + Send>,
}

impl EpochManager {
    pub fn enter_read(&self) -> ReadGuard {
        let epoch = self.current_epoch.load(Ordering::Acquire);
        self.reader_epochs.insert(std::thread::current().id(), epoch);
        ReadGuard { epoch_mgr: self }
    }
    
    pub fn retire_slice(&self, slice: RetiredSlice) {
        let epoch = self.current_epoch.load(Ordering::Acquire);
        self.retire_lists[epoch as usize % self.retire_lists.len()]
            .lock().unwrap().push(slice);
    }
    
    pub fn advance_epoch(&self) {
        let new_epoch = self.current_epoch.fetch_add(1, Ordering::AcqRel);
        self.gc_old_slices(new_epoch);
    }
}
```

### 7.5 Core APIs

#### 7.5.1 PinManager Interface

```rust

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct PinKey(pub u64);  // Logical object/slice identifier

#[derive(Debug, Clone, Copy)]
pub enum PinPriority {
    Background,              // Best-effort, can be evicted anytime
    QueryCritical,          // Important for query performance  
    TailLatencyCritical,    // Must stay pinned for low tail latency
}

#[derive(Debug, Clone)]
pub struct PinOpts {
    pub priority: PinPriority,
    pub numa_prefer: Option<u16>,        // NUMA node preference
    pub use_hugepages: bool,             // Request huge page allocation
    pub onfault: bool,                   // Use mlock2(MLOCK_ONFAULT) if available
    pub lifetime_class: LifetimeClass,   // Lifetime management class
    pub prefetch_adjacent: usize,        // Number of adjacent extents to prefetch
    pub ttl_ms: Option<u64>,             // Auto-demote after TTL
}

pub trait PinManager: Send + Sync {
    /// Pin a slice with the given options
    fn pin_slice(&self, key: PinKey, opts: &PinOpts) -> anyhow::Result<()>;
    
    /// Unpin a slice, allowing it to be swapped out
    fn unpin_slice(&self, key: PinKey);
    
    /// Change the priority of an already pinned slice
    fn promote(&self, key: PinKey, new_priority: PinPriority);
    
    /// Get current pinning statistics
    fn stats(&self) -> PinStats;
    
    /// Check if a slice is currently pinned
    fn is_pinned(&self, key: PinKey) -> bool;
    
    /// Force garbage collection of expired pins
    fn gc_expired(&self) -> usize;
}

#[derive(Debug, Clone)]
pub struct PinStats {
    pub total_pinned_bytes: u64,
    pub total_pinned_count: usize,
    pub budget_used_bytes: u64,
    pub budget_total_bytes: u64,
    pub huge_page_bytes: u64,
    pub numa_local_bytes: u64,
    pub by_priority: HashMap<PinPriority, PinPriorityStats>,
}

#[derive(Debug, Clone)]
pub struct PinPriorityStats {
    pub pinned_count: usize,
    pub pinned_bytes: u64,
    pub eviction_count: u64,
    pub average_pin_time_ms: f64,
}
```

#### 7.5.2 ExtentIndex Interface

```rust
pub trait ExtentIndex: Send + Sync {
    /// Look up extents for a table's row group
    fn lookup_rowgroup(&self, table_id: u64, rg_id: u64) -> &[ExtentRef];
    
    /// Look up extents for a graph partition
    fn lookup_graph_partition(&self, part_id: u64) -> &[ExtentRef];
    
    /// Look up extents for a vector index list
    fn lookup_vector_list(&self, list_id: u64) -> &[ExtentRef];
    
    /// Get connection information for prefetching
    fn get_connections(&self, extent: &ExtentRef) -> Option<&ExtentConnections>;
    
    /// Update hotness statistics for an extent
    fn record_access(&self, extent: &ExtentRef, access_type: AccessType);
    
    /// Get extents that should be considered for pinning
    fn get_hot_extents(&self, limit: usize) -> Vec<(ExtentRef, f32)>; // (extent, hotness_score)
}

#[derive(Debug, Clone, Copy)]
pub enum AccessType {
    SequentialRead,
    RandomRead, 
    Write,
    Scan,
}
```

### 7.6 Query Engine Integration

#### 7.6.1 Planner Hints and Prefetching

**SQL Query Planning:**

```rust
pub struct QueryPlanHints {
    pub predicted_rowgroups: Vec<u64>,     // Rowgroups likely to be accessed
    pub join_order_extents: Vec<ExtentRef>, // Extents needed for join execution
    pub filter_selectivity: HashMap<String, f32>, // Column selectivity estimates
    pub scan_pattern: ScanPattern,          // Sequential vs random access
}

pub enum ScanPattern {
    Sequential { prefetch_distance: usize },
    Random { locality_hint: f32 },
    Hybrid { seq_threshold: usize },
}

// Integration with query planner
impl QueryPlanner {
    pub fn generate_pin_hints(&self, plan: &ExecutionPlan) -> Vec<PinRequest> {
        let mut requests = Vec::new();
        
        // Pin critical path extents
        for operator in plan.critical_path() {
            if let Some(table_scan) = operator.as_table_scan() {
                let rowgroups = self.estimate_accessed_rowgroups(table_scan);
                for rg_id in rowgroups {
                    let extents = self.extent_index.lookup_rowgroup(table_scan.table_id, rg_id);
                    for extent in extents {
                        requests.push(PinRequest {
                            key: PinKey(extent.as_pin_key()),
                            priority: PinPriority::QueryCritical,
                            prefetch_neighbors: true,
                        });
                    }
                }
            }
        }
        
        requests
    }
}
```

**Graph Traversal Optimization:**

```rust
pub struct GraphTraversalHints {
    pub start_partitions: Vec<u64>,        // Starting graph partitions
    pub max_hops: usize,                   // Maximum traversal depth
    pub expansion_factor: f32,             // Expected fan-out per hop
    pub community_locality: f32,           // Likelihood of staying in community
}

impl GraphEngine {
    pub fn prepare_traversal(&self, hints: &GraphTraversalHints) -> anyhow::Result<()> {
        // Pin starting partitions
        for &part_id in &hints.start_partitions {
            let extents = self.extent_index.lookup_graph_partition(part_id);
            for extent in extents {
                self.pin_manager.pin_slice(
                    PinKey(extent.as_pin_key()),
                    &PinOpts {
                        priority: PinPriority::TailLatencyCritical,
                        use_hugepages: true,
                        prefetch_adjacent: 2, // Prefetch 2 neighbor partitions
                        lifetime_class: LifetimeClass::Session,
                        ..Default::default()
                    },
                )?;
            }
        }
        
        // Prefetch likely expansion targets
        self.prefetch_expansion_candidates(hints)?;
        
        Ok(())
    }
}
```

#### 7.6.2 Performance Monitoring and Adaptive Tuning

```yaml
performance_monitoring:
  metrics:
    page_fault_rate: "per_second"        # Page faults/sec per extent
    cache_hit_ratio: "percentage"        # % of accesses hitting pinned memory
    numa_locality_score: "0_to_1"        # % of accesses that are NUMA-local
    huge_page_utilization: "percentage"  # % of huge pages in use
    pin_budget_pressure: "0_to_1"       # How close to pin budget limit
    
  adaptive_thresholds:
    pin_pressure_threshold: 0.85         # Start evicting at 85% budget
    page_fault_spike_threshold: 1000     # Pin more aggressively above 1K faults/sec
    numa_miss_threshold: 0.3             # Rebalance if >30% cross-NUMA accesses
    
  auto_tuning:
    enable_adaptive_pinning: true        # Automatically adjust pin priorities
    learning_window_minutes: 30          # Learn access patterns over 30 min
    rebalancing_interval_minutes: 10     # Rebalance NUMA placement every 10 min
```

**Adaptive Pin Controller:**

```rust
pub struct AdaptivePinController {
    pin_manager: Arc<dyn PinManager>,
    extent_index: Arc<dyn ExtentIndex>,
    metrics_collector: MetricsCollector,
    learning_window: Duration,
}

impl AdaptivePinController {
    pub async fn optimization_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        
        loop {
            interval.tick().await;
            
            // Collect recent access patterns
            let hot_extents = self.extent_index.get_hot_extents(1000);
            let stats = self.pin_manager.stats();
            
            // Check if we need to evict low-priority pins
            if stats.budget_used_bytes as f64 / stats.budget_total_bytes as f64 > 0.85 {
                self.evict_cold_pins(&hot_extents).await;
            }
            
            // Pin newly hot extents
            for (extent, hotness) in hot_extents {
                if hotness > 0.7 && !self.pin_manager.is_pinned(PinKey(extent.as_pin_key())) {
                    let priority = if hotness > 0.9 {
                        PinPriority::TailLatencyCritical
                    } else {
                        PinPriority::QueryCritical
                    };
                    
                    let _ = self.pin_manager.pin_slice(
                        PinKey(extent.as_pin_key()),
                        &PinOpts {
                            priority,
                            use_hugepages: extent.len >= 2 * 1024 * 1024, // 2MB+
                            lifetime_class: LifetimeClass::Task,
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }
}
```

### 7.7 Virtual Actor Model Integration

#### 7.7.1 Actor-Centric Memory Management

The advanced memory management system integrates seamlessly with Orbit-RS's virtual actor model, where **actors become the fundamental unit of memory locality and pinning decisions**.

**Actor Memory Ownership:**

```rust
pub trait ActorMemoryManager {
    /// Pin memory for an actor's persistent state
    fn pin_actor_state(&self, actor_ref: &AddressableReference, priority: PinPriority) -> Result<()>;
    
    /// Pin memory for an actor's working set (temporary data)
    fn pin_actor_working_set(&self, actor_ref: &AddressableReference, ttl_ms: u64) -> Result<()>;
    
    /// Unpin all memory associated with an actor (on deactivation)
    fn unpin_actor_memory(&self, actor_ref: &AddressableReference);
    
    /// Get memory statistics for an actor
    fn actor_memory_stats(&self, actor_ref: &AddressableReference) -> ActorMemoryStats;
}

#[derive(Debug, Clone)]
pub struct ActorMemoryStats {
    pub persistent_state_bytes: u64,
    pub working_set_bytes: u64,
    pub pinned_extents: Vec<ExtentRef>,
    pub numa_node: Option<u16>,
    pub last_access: Instant,
}
```

#### 7.7.2 Actor Lifecycle Integration

**OnActivate - Intelligent Warming:**

```rust
use orbit_shared::addressable::{Addressable, AddressableReference};

#[async_trait]
imppl<T: Addressable> ActorLifecycle for T {
    async fn on_activate(&mut self, actor_ref: &AddressableReference) -> Result<()> {
        let memory_manager = self.get_memory_manager();
        
        // 1. Pin persistent state based on actor type and access patterns
        match self.get_memory_profile() {
            ActorMemoryProfile::Hot => {
                // Critical path actors - pin immediately with tail latency priority
                memory_manager.pin_actor_state(actor_ref, PinPriority::TailLatencyCritical)?;
            },
            ActorMemoryProfile::Warm => {
                // Frequently accessed - pin with query critical priority
                memory_manager.pin_actor_state(actor_ref, PinPriority::QueryCritical)?;
            },
            ActorMemoryProfile::Cold => {
                // Infrequently accessed - use background priority or no pinning
                memory_manager.pin_actor_state(actor_ref, PinPriority::Background)?;
            },
        }
        
        // 2. Prefetch connected actors' state based on actor relationships
        if let Some(connected_actors) = self.get_connected_actors() {
            for connected_ref in connected_actors {
                memory_manager.prefetch_actor_state(&connected_ref)?;
            }
        }
        
        Ok(())
    }
    
    async fn on_deactivate(&mut self, actor_ref: &AddressableReference) -> Result<()> {
        // Unpin all memory when actor is deactivated
        self.get_memory_manager().unpin_actor_memory(actor_ref);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ActorMemoryProfile {
    Hot,    // <1ms response time requirements
    Warm,   // <10ms response time requirements
    Cold,   // >100ms acceptable
}
```

#### 7.7.3 Actor Type-Specific Memory Strategies

**Graph Database Actors:**

```rust
use orbit_protocols::graph_database::GraphDatabaseActor;

impl GraphDatabaseActor {
    fn get_memory_profile(&self) -> ActorMemoryProfile {
        // Graph traversal actors need hot memory for low-latency path finding
        ActorMemoryProfile::Hot
    }
    
    fn get_connected_actors(&self) -> Option<Vec<AddressableReference>> {
        // Return neighboring graph partition actors for prefetching
        Some(self.get_adjacent_partition_actors())
    }
    
    async fn on_activate(&mut self, actor_ref: &AddressableReference) -> Result<()> {
        // Pin the graph partition this actor manages
        let partition_id = self.get_partition_id();
        let extents = self.extent_index.lookup_graph_partition(partition_id);
        
        for extent in extents {
            self.pin_manager.pin_slice(
                PinKey(extent.as_pin_key()),
                &PinOpts {
                    priority: PinPriority::TailLatencyCritical,
                    use_hugepages: true,
                    numa_prefer: Some(self.get_numa_node()),
                    lifetime_class: LifetimeClass::LongLived,
                    prefetch_adjacent: 3, // Prefetch 3 neighbor partitions
                    ..Default::default()
                },
            )?;
        }
        
        Ok(())
    }
}
```

**Time Series Actors:**

```rust
use orbit_protocols::time_series::TimeSeriesActor;

impl TimeSeriesActor {
    fn get_memory_profile(&self) -> ActorMemoryProfile {
        match self.get_query_pattern() {
            QueryPattern::RealTimeAnalytics => ActorMemoryProfile::Hot,
            QueryPattern::RecentDataQueries => ActorMemoryProfile::Warm,
            QueryPattern::HistoricalAnalysis => ActorMemoryProfile::Cold,
        }
    }
    
    async fn on_activate(&mut self, actor_ref: &AddressableReference) -> Result<()> {
        // Pin recent time buckets for real-time queries
        let recent_buckets = self.get_recent_time_buckets(Duration::from_secs(3600)); // Last hour
        
        for bucket in recent_buckets {
            let extents = self.extent_index.lookup_time_bucket(bucket.bucket_id);
            for extent in extents {
                self.pin_manager.pin_slice(
                    PinKey(extent.as_pin_key()),
                    &PinOpts {
                        priority: PinPriority::QueryCritical,
                        use_hugepages: extent.len >= 64 * 1024 * 1024, // 64MB+
                        lifetime_class: LifetimeClass::Session,
                        ttl_ms: Some(3600 * 1000), // 1 hour TTL
                        ..Default::default()
                    },
                )?;
            }
        }
        
        Ok(())
    }
}
```

#### 7.7.4 Actor Communication and Memory Locality

**Message-Based Memory Hints:**

```rust
use orbit_shared::actor_communication::{Message, MessageContent};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryHint {
    WillAccessSoon {
        actor_refs: Vec<AddressableReference>,
        estimated_delay_ms: u64,
    },
    AccessComplete {
        actor_refs: Vec<AddressableReference>,
    },
    MigrateActor {
        actor_ref: AddressableReference,
        target_numa_node: u16,
        reason: MigrationReason,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MigrationReason {
    NumaLocality,     // Move closer to frequently accessed data
    LoadBalancing,    // Distribute load across nodes
    MemoryPressure,   // Move to node with more available memory
}

// Actors can send memory hints to optimize cross-actor data access
impl ActorCommunication {
    pub async fn send_memory_hint(&self, hint: MemoryHint) -> Result<()> {
        let message = Message {
            content: MessageContent::MemoryHint(hint),
            target: MessageTarget::BroadcastToMemoryManagers,
            ..Default::default()
        };
        
        self.send_message(message).await
    }
}
```

**NUMA-Aware Actor Placement:**

```rust
pub struct ActorPlacementStrategy {
    pin_manager: Arc<dyn PinManager>,
    extent_index: Arc<dyn ExtentIndex>,
    numa_topology: NumaTopology,
}

impl ActorPlacementStrategy {
    pub fn optimal_placement(&self, actor_ref: &AddressableReference) -> PlacementDecision {
        // Analyze which extents this actor will likely access
        let predicted_extents = self.predict_actor_extents(actor_ref);
        
        // Find NUMA node with most of the actor's data already pinned
        let mut numa_scores = HashMap::new();
        for extent in predicted_extents {
            if let Some(numa_node) = self.get_extent_numa_placement(&extent) {
                *numa_scores.entry(numa_node).or_insert(0u64) += extent.len as u64;
            }
        }
        
        let optimal_numa = numa_scores
            .into_iter()
            .max_by_key(|(_, score)| *score)
            .map(|(node, _)| node);
        
        PlacementDecision {
            preferred_numa_node: optimal_numa,
            co_location_actors: self.find_co_location_candidates(actor_ref),
            memory_requirements: self.estimate_memory_requirements(actor_ref),
        }
    }
}
```

#### 7.7.5 Distributed Memory Coordination

**Cross-Node Memory Management:**

```rust
pub struct DistributedMemoryCoordinator {
    local_pin_manager: Arc<dyn PinManager>,
    cluster_memory_view: Arc<ClusterMemoryView>,
    replication_manager: Arc<ReplicationManager>,
}

impl DistributedMemoryCoordinator {
    /// Coordinate pinning across multiple nodes for replicated data
    pub async fn coordinate_replicated_pinning(&self, 
        extent: &ExtentRef, 
        priority: PinPriority
    ) -> Result<()> {
        let replica_nodes = self.replication_manager.get_replica_nodes(extent)?;
        
        // Pin on primary replica with high priority
        if let Some(primary_node) = replica_nodes.primary {
            if primary_node == self.get_local_node_id() {
                self.local_pin_manager.pin_slice(
                    PinKey(extent.as_pin_key()),
                    &PinOpts {
                        priority,
                        use_hugepages: true,
                        lifetime_class: LifetimeClass::LongLived,
                        ..Default::default()
                    },
                )?;
            } else {
                self.send_remote_pin_request(primary_node, extent, priority).await?;
            }
        }
        
        // Pin on secondary replicas with lower priority
        for secondary_node in replica_nodes.secondaries {
            let secondary_priority = match priority {
                PinPriority::TailLatencyCritical => PinPriority::QueryCritical,
                PinPriority::QueryCritical => PinPriority::Background,
                PinPriority::Background => continue, // Skip pinning on secondaries
            };
            
            if secondary_node == self.get_local_node_id() {
                self.local_pin_manager.pin_slice(
                    PinKey(extent.as_pin_key()),
                    &PinOpts {
                        priority: secondary_priority,
                        lifetime_class: LifetimeClass::Task,
                        ..Default::default()
                    },
                )?;
            } else {
                self.send_remote_pin_request(secondary_node, extent, secondary_priority).await?;
            }
        }
        
        Ok(())
    }
    
    /// Handle actor migration with memory state preservation
    pub async fn migrate_actor_with_memory(&self, 
        actor_ref: &AddressableReference,
        target_node: NodeId
    ) -> Result<()> {
        // 1. Get current memory state
        let memory_stats = self.local_pin_manager.get_actor_memory_stats(actor_ref);
        
        // 2. Pre-warm target node by pinning actor's data there
        for extent in &memory_stats.pinned_extents {
            self.send_remote_pin_request(
                target_node, 
                extent, 
                PinPriority::QueryCritical
            ).await?;
        }
        
        // 3. Migrate the actor
        self.migrate_actor(actor_ref, target_node).await?;
        
        // 4. Unpin memory on source node (with delay for safety)
        tokio::time::sleep(Duration::from_secs(5)).await;
        for extent in &memory_stats.pinned_extents {
            self.local_pin_manager.unpin_slice(PinKey(extent.as_pin_key()));
        }
        
        Ok(())
    }
}
```

#### 7.7.6 Benefits of Actor-Memory Integration

**1. Natural Locality Boundaries:**

- Actors define clear ownership boundaries for memory regions
- Pin/unpin decisions align with actor lifecycle (activate/deactivate)
- Actor relationships guide prefetching strategies

**2. Workload-Aware Optimization:**

- Different actor types get different memory profiles (hot/warm/cold)
- Graph actors optimize for traversal locality
- Time series actors optimize for temporal locality
- Document actors optimize for key-based access

**3. Distributed Coordination:**

- Actor migration triggers coordinated memory management across nodes
- Replication-aware pinning reduces cross-node data access
- Message-based memory hints enable predictive optimization

**4. Resource Management:**

- Pin budgets can be allocated per actor type or priority class
- Actor deactivation automatically frees associated memory
- NUMA-aware placement reduces memory access latency

**5. Performance Guarantees:**

- Critical path actors get guaranteed memory pinning
- Tail latency SLAs can be enforced at the actor level
- Memory pressure triggers predictable eviction based on actor priorities

#### Expected Performance

| Metric | Per Node | 3-Node Cluster Total |
|--------|----------|----------------------|
| **Storage Capacity** | 800TB | 2.4PB (1PB usable) |
| **Memory Usage** | 256-512GB | 768GB-1.5TB |
| **Read Throughput** | 4 GB/s | 12 GB/s |
| **Write Throughput** | 4 GB/s | 12 GB/s |
| **IOPS** | 64K | 192K |
| **Latency** | <10μs | <15μs (cross-node) |

#### Cost Analysis (3-Node vs Traditional)

| Approach | Nodes | Monthly Cost | Savings |
|----------|-------|-------------|----------|
| **3-Node Ultra-Dense** | 3 | **$18K** | **$55K saved** |
| Traditional 50-Node | 50 | $50K | Baseline |
| Traditional 100-Node | 100 | $73K | Reference |

**Benefits of 3-Node Approach:**

- **94% fewer nodes** than traditional 50-node deployment
- **$55K/month savings** vs 50-node cluster
- **Simplified operations** with only 3 nodes to manage
- **Higher density** utilization per node
- **Reduced network complexity** with fewer inter-node connections

### 5. Bare Metal Deployment

For maximum performance on dedicated hardware:

**Hardware Specifications:**

```yaml
recommendedHardware:
  cpu: "AMD EPYC 7763 or Intel Xeon Gold 6338"
  cores: "64+ cores (128+ threads)"
  memory: "512GB+ DDR4-3200"
  storage: "Multiple NVMe Gen4 drives (Samsung PM1735)"
  network: "100Gbps Ethernet or InfiniBand"
  numa: "2-4 socket configuration"
```

**System Preparation:**

```bash

#!/bin/bash
# bare-metal-setup.sh

# Configure BIOS settings (manufacturer-specific)
# - Enable NUMA
# - Configure memory interleaving
# - Enable huge page support
# - Optimize CPU P-states

# Install optimized kernel
# Ubuntu example:
apt install linux-image-5.15.0-generic-hwe-20.04

# Configure huge pages permanently  
echo "vm.nr_hugepages=32768" >> /etc/sysctl.conf  # 64GB of 2MB pages
echo "kernel.shmmax=68719476736" >> /etc/sysctl.conf

# Configure NUMA balancing
echo "kernel.numa_balancing=1" >> /etc/sysctl.conf
echo "vm.zone_reclaim_mode=1" >> /etc/sysctl.conf

# Optimize NVMe drives
for drive in /dev/nvme*n1; do
    echo none > /sys/block/$(basename $drive)/queue/scheduler
    echo 1 > /sys/block/$(basename $drive)/queue/nomerges
done

# Configure CPU governor
echo performance > /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor

# Setup RAID 0 across multiple NVMe drives for maximum performance
mdadm --create /dev/md0 --level=0 --raid-devices=4 /dev/nvme[0-3]n1
mkfs.ext4 -F /dev/md0
mount -o noatime,nodiratime,data=writeback /dev/md0 /mnt/orbit-data
```

## Performance Analysis

### Expected Performance Improvements

| Metric | Traditional | Memory-Mapped | Improvement |
|--------|------------|---------------|-------------|
| **Nodes Required** | 100-200 | 30-50 | 3-4x reduction |
| **RAM per Node** | 64-128GB | 32-64GB | 50% reduction |
| **Read Latency** | 10-50μs | 2-10μs | 2-5x faster |
| **Write Latency** | 50-200μs | 5-20μs | 5-10x faster |
| **Memory Efficiency** | 60% | 85% | 25% better |
| **Cost per Month** | $100K | $73K | $27K savings |

### Benchmark Scenarios

#### 1. Actor Lease Operations

```rust
// Benchmark configuration
const OPERATIONS: usize = 1_000_000;
const CONCURRENCY: usize = 100;
const LEASE_SIZE: usize = 256; // bytes

// Expected results:
// - Traditional: 50K ops/sec, 100μs p99 latency
// - Memory-mapped: 200K ops/sec, 20μs p99 latency
```

#### 2. Large Dataset Scans

```rust
// Scanning 1TB dataset
const DATASET_SIZE: usize = 1_000_000_000_000; // 1TB
const SCAN_PATTERN: ScanPattern = ScanPattern::Sequential;

// Expected results:
// - Traditional: 2GB/s throughput, 50% CPU
// - Memory-mapped: 8GB/s throughput, 30% CPU (OS handles prefetch)
```

#### 3. Memory Pressure Tests

```rust
// Simulate memory pressure
const TOTAL_DATA: usize = 10_000_000_000_000; // 10TB
const AVAILABLE_RAM: usize = 64_000_000_000;   // 64GB

// Expected behavior:
// - Traditional: OOM errors, performance degradation
// - Memory-mapped: Graceful page eviction, consistent performance
```

## Security Considerations

### Memory Protection

1. **Address Space Layout Randomization (ASLR)**
   - Memory-mapped regions should support ASLR
   - Random base addresses for security

2. **Memory Encryption**
   - Support for Intel TME/SME encryption
   - Per-region encryption keys

3. **Access Control**
   - Page-level permissions (read/write/execute)
   - Integration with existing security model

### Implementation

```rust
/// Security configuration for memory-mapped regions

#[derive(Debug, Clone)]
pub struct MMapSecurityConfig {
    /// Enable address space randomization
    pub enable_aslr: bool,
    /// Memory encryption settings
    pub encryption: MemoryEncryption,
    /// Access control policies
    pub access_control: AccessControlPolicy,
}

pub enum MemoryEncryption {
    None,
    SystemDefault,
    PerRegion { key_derivation: KeyDerivation },
}
```

## Testing Strategy

### 1. Unit Tests

- Memory mapping operations
- Configuration parsing
- NUMA topology detection
- Huge page management
- I/O ring operations

### 2. Integration Tests

- Full persistence provider functionality
- Kubernetes deployment validation
- Performance regression tests
- Failure recovery scenarios

### 3. Performance Tests

- Throughput benchmarks
- Latency measurements
- Memory usage profiling
- Scalability testing

### 4. Stress Tests

- Memory pressure scenarios
- High concurrency loads
- Network partition handling
- Hardware failure simulation

### Test Infrastructure

```rust
/// Comprehensive test suite for mmap architecture

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_mmap_provider_basic_operations() {
        let config = MMapConfig::default();
        let provider = MMapPersistenceProvider::new(config).await.unwrap();
        
        // Test basic read/write operations
        let test_data = vec![1, 2, 3, 4, 5];
        provider.write_data(0, &test_data).await.unwrap();
        
        let read_data = provider.read_data(0, test_data.len()).await.unwrap();
        assert_eq!(test_data, read_data);
    }
    
    #[tokio::test]
    async fn test_huge_page_allocation() {
        let mut config = MMapConfig::default();
        config.enable_huge_pages = true;
        config.huge_page_size = HugePageSize::Size2MB;
        
        let provider = MMapPersistenceProvider::new(config).await.unwrap();
        
        // Verify huge pages are being used
        let stats = provider.get_memory_stats().await.unwrap();
        assert!(stats.huge_pages_allocated > 0);
    }
    
    #[tokio::test]
    async fn test_numa_awareness() {
        let mut config = MMapConfig::default();
        config.numa_policy = NumaPolicy::Local;
        
        let provider = MMapPersistenceProvider::new(config).await.unwrap();
        
        // Verify NUMA-local allocation
        let topology = NumaTopology::detect().unwrap();
        if topology.node_count > 1 {
            let stats = provider.get_numa_stats().await.unwrap();
            assert!(stats.local_allocations > stats.remote_allocations);
        }
    }
}
```

## Migration Path

### From Existing Backends

#### 1. Assessment Phase

- Analyze current data size and access patterns
- Estimate resource requirements for mmap deployment
- Plan migration timeline and rollback procedures

#### 2. Parallel Deployment

```rust
/// Migration configuration

#[derive(Debug, Clone)]
pub struct MigrationConfig {
    /// Source persistence backend
    pub source_backend: BackendType,
    /// Target mmap configuration
    pub target_config: MMapConfig,
    /// Migration batch size
    pub batch_size: usize,
    /// Validation settings
    pub validation: ValidationConfig,
}

/// Migration orchestrator
pub struct MigrationOrchestrator {
    source: Arc<dyn PersistenceProvider>,
    target: Arc<MMapPersistenceProvider>,
    config: MigrationConfig,
}

impl MigrationOrchestrator {
    /// Execute data migration
    pub async fn migrate(&self) -> OrbitResult<MigrationReport> {
        let mut report = MigrationReport::new();
        
        // Phase 1: Copy existing data
        self.copy_data_phase(&mut report).await?;
        
        // Phase 2: Sync incremental changes
        self.sync_changes_phase(&mut report).await?;
        
        // Phase 3: Switch traffic
        self.switch_traffic_phase(&mut report).await?;
        
        Ok(report)
    }
}
```

#### 3. Validation and Cutover

- Data integrity verification
- Performance validation
- Gradual traffic migration
- Rollback capabilities

## Open Questions

### Technical Questions

1. **Kernel Version Requirements**: What's the minimum kernel version for optimal mmap performance?
2. **Container Support**: How well do memory-mapped files work in containerized environments?
3. **Cross-Platform**: What's the performance difference between Linux, Windows, and macOS?
4. **Storage Types**: How do different storage types (NVMe, SATA SSD, cloud block storage) affect performance?

### Operational Questions

1. **Backup Strategy**: How to backup memory-mapped files efficiently?
2. **Disaster Recovery**: What's the recovery time for large mmap regions?
3. **Monitoring**: What metrics are most important for mmap-based systems?
4. **Capacity Planning**: How to predict memory and storage requirements?

### Future Considerations

1. **Hardware Trends**: How will future CPU/memory/storage developments affect this architecture?
2. **Cloud Native**: Integration with cloud-native storage solutions
3. **Edge Computing**: Applicability to edge deployment scenarios
4. **Machine Learning**: Optimizations for ML/AI workloads

## Conclusion

The memory-mapped file architecture represents a significant opportunity to improve orbit-rs's scalability and cost-effectiveness for large-scale deployments. By leveraging modern hardware capabilities and operating system optimizations, we can achieve better performance with dramatically reduced resource requirements.

The proposed implementation plan balances ambitious performance goals with practical development milestones. The extensive configuration system ensures the architecture can adapt to various deployment scenarios while maintaining operational simplicity.

Success metrics for this RFC include:

- 50-75% reduction in infrastructure costs
- 3x reduction in required nodes for petabyte-scale deployments
- Improved performance across all key metrics
- Successful production deployments within 6 months

## References

- [Linux Memory Management Documentation](https://www.kernel.org/doc/html/latest/admin-guide/mm/index.html)
- [io_uring Documentation](https://kernel.dk/io_uring.pdf)
- [NUMA Optimization Guide](https://www.kernel.org/doc/html/latest/vm/numa.html)
- [Transparent Huge Pages](https://www.kernel.org/doc/html/latest/admin-guide/mm/transhuge.html)
- [High Performance Computing with NVMe](https://nvmexpress.org/resources/)
