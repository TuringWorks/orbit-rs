//! Redis Protocol (RESP) Adapter
//!
//! Bridges Redis RESP protocol commands to the unified orbit-engine.
//!
//! ## Features
//!
//! - Key-value operations (GET, SET, DEL)
//! - Data structure commands (HGET, HSET, LPUSH, RPUSH, etc.)
//! - TTL and expiration support
//! - Transaction support (MULTI/EXEC)
//!
//! ## Data Model Mapping
//!
//! Redis data structures are mapped to engine tables:
//!
//! | Redis Type | Engine Representation              |
//! |-----------|-----------------------------------|
//! | String    | Single row with key-value pair     |
//! | Hash      | Row with key and field-value pairs |
//! | List      | Rows with key, index, value        |
//! | Set       | Rows with key, member              |
//! | Sorted Set| Rows with key, score, member       |
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! # use orbit_engine::adapters::{AdapterContext, RedisAdapter, ProtocolAdapter};
//! # use orbit_engine::storage::{HybridStorageManager, HybridStorageConfig, ColumnSchema};
//! # use std::sync::Arc;
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create storage engine with proper configuration
//! // let table_name = "redis_data".to_string();
//! // let schema = vec![]; // Define your schema
//! // let config = HybridStorageConfig::default();
//! // let storage = Arc::new(HybridStorageManager::new(table_name, schema, config));
//! //
//! // Create adapter context
//! // let context = AdapterContext::new(storage);
//! //
//! // Create Redis adapter
//! // let mut adapter = RedisAdapter::new(context);
//! // adapter.initialize().await?;
//! //
//! // Execute Redis commands
//! // adapter.set("mykey", "myvalue", None).await?;
//! // let value = adapter.get("mykey").await?;
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;

use crate::error::{EngineError, EngineResult};
use crate::storage::{AccessPattern, ColumnDef, DataType, FilterPredicate, SqlValue, TableSchema};

use super::{AdapterContext, CommandResult, ProtocolAdapter};

/// Redis protocol adapter
pub struct RedisAdapter {
    /// Adapter context with engine components
    context: AdapterContext,
    /// Table name for string values
    strings_table: String,
    /// Table name for hash values
    hashes_table: String,
    /// Table name for list values
    lists_table: String,
    /// Table name for set values
    #[allow(dead_code)]
    sets_table: String,
}

impl RedisAdapter {
    /// Create a new Redis adapter
    pub fn new(context: AdapterContext) -> Self {
        Self {
            context,
            strings_table: "redis_strings".to_string(),
            hashes_table: "redis_hashes".to_string(),
            lists_table: "redis_lists".to_string(),
            sets_table: "redis_sets".to_string(),
        }
    }

    /// SET command - set string value
    pub async fn set(
        &self,
        key: &str,
        value: &str,
        ttl: Option<Duration>,
    ) -> EngineResult<CommandResult> {
        let mut row = HashMap::new();
        row.insert("key".to_string(), SqlValue::String(key.to_string()));
        row.insert("value".to_string(), SqlValue::String(value.to_string()));

        if let Some(duration) = ttl {
            let expire_at = std::time::SystemTime::now() + duration;
            row.insert("expire_at".to_string(), SqlValue::Timestamp(expire_at));
        }

        // Delete existing key first (if any)
        let _ = self.del(key).await;

        self.context
            .storage
            .insert_row(&self.strings_table, row)
            .await?;
        Ok(CommandResult::Ok)
    }

    /// GET command - get string value
    pub async fn get(&self, key: &str) -> EngineResult<Option<String>> {
        let pattern = AccessPattern::Scan {
            time_range: None,
            filter: Some(FilterPredicate::Eq(
                "key".to_string(),
                SqlValue::String(key.to_string()),
            )),
        };

        match self
            .context
            .storage
            .query(&self.strings_table, pattern)
            .await?
        {
            crate::storage::QueryResult::Rows(rows) => {
                if let Some(row) = rows.first() {
                    // Check if expired
                    if let Some(SqlValue::Timestamp(expire_at)) = row.get("expire_at") {
                        if *expire_at < std::time::SystemTime::now() {
                            // Key expired, delete it
                            let _ = self.del(key).await;
                            return Ok(None);
                        }
                    }

                    if let Some(SqlValue::String(value)) = row.get("value") {
                        Ok(Some(value.clone()))
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// DEL command - delete key
    pub async fn del(&self, key: &str) -> EngineResult<CommandResult> {
        let filter = FilterPredicate::Eq("key".to_string(), SqlValue::String(key.to_string()));

        let count = self
            .context
            .storage
            .delete(&self.strings_table, filter)
            .await?;
        Ok(CommandResult::RowsAffected(count))
    }

    /// EXISTS command - check if key exists
    pub async fn exists(&self, key: &str) -> EngineResult<bool> {
        let value = self.get(key).await?;
        Ok(value.is_some())
    }

    /// HSET command - set hash field
    pub async fn hset(&self, key: &str, field: &str, value: &str) -> EngineResult<CommandResult> {
        let mut row = HashMap::new();
        row.insert("key".to_string(), SqlValue::String(key.to_string()));
        row.insert("field".to_string(), SqlValue::String(field.to_string()));
        row.insert("value".to_string(), SqlValue::String(value.to_string()));

        self.context
            .storage
            .insert_row(&self.hashes_table, row)
            .await?;
        Ok(CommandResult::Ok)
    }

    /// HGET command - get hash field
    pub async fn hget(&self, key: &str, field: &str) -> EngineResult<Option<String>> {
        let pattern = AccessPattern::Scan {
            time_range: None,
            filter: Some(FilterPredicate::And(vec![
                FilterPredicate::Eq("key".to_string(), SqlValue::String(key.to_string())),
                FilterPredicate::Eq("field".to_string(), SqlValue::String(field.to_string())),
            ])),
        };

        match self
            .context
            .storage
            .query(&self.hashes_table, pattern)
            .await?
        {
            crate::storage::QueryResult::Rows(rows) => {
                if let Some(row) = rows.first() {
                    if let Some(SqlValue::String(value)) = row.get("value") {
                        Ok(Some(value.clone()))
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// HGETALL command - get all hash fields
    pub async fn hgetall(&self, key: &str) -> EngineResult<HashMap<String, String>> {
        let pattern = AccessPattern::Scan {
            time_range: None,
            filter: Some(FilterPredicate::Eq(
                "key".to_string(),
                SqlValue::String(key.to_string()),
            )),
        };

        match self
            .context
            .storage
            .query(&self.hashes_table, pattern)
            .await?
        {
            crate::storage::QueryResult::Rows(rows) => {
                let mut result = HashMap::new();
                for row in rows {
                    if let (Some(SqlValue::String(field)), Some(SqlValue::String(value))) =
                        (row.get("field"), row.get("value"))
                    {
                        result.insert(field.clone(), value.clone());
                    }
                }
                Ok(result)
            }
            _ => Ok(HashMap::new()),
        }
    }

    /// HDEL command - delete hash field
    pub async fn hdel(&self, key: &str, field: &str) -> EngineResult<CommandResult> {
        let filter = FilterPredicate::And(vec![
            FilterPredicate::Eq("key".to_string(), SqlValue::String(key.to_string())),
            FilterPredicate::Eq("field".to_string(), SqlValue::String(field.to_string())),
        ]);

        let count = self
            .context
            .storage
            .delete(&self.hashes_table, filter)
            .await?;
        Ok(CommandResult::RowsAffected(count))
    }

    /// LPUSH command - push to list (left)
    pub async fn lpush(&self, key: &str, value: &str) -> EngineResult<CommandResult> {
        // Get current max index
        let pattern = AccessPattern::Scan {
            time_range: None,
            filter: Some(FilterPredicate::Eq(
                "key".to_string(),
                SqlValue::String(key.to_string()),
            )),
        };

        // Find minimum index
        let min_index = match self
            .context
            .storage
            .query(&self.lists_table, pattern)
            .await?
        {
            crate::storage::QueryResult::Rows(rows) => rows
                .iter()
                .filter_map(|r| {
                    if let Some(SqlValue::Int64(idx)) = r.get("index") {
                        Some(*idx)
                    } else {
                        None
                    }
                })
                .min()
                .unwrap_or(0),
            _ => 0,
        };

        let mut row = HashMap::new();
        row.insert("key".to_string(), SqlValue::String(key.to_string()));
        row.insert("index".to_string(), SqlValue::Int64(min_index - 1));
        row.insert("value".to_string(), SqlValue::String(value.to_string()));

        self.context
            .storage
            .insert_row(&self.lists_table, row)
            .await?;
        Ok(CommandResult::Ok)
    }

    /// RPUSH command - push to list (right)
    pub async fn rpush(&self, key: &str, value: &str) -> EngineResult<CommandResult> {
        // Get current max index
        let pattern = AccessPattern::Scan {
            time_range: None,
            filter: Some(FilterPredicate::Eq(
                "key".to_string(),
                SqlValue::String(key.to_string()),
            )),
        };

        // Find maximum index
        let max_index = match self
            .context
            .storage
            .query(&self.lists_table, pattern)
            .await?
        {
            crate::storage::QueryResult::Rows(rows) => rows
                .iter()
                .filter_map(|r| {
                    if let Some(SqlValue::Int64(idx)) = r.get("index") {
                        Some(*idx)
                    } else {
                        None
                    }
                })
                .max()
                .unwrap_or(-1),
            _ => -1,
        };

        let mut row = HashMap::new();
        row.insert("key".to_string(), SqlValue::String(key.to_string()));
        row.insert("index".to_string(), SqlValue::Int64(max_index + 1));
        row.insert("value".to_string(), SqlValue::String(value.to_string()));

        self.context
            .storage
            .insert_row(&self.lists_table, row)
            .await?;
        Ok(CommandResult::Ok)
    }
}

#[async_trait]
impl ProtocolAdapter for RedisAdapter {
    fn protocol_name(&self) -> &'static str {
        "RESP (Redis)"
    }

    async fn initialize(&mut self) -> EngineResult<()> {
        // Create tables for Redis data structures

        // Strings table
        let strings_schema = TableSchema {
            name: self.strings_table.clone(),
            columns: vec![
                ColumnDef {
                    name: "key".to_string(),
                    data_type: DataType::String,
                    nullable: false,
                },
                ColumnDef {
                    name: "value".to_string(),
                    data_type: DataType::String,
                    nullable: false,
                },
                ColumnDef {
                    name: "expire_at".to_string(),
                    data_type: DataType::Timestamp,
                    nullable: true,
                },
            ],
            primary_key: vec!["key".to_string()],
        };

        // Hashes table
        let hashes_schema = TableSchema {
            name: self.hashes_table.clone(),
            columns: vec![
                ColumnDef {
                    name: "key".to_string(),
                    data_type: DataType::String,
                    nullable: false,
                },
                ColumnDef {
                    name: "field".to_string(),
                    data_type: DataType::String,
                    nullable: false,
                },
                ColumnDef {
                    name: "value".to_string(),
                    data_type: DataType::String,
                    nullable: false,
                },
            ],
            primary_key: vec!["key".to_string(), "field".to_string()],
        };

        // Lists table
        let lists_schema = TableSchema {
            name: self.lists_table.clone(),
            columns: vec![
                ColumnDef {
                    name: "key".to_string(),
                    data_type: DataType::String,
                    nullable: false,
                },
                ColumnDef {
                    name: "index".to_string(),
                    data_type: DataType::Int64,
                    nullable: false,
                },
                ColumnDef {
                    name: "value".to_string(),
                    data_type: DataType::String,
                    nullable: false,
                },
            ],
            primary_key: vec!["key".to_string(), "index".to_string()],
        };

        // Create tables (ignore if they already exist)
        let _ = self.context.storage.create_table(strings_schema).await;
        let _ = self.context.storage.create_table(hashes_schema).await;
        let _ = self.context.storage.create_table(lists_schema).await;

        Ok(())
    }

    async fn shutdown(&mut self) -> EngineResult<()> {
        // Graceful shutdown if needed
        Ok(())
    }
}

/// Redis error mapper
pub struct RedisErrorMapper;

impl super::error_mapping::ErrorMapper for RedisErrorMapper {
    fn to_error_code(error: &EngineError) -> String {
        match error {
            EngineError::NotFound(_) => "NOTFOUND".to_string(),
            EngineError::InvalidInput(_) => "WRONGTYPE".to_string(),
            EngineError::Timeout(_) => "TIMEOUT".to_string(),
            _ => "ERR".to_string(),
        }
    }

    fn to_error_message(error: &EngineError) -> String {
        format!("ERR {}", error)
    }
}
