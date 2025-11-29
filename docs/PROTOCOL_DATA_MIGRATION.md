# Protocol Data Migration Guide

This guide provides comprehensive procedures for migrating data between Orbit-RS versions, across protocols, and from external systems.

## Table of Contents

1. [Overview](#overview)
2. [Version Migration](#version-migration)
3. [Cross-Protocol Migration](#cross-protocol-migration)
4. [External System Migration](#external-system-migration)
5. [Data Format Changes](#data-format-changes)
6. [Migration Tools](#migration-tools)
7. [Rollback Procedures](#rollback-procedures)

---

## Overview

### Migration Scenarios

1. **Version Migration** - Upgrading Orbit-RS to a new version
2. **Cross-Protocol Migration** - Moving data between protocols (e.g., AQL → Cypher)
3. **Import Migration** - Importing from external systems (ArangoDB, Neo4j, etc.)
4. **Export Migration** - Exporting to external systems

### Pre-Migration Checklist

- [ ] **Backup all data** (see [PROTOCOL_BACKUP_RESTORE.md](PROTOCOL_BACKUP_RESTORE.md))
- [ ] **Test migration on copy** of production data
- [ ] **Document current data schema** and statistics
- [ ] **Plan downtime window** (if required)
- [ ] **Prepare rollback plan**
- [ ] **Verify disk space** (2-3x current data size)
- [ ] **Check compatibility matrix**

---

## Version Migration

### Compatibility Matrix

| From Version | To Version | Migration Type | Downtime Required |
|--------------|------------|----------------|-------------------|
| 0.1.x → 0.2.x | Rolling | No downtime | No |
| 0.2.x → 0.3.x | In-place | Minimal | Optional |
| 0.x.x → 1.0.0 | Manual | Required | Yes |

### Rolling Migration (Minor Versions)

For backward-compatible changes (patch/minor versions):

#### Step 1: Backup Current State

```bash
# Create backup
./scripts/backup-protocol.sh aql /backup/aql-v0.1.0
./scripts/backup-protocol.sh cypher /backup/cypher-v0.1.0
./scripts/backup-protocol.sh graphrag /backup/graphrag-v0.1.0
```

#### Step 2: Update Binary

```bash
# Stop server gracefully
systemctl stop orbit-server

# Backup old binary
cp /usr/local/bin/orbit-server /usr/local/bin/orbit-server.v0.1.0

# Install new version
cargo build --release
cp target/release/orbit-server /usr/local/bin/

# Start server
systemctl start orbit-server
```

#### Step 3: Verify Migration

```bash
# Check logs
journalctl -u orbit-server -f

# Verify data integrity
orbit-cli verify --protocol aql
orbit-cli verify --protocol cypher
orbit-cli verify --protocol graphrag
```

### In-Place Migration (Major Schema Changes)

For versions with schema changes:

#### Step 1: Pre-Migration Validation

```bash
# Run migration validator
orbit-migrate validate \
  --from-version 0.2.0 \
  --to-version 0.3.0 \
  --data-dir /var/lib/orbit-data

# Output example:
# ✓ AQL schema compatible
# ✓ Cypher schema compatible
# ⚠ GraphRAG requires embedding re-indexing
# Estimated migration time: 2 hours
# Required disk space: 50 GB
```

#### Step 2: Run Migration

```bash
# Stop server
systemctl stop orbit-server

# Run migration tool
orbit-migrate run \
  --from-version 0.2.0 \
  --to-version 0.3.0 \
  --data-dir /var/lib/orbit-data \
  --backup-dir /backup/pre-migration \
  --parallel-workers 4

# Monitor progress
tail -f /var/log/orbit-migrate.log
```

#### Step 3: Post-Migration Tasks

```bash
# Verify data
orbit-migrate verify \
  --data-dir /var/lib/orbit-data

# Start server with new version
systemctl start orbit-server

# Run smoke tests
orbit-cli test --protocol all
```

### Manual Migration (Breaking Changes)

For major version upgrades with breaking changes:

#### Step 1: Export All Data

```bash
# Export AQL data
orbit-export aql \
  --data-dir /var/lib/orbit-data/aql \
  --output /export/aql-export.jsonl \
  --format jsonl

# Export Cypher data
orbit-export cypher \
  --data-dir /var/lib/orbit-data/cypher \
  --output /export/cypher-export.cypher \
  --format cypher

# Export GraphRAG data
orbit-export graphrag \
  --data-dir /var/lib/orbit-data/graphrag \
  --output /export/graphrag-export.json \
  --format json \
  --include-embeddings
```

#### Step 2: Clean Installation

```bash
# Stop old server
systemctl stop orbit-server

# Move old data
mv /var/lib/orbit-data /var/lib/orbit-data.old

# Install new version
cargo build --release
cp target/release/orbit-server /usr/local/bin/

# Initialize new data directory
mkdir -p /var/lib/orbit-data
orbit-server init --data-dir /var/lib/orbit-data
```

#### Step 3: Import Data

```bash
# Import AQL data
orbit-import aql \
  --input /export/aql-export.jsonl \
  --data-dir /var/lib/orbit-data/aql \
  --batch-size 1000

# Import Cypher data
orbit-import cypher \
  --input /export/cypher-export.cypher \
  --data-dir /var/lib/orbit-data/cypher

# Import GraphRAG data
orbit-import graphrag \
  --input /export/graphrag-export.json \
  --data-dir /var/lib/orbit-data/graphrag \
  --recompute-embeddings  # If embedding format changed
```

---

## Cross-Protocol Migration

### AQL to Cypher Migration

Convert ArangoDB document/graph data to Neo4j property graph:

#### Use Case
- Migrating from document-centric to graph-centric model
- Leveraging Cypher's pattern matching capabilities

#### Migration Script

```bash
orbit-migrate convert \
  --from aql \
  --to cypher \
  --source-dir /var/lib/orbit-data/aql \
  --target-dir /var/lib/orbit-data/cypher \
  --mapping-file aql-to-cypher-mapping.yaml
```

#### Mapping Configuration

```yaml
# aql-to-cypher-mapping.yaml
collections:
  users:
    node_label: User
    property_mapping:
      _key: id
      name: name
      email: email
      age: age

  posts:
    node_label: Post
    property_mapping:
      _key: id
      title: title
      content: content
      author: authorId

edges:
  authored:
    relationship_type: AUTHORED
    from_collection: users
    to_collection: posts
    property_mapping:
      timestamp: createdAt
```

#### Post-Migration Validation

```cypher
// Verify node count
MATCH (u:User) RETURN count(u) as user_count;
MATCH (p:Post) RETURN count(p) as post_count;

// Verify relationships
MATCH (u:User)-[r:AUTHORED]->(p:Post) RETURN count(r) as authored_count;

// Sample data check
MATCH (u:User {id: "user123"})-[:AUTHORED]->(p:Post)
RETURN u.name, p.title LIMIT 10;
```

### Cypher to GraphRAG Migration

Convert property graph to knowledge graph with embeddings:

```bash
orbit-migrate convert \
  --from cypher \
  --to graphrag \
  --source-dir /var/lib/orbit-data/cypher \
  --target-dir /var/lib/orbit-data/graphrag \
  --embedding-model sentence-transformers/all-MiniLM-L6-v2 \
  --batch-size 100
```

#### Mapping Configuration

```yaml
# cypher-to-graphrag-mapping.yaml
nodes:
  Person:
    entity_type: PERSON
    text_field: name
    properties:
      - age
      - occupation
    confidence: 1.0

  Organization:
    entity_type: ORGANIZATION
    text_field: name
    properties:
      - industry
      - founded
    confidence: 1.0

relationships:
  WORKS_FOR:
    relationship_type: employment
    confidence_field: since
    extract_context: true
```

---

## External System Migration

### Importing from ArangoDB

#### Step 1: Export from ArangoDB

```bash
# Using arangodump
arangodump \
  --server.endpoint tcp://localhost:8529 \
  --server.database mydb \
  --output-directory /export/arango \
  --include-system-collections false
```

#### Step 2: Convert to Orbit-RS Format

```bash
# Convert ArangoDB dump to Orbit-RS format
orbit-import aql \
  --input /export/arango \
  --format arangodump \
  --data-dir /var/lib/orbit-data/aql \
  --preserve-keys \
  --batch-size 5000
```

#### Step 3: Verify Import

```bash
# Check collection counts
orbit-cli query aql "FOR c IN collections RETURN c" | jq

# Verify sample documents
orbit-cli query aql "FOR doc IN users LIMIT 10 RETURN doc"
```

### Importing from Neo4j

#### Step 1: Export from Neo4j

```bash
# Using neo4j-admin export
neo4j-admin database export neo4j \
  --to=/export/neo4j/graph.dump

# Or use Cypher export
cypher-shell < export-query.cypher > /export/neo4j/data.cypher
```

#### Step 2: Convert to Orbit-RS Format

```bash
# Import Neo4j dump
orbit-import cypher \
  --input /export/neo4j/data.cypher \
  --format cypher \
  --data-dir /var/lib/orbit-data/cypher \
  --batch-size 1000
```

### Importing from CSV/JSON

#### CSV Import (Tabular Data → AQL)

```bash
# Import CSV as documents
orbit-import aql \
  --input /data/users.csv \
  --format csv \
  --collection users \
  --data-dir /var/lib/orbit-data/aql \
  --csv-delimiter "," \
  --csv-header-row \
  --generate-keys
```

#### JSON Lines Import (Documents → AQL)

```bash
# Import JSONL documents
orbit-import aql \
  --input /data/documents.jsonl \
  --format jsonl \
  --collection documents \
  --data-dir /var/lib/orbit-data/aql \
  --key-field _id
```

#### Graph Import (Edge List → Cypher)

```bash
# Import nodes
orbit-import cypher \
  --input /data/nodes.csv \
  --format csv-nodes \
  --node-label Person \
  --id-field id \
  --data-dir /var/lib/orbit-data/cypher

# Import edges
orbit-import cypher \
  --input /data/edges.csv \
  --format csv-edges \
  --relationship-type KNOWS \
  --from-field source \
  --to-field target \
  --data-dir /var/lib/orbit-data/cypher
```

---

## Data Format Changes

### Schema Evolution

#### Adding New Fields

```rust
// Old schema
#[derive(Serialize, Deserialize)]
struct User {
    id: String,
    name: String,
}

// New schema (backward compatible)
#[derive(Serialize, Deserialize)]
struct User {
    id: String,
    name: String,
    #[serde(default)]
    email: Option<String>,  // New optional field
}
```

Migration not required - old data loads with `email: None`

#### Renaming Fields

```bash
# Use migration script
orbit-migrate transform \
  --protocol aql \
  --collection users \
  --operation rename-field \
  --from-field username \
  --to-field name
```

#### Changing Field Types

```bash
# Example: String → Integer
orbit-migrate transform \
  --protocol aql \
  --collection users \
  --operation change-type \
  --field age \
  --from-type string \
  --to-type integer \
  --conversion-function "parse_int"
```

### Embedding Model Changes

When upgrading embedding models:

```bash
# Re-compute all embeddings
orbit-migrate recompute-embeddings \
  --protocol graphrag \
  --kg-name my_knowledge_graph \
  --model sentence-transformers/all-mpnet-base-v2 \
  --batch-size 100 \
  --parallel-workers 4
```

---

## Migration Tools

### orbit-migrate CLI

```bash
# Validate migration
orbit-migrate validate --from-version X --to-version Y

# Dry run
orbit-migrate run --dry-run --show-changes

# Run migration
orbit-migrate run --backup-first --parallel-workers 4

# Verify migration
orbit-migrate verify --check-integrity --check-counts

# Rollback
orbit-migrate rollback --to-backup /backup/timestamp
```

### Custom Migration Scripts

Example custom migration for complex transformations:

```rust
// migrate-user-schema.rs
use orbit_server::protocols::aql::{AqlStorage, AqlDocument};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let storage = AqlStorage::new("/var/lib/orbit-data/aql");
    storage.initialize().await?;

    // Get all users
    let users = storage.get_all_documents("users").await?;

    for mut user in users {
        // Transform data
        if let Some(username) = user.data.remove("username") {
            user.data.insert("name".to_string(), username);
        }

        // Add default email if missing
        user.data.entry("email".to_string())
            .or_insert(AqlValue::Null);

        // Save updated document
        storage.store_document(user).await?;
    }

    println!("Migration completed: {} users updated", users.len());
    Ok(())
}
```

---

## Rollback Procedures

### Automatic Rollback

```bash
# Rollback to last backup (created automatically during migration)
orbit-migrate rollback --auto

# Rollback to specific backup
orbit-migrate rollback --backup-id 20250323-143022
```

### Manual Rollback

```bash
# Stop server
systemctl stop orbit-server

# Restore from backup
rm -rf /var/lib/orbit-data
cp -r /backup/orbit-data-20250323 /var/lib/orbit-data

# Restore old binary if needed
cp /usr/local/bin/orbit-server.v0.2.0 /usr/local/bin/orbit-server

# Start server
systemctl start orbit-server

# Verify rollback
orbit-cli verify --protocol all
```

### Partial Rollback

If only one protocol fails:

```bash
# Rollback specific protocol
orbit-migrate rollback \
  --protocol aql \
  --backup-dir /backup/aql-20250323

# Keep other protocols unchanged
# Verify
orbit-cli verify --protocol aql
```

---

## Migration Monitoring

### Progress Tracking

```bash
# Monitor migration progress
watch -n 1 'cat /var/log/orbit-migrate.log | grep Progress'

# Example output:
# [Progress] AQL: 45% (450,000/1,000,000 documents)
# [Progress] Cypher: 67% (670,000/1,000,000 nodes)
# [Progress] GraphRAG: 23% (23,000/100,000 embeddings)
```

### Performance Metrics

Key metrics to monitor during migration:

- **Migration speed**: Documents/nodes per second
- **Memory usage**: Should stay within limits
- **Disk I/O**: Watch for bottlenecks
- **Error rate**: Should be near zero
- **Estimated completion time**: Based on current rate

### Health Checks

```bash
# Check migration health
orbit-migrate health \
  --check-memory \
  --check-disk-space \
  --check-data-integrity

# Example output:
# ✓ Memory usage: 4.2 GB / 8 GB (52%)
# ✓ Disk space: 120 GB / 500 GB available
# ✓ Data integrity: All checksums valid
# ⚠ Warning: Migration slower than expected (ETA +30 min)
```

---

## Best Practices

### Planning

1. **Test on staging** - Always test migration on copy of production data
2. **Schedule maintenance window** - Communicate downtime to users
3. **Document changes** - Keep migration log for audit trail
4. **Verify checksums** - Compare data before/after migration

### Execution

1. **Incremental migration** - Migrate in batches if possible
2. **Monitor continuously** - Watch logs and metrics
3. **Validate frequently** - Check data integrity at each step
4. **Keep backups** - Don't delete backups until fully verified

### Troubleshooting

Common issues and solutions:

| Issue | Cause | Solution |
|-------|-------|----------|
| Out of memory | Large batch size | Reduce batch size, increase swap |
| Slow migration | Disk I/O bottleneck | Use SSD, increase workers |
| Data corruption | Power failure | Always backup first, use checksums |
| Type mismatch | Schema incompatibility | Use custom transform script |

---

## Migration Checklist

### Pre-Migration
- [ ] Backup all protocol data
- [ ] Test migration on staging
- [ ] Verify disk space (3x current size)
- [ ] Schedule maintenance window
- [ ] Notify users of downtime
- [ ] Prepare rollback plan

### During Migration
- [ ] Stop server gracefully
- [ ] Run migration with monitoring
- [ ] Watch for errors in logs
- [ ] Monitor system resources
- [ ] Verify progress periodically

### Post-Migration
- [ ] Verify data integrity
- [ ] Check data counts match
- [ ] Run smoke tests
- [ ] Start server with new version
- [ ] Monitor for issues (24-48 hours)
- [ ] Archive old backups (after verification)

---

## Support and Resources

- Migration tools: `orbit/tools/migrate/`

- Issue tracker: https://github.com/your-org/orbit-rs/issues
- Community forum: https://community.orbit-rs.dev

For complex migrations, consider consulting with the Orbit-RS team.
