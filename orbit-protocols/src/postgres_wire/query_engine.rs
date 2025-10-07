//! SQL query engine for actor operations

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{ProtocolError, ProtocolResult};
use crate::postgres_wire::graphrag_engine::GraphRAGQueryEngine;
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
    // In-memory storage for demonstration
    // TODO: Replace with OrbitClient integration
    actors: Arc<RwLock<HashMap<String, ActorRecord>>>,
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

        match statement {
            Statement::Select {
                columns,
                table,
                where_clause,
            } => self.execute_select(columns, &table, where_clause).await,
            Statement::Insert {
                table,
                columns,
                values,
            } => self.execute_insert(&table, columns, values).await,
            Statement::Update {
                table,
                set_clauses,
                where_clause,
            } => self.execute_update(&table, set_clauses, where_clause).await,
            Statement::Delete {
                table,
                where_clause,
            } => self.execute_delete(&table, where_clause).await,
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
        } else {
            Err(ProtocolError::PostgresError(format!(
                "Unsupported SQL statement: {}",
                sql
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

        // Parse table
        let table = parts[from_idx + 1].to_string();

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
        let sql = sql.to_uppercase();

        if !sql.contains("INSERT INTO") || !sql.contains("VALUES") {
            return Err(ProtocolError::PostgresError(
                "Invalid INSERT syntax".to_string(),
            ));
        }

        // Extract table name
        let table_start = sql.find("INTO").unwrap() + 4;
        let table_end = sql[table_start..].find('(').unwrap() + table_start;
        let table = sql[table_start..table_end].trim().to_string();

        // Extract columns
        let col_start = table_end + 1;
        let col_end = sql[col_start..].find(')').unwrap() + col_start;
        let columns: Vec<String> = sql[col_start..col_end]
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        // Extract values
        let val_start = sql.find("VALUES").unwrap() + 6;
        let val_start = sql[val_start..].find('(').unwrap() + val_start + 1;
        let val_end = sql[val_start..].rfind(')').unwrap() + val_start;
        let values: Vec<String> = sql[val_start..val_end]
            .split(',')
            .map(|s| s.trim().trim_matches('\'').trim_matches('"').to_string())
            .collect();

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

        let table = parts[1].to_string();

        // Find SET
        let set_idx = parts
            .iter()
            .position(|&p| p.to_uppercase() == "SET")
            .ok_or_else(|| ProtocolError::PostgresError("Missing SET clause".to_string()))?;

        // Find WHERE or end
        let where_idx = parts.iter().position(|&p| p.to_uppercase() == "WHERE");
        let set_end = where_idx.unwrap_or(parts.len());

        // Parse SET clauses
        let set_str = parts[set_idx + 1..set_end].join(" ");
        let set_clauses: Vec<(String, String)> = set_str
            .split(',')
            .map(|s| {
                let kv: Vec<&str> = s.split('=').collect();
                (
                    kv[0].trim().to_string(),
                    kv[1]
                        .trim()
                        .trim_matches('\'')
                        .trim_matches('"')
                        .to_string(),
                )
            })
            .collect();

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

        let table = parts[from_idx + 1].to_string();

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
        let value = parts[2].trim_matches('\'').trim_matches('"').to_string();

        Ok(WhereClause {
            conditions: vec![Condition {
                column,
                operator,
                value,
            }],
        })
    }

    /// Execute SELECT query
    async fn execute_select(
        &self,
        columns: Vec<String>,
        table: &str,
        where_clause: Option<WhereClause>,
    ) -> ProtocolResult<QueryResult> {
        if table.to_uppercase() != "ACTORS" {
            return Err(ProtocolError::PostgresError(format!(
                "Unknown table: {}",
                table
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
            for col in &columns {
                let value = match col.to_uppercase().as_str() {
                    "*" | "ACTOR_ID" => Some(actor.actor_id.clone()),
                    "ACTOR_TYPE" => Some(actor.actor_type.clone()),
                    "STATE" => Some(actor.state.to_string()),
                    _ => None,
                };
                row.push(value);
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

    /// Execute INSERT query
    async fn execute_insert(
        &self,
        table: &str,
        columns: Vec<String>,
        values: Vec<String>,
    ) -> ProtocolResult<QueryResult> {
        if table.to_uppercase() != "ACTORS" {
            return Err(ProtocolError::PostgresError(format!(
                "Unknown table: {}",
                table
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

    /// Execute UPDATE query
    async fn execute_update(
        &self,
        table: &str,
        set_clauses: Vec<(String, String)>,
        where_clause: Option<WhereClause>,
    ) -> ProtocolResult<QueryResult> {
        if table.to_uppercase() != "ACTORS" {
            return Err(ProtocolError::PostgresError(format!(
                "Unknown table: {}",
                table
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

    /// Execute DELETE query
    async fn execute_delete(
        &self,
        table: &str,
        where_clause: Option<WhereClause>,
    ) -> ProtocolResult<QueryResult> {
        if table.to_uppercase() != "ACTORS" {
            return Err(ProtocolError::PostgresError(format!(
                "Unknown table: {}",
                table
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
}

impl Default for QueryEngine {
    fn default() -> Self {
        Self::new()
    }
}
