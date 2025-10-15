//! SQL query engine for actor operations

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{ProtocolError, ProtocolResult};
use crate::postgres_wire::graphrag_engine::GraphRAGQueryEngine;
use crate::postgres_wire::persistent_storage::{PersistentTableStorage, QueryCondition, TableRow};
use crate::postgres_wire::vector_engine::VectorQueryEngine;
use orbit_client::OrbitClient;

/// Query result types
#[derive(Debug, Clone)]
pub enum QueryResult {
    Select {
        columns: Vec<String>,
        rows: Vec<Vec<Option<String>>>,
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
}

/// Parsed SQL statement
#[derive(Debug, Clone)]
enum Statement {
    Select {
        columns: Vec<String>,
        table: String,
        where_clause: Option<WhereClause>,
    },
    Insert {
        table: String,
        columns: Vec<String>,
        values: Vec<String>,
    },
    Update {
        table: String,
        set_clauses: Vec<(String, String)>,
        where_clause: Option<WhereClause>,
    },
    Delete {
        table: String,
        where_clause: Option<WhereClause>,
    },
    CreateTable {
        table: String,
        columns: Vec<SimpleColumnDef>,
        if_not_exists: bool,
    },
    DropTable {
        table: String,
        if_exists: bool,
    },
}

/// Simple column definition for basic DDL support
#[derive(Debug, Clone)]
struct SimpleColumnDef {
    name: String,
    data_type: String,
    constraints: Vec<String>,
}

#[derive(Debug, Clone)]
struct WhereClause {
    conditions: Vec<Condition>,
}

#[derive(Debug, Clone)]
struct Condition {
    column: String,
    operator: String,
    value: String,
}

/// In-memory actor storage for demonstration
/// In production, this would use OrbitClient
#[derive(Debug, Clone)]
struct ActorRecord {
    actor_id: String,
    actor_type: String,
    state: JsonValue,
}

/// Query engine that translates SQL to actor operations
pub struct QueryEngine {
    // In-memory storage for demonstration (legacy actor operations)
    // TODO: Replace with OrbitClient integration
    actors: Arc<RwLock<HashMap<String, ActorRecord>>>,
    // Optional persistent table storage for regular SQL tables
    persistent_storage: Option<Arc<dyn PersistentTableStorage>>,
    // Optional vector query engine for pgvector compatibility
    vector_engine: Option<VectorQueryEngine>,
    // Optional GraphRAG query engine
    graphrag_engine: Option<GraphRAGQueryEngine>,
}

impl QueryEngine {
    /// Create a new query engine
    pub fn new() -> Self {
        Self {
            actors: Arc::new(RwLock::new(HashMap::new())),
            persistent_storage: None,
            vector_engine: None,
            graphrag_engine: None,
        }
    }

    /// Create a new query engine with persistent storage
    pub fn new_with_persistent_storage(storage: Arc<dyn PersistentTableStorage>) -> Self {
        Self {
            actors: Arc::new(RwLock::new(HashMap::new())),
            persistent_storage: Some(storage),
            vector_engine: None,
            graphrag_engine: None,
        }
    }

    /// Create a new query engine with vector support
    pub fn new_with_vector_support(orbit_client: OrbitClient) -> Self {
        // Since OrbitClient doesn't implement Clone, we need to create separate instances
        // For now, we'll create the GraphRAG engine in placeholder mode
        // This needs to be fixed when OrbitClient supports cloning or sharing
        Self {
            actors: Arc::new(RwLock::new(HashMap::new())),
            persistent_storage: None,
            vector_engine: Some(VectorQueryEngine::new(orbit_client)),
            graphrag_engine: Some(GraphRAGQueryEngine::new_placeholder()),
        }
    }

    /// Create a new query engine with both persistent storage and vector support
    pub fn new_with_persistent_and_vector_support(
        storage: Arc<dyn PersistentTableStorage>,
        orbit_client: OrbitClient,
    ) -> Self {
        Self {
            actors: Arc::new(RwLock::new(HashMap::new())),
            persistent_storage: Some(storage),
            vector_engine: Some(VectorQueryEngine::new(orbit_client)),
            graphrag_engine: Some(GraphRAGQueryEngine::new_placeholder()),
        }
    }

    /// Execute a SQL query
    pub async fn execute_query(&self, sql: &str) -> ProtocolResult<QueryResult> {
        let sql_upper = sql.trim().to_uppercase();

        // Check if this is a GraphRAG function query
        if self.is_graphrag_query(&sql_upper) {
            if let Some(ref graphrag_engine) = self.graphrag_engine {
                return graphrag_engine.execute_graphrag_query(sql).await;
            } else {
                return Err(ProtocolError::PostgresError(
                    "GraphRAG support not enabled. Use new_with_vector_support() to enable GraphRAG functions.".to_string()
                ));
            }
        }

        // Check if this is a vector-related query
        if let Some(ref vector_engine) = self.vector_engine {
            if self.is_vector_query(&sql_upper) {
                return vector_engine.execute_vector_query(sql).await;
            }
        }

        // Handle regular SQL queries
        let statement = self.parse_sql(sql)?;

        // Route queries based on table type and storage availability
        match statement {
            Statement::Select {
                columns,
                table,
                where_clause,
            } => {
                if table.to_uppercase() == "ACTORS" {
                    self.execute_actor_select(columns, &table, where_clause)
                        .await
                } else if let Some(ref storage) = self.persistent_storage {
                    self.execute_persistent_select(storage, columns, &table, where_clause)
                        .await
                } else {
                    Err(ProtocolError::PostgresError(format!(
                        "Table '{}' not found. Use actors table for actor queries or enable persistent storage.",
                        table
                    )))
                }
            }
            Statement::Insert {
                table,
                columns,
                values,
            } => {
                if table.to_uppercase() == "ACTORS" {
                    self.execute_actor_insert(&table, columns, values).await
                } else if let Some(ref storage) = self.persistent_storage {
                    self.execute_persistent_insert(storage, &table, columns, values)
                        .await
                } else {
                    Err(ProtocolError::PostgresError(format!(
                        "Table '{}' not found. Use actors table for actor queries or enable persistent storage.",
                        table
                    )))
                }
            }
            Statement::Update {
                table,
                set_clauses,
                where_clause,
            } => {
                if table.to_uppercase() == "ACTORS" {
                    self.execute_actor_update(&table, set_clauses, where_clause)
                        .await
                } else if let Some(ref storage) = self.persistent_storage {
                    self.execute_persistent_update(storage, &table, set_clauses, where_clause)
                        .await
                } else {
                    Err(ProtocolError::PostgresError(format!(
                        "Table '{}' not found. Use actors table for actor queries or enable persistent storage.",
                        table
                    )))
                }
            }
            Statement::Delete {
                table,
                where_clause,
            } => {
                if table.to_uppercase() == "ACTORS" {
                    self.execute_actor_delete(&table, where_clause).await
                } else if let Some(ref storage) = self.persistent_storage {
                    self.execute_persistent_delete(storage, &table, where_clause)
                        .await
                } else {
                    Err(ProtocolError::PostgresError(format!(
                        "Table '{}' not found. Use actors table for actor queries or enable persistent storage.",
                        table
                    )))
                }
            }
            Statement::CreateTable {
                table,
                columns,
                if_not_exists,
            } => {
                if let Some(ref storage) = self.persistent_storage {
                    self.execute_create_table(storage, &table, columns, if_not_exists)
                        .await
                } else {
                    Err(ProtocolError::PostgresError(
                        "CREATE TABLE requires persistent storage to be enabled".to_string(),
                    ))
                }
            }
            Statement::DropTable { table, if_exists } => {
                if let Some(ref storage) = self.persistent_storage {
                    self.execute_drop_table(storage, &table, if_exists).await
                } else {
                    Err(ProtocolError::PostgresError(
                        "DROP TABLE requires persistent storage to be enabled".to_string(),
                    ))
                }
            }
        }
    }

    /// Check if a query is vector-related
    fn is_vector_query(&self, sql: &str) -> bool {
        sql.contains("CREATE EXTENSION VECTOR")
            || sql.contains("VECTOR(")
            || sql.contains("HALFVEC(")
            || sql.contains("<->")
            || sql.contains("<#>")
            || sql.contains("<=>")
            || sql.contains("VECTOR_DIMS")
            || sql.contains("VECTOR_NORM")
            || (sql.contains("CREATE INDEX")
                && (sql.contains("USING IVFFLAT") || sql.contains("USING HNSW")))
    }

    /// Check if a query contains GraphRAG functions
    fn is_graphrag_query(&self, sql: &str) -> bool {
        sql.contains("GRAPHRAG_BUILD(")
            || sql.contains("GRAPHRAG_QUERY(")
            || sql.contains("GRAPHRAG_EXTRACT(")
            || sql.contains("GRAPHRAG_REASON(")
            || sql.contains("GRAPHRAG_STATS(")
            || sql.contains("GRAPHRAG_ENTITIES(")
            || sql.contains("GRAPHRAG_SIMILAR(")
    }

    /// Parse SQL statement
    fn parse_sql(&self, sql: &str) -> ProtocolResult<Statement> {
        let sql = sql.trim().to_uppercase();
        let original_sql = sql.clone();

        if sql.starts_with("SELECT") {
            self.parse_select(&original_sql)
        } else if sql.starts_with("INSERT") {
            self.parse_insert(&original_sql)
        } else if sql.starts_with("UPDATE") {
            self.parse_update(&original_sql)
        } else if sql.starts_with("DELETE") {
            self.parse_delete(&original_sql)
        } else if sql.starts_with("CREATE TABLE") {
            self.parse_create_table(&original_sql)
        } else if sql.starts_with("DROP TABLE") {
            self.parse_drop_table(&original_sql)
        } else {
            Err(ProtocolError::PostgresError(format!(
                "Unsupported SQL statement: {sql}"
            )))
        }
    }

    /// Parse SELECT statement
    fn parse_select(&self, sql: &str) -> ProtocolResult<Statement> {
        // Simple parser: SELECT columns FROM table [WHERE condition]
        let parts: Vec<&str> = sql.split_whitespace().collect();

        if parts.len() < 4 || parts[0].to_uppercase() != "SELECT" {
            return Err(ProtocolError::PostgresError(
                "Invalid SELECT syntax".to_string(),
            ));
        }

        // Find FROM
        let from_idx = parts
            .iter()
            .position(|&p| p.to_uppercase() == "FROM")
            .ok_or_else(|| ProtocolError::PostgresError("Missing FROM clause".to_string()))?;

        // Parse columns
        let columns_str = parts[1..from_idx].join(" ");
        let columns: Vec<String> = if columns_str == "*" {
            vec!["*".to_string()]
        } else {
            columns_str
                .split(',')
                .map(|s| s.trim().to_string())
                .collect()
        };

        // Parse table - convert to uppercase and trim semicolon
        let table = parts[from_idx + 1].trim_end_matches(';').to_uppercase();

        // Parse WHERE clause if present
        let where_idx = parts.iter().position(|&p| p.to_uppercase() == "WHERE");
        let where_clause = if let Some(idx) = where_idx {
            Some(self.parse_where_clause(&parts[idx + 1..])?)
        } else {
            None
        };

        Ok(Statement::Select {
            columns,
            table,
            where_clause,
        })
    }

    /// Parse INSERT statement
    fn parse_insert(&self, sql: &str) -> ProtocolResult<Statement> {
        // Simple parser: INSERT INTO table (columns) VALUES (values)
        let sql_upper = sql.to_uppercase();

        if !sql_upper.contains("INSERT INTO") || !sql_upper.contains("VALUES") {
            return Err(ProtocolError::PostgresError(
                "Invalid INSERT syntax".to_string(),
            ));
        }

        // Find positions using uppercase version
        let table_start = sql_upper.find("INTO").unwrap() + 4;
        let table_end = sql_upper[table_start..].find('(').unwrap() + table_start;
        let col_start = table_end + 1;
        let col_end = sql_upper[col_start..].find(')').unwrap() + col_start;
        let val_keyword_pos = sql_upper.find("VALUES").unwrap() + 6;
        let val_start = sql_upper[val_keyword_pos..].find('(').unwrap() + val_keyword_pos + 1;
        let val_end = sql_upper[val_start..].rfind(')').unwrap() + val_start;

        // Extract data using original SQL to preserve case
        let table = sql[table_start..table_end].trim().to_uppercase();
        let columns: Vec<String> = sql_upper[col_start..col_end]
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        let values = self.parse_csv_values(&sql[val_start..val_end]);

        Ok(Statement::Insert {
            table,
            columns,
            values,
        })
    }

    /// Parse UPDATE statement
    fn parse_update(&self, sql: &str) -> ProtocolResult<Statement> {
        // Simple parser: UPDATE table SET col=val [WHERE condition]
        let parts: Vec<&str> = sql.split_whitespace().collect();

        if parts.len() < 4 || parts[0].to_uppercase() != "UPDATE" {
            return Err(ProtocolError::PostgresError(
                "Invalid UPDATE syntax".to_string(),
            ));
        }

        let table = parts[1].trim_end_matches(';').to_uppercase();

        // Find SET
        let set_idx = parts
            .iter()
            .position(|&p| p.to_uppercase() == "SET")
            .ok_or_else(|| ProtocolError::PostgresError("Missing SET clause".to_string()))?;

        // Find WHERE or end
        let where_idx = parts.iter().position(|&p| p.to_uppercase() == "WHERE");
        let set_end = where_idx.unwrap_or(parts.len());

        // Parse SET clauses safely with JSON support
        let set_str = parts[set_idx + 1..set_end].join(" ");
        let set_clauses = self.parse_set_clauses(&set_str);

        // Parse WHERE clause
        let where_clause = if let Some(idx) = where_idx {
            Some(self.parse_where_clause(&parts[idx + 1..])?)
        } else {
            None
        };

        Ok(Statement::Update {
            table,
            set_clauses,
            where_clause,
        })
    }

    /// Parse DELETE statement
    fn parse_delete(&self, sql: &str) -> ProtocolResult<Statement> {
        // Simple parser: DELETE FROM table [WHERE condition]
        let parts: Vec<&str> = sql.split_whitespace().collect();

        if parts.len() < 3 || parts[0].to_uppercase() != "DELETE" {
            return Err(ProtocolError::PostgresError(
                "Invalid DELETE syntax".to_string(),
            ));
        }

        // Find FROM
        let from_idx = parts
            .iter()
            .position(|&p| p.to_uppercase() == "FROM")
            .ok_or_else(|| ProtocolError::PostgresError("Missing FROM clause".to_string()))?;

        let table = parts[from_idx + 1].trim_end_matches(';').to_uppercase();

        // Parse WHERE clause
        let where_idx = parts.iter().position(|&p| p.to_uppercase() == "WHERE");
        let where_clause = if let Some(idx) = where_idx {
            Some(self.parse_where_clause(&parts[idx + 1..])?)
        } else {
            None
        };

        Ok(Statement::Delete {
            table,
            where_clause,
        })
    }

    /// Parse SET clauses respecting quotes and JSON braces
    fn parse_set_clauses(&self, set_str: &str) -> Vec<(String, String)> {
        let mut clauses = Vec::new();
        let mut current_clause = String::new();
        let mut in_quotes = false;
        let mut quote_char = '\0';
        let mut brace_depth = 0;
        let chars: Vec<char> = set_str.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];

            match ch {
                '\'' | '"' if !in_quotes => {
                    in_quotes = true;
                    quote_char = ch;
                    current_clause.push(ch);
                }
                c if in_quotes && c == quote_char => {
                    in_quotes = false;
                    current_clause.push(ch);
                }
                '{' if !in_quotes => {
                    brace_depth += 1;
                    current_clause.push(ch);
                }
                '}' if !in_quotes => {
                    brace_depth -= 1;
                    current_clause.push(ch);
                }
                ',' if !in_quotes && brace_depth == 0 => {
                    // Found a separator - parse the current clause
                    if let Some(parsed) = self.parse_single_set_clause(&current_clause) {
                        clauses.push(parsed);
                    }
                    current_clause.clear();
                }
                _ => {
                    current_clause.push(ch);
                }
            }
            i += 1;
        }

        // Add the last clause
        if !current_clause.is_empty() {
            if let Some(parsed) = self.parse_single_set_clause(&current_clause) {
                clauses.push(parsed);
            }
        }

        clauses
    }

    /// Parse a single SET clause (key = value)
    fn parse_single_set_clause(&self, clause: &str) -> Option<(String, String)> {
        let eq_pos = clause.find('=')?;
        let key = clause[..eq_pos].trim().to_string();
        let value = clause[eq_pos + 1..]
            .trim()
            .trim_matches('\'')
            .trim_matches('"')
            .to_string();
        Some((key, value))
    }

    /// Parse CSV values respecting quotes and JSON braces
    fn parse_csv_values(&self, values_str: &str) -> Vec<String> {
        let mut values = Vec::new();
        let mut current_value = String::new();
        let mut in_quotes = false;
        let mut quote_char = '\0';
        let mut brace_depth = 0;
        let chars: Vec<char> = values_str.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];

            match ch {
                '\'' | '"' if !in_quotes => {
                    in_quotes = true;
                    quote_char = ch;
                    current_value.push(ch);
                }
                c if in_quotes && c == quote_char => {
                    in_quotes = false;
                    current_value.push(ch);
                }
                '{' if !in_quotes => {
                    brace_depth += 1;
                    current_value.push(ch);
                }
                '}' if !in_quotes => {
                    brace_depth -= 1;
                    current_value.push(ch);
                }
                ',' if !in_quotes && brace_depth == 0 => {
                    // Found a separator - add the current value
                    values.push(
                        current_value
                            .trim()
                            .trim_matches('\'')
                            .trim_matches('"')
                            .to_string(),
                    );
                    current_value.clear();
                }
                _ => {
                    current_value.push(ch);
                }
            }
            i += 1;
        }

        // Add the last value
        if !current_value.is_empty() {
            values.push(
                current_value
                    .trim()
                    .trim_matches('\'')
                    .trim_matches('"')
                    .to_string(),
            );
        }

        values
    }

    /// Parse WHERE clause
    fn parse_where_clause(&self, parts: &[&str]) -> ProtocolResult<WhereClause> {
        // Simple parser: column operator value
        if parts.len() < 3 {
            return Err(ProtocolError::PostgresError(
                "Invalid WHERE clause".to_string(),
            ));
        }

        let column = parts[0].to_string();
        let operator = parts[1].to_string();
        // Parse the value more carefully - it might span multiple parts if it contains spaces
        let value_part = parts[2..].join(" ");
        let value = value_part
            .trim_end_matches(';') // Remove trailing semicolon first
            .trim_matches('\'')
            .trim_matches('"')
            .to_string();

        Ok(WhereClause {
            conditions: vec![Condition {
                column,
                operator,
                value,
            }],
        })
    }

    /// Parse CREATE TABLE statement
    fn parse_create_table(&self, sql: &str) -> ProtocolResult<Statement> {
        // Simple parser: CREATE TABLE [IF NOT EXISTS] table_name (column_definitions)
        let sql_upper = sql.to_uppercase();

        // Check for IF NOT EXISTS
        let if_not_exists = sql_upper.contains("IF NOT EXISTS");

        // Find table name
        let table_start = if if_not_exists {
            sql_upper.find("EXISTS").unwrap() + 6
        } else {
            sql_upper.find("TABLE").unwrap() + 5
        };

        let table_end = sql[table_start..].find('(').unwrap() + table_start;
        let table_name = sql[table_start..table_end].trim().to_uppercase();

        // Find column definitions between parentheses
        let col_start = table_end + 1;
        let col_end = sql.rfind(')').ok_or_else(|| {
            ProtocolError::PostgresError(
                "Invalid CREATE TABLE syntax: missing closing parenthesis".to_string(),
            )
        })?;

        let column_defs_str = &sql[col_start..col_end];
        let mut columns = Vec::new();

        // Split by commas and parse each column definition
        for col_def in column_defs_str.split(',') {
            let col_def = col_def.trim();
            let parts: Vec<&str> = col_def.split_whitespace().collect();

            if parts.len() >= 2 {
                let name = parts[0].to_string();
                let data_type = parts[1].to_string();
                let constraints = parts[2..].iter().map(|s| s.to_string()).collect();

                columns.push(SimpleColumnDef {
                    name,
                    data_type,
                    constraints,
                });
            }
        }

        Ok(Statement::CreateTable {
            table: table_name,
            columns,
            if_not_exists,
        })
    }

    /// Parse DROP TABLE statement
    fn parse_drop_table(&self, sql: &str) -> ProtocolResult<Statement> {
        // Simple parser: DROP TABLE [IF EXISTS] table_name
        let sql_upper = sql.to_uppercase();

        // Check for IF EXISTS
        let if_exists = sql_upper.contains("IF EXISTS");

        // Find table name
        let table_start = if if_exists {
            sql_upper.find("EXISTS").unwrap() + 6
        } else {
            sql_upper.find("TABLE").unwrap() + 5
        };

        let table_name = sql[table_start..]
            .trim()
            .trim_end_matches(';')
            .to_uppercase();

        Ok(Statement::DropTable {
            table: table_name,
            if_exists,
        })
    }

    /// Execute SELECT query on actors table
    async fn execute_actor_select(
        &self,
        columns: Vec<String>,
        table: &str,
        where_clause: Option<WhereClause>,
    ) -> ProtocolResult<QueryResult> {
        if table.to_uppercase() != "ACTORS" {
            return Err(ProtocolError::PostgresError(format!(
                "Unknown table: {table}"
            )));
        }

        let actors = self.actors.read().await;
        let mut rows = Vec::new();

        for actor in actors.values() {
            // Apply WHERE filter
            if let Some(ref wc) = where_clause {
                if !self.matches_where(actor, wc) {
                    continue;
                }
            }

            // Build row
            let mut row = Vec::new();
            if columns.len() == 1 && columns[0] == "*" {
                // For SELECT *, add all columns in the expected order
                row.push(Some(actor.actor_id.clone()));
                row.push(Some(actor.actor_type.clone()));
                row.push(Some(actor.state.to_string()));
            } else {
                // For specific columns
                for col in &columns {
                    let value = match col.to_uppercase().as_str() {
                        "ACTOR_ID" => Some(actor.actor_id.clone()),
                        "ACTOR_TYPE" => Some(actor.actor_type.clone()),
                        "STATE" => Some(actor.state.to_string()),
                        _ => None,
                    };
                    row.push(value);
                }
            }
            rows.push(row);
        }

        // Determine columns
        let result_columns = if columns.len() == 1 && columns[0] == "*" {
            vec![
                "actor_id".to_string(),
                "actor_type".to_string(),
                "state".to_string(),
            ]
        } else {
            columns
        };

        Ok(QueryResult::Select {
            columns: result_columns,
            rows,
        })
    }

    /// Execute INSERT query on actors table
    async fn execute_actor_insert(
        &self,
        table: &str,
        columns: Vec<String>,
        values: Vec<String>,
    ) -> ProtocolResult<QueryResult> {
        if table.to_uppercase() != "ACTORS" {
            return Err(ProtocolError::PostgresError(format!(
                "Unknown table: {table}"
            )));
        }

        if columns.len() != values.len() {
            return Err(ProtocolError::PostgresError(
                "Column count doesn't match value count".to_string(),
            ));
        }

        let mut actor_id = None;
        let mut actor_type = None;
        let mut state = JsonValue::Object(serde_json::Map::new());

        for (col, val) in columns.iter().zip(values.iter()) {
            match col.to_uppercase().as_str() {
                "ACTOR_ID" => actor_id = Some(val.clone()),
                "ACTOR_TYPE" => actor_type = Some(val.clone()),
                "STATE" => {
                    state = serde_json::from_str(val)
                        .unwrap_or_else(|_| JsonValue::String(val.clone()));
                }
                _ => {}
            }
        }

        let actor_id =
            actor_id.ok_or_else(|| ProtocolError::PostgresError("Missing actor_id".to_string()))?;
        let actor_type = actor_type
            .ok_or_else(|| ProtocolError::PostgresError("Missing actor_type".to_string()))?;

        let record = ActorRecord {
            actor_id: actor_id.clone(),
            actor_type,
            state,
        };

        let mut actors = self.actors.write().await;
        actors.insert(actor_id, record);

        Ok(QueryResult::Insert { count: 1 })
    }

    /// Execute UPDATE query on actors table
    async fn execute_actor_update(
        &self,
        table: &str,
        set_clauses: Vec<(String, String)>,
        where_clause: Option<WhereClause>,
    ) -> ProtocolResult<QueryResult> {
        if table.to_uppercase() != "ACTORS" {
            return Err(ProtocolError::PostgresError(format!(
                "Unknown table: {table}"
            )));
        }

        let mut actors = self.actors.write().await;
        let mut count = 0;

        for actor in actors.values_mut() {
            // Apply WHERE filter
            if let Some(ref wc) = where_clause {
                if !self.matches_where(actor, wc) {
                    continue;
                }
            }

            // Apply updates
            for (col, val) in &set_clauses {
                match col.to_uppercase().as_str() {
                    "STATE" => {
                        actor.state = serde_json::from_str(val)
                            .unwrap_or_else(|_| JsonValue::String(val.clone()));
                    }
                    "ACTOR_TYPE" => {
                        actor.actor_type = val.clone();
                    }
                    _ => {}
                }
            }
            count += 1;
        }

        Ok(QueryResult::Update { count })
    }

    /// Execute DELETE query on actors table
    async fn execute_actor_delete(
        &self,
        table: &str,
        where_clause: Option<WhereClause>,
    ) -> ProtocolResult<QueryResult> {
        if table.to_uppercase() != "ACTORS" {
            return Err(ProtocolError::PostgresError(format!(
                "Unknown table: {table}"
            )));
        }

        let mut actors = self.actors.write().await;
        let mut to_delete = Vec::new();

        for (id, actor) in actors.iter() {
            // Apply WHERE filter
            if let Some(ref wc) = where_clause {
                if !self.matches_where(actor, wc) {
                    continue;
                }
            }
            to_delete.push(id.clone());
        }

        let count = to_delete.len();
        for id in to_delete {
            actors.remove(&id);
        }

        Ok(QueryResult::Delete { count })
    }

    /// Check if actor matches WHERE clause
    fn matches_where(&self, actor: &ActorRecord, where_clause: &WhereClause) -> bool {
        for condition in &where_clause.conditions {
            let value = match condition.column.to_uppercase().as_str() {
                "ACTOR_ID" => &actor.actor_id,
                "ACTOR_TYPE" => &actor.actor_type,
                _ => return false,
            };

            let matches = match condition.operator.as_str() {
                "=" => value == &condition.value,
                "!=" | "<>" => value != &condition.value,
                _ => false,
            };

            if !matches {
                return false;
            }
        }
        true
    }

    /// Execute SELECT query on persistent storage
    async fn execute_persistent_select(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        columns: Vec<String>,
        table: &str,
        where_clause: Option<WhereClause>,
    ) -> ProtocolResult<QueryResult> {
        // Check if table exists
        if !storage.table_exists(table).await? {
            return Err(ProtocolError::PostgresError(format!(
                "Table '{}' does not exist",
                table
            )));
        }

        // Convert WHERE clause to QueryConditions
        let conditions = if let Some(wc) = where_clause {
            wc.conditions
                .into_iter()
                .map(|c| QueryCondition {
                    column: c.column.to_uppercase(), // Normalize column names to uppercase
                    operator: c.operator,
                    value: match serde_json::from_str(&c.value) {
                        Ok(json_val) => json_val,
                        Err(_) => JsonValue::String(c.value),
                    },
                })
                .collect()
        } else {
            vec![]
        };

        // Execute select query - pass normalized column names to storage
        let storage_columns = if columns.len() == 1 && columns[0] == "*" {
            // For SELECT *, pass empty columns to storage (no filtering)
            vec![]
        } else {
            // Normalize column names to uppercase
            columns.into_iter().map(|c| c.to_uppercase()).collect()
        };
        
        let rows = storage
            .select_rows(table, storage_columns.clone(), conditions, None)
            .await?;

        // Convert TableRows to QueryResult format
        let result_columns = if storage_columns.is_empty() {
            // For SELECT *, get columns from schema and normalize to uppercase
            if let Some(schema) = storage.get_table_schema(table).await? {
                schema.columns.into_iter().map(|c| c.name.to_uppercase()).collect()
            } else {
                vec![]
            }
        } else {
            // Use the normalized column names
            storage_columns.clone()
        };

        let result_rows: Vec<Vec<Option<String>>> = rows
            .into_iter()
            .map(|row| {
                result_columns
                    .iter()
                    .map(|col| {
                        // Try both original case and uppercase for compatibility
                        let value = row.values.get(col).or_else(|| row.values.get(&col.to_uppercase()));
                        value.map(|v| match v {
                            JsonValue::String(s) => s.clone(),
                            JsonValue::Number(n) => n.to_string(),
                            JsonValue::Bool(b) => b.to_string(),
                            JsonValue::Null => "NULL".to_string(),
                            _ => v.to_string(),
                        })
                    })
                    .collect()
            })
            .collect();

        Ok(QueryResult::Select {
            columns: result_columns,
            rows: result_rows,
        })
    }

    /// Execute INSERT query on persistent storage
    async fn execute_persistent_insert(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        table: &str,
        columns: Vec<String>,
        values: Vec<String>,
    ) -> ProtocolResult<QueryResult> {
        if columns.len() != values.len() {
            return Err(ProtocolError::PostgresError(
                "Column count doesn't match value count".to_string(),
            ));
        }

        // Check if table exists
        if !storage.table_exists(table).await? {
            return Err(ProtocolError::PostgresError(format!(
                "Table '{}' does not exist",
                table
            )));
        }

        // Get table schema to handle column types properly
        let schema = storage.get_table_schema(table).await?;
        let schema = schema.ok_or_else(|| {
            ProtocolError::PostgresError(format!("Table '{}' schema not found", table))
        })?;

        // Build row data
        let mut row_values = std::collections::HashMap::new();
        let now = chrono::Utc::now();

        // Handle SERIAL columns (auto-increment)
        use crate::postgres_wire::persistent_storage::ColumnType;
        for column_def in &schema.columns {
            if matches!(column_def.data_type, ColumnType::Serial) {
                // Generate next ID - for now use a simple counter based on current time
                let next_id = chrono::Utc::now().timestamp_micros() % 1000000;
                row_values.insert(column_def.name.to_uppercase(), JsonValue::Number(serde_json::Number::from(next_id)));
            }
        }

        for (col, val) in columns.iter().zip(values.iter()) {
            let col_upper = col.to_uppercase();
            
            // Skip SERIAL columns as they're auto-generated
            if let Some(column_def) = schema.columns.iter().find(|c| c.name.to_uppercase() == col_upper) {
                if matches!(column_def.data_type, ColumnType::Serial) {
                    continue;
                }
            }
            
            // Try to parse as JSON, fall back to string
            let json_val = match serde_json::from_str(val) {
                Ok(json) => json,
                Err(_) => JsonValue::String(val.clone()),
            };
            row_values.insert(col_upper, json_val);
        }

        let row = TableRow {
            values: row_values,
            created_at: now,
            updated_at: now,
        };

        // Insert the row
        storage.insert_row(table, row).await?;

        Ok(QueryResult::Insert { count: 1 })
    }

    /// Execute UPDATE query on persistent storage
    async fn execute_persistent_update(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        table: &str,
        set_clauses: Vec<(String, String)>,
        where_clause: Option<WhereClause>,
    ) -> ProtocolResult<QueryResult> {
        // Check if table exists
        if !storage.table_exists(table).await? {
            return Err(ProtocolError::PostgresError(format!(
                "Table '{}' does not exist",
                table
            )));
        }

        // Convert SET clauses to HashMap
        let mut set_values = std::collections::HashMap::new();
        for (col, val) in set_clauses {
            let json_val = match serde_json::from_str(&val) {
                Ok(json) => json,
                Err(_) => JsonValue::String(val),
            };
            set_values.insert(col.to_uppercase(), json_val); // Normalize to uppercase
        }

        // Convert WHERE clause to QueryConditions  
        let conditions = if let Some(wc) = where_clause {
            wc.conditions
                .into_iter()
                .map(|c| QueryCondition {
                    column: c.column.to_uppercase(), // Normalize column names to uppercase
                    operator: c.operator,
                    value: match serde_json::from_str(&c.value) {
                        Ok(json_val) => json_val,
                        Err(_) => JsonValue::String(c.value),
                    },
                })
                .collect()
        } else {
            vec![]
        };

        // Execute update
        let count = storage.update_rows(table, set_values, conditions).await?;

        Ok(QueryResult::Update {
            count: count as usize,
        })
    }

    /// Execute DELETE query on persistent storage
    async fn execute_persistent_delete(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        table: &str,
        where_clause: Option<WhereClause>,
    ) -> ProtocolResult<QueryResult> {
        // Check if table exists
        if !storage.table_exists(table).await? {
            return Err(ProtocolError::PostgresError(format!(
                "Table '{}' does not exist",
                table
            )));
        }

        // Convert WHERE clause to QueryConditions
        let conditions = if let Some(wc) = where_clause {
            wc.conditions
                .into_iter()
                .map(|c| QueryCondition {
                    column: c.column.to_uppercase(), // Normalize column names to uppercase
                    operator: c.operator,
                    value: match serde_json::from_str(&c.value) {
                        Ok(json_val) => json_val,
                        Err(_) => JsonValue::String(c.value),
                    },
                })
                .collect()
        } else {
            vec![]
        };

        // Execute delete
        let count = storage.delete_rows(table, conditions).await?;

        Ok(QueryResult::Delete {
            count: count as usize,
        })
    }

    /// Execute CREATE TABLE on persistent storage
    async fn execute_create_table(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        table: &str,
        columns: Vec<SimpleColumnDef>,
        if_not_exists: bool,
    ) -> ProtocolResult<QueryResult> {
        use crate::postgres_wire::persistent_storage::{ColumnDefinition, ColumnType, TableSchema};

        // Check if table already exists
        if storage.table_exists(table).await? {
            if if_not_exists {
                // IF NOT EXISTS specified, just return success without creating
                return Ok(QueryResult::Select {
                    columns: vec![],
                    rows: vec![],
                });
            } else {
                return Err(ProtocolError::PostgresError(format!(
                    "Table '{}' already exists",
                    table
                )));
            }
        }

        // Convert simple column definitions to persistent storage format
        let mut column_defs = Vec::new();
        for col in columns {
            let column_type = match col.data_type.to_uppercase().as_str() {
                "INTEGER" | "INT" => ColumnType::Integer,
                "BIGINT" => ColumnType::BigInt,
                "SERIAL" => ColumnType::Serial,
                "TEXT" => ColumnType::Text,
                "BOOLEAN" | "BOOL" => ColumnType::Boolean,
                "JSON" => ColumnType::Json,
                "TIMESTAMP" => ColumnType::Timestamp,
                data_type => {
                    if data_type.starts_with("VARCHAR") {
                        // Extract length if present
                        let len = if let Some(start) = data_type.find('(') {
                            let end = data_type.find(')').unwrap_or(data_type.len());
                            data_type[start + 1..end].parse().unwrap_or(255)
                        } else {
                            255
                        };
                        ColumnType::Varchar(len)
                    } else {
                        // Default to text for unknown types
                        ColumnType::Text
                    }
                }
            };

            let nullable = !col
                .constraints
                .iter()
                .any(|c| c.to_uppercase() == "NOT" || c.to_uppercase().contains("NULL"));

            column_defs.push(ColumnDefinition {
                name: col.name.to_uppercase(), // Normalize to uppercase for consistency
                data_type: column_type,
                nullable,
                default_value: None, // TODO: Parse DEFAULT values
            });
        }

        let schema = TableSchema {
            name: table.to_string(),
            columns: column_defs,
            created_at: chrono::Utc::now(),
            row_count: 0,
        };

        // Create the table
        storage.create_table(schema).await?;

        Ok(QueryResult::Select {
            columns: vec![],
            rows: vec![],
        })
    }

    /// Execute DROP TABLE on persistent storage
    async fn execute_drop_table(
        &self,
        storage: &Arc<dyn PersistentTableStorage>,
        table: &str,
        if_exists: bool,
    ) -> ProtocolResult<QueryResult> {
        // Check if table exists
        if !storage.table_exists(table).await? {
            if if_exists {
                // IF EXISTS specified, just return success without dropping
                return Ok(QueryResult::Select {
                    columns: vec![],
                    rows: vec![],
                });
            } else {
                return Err(ProtocolError::PostgresError(format!(
                    "Table '{}' does not exist",
                    table
                )));
            }
        }

        // Drop the table
        storage.drop_table(table).await?;

        Ok(QueryResult::Select {
            columns: vec![],
            rows: vec![],
        })
    }
}

impl Default for QueryEngine {
    fn default() -> Self {
        Self::new()
    }
}
