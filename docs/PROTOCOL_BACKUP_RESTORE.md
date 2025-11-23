# Protocol Backup and Restore Procedures

This guide provides comprehensive backup and restore procedures for all Orbit-RS protocol implementations.

## Table of Contents

1. [Overview](#overview)
2. [Backup Strategies](#backup-strategies)
3. [AQL/ArangoDB Protocol](#aql-arangodb-protocol)
4. [Cypher/Neo4j Protocol](#cypher-neo4j-protocol)
5. [GraphRAG Protocol](#graphrag-protocol)
6. [PostgreSQL Wire Protocol](#postgresql-wire-protocol)
7. [Cross-Protocol Backup](#cross-protocol-backup)
8. [Disaster Recovery](#disaster-recovery)
9. [Automated Backup](#automated-backup)
10. [Cloud Backup](#cloud-backup)

---

## Overview

### Backup Types

| Type | Description | Use Case | Recovery Time |
|------|-------------|----------|---------------|
| **Full** | Complete data snapshot | Initial backup, monthly archives | Hours |
| **Incremental** | Changes since last backup | Daily backups | Minutes |
| **Continuous** | Real-time replication | HA/DR scenarios | Seconds |
| **Logical** | Export in readable format | Migration, audit | Hours |
| **Physical** | Raw RocksDB files | Fast recovery | Minutes |

### Backup Location Matrix

```
/backup/
├── daily/
│   ├── 2025-03-23/
│   │   ├── aql/
│   │   ├── cypher/
│   │   ├── graphrag/
│   │   └── postgres/
│   └── 2025-03-24/
├── weekly/
│   └── 2025-W12/
├── monthly/
│   └── 2025-03/
└── archives/
```

### RTO/RPO Targets

| Criticality | RTO (Recovery Time Objective) | RPO (Recovery Point Objective) |
|-------------|-------------------------------|--------------------------------|
| **Critical** | < 1 hour | < 15 minutes |
| **High** | < 4 hours | < 1 hour |
| **Medium** | < 24 hours | < 6 hours |
| **Low** | < 72 hours | < 24 hours |

---

## Backup Strategies

### Strategy 1: Full + Incremental (Recommended)

```bash
# Full backup weekly (Sunday 2 AM)
0 2 * * 0 /usr/local/bin/backup-full.sh

# Incremental backup daily (2 AM)
0 2 * * 1-6 /usr/local/bin/backup-incremental.sh

# Retention: 7 daily, 4 weekly, 12 monthly
```

### Strategy 2: Continuous Replication

```bash
# Enable WAL-based replication
orbit-server --enable-wal-replication \
  --replica-host backup-server:5678

# Asynchronous replication with <5s lag
```

### Strategy 3: Snapshot-Based

```bash
# Use filesystem snapshots (LVM/ZFS/Btrfs)
# Daily snapshots with copy-on-write
lvcreate --size 10G --snapshot --name orbit-snap /dev/vg0/orbit-data
```

---

## AQL/ArangoDB Protocol

### Full Backup

#### Method 1: Physical Backup (Fast)

```bash
#!/bin/bash
# backup-aql.sh

TIMESTAMP=$(date +%Y%m%d-%H%M%S)
DATA_DIR="/var/lib/orbit-data/aql"
BACKUP_DIR="/backup/aql/full-${TIMESTAMP}"

# Stop writes (optional - for consistency)
orbit-cli aql pause-writes

# Backup RocksDB files
mkdir -p "${BACKUP_DIR}"
rsync -av --progress "${DATA_DIR}/rocksdb/" "${BACKUP_DIR}/rocksdb/"

# Backup metadata
cp -r "${DATA_DIR}/metadata" "${BACKUP_DIR}/"

# Create checksum
cd "${BACKUP_DIR}" && sha256sum -r . > checksums.sha256

# Resume writes
orbit-cli aql resume-writes

# Compress backup (optional)
tar -czf "${BACKUP_DIR}.tar.gz" -C "$(dirname ${BACKUP_DIR})" "$(basename ${BACKUP_DIR})"

echo "Backup completed: ${BACKUP_DIR}.tar.gz"
```

#### Method 2: Logical Backup (Portable)

```bash
# Export all collections to JSON
orbit-export aql \
  --data-dir /var/lib/orbit-data/aql \
  --output /backup/aql/export-${TIMESTAMP}.jsonl \
  --format jsonl \
  --include-metadata \
  --compress gzip

# Export with collection separation
orbit-export aql \
  --data-dir /var/lib/orbit-data/aql \
  --output-dir /backup/aql/collections-${TIMESTAMP} \
  --format jsonl \
  --split-by-collection \
  --compress gzip
```

### Incremental Backup

```bash
#!/bin/bash
# backup-aql-incremental.sh

LAST_BACKUP=$(cat /var/lib/orbit-data/aql/.last-backup-timestamp)
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
BACKUP_DIR="/backup/aql/incr-${TIMESTAMP}"

# Backup only changed files since last backup
mkdir -p "${BACKUP_DIR}"
rsync -av --progress \
  --compare-dest="/backup/aql/full-$(cat /var/lib/orbit-data/aql/.last-full-backup)/" \
  "${DATA_DIR}/rocksdb/" \
  "${BACKUP_DIR}/rocksdb/"

# Save timestamp
echo "${TIMESTAMP}" > /var/lib/orbit-data/aql/.last-backup-timestamp
```

### Restore AQL Data

#### Full Restore

```bash
#!/bin/bash
# restore-aql.sh

BACKUP_FILE="/backup/aql/full-20250323-020000.tar.gz"
DATA_DIR="/var/lib/orbit-data/aql"

# Stop server
systemctl stop orbit-server

# Backup current data (safety)
mv "${DATA_DIR}" "${DATA_DIR}.before-restore"

# Extract backup
mkdir -p "${DATA_DIR}"
tar -xzf "${BACKUP_FILE}" -C "${DATA_DIR}" --strip-components=1

# Verify checksums
cd "${DATA_DIR}" && sha256sum -c checksums.sha256

if [ $? -eq 0 ]; then
    echo "Checksum verification passed"
    # Start server
    systemctl start orbit-server

    # Verify data
    orbit-cli aql verify
else
    echo "Checksum verification FAILED - rolling back"
    rm -rf "${DATA_DIR}"
    mv "${DATA_DIR}.before-restore" "${DATA_DIR}"
    systemctl start orbit-server
    exit 1
fi
```

#### Logical Restore

```bash
# Restore from JSON export
orbit-import aql \
  --input /backup/aql/export-20250323.jsonl.gz \
  --data-dir /var/lib/orbit-data/aql \
  --format jsonl \
  --decompress gzip \
  --batch-size 5000 \
  --on-conflict replace

# Verify import
orbit-cli aql query "FOR c IN collections RETURN {name: c, count: LENGTH(c)}"
```

#### Point-in-Time Recovery

```bash
# Restore full backup
./restore-aql.sh /backup/aql/full-20250320.tar.gz

# Apply incremental backups in order
for INCR in /backup/aql/incr-2025032[1-3]-*.tar.gz; do
    echo "Applying incremental: ${INCR}"
    rsync -av --progress \
        <(tar -xzf "${INCR}" -O) \
        /var/lib/orbit-data/aql/rocksdb/
done

# Replay WAL to specific timestamp
orbit-cli aql replay-wal \
  --until-timestamp "2025-03-23T14:30:00Z"
```

---

## Cypher/Neo4j Protocol

### Full Backup

#### Physical Backup

```bash
#!/bin/bash
# backup-cypher.sh

TIMESTAMP=$(date +%Y%m%d-%H%M%S)
DATA_DIR="/var/lib/orbit-data/cypher"
BACKUP_DIR="/backup/cypher/full-${TIMESTAMP}"

# Create checkpoint (flush memtables)
orbit-cli cypher checkpoint

# Backup RocksDB files
mkdir -p "${BACKUP_DIR}"
rsync -av --progress "${DATA_DIR}/rocksdb/" "${BACKUP_DIR}/rocksdb/"

# Backup graph metadata
orbit-export cypher \
  --metadata-only \
  --output "${BACKUP_DIR}/metadata.json"

# Create manifest
cat > "${BACKUP_DIR}/MANIFEST" <<EOF
timestamp: ${TIMESTAMP}
protocol: cypher
backup_type: full
node_count: $(orbit-cli cypher query "MATCH (n) RETURN count(n)" | jq -r '.[0]["count(n)"]')
relationship_count: $(orbit-cli cypher query "MATCH ()-[r]->() RETURN count(r)" | jq -r '.[0]["count(r)"]')
EOF

# Compress
tar -czf "${BACKUP_DIR}.tar.gz" -C "$(dirname ${BACKUP_DIR})" "$(basename ${BACKUP_DIR})"
```

#### Logical Backup (Cypher Export)

```bash
# Export entire graph as Cypher statements
orbit-export cypher \
  --data-dir /var/lib/orbit-data/cypher \
  --output /backup/cypher/graph-${TIMESTAMP}.cypher \
  --format cypher \
  --include-indexes

# Example output:
# CREATE (n:Person {id: '1', name: 'Alice', age: 30});
# CREATE (n:Person {id: '2', name: 'Bob', age: 25});
# MATCH (a:Person {id: '1'}), (b:Person {id: '2'})
# CREATE (a)-[:KNOWS {since: 2020}]->(b);
```

### Restore Cypher Data

#### Physical Restore

```bash
#!/bin/bash
# restore-cypher.sh

BACKUP_FILE="/backup/cypher/full-20250323.tar.gz"
DATA_DIR="/var/lib/orbit-data/cypher"

# Stop server
systemctl stop orbit-server

# Restore data
rm -rf "${DATA_DIR}"
mkdir -p "${DATA_DIR}"
tar -xzf "${BACKUP_FILE}" -C "${DATA_DIR}" --strip-components=1

# Start server
systemctl start orbit-server

# Verify graph integrity
orbit-cli cypher query "
MATCH (n)
WITH labels(n) as nodeType, count(n) as count
RETURN nodeType, count
ORDER BY count DESC
"
```

#### Logical Restore (Cypher Import)

```bash
# Clear existing data (if needed)
orbit-cli cypher query "MATCH (n) DETACH DELETE n"

# Import Cypher file
orbit-import cypher \
  --input /backup/cypher/graph-20250323.cypher \
  --data-dir /var/lib/orbit-data/cypher \
  --batch-size 1000

# Rebuild indexes
orbit-cli cypher query "
CREATE INDEX person_id FOR (p:Person) ON (p.id);
CREATE INDEX person_name FOR (p:Person) ON (p.name);
"
```

### Graph Consistency Check

```bash
# Verify graph structure
orbit-cli cypher verify \
  --check-dangling-relationships \
  --check-duplicate-nodes \
  --check-index-consistency

# Example output:
# ✓ No dangling relationships found
# ✓ No duplicate nodes found
# ✓ All indexes consistent
# Graph statistics:
#   Nodes: 1,234,567
#   Relationships: 5,678,901
#   Node labels: 12
#   Relationship types: 8
```

---

## GraphRAG Protocol

### Full Backup

#### Physical Backup with Embeddings

```bash
#!/bin/bash
# backup-graphrag.sh

TIMESTAMP=$(date +%Y%m%d-%H%M%S)
DATA_DIR="/var/lib/orbit-data/graphrag"
BACKUP_DIR="/backup/graphrag/full-${TIMESTAMP}"

# Create checkpoint
orbit-cli graphrag checkpoint --kg my_knowledge_graph

# Backup RocksDB files (including embeddings)
mkdir -p "${BACKUP_DIR}"
rsync -av --progress "${DATA_DIR}/rocksdb/" "${BACKUP_DIR}/rocksdb/"

# Export knowledge graph metadata
orbit-export graphrag \
  --kg-name my_knowledge_graph \
  --metadata-only \
  --output "${BACKUP_DIR}/kg-metadata.json"

# Save embedding model info
cat > "${BACKUP_DIR}/embedding-model.json" <<EOF
{
  "model_name": "sentence-transformers/all-MiniLM-L6-v2",
  "model_version": "v2.0",
  "embedding_dim": 384,
  "last_updated": "${TIMESTAMP}"
}
EOF

tar -czf "${BACKUP_DIR}.tar.gz" -C "$(dirname ${BACKUP_DIR})" "$(basename ${BACKUP_DIR})"
```

#### Logical Backup (JSON + Embeddings)

```bash
# Export with embeddings (large file)
orbit-export graphrag \
  --kg-name my_knowledge_graph \
  --data-dir /var/lib/orbit-data/graphrag \
  --output /backup/graphrag/kg-${TIMESTAMP}.json \
  --format json \
  --include-embeddings \
  --compress gzip

# Export without embeddings (smaller, can recompute)
orbit-export graphrag \
  --kg-name my_knowledge_graph \
  --data-dir /var/lib/orbit-data/graphrag \
  --output /backup/graphrag/kg-no-emb-${TIMESTAMP}.json \
  --format json \
  --exclude-embeddings \
  --compress gzip
```

### Restore GraphRAG Data

#### Physical Restore

```bash
#!/bin/bash
# restore-graphrag.sh

BACKUP_FILE="/backup/graphrag/full-20250323.tar.gz"
DATA_DIR="/var/lib/orbit-data/graphrag"

systemctl stop orbit-server

# Restore files
rm -rf "${DATA_DIR}"
mkdir -p "${DATA_DIR}"
tar -xzf "${BACKUP_FILE}" -C "${DATA_DIR}" --strip-components=1

systemctl start orbit-server

# Verify knowledge graph
orbit-cli graphrag verify --kg my_knowledge_graph
```

#### Logical Restore with Embedding Recomputation

```bash
# Import entities and relationships (without embeddings)
orbit-import graphrag \
  --input /backup/graphrag/kg-no-emb-20250323.json.gz \
  --kg-name my_knowledge_graph \
  --data-dir /var/lib/orbit-data/graphrag \
  --format json \
  --decompress gzip

# Recompute embeddings (if model changed)
orbit-cli graphrag recompute-embeddings \
  --kg my_knowledge_graph \
  --model sentence-transformers/all-mpnet-base-v2 \
  --batch-size 100 \
  --parallel-workers 4

# Rebuild vector index
orbit-cli graphrag rebuild-index --kg my_knowledge_graph
```

### Embedding Migration

When upgrading embedding models:

```bash
# Backup current embeddings
orbit-export graphrag \
  --kg-name my_knowledge_graph \
  --embeddings-only \
  --output /backup/graphrag/embeddings-old-model.bin

# Recompute with new model
orbit-cli graphrag recompute-embeddings \
  --kg my_knowledge_graph \
  --model sentence-transformers/all-mpnet-base-v2

# Compare quality (optional)
orbit-cli graphrag compare-embeddings \
  --kg my_knowledge_graph \
  --old-backup /backup/graphrag/embeddings-old-model.bin \
  --metrics cosine-similarity

# Rollback if needed
orbit-import graphrag \
  --embeddings-only \
  --input /backup/graphrag/embeddings-old-model.bin
```

---

## PostgreSQL Wire Protocol

### Full Backup

```bash
#!/bin/bash
# backup-postgres.sh

TIMESTAMP=$(date +%Y%m%d-%H%M%S)
DATA_DIR="/var/lib/orbit-data/postgres"
BACKUP_DIR="/backup/postgres/full-${TIMESTAMP}"

# Backup using pg_dump-like format
orbit-export postgres \
  --data-dir "${DATA_DIR}" \
  --output "${BACKUP_DIR}/dump.sql" \
  --format sql \
  --include-schema \
  --include-data

# Or backup as RocksDB files
rsync -av --progress "${DATA_DIR}/rocksdb/" "${BACKUP_DIR}/rocksdb/"

tar -czf "${BACKUP_DIR}.tar.gz" -C "$(dirname ${BACKUP_DIR})" "$(basename ${BACKUP_DIR})"
```

### Restore PostgreSQL Data

```bash
# Restore from SQL dump
orbit-import postgres \
  --input /backup/postgres/dump.sql \
  --data-dir /var/lib/orbit-data/postgres \
  --format sql

# Or restore RocksDB files
systemctl stop orbit-server
rm -rf /var/lib/orbit-data/postgres/rocksdb
tar -xzf /backup/postgres/full-20250323.tar.gz -C /var/lib/orbit-data/postgres
systemctl start orbit-server
```

---

## Cross-Protocol Backup

### Full System Backup

```bash
#!/bin/bash
# backup-all-protocols.sh

TIMESTAMP=$(date +%Y%m%d-%H%M%S)
BACKUP_ROOT="/backup/full-system/${TIMESTAMP}"

mkdir -p "${BACKUP_ROOT}"

# Backup each protocol
for PROTOCOL in aql cypher graphrag postgres; do
    echo "Backing up ${PROTOCOL}..."
    orbit-backup ${PROTOCOL} \
        --data-dir /var/lib/orbit-data/${PROTOCOL} \
        --output "${BACKUP_ROOT}/${PROTOCOL}" \
        --format physical \
        --compress gzip
done

# Backup server configuration
cp /etc/orbit-server/config.yaml "${BACKUP_ROOT}/config.yaml"

# Create system manifest
cat > "${BACKUP_ROOT}/MANIFEST.yaml" <<EOF
backup_timestamp: ${TIMESTAMP}
orbit_version: $(orbit-server --version)
protocols:
  aql:
    collections: $(orbit-cli aql query "RETURN LENGTH(collections)" | jq -r '.[0]')
  cypher:
    nodes: $(orbit-cli cypher query "MATCH (n) RETURN count(n)" | jq -r '.[0]["count(n)"]')
  graphrag:
    entities: $(orbit-cli graphrag stats --kg all | jq -r '.total_entities')
  postgres:
    tables: $(orbit-cli postgres query "SELECT count(*) FROM information_schema.tables" | jq -r '.[0].count')
EOF

# Create archive
tar -czf "${BACKUP_ROOT}.tar.gz" -C "$(dirname ${BACKUP_ROOT})" "$(basename ${BACKUP_ROOT})"

# Upload to cloud (optional)
aws s3 cp "${BACKUP_ROOT}.tar.gz" s3://orbit-backups/full-system/

echo "Full system backup completed: ${BACKUP_ROOT}.tar.gz"
```

### Selective Protocol Restore

```bash
# Restore only specific protocols
orbit-restore \
  --backup /backup/full-system/20250323.tar.gz \
  --protocols aql,cypher \
  --data-dir /var/lib/orbit-data \
  --verify-checksums
```

---

## Disaster Recovery

### DR Scenario 1: Complete Data Loss

```bash
# 1. Provision new server
# 2. Install Orbit-RS
# 3. Download latest backup
aws s3 cp s3://orbit-backups/full-system/latest.tar.gz /tmp/

# 4. Restore all protocols
tar -xzf /tmp/latest.tar.gz -C /var/lib/orbit-data

# 5. Start server
systemctl start orbit-server

# 6. Verify all protocols
for PROTOCOL in aql cypher graphrag postgres; do
    orbit-cli ${PROTOCOL} verify
done
```

### DR Scenario 2: Corruption in Single Protocol

```bash
# 1. Identify corrupted protocol
orbit-cli verify-all

# Output: Cypher protocol corrupted (checksum mismatch)

# 2. Stop server
systemctl stop orbit-server

# 3. Restore only corrupted protocol
orbit-restore \
  --backup /backup/full-system/20250323.tar.gz \
  --protocols cypher \
  --data-dir /var/lib/orbit-data

# 4. Verify restoration
orbit-cli cypher verify --detailed

# 5. Start server
systemctl start orbit-server
```

### DR Scenario 3: Ransomware Attack

```bash
# 1. Immediately isolate system
systemctl stop orbit-server
iptables -A INPUT -j DROP
iptables -A OUTPUT -j DROP

# 2. Assess damage
find /var/lib/orbit-data -name "*.encrypted" -o -name "README.txt"

# 3. Restore from immutable backup
# (Assumes backups are in write-once/read-many storage)
aws s3 cp s3://orbit-backups-immutable/20250320.tar.gz /tmp/

# 4. Wipe and restore
rm -rf /var/lib/orbit-data
mkdir -p /var/lib/orbit-data
tar -xzf /tmp/20250320.tar.gz -C /var/lib/orbit-data

# 5. Update firewall rules
# 6. Start server and verify
systemctl start orbit-server
orbit-cli verify-all --paranoid
```

---

## Automated Backup

### Systemd Timer Setup

```ini
# /etc/systemd/system/orbit-backup.timer
[Unit]
Description=Orbit-RS Daily Backup Timer

[Timer]
OnCalendar=daily
OnCalendar=02:00
Persistent=true

[Install]
WantedBy=timers.target
```

```ini
# /etc/systemd/system/orbit-backup.service
[Unit]
Description=Orbit-RS Backup Service
After=network.target

[Service]
Type=oneshot
User=orbit
ExecStart=/usr/local/bin/backup-all-protocols.sh
StandardOutput=journal
StandardError=journal
```

Enable and start:
```bash
systemctl enable orbit-backup.timer
systemctl start orbit-backup.timer
systemctl list-timers --all
```

### Backup Rotation Script

```bash
#!/bin/bash
# rotate-backups.sh

BACKUP_DIR="/backup"

# Keep: 7 daily, 4 weekly, 12 monthly
# Delete daily backups older than 7 days
find "${BACKUP_DIR}/daily" -type f -mtime +7 -delete

# Delete weekly backups older than 4 weeks
find "${BACKUP_DIR}/weekly" -type f -mtime +28 -delete

# Delete monthly backups older than 12 months
find "${BACKUP_DIR}/monthly" -type f -mtime +365 -delete

# Archive old backups to cold storage
find "${BACKUP_DIR}/monthly" -type f -mtime +90 | while read -r file; do
    aws s3 cp "${file}" s3://orbit-backups-archive/ --storage-class GLACIER
    rm "${file}"
done
```

---

## Cloud Backup

### AWS S3 Integration

```bash
# Upload backup to S3
aws s3 cp /backup/aql/full-20250323.tar.gz \
    s3://orbit-backups/aql/ \
    --storage-class STANDARD_IA \
    --server-side-encryption AES256

# Enable versioning
aws s3api put-bucket-versioning \
    --bucket orbit-backups \
    --versioning-configuration Status=Enabled

# Set lifecycle policy
cat > lifecycle-policy.json <<EOF
{
  "Rules": [
    {
      "Id": "MoveToGlacier",
      "Status": "Enabled",
      "Transitions": [
        {
          "Days": 90,
          "StorageClass": "GLACIER"
        }
      ],
      "Expiration": {
        "Days": 2555
      }
    }
  ]
}
EOF

aws s3api put-bucket-lifecycle-configuration \
    --bucket orbit-backups \
    --lifecycle-configuration file://lifecycle-policy.json
```

### Restore from S3

```bash
# List available backups
aws s3 ls s3://orbit-backups/full-system/

# Download backup
aws s3 cp s3://orbit-backups/full-system/20250323.tar.gz /tmp/

# Restore from Glacier (if archived)
aws s3api restore-object \
    --bucket orbit-backups \
    --key full-system/20250323.tar.gz \
    --restore-request Days=7

# Wait for restoration (check status)
aws s3api head-object \
    --bucket orbit-backups \
    --key full-system/20250323.tar.gz

# Download when ready
aws s3 cp s3://orbit-backups/full-system/20250323.tar.gz /tmp/
```

### Azure Blob Storage

```bash
# Upload to Azure
az storage blob upload \
    --account-name orbitbackups \
    --container-name backups \
    --name aql/full-20250323.tar.gz \
    --file /backup/aql/full-20250323.tar.gz \
    --tier Cool

# Download from Azure
az storage blob download \
    --account-name orbitbackups \
    --container-name backups \
    --name aql/full-20250323.tar.gz \
    --file /tmp/restore.tar.gz
```

---

## Monitoring and Alerts

### Backup Health Checks

```bash
#!/bin/bash
# check-backup-health.sh

# Check if backup exists
LATEST_BACKUP=$(ls -t /backup/daily/ | head -1)
if [ -z "${LATEST_BACKUP}" ]; then
    echo "CRITICAL: No recent backups found"
    exit 2
fi

# Check backup age
BACKUP_AGE=$(find /backup/daily/${LATEST_BACKUP} -mtime +1)
if [ -n "${BACKUP_AGE}" ]; then
    echo "WARNING: Latest backup is older than 24 hours"
    exit 1
fi

# Verify backup integrity
tar -tzf /backup/daily/${LATEST_BACKUP}/*.tar.gz > /dev/null 2>&1
if [ $? -ne 0 ]; then
    echo "CRITICAL: Backup integrity check failed"
    exit 2
fi

echo "OK: Backup health check passed"
exit 0
```

### Alert Integration

```yaml
# prometheus-alerts.yml
groups:
  - name: orbit_backup_alerts
    rules:
      - alert: BackupTooOld
        expr: time() - orbit_last_backup_timestamp_seconds > 86400
        annotations:
          summary: "Orbit backup is too old"
          description: "Last backup was {{ $value }} seconds ago"

      - alert: BackupFailed
        expr: orbit_backup_failures_total > 0
        annotations:
          summary: "Orbit backup failed"
          description: "{{ $value }} backup failures detected"
```

---

## Best Practices

### DO
✅ Test restores regularly (at least monthly)
✅ Store backups in multiple locations
✅ Encrypt backups containing sensitive data
✅ Use immutable storage for critical backups
✅ Monitor backup success/failure
✅ Document restore procedures
✅ Verify checksums after restore

### DON'T
❌ Store backups on same disk as data
❌ Skip testing restore procedures
❌ Forget to backup configuration files
❌ Keep backups indefinitely (cost/compliance)
❌ Use unencrypted backups for sensitive data
❌ Assume backups are good without verification

---

## Backup Checklist

### Daily
- [ ] Run incremental backup
- [ ] Verify backup completed successfully
- [ ] Check backup size is reasonable
- [ ] Rotate old daily backups

### Weekly
- [ ] Run full backup
- [ ] Test restore on staging environment
- [ ] Verify backup integrity
- [ ] Upload to cloud storage

### Monthly
- [ ] Archive monthly backup
- [ ] Perform disaster recovery drill
- [ ] Review backup retention policy
- [ ] Update documentation

### Quarterly
- [ ] Full DR test (restore to new system)
- [ ] Review and update procedures
- [ ] Audit backup access logs
- [ ] Capacity planning for backups

---

## Emergency Contacts

- **On-Call Engineer**: +1-555-ORBIT-RS
- **Backup Administrator**: backup-admin@orbit-rs.dev
- **Cloud Support**: AWS/Azure support portal
- **Escalation**: cto@orbit-rs.dev

---

## Additional Resources

- Backup scripts: `orbit/scripts/backup/`
- Restore scripts: `orbit/scripts/restore/`
- Monitoring dashboards: `orbit/monitoring/grafana/`
- Runbooks: `orbit/docs/runbooks/`

For questions or issues, contact: support@orbit-rs.dev
