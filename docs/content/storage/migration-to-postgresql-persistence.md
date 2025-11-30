# Migration Guide: PostgreSQL Persistence in Orbit-RS

This guide helps you migrate from in-memory PostgreSQL to persistent RocksDB-backed PostgreSQL storage in Orbit-RS, and provides best practices for production deployments.

## 📋 Table of Contents

1. [Migration Overview](#migration-overview)
2. [Pre-Migration Checklist](#pre-migration-checklist)
3. [Step-by-Step Migration](#step-by-step-migration)
4. [Data Migration Strategies](#data-migration-strategies)
5. [Production Best Practices](#production-best-practices)
6. [Performance Tuning](#performance-tuning)
7. [Monitoring and Observability](#monitoring-and-observability)
8. [Backup and Recovery](#backup-and-recovery)
9. [Troubleshooting](#troubleshooting)

## Migration Overview

### What Changes with PostgreSQL Persistence

#### Before (In-Memory)

```text
✅ Fast read/write operations
❌ Data lost on server restart
❌ No durability guarantees
❌ Limited by available RAM
❌ No crash consistency
```

#### After (RocksDB Persistent)

```text
✅ Fast read/write operations (LSM-tree optimized)
✅ Data survives server restarts
✅ Full ACID durability guarantees  
✅ Scalable beyond RAM limits
✅ Crash consistency with WAL
✅ Enterprise-grade reliability
```

### Compatibility

- **SQL Syntax**: 100% compatible - no SQL changes required
- **Client Connections**: Same connection parameters (host, port, user)
- **Data Types**: All existing types supported with enhanced performance
- **Transactions**: Enhanced ACID guarantees with persistent storage

## Pre-Migration Checklist

### 1. System Requirements

```bash
# Verify sufficient disk space (recommended: 3x your data size)
df -h

# Check available memory for RocksDB caches (recommended: 4GB+)
free -h

# Ensure proper permissions for data directory
mkdir -p ./orbit_integrated_data
chmod 755 ./orbit_integrated_data
```

### 2. Backup Existing Data (if applicable)

```bash
# If migrating from existing system, export current state
# Using PostgreSQL client to export:
pg_dump -h localhost -p 15432 -U orbit -d actors > pre_migration_backup.sql

# Or manually export table structures and data:
psql -h localhost -p 15432 -U orbit -d actors -c "\dt"  # List tables
psql -h localhost -p 15432 -U orbit -d actors -c "SELECT * FROM important_table;" > table_backup.csv
```

### 3. Configuration Review

```toml
# Review your orbit-server configuration
# File: config/orbit-server.toml

[persistence]
provider = "rocksdb"                    # ✅ Enable RocksDB
data_dir = "/path/to/persistent/data"  # ✅ Set data directory

[persistence.rocksdb]
enable_wal = true                      # ✅ Enable Write-Ahead Log
write_buffer_size = 134217728          # 128MB (adjust based on workload)
block_cache_size = 268435456           # 256MB (adjust based on available RAM)
max_background_jobs = 8                # CPU cores (recommended)
enable_statistics = true               # ✅ Enable for monitoring
```

## Step-by-Step Migration

### Phase 1: Enable Persistence (Zero Downtime)

#### Step 1.1: Update Server Configuration

```bash
# If using integrated-server example, no config changes needed
# Persistence is automatically enabled with RocksDB

# For custom configurations, ensure RocksDB is configured:
# config/orbit-server.toml should have [persistence] section
```

#### Step 1.2: Start New Server with Persistence

```bash
# Start the server with persistence enabled
cargo run --package orbit-server --example integrated-server

# Verify persistence is active in logs:
# INFO PostgreSQL using persistent RocksDB storage at: ./orbit_integrated_data/postgresql
```

#### Step 1.3: Test Persistence

```sql
-- Connect and test persistence
psql -h localhost -p 15432 -U orbit -d actors

-- Create a test table
CREATE TABLE migration_test (
    id INTEGER PRIMARY KEY,
    test_data TEXT,
    created_at TIMESTAMP DEFAULT NOW()
);

-- Insert test data
INSERT INTO migration_test (id, test_data) VALUES (1, 'Migration test successful');

-- Verify data exists
SELECT * FROM migration_test;
```

### Phase 2: Data Migration (if needed)

#### Step 2.1: Schema Recreation

```sql
-- Recreate all your existing tables
-- Tables created with persistence enabled will automatically persist

-- Example migration script:
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    username TEXT NOT NULL,
    email TEXT,
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE posts (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    content TEXT,
    published_at TIMESTAMP DEFAULT NOW()
);

-- Add more table definitions as needed
```

#### Step 2.2: Data Import

```bash
# Method 1: Import from SQL dump
psql -h localhost -p 15432 -U orbit -d actors -f pre_migration_backup.sql

# Method 2: CSV import (for individual tables)
psql -h localhost -p 15432 -U orbit -d actors \
  -c "\COPY users(id,username,email,created_at) FROM 'users_backup.csv' CSV HEADER;"

# Method 3: Manual INSERT statements
psql -h localhost -p 15432 -U orbit -d actors \
  -c "INSERT INTO users (id, username, email) VALUES (1, 'alice', 'alice@example.com');"
```

### Phase 3: Validation and Cutover

#### Step 3.1: Data Validation

```sql
-- Verify row counts match expectations
SELECT 'users' as table_name, COUNT(*) as row_count FROM users
UNION ALL
SELECT 'posts', COUNT(*) FROM posts;

-- Verify data integrity with sample queries
SELECT * FROM users WHERE id = 1;
SELECT * FROM posts WHERE user_id = 1;

-- Test complex queries
SELECT u.username, COUNT(p.id) as post_count 
FROM users u LEFT JOIN posts p ON u.id = p.user_id 
GROUP BY u.username;
```

#### Step 3.2: Persistence Validation

```bash
# Test 1: Stop and restart server
# Stop current server (Ctrl+C)
# Restart server
cargo run --package orbit-server --example integrated-server

# Test 2: Verify data persisted
psql -h localhost -p 15432 -U orbit -d actors
```

```sql
-- All data should still be present
SELECT * FROM migration_test;
SELECT COUNT(*) FROM users;
SELECT COUNT(*) FROM posts;
```

#### Step 3.3: Performance Validation

```sql
-- Test query performance (should be comparable or better)
\timing
SELECT * FROM users WHERE id = 1;
SELECT * FROM users WHERE username = 'alice';
SELECT COUNT(*) FROM posts WHERE user_id = 1;
```

### Phase 4: Production Cutover

#### Step 4.1: Final Backup

```bash
# Create final backup before cutover
pg_dump -h localhost -p 15432 -U orbit -d actors > final_migration_backup.sql

# Backup RocksDB data directory
tar -czf orbit-rocksdb-backup-$(date +%Y%m%d-%H%M).tar.gz ./orbit_integrated_data/
```

#### Step 4.2: Update Applications

```bash
# Update application connection strings (if needed)
# No changes required if using same host:port

# Update monitoring and alerting
# Update backup scripts to include RocksDB data directory
# Update recovery procedures
```

## Data Migration Strategies

### Strategy 1: Fresh Start (Recommended for New Deployments)

```sql
-- Start with persistent server from day 1
-- Create all tables with persistence enabled
CREATE TABLE production_table (
    id INTEGER PRIMARY KEY,
    data TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT NOW()
);

-- All subsequent operations are automatically persistent
```

### Strategy 2: Export/Import Migration

```bash
# 1. Export from existing system
pg_dump -h old_server -p 5432 database_name > migration.sql

# 2. Clean up SQL if needed (remove unsupported features)
# Edit migration.sql to ensure compatibility

# 3. Import to Orbit-RS with persistence
psql -h localhost -p 15432 -U orbit -d actors -f migration.sql
```

### Strategy 3: Live Migration (Advanced)

```python
#!/usr/bin/env python3
"""
Live migration script for large datasets
Migrates data in chunks while system remains online
"""

import psycopg2
import time

# Connection configurations
old_db = psycopg2.connect(
    host="old_server",
    port=5432,
    user="postgres",
    database="production"
)

new_db = psycopg2.connect(
    host="localhost", 
    port=15432,
    user="orbit",
    database="actors"
)

def migrate_table_in_chunks(table_name, chunk_size=10000):
    """Migrate table data in chunks"""
    old_cursor = old_db.cursor()
    new_cursor = new_db.cursor()
    
    # Get total rows
    old_cursor.execute(f"SELECT COUNT(*) FROM {table_name}")
    total_rows = old_cursor.fetchone()[0]
    
    print(f"Migrating {total_rows} rows from {table_name}")
    
    # Migrate in chunks
    for offset in range(0, total_rows, chunk_size):
        # Read chunk from old database
        old_cursor.execute(f"SELECT * FROM {table_name} LIMIT {chunk_size} OFFSET {offset}")
        rows = old_cursor.fetchall()
        
        # Insert into new database
        # (This example assumes compatible schemas)
        for row in rows:
            placeholders = ','.join(['%s'] * len(row))
            new_cursor.execute(f"INSERT INTO {table_name} VALUES ({placeholders})", row)
        
        new_db.commit()
        print(f"Migrated {min(offset + chunk_size, total_rows)}/{total_rows} rows")
        time.sleep(0.1)  # Rate limiting

# Migrate all tables
tables_to_migrate = ['users', 'posts', 'comments']
for table in tables_to_migrate:
    migrate_table_in_chunks(table)

print("Migration complete!")
```

## Production Best Practices

### 1. Configuration Optimization

```toml
# Production-tuned RocksDB configuration
[persistence.rocksdb]
enable_wal = true                      # REQUIRED for durability
write_buffer_size = 268435456          # 256MB (increase for write-heavy workloads)
max_write_buffer_number = 4            # Multiple memtables for write throughput
target_file_size_base = 67108864       # 64MB SST files
max_background_jobs = 16               # Match CPU cores
block_cache_size = 2147483648          # 2GB (adjust based on available RAM)
enable_statistics = true               # Enable for monitoring
compression_type = "lz4"               # Fast compression for hot data
```

### 2. Directory Structure

```bash
# Recommended production directory structure
/var/lib/orbit/
├── data/                          # Main data directory
│   ├── postgresql/                # PostgreSQL persistent storage
│   │   ├── pg_tables/            # Table schemas
│   │   ├── pg_data/              # Row data
│   │   └── pg_metadata/          # Metadata
│   └── cluster/                   # Cluster metadata
├── config/
│   └── orbit-server.toml         # Configuration
├── logs/                          # Application logs
└── backups/                       # Backup storage
    ├── daily/
    ├── weekly/
    └── monthly/
```

### 3. Security Considerations

```toml
# Production security configuration
[security]
enable_tls = true
cert_path = "/etc/orbit/tls/server.crt"
key_path = "/etc/orbit/tls/server.key"

[protocols]
postgres_enabled = true
postgres_bind_address = "0.0.0.0"     # Or specific interface
postgres_port = 5432                  # Standard PostgreSQL port

# Network security
[firewall]
# Allow only necessary ports
# PostgreSQL: 5432
# gRPC: 50051  
# Redis: 6379 (if needed)
```

### 4. Resource Management

```bash
# System limits for production
echo "orbit soft nofile 65536" >> /etc/security/limits.conf
echo "orbit hard nofile 65536" >> /etc/security/limits.conf

# Kernel parameters
echo "vm.swappiness = 1" >> /etc/sysctl.conf
echo "net.core.somaxconn = 65535" >> /etc/sysctl.conf

# Apply changes
sysctl -p
```

## Performance Tuning

### 1. Write Performance

```toml
# Optimize for write-heavy workloads
[persistence.rocksdb]
write_buffer_size = 536870912          # 512MB
max_write_buffer_number = 6            # More memtables
min_write_buffer_number_to_merge = 2   # Reduce write stalls
level0_file_num_compaction_trigger = 8 # Delay compaction
```

### 2. Read Performance

```toml
# Optimize for read-heavy workloads
[persistence.rocksdb]
block_cache_size = 4294967296          # 4GB cache
cache_index_and_filter_blocks = true   # Cache metadata
pin_l0_filter_and_index_blocks = true  # Pin hot blocks
bloom_filter_bits_per_key = 10         # Bloom filters for fast lookups
```

### 3. Mixed Workloads

```toml
# Balanced configuration
[persistence.rocksdb]
write_buffer_size = 268435456          # 256MB
block_cache_size = 2147483648          # 2GB
max_background_jobs = 8                # CPU cores
enable_pipelined_write = true          # Parallel writes
allow_concurrent_memtable_write = true # Concurrent memtable access
```

### 4. Query Optimization

```sql
-- Use primary key lookups when possible
SELECT * FROM users WHERE id = 123;                    -- ✅ Fast O(log n)

-- Avoid full table scans on large tables
SELECT * FROM users WHERE email LIKE '%@gmail.com';    -- ❌ Slow O(n)

-- Use LIMIT for large result sets
SELECT * FROM posts ORDER BY created_at DESC LIMIT 50; -- ✅ Bounded

-- Consider creating optimized tables for common queries
CREATE TABLE user_email_index AS 
SELECT id, username, email FROM users;                 -- ✅ Smaller, focused table
```

## Monitoring and Observability

### 1. Built-in Metrics

```bash
# Enable detailed logging
RUST_LOG=orbit_protocols::postgres_wire::persistent_storage=info \
cargo run --package orbit-server --example integrated-server

# Monitor key log messages:
# - "PostgreSQL using persistent RocksDB storage" (startup)
# - "Created table: TABLE_NAME" (DDL operations)
# - "Inserted row ... into table ..." (DML operations)
# - RocksDB compaction messages (background maintenance)
```

### 2. Performance Monitoring

```python
#!/usr/bin/env python3
"""
PostgreSQL performance monitor for Orbit-RS
"""
import psycopg2
import time
import json

def monitor_performance():
    conn = psycopg2.connect(
        host="localhost",
        port=15432,
        user="orbit", 
        database="actors"
    )
    
    while True:
        cursor = conn.cursor()
        
        # Test query performance
        start_time = time.time()
        cursor.execute("SELECT COUNT(*) FROM users;")
        query_time = time.time() - start_time
        
        row_count = cursor.fetchone()[0]
        
        metrics = {
            "timestamp": time.time(),
            "query_time_ms": query_time * 1000,
            "row_count": row_count,
            "queries_per_second": 1 / query_time if query_time > 0 else 0
        }
        
        print(json.dumps(metrics))
        time.sleep(30)  # Monitor every 30 seconds

if __name__ == "__main__":
    monitor_performance()
```

### 3. Health Checks

```bash
#!/bin/bash
# PostgreSQL health check script

HEALTH_CHECK_TIMEOUT=5

# Test connection
if timeout $HEALTH_CHECK_TIMEOUT psql -h localhost -p 15432 -U orbit -d actors -c "SELECT 1;" > /dev/null 2>&1; then
    echo "✅ PostgreSQL connection: OK"
else
    echo "❌ PostgreSQL connection: FAILED"
    exit 1
fi

# Test persistence (check if RocksDB files exist)
if [ -d "./orbit_integrated_data/postgresql" ] && [ "$(ls -A ./orbit_integrated_data/postgresql)" ]; then
    echo "✅ RocksDB persistence: OK"
else
    echo "❌ RocksDB persistence: FAILED"
    exit 1
fi

# Test write/read operations
psql -h localhost -p 15432 -U orbit -d actors -c "
CREATE TABLE IF NOT EXISTS health_check (
    id INTEGER PRIMARY KEY,
    check_time TIMESTAMP DEFAULT NOW()
);
INSERT INTO health_check (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET check_time = NOW();
SELECT 'Health check successful' FROM health_check WHERE id = 1;
" > /dev/null 2>&1

if [ $? -eq 0 ]; then
    echo "✅ Write/Read operations: OK"
else
    echo "❌ Write/Read operations: FAILED"
    exit 1
fi

echo "🎉 All health checks passed!"
```

## Backup and Recovery

### 1. Automated Backup Strategy

```bash
#!/bin/bash
# Production backup script

BACKUP_DIR="/var/lib/orbit/backups"
DATA_DIR="./orbit_integrated_data"
RETENTION_DAYS=30

# Create backup directory with timestamp
BACKUP_TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_PATH="$BACKUP_DIR/backup_$BACKUP_TIMESTAMP"

mkdir -p "$BACKUP_PATH"

echo "🔄 Starting Orbit-RS backup..."

# 1. RocksDB data backup (hot backup)
echo "📁 Backing up RocksDB data..."
cp -r "$DATA_DIR" "$BACKUP_PATH/rocksdb_data"

# 2. SQL schema and data export
echo "📊 Exporting SQL data..."
pg_dump -h localhost -p 15432 -U orbit -d actors > "$BACKUP_PATH/schema_and_data.sql"

# 3. Configuration backup
echo "⚙️ Backing up configuration..."
cp -r config/ "$BACKUP_PATH/config" 2>/dev/null || echo "No config directory found"

# 4. Create backup manifest
cat > "$BACKUP_PATH/backup_manifest.json" << EOF
{
    "timestamp": "$BACKUP_TIMESTAMP",
    "backup_type": "full",
    "components": {
        "rocksdb_data": "rocksdb_data/",
        "sql_export": "schema_and_data.sql",
        "configuration": "config/"
    },
    "orbit_version": "$(cargo --version)",
    "hostname": "$(hostname)"
}
EOF

# 5. Compress backup
echo "🗜️ Compressing backup..."
tar -czf "$BACKUP_PATH.tar.gz" -C "$BACKUP_DIR" "backup_$BACKUP_TIMESTAMP"
rm -rf "$BACKUP_PATH"

# 6. Cleanup old backups
echo "🧹 Cleaning up old backups..."
find "$BACKUP_DIR" -name "backup_*.tar.gz" -mtime +$RETENTION_DAYS -delete

echo "✅ Backup completed: $BACKUP_PATH.tar.gz"

# 7. Verify backup integrity
if tar -tzf "$BACKUP_PATH.tar.gz" > /dev/null 2>&1; then
    echo "✅ Backup integrity verified"
else
    echo "❌ Backup integrity check failed!"
    exit 1
fi
```

### 2. Recovery Procedures

```bash
#!/bin/bash
# Recovery script

BACKUP_FILE="$1"
RECOVERY_DIR="./orbit_recovery"

if [ -z "$BACKUP_FILE" ]; then
    echo "Usage: $0 <backup_file.tar.gz>"
    exit 1
fi

echo "🔄 Starting recovery from $BACKUP_FILE..."

# 1. Stop Orbit server (if running)
echo "⏹️ Stopping Orbit server..."
pkill -f "integrated-server" || echo "Server not running"

# 2. Extract backup
echo "📦 Extracting backup..."
mkdir -p "$RECOVERY_DIR"
tar -xzf "$BACKUP_FILE" -C "$RECOVERY_DIR"

# 3. Restore RocksDB data
echo "💾 Restoring RocksDB data..."
rm -rf ./orbit_integrated_data
cp -r "$RECOVERY_DIR"/backup_*/rocksdb_data ./orbit_integrated_data

# 4. Restore configuration (if needed)
echo "⚙️ Restoring configuration..."
cp -r "$RECOVERY_DIR"/backup_*/config/* ./config/ 2>/dev/null || echo "No config to restore"

# 5. Start server
echo "🚀 Starting recovered server..."
cargo run --package orbit-server --example integrated-server &
SERVER_PID=$!

# 6. Wait for server startup
echo "⏳ Waiting for server startup..."
sleep 10

# 7. Verify recovery
echo "🔍 Verifying recovery..."
if psql -h localhost -p 15432 -U orbit -d actors -c "SELECT COUNT(*) FROM pg_tables;" > /dev/null 2>&1; then
    echo "✅ Recovery successful! Server is responding."
    echo "📊 Verifying data integrity..."
    psql -h localhost -p 15432 -U orbit -d actors -c "\dt"
else
    echo "❌ Recovery failed! Server not responding."
    kill $SERVER_PID 2>/dev/null
    exit 1
fi

# Cleanup
rm -rf "$RECOVERY_DIR"
echo "🎉 Recovery completed successfully!"
```

## Troubleshooting

### Common Issues and Solutions

#### 1. Data Directory Permissions

**Problem**: Server fails to start with permission errors

```bash
# Solution: Fix directory permissions
sudo chown -R $USER:$USER ./orbit_integrated_data
chmod -R 755 ./orbit_integrated_data
```

#### 2. Port Conflicts

**Problem**: PostgreSQL port 15432 is already in use

```bash
# Check what's using the port
lsof -i :15432
netstat -tulpn | grep :15432

# Solution 1: Stop conflicting service
sudo systemctl stop conflicting-service

# Solution 2: Change port in configuration
# Edit config to use different port, e.g., 15433
```

#### 3. RocksDB Corruption

**Problem**: RocksDB files corrupted after crash

```bash
# Check RocksDB integrity
ls -la ./orbit_integrated_data/postgresql/

# Solution: Restore from backup
# Use recovery script with latest backup
./recovery_script.sh backup_20241015_120000.tar.gz
```

#### 4. Memory Issues

**Problem**: Server using too much memory

```toml
# Solution: Reduce RocksDB cache sizes
[persistence.rocksdb]
write_buffer_size = 67108864    # 64MB (reduced from 256MB)
block_cache_size = 536870912   # 512MB (reduced from 2GB)
max_write_buffer_number = 2    # Reduce concurrent memtables
```

#### 5. Slow Query Performance

**Problem**: Queries are slower than expected

```sql
-- Check table sizes
SELECT 
    schemaname,
    tablename,
    pg_total_relation_size(schemaname||'.'||tablename) as size_bytes
FROM pg_tables
ORDER BY size_bytes DESC;

-- Solutions:
-- 1. Use primary key lookups instead of full table scans
SELECT * FROM users WHERE id = 123;  -- Instead of WHERE name = 'alice'

-- 2. Limit large result sets
SELECT * FROM large_table LIMIT 1000;

-- 3. Consider table reorganization for frequently accessed data
```

#### 6. Backup/Recovery Issues

**Problem**: Backup restoration fails

```bash
# Verify backup file integrity
tar -tzf backup_file.tar.gz

# Check backup contents
tar -tzf backup_file.tar.gz | head -20

# Manual extraction for inspection
mkdir -p /tmp/backup_inspect
tar -xzf backup_file.tar.gz -C /tmp/backup_inspect
ls -la /tmp/backup_inspect
```

### Getting Help

1. **Enable Debug Logging**:

   ```bash
   RUST_LOG=debug cargo run --package orbit-server --example integrated-server
   ```

2. **Check System Resources**:

   ```bash
   # Memory usage
   free -h
   
   # Disk space  
   df -h
   
   # Process information
   ps aux | grep orbit
   ```

3. **Community Support**:
   - [GitHub Issues](https://github.com/TuringWorks/orbit-rs/issues) - Bug reports and feature requests
   - [GitHub Discussions](https://github.com/TuringWorks/orbit-rs/discussions) - Questions and community help
   - Include logs, configuration, and system details when asking for help

---

**🎉 You're now ready for production!** This migration guide ensures a smooth transition to persistent PostgreSQL storage with enterprise-grade reliability and performance.
