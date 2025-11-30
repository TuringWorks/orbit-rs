# Protocol Persistence Operations Guide

**Comprehensive guide to persistence management across all Orbit-RS protocols**

## Quick Links

- **[Performance Tuning](PROTOCOL_PERSISTENCE_TUNING.md)** - Optimize storage performance for your workload
- **[Data Migration](PROTOCOL_DATA_MIGRATION.md)** - Migrate data between versions and protocols
- **[Backup & Restore](PROTOCOL_BACKUP_RESTORE.md)** - Protect and recover your data

---

## Overview

This guide consolidates all persistence-related operations for Orbit-RS protocol implementations. All protocols use RocksDB as their underlying storage engine, providing:

- **Persistence** - Data survives server restarts
- **Consistency** - ACID-like guarantees
- **Performance** - Tunable for different workloads
- **Durability** - Write-ahead logging and checksums

### Supported Protocols

| Protocol | Storage Type | Use Case |
|----------|--------------|----------|
| **AQL/ArangoDB** | Document + Graph | Multi-model queries |
| **Cypher/Neo4j** | Property Graph | Graph traversals |
| **GraphRAG** | Knowledge Graph + Embeddings | AI/ML knowledge bases |
| **PostgreSQL** | Relational + JSONB | SQL queries |

---

## Getting Started

### 1. Initial Setup

```bash
# Create data directories
mkdir -p /var/lib/orbit-data/{aql,cypher,graphrag,postgres}

# Set permissions
chown -R orbit:orbit /var/lib/orbit-data
chmod 750 /var/lib/orbit-data

# Initialize storage
orbit-server init --data-dir /var/lib/orbit-data
```

### 2. Performance Tuning

Start with baseline configuration:

```rust
// config.yaml
storage:
  aql:
    block_cache_mb: 512
    write_buffer_mb: 128
    max_open_files: 1000

  cypher:
    block_cache_mb: 1024
    write_buffer_mb: 128
    max_open_files: 500

  graphrag:
    block_cache_mb: 2048
    write_buffer_mb: 256
    max_open_files: 1000
```

📖 **See [Performance Tuning Guide](PROTOCOL_PERSISTENCE_TUNING.md) for workload-specific optimization**

### 3. Backup Strategy

Set up automated backups:

```bash
# Full backup weekly
0 2 * * 0 /usr/local/bin/backup-full.sh

# Incremental backup daily
0 2 * * 1-6 /usr/local/bin/backup-incremental.sh
```

📖 **See [Backup & Restore Guide](PROTOCOL_BACKUP_RESTORE.md) for complete procedures**

---

## Common Operations

### Verify Data Integrity

```bash
# Check all protocols
orbit-cli verify-all

# Check specific protocol
orbit-cli aql verify --detailed
orbit-cli cypher verify --check-relationships
orbit-cli graphrag verify --check-embeddings
```

### Monitor Storage Usage

```bash
# Check disk usage
orbit-cli storage-stats

# Example output:
# Protocol    Data Size    Index Size    Total      Items
# ────────────────────────────────────────────────────────
# AQL         12.5 GB      2.3 GB       14.8 GB    1.2M docs
# Cypher      8.2 GB       1.5 GB       9.7 GB     500K nodes
# GraphRAG    45.3 GB      8.7 GB       54.0 GB    2.5M entities
# Postgres    3.1 GB       0.8 GB       3.9 GB     800K rows
# ────────────────────────────────────────────────────────
# Total       69.1 GB      13.3 GB      82.4 GB
```

### Compact Storage

```bash
# Trigger manual compaction (reduces disk usage)
orbit-cli compact --protocol aql
orbit-cli compact --protocol cypher --level 0

# Full compaction (slower, more thorough)
orbit-cli compact --protocol all --full
```

---

## Troubleshooting

### Issue: High Memory Usage

**Symptoms:**
- Server using excessive RAM
- OOM killer terminating process

**Solutions:**
```bash
# 1. Reduce block cache
orbit-server --aql-block-cache 256MB

# 2. Limit write buffers
orbit-server --max-write-buffers 4

# 3. Check for memory leaks
orbit-cli memory-profile --duration 60s
```

📖 **See [Tuning Guide - Memory Management](PROTOCOL_PERSISTENCE_TUNING.md#memory-budget)**

### Issue: Slow Queries

**Symptoms:**
- Query latency >1s
- High disk I/O

**Solutions:**
```bash
# 1. Check cache hit rate
orbit-cli stats --protocol aql | grep cache_hit_rate
# Target: >90%

# 2. Increase block cache if hit rate is low
orbit-server --aql-block-cache 2GB

# 3. Enable bloom filters
orbit-server --enable-bloom-filters

# 4. Profile slow queries
orbit-cli profile-query --protocol aql "FOR doc IN users RETURN doc"
```

### Issue: Write Stalls

**Symptoms:**
- Writes timing out
- "write stall" in logs

**Solutions:**
```bash
# 1. Check compaction status
orbit-cli compaction-stats

# 2. Increase level 0 trigger
orbit-server --level0-slowdown-trigger 20

# 3. Add more compaction threads
orbit-server --max-background-jobs 8

# 4. Temporary: disable auto-compaction during bulk load
orbit-cli disable-auto-compaction --protocol aql
# ... bulk load ...
orbit-cli enable-auto-compaction --protocol aql
orbit-cli compact --protocol aql --full
```

### Issue: Data Corruption

**Symptoms:**
- Checksum errors in logs
- Server refusing to start
- Query returning corrupt data

**Solutions:**
```bash
# 1. Verify extent of corruption
orbit-cli verify --protocol aql --detailed

# 2. Try repair (if minor corruption)
orbit-cli repair --protocol aql

# 3. If repair fails, restore from backup
systemctl stop orbit-server
./restore-protocol.sh aql /backup/aql/latest.tar.gz
systemctl start orbit-server
orbit-cli verify --protocol aql
```

📖 **See [Backup & Restore - Disaster Recovery](PROTOCOL_BACKUP_RESTORE.md#disaster-recovery)**

---

## Operational Workflows

### 1. Version Upgrade

```bash
# 1. Backup current state
./backup-all-protocols.sh

# 2. Test migration on staging
orbit-migrate validate --from 0.2.0 --to 0.3.0

# 3. Run migration
systemctl stop orbit-server
orbit-migrate run --from 0.2.0 --to 0.3.0
systemctl start orbit-server

# 4. Verify
orbit-cli verify-all
```

📖 **See [Migration Guide - Version Migration](PROTOCOL_DATA_MIGRATION.md#version-migration)**

### 2. Protocol Migration

```bash
# Example: AQL → Cypher
orbit-migrate convert \
  --from aql \
  --to cypher \
  --mapping-file aql-to-cypher.yaml \
  --validate-before \
  --verify-after
```

📖 **See [Migration Guide - Cross-Protocol](PROTOCOL_DATA_MIGRATION.md#cross-protocol-migration)**

### 3. Disaster Recovery

```bash
# Complete data loss scenario
# 1. Provision new server
# 2. Download latest backup
aws s3 cp s3://backups/latest.tar.gz /tmp/

# 3. Restore
tar -xzf /tmp/latest.tar.gz -C /var/lib/orbit-data

# 4. Verify and start
orbit-cli verify-all
systemctl start orbit-server
```

📖 **See [Backup & Restore - DR Scenarios](PROTOCOL_BACKUP_RESTORE.md#disaster-recovery)**

---

## Performance Optimization Cheatsheet

### Read-Heavy Workloads

```yaml
# Optimize for queries
storage:
  block_cache_mb: 4096      # Large cache
  compression: lz4          # Fast decompression
  bloom_filters: true       # Skip unnecessary reads
  parallelism: 16           # Max CPU usage
```

### Write-Heavy Workloads

```yaml
# Optimize for ingestion
storage:
  write_buffer_mb: 512      # Large write buffers
  max_write_buffers: 8      # More buffers
  compression: none         # Skip during writes
  level0_trigger: 8         # Delay compaction
```

### Balanced Workloads

```yaml
# Default recommended settings
storage:
  block_cache_mb: 1024
  write_buffer_mb: 256
  compression: lz4
  parallelism: 8
```

📖 **See [Tuning Guide - Workload-Specific](PROTOCOL_PERSISTENCE_TUNING.md#workload-specific-tuning)**

---

## Monitoring Dashboard

Key metrics to track:

### Storage Metrics
- **Disk usage** - Track growth rate
- **Compaction lag** - Should be near zero
- **Write amplification** - Target <10x
- **Cache hit rate** - Target >90%

### Performance Metrics
- **Query latency** - p50, p95, p99
- **Write throughput** - ops/sec
- **Read throughput** - ops/sec
- **CPU usage** - per protocol

### Health Metrics
- **Backup age** - Last successful backup
- **Replication lag** - If using replicas
- **Error rate** - Protocol errors
- **Availability** - Uptime %

```bash
# Export metrics for Prometheus
orbit-server --enable-metrics --metrics-port 9090

# View in Grafana
# Import dashboard: orbit/monitoring/grafana/storage-dashboard.json
```

---

## Best Practices

### ✅ DO

1. **Backup Regularly**
   - Automate daily backups
   - Test restores monthly
   - Store backups off-site

2. **Monitor Continuously**
   - Set up alerts for issues
   - Track performance trends
   - Review logs regularly

3. **Tune for Workload**
   - Profile actual usage
   - Adjust based on metrics
   - Re-tune after changes

4. **Plan for Growth**
   - Monitor disk usage trends
   - Provision ahead of needs
   - Archive old data

5. **Document Everything**
   - Configuration changes
   - Performance baselines
   - Incident postmortems

### ❌ DON'T

1. **Skip Backups**
   - Data loss is permanent
   - Downtime is expensive
   - Compliance violations

2. **Over-Tune Prematurely**
   - Start with defaults
   - Measure before changing
   - One change at a time

3. **Ignore Warnings**
   - Write stalls mean problems
   - Checksum errors = corruption
   - Memory warnings = instability

4. **Mix Workloads Carelessly**
   - OLTP and OLAP compete
   - Batch jobs impact queries
   - Isolate when possible

5. **Forget to Verify**
   - Test backups work
   - Validate after migration
   - Check integrity regularly

---

## Quick Reference

### Essential Commands

```bash
# Status
orbit-cli status                    # Server status
orbit-cli storage-stats             # Storage usage
orbit-cli verify-all                # Data integrity

# Operations
orbit-cli backup --protocol all     # Backup all data
orbit-cli restore --backup /path    # Restore from backup
orbit-cli compact --protocol all    # Compact storage

# Troubleshooting
orbit-cli logs --follow             # View logs
orbit-cli health-check              # System health
orbit-cli debug-dump                # Debug information
```

### Configuration Files

```
/etc/orbit-server/
├── config.yaml              # Main configuration
├── tuning.yaml              # Storage tuning
└── backup.yaml              # Backup settings

/var/lib/orbit-data/
├── aql/rocksdb/             # AQL data
├── cypher/rocksdb/          # Cypher data
├── graphrag/rocksdb/        # GraphRAG data
└── postgres/rocksdb/        # Postgres data

/var/log/orbit-server/
├── server.log               # Server logs
├── storage.log              # Storage operations
└── backup.log               # Backup operations
```

### Support Resources

- **Documentation**: https://docs.orbit-rs.dev
- **GitHub Issues**: https://github.com/orbit-rs/orbit-rs/issues
- **Community Forum**: https://community.orbit-rs.dev
- **Email Support**: support@orbit-rs.dev

---

## Document Index

This is the main index for protocol persistence documentation:

1. **[PROTOCOL_PERSISTENCE_TUNING.md](PROTOCOL_PERSISTENCE_TUNING.md)**
   - RocksDB tuning for each protocol
   - Performance optimization techniques
   - Workload-specific configurations
   - Monitoring and profiling

2. **[PROTOCOL_DATA_MIGRATION.md](PROTOCOL_DATA_MIGRATION.md)**
   - Version upgrade procedures
   - Cross-protocol data conversion
   - Import from external systems
   - Schema evolution strategies

3. **[PROTOCOL_BACKUP_RESTORE.md](PROTOCOL_BACKUP_RESTORE.md)**
   - Backup strategies and schedules
   - Physical and logical backups
   - Disaster recovery procedures
   - Cloud backup integration

4. **[STORAGE_ARCHITECTURE_CURRENT.md](STORAGE_ARCHITECTURE_CURRENT.md)**
   - Current storage implementation
   - Architecture diagrams
   - Design decisions
   - Technical specifications

5. **[PROTOCOL_100_PERCENT_COMPLETE.md](PROTOCOL_100_PERCENT_COMPLETE.md)**
   - Protocol implementation status
   - Feature completeness matrix
   - Known limitations
   - Roadmap

---

**Last Updated**: 2025-03-23
**Version**: 1.0
**Maintainer**: Orbit-RS Team

For questions or feedback, please open an issue on GitHub or contact support@orbit-rs.dev.
