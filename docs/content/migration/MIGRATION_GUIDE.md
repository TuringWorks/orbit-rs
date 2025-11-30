---
layout: default
title: "Migration Guides"
subtitle: "Migrate to Orbit-RS from PostgreSQL, Redis, and other databases"
category: "migration"
permalink: /migration/
---

# Migration Guides

Migrate your existing databases to Orbit-RS with minimal downtime and maximum compatibility.

## Table of Contents

- [Overview](#overview)
- [PostgreSQL Migration](#postgresql-migration)
- [Redis Migration](#redis-migration)
- [Multi-Database Consolidation](#multi-database-consolidation)
- [Data Verification](#data-verification)
- [Rollback Procedures](#rollback-procedures)

---

## Overview

Orbit-RS supports native wire protocols for PostgreSQL, Redis, MySQL, and CQL, making migration straightforward. Your existing applications can connect to Orbit-RS using their current client libraries with minimal configuration changes.

### Migration Benefits

| Feature | Traditional Setup | Orbit-RS |
|---------|------------------|----------|
| **Infrastructure** | Multiple database servers | Single unified server |
| **Protocols** | Separate clients per database | Native multi-protocol support |
| **Storage** | Isolated per database | Unified tiered storage |
| **Operations** | Multiple backup/restore procedures | Single operational model |
| **Scaling** | Scale each database separately | Unified cluster scaling |

### Prerequisites

- Orbit-RS server installed and running
- Access to source database(s)
- Sufficient storage for data migration
- Planned maintenance window (for live migrations)

---

## PostgreSQL Migration

Migrate from PostgreSQL to Orbit-RS while maintaining full SQL compatibility.

### Step 1: Verify Compatibility

Orbit-RS supports PostgreSQL wire protocol with these features:

**Fully Supported:**
- DDL: CREATE/ALTER/DROP TABLE, INDEX, VIEW
- DML: SELECT, INSERT, UPDATE, DELETE
- JOINs: INNER, LEFT, RIGHT, FULL OUTER, CROSS
- Aggregations: COUNT, SUM, AVG, MIN, MAX, GROUP BY, HAVING
- Window functions: ROW_NUMBER, RANK, DENSE_RANK, LAG, LEAD
- Transactions: BEGIN, COMMIT, ROLLBACK, SAVEPOINT
- Data types: INTEGER, BIGINT, TEXT, VARCHAR, BOOLEAN, TIMESTAMP, JSONB, VECTOR
- Extensions: pgvector compatibility for vector operations

**Check Your Schema:**

```bash
# Export your PostgreSQL schema
pg_dump -h your-postgres-host -U postgres -s your_database > schema.sql

# Review for unsupported features
grep -E "(CREATE EXTENSION|CREATE FUNCTION|CREATE TRIGGER)" schema.sql
```

### Step 2: Schema Migration

**Option A: Direct Schema Import**

```bash
# Connect to Orbit-RS PostgreSQL port
psql -h localhost -p 5432 -U orbit -d actors

# Import schema (DDL only)
\i schema.sql
```

**Option B: Manual Schema Creation**

```sql
-- Connect to Orbit-RS
psql -h localhost -p 5432 -U orbit -d actors

-- Create tables
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    email VARCHAR(255) NOT NULL UNIQUE,
    name TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE orders (
    id SERIAL PRIMARY KEY,
    user_id INTEGER REFERENCES users(id),
    total DECIMAL(10,2),
    status VARCHAR(50),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_orders_user_id ON orders(user_id);
CREATE INDEX idx_orders_status ON orders(status);
```

### Step 3: Data Migration

**Option A: pg_dump/psql (Recommended for < 100GB)**

```bash
# Export data from PostgreSQL
pg_dump -h source-postgres -U postgres -a --disable-triggers your_database > data.sql

# Import to Orbit-RS
psql -h localhost -p 5432 -U orbit -d actors < data.sql
```

**Option B: COPY Command (Large Tables)**

```bash
# Export from PostgreSQL
psql -h source-postgres -U postgres -d your_database \
  -c "\COPY users TO '/tmp/users.csv' WITH CSV HEADER"

# Import to Orbit-RS
psql -h localhost -p 5432 -U orbit -d actors \
  -c "\COPY users FROM '/tmp/users.csv' WITH CSV HEADER"
```

**Option C: Streaming Migration (Minimal Downtime)**

```bash
# Use logical replication for live migration
# 1. Set up publication on source PostgreSQL
psql -h source-postgres -U postgres -d your_database \
  -c "CREATE PUBLICATION orbit_migration FOR ALL TABLES;"

# 2. Create subscription on Orbit-RS
psql -h localhost -p 5432 -U orbit -d actors \
  -c "CREATE SUBSCRIPTION orbit_sub
      CONNECTION 'host=source-postgres dbname=your_database user=postgres'
      PUBLICATION orbit_migration;"

# 3. Monitor replication lag
psql -h localhost -p 5432 -U orbit -d actors \
  -c "SELECT * FROM pg_stat_subscription;"
```

### Step 4: Application Cutover

**Update Connection String:**

```python
# Before (PostgreSQL)
connection_string = "postgresql://user:pass@postgres-host:5432/mydb"

# After (Orbit-RS)
connection_string = "postgresql://orbit:orbit@orbit-host:5432/actors"
```

```java
// Before (PostgreSQL)
String url = "jdbc:postgresql://postgres-host:5432/mydb";

// After (Orbit-RS)
String url = "jdbc:postgresql://orbit-host:5432/actors";
```

### Step 5: Verification

```sql
-- Compare row counts
SELECT 'users' as table_name, COUNT(*) as count FROM users
UNION ALL
SELECT 'orders', COUNT(*) FROM orders;

-- Verify sample data
SELECT * FROM users LIMIT 10;
SELECT * FROM orders WHERE user_id = 1;

-- Test queries
SELECT u.name, COUNT(o.id) as order_count, SUM(o.total) as total_spent
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
GROUP BY u.id, u.name
ORDER BY total_spent DESC
LIMIT 10;
```

---

## Redis Migration

Migrate from Redis to Orbit-RS while maintaining RESP protocol compatibility.

### Step 1: Verify Command Compatibility

Orbit-RS supports 124+ Redis commands including:

**Strings:** GET, SET, MGET, MSET, INCR, DECR, APPEND, STRLEN
**Hashes:** HGET, HSET, HMGET, HMSET, HDEL, HGETALL, HKEYS, HVALS
**Lists:** LPUSH, RPUSH, LPOP, RPOP, LRANGE, LLEN, LINDEX
**Sets:** SADD, SREM, SMEMBERS, SISMEMBER, SUNION, SINTER
**Sorted Sets:** ZADD, ZREM, ZRANGE, ZRANGEBYSCORE, ZRANK
**Keys:** DEL, EXISTS, EXPIRE, TTL, KEYS, SCAN, TYPE
**Time Series:** TS.CREATE, TS.ADD, TS.RANGE, TS.GET (RedisTimeSeries compatible)
**Vectors:** VECTOR.ADD, VECTOR.SEARCH (AI/ML similarity search)

### Step 2: Data Export from Redis

**Option A: RDB Snapshot**

```bash
# Trigger RDB save on source Redis
redis-cli -h source-redis BGSAVE

# Wait for completion
redis-cli -h source-redis LASTSAVE

# Copy RDB file (if direct access available)
scp source-redis:/var/lib/redis/dump.rdb ./dump.rdb
```

**Option B: SCAN and Migrate (Recommended)**

```bash
# Create migration script
cat > migrate_redis.py << 'EOF'
import redis

source = redis.Redis(host='source-redis', port=6379)
target = redis.Redis(host='localhost', port=6379)  # Orbit-RS

def migrate_key(key):
    key_type = source.type(key).decode()
    ttl = source.ttl(key)

    if key_type == 'string':
        value = source.get(key)
        target.set(key, value)
    elif key_type == 'hash':
        value = source.hgetall(key)
        target.hset(key, mapping=value)
    elif key_type == 'list':
        values = source.lrange(key, 0, -1)
        if values:
            target.rpush(key, *values)
    elif key_type == 'set':
        values = source.smembers(key)
        if values:
            target.sadd(key, *values)
    elif key_type == 'zset':
        values = source.zrange(key, 0, -1, withscores=True)
        if values:
            target.zadd(key, dict(values))

    if ttl > 0:
        target.expire(key, ttl)

# Migrate all keys using SCAN
cursor = 0
migrated = 0
while True:
    cursor, keys = source.scan(cursor, count=1000)
    for key in keys:
        migrate_key(key)
        migrated += 1
        if migrated % 10000 == 0:
            print(f"Migrated {migrated} keys...")
    if cursor == 0:
        break

print(f"Migration complete: {migrated} keys")
EOF

python migrate_redis.py
```

**Option C: redis-dump/redis-load**

```bash
# Install redis-dump tool
npm install -g redis-dump

# Export from source Redis
redis-dump -h source-redis -p 6379 > redis_backup.json

# Import to Orbit-RS (using custom script)
cat > load_redis.py << 'EOF'
import json
import redis

target = redis.Redis(host='localhost', port=6379)

with open('redis_backup.json', 'r') as f:
    for line in f:
        data = json.loads(line)
        key = data['key']
        value = data['value']
        ttl = data.get('ttl', -1)

        # Set based on type
        if isinstance(value, str):
            target.set(key, value)
        elif isinstance(value, dict):
            target.hset(key, mapping=value)
        elif isinstance(value, list):
            target.rpush(key, *value)

        if ttl > 0:
            target.expire(key, ttl)
EOF

python load_redis.py
```

### Step 3: Application Cutover

**Update Connection:**

```python
# Before (Redis)
import redis
r = redis.Redis(host='redis-host', port=6379)

# After (Orbit-RS) - No code change needed!
import redis
r = redis.Redis(host='orbit-host', port=6379)
```

```javascript
// Before (Redis)
const Redis = require('ioredis');
const redis = new Redis({ host: 'redis-host', port: 6379 });

// After (Orbit-RS)
const redis = new Redis({ host: 'orbit-host', port: 6379 });
```

### Step 4: Verification

```bash
# Connect to Orbit-RS Redis port
redis-cli -h localhost -p 6379

# Check key count
DBSIZE

# Sample keys
SCAN 0 COUNT 10

# Verify data types
TYPE user:1
HGETALL user:1

# Test operations
SET test:key "hello"
GET test:key
INCR counter:test
```

---

## Multi-Database Consolidation

Consolidate PostgreSQL, Redis, MySQL, and other databases into a single Orbit-RS instance.

### Architecture Overview

```
Before:                              After:
┌─────────────┐                     ┌─────────────────────────────────┐
│ PostgreSQL  │──┐                  │         Orbit-RS                │
│   :5432     │  │                  │                                 │
└─────────────┘  │                  │  ┌─────────┐ ┌─────────┐       │
                 │                  │  │ PG Wire │ │  RESP   │       │
┌─────────────┐  │ ┌─────────┐     │  │  :5432  │ │  :6379  │       │
│   Redis     │──┼─│   App   │ ──► │  └────┬────┘ └────┬────┘       │
│   :6379     │  │ └─────────┘     │       │           │             │
└─────────────┘  │                  │  ┌────▼───────────▼────┐       │
                 │                  │  │  Unified Storage     │       │
┌─────────────┐  │                  │  │  (RocksDB + Tiering) │       │
│   MySQL     │──┘                  │  └──────────────────────┘       │
│   :3306     │                     └─────────────────────────────────┘
└─────────────┘
```

### Step 1: Plan Migration Order

**Recommended Order:**
1. **Redis first** - Usually stateless/cacheable, lowest risk
2. **PostgreSQL second** - Core relational data
3. **MySQL third** - Additional relational workloads
4. **CQL/Cassandra last** - Wide-column workloads

### Step 2: Set Up Orbit-RS

```bash
# Start Orbit-RS with all protocols enabled
./orbit-server --config orbit-server.toml

# Verify all ports are listening
netstat -tlnp | grep orbit
# Expected:
# :5432 (PostgreSQL)
# :6379 (Redis)
# :3306 (MySQL)
# :9042 (CQL)
# :8080 (REST API)
```

### Step 3: Parallel Migration

**Create migration script for each database:**

```bash
#!/bin/bash
# migrate_all.sh

echo "=== Starting Multi-Database Migration ==="

# Phase 1: Redis (cache data)
echo "Phase 1: Migrating Redis..."
python migrate_redis.py
echo "Redis migration complete"

# Phase 2: PostgreSQL (primary data)
echo "Phase 2: Migrating PostgreSQL..."
pg_dump -h pg-host -U postgres -d maindb > pg_backup.sql
psql -h localhost -p 5432 -U orbit -d actors < pg_backup.sql
echo "PostgreSQL migration complete"

# Phase 3: MySQL (secondary data)
echo "Phase 3: Migrating MySQL..."
mysqldump -h mysql-host -u root -p appdb > mysql_backup.sql
mysql -h localhost -P 3306 -u orbit -p actors < mysql_backup.sql
echo "MySQL migration complete"

echo "=== Migration Complete ==="
```

### Step 4: Update Application Configuration

**Before (Multiple Databases):**

```yaml
# config.yaml
databases:
  postgres:
    host: postgres-host
    port: 5432
    database: maindb
  redis:
    host: redis-host
    port: 6379
  mysql:
    host: mysql-host
    port: 3306
    database: appdb
```

**After (Orbit-RS):**

```yaml
# config.yaml
databases:
  postgres:
    host: orbit-host
    port: 5432
    database: actors
  redis:
    host: orbit-host
    port: 6379
  mysql:
    host: orbit-host
    port: 3306
    database: actors
```

### Step 5: Cross-Protocol Queries

Orbit-RS enables queries across data that was previously siloed:

```sql
-- Query combining relational and cached data
-- (Via PostgreSQL wire protocol with Redis data access)

-- User profile with cached session data
SELECT
    u.id,
    u.name,
    u.email,
    redis_get('session:' || u.id) as session_data,
    redis_hget('user:' || u.id || ':prefs', 'theme') as theme
FROM users u
WHERE u.active = true;
```

---

## Data Verification

### Automated Verification Script

```bash
#!/bin/bash
# verify_migration.sh

echo "=== Data Verification ==="

# PostgreSQL verification
echo "Checking PostgreSQL tables..."
psql -h localhost -p 5432 -U orbit -d actors -c "
SELECT
    schemaname,
    tablename,
    n_live_tup as row_count
FROM pg_stat_user_tables
ORDER BY n_live_tup DESC;
"

# Redis verification
echo "Checking Redis keys..."
redis-cli -h localhost -p 6379 INFO keyspace

# Compare with source (if accessible)
echo "Comparing row counts..."
SOURCE_COUNT=$(psql -h source-postgres -U postgres -t -c "SELECT COUNT(*) FROM users;")
TARGET_COUNT=$(psql -h localhost -p 5432 -U orbit -t -c "SELECT COUNT(*) FROM users;")

if [ "$SOURCE_COUNT" = "$TARGET_COUNT" ]; then
    echo "Row counts match: $SOURCE_COUNT"
else
    echo "WARNING: Row count mismatch! Source: $SOURCE_COUNT, Target: $TARGET_COUNT"
fi
```

### Checksum Verification

```sql
-- Generate checksums for verification
SELECT
    'users' as table_name,
    COUNT(*) as row_count,
    SUM(HASHTEXT(id::text || email || COALESCE(name, ''))) as checksum
FROM users;
```

---

## Rollback Procedures

### Quick Rollback

If issues occur during migration:

```bash
# 1. Stop routing traffic to Orbit-RS
# Update load balancer or DNS to point back to source

# 2. Verify source database is still operational
psql -h source-postgres -U postgres -c "SELECT 1;"
redis-cli -h source-redis PING

# 3. Application will automatically reconnect to source
```

### Data Rollback (If Writes Occurred)

```bash
# Export any new data from Orbit-RS
pg_dump -h localhost -p 5432 -U orbit -d actors \
  --data-only \
  --table=users \
  --where="created_at > '2024-01-15'" \
  > new_data.sql

# Import back to source PostgreSQL
psql -h source-postgres -U postgres -d your_database < new_data.sql
```

---

## Migration Support

Need help with your migration?

- **Documentation:** [Project Overview](../project_overview.md)
- **GitHub Issues:** [Report Migration Issues](https://github.com/TuringWorks/orbit-rs/issues)
- **Discussions:** [Community Q&A](https://github.com/TuringWorks/orbit-rs/discussions)

---

**Last Updated:** November 2025
