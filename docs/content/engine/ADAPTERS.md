# Protocol Adapter Development Guide

This guide explains how to develop custom protocol adapters for Orbit Engine.

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Core Concepts](#core-concepts)
- [Creating an Adapter](#creating-an-adapter)
- [Type Mapping](#type-mapping)
- [Error Handling](#error-handling)
- [Transactions](#transactions)
- [Testing](#testing)
- [Examples](#examples)

## Overview

Protocol adapters bridge protocol-specific requests to Orbit Engine's unified storage, transaction, and query APIs. This allows Orbit Engine to support multiple database protocols (PostgreSQL, Redis, MongoDB, Cassandra, etc.) using the same underlying storage engine.

### Why Protocol Adapters?

**Problem**: Different database protocols have:

- Different data types (PostgreSQL SMALLINT vs Redis strings)
- Different command semantics (SQL INSERT vs Redis SET)
- Different transaction models (ACID vs eventual consistency)
- Different error codes and messages

**Solution**: Protocol adapters translate between protocol-specific APIs and Orbit Engine's unified internal APIs.

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│              Client Applications                            │
│   psql │ redis-cli │ mongo shell │ cqlsh │ custom           │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│              Wire Protocols (orbit/protocols)               │
│   PostgreSQL │ RESP │ MongoDB │ CQL │ OrbitQL               │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│           Protocol Adapters (orbit/engine/adapters)         │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐             │
│  │ PostgreSQL │  │   Redis    │  │  MongoDB   │  ...        │
│  │  Adapter   │  │  Adapter   │  │  Adapter   │             │
│  └────────────┘  └────────────┘  └────────────┘             │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                    Orbit Engine Core                        │
│  Storage | Transactions | Clustering | Query Execution      │
└─────────────────────────────────────────────────────────────┘
```

## Core Concepts

### 1. AdapterContext

Provides access to engine components:

```rust
pub struct AdapterContext {
    /// Storage engine reference
    pub storage: Arc<dyn TableStorage>,
    /// Transaction manager reference (optional)
    pub transaction_manager: Option<Arc<dyn TransactionManager>>,
}
```

### 2. ProtocolAdapter Trait

All adapters implement this base trait:

```rust
#[async_trait]
pub trait ProtocolAdapter: Send + Sync {
    /// Protocol name (e.g., "PostgreSQL", "RESP", "MongoDB")
    fn protocol_name(&self) -> &'static str;

    /// Initialize the adapter
    async fn initialize(&mut self) -> EngineResult<()>;

    /// Shutdown the adapter gracefully
    async fn shutdown(&mut self) -> EngineResult<()>;
}
```

### 3. CommandResult

Unified result type for all operations:

```rust
pub enum CommandResult {
    /// Rows returned from query
    Rows(Vec<Row>),
    /// Number of rows affected by write operation
    RowsAffected(usize),
    /// Simple OK response
    Ok,
    /// Error response
    Error(String),
    /// Custom response data
    Custom(Vec<u8>),
}
```

## Creating an Adapter

### Step 1: Define Your Adapter Struct

```rust
use orbit_engine::adapters::{AdapterContext, ProtocolAdapter, CommandResult};
use orbit_engine::error::{EngineError, EngineResult};

pub struct MongoDbAdapter {
    context: AdapterContext,
    // Adapter-specific state
    database_name: String,
}

impl MongoDbAdapter {
    pub fn new(context: AdapterContext) -> Self {
        Self {
            context,
            database_name: "default".to_string(),
        }
    }
}
```

### Step 2: Implement ProtocolAdapter

```rust
#[async_trait]
impl ProtocolAdapter for MongoDbAdapter {
    fn protocol_name(&self) -> &'static str {
        "MongoDB"
    }

    async fn initialize(&mut self) -> EngineResult<()> {
        // Create default collections if needed
        Ok(())
    }

    async fn shutdown(&mut self) -> EngineResult<()> {
        // Cleanup resources
        Ok(())
    }
}
```

### Step 3: Implement Protocol-Specific Operations

```rust
impl MongoDbAdapter {
    /// MongoDB insertOne operation
    pub async fn insert_one(
        &self,
        collection: &str,
        document: BsonDocument,
    ) -> EngineResult<CommandResult> {
        // Convert BSON to Row
        let row = self.bson_to_row(document)?;

        // Insert via storage engine
        self.context.storage.insert_row(collection, row).await?;

        Ok(CommandResult::RowsAffected(1))
    }

    /// MongoDB find operation
    pub async fn find(
        &self,
        collection: &str,
        filter: Option<BsonDocument>,
    ) -> EngineResult<CommandResult> {
        // Convert BSON filter to FilterPredicate
        let predicate = filter
            .map(|f| self.bson_filter_to_predicate(f))
            .transpose()?;

        // Query via storage engine
        let pattern = AccessPattern::Scan {
            filter: predicate,
            limit: None,
        };

        let result = self.context.storage.query(collection, pattern).await?;

        // Convert rows to BSON documents
        let documents: Vec<Row> = result.rows;

        Ok(CommandResult::Rows(documents))
    }
}
```

## Type Mapping

### Example: MongoDB BSON to SqlValue

```rust
use bson::{Bson, Document as BsonDocument};
use orbit_engine::storage::SqlValue;
use std::collections::HashMap;

impl MongoDbAdapter {
    fn bson_to_sql_value(&self, bson: &Bson) -> EngineResult<SqlValue> {
        match bson {
            Bson::Null => Ok(SqlValue::Null),
            Bson::Boolean(b) => Ok(SqlValue::Boolean(*b)),
            Bson::Int32(i) => Ok(SqlValue::Int32(*i)),
            Bson::Int64(i) => Ok(SqlValue::Int64(*i)),
            Bson::Double(f) => Ok(SqlValue::Float64(*f)),
            Bson::String(s) => Ok(SqlValue::String(s.clone())),
            Bson::Binary(bin) => Ok(SqlValue::Binary(bin.bytes.clone())),
            Bson::DateTime(dt) => {
                let timestamp = std::time::SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_millis(dt.timestamp_millis() as u64);
                Ok(SqlValue::Timestamp(timestamp))
            }
            Bson::Document(doc) => {
                // Serialize nested documents as JSON
                let json = serde_json::to_string(doc)
                    .map_err(|e| EngineError::serialization(e.to_string()))?;
                Ok(SqlValue::String(json))
            }
            Bson::Array(arr) => {
                // Serialize arrays as JSON
                let json = serde_json::to_string(arr)
                    .map_err(|e| EngineError::serialization(e.to_string()))?;
                Ok(SqlValue::String(json))
            }
            _ => Err(EngineError::not_supported(format!(
                "BSON type {:?} not supported",
                bson
            ))),
        }
    }

    fn bson_to_row(&self, doc: BsonDocument) -> EngineResult<HashMap<String, SqlValue>> {
        let mut row = HashMap::new();
        for (key, value) in doc {
            row.insert(key, self.bson_to_sql_value(&value)?);
        }
        Ok(row)
    }

    fn sql_value_to_bson(&self, value: &SqlValue) -> EngineResult<Bson> {
        match value {
            SqlValue::Null => Ok(Bson::Null),
            SqlValue::Boolean(b) => Ok(Bson::Boolean(*b)),
            SqlValue::Int16(i) => Ok(Bson::Int32(*i as i32)),
            SqlValue::Int32(i) => Ok(Bson::Int32(*i)),
            SqlValue::Int64(i) => Ok(Bson::Int64(*i)),
            SqlValue::Float64(f) => Ok(Bson::Double(*f)),
            SqlValue::String(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => {
                Ok(Bson::String(s.clone()))
            }
            SqlValue::Binary(b) => {
                Ok(Bson::Binary(bson::Binary {
                    subtype: bson::spec::BinarySubtype::Generic,
                    bytes: b.clone(),
                }))
            }
            SqlValue::Timestamp(t) => {
                let duration = t.duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map_err(|e| EngineError::conversion(e.to_string()))?;
                Ok(Bson::DateTime(bson::DateTime::from_millis(
                    duration.as_millis() as i64
                )))
            }
            _ => Err(EngineError::not_supported(format!(
                "SqlValue {:?} not supported for BSON",
                value
            ))),
        }
    }
}
```

### Example: Cassandra CQL to SqlValue

```rust
impl CassandraAdapter {
    fn cql_type_to_data_type(&self, cql_type: &str) -> EngineResult<DataType> {
        match cql_type.to_lowercase().as_str() {
            "text" | "varchar" | "ascii" => Ok(DataType::String),
            "int" => Ok(DataType::Int32),
            "bigint" | "counter" => Ok(DataType::Int64),
            "float" => Ok(DataType::Float32),
            "double" => Ok(DataType::Float64),
            "boolean" => Ok(DataType::Boolean),
            "blob" => Ok(DataType::Binary),
            "timestamp" => Ok(DataType::Timestamp),
            "uuid" | "timeuuid" => Ok(DataType::String),  // Store as string
            _ => Err(EngineError::not_supported(format!(
                "CQL type '{}' not supported",
                cql_type
            ))),
        }
    }
}
```

## Error Handling

### Mapping Engine Errors to Protocol Errors

```rust
use orbit_engine::error::EngineError;

impl MongoDbAdapter {
    fn engine_error_to_mongo_error(&self, error: EngineError) -> i32 {
        match error {
            EngineError::NotFound(_) => 26,         // NamespaceNotFound
            EngineError::AlreadyExists(_) => 48,    // NamespaceExists
            EngineError::Conflict(_) => 11000,      // DuplicateKey
            EngineError::Timeout(_) => 50,          // ExceededTimeLimit
            EngineError::Transaction(_) => 251,     // TransactionTooLarge
            EngineError::Storage(_) => 1,           // InternalError
            _ => 1,                                 // InternalError
        }
    }

    fn to_mongo_error_response(&self, error: EngineError) -> BsonDocument {
        let code = self.engine_error_to_mongo_error(error.clone());
        bson::doc! {
            "ok": 0,
            "errmsg": error.to_string(),
            "code": code,
            "codeName": self.error_code_to_name(code),
        }
    }

    fn error_code_to_name(&self, code: i32) -> &'static str {
        match code {
            26 => "NamespaceNotFound",
            48 => "NamespaceExists",
            11000 => "DuplicateKey",
            50 => "ExceededTimeLimit",
            251 => "TransactionTooLarge",
            _ => "InternalError",
        }
    }
}
```

### PostgreSQL Error Mapping

```rust
impl PostgresAdapter {
    fn engine_error_to_pg_sqlstate(&self, error: &EngineError) -> &'static str {
        match error {
            EngineError::NotFound(_) => "42P01",           // undefined_table
            EngineError::AlreadyExists(_) => "42P07",      // duplicate_table
            EngineError::Conflict(_) => "23505",           // unique_violation
            EngineError::Timeout(_) => "57014",            // query_canceled
            EngineError::Transaction(_) => "40001",        // serialization_failure
            EngineError::Serialization(_) => "22P02",      // invalid_text_representation
            EngineError::InvalidInput(_) => "22000",       // data_exception
            _ => "XX000",                                  // internal_error
        }
    }
}
```

## Transactions

### Using TransactionAdapter

```rust
use orbit_engine::adapters::TransactionAdapter;
use orbit_engine::transaction::IsolationLevel;

impl MongoDbAdapter {
    pub async fn start_transaction(&mut self) -> EngineResult<String> {
        // Generate protocol-specific transaction ID
        let protocol_tx_id = uuid::Uuid::new_v4().to_string();

        // Begin engine transaction with snapshot isolation
        let tx_adapter = TransactionAdapter::new(self.context.clone());
        tx_adapter
            .begin(protocol_tx_id.clone(), IsolationLevel::RepeatableRead)
            .await?;

        Ok(protocol_tx_id)
    }

    pub async fn commit_transaction(&mut self, tx_id: &str) -> EngineResult<()> {
        let mut tx_adapter = TransactionAdapter::new(self.context.clone());
        tx_adapter.commit(tx_id).await
    }

    pub async fn abort_transaction(&mut self, tx_id: &str) -> EngineResult<()> {
        let mut tx_adapter = TransactionAdapter::new(self.context.clone());
        tx_adapter.rollback(tx_id).await
    }
}
```

### Example: MongoDB Multi-Document Transaction

```rust
impl MongoDbAdapter {
    pub async fn transaction_workflow(
        &mut self,
        operations: Vec<MongoOperation>,
    ) -> EngineResult<CommandResult> {
        // Start transaction
        let tx_id = self.start_transaction().await?;

        // Execute operations
        for op in operations {
            let result = match op {
                MongoOperation::InsertOne { collection, document } => {
                    self.insert_one(&collection, document).await
                }
                MongoOperation::UpdateOne { collection, filter, update } => {
                    self.update_one(&collection, filter, update).await
                }
                MongoOperation::DeleteOne { collection, filter } => {
                    self.delete_one(&collection, filter).await
                }
            };

            // Abort on error
            if let Err(e) = result {
                self.abort_transaction(&tx_id).await?;
                return Err(e);
            }
        }

        // Commit transaction
        self.commit_transaction(&tx_id).await?;

        Ok(CommandResult::Ok)
    }
}
```

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use orbit_engine::storage::HybridStorageManager;

    #[tokio::test]
    async fn test_mongo_insert_and_find() {
        // Create in-memory storage
        let storage = Arc::new(HybridStorageManager::new_in_memory());
        let context = AdapterContext::new(
            storage as Arc<dyn orbit_engine::storage::TableStorage>
        );

        let mut adapter = MongoDbAdapter::new(context);
        adapter.initialize().await.unwrap();

        // Insert document
        let doc = bson::doc! {
            "_id": "user123",
            "name": "Alice",
            "age": 30,
        };

        adapter.insert_one("users", doc.clone()).await.unwrap();

        // Find document
        let filter = bson::doc! { "_id": "user123" };
        let result = adapter.find("users", Some(filter)).await.unwrap();

        match result {
            CommandResult::Rows(rows) => {
                assert_eq!(rows.len(), 1);
                let row = &rows[0];
                assert_eq!(
                    row.get("name"),
                    Some(&SqlValue::String("Alice".to_string()))
                );
            }
            _ => panic!("Expected Rows"),
        }
    }

    #[tokio::test]
    async fn test_mongo_transaction() {
        let storage = Arc::new(HybridStorageManager::new_in_memory());
        let context = AdapterContext::new(
            storage as Arc<dyn orbit_engine::storage::TableStorage>
        );

        let mut adapter = MongoDbAdapter::new(context);
        adapter.initialize().await.unwrap();

        // Start transaction
        let tx_id = adapter.start_transaction().await.unwrap();

        // Insert in transaction
        let doc = bson::doc! { "_id": "tx-test", "value": 100 };
        adapter.insert_one("accounts", doc).await.unwrap();

        // Commit
        adapter.commit_transaction(&tx_id).await.unwrap();

        // Verify inserted
        let result = adapter.find("accounts", None).await.unwrap();
        match result {
            CommandResult::Rows(rows) => assert_eq!(rows.len(), 1),
            _ => panic!("Expected Rows"),
        }
    }
}
```

### Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_mongo_with_tiered_storage() {
        // Create storage with all tiers
        let backend = StorageBackend::S3(S3Config {
            endpoint: "http://localhost:9000".to_string(),
            region: "us-east-1".to_string(),
            bucket: "test-bucket".to_string(),
            access_key: "minioadmin".to_string(),
            secret_key: "minioadmin".to_string(),
        });

        let storage = Arc::new(
            HybridStorageManager::with_cold_tier(backend).await.unwrap()
        );

        let context = AdapterContext::new(
            storage as Arc<dyn orbit_engine::storage::TableStorage>
        );

        let mut adapter = MongoDbAdapter::new(context);
        adapter.initialize().await.unwrap();

        // Insert 1000 documents
        for i in 0..1000 {
            let doc = bson::doc! {
                "_id": format!("doc-{}", i),
                "value": i,
                "timestamp": bson::DateTime::now(),
            };
            adapter.insert_one("test_collection", doc).await.unwrap();
        }

        // Query all
        let result = adapter.find("test_collection", None).await.unwrap();
        match result {
            CommandResult::Rows(rows) => assert_eq!(rows.len(), 1000),
            _ => panic!("Expected Rows"),
        }
    }
}
```

## Examples

### Complete MongoDB Adapter



### Complete Cassandra Adapter



### GraphQL Adapter

```rust
use orbit_engine::adapters::{AdapterContext, ProtocolAdapter, CommandResult};

pub struct GraphQLAdapter {
    context: AdapterContext,
    schema: GraphQLSchema,
}

impl GraphQLAdapter {
    pub async fn execute_query(&self, query: &str) -> EngineResult<CommandResult> {
        // Parse GraphQL query
        let parsed = self.parse_query(query)?;

        // Convert to storage operations
        match parsed.operation {
            GraphQLOperation::Query { selection } => {
                self.execute_selection(selection).await
            }
            GraphQLOperation::Mutation { selection } => {
                self.execute_mutation(selection).await
            }
        }
    }
}
```

## Best Practices

### 1. Type Safety

Always validate protocol types before conversion:

```rust
fn validate_bson_document(&self, doc: &BsonDocument) -> EngineResult<()> {
    // Ensure required fields exist
    if !doc.contains_key("_id") {
        return Err(EngineError::invalid_input("Missing _id field"));
    }
    Ok(())
}
```

### 2. Efficient Batch Operations

Use engine batch operations for bulk inserts:

```rust
pub async fn insert_many(
    &self,
    collection: &str,
    documents: Vec<BsonDocument>,
) -> EngineResult<CommandResult> {
    let rows: Vec<Row> = documents
        .into_iter()
        .map(|doc| self.bson_to_row(doc))
        .collect::<EngineResult<Vec<_>>>()?;

    // Batch insert
    for row in rows {
        self.context.storage.insert_row(collection, row).await?;
    }

    Ok(CommandResult::RowsAffected(documents.len()))
}
```

### 3. Resource Cleanup

Always implement proper shutdown:

```rust
async fn shutdown(&mut self) -> EngineResult<()> {
    // Close active transactions
    for tx_id in &self.active_transactions {
        let _ = self.abort_transaction(tx_id).await;
    }

    // Release resources
    self.active_transactions.clear();

    Ok(())
}
```

### 4. Error Context

Provide helpful error messages:

```rust
Err(EngineError::invalid_input(format!(
    "Invalid BSON type for field '{}': expected String, got {:?}",
    field_name, bson_value
)))
```

## Checklist

When creating a new adapter:

- [ ] Define adapter struct with `AdapterContext`
- [ ] Implement `ProtocolAdapter` trait
- [ ] Implement protocol-specific operations
- [ ] Create type mapping functions (protocol types ↔ SqlValue)
- [ ] Implement error mapping (EngineError → protocol errors)
- [ ] Add transaction support if protocol supports it
- [ ] Write unit tests
- [ ] Write integration tests
- [ ] Add documentation and examples
- [ ] Consider batch operations for performance
- [ ] Implement proper resource cleanup

## Next Steps


- See [ARCHITECTURE.md](ARCHITECTURE.md) for engine internals
- See [DEPLOYMENT.md](DEPLOYMENT.md) for production deployment

---

**Questions?** Open an issue at <https://github.com/orbit-rs/orbit/issues>
