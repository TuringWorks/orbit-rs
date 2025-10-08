# 3-Node Petabyte Configuration for Orbit-RS

This configuration handles **1PB of data** with only **3 dedicated Kubernetes nodes** using memory-mapped file architecture.

## Overview

- **Challenge**: 1PB data across only 3 nodes = ~334TB per node
- **Solution**: Ultra-high density configuration with memory-mapped files
- **Approach**: Maximize storage density while maintaining performance
- **Replication**: 2x replication across 3 nodes for reliability

## Resource Requirements Per Node

### Hardware Specifications

| Component | Requirement | Reasoning |
|-----------|-------------|-----------|
| **CPU** | 32-64 cores | Handle high I/O concurrency |
| **Memory** | 256-512 GB RAM | Large page cache for hot data |
| **Storage** | 500-800 TB NVMe | Raw storage per node |
| **Network** | 100 Gbps | Inter-node replication |
| **Instance Type** | AWS i4i.24xlarge or bare metal | Maximum density |

### Storage Calculation

```
Raw Data: 1 PB
Replication: 2x = 2 PB total
Safety Buffer: 20% = 2.4 PB required
Per Node: 2.4 PB ÷ 3 = 800 TB per node
```

### Memory Calculation

```
Data per Node: ~334 TB
Working Set (hot data): ~10% = 33 TB
Page Cache Target: 64-128 GB per node
Application Memory: 64-128 GB per node
Total Memory: 256-512 GB per node
```

## Deployment Files

1. `01-storage-ultra-dense.yaml` - Ultra-high density storage
2. `02-configmap-3node.yaml` - 3-node specific configuration
3. `03-statefulset-3node.yaml` - High-density StatefulSet
4. `04-services-3node.yaml` - Load balancing services
5. `05-monitoring-3node.yaml` - Monitoring for 3-node cluster