//! PostgreSQL Actor Implementation
//!
//! This module provides the concrete implementation of the PostgreSQL database actor
//! with full SQL support, connection pooling, transaction management, and performance optimization.

use super::*;
use crate::database::{utils, DatabaseConfig};
use async_trait::async_trait;
use orbit_shared::addressable::{Addressable, AddressableReference, Key};
use orbit_shared::exception::{OrbitError, OrbitResult};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

// Mock PostgreSQL connection pool for demonstration
// In a real implementation, you would use sqlx::PgPool or similar
#[derive(Debug, Clone)]
pub struct MockConnectionPool {
    pub is_connected: bool,
    pub connection_count: u32,
    pub max_connections: u32,
}

impl MockConnectionPool {
    pub fn new(max_connections: u32) -> Self {
        Self {
            is_connected: false,
            connection_count: 0,
            max_connections,
        }
    }

    pub async fn connect(&mut self, _config: &DatabaseConfig) -> Result<(), String> {
        // Simulate connection establishment
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        self.is_connected = true;
        self.connection_count = 1;
        Ok(())
    }

    pub async fn execute_query(
        &self,
        sql: &str,
        _params: &[JsonValue],
    ) -> Result<QueryResponse, String> {
        if !self.is_connected {
            return Err("Not connected to database".to_string());
        }

        // Simulate query execution
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Parse simple SQL commands
        let sql_upper = sql.trim().to_uppercase();

        if sql_upper.starts_with("SELECT") {
            if sql_upper.contains("SELECT 1") {
                Ok(QueryResponse {
                    columns: vec![ColumnInfo {
                        name: "?column?".to_string(),
                        data_type: "integer".to_string(),
                        nullable: false,
                        default_value: None,
                        is_primary_key: false,
                        ordinal_position: 1,
                    }],
                    rows: vec![vec![JsonValue::Number(1.into())]],
                    rows_affected: None,
                    execution_time_ms: 50,
                    query_plan: Some("Result  (cost=0.00..0.01 rows=1 width=4)".to_string()),
                    warnings: vec![],
                })
            } else if sql_upper.contains("SELECT VERSION()") {
                Ok(QueryResponse {
                    columns: vec![ColumnInfo {
                        name: "version".to_string(),
                        data_type: "text".to_string(),
                        nullable: false,
                        default_value: None,
                        is_primary_key: false,
                        ordinal_position: 1,
                    }],
                    rows: vec![vec![JsonValue::String(
                        "PostgreSQL 15.0 on Orbit-RS".to_string(),
                    )]],
                    rows_affected: None,
                    execution_time_ms: 50,
                    query_plan: None,
                    warnings: vec![],
                })
            } else {
                // Generic SELECT result
                Ok(QueryResponse {
                    columns: vec![
                        ColumnInfo {
                            name: "id".to_string(),
                            data_type: "integer".to_string(),
                            nullable: false,
                            default_value: None,
                            is_primary_key: true,
                            ordinal_position: 1,
                        },
                        ColumnInfo {
                            name: "name".to_string(),
                            data_type: "text".to_string(),
                            nullable: true,
                            default_value: None,
                            is_primary_key: false,
                            ordinal_position: 2,
                        },
                    ],
                    rows: vec![
                        vec![
                            JsonValue::Number(1.into()),
                            JsonValue::String("Test User 1".to_string()),
                        ],
                        vec![
                            JsonValue::Number(2.into()),
                            JsonValue::String("Test User 2".to_string()),
                        ],
                    ],
                    rows_affected: None,
                    execution_time_ms: 50,
                    query_plan: Some(
                        "Seq Scan on users  (cost=0.00..25.50 rows=1550 width=36)".to_string(),
                    ),
                    warnings: vec![],
                })
            }
        } else if sql_upper.starts_with("INSERT") {
            Ok(QueryResponse {
                columns: vec![],
                rows: vec![],
                rows_affected: Some(1),
                execution_time_ms: 30,
                query_plan: None,
                warnings: vec![],
            })
        } else if sql_upper.starts_with("UPDATE") {
            Ok(QueryResponse {
                columns: vec![],
                rows: vec![],
                rows_affected: Some(1),
                execution_time_ms: 40,
                query_plan: None,
                warnings: vec![],
            })
        } else if sql_upper.starts_with("DELETE") {
            Ok(QueryResponse {
                columns: vec![],
                rows: vec![],
                rows_affected: Some(1),
                execution_time_ms: 35,
                query_plan: None,
                warnings: vec![],
            })
        } else if sql_upper.starts_with("CREATE") {
            Ok(QueryResponse {
                columns: vec![],
                rows: vec![],
                rows_affected: None,
                execution_time_ms: 100,
                query_plan: None,
                warnings: vec![],
            })
        } else if sql_upper.starts_with("DROP") {
            Ok(QueryResponse {
                columns: vec![],
                rows: vec![],
                rows_affected: None,
                execution_time_ms: 80,
                query_plan: None,
                warnings: vec![],
            })
        } else {
            Err(format!("Unsupported SQL command: {}", sql))
        }
    }

    pub async fn begin_transaction(&self) -> Result<String, String> {
        if !self.is_connected {
            return Err("Not connected to database".to_string());
        }
        Ok(utils::generate_transaction_id())
    }

    pub async fn commit_transaction(&self, _tx_id: &str) -> Result<(), String> {
        if !self.is_connected {
            return Err("Not connected to database".to_string());
        }
        Ok(())
    }

    pub async fn rollback_transaction(&self, _tx_id: &str) -> Result<(), String> {
        if !self.is_connected {
            return Err("Not connected to database".to_string());
        }
        Ok(())
    }

    pub async fn close(&mut self) {
        self.is_connected = false;
        self.connection_count = 0;
    }
}

/// PostgreSQL Actor implementation
pub struct PostgresActor {
    /// Base actor state
    pub state: BaseActorState,
    /// Database connection pool
    pub connection_pool: Arc<Mutex<MockConnectionPool>>,
    /// Actor reference for addressability
    pub reference: AddressableReference,
    /// Performance statistics
    pub stats: Arc<RwLock<PostgresStats>>,
}

/// PostgreSQL-specific statistics
#[derive(Debug, Clone, Default)]
pub struct PostgresStats {
    pub total_queries: u64,
    pub successful_queries: u64,
    pub failed_queries: u64,
    pub total_query_time_ms: u64,
    pub connection_attempts: u64,
    pub successful_connections: u64,
    pub connection_failures: u64,
    pub active_transactions: u32,
    pub prepared_statements_count: u32,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

impl PostgresActor {
    /// Create a new PostgreSQL actor
    pub fn new(actor_id: String) -> Self {
        let config = DatabaseConfig::default();
        let state = BaseActorState::new(actor_id.clone(), config.clone());

        let reference = AddressableReference {
            addressable_type: "PostgresActor".to_string(),
            key: Key::StringKey { key: actor_id },
        };

        Self {
            state,
            connection_pool: Arc::new(Mutex::new(MockConnectionPool::new(
                config.pool.max_connections,
            ))),
            reference,
            stats: Arc::new(RwLock::new(PostgresStats::default())),
        }
    }

    /// Update PostgreSQL-specific statistics
    async fn update_stats<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut PostgresStats),
    {
        let mut stats = self.stats.write().await;
        update_fn(&mut *stats);
    }

    /// Handle schema operations specific to PostgreSQL
    async fn handle_schema_request(
        &mut self,
        request: SchemaRequest,
    ) -> OrbitResult<SchemaResponse> {
        match request {
            SchemaRequest::CreateTable {
                table_definition,
                if_not_exists,
            } => {
                let sql = self.build_create_table_sql(&table_definition, if_not_exists)?;
                let _result = self.execute_sql(&sql, vec![]).await?;

                Ok(SchemaResponse::TableCreated {
                    table_name: table_definition.name,
                    columns: table_definition.columns,
                })
            }
            SchemaRequest::DropTable {
                table_name,
                if_exists,
                cascade,
            } => {
                let mut sql = format!("DROP TABLE {}", if if_exists { "IF EXISTS " } else { "" });
                sql.push_str(&table_name);
                if cascade {
                    sql.push_str(" CASCADE");
                }

                let _result = self.execute_sql(&sql, vec![]).await?;

                Ok(SchemaResponse::TableDropped { table_name })
            }
            SchemaRequest::CreateIndex {
                index_definition,
                if_not_exists,
            } => {
                let sql = self.build_create_index_sql(&index_definition, if_not_exists)?;
                let _result = self.execute_sql(&sql, vec![]).await?;

                Ok(SchemaResponse::IndexCreated {
                    index_name: index_definition.name,
                    table_name: index_definition.table_name,
                    index_type: format!("{:?}", index_definition.index_type),
                })
            }
            SchemaRequest::CreateExtension {
                extension_name,
                if_not_exists,
                schema,
                version,
            } => {
                let mut sql = "CREATE EXTENSION ".to_string();
                if if_not_exists {
                    sql.push_str("IF NOT EXISTS ");
                }
                sql.push_str(&extension_name);

                if let Some(schema) = schema {
                    sql.push_str(" SCHEMA ");
                    sql.push_str(&schema);
                }

                let version_clone = version.clone();
                if let Some(version) = version {
                    sql.push_str(" VERSION ");
                    sql.push_str(&version);
                }

                let _result = self.execute_sql(&sql, vec![]).await?;

                Ok(SchemaResponse::ExtensionCreated {
                    extension_name,
                    version: version_clone.unwrap_or("latest".to_string()),
                })
            }
            SchemaRequest::GetTableInfo { table_name } => {
                // Query table information
                let sql = format!(
                    "SELECT column_name, data_type, is_nullable, column_default 
                     FROM information_schema.columns 
                     WHERE table_name = '{}' 
                     ORDER BY ordinal_position",
                    table_name
                );

                let result = self.execute_sql(&sql, vec![]).await?;

                let columns = result
                    .rows
                    .into_iter()
                    .enumerate()
                    .map(|(i, row)| {
                        ColumnInfo {
                            name: row
                                .get(0)
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string(),
                            data_type: row
                                .get(1)
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string(),
                            nullable: row.get(2).and_then(|v| v.as_str()).unwrap_or("YES") == "YES",
                            default_value: row
                                .get(3)
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            is_primary_key: false, // Would need separate query to determine this
                            ordinal_position: i as i32 + 1,
                        }
                    })
                    .collect();

                Ok(SchemaResponse::TableInfo {
                    table_info: TableInfo {
                        name: table_name.clone(),
                        schema: "public".to_string(),
                        columns,
                        constraints: vec![], // Would need separate queries
                        indexes: vec![],     // Would need separate queries
                        row_count: None,     // Would need COUNT query
                        size_bytes: None,    // Would need pg_total_relation_size
                        created_at: None,    // Not easily available in PostgreSQL
                    },
                })
            }
            SchemaRequest::ListTables { schema_name } => {
                let schema = schema_name.unwrap_or_else(|| "public".to_string());
                let sql = format!(
                    "SELECT table_name, table_type 
                     FROM information_schema.tables 
                     WHERE table_schema = '{}' 
                     ORDER BY table_name",
                    schema
                );

                let result = self.execute_sql(&sql, vec![]).await?;

                let tables = result
                    .rows
                    .into_iter()
                    .map(|row| TableSummary {
                        name: row
                            .get(0)
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        schema: schema.clone(),
                        table_type: row
                            .get(1)
                            .and_then(|v| v.as_str())
                            .unwrap_or("BASE TABLE")
                            .to_string(),
                        row_count: None,
                        size_bytes: None,
                    })
                    .collect();

                Ok(SchemaResponse::TableList { tables })
            }
            _ => Err(OrbitError::internal("Schema operation not yet implemented")),
        }
    }

    /// Build CREATE TABLE SQL statement
    fn build_create_table_sql(
        &self,
        table_def: &TableDefinition,
        if_not_exists: bool,
    ) -> OrbitResult<String> {
        let mut sql = "CREATE TABLE ".to_string();

        if if_not_exists {
            sql.push_str("IF NOT EXISTS ");
        }

        if let Some(schema) = &table_def.schema {
            sql.push_str(schema);
            sql.push('.');
        }

        sql.push_str(&table_def.name);
        sql.push_str(" (");

        // Add columns
        let column_definitions: Vec<String> = table_def
            .columns
            .iter()
            .map(|col| {
                let mut col_def = format!("{} {}", col.name, col.data_type);

                if !col.nullable {
                    col_def.push_str(" NOT NULL");
                }

                if let Some(default) = &col.default_value {
                    col_def.push_str(&format!(" DEFAULT {}", default));
                }

                // Add column constraints
                for constraint in &col.constraints {
                    match constraint {
                        ColumnConstraint::PrimaryKey => col_def.push_str(" PRIMARY KEY"),
                        ColumnConstraint::Unique => col_def.push_str(" UNIQUE"),
                        ColumnConstraint::Check { expression } => {
                            col_def.push_str(&format!(" CHECK ({})", expression));
                        }
                        ColumnConstraint::References {
                            table,
                            column,
                            on_delete,
                            on_update,
                        } => {
                            col_def.push_str(&format!(" REFERENCES {}({})", table, column));
                            if let Some(action) = on_delete {
                                col_def.push_str(&format!(
                                    " ON DELETE {}",
                                    self.referential_action_to_sql(action)
                                ));
                            }
                            if let Some(action) = on_update {
                                col_def.push_str(&format!(
                                    " ON UPDATE {}",
                                    self.referential_action_to_sql(action)
                                ));
                            }
                        }
                        _ => {}
                    }
                }

                col_def
            })
            .collect();

        sql.push_str(&column_definitions.join(", "));

        // Add table constraints
        for constraint in &table_def.constraints {
            sql.push_str(", ");
            match constraint {
                TableConstraint::PrimaryKey { columns, name } => {
                    if let Some(name) = name {
                        sql.push_str(&format!("CONSTRAINT {} ", name));
                    }
                    sql.push_str(&format!("PRIMARY KEY ({})", columns.join(", ")));
                }
                TableConstraint::ForeignKey {
                    columns,
                    referenced_table,
                    referenced_columns,
                    name,
                    on_delete,
                    on_update,
                } => {
                    if let Some(name) = name {
                        sql.push_str(&format!("CONSTRAINT {} ", name));
                    }
                    sql.push_str(&format!(
                        "FOREIGN KEY ({}) REFERENCES {}({})",
                        columns.join(", "),
                        referenced_table,
                        referenced_columns.join(", ")
                    ));
                    if let Some(action) = on_delete {
                        sql.push_str(&format!(
                            " ON DELETE {}",
                            self.referential_action_to_sql(action)
                        ));
                    }
                    if let Some(action) = on_update {
                        sql.push_str(&format!(
                            " ON UPDATE {}",
                            self.referential_action_to_sql(action)
                        ));
                    }
                }
                TableConstraint::Unique { columns, name } => {
                    if let Some(name) = name {
                        sql.push_str(&format!("CONSTRAINT {} ", name));
                    }
                    sql.push_str(&format!("UNIQUE ({})", columns.join(", ")));
                }
                TableConstraint::Check { expression, name } => {
                    if let Some(name) = name {
                        sql.push_str(&format!("CONSTRAINT {} ", name));
                    }
                    sql.push_str(&format!("CHECK ({})", expression));
                }
                _ => return Err(OrbitError::internal("Table constraint not supported")),
            }
        }

        sql.push(')');

        // Add table options
        for (key, value) in &table_def.options {
            sql.push_str(&format!(" {} {}", key, value));
        }

        Ok(sql)
    }

    /// Build CREATE INDEX SQL statement
    fn build_create_index_sql(
        &self,
        index_def: &IndexDefinition,
        if_not_exists: bool,
    ) -> OrbitResult<String> {
        let mut sql = "CREATE ".to_string();

        if index_def.unique {
            sql.push_str("UNIQUE ");
        }

        sql.push_str("INDEX ");

        if if_not_exists {
            sql.push_str("IF NOT EXISTS ");
        }

        if index_def.concurrent {
            sql.push_str("CONCURRENTLY ");
        }

        sql.push_str(&index_def.name);
        sql.push_str(&format!(" ON {}", index_def.table_name));

        // Add index method
        match index_def.index_type {
            IndexType::BTree => sql.push_str(" USING btree"),
            IndexType::Hash => sql.push_str(" USING hash"),
            IndexType::Gin => sql.push_str(" USING gin"),
            IndexType::Gist => sql.push_str(" USING gist"),
            IndexType::Spgist => sql.push_str(" USING spgist"),
            IndexType::Brin => sql.push_str(" USING brin"),
            IndexType::Ivfflat => sql.push_str(" USING ivfflat"),
            IndexType::Hnsw => sql.push_str(" USING hnsw"),
        }

        // Add columns
        sql.push_str(" (");
        let column_specs: Vec<String> = index_def
            .columns
            .iter()
            .map(|col| {
                let mut spec = col.column_name.clone();

                if let Some(op_class) = &col.operator_class {
                    spec.push(' ');
                    spec.push_str(op_class);
                }

                if let Some(order) = &col.sort_order {
                    spec.push(' ');
                    spec.push_str(match order {
                        SortOrder::Asc => "ASC",
                        SortOrder::Desc => "DESC",
                    });
                }

                if let Some(nulls) = &col.nulls_order {
                    spec.push_str(" NULLS ");
                    spec.push_str(match nulls {
                        NullsOrder::First => "FIRST",
                        NullsOrder::Last => "LAST",
                    });
                }

                spec
            })
            .collect();

        sql.push_str(&column_specs.join(", "));
        sql.push(')');

        // Add WHERE clause for partial indexes
        if let Some(where_clause) = &index_def.where_clause {
            sql.push_str(&format!(" WHERE {}", where_clause));
        }

        // Add index options
        if !index_def.options.is_empty() {
            sql.push_str(" WITH (");
            let options: Vec<String> = index_def
                .options
                .iter()
                .map(|(k, v)| format!("{} = {}", k, v))
                .collect();
            sql.push_str(&options.join(", "));
            sql.push(')');
        }

        Ok(sql)
    }

    /// Convert referential action to SQL
    fn referential_action_to_sql(&self, action: &ReferentialAction) -> &'static str {
        match action {
            ReferentialAction::NoAction => "NO ACTION",
            ReferentialAction::Restrict => "RESTRICT",
            ReferentialAction::Cascade => "CASCADE",
            ReferentialAction::SetNull => "SET NULL",
            ReferentialAction::SetDefault => "SET DEFAULT",
        }
    }
}

#[async_trait]
impl DatabaseActor for PostgresActor {
    async fn initialize(&mut self, config: DatabaseConfig) -> OrbitResult<()> {
        info!("Initializing PostgreSQL actor: {}", self.state.actor_id);

        // Update configuration
        self.state.config = config.clone();

        // Initialize connection pool
        let mut pool = self.connection_pool.lock().await;
        pool.connect(&config)
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to connect to PostgreSQL: {}", e)))?;

        self.update_stats(|stats| {
            stats.connection_attempts += 1;
            stats.successful_connections += 1;
        })
        .await;

        // Update connection pool status
        self.state
            .update_pool_status(|status| {
                status.total_connections = pool.connection_count;
                status.active_connections = pool.connection_count;
                status.last_error = None;
            })
            .await;

        self.state.is_initialized = true;

        info!(
            "PostgreSQL actor initialized successfully: {}",
            self.state.actor_id
        );
        Ok(())
    }

    async fn shutdown(&mut self) -> OrbitResult<()> {
        info!("Shutting down PostgreSQL actor: {}", self.state.actor_id);

        // Close all active transactions
        let expired = self
            .state
            .cleanup_expired_transactions(std::time::Duration::from_secs(0))
            .await;
        if expired > 0 {
            warn!(
                "Forcibly closed {} active transactions during shutdown",
                expired
            );
        }

        // Close connection pool
        let mut pool = self.connection_pool.lock().await;
        pool.close().await;

        self.state.is_initialized = false;

        info!(
            "PostgreSQL actor shut down successfully: {}",
            self.state.actor_id
        );
        Ok(())
    }

    async fn is_healthy(&self) -> bool {
        if !self.state.is_initialized {
            return false;
        }

        let pool = self.connection_pool.lock().await;
        pool.is_connected
    }

    async fn get_metrics(&self) -> DatabaseMetrics {
        let mut base_metrics = self.state.get_current_metrics().await;
        let stats = self.stats.read().await;
        let pool = self.connection_pool.lock().await;

        // Update metrics with current data
        base_metrics.pool_size = pool.connection_count;
        base_metrics.active_connections = if pool.is_connected {
            pool.connection_count
        } else {
            0
        };
        base_metrics.idle_connections = 0;
        base_metrics.pending_requests = 0;

        if stats.total_queries > 0 {
            base_metrics.queries_per_second = stats.successful_queries as f64 / 60.0; // Approximate
            base_metrics.average_query_time_ms =
                stats.total_query_time_ms as f64 / stats.total_queries as f64;
            base_metrics.error_rate = stats.failed_queries as f64 / stats.total_queries as f64;
        }

        base_metrics.cache_hit_ratio = if stats.cache_hits + stats.cache_misses > 0 {
            stats.cache_hits as f64 / (stats.cache_hits + stats.cache_misses) as f64
        } else {
            0.0
        };

        base_metrics
    }

    async fn handle_message(&mut self, message: DatabaseMessage) -> OrbitResult<DatabaseResponse> {
        self.state.update_activity().await;

        match message {
            DatabaseMessage::Schema(req) => {
                let response = self.handle_schema_request(req).await?;
                Ok(DatabaseResponse::Schema(response))
            }
            _ => {
                // Use the common message handler for other message types
                super::message_handling::route_message(self, message).await
            }
        }
    }

    async fn execute_sql(
        &mut self,
        sql: &str,
        parameters: Vec<JsonValue>,
    ) -> OrbitResult<QueryResponse> {
        let start_time = std::time::Instant::now();

        self.update_stats(|stats| {
            stats.total_queries += 1;
        })
        .await;

        let query_hash = utils::generate_query_hash(sql);

        // Check query cache first
        if let Some(_cached_query) = self.state.get_cached_query(&query_hash).await {
            self.update_stats(|stats| {
                stats.cache_hits += 1;
            })
            .await;

            debug!("Using cached query plan for: {}", sql);
        } else {
            self.update_stats(|stats| {
                stats.cache_misses += 1;
            })
            .await;
        }

        let pool = self.connection_pool.lock().await;
        let result = pool.execute_query(sql, &parameters).await;

        let duration = start_time.elapsed();
        let duration_ms = duration.as_millis() as u64;

        match result {
            Ok(mut response) => {
                // Update response with actual execution time
                response.execution_time_ms = duration_ms;

                // Cache the query plan if available
                if let Some(plan) = &response.query_plan {
                    self.state
                        .cache_query(query_hash, plan.clone(), duration_ms as f64)
                        .await;
                }

                self.update_stats(|stats| {
                    stats.successful_queries += 1;
                    stats.total_query_time_ms += duration_ms;
                })
                .await;

                debug!("Executed SQL query in {}ms: {}", duration_ms, sql);
                Ok(response)
            }
            Err(e) => {
                self.update_stats(|stats| {
                    stats.failed_queries += 1;
                })
                .await;

                // Update metrics with error information
                self.state
                    .update_metrics(|metrics| {
                        metrics.last_error = Some(e.clone());
                    })
                    .await;

                error!("Failed to execute SQL query: {} - Error: {}", sql, e);
                Err(OrbitError::internal(format!("SQL execution failed: {}", e)))
            }
        }
    }

    async fn test_connection(&self) -> OrbitResult<bool> {
        let pool = self.connection_pool.lock().await;
        Ok(pool.is_connected)
    }
}

impl Addressable for PostgresActor {
    fn addressable_type() -> &'static str {
        "PostgresActor"
    }
}

impl Clone for PostgresActor {
    fn clone(&self) -> Self {
        // Note: In a real implementation, you'd want to be more careful about cloning
        // especially with connection pools and shared state
        Self {
            state: BaseActorState::new(self.state.actor_id.clone(), self.state.config.clone()),
            connection_pool: Arc::clone(&self.connection_pool),
            reference: self.reference.clone(),
            stats: Arc::clone(&self.stats),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::messages::*;

    #[tokio::test]
    async fn test_postgres_actor_creation() {
        let actor = PostgresActor::new("test-postgres".to_string());

        assert_eq!(actor.state.actor_id, "test-postgres");
        assert!(!actor.state.is_initialized);
        assert!(!actor.is_healthy().await);
    }

    #[tokio::test]
    async fn test_postgres_actor_initialization() {
        let mut actor = PostgresActor::new("test-postgres".to_string());
        let config = DatabaseConfig::default();

        let result = actor.initialize(config).await;
        assert!(result.is_ok());
        assert!(actor.state.is_initialized);
        assert!(actor.is_healthy().await);
    }

    #[tokio::test]
    async fn test_sql_execution() {
        let mut actor = PostgresActor::new("test-postgres".to_string());
        let config = DatabaseConfig::default();
        actor.initialize(config).await.unwrap();

        // Test SELECT 1
        let result = actor.execute_sql("SELECT 1", vec![]).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.columns.len(), 1);
        assert_eq!(response.rows.len(), 1);
        assert_eq!(response.rows[0][0], JsonValue::Number(1.into()));
    }

    #[tokio::test]
    async fn test_create_table_sql_generation() {
        let actor = PostgresActor::new("test-postgres".to_string());

        let table_def = TableDefinition {
            name: "users".to_string(),
            schema: Some("public".to_string()),
            columns: vec![
                ColumnDefinition {
                    name: "id".to_string(),
                    data_type: "SERIAL".to_string(),
                    nullable: false,
                    default_value: None,
                    constraints: vec![ColumnConstraint::PrimaryKey],
                },
                ColumnDefinition {
                    name: "name".to_string(),
                    data_type: "TEXT".to_string(),
                    nullable: false,
                    default_value: None,
                    constraints: vec![],
                },
            ],
            constraints: vec![],
            options: HashMap::new(),
        };

        let sql = actor.build_create_table_sql(&table_def, true).unwrap();
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS"));
        assert!(sql.contains("public.users"));
        assert!(sql.contains("id SERIAL NOT NULL PRIMARY KEY"));
        assert!(sql.contains("name TEXT NOT NULL"));
    }

    #[tokio::test]
    async fn test_metrics_collection() {
        let mut actor = PostgresActor::new("test-postgres".to_string());
        let config = DatabaseConfig::default();
        actor.initialize(config).await.unwrap();

        // Execute a query to generate metrics
        let _result = actor.execute_sql("SELECT 1", vec![]).await.unwrap();

        let metrics = actor.get_metrics().await;
        assert_eq!(metrics.active_connections, 1);
    }

    #[tokio::test]
    async fn test_addressable_invoke() {
        let mut actor = PostgresActor::new("test-postgres".to_string());
        let config = DatabaseConfig::default();
        actor.initialize(config).await.unwrap();

        // Test invoke execute_sql method
        let args = vec![
            serde_json::Value::String("SELECT 1".to_string()),
            serde_json::Value::Array(vec![]),
        ];

        let result = actor.invoke("execute_sql", args).await;
        assert!(result.is_ok());

        // Test invoke is_healthy method
        let result = actor.invoke("is_healthy", vec![]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::Value::Bool(true));
    }
}
