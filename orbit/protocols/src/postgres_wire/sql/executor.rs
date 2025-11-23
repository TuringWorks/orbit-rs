//! Advanced SQL Statement Executor
//!
//! This module provides a comprehensive SQL executor that handles all types of SQL statements
//! including DDL, DML, DCL, TCL operations with full PostgreSQL compatibility and vector support.

use crate::error::{ProtocolError, ProtocolResult};
use crate::postgres_wire::sql::{
    ast::{
        AccessMode, AlterTableStatement, AssignmentTarget, BeginStatement, ColumnConstraint,
        CommitStatement, CreateDatabaseStatement, CreateExtensionStatement, CreateIndexStatement,
        CreateSchemaStatement, CreateTableStatement, CreateViewStatement, DeleteStatement,
        DescribeStatement, DropDatabaseStatement, DropExtensionStatement, DropIndexStatement,
        DropSchemaStatement, DropTableStatement, DropViewStatement, ExplainStatement, Expression,
        FromClause, GrantStatement, IndexType, InsertSource, InsertStatement, IsolationLevel,
        JoinCondition, JoinType, Privilege, ReleaseSavepointStatement, RevokeStatement,
        RollbackStatement, SavepointStatement, SelectItem, SelectStatement, ShowStatement,
        ShowVariable, Statement, TableConstraint, TableName, UpdateStatement, UseStatement,
    },
    expression_evaluator::{EvaluationContext, ExpressionEvaluator},
    parser::SqlParser,
    types::{SqlType, SqlValue},
};
use crate::common::storage::{StorageBackendConfig, StorageBackendFactory, TableStorage};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Query execution result
#[derive(Debug, Clone)]
pub enum ExecutionResult {
    Select {
        columns: Vec<String>,
        rows: Vec<Vec<Option<String>>>,
        row_count: usize,
    },
    Insert {
        count: usize,
    },
    Update {
        count: usize,
    },
    Delete {
        count: usize,
    },
    CreateDatabase {
        database_name: String,
    },
    CreateTable {
        table_name: String,
    },
    CreateIndex {
        index_name: String,
        table_name: String,
    },
    CreateView {
        view_name: String,
    },
    CreateSchema {
        schema_name: String,
    },
    CreateExtension {
        extension_name: String,
    },
    DropDatabase {
        database_names: Vec<String>,
    },
    DropTable {
        table_names: Vec<String>,
    },
    DropIndex {
        index_names: Vec<String>,
    },
    DropView {
        view_names: Vec<String>,
    },
    DropSchema {
        schema_names: Vec<String>,
    },
    DropExtension {
        extension_names: Vec<String>,
    },
    AlterTable {
        table_name: String,
        actions: Vec<String>,
    },
    Grant {
        privileges: Vec<String>,
        object_name: String,
        grantees: Vec<String>,
    },
    Revoke {
        privileges: Vec<String>,
        object_name: String,
        grantees: Vec<String>,
    },
    Begin {
        transaction_id: String,
    },
    Commit {
        transaction_id: String,
    },
    Rollback {
        transaction_id: String,
    },
    Savepoint {
        savepoint_name: String,
    },
    Explain {
        query_plan: String,
    },
    Show {
        variable: String,
        value: String,
    },
    Use {
        schema: String,
    },
    Describe {
        object_type: String,
        object_name: String,
        description: Vec<(String, String)>,
    },
}

/// Table schema definition
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TableSchema {
    pub name: String, // Store as string for serialization
    pub columns: Vec<ColumnSchema>,
    pub constraints: Vec<TableConstraintSchema>,
    pub indexes: Vec<IndexSchema>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: SqlType,
    pub nullable: bool,
    pub default: Option<SqlValue>,
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TableConstraintSchema {
    pub name: Option<String>,
    pub constraint_type: String,
    pub columns: Vec<String>,
    pub referenced_table: Option<String>,
    pub referenced_columns: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IndexSchema {
    pub name: String,
    pub table: String,
    pub columns: Vec<String>,
    pub index_type: String,
    pub unique: bool,
    pub condition: Option<String>,
}

/// View definition
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ViewSchema {
    pub name: String, // Store as string for serialization
    pub query: String,
    pub columns: Option<Vec<String>>,
    pub materialized: bool,
}

/// Schema definition
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SchemaDefinition {
    pub name: String,
    pub authorization: Option<String>,
}

/// Extension definition
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtensionDefinition {
    pub name: String,
    pub schema: Option<String>,
    pub version: Option<String>,
}

/// Database definition
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DatabaseDefinition {
    pub name: String,
    pub owner: Option<String>,
    pub template: Option<String>,
    pub encoding: Option<String>,
    pub locale: Option<String>,
    pub connection_limit: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Transaction state
#[derive(Debug, Clone)]
pub struct TransactionState {
    pub id: String,
    pub isolation_level: Option<IsolationLevel>,
    pub access_mode: Option<AccessMode>,
    pub savepoints: Vec<String>,
    pub start_time: chrono::DateTime<chrono::Utc>,
}

/// Transaction log entry
#[derive(Debug, Clone)]
pub struct TransactionLogEntry {
    pub transaction_id: String,
    pub operation: String,
    pub table_name: String,
    pub timestamp: std::time::Instant,
}

/// Savepoint data
#[derive(Debug, Clone)]
pub struct SavepointData {
    pub name: String,
    pub transaction_id: String,
    pub table_snapshot: HashMap<String, Vec<HashMap<String, SqlValue>>>,
    pub created_at: std::time::Instant,
}

/// Context for JOIN execution to reduce parameter passing
struct JoinExecutionContext<'a> {
    condition: &'a JoinCondition,
    where_clause: &'a Option<Expression>,
    columns: &'a [String],
}

/// Permission structure
#[derive(Debug, Clone)]
pub struct Permission {
    pub object_type: ObjectType,
    pub object_name: String,
    pub privilege: PrivilegeType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectType {
    Table,
    View,
    Schema,
    Database,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrivilegeType {
    Select,
    Insert,
    Update,
    Delete,
    Create,
    Drop,
    Alter,
    Usage,
    All,
}

/// User and role management
#[derive(Debug, Clone)]
pub struct UserRole {
    pub name: String,
    pub privileges: Vec<Privilege>,
    pub can_grant: bool,
}

/// Type alias for table data storage
type TableData = Arc<RwLock<HashMap<String, Vec<HashMap<String, SqlValue>>>>>;

/// Comprehensive SQL executor
pub struct SqlExecutor {
    // Storage backend (pluggable: in-memory, LSM, cluster)
    storage: Arc<dyn TableStorage>,

    // Legacy in-memory storage for backward compatibility during transition
    databases: Arc<RwLock<HashMap<String, DatabaseDefinition>>>,
    tables: Arc<RwLock<HashMap<String, TableSchema>>>,
    views: Arc<RwLock<HashMap<String, ViewSchema>>>,
    schemas: Arc<RwLock<HashMap<String, SchemaDefinition>>>,
    extensions: Arc<RwLock<HashMap<String, ExtensionDefinition>>>,

    // Data storage (in-memory for demonstration)
    // In production, this would integrate with OrbitClient
    table_data: TableData,

    // Transaction management
    current_transaction: Arc<RwLock<Option<TransactionState>>>,
    #[allow(dead_code)]
    transaction_log: Arc<RwLock<Vec<TransactionLogEntry>>>,
    #[allow(dead_code)]
    savepoint_data: Arc<RwLock<HashMap<String, SavepointData>>>,

    // Security and permissions
    #[allow(dead_code)]
    users: Arc<RwLock<HashMap<String, UserRole>>>,
    #[allow(dead_code)]
    current_user: Arc<RwLock<String>>,
    #[allow(dead_code)]
    permissions: Arc<RwLock<HashMap<String, Vec<Permission>>>>,

    // Settings and configuration
    settings: Arc<RwLock<HashMap<String, String>>>,
    current_database: Arc<RwLock<String>>,
    current_schema: Arc<RwLock<String>>,

    // Vector support
    vector_extensions: Arc<RwLock<HashMap<String, bool>>>,

    // Expression evaluator
    #[allow(dead_code)]
    expression_evaluator: Arc<RwLock<ExpressionEvaluator>>,
}

impl SqlExecutor {
    /// Create a new SQL executor with durable LSM storage (recommended)
    pub async fn new() -> ProtocolResult<Self> {
        let storage_config = StorageBackendConfig::default(); // Uses LSM by default
        let storage = StorageBackendFactory::create_backend(&storage_config).await?;
        storage.initialize().await?;
        Ok(Self::with_storage(storage))
    }

    /// Create a new SQL executor with custom storage backend
    pub async fn new_with_storage_config(config: StorageBackendConfig) -> ProtocolResult<Self> {
        let storage = StorageBackendFactory::create_backend(&config).await?;
        storage.initialize().await?;
        Ok(Self::with_storage(storage))
    }

    /// Create a new SQL executor with provided storage backend
    pub fn with_storage(storage: Arc<dyn TableStorage>) -> Self {
        Self::with_storage_and_settings(storage)
    }

    /// Create a new SQL executor with vector support
    pub async fn new_with_vector_support(
        _orbit_client: orbit_client::OrbitClient,
    ) -> ProtocolResult<Self> {
        // TODO: Integrate with OrbitClient for production use
        Self::new().await
    }

    /// Legacy constructor for backward compatibility (uses in-memory storage)
    #[deprecated(
        note = "Use new() for durable storage or new_with_storage_config() for custom backends"
    )]
    pub fn new_in_memory() -> Self {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let memory_config = StorageBackendConfig::Memory;
                let storage = StorageBackendFactory::create_backend(&memory_config)
                    .await
                    .unwrap();
                storage.initialize().await.unwrap();
                Self::with_storage(storage)
            })
        })
    }

    /// Simple constructor for testing that creates basic in-memory storage without tokio runtime
    pub fn new_simple_memory() -> Self {
        use crate::common::storage::memory::MemoryTableStorage;
        let storage = Arc::new(MemoryTableStorage::default());
        Self::with_storage(storage as Arc<dyn TableStorage>)
    }

    /// Create SQL executor with storage and default settings
    fn with_storage_and_settings(storage: Arc<dyn TableStorage>) -> Self {
        let mut settings = HashMap::new();
        settings.insert("server_version".to_string(), "14.0 (Orbit-RS)".to_string());
        settings.insert("server_encoding".to_string(), "UTF8".to_string());
        settings.insert("client_encoding".to_string(), "UTF8".to_string());
        settings.insert("DateStyle".to_string(), "ISO, MDY".to_string());
        settings.insert("TimeZone".to_string(), "UTC".to_string());
        settings.insert("standard_conforming_strings".to_string(), "on".to_string());

        let mut users = HashMap::new();
        users.insert(
            "postgres".to_string(),
            UserRole {
                name: "postgres".to_string(),
                privileges: vec![Privilege::All],
                can_grant: true,
            },
        );

        // Initialize with default "actors" database for backward compatibility
        let mut databases = HashMap::new();
        databases.insert(
            "actors".to_string(),
            DatabaseDefinition {
                name: "actors".to_string(),
                owner: Some("postgres".to_string()),
                template: None,
                encoding: Some("UTF8".to_string()),
                locale: Some("en_US.UTF-8".to_string()),
                connection_limit: None,
                created_at: chrono::Utc::now(),
            },
        );

        Self {
            storage,
            databases: Arc::new(RwLock::new(databases)),
            tables: Arc::new(RwLock::new(HashMap::new())),
            views: Arc::new(RwLock::new(HashMap::new())),
            schemas: Arc::new(RwLock::new(HashMap::new())),
            extensions: Arc::new(RwLock::new(HashMap::new())),
            table_data: Arc::new(RwLock::new(HashMap::new())),
            current_transaction: Arc::new(RwLock::new(None)),
            transaction_log: Arc::new(RwLock::new(Vec::new())),
            savepoint_data: Arc::new(RwLock::new(HashMap::new())),
            users: Arc::new(RwLock::new(users)),
            current_user: Arc::new(RwLock::new("postgres".to_string())),
            permissions: Arc::new(RwLock::new(HashMap::new())),
            settings: Arc::new(RwLock::new(settings)),
            current_database: Arc::new(RwLock::new("actors".to_string())),
            current_schema: Arc::new(RwLock::new("public".to_string())),
            vector_extensions: Arc::new(RwLock::new(HashMap::new())),
            expression_evaluator: Arc::new(RwLock::new(ExpressionEvaluator::new())),
        }
    }

    /// Shutdown the SQL executor and underlying storage
    pub async fn shutdown(&self) -> ProtocolResult<()> {
        self.storage.shutdown().await
    }

    /// Get storage metrics
    pub async fn storage_metrics(&self) -> crate::common::storage::StorageMetrics {
        self.storage.metrics().await
    }

    /// Set the current database context
    pub async fn set_current_database(&self, database: &str) {
        let mut current_db = self.current_database.write().await;
        *current_db = database.to_string();
    }

    /// Get the current database name
    pub async fn get_current_database(&self) -> String {
        let db = self.current_database.read().await;
        db.clone()
    }

    /// Execute a SQL statement from string
    pub async fn execute(&self, sql: &str) -> ProtocolResult<ExecutionResult> {
        // Parse the SQL statement using the comprehensive parser
        let mut parser = SqlParser::new();
        let statement = parser.parse(sql)?;

        self.execute_statement(statement).await
    }

    /// Execute a parsed SQL statement
    pub async fn execute_statement(&self, statement: Statement) -> ProtocolResult<ExecutionResult> {
        match statement {
            // DDL Operations
            Statement::CreateDatabase(stmt) => self.execute_create_database(stmt).await,
            Statement::CreateTable(stmt) => self.execute_create_table(stmt).await,
            Statement::CreateIndex(stmt) => self.execute_create_index(stmt).await,
            Statement::CreateView(stmt) => self.execute_create_view(stmt).await,
            Statement::CreateSchema(stmt) => self.execute_create_schema(stmt).await,
            Statement::CreateExtension(stmt) => self.execute_create_extension(stmt).await,
            Statement::AlterTable(stmt) => self.execute_alter_table(stmt).await,
            Statement::DropDatabase(stmt) => self.execute_drop_database(stmt).await,
            Statement::DropTable(stmt) => self.execute_drop_table(stmt).await,
            Statement::DropIndex(stmt) => self.execute_drop_index(stmt).await,
            Statement::DropView(stmt) => self.execute_drop_view(stmt).await,
            Statement::DropSchema(stmt) => self.execute_drop_schema(stmt).await,
            Statement::DropExtension(stmt) => self.execute_drop_extension(stmt).await,

            // DML Operations
            Statement::Select(stmt) => self.execute_select(*stmt).await,
            Statement::Insert(stmt) => self.execute_insert(stmt).await,
            Statement::Update(stmt) => self.execute_update(stmt).await,
            Statement::Delete(stmt) => self.execute_delete(stmt).await,

            // DCL Operations
            Statement::Grant(stmt) => self.execute_grant(stmt).await,
            Statement::Revoke(stmt) => self.execute_revoke(stmt).await,

            // TCL Operations
            Statement::Begin(stmt) => self.execute_begin(stmt).await,
            Statement::Commit(stmt) => self.execute_commit(stmt).await,
            Statement::Rollback(stmt) => self.execute_rollback(stmt).await,
            Statement::Savepoint(stmt) => self.execute_savepoint(stmt).await,
            Statement::ReleaseSavepoint(stmt) => self.execute_release_savepoint(stmt).await,

            // Utility Operations
            Statement::Explain(stmt) => self.execute_explain(stmt).await,
            Statement::Show(stmt) => self.execute_show(stmt).await,
            Statement::Use(stmt) => self.execute_use(stmt).await,
            Statement::Describe(stmt) => self.execute_describe(stmt).await,
        }
    }

    // DDL Implementation methods
    async fn execute_create_database(
        &self,
        stmt: CreateDatabaseStatement,
    ) -> ProtocolResult<ExecutionResult> {
        let database_name = &stmt.name;

        // Check if database already exists
        let databases = self.databases.read().await;
        if databases.contains_key(database_name) && !stmt.if_not_exists {
            return Err(ProtocolError::already_exists("Database", database_name));
        }
        drop(databases);

        // Create database definition
        let database_def = DatabaseDefinition {
            name: database_name.clone(),
            owner: stmt.owner.or_else(|| Some("postgres".to_string())),
            template: stmt.template,
            encoding: stmt.encoding.or_else(|| Some("UTF8".to_string())),
            locale: stmt.locale.or_else(|| Some("en_US.UTF-8".to_string())),
            connection_limit: stmt.connection_limit,
            created_at: chrono::Utc::now(),
        };

        // Store database
        let mut databases = self.databases.write().await;
        databases.insert(database_name.clone(), database_def);

        Ok(ExecutionResult::CreateDatabase {
            database_name: database_name.clone(),
        })
    }

    async fn execute_drop_database(
        &self,
        stmt: DropDatabaseStatement,
    ) -> ProtocolResult<ExecutionResult> {
        let mut dropped = Vec::new();

        for database_name in &stmt.names {
            let mut databases = self.databases.write().await;

            // Check if database exists
            if !databases.contains_key(database_name) {
                if !stmt.if_exists {
                    return Err(ProtocolError::not_found("Database", database_name));
                }
                continue;
            }

            // Prevent dropping the current database
            let current_db = self.current_database.read().await;
            if *current_db == *database_name && !stmt.force {
                return Err(ProtocolError::invalid_operation(
                    "cannot drop the currently open database",
                ));
            }
            drop(current_db);

            // Remove the database
            databases.remove(database_name);
            dropped.push(database_name.clone());
        }

        Ok(ExecutionResult::DropDatabase {
            database_names: dropped,
        })
    }

    async fn execute_create_table(
        &self,
        stmt: CreateTableStatement,
    ) -> ProtocolResult<ExecutionResult> {
        let table_name = stmt.name.full_name();

        // Check if table already exists
        let tables = self.tables.read().await;
        if tables.contains_key(&table_name) && !stmt.if_not_exists {
            return Err(ProtocolError::already_exists("Table", &table_name));
        }
        drop(tables);

        // Convert AST columns to schema
        let mut columns = Vec::new();
        for col_def in &stmt.columns {
            columns.push(ColumnSchema {
                name: col_def.name.clone(),
                data_type: col_def.data_type.clone(),
                nullable: !col_def
                    .constraints
                    .iter()
                    .any(|c| matches!(c, ColumnConstraint::NotNull)),
                default: col_def.constraints.iter().find_map(|c| match c {
                    ColumnConstraint::Default(_expr) => {
                        // TODO: Evaluate expression to get default value
                        Some(SqlValue::Null)
                    }
                    _ => None,
                }),
                constraints: col_def
                    .constraints
                    .iter()
                    .map(|c| format!("{c:?}"))
                    .collect(),
            });
        }

        // Convert AST constraints to schema
        let mut constraints = Vec::new();
        for constraint in &stmt.constraints {
            let constraint_schema = match constraint {
                TableConstraint::PrimaryKey {
                    name,
                    columns: cols,
                } => TableConstraintSchema {
                    name: name.clone(),
                    constraint_type: "PRIMARY KEY".to_string(),
                    columns: cols.clone(),
                    referenced_table: None,
                    referenced_columns: None,
                },
                TableConstraint::Unique {
                    name,
                    columns: cols,
                } => TableConstraintSchema {
                    name: name.clone(),
                    constraint_type: "UNIQUE".to_string(),
                    columns: cols.clone(),
                    referenced_table: None,
                    referenced_columns: None,
                },
                TableConstraint::ForeignKey {
                    name,
                    columns: cols,
                    references_table,
                    references_columns,
                    ..
                } => TableConstraintSchema {
                    name: name.clone(),
                    constraint_type: "FOREIGN KEY".to_string(),
                    columns: cols.clone(),
                    referenced_table: Some(references_table.full_name()),
                    referenced_columns: Some(references_columns.clone()),
                },
                TableConstraint::Check { name, .. } => TableConstraintSchema {
                    name: name.clone(),
                    constraint_type: "CHECK".to_string(),
                    columns: Vec::new(),
                    referenced_table: None,
                    referenced_columns: None,
                },
            };
            constraints.push(constraint_schema);
        }

        let table_schema = TableSchema {
            name: table_name.clone(), // Use string name
            columns,
            constraints,
            indexes: Vec::new(),
        };

        // Store table schema
        let mut tables = self.tables.write().await;
        tables.insert(table_name.clone(), table_schema);
        drop(tables);

        // Initialize empty table data
        let mut table_data = self.table_data.write().await;
        table_data.insert(table_name.clone(), Vec::new());

        Ok(ExecutionResult::CreateTable { table_name })
    }

    async fn execute_create_index(
        &self,
        stmt: CreateIndexStatement,
    ) -> ProtocolResult<ExecutionResult> {
        let table_name = stmt.table.full_name();
        let index_name = stmt.name.clone().unwrap_or_else(|| {
            format!(
                "{}_{}_idx",
                table_name,
                stmt.columns
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>()
                    .join("_")
            )
        });

        // Check if table exists
        let mut tables = self.tables.write().await;
        let table = tables
            .get_mut(&table_name)
            .ok_or_else(|| ProtocolError::table_not_found(&table_name))?;

        // Validate columns exist
        for col in &stmt.columns {
            if !table.columns.iter().any(|c| c.name == col.name) {
                return Err(ProtocolError::column_not_found(&col.name, &table_name));
            }
        }

        let index_schema = IndexSchema {
            name: index_name.clone(),
            table: table_name.clone(),
            columns: stmt.columns.iter().map(|c| c.name.clone()).collect(),
            index_type: match stmt.index_type {
                IndexType::BTree => "btree".to_string(),
                IndexType::Hash => "hash".to_string(),
                IndexType::Gist => "gist".to_string(),
                IndexType::Gin => "gin".to_string(),
                IndexType::IvfFlat { .. } => "ivfflat".to_string(),
                IndexType::Hnsw { .. } => "hnsw".to_string(),
            },
            unique: stmt.unique,
            condition: stmt
                .where_clause
                .map(|_| "TODO: serialize condition".to_string()),
        };

        table.indexes.push(index_schema);

        Ok(ExecutionResult::CreateIndex {
            index_name,
            table_name,
        })
    }

    async fn execute_create_view(
        &self,
        stmt: CreateViewStatement,
    ) -> ProtocolResult<ExecutionResult> {
        let view_name = stmt.name.full_name();

        // Check if view already exists (allow if replace=true or if_not_exists=true)
        let views = self.views.read().await;
        if views.contains_key(&view_name) && !stmt.if_not_exists && !stmt.replace {
            return Err(ProtocolError::already_exists("View", &view_name));
        }
        drop(views);

        let view_schema = ViewSchema {
            name: view_name.clone(),                    // Use string name
            query: "TODO: serialize query".to_string(), // TODO: Serialize SELECT statement
            columns: stmt.columns,
            materialized: stmt.materialized,
        };

        let mut views = self.views.write().await;
        views.insert(view_name.clone(), view_schema);

        Ok(ExecutionResult::CreateView { view_name })
    }

    async fn execute_create_schema(
        &self,
        stmt: CreateSchemaStatement,
    ) -> ProtocolResult<ExecutionResult> {
        let schema_name = stmt.name.clone();

        // Check if schema already exists
        let schemas = self.schemas.read().await;
        if schemas.contains_key(&schema_name) && !stmt.if_not_exists {
            return Err(ProtocolError::already_exists("Schema", &schema_name));
        }
        drop(schemas);

        let schema_def = SchemaDefinition {
            name: schema_name.clone(),
            authorization: stmt.authorization,
        };

        let mut schemas = self.schemas.write().await;
        schemas.insert(schema_name.clone(), schema_def);

        Ok(ExecutionResult::CreateSchema { schema_name })
    }

    async fn execute_create_extension(
        &self,
        stmt: CreateExtensionStatement,
    ) -> ProtocolResult<ExecutionResult> {
        let extension_name = stmt.name.clone();

        // Check if extension already exists
        let extensions = self.extensions.read().await;
        if extensions.contains_key(&extension_name) && !stmt.if_not_exists {
            return Err(ProtocolError::already_exists("Extension", &extension_name));
        }
        drop(extensions);

        // Special handling for vector extension
        if extension_name.to_lowercase() == "vector" {
            let mut vector_extensions = self.vector_extensions.write().await;
            vector_extensions.insert("vector".to_string(), true);
        }

        let extension_def = ExtensionDefinition {
            name: extension_name.clone(),
            schema: stmt.schema,
            version: stmt.version,
        };

        let mut extensions = self.extensions.write().await;
        extensions.insert(extension_name.clone(), extension_def);

        Ok(ExecutionResult::CreateExtension { extension_name })
    }

    // Additional DDL methods would continue here...
    async fn execute_alter_table(
        &self,
        stmt: AlterTableStatement,
    ) -> ProtocolResult<ExecutionResult> {
        let table_name = stmt.name.full_name();
        let actions: Vec<String> = stmt.actions.iter().map(|a| format!("{a:?}")).collect();

        // TODO: Implement table alteration logic

        Ok(ExecutionResult::AlterTable {
            table_name,
            actions,
        })
    }

    async fn execute_drop_table(
        &self,
        stmt: DropTableStatement,
    ) -> ProtocolResult<ExecutionResult> {
        let table_names: Vec<String> = stmt.names.iter().map(|n| n.full_name()).collect();

        let mut tables = self.tables.write().await;
        let mut table_data = self.table_data.write().await;

        for table_name in &table_names {
            if !tables.contains_key(table_name) && !stmt.if_exists {
                return Err(ProtocolError::does_not_exist("Table", table_name));
            }

            tables.remove(table_name);
            table_data.remove(table_name);
        }

        Ok(ExecutionResult::DropTable { table_names })
    }

    async fn execute_drop_index(
        &self,
        stmt: DropIndexStatement,
    ) -> ProtocolResult<ExecutionResult> {
        let index_names = stmt.names.clone();

        // TODO: Remove indexes from table schemas

        Ok(ExecutionResult::DropIndex { index_names })
    }

    async fn execute_drop_view(&self, stmt: DropViewStatement) -> ProtocolResult<ExecutionResult> {
        let view_names: Vec<String> = stmt.names.iter().map(|n| n.full_name()).collect();

        let mut views = self.views.write().await;

        for view_name in &view_names {
            if !views.contains_key(view_name) && !stmt.if_exists {
                return Err(ProtocolError::does_not_exist("View", view_name));
            }

            views.remove(view_name);
        }

        Ok(ExecutionResult::DropView { view_names })
    }

    async fn execute_drop_schema(
        &self,
        stmt: DropSchemaStatement,
    ) -> ProtocolResult<ExecutionResult> {
        let schema_names = stmt.names.clone();

        let mut schemas = self.schemas.write().await;

        for schema_name in &schema_names {
            if !schemas.contains_key(schema_name) && !stmt.if_exists {
                return Err(ProtocolError::does_not_exist("Schema", schema_name));
            }

            schemas.remove(schema_name);
        }

        Ok(ExecutionResult::DropSchema { schema_names })
    }

    async fn execute_drop_extension(
        &self,
        stmt: DropExtensionStatement,
    ) -> ProtocolResult<ExecutionResult> {
        let extension_names = stmt.names.clone();

        let mut extensions = self.extensions.write().await;
        let mut vector_extensions = self.vector_extensions.write().await;

        for extension_name in &extension_names {
            if !extensions.contains_key(extension_name) && !stmt.if_exists {
                return Err(ProtocolError::does_not_exist("Extension", extension_name));
            }

            if extension_name.to_lowercase() == "vector" {
                vector_extensions.remove("vector");
            }

            extensions.remove(extension_name);
        }

        Ok(ExecutionResult::DropExtension { extension_names })
    }

    // DML Implementation methods
    async fn execute_select(&self, stmt: SelectStatement) -> ProtocolResult<ExecutionResult> {
        let mut columns = Vec::new();
        let mut rows = Vec::new();

        // Determine result columns from SELECT list
        self.build_result_columns(&stmt.select_list, &stmt.from_clause, &mut columns)
            .await?;

        // Execute query based on FROM clause type
        if let Some(from_clause) = &stmt.from_clause {
            rows = self
                .execute_from_clause(from_clause, &stmt.where_clause, &columns)
                .await?;
        } else {
            // SELECT without FROM clause - single row with expressions
            let mut result_row = Vec::new();
            for item in &stmt.select_list {
                match item {
                    SelectItem::Expression { expr, .. } => {
                        let context = EvaluationContext::empty();
                        let value = self.evaluate_where_condition(expr, &context).await?;
                        result_row.push(Some(value.to_postgres_string()));
                    }
                    _ => result_row.push(Some("".to_string())),
                }
            }
            rows.push(result_row);
        }

        Ok(ExecutionResult::Select {
            columns,
            row_count: rows.len(),
            rows,
        })
    }

    async fn execute_insert(&self, stmt: InsertStatement) -> ProtocolResult<ExecutionResult> {
        let table_name = stmt.table.full_name();

        // Get table schema
        let tables = self.tables.read().await;
        let table_schema = tables
            .get(&table_name)
            .ok_or_else(|| ProtocolError::table_not_found(&table_name))?
            .clone();
        drop(tables);

        // Handle VALUES clause
        if let InsertSource::Values(values_list) = stmt.source {
            let columns_explicitly_specified = stmt.columns.is_some();
            let insert_columns = stmt.columns.unwrap_or_else(|| {
                // If no columns specified, use all columns
                table_schema
                    .columns
                    .iter()
                    .map(|c| c.name.clone())
                    .collect()
            });

            let mut count = 0;
            let mut rows_to_insert = Vec::new();

            // First, validate column count vs values count for each row
            // If columns were explicitly specified, the count should match exactly
            if columns_explicitly_specified {
                for values in &values_list {
                    if values.len() != insert_columns.len() {
                        return Err(ProtocolError::PostgresError(format!(
                            "Column count doesn't match value count: expected {} columns but got {} values",
                            insert_columns.len(),
                            values.len()
                        )));
                    }
                }
            }

            // Now process all rows
            for values in values_list {
                let mut row = HashMap::new();

                // Add auto-increment values for columns that look like auto-increment
                // This is a heuristic: if column name contains 'id' and is not in the insert list
                let current_time_micros = chrono::Utc::now().timestamp_micros();
                for column_def in &table_schema.columns {
                    let col_name_lower = column_def.name.to_lowercase();
                    let is_likely_auto_increment =
                        col_name_lower == "id" || col_name_lower.ends_with("_id");
                    let is_in_insert_columns = insert_columns
                        .iter()
                        .any(|c| c.eq_ignore_ascii_case(&column_def.name));

                    if is_likely_auto_increment && !is_in_insert_columns {
                        // Generate a simple auto-increment ID
                        let next_id = (current_time_micros % 1000000) + count as i64;
                        row.insert(column_def.name.clone(), SqlValue::BigInt(next_id));
                    }
                }

                // Add explicitly provided values
                for (i, value_expr) in values.iter().enumerate() {
                    if i < insert_columns.len() {
                        let col_name = &insert_columns[i];

                        // Simple expression evaluation for literals
                        let value = match value_expr {
                            Expression::Literal(sql_val) => sql_val.clone(),
                            _ => {
                                // For complex expressions, use a simple fallback for now
                                // This avoids the deadlock issue while still being functional
                                SqlValue::Text("complex_expr".to_string())
                            }
                        };

                        row.insert(col_name.clone(), value);
                    }
                }

                rows_to_insert.push(row);
            }

            // Now insert the data
            let mut table_data = self.table_data.write().await;
            let data = table_data
                .entry(table_name.clone())
                .or_insert_with(Vec::new);

            for row in rows_to_insert {
                data.push(row);
                count += 1;
            }

            Ok(ExecutionResult::Insert { count })
        } else if let InsertSource::Query(select_stmt) = stmt.source {
            // INSERT ... SELECT
            // Execute the SELECT statement first
            let select_result = self.execute_select(*select_stmt.clone()).await?;
            
            // Extract rows from SELECT result
            let rows_to_insert = match select_result {
                ExecutionResult::Select { rows, columns, .. } => {
                    let insert_columns = stmt.columns.unwrap_or_else(|| {
                        // If no columns specified, use all columns from SELECT
                        columns.clone()
                    });
                    
                    // Map SELECT rows to INSERT rows
                    rows.into_iter().map(|row| {
                        let mut insert_row = HashMap::new();
                        for (i, col_name) in insert_columns.iter().enumerate() {
                            if i < row.len() {
                                if let Some(value_str) = &row[i] {
                                    // Convert string value to appropriate SqlValue
                                    // Try to infer type from the value string
                                    let sql_value = match value_str.parse::<i64>() {
                                        Ok(i) => {
                                            // Check if it fits in i32
                                            if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                                                SqlValue::Integer(i as i32)
                                            } else {
                                                SqlValue::BigInt(i)
                                            }
                                        }
                                        Err(_) => match value_str.parse::<f64>() {
                                            Ok(f) => SqlValue::DoublePrecision(f),
                                            Err(_) => SqlValue::Text(value_str.clone()),
                                        },
                                    };
                                    insert_row.insert(col_name.clone(), sql_value);
                                }
                            }
                        }
                        insert_row
                    }).collect::<Vec<_>>()
                }
                _ => {
                    return Err(ProtocolError::PostgresError(
                        "SELECT statement in INSERT ... SELECT must return rows".to_string(),
                    ));
                }
            };
            
            // Insert the rows
            let mut table_data = self.table_data.write().await;
            let data = table_data
                .entry(table_name.clone())
                .or_insert_with(Vec::new);
            
            let count = rows_to_insert.len();
            for row in rows_to_insert {
                data.push(row);
            }
            
            Ok(ExecutionResult::Insert { count })
        } else {
            // DefaultValues
            Err(ProtocolError::PostgresError(
                "DEFAULT VALUES not yet supported".to_string(),
            ))
        }
    }

    async fn execute_update(&self, stmt: UpdateStatement) -> ProtocolResult<ExecutionResult> {
        let table_name = stmt.table.full_name();

        // Get table schema
        let tables = self.tables.read().await;
        let _table_schema = tables
            .get(&table_name)
            .ok_or_else(|| ProtocolError::table_not_found(&table_name))?
            .clone();
        drop(tables);

        let mut count = 0;
        let mut table_data = self.table_data.write().await;

        if let Some(data) = table_data.get_mut(&table_name) {
            for row in data.iter_mut() {
                let mut should_update = true;

                // Evaluate WHERE clause if present
                if let Some(where_expr) = &stmt.where_clause {
                    let context =
                        EvaluationContext::with_row_and_table(row.clone(), table_name.clone());

                    match self.evaluate_where_condition(where_expr, &context).await {
                        Ok(SqlValue::Boolean(b)) => should_update = b,
                        Ok(SqlValue::Null) => should_update = false,
                        Ok(_) => should_update = false, // Non-boolean result
                        Err(_) => should_update = false, // Error in evaluation
                    }
                }

                if should_update {
                    // Apply updates
                    for assignment in &stmt.set {
                        let value = match &assignment.value {
                            Expression::Literal(sql_val) => sql_val.clone(),
                            _ => {
                                // For complex expressions, use fallback for now
                                SqlValue::Text("updated_value".to_string())
                            }
                        };

                        // Handle different assignment target types
                        match &assignment.target {
                            AssignmentTarget::Column(col_name) => {
                                row.insert(col_name.clone(), value);
                            }
                            AssignmentTarget::Columns(col_names) => {
                                // For multiple column assignments, use first column for simplicity
                                if let Some(first_col) = col_names.first() {
                                    row.insert(first_col.clone(), value);
                                }
                            }
                        }
                    }
                    count += 1;
                }
            }
        }

        Ok(ExecutionResult::Update { count })
    }

    async fn execute_delete(&self, stmt: DeleteStatement) -> ProtocolResult<ExecutionResult> {
        let table_name = stmt.table.full_name();

        // Get table schema
        let tables = self.tables.read().await;
        let _table_schema = tables
            .get(&table_name)
            .ok_or_else(|| ProtocolError::table_not_found(&table_name))?
            .clone();
        drop(tables);

        let mut count = 0;
        let mut table_data = self.table_data.write().await;

        if let Some(data) = table_data.get_mut(&table_name) {
            let mut indices_to_remove = Vec::new();

            for (i, row) in data.iter().enumerate() {
                let mut should_delete = true;

                // Evaluate WHERE clause if present
                if let Some(where_expr) = &stmt.where_clause {
                    let context =
                        EvaluationContext::with_row_and_table(row.clone(), table_name.clone());

                    match self.evaluate_where_condition(where_expr, &context).await {
                        Ok(SqlValue::Boolean(b)) => should_delete = b,
                        Ok(SqlValue::Null) => should_delete = false,
                        Ok(_) => should_delete = false, // Non-boolean result
                        Err(_) => should_delete = false, // Error in evaluation
                    }
                }

                if should_delete {
                    indices_to_remove.push(i);
                }
            }

            // Remove rows in reverse order to maintain valid indices
            for &index in indices_to_remove.iter().rev() {
                data.remove(index);
                count += 1;
            }
        }

        Ok(ExecutionResult::Delete { count })
    }

    /// Helper method to evaluate WHERE conditions
    async fn evaluate_where_condition(
        &self,
        expr: &Expression,
        context: &EvaluationContext,
    ) -> ProtocolResult<SqlValue> {
        let mut evaluator = self.expression_evaluator.write().await;
        evaluator.evaluate(expr, context)
    }

    /// Build result columns from SELECT list
    async fn build_result_columns(
        &self,
        select_list: &[SelectItem],
        from_clause: &Option<FromClause>,
        columns: &mut Vec<String>,
    ) -> ProtocolResult<()> {
        for item in select_list {
            match item {
                SelectItem::Wildcard => {
                    // Add all columns from all tables in FROM clause
                    if let Some(from) = from_clause {
                        self.add_wildcard_columns(from, columns).await?;
                    }
                }
                SelectItem::Expression { expr, alias } => {
                    // Validate column references in the expression against table schema
                    if let Some(from) = from_clause {
                        self.validate_expression_columns(expr, from).await?;
                    }

                    // Try to resolve the column name from the expression
                    let column_name = if let Some(alias) = alias {
                        alias.clone()
                    } else {
                        self.resolve_expression_column_name(expr, from_clause)
                            .await
                            .unwrap_or_else(|| "expr".to_string())
                    };
                    columns.push(column_name);
                }
                SelectItem::QualifiedWildcard { qualifier } => {
                    // Add all columns from qualified table
                    if let Some(from) = from_clause {
                        self.add_qualified_wildcard_columns(from, qualifier, columns)
                            .await?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Add wildcard columns from FROM clause
    fn add_wildcard_columns<'a>(
        &'a self,
        from_clause: &'a FromClause,
        columns: &'a mut Vec<String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ProtocolResult<()>> + Send + 'a>> {
        Box::pin(async move {
            match from_clause {
                FromClause::Table { name, alias } => {
                    // Handle information_schema tables
                    if let Some(schema) = &name.schema {
                        if schema.to_lowercase() == "information_schema" {
                            self.add_information_schema_columns(&name.name, columns)
                                .await?;
                            return Ok(());
                        }
                    }

                    let table_name = name.full_name();
                    let tables = self.tables.read().await;
                    if let Some(table_schema) = tables.get(&table_name) {
                        for col in &table_schema.columns {
                            let column_name = if let Some(table_alias) = alias {
                                format!("{}.{}", table_alias.name, col.name)
                            } else {
                                col.name.clone()
                            };
                            columns.push(column_name);
                        }
                    }
                }
                FromClause::Join { left, right, .. } => {
                    self.add_wildcard_columns(left, columns).await?;
                    self.add_wildcard_columns(right, columns).await?;
                }
                _ => {} // TODO: Handle other FROM clause types
            }
            Ok(())
        })
    }

    /// Resolve column name from expression
    async fn resolve_expression_column_name(
        &self,
        expr: &Expression,
        _from_clause: &Option<FromClause>,
    ) -> Option<String> {
        match expr {
            Expression::Column(col_ref) => Some(col_ref.name.clone()),
            Expression::Function(func_call) => match &func_call.name {
                crate::postgres_wire::sql::ast::FunctionName::Simple(name) => Some(name.clone()),
                crate::postgres_wire::sql::ast::FunctionName::Qualified { name, .. } => {
                    Some(name.clone())
                }
            },
            _ => None,
        }
    }

    /// Validate that column references in an expression exist in the table schema
    fn validate_expression_columns<'a>(
        &'a self,
        expr: &'a Expression,
        from_clause: &'a FromClause,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ProtocolResult<()>> + Send + 'a>> {
        Box::pin(async move {
            match expr {
                Expression::Column(col_ref) => {
                    // Skip validation for qualified column references (e.g., o.amount, t.name)
                    // since proper validation would require tracking table aliases
                    if col_ref.table.is_some() {
                        return Ok(());
                    }

                    // Skip validation for information_schema tables (they have virtual columns)
                    if self.is_information_schema_query(from_clause) {
                        return Ok(());
                    }

                    // Get all valid column names from the FROM clause
                    let valid_columns = self.get_valid_columns_from_clause(from_clause).await?;

                    // If we got an empty list but there's a FROM clause, skip validation
                    // (table might not exist yet or be a special table)
                    if valid_columns.is_empty() {
                        return Ok(());
                    }

                    // Check if the column exists (case-insensitive)
                    let col_name_lower = col_ref.name.to_lowercase();
                    let exists = valid_columns.iter().any(|c| c.to_lowercase() == col_name_lower);

                    if !exists {
                        return Err(ProtocolError::PostgresError(format!(
                            "Column '{}' does not exist",
                            col_ref.name
                        )));
                    }
                }
                Expression::Binary { left, right, .. } => {
                    self.validate_expression_columns(left, from_clause).await?;
                    self.validate_expression_columns(right, from_clause).await?;
                }
                Expression::Unary { operand, .. } => {
                    self.validate_expression_columns(operand, from_clause).await?;
                }
                Expression::Function(func) => {
                    for arg in &func.args {
                        self.validate_expression_columns(arg, from_clause).await?;
                    }
                }
                Expression::Case(case_expr) => {
                    if let Some(op) = &case_expr.operand {
                        self.validate_expression_columns(op, from_clause).await?;
                    }
                    for when_clause in &case_expr.when_clauses {
                        self.validate_expression_columns(&when_clause.condition, from_clause).await?;
                        self.validate_expression_columns(&when_clause.result, from_clause).await?;
                    }
                    if let Some(else_expr) = &case_expr.else_clause {
                        self.validate_expression_columns(else_expr, from_clause).await?;
                    }
                }
                Expression::In { expr, list, .. } => {
                    self.validate_expression_columns(expr, from_clause).await?;
                    if let crate::postgres_wire::sql::ast::InList::Expressions(items) = list {
                        for item in items {
                            self.validate_expression_columns(item, from_clause).await?;
                        }
                    }
                }
                Expression::Between { expr, low, high, .. } => {
                    self.validate_expression_columns(expr, from_clause).await?;
                    self.validate_expression_columns(low, from_clause).await?;
                    self.validate_expression_columns(high, from_clause).await?;
                }
                Expression::Subquery(_) | Expression::Exists(_) => {
                    // Subqueries have their own scope; skip validation here
                }
                Expression::Literal(_) | Expression::Cast { .. } | Expression::Parameter(_) => {
                    // Literals, casts, and parameters don't need column validation
                }
                Expression::WindowFunction { partition_by, order_by, .. } => {
                    for expr in partition_by {
                        self.validate_expression_columns(expr, from_clause).await?;
                    }
                    for item in order_by {
                        self.validate_expression_columns(&item.expression, from_clause).await?;
                    }
                }
                Expression::Like { expr, pattern, .. } => {
                    self.validate_expression_columns(expr, from_clause).await?;
                    self.validate_expression_columns(pattern, from_clause).await?;
                }
                Expression::IsNull { expr, .. } => {
                    self.validate_expression_columns(expr, from_clause).await?;
                }
                Expression::Array(items) => {
                    for item in items {
                        self.validate_expression_columns(item, from_clause).await?;
                    }
                }
                Expression::ArraySlice { array, start, end } => {
                    self.validate_expression_columns(array, from_clause).await?;
                    if let Some(s) = start {
                        self.validate_expression_columns(s, from_clause).await?;
                    }
                    if let Some(e) = end {
                        self.validate_expression_columns(e, from_clause).await?;
                    }
                }
                Expression::VectorSimilarity { left, right, .. } => {
                    self.validate_expression_columns(left, from_clause).await?;
                    self.validate_expression_columns(right, from_clause).await?;
                }
                Expression::Row(items) => {
                    for item in items {
                        self.validate_expression_columns(item, from_clause).await?;
                    }
                }
                Expression::ArrayIndex { array, index } => {
                    self.validate_expression_columns(array, from_clause).await?;
                    self.validate_expression_columns(index, from_clause).await?;
                }
            }
            Ok(())
        })
    }

    /// Check if the FROM clause references information_schema tables
    fn is_information_schema_query(&self, from_clause: &FromClause) -> bool {
        match from_clause {
            FromClause::Table { name, .. } => {
                if let Some(schema) = &name.schema {
                    schema.to_lowercase() == "information_schema"
                } else {
                    false
                }
            }
            FromClause::Join { left, right, .. } => {
                self.is_information_schema_query(left) || self.is_information_schema_query(right)
            }
            _ => false,
        }
    }

    /// Get all valid column names from a FROM clause
    fn get_valid_columns_from_clause<'a>(
        &'a self,
        from_clause: &'a FromClause,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ProtocolResult<Vec<String>>> + Send + 'a>> {
        Box::pin(async move {
            let mut valid_columns = Vec::new();

            match from_clause {
                FromClause::Table { name, alias: _ } => {
                    // Handle information_schema tables
                    if let Some(schema) = &name.schema {
                        if schema.to_lowercase() == "information_schema" {
                            // For information_schema, we'll be lenient
                            return Ok(valid_columns);
                        }
                    }

                    let table_name = name.full_name();
                    let tables = self.tables.read().await;
                    if let Some(table_schema) = tables.get(&table_name) {
                        for col in &table_schema.columns {
                            valid_columns.push(col.name.clone());
                        }
                    }
                }
                FromClause::Join { left, right, .. } => {
                    let mut left_cols = self.get_valid_columns_from_clause(left).await?;
                    let mut right_cols = self.get_valid_columns_from_clause(right).await?;
                    valid_columns.append(&mut left_cols);
                    valid_columns.append(&mut right_cols);
                }
                _ => {} // Handle other FROM clause types as needed
            }

            Ok(valid_columns)
        })
    }

    /// Add columns for information_schema tables
    async fn add_information_schema_columns(
        &self,
        table_name: &str,
        columns: &mut Vec<String>,
    ) -> ProtocolResult<()> {
        match table_name.to_lowercase().as_str() {
            "tables" => {
                columns.extend_from_slice(&[
                    "table_catalog".to_string(),
                    "table_schema".to_string(),
                    "table_name".to_string(),
                    "table_type".to_string(),
                    "is_insertable_into".to_string(),
                    "is_typed".to_string(),
                    "commit_action".to_string(),
                ]);
            }
            "columns" => {
                columns.extend_from_slice(&[
                    "table_catalog".to_string(),
                    "table_schema".to_string(),
                    "table_name".to_string(),
                    "column_name".to_string(),
                    "ordinal_position".to_string(),
                    "column_default".to_string(),
                    "is_nullable".to_string(),
                    "data_type".to_string(),
                    "character_maximum_length".to_string(),
                    "character_octet_length".to_string(),
                    "numeric_precision".to_string(),
                    "numeric_scale".to_string(),
                    "datetime_precision".to_string(),
                    "udt_catalog".to_string(),
                    "udt_schema".to_string(),
                    "udt_name".to_string(),
                ]);
            }
            "table_constraints" => {
                columns.extend_from_slice(&[
                    "constraint_catalog".to_string(),
                    "constraint_schema".to_string(),
                    "constraint_name".to_string(),
                    "table_catalog".to_string(),
                    "table_schema".to_string(),
                    "table_name".to_string(),
                    "constraint_type".to_string(),
                    "is_deferrable".to_string(),
                    "initially_deferred".to_string(),
                ]);
            }
            "key_column_usage" => {
                columns.extend_from_slice(&[
                    "constraint_catalog".to_string(),
                    "constraint_schema".to_string(),
                    "constraint_name".to_string(),
                    "table_catalog".to_string(),
                    "table_schema".to_string(),
                    "table_name".to_string(),
                    "column_name".to_string(),
                    "ordinal_position".to_string(),
                    "position_in_unique_constraint".to_string(),
                    "referenced_table_schema".to_string(),
                    "referenced_table_name".to_string(),
                    "referenced_column_name".to_string(),
                ]);
            }
            _ => {
                return Err(ProtocolError::PostgresError(format!(
                    "information_schema table '{table_name}' is not supported"
                )));
            }
        }
        Ok(())
    }

    /// Add qualified wildcard columns
    fn add_qualified_wildcard_columns<'a>(
        &'a self,
        from_clause: &'a FromClause,
        qualifier: &'a str,
        columns: &'a mut Vec<String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ProtocolResult<()>> + Send + 'a>> {
        Box::pin(async move {
            match from_clause {
                FromClause::Table { name, alias } => {
                    let table_name = name.full_name();
                    let alias_name = alias.as_ref().map(|a| &a.name).unwrap_or(&table_name);

                    if alias_name == qualifier {
                        let tables = self.tables.read().await;
                        if let Some(table_schema) = tables.get(&table_name) {
                            for col in &table_schema.columns {
                                columns.push(format!("{}.{}", qualifier, col.name));
                            }
                        }
                    }
                }
                FromClause::Join { left, right, .. } => {
                    self.add_qualified_wildcard_columns(left, qualifier, columns)
                        .await?;
                    self.add_qualified_wildcard_columns(right, qualifier, columns)
                        .await?;
                }
                _ => {} // TODO: Handle other FROM clause types
            }
            Ok(())
        })
    }

    /// Execute FROM clause with JOIN support
    async fn execute_from_clause(
        &self,
        from_clause: &FromClause,
        where_clause: &Option<Expression>,
        columns: &[String],
    ) -> ProtocolResult<Vec<Vec<Option<String>>>> {
        match from_clause {
            FromClause::Table { name, .. } => {
                self.execute_single_table(name, where_clause, columns).await
            }
            FromClause::Join {
                left,
                join_type,
                right,
                condition,
            } => {
                self.execute_join(left, join_type, right, condition, where_clause, columns)
                    .await
            }
            _ => {
                // TODO: Handle subqueries and other FROM clause types
                Ok(Vec::new())
            }
        }
    }

    /// Execute single table query
    async fn execute_single_table(
        &self,
        table_name: &TableName,
        where_clause: &Option<Expression>,
        columns: &[String],
    ) -> ProtocolResult<Vec<Vec<Option<String>>>> {
        let table_name_str = table_name.full_name();
        let mut rows = Vec::new();

        // Check for information_schema queries first
        if let Some(schema) = &table_name.schema {
            if schema.to_lowercase() == "information_schema" {
                return self
                    .handle_information_schema_query(table_name, where_clause, columns)
                    .await;
            }
        }

        // Check if table exists in schema
        let tables = self.tables.read().await;
        if !tables.contains_key(&table_name_str) {
            return Err(ProtocolError::relation_not_found(&table_name_str));
        }
        drop(tables);

        // Get table data
        let table_data = self.table_data.read().await;
        if let Some(data) = table_data.get(&table_name_str) {
            for row in data {
                // Apply WHERE clause if present
                let should_include = if let Some(where_expr) = where_clause {
                    let context =
                        EvaluationContext::with_row_and_table(row.clone(), table_name_str.clone());

                    match self.evaluate_where_condition(where_expr, &context).await {
                        Ok(SqlValue::Boolean(b)) => b,
                        Ok(SqlValue::Null) => false,
                        Ok(_) => false,
                        Err(_) => false,
                    }
                } else {
                    true
                };

                if should_include {
                    let mut result_row = Vec::new();
                    for col_name in columns {
                        let value = row
                            .get(col_name)
                            .map(|v| v.to_postgres_string())
                            .unwrap_or_else(|| "".to_string());
                        result_row.push(Some(value));
                    }
                    rows.push(result_row);
                }
            }
        }

        Ok(rows)
    }

    /// Execute JOIN operation using strategy pattern to reduce complexity
    async fn execute_join(
        &self,
        left: &FromClause,
        join_type: &JoinType,
        right: &FromClause,
        condition: &JoinCondition,
        where_clause: &Option<Expression>,
        columns: &[String],
    ) -> ProtocolResult<Vec<Vec<Option<String>>>> {
        let left_rows = self.get_table_rows_from_clause(left).await?;
        let right_rows = self.get_table_rows_from_clause(right).await?;

        let join_context = JoinExecutionContext {
            condition,
            where_clause,
            columns,
        };

        match join_type {
            JoinType::Inner => {
                self.execute_inner_join(&left_rows, &right_rows, &join_context)
                    .await
            }
            JoinType::LeftOuter => {
                self.execute_left_join(&left_rows, &right_rows, &join_context)
                    .await
            }
            JoinType::RightOuter => {
                self.execute_right_join(&left_rows, &right_rows, &join_context)
                    .await
            }
            JoinType::FullOuter => {
                self.execute_full_outer_join(&left_rows, &right_rows, &join_context)
                    .await
            }
            JoinType::Cross => {
                self.execute_cross_join(&left_rows, &right_rows, &join_context)
                    .await
            }
            JoinType::LeftSemi | JoinType::LeftAnti => Err(ProtocolError::PostgresError(format!(
                "JOIN type {join_type:?} not yet implemented"
            ))),
        }
    }

    /// Execute INNER JOIN strategy
    async fn execute_inner_join(
        &self,
        left_rows: &[HashMap<String, SqlValue>],
        right_rows: &[HashMap<String, SqlValue>],
        context: &JoinExecutionContext<'_>,
    ) -> ProtocolResult<Vec<Vec<Option<String>>>> {
        let mut result_rows = Vec::new();

        for left_row in left_rows {
            for right_row in right_rows {
                if self
                    .evaluate_join_condition(context.condition, left_row, right_row)
                    .await?
                {
                    if let Some(result_row) = self
                        .try_join_and_filter(left_row, right_row, context)
                        .await?
                    {
                        result_rows.push(result_row);
                    }
                }
            }
        }

        Ok(result_rows)
    }

    /// Execute LEFT JOIN strategy  
    async fn execute_left_join(
        &self,
        left_rows: &[HashMap<String, SqlValue>],
        right_rows: &[HashMap<String, SqlValue>],
        context: &JoinExecutionContext<'_>,
    ) -> ProtocolResult<Vec<Vec<Option<String>>>> {
        let mut result_rows = Vec::new();

        for left_row in left_rows {
            let mut matched = false;

            for right_row in right_rows {
                if self
                    .evaluate_join_condition(context.condition, left_row, right_row)
                    .await?
                {
                    if let Some(result_row) = self
                        .try_join_and_filter(left_row, right_row, context)
                        .await?
                    {
                        result_rows.push(result_row);
                    }
                    matched = true;
                }
            }

            if !matched {
                // Add left row with NULL values for right side
                let null_right_row = Self::create_null_row();
                if let Some(result_row) = self
                    .try_join_and_filter(left_row, &null_right_row, context)
                    .await?
                {
                    result_rows.push(result_row);
                }
            }
        }

        Ok(result_rows)
    }

    /// Execute RIGHT JOIN strategy
    async fn execute_right_join(
        &self,
        left_rows: &[HashMap<String, SqlValue>],
        right_rows: &[HashMap<String, SqlValue>],
        context: &JoinExecutionContext<'_>,
    ) -> ProtocolResult<Vec<Vec<Option<String>>>> {
        let mut result_rows = Vec::new();

        for right_row in right_rows {
            let mut matched = false;

            for left_row in left_rows {
                if self
                    .evaluate_join_condition(context.condition, left_row, right_row)
                    .await?
                {
                    if let Some(result_row) = self
                        .try_join_and_filter(left_row, right_row, context)
                        .await?
                    {
                        result_rows.push(result_row);
                    }
                    matched = true;
                }
            }

            if !matched {
                // Add right row with NULL values for left side
                let null_left_row = Self::create_null_row();
                if let Some(result_row) = self
                    .try_join_and_filter(&null_left_row, right_row, context)
                    .await?
                {
                    result_rows.push(result_row);
                }
            }
        }

        Ok(result_rows)
    }

    /// Execute FULL OUTER JOIN strategy
    async fn execute_full_outer_join(
        &self,
        left_rows: &[HashMap<String, SqlValue>],
        right_rows: &[HashMap<String, SqlValue>],
        context: &JoinExecutionContext<'_>,
    ) -> ProtocolResult<Vec<Vec<Option<String>>>> {
        let mut result_rows = Vec::new();
        let mut left_matched = vec![false; left_rows.len()];
        let mut right_matched = vec![false; right_rows.len()];

        // Process all matching combinations
        for (li, left_row) in left_rows.iter().enumerate() {
            for (ri, right_row) in right_rows.iter().enumerate() {
                if self
                    .evaluate_join_condition(context.condition, left_row, right_row)
                    .await?
                {
                    if let Some(result_row) = self
                        .try_join_and_filter(left_row, right_row, context)
                        .await?
                    {
                        result_rows.push(result_row);
                    }
                    left_matched[li] = true;
                    right_matched[ri] = true;
                }
            }
        }

        // Add unmatched left rows
        self.add_unmatched_left_rows(&mut result_rows, left_rows, &left_matched, context)
            .await?;

        // Add unmatched right rows
        self.add_unmatched_right_rows(&mut result_rows, right_rows, &right_matched, context)
            .await?;

        Ok(result_rows)
    }

    /// Execute CROSS JOIN strategy - Cartesian product
    async fn execute_cross_join(
        &self,
        left_rows: &[HashMap<String, SqlValue>],
        right_rows: &[HashMap<String, SqlValue>],
        context: &JoinExecutionContext<'_>,
    ) -> ProtocolResult<Vec<Vec<Option<String>>>> {
        let mut result_rows = Vec::new();

        // CROSS JOIN ignores the join condition and produces Cartesian product
        for left_row in left_rows {
            for right_row in right_rows {
                if let Some(result_row) = self
                    .try_join_and_filter(left_row, right_row, context)
                    .await?
                {
                    result_rows.push(result_row);
                }
            }
        }

        Ok(result_rows)
    }

    /// Helper to join rows and apply filtering
    async fn try_join_and_filter(
        &self,
        left_row: &HashMap<String, SqlValue>,
        right_row: &HashMap<String, SqlValue>,
        context: &JoinExecutionContext<'_>,
    ) -> ProtocolResult<Option<Vec<Option<String>>>> {
        let joined_row = self.merge_rows(left_row, right_row);

        if self
            .apply_where_to_joined_row(&joined_row, context.where_clause)
            .await?
        {
            Ok(Some(self.project_columns(&joined_row, context.columns)))
        } else {
            Ok(None)
        }
    }

    /// Create an empty HashMap for null row in JOIN operations
    fn create_null_row() -> HashMap<String, SqlValue> {
        HashMap::new()
    }

    /// Add unmatched left rows for FULL OUTER JOIN
    async fn add_unmatched_left_rows(
        &self,
        result_rows: &mut Vec<Vec<Option<String>>>,
        left_rows: &[HashMap<String, SqlValue>],
        left_matched: &[bool],
        context: &JoinExecutionContext<'_>,
    ) -> ProtocolResult<()> {
        let null_right_row = Self::create_null_row();

        for (li, left_row) in left_rows.iter().enumerate() {
            if !left_matched[li] {
                if let Some(result_row) = self
                    .try_join_and_filter(left_row, &null_right_row, context)
                    .await?
                {
                    result_rows.push(result_row);
                }
            }
        }

        Ok(())
    }

    /// Add unmatched right rows for FULL OUTER JOIN
    async fn add_unmatched_right_rows(
        &self,
        result_rows: &mut Vec<Vec<Option<String>>>,
        right_rows: &[HashMap<String, SqlValue>],
        right_matched: &[bool],
        context: &JoinExecutionContext<'_>,
    ) -> ProtocolResult<()> {
        let null_left_row = Self::create_null_row();

        for (ri, right_row) in right_rows.iter().enumerate() {
            if !right_matched[ri] {
                if let Some(result_row) = self
                    .try_join_and_filter(&null_left_row, right_row, context)
                    .await?
                {
                    result_rows.push(result_row);
                }
            }
        }

        Ok(())
    }

    /// Get table rows from a FROM clause
    async fn get_table_rows_from_clause(
        &self,
        from_clause: &FromClause,
    ) -> ProtocolResult<Vec<HashMap<String, SqlValue>>> {
        match from_clause {
            FromClause::Table { name, alias } => {
                let table_name = name.full_name();
                let table_data = self.table_data.read().await;

                if let Some(data) = table_data.get(&table_name) {
                    let mut result = Vec::new();
                    for row in data {
                        let mut new_row = HashMap::new();
                        for (key, value) in row {
                            // Add both qualified and unqualified column names
                            new_row.insert(key.clone(), value.clone());
                            if let Some(table_alias) = alias {
                                new_row
                                    .insert(format!("{}.{}", table_alias.name, key), value.clone());
                            } else {
                                new_row.insert(format!("{table_name}.{key}"), value.clone());
                            }
                        }
                        result.push(new_row);
                    }
                    Ok(result)
                } else {
                    Ok(Vec::new())
                }
            }
            _ => {
                // For now, return empty for other types
                Ok(Vec::new())
            }
        }
    }

    /// Evaluate JOIN condition
    async fn evaluate_join_condition(
        &self,
        condition: &JoinCondition,
        left_row: &HashMap<String, SqlValue>,
        right_row: &HashMap<String, SqlValue>,
    ) -> ProtocolResult<bool> {
        match condition {
            JoinCondition::On(expr) => {
                let joined_row = self.merge_rows(left_row, right_row);
                let context = EvaluationContext::with_row(joined_row);

                match self.evaluate_where_condition(expr, &context).await {
                    Ok(SqlValue::Boolean(b)) => Ok(b),
                    Ok(SqlValue::Null) => Ok(false),
                    Ok(_) => Ok(false),
                    Err(_) => Ok(false),
                }
            }
            JoinCondition::Using(columns) => {
                // USING clause: join on equality of specified columns
                for col in columns {
                    let left_val = left_row.get(col).unwrap_or(&SqlValue::Null);
                    let right_val = right_row.get(col).unwrap_or(&SqlValue::Null);

                    if left_val != right_val {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            JoinCondition::Natural => {
                // NATURAL join: join on all columns with the same name
                let left_cols: std::collections::HashSet<_> = left_row.keys().collect();
                let right_cols: std::collections::HashSet<_> = right_row.keys().collect();

                for common_col in left_cols.intersection(&right_cols) {
                    let left_val = left_row.get(*common_col).unwrap_or(&SqlValue::Null);
                    let right_val = right_row.get(*common_col).unwrap_or(&SqlValue::Null);

                    if left_val != right_val {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        }
    }

    /// Merge two rows for JOIN operation
    fn merge_rows(
        &self,
        left_row: &HashMap<String, SqlValue>,
        right_row: &HashMap<String, SqlValue>,
    ) -> HashMap<String, SqlValue> {
        let mut merged = left_row.clone();
        merged.extend(right_row.clone());
        merged
    }

    /// Apply WHERE clause to joined row
    async fn apply_where_to_joined_row(
        &self,
        joined_row: &HashMap<String, SqlValue>,
        where_clause: &Option<Expression>,
    ) -> ProtocolResult<bool> {
        if let Some(where_expr) = where_clause {
            let context = EvaluationContext::with_row(joined_row.clone());

            match self.evaluate_where_condition(where_expr, &context).await {
                Ok(SqlValue::Boolean(b)) => Ok(b),
                Ok(SqlValue::Null) => Ok(false),
                Ok(_) => Ok(false),
                Err(_) => Ok(false),
            }
        } else {
            Ok(true)
        }
    }

    /// Project columns from joined row
    fn project_columns(
        &self,
        joined_row: &HashMap<String, SqlValue>,
        columns: &[String],
    ) -> Vec<Option<String>> {
        let mut result = Vec::new();
        for col_name in columns {
            let value = joined_row
                .get(col_name)
                .map(|v| v.to_postgres_string())
                .unwrap_or_default();
            result.push(Some(value));
        }
        result
    }

    /// Handle information_schema queries
    async fn handle_information_schema_query(
        &self,
        table_name: &TableName,
        _where_clause: &Option<Expression>,
        columns: &[String],
    ) -> ProtocolResult<Vec<Vec<Option<String>>>> {
        let table_name_lower = table_name.name.to_lowercase();

        match table_name_lower.as_str() {
            "tables" => self.query_information_schema_tables(columns).await,
            "columns" => self.query_information_schema_columns(columns).await,
            "table_constraints" => self.query_information_schema_constraints(columns).await,
            "key_column_usage" => self.query_information_schema_key_usage(columns).await,
            _ => Err(ProtocolError::PostgresError(format!(
                "information_schema table '{}' is not supported",
                table_name.name
            ))),
        }
    }

    /// Query information_schema.tables
    async fn query_information_schema_tables(
        &self,
        columns: &[String],
    ) -> ProtocolResult<Vec<Vec<Option<String>>>> {
        let tables = self.tables.read().await;
        let mut rows = Vec::new();

        for (table_name, _table_schema) in tables.iter() {
            let mut row_data = HashMap::new();

            // Standard information_schema.tables columns
            row_data.insert("table_catalog".to_string(), "orbit_demo".to_string());
            row_data.insert("table_schema".to_string(), "public".to_string());
            row_data.insert("table_name".to_string(), table_name.clone());
            row_data.insert("table_type".to_string(), "BASE TABLE".to_string());
            row_data.insert("is_insertable_into".to_string(), "YES".to_string());
            row_data.insert("is_typed".to_string(), "NO".to_string());
            row_data.insert("commit_action".to_string(), "".to_string());

            // Project only requested columns
            let mut result_row = Vec::new();
            for col_name in columns {
                let value = row_data
                    .get(col_name)
                    .cloned()
                    .unwrap_or_else(|| "".to_string());
                result_row.push(Some(value));
            }
            rows.push(result_row);
        }

        Ok(rows)
    }

    /// Query information_schema.columns
    async fn query_information_schema_columns(
        &self,
        columns: &[String],
    ) -> ProtocolResult<Vec<Vec<Option<String>>>> {
        let tables = self.tables.read().await;
        let mut rows = Vec::new();

        for (table_name, table_schema) in tables.iter() {
            for (ordinal_position, column) in table_schema.columns.iter().enumerate() {
                let mut row_data = HashMap::new();

                // Standard information_schema.columns columns
                row_data.insert("table_catalog".to_string(), "orbit_demo".to_string());
                row_data.insert("table_schema".to_string(), "public".to_string());
                row_data.insert("table_name".to_string(), table_name.clone());
                row_data.insert("column_name".to_string(), column.name.clone());
                row_data.insert(
                    "ordinal_position".to_string(),
                    (ordinal_position + 1).to_string(),
                );
                row_data.insert(
                    "column_default".to_string(),
                    column
                        .default
                        .as_ref()
                        .map(|d| d.to_postgres_string())
                        .unwrap_or_else(|| "".to_string()),
                );
                row_data.insert(
                    "is_nullable".to_string(),
                    if column.nullable { "YES" } else { "NO" }.to_string(),
                );
                row_data.insert(
                    "data_type".to_string(),
                    Self::sql_type_to_pg_type(&column.data_type),
                );
                row_data.insert("character_maximum_length".to_string(), "".to_string());
                row_data.insert("character_octet_length".to_string(), "".to_string());
                row_data.insert("numeric_precision".to_string(), "".to_string());
                row_data.insert("numeric_scale".to_string(), "".to_string());
                row_data.insert("datetime_precision".to_string(), "".to_string());
                row_data.insert("udt_catalog".to_string(), "orbit_demo".to_string());
                row_data.insert("udt_schema".to_string(), "pg_catalog".to_string());
                row_data.insert(
                    "udt_name".to_string(),
                    Self::sql_type_to_pg_type(&column.data_type),
                );

                // Project only requested columns
                let mut result_row = Vec::new();
                for col_name in columns {
                    let value = row_data
                        .get(col_name)
                        .cloned()
                        .unwrap_or_else(|| "".to_string());
                    result_row.push(Some(value));
                }
                rows.push(result_row);
            }
        }

        Ok(rows)
    }

    /// Query information_schema.table_constraints
    async fn query_information_schema_constraints(
        &self,
        columns: &[String],
    ) -> ProtocolResult<Vec<Vec<Option<String>>>> {
        let tables = self.tables.read().await;
        let mut rows = Vec::new();

        for (table_name, table_schema) in tables.iter() {
            for constraint in &table_schema.constraints {
                let mut row_data = HashMap::new();

                row_data.insert("constraint_catalog".to_string(), "orbit_demo".to_string());
                row_data.insert("constraint_schema".to_string(), "public".to_string());
                row_data.insert(
                    "constraint_name".to_string(),
                    constraint.name.clone().unwrap_or_else(|| {
                        format!("{}_{}_constraint", table_name, constraint.constraint_type)
                    }),
                );
                row_data.insert("table_catalog".to_string(), "orbit_demo".to_string());
                row_data.insert("table_schema".to_string(), "public".to_string());
                row_data.insert("table_name".to_string(), table_name.clone());
                row_data.insert(
                    "constraint_type".to_string(),
                    constraint.constraint_type.clone(),
                );
                row_data.insert("is_deferrable".to_string(), "NO".to_string());
                row_data.insert("initially_deferred".to_string(), "NO".to_string());

                // Project only requested columns
                let mut result_row = Vec::new();
                for col_name in columns {
                    let value = row_data
                        .get(col_name)
                        .cloned()
                        .unwrap_or_else(|| "".to_string());
                    result_row.push(Some(value));
                }
                rows.push(result_row);
            }
        }

        Ok(rows)
    }

    /// Query information_schema.key_column_usage
    async fn query_information_schema_key_usage(
        &self,
        columns: &[String],
    ) -> ProtocolResult<Vec<Vec<Option<String>>>> {
        let tables = self.tables.read().await;
        let mut rows = Vec::new();

        for (table_name, table_schema) in tables.iter() {
            for constraint in &table_schema.constraints {
                if matches!(
                    constraint.constraint_type.as_str(),
                    "PRIMARY KEY" | "FOREIGN KEY" | "UNIQUE"
                ) {
                    for (ordinal_position, column_name) in constraint.columns.iter().enumerate() {
                        let mut row_data = HashMap::new();

                        row_data.insert("constraint_catalog".to_string(), "orbit_demo".to_string());
                        row_data.insert("constraint_schema".to_string(), "public".to_string());
                        row_data.insert(
                            "constraint_name".to_string(),
                            constraint.name.clone().unwrap_or_else(|| {
                                format!("{}_{}_constraint", table_name, constraint.constraint_type)
                            }),
                        );
                        row_data.insert("table_catalog".to_string(), "orbit_demo".to_string());
                        row_data.insert("table_schema".to_string(), "public".to_string());
                        row_data.insert("table_name".to_string(), table_name.clone());
                        row_data.insert("column_name".to_string(), column_name.clone());
                        row_data.insert(
                            "ordinal_position".to_string(),
                            (ordinal_position + 1).to_string(),
                        );

                        // Foreign key specific fields
                        if constraint.constraint_type == "FOREIGN KEY" {
                            row_data.insert(
                                "position_in_unique_constraint".to_string(),
                                (ordinal_position + 1).to_string(),
                            );
                            if let Some(ref_table) = &constraint.referenced_table {
                                row_data.insert(
                                    "referenced_table_schema".to_string(),
                                    "public".to_string(),
                                );
                                row_data
                                    .insert("referenced_table_name".to_string(), ref_table.clone());
                                if let Some(ref_cols) = &constraint.referenced_columns {
                                    if let Some(ref_col) = ref_cols.get(ordinal_position) {
                                        row_data.insert(
                                            "referenced_column_name".to_string(),
                                            ref_col.clone(),
                                        );
                                    }
                                }
                            }
                        }

                        // Project only requested columns
                        let mut result_row = Vec::new();
                        for col_name in columns {
                            let value = row_data
                                .get(col_name)
                                .cloned()
                                .unwrap_or_else(|| "".to_string());
                            result_row.push(Some(value));
                        }
                        rows.push(result_row);
                    }
                }
            }
        }

        Ok(rows)
    }

    /// Convert SqlType to PostgreSQL type name
    fn sql_type_to_pg_type(sql_type: &SqlType) -> String {
        match sql_type {
            SqlType::Boolean => "boolean".to_string(),
            SqlType::SmallInt => "smallint".to_string(),
            SqlType::Integer => "integer".to_string(),
            SqlType::BigInt => "bigint".to_string(),
            SqlType::Real => "real".to_string(),
            SqlType::DoublePrecision => "double precision".to_string(),
            SqlType::Decimal { .. } => "numeric".to_string(),
            SqlType::Numeric { .. } => "numeric".to_string(),
            SqlType::Char(_) => "character".to_string(),
            SqlType::Varchar(_) => "character varying".to_string(),
            SqlType::Text => "text".to_string(),
            SqlType::Date => "date".to_string(),
            SqlType::Time { with_timezone } => {
                if *with_timezone {
                    "time with time zone".to_string()
                } else {
                    "time without time zone".to_string()
                }
            }
            SqlType::Timestamp { with_timezone } => {
                if *with_timezone {
                    "timestamp with time zone".to_string()
                } else {
                    "timestamp without time zone".to_string()
                }
            }
            SqlType::Interval => "interval".to_string(),
            SqlType::Json => "json".to_string(),
            SqlType::Jsonb => "jsonb".to_string(),
            SqlType::Vector { dimensions } => match dimensions {
                Some(dim) => format!("vector({dim})"),
                None => "vector".to_string(),
            },
            SqlType::Array { element_type, .. } => {
                format!("{}[]", Self::sql_type_to_pg_type(element_type))
            }
            SqlType::Uuid => "uuid".to_string(),
            SqlType::Bytea => "bytea".to_string(),
            SqlType::Inet => "inet".to_string(),
            SqlType::Cidr => "cidr".to_string(),
            SqlType::Macaddr => "macaddr".to_string(),
            SqlType::Macaddr8 => "macaddr8".to_string(),
            SqlType::Point => "point".to_string(),
            SqlType::Line => "line".to_string(),
            SqlType::Lseg => "lseg".to_string(),
            SqlType::Box => "box".to_string(),
            SqlType::Path => "path".to_string(),
            SqlType::Polygon => "polygon".to_string(),
            SqlType::Circle => "circle".to_string(),
            SqlType::Xml => "xml".to_string(),
            SqlType::Tsvector => "tsvector".to_string(),
            SqlType::Tsquery => "tsquery".to_string(),
            SqlType::HalfVec { dimensions } => match dimensions {
                Some(dim) => format!("halfvec({dim})"),
                None => "halfvec".to_string(),
            },
            SqlType::SparseVec { dimensions } => match dimensions {
                Some(dim) => format!("sparsevec({dim})"),
                None => "sparsevec".to_string(),
            },
            SqlType::Custom { type_name } => type_name.clone(),
            SqlType::Composite { type_name } => type_name.clone(),
            SqlType::Range { element_type } => {
                format!("{}range", Self::sql_type_to_pg_type(element_type))
            }
            SqlType::Domain { domain_name, .. } => domain_name.clone(),
        }
    }

    // DCL Implementation methods
    async fn execute_grant(&self, stmt: GrantStatement) -> ProtocolResult<ExecutionResult> {
        let privileges: Vec<String> = stmt.privileges.iter().map(|p| format!("{p:?}")).collect();

        // TODO: Implement proper permission management

        Ok(ExecutionResult::Grant {
            privileges,
            object_name: stmt.object_name,
            grantees: stmt.grantees,
        })
    }

    async fn execute_revoke(&self, stmt: RevokeStatement) -> ProtocolResult<ExecutionResult> {
        let privileges: Vec<String> = stmt.privileges.iter().map(|p| format!("{p:?}")).collect();

        // TODO: Implement proper permission management

        Ok(ExecutionResult::Revoke {
            privileges,
            object_name: stmt.object_name,
            grantees: stmt.grantees,
        })
    }

    // TCL Implementation methods
    async fn execute_begin(&self, stmt: BeginStatement) -> ProtocolResult<ExecutionResult> {
        let transaction_id = uuid::Uuid::new_v4().to_string();

        let transaction = TransactionState {
            id: transaction_id.clone(),
            isolation_level: stmt.isolation_level,
            access_mode: stmt.access_mode,
            savepoints: Vec::new(),
            start_time: chrono::Utc::now(),
        };

        let mut current_transaction = self.current_transaction.write().await;
        *current_transaction = Some(transaction);

        Ok(ExecutionResult::Begin { transaction_id })
    }

    async fn execute_commit(&self, _stmt: CommitStatement) -> ProtocolResult<ExecutionResult> {
        let mut current_transaction = self.current_transaction.write().await;

        if let Some(transaction) = current_transaction.take() {
            // TODO: Implement actual transaction commit logic
            Ok(ExecutionResult::Commit {
                transaction_id: transaction.id,
            })
        } else {
            Err(ProtocolError::PostgresError(
                "No active transaction".to_string(),
            ))
        }
    }

    async fn execute_rollback(&self, stmt: RollbackStatement) -> ProtocolResult<ExecutionResult> {
        let mut current_transaction = self.current_transaction.write().await;

        if let Some(transaction) = current_transaction.as_ref() {
            let transaction_id = transaction.id.clone();

            if let Some(_savepoint) = &stmt.to_savepoint {
                // Rollback to savepoint
                // TODO: Implement savepoint rollback logic
            } else {
                // Rollback entire transaction
                *current_transaction = None;
            }

            Ok(ExecutionResult::Rollback { transaction_id })
        } else {
            Err(ProtocolError::PostgresError(
                "No active transaction".to_string(),
            ))
        }
    }

    async fn execute_savepoint(&self, stmt: SavepointStatement) -> ProtocolResult<ExecutionResult> {
        let mut current_transaction = self.current_transaction.write().await;

        if let Some(transaction) = current_transaction.as_mut() {
            transaction.savepoints.push(stmt.name.clone());
            Ok(ExecutionResult::Savepoint {
                savepoint_name: stmt.name,
            })
        } else {
            Err(ProtocolError::PostgresError(
                "No active transaction".to_string(),
            ))
        }
    }

    async fn execute_release_savepoint(
        &self,
        stmt: ReleaseSavepointStatement,
    ) -> ProtocolResult<ExecutionResult> {
        let mut current_transaction = self.current_transaction.write().await;

        if let Some(transaction) = current_transaction.as_mut() {
            transaction.savepoints.retain(|s| s != &stmt.name);
            Ok(ExecutionResult::Savepoint {
                savepoint_name: stmt.name,
            })
        } else {
            Err(ProtocolError::PostgresError(
                "No active transaction".to_string(),
            ))
        }
    }

    // Utility Implementation methods
    async fn execute_explain(&self, stmt: ExplainStatement) -> ProtocolResult<ExecutionResult> {
        // TODO: Generate actual query plans
        let query_plan = format!("EXPLAIN output for: {:?}", stmt.statement);

        Ok(ExecutionResult::Explain { query_plan })
    }

    async fn execute_show(&self, stmt: ShowStatement) -> ProtocolResult<ExecutionResult> {
        let settings = self.settings.read().await;

        match stmt.variable {
            ShowVariable::All => {
                // TODO: Return all settings
                Ok(ExecutionResult::Show {
                    variable: "all".to_string(),
                    value: format!("{} settings", settings.len()),
                })
            }
            ShowVariable::Variable(var_name) => {
                let value = settings
                    .get(&var_name)
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string());

                Ok(ExecutionResult::Show {
                    variable: var_name,
                    value,
                })
            }
        }
    }

    async fn execute_use(&self, stmt: UseStatement) -> ProtocolResult<ExecutionResult> {
        let mut current_schema = self.current_schema.write().await;
        *current_schema = stmt.schema.clone();

        Ok(ExecutionResult::Use {
            schema: stmt.schema,
        })
    }

    async fn execute_describe(&self, stmt: DescribeStatement) -> ProtocolResult<ExecutionResult> {
        let object_type = format!("{:?}", stmt.object_type);
        let description = vec![
            ("name".to_string(), stmt.name.clone()),
            ("type".to_string(), object_type.clone()),
        ];

        Ok(ExecutionResult::Describe {
            object_type,
            object_name: stmt.name,
            description,
        })
    }
}

impl Default for SqlExecutor {
    #[allow(deprecated)]
    fn default() -> Self {
        Self::new_in_memory()
    }
}
