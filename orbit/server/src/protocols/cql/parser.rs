//! CQL statement parser
//!
//! This module provides a parser for CQL (Cassandra Query Language) statements.

use super::types::{CqlType, CqlValue};
use crate::protocols::error::{ProtocolError, ProtocolResult};
use std::collections::HashMap;

/// Parsed CQL statement
#[derive(Debug, Clone, PartialEq)]
pub enum CqlStatement {
    /// SELECT statement
    Select {
        /// Columns to select (* for all)
        columns: Vec<String>,
        /// Table name (keyspace.table)
        table: String,
        /// WHERE clause conditions
        where_clause: Option<Vec<WhereCondition>>,
        /// LIMIT clause
        limit: Option<usize>,
        /// ALLOW FILTERING
        allow_filtering: bool,
    },
    /// INSERT statement
    Insert {
        /// Table name
        table: String,
        /// Column names
        columns: Vec<String>,
        /// Values to insert
        values: Vec<CqlValue>,
        /// IF NOT EXISTS
        if_not_exists: bool,
        /// TTL (time to live) in seconds
        ttl: Option<i32>,
    },
    /// UPDATE statement
    Update {
        /// Table name
        table: String,
        /// SET clause assignments
        assignments: HashMap<String, CqlValue>,
        /// WHERE clause conditions
        where_clause: Vec<WhereCondition>,
        /// IF conditions
        if_clause: Option<Vec<WhereCondition>>,
        /// TTL
        ttl: Option<i32>,
    },
    /// DELETE statement
    Delete {
        /// Columns to delete (empty for entire row)
        columns: Vec<String>,
        /// Table name
        table: String,
        /// WHERE clause conditions
        where_clause: Vec<WhereCondition>,
        /// IF conditions
        if_clause: Option<Vec<WhereCondition>>,
    },
    /// CREATE KEYSPACE statement
    CreateKeyspace {
        /// Keyspace name
        name: String,
        /// IF NOT EXISTS
        if_not_exists: bool,
        /// Replication strategy
        replication: HashMap<String, String>,
        /// Durable writes
        durable_writes: bool,
    },
    /// CREATE TABLE statement
    CreateTable {
        /// Table name (keyspace.table)
        name: String,
        /// IF NOT EXISTS
        if_not_exists: bool,
        /// Column definitions
        columns: Vec<ColumnDef>,
        /// Primary key columns
        primary_key: Vec<String>,
        /// Clustering key columns
        clustering_key: Vec<(String, ClusteringOrder)>,
        /// Table options
        options: HashMap<String, String>,
    },
    /// DROP KEYSPACE statement
    DropKeyspace {
        /// Keyspace name
        name: String,
        /// IF EXISTS
        if_exists: bool,
    },
    /// DROP TABLE statement
    DropTable {
        /// Table name
        name: String,
        /// IF EXISTS
        if_exists: bool,
    },
    /// USE statement (switch keyspace)
    Use {
        /// Keyspace name
        keyspace: String,
    },
    /// BATCH statement
    Batch {
        /// Batch type (LOGGED, UNLOGGED, COUNTER)
        batch_type: BatchType,
        /// Statements in batch
        statements: Vec<CqlStatement>,
    },
    /// TRUNCATE statement
    Truncate {
        /// Table name
        table: String,
    },
    /// CREATE INDEX statement
    CreateIndex {
        /// Index name
        name: String,
        /// IF NOT EXISTS
        if_not_exists: bool,
        /// Table name
        table: String,
        /// Column to index
        column: String,
    },
    /// CREATE TYPE (UDT) statement
    CreateType {
        /// Type name
        name: String,
        /// IF NOT EXISTS
        if_not_exists: bool,
        /// Fields of the UDT
        fields: Vec<(String, CqlType)>,
    },
    /// CREATE MATERIALIZED VIEW statement
    CreateMaterializedView {
        /// View name
        name: String,
        /// IF NOT EXISTS
        if_not_exists: bool,
        /// Source table
        source_table: String,
        /// Selected columns
        columns: Vec<String>,
        /// WHERE clause for the view
        where_clause: Option<String>,
        /// Primary key columns
        primary_key: Vec<String>,
    },
}

/// WHERE clause condition
#[derive(Debug, Clone, PartialEq)]
pub struct WhereCondition {
    /// Column name
    pub column: String,
    /// Operator
    pub operator: ComparisonOperator,
    /// Value
    pub value: CqlValue,
}

/// Comparison operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOperator {
    /// Equality (=)
    Equal,
    /// Greater than (>)
    GreaterThan,
    /// Greater than or equal (>=)
    GreaterThanOrEqual,
    /// Less than (<)
    LessThan,
    /// Less than or equal (<=)
    LessThanOrEqual,
    /// Not equal (!=)
    NotEqual,
    /// IN operator
    In,
    /// CONTAINS operator (for collections)
    Contains,
    /// CONTAINS KEY operator (for maps)
    ContainsKey,
}

/// Column definition for CREATE TABLE
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    /// Column name
    pub name: String,
    /// Data type
    pub data_type: CqlType,
    /// Is static column
    pub is_static: bool,
}

/// Clustering order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusteringOrder {
    /// Ascending order
    Asc,
    /// Descending order
    Desc,
}

/// Batch type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchType {
    /// Logged batch (default)
    Logged,
    /// Unlogged batch
    Unlogged,
    /// Counter batch
    Counter,
}

/// CQL parser
pub struct CqlParser {
    /// Current keyspace
    current_keyspace: Option<String>,
}

impl CqlParser {
    /// Create a new parser
    pub fn new() -> Self {
        Self {
            current_keyspace: None,
        }
    }

    /// Set current keyspace
    pub fn set_keyspace(&mut self, keyspace: String) {
        self.current_keyspace = Some(keyspace);
    }

    /// Get current keyspace
    pub fn current_keyspace(&self) -> Option<&str> {
        self.current_keyspace.as_deref()
    }

    /// Parse a CQL statement
    pub fn parse(&self, query: &str) -> ProtocolResult<CqlStatement> {
        let query = query.trim();
        let query_upper = query.to_uppercase();

        if query_upper.starts_with("SELECT") {
            self.parse_select(query)
        } else if query_upper.starts_with("INSERT") {
            self.parse_insert(query)
        } else if query_upper.starts_with("UPDATE") {
            self.parse_update(query)
        } else if query_upper.starts_with("DELETE") {
            self.parse_delete(query)
        } else if query_upper.starts_with("CREATE KEYSPACE") {
            self.parse_create_keyspace(query)
        } else if query_upper.starts_with("CREATE TABLE") {
            self.parse_create_table(query)
        } else if query_upper.starts_with("CREATE INDEX") {
            self.parse_create_index(query)
        } else if query_upper.starts_with("CREATE TYPE") {
            self.parse_create_type(query)
        } else if query_upper.starts_with("CREATE MATERIALIZED VIEW") {
            self.parse_create_materialized_view(query)
        } else if query_upper.starts_with("DROP KEYSPACE") {
            self.parse_drop_keyspace(query)
        } else if query_upper.starts_with("DROP TABLE") {
            self.parse_drop_table(query)
        } else if query_upper.starts_with("USE") {
            self.parse_use(query)
        } else if query_upper.starts_with("BEGIN BATCH") {
            self.parse_batch(query)
        } else if query_upper.starts_with("TRUNCATE") {
            self.parse_truncate(query)
        } else {
            Err(ProtocolError::ParseError(format!(
                "Unsupported CQL statement: {}",
                query
            )))
        }
    }

    /// Parse SELECT statement
    fn parse_select(&self, query: &str) -> ProtocolResult<CqlStatement> {
        // Simplified parser - in production, use a proper parser like pest or nom
        let parts: Vec<&str> = query.split_whitespace().collect();

        if parts.len() < 4 {
            return Err(ProtocolError::ParseError(
                "Invalid SELECT statement".to_string(),
            ));
        }

        // Extract columns (SELECT columns FROM table)
        let from_index = parts
            .iter()
            .position(|&p| p.to_uppercase() == "FROM")
            .ok_or_else(|| ProtocolError::ParseError("Missing FROM clause".to_string()))?;

        let columns_str = parts[1..from_index].join(" ");
        let columns = if columns_str == "*" {
            vec!["*".to_string()]
        } else {
            columns_str
                .split(',')
                .map(|s| s.trim().to_string())
                .collect()
        };

        // Extract table name
        let table = parts
            .get(from_index + 1)
            .ok_or_else(|| ProtocolError::ParseError("Missing table name".to_string()))?
            .to_string();

        // Look for WHERE clause
        let where_clause =
            if let Some(where_index) = parts.iter().position(|&p| p.to_uppercase() == "WHERE") {
                Some(self.parse_where_clause(&parts[where_index + 1..])?)
            } else {
                None
            };

        // Look for LIMIT
        let limit =
            if let Some(limit_index) = parts.iter().position(|&p| p.to_uppercase() == "LIMIT") {
                parts
                    .get(limit_index + 1)
                    .and_then(|s| s.parse::<usize>().ok())
            } else {
                None
            };

        // Check for ALLOW FILTERING
        let allow_filtering = query.to_uppercase().contains("ALLOW FILTERING");

        Ok(CqlStatement::Select {
            columns,
            table: self.resolve_table_name(&table),
            where_clause,
            limit,
            allow_filtering,
        })
    }

    /// Parse INSERT statement
    fn parse_insert(&self, query: &str) -> ProtocolResult<CqlStatement> {
        // INSERT INTO table (col1, col2) VALUES (val1, val2) [USING TTL seconds]
        let query_upper = query.to_uppercase();
        let if_not_exists = query_upper.contains("IF NOT EXISTS");

        // Extract table name
        let into_index = query_upper
            .find("INTO ")
            .ok_or_else(|| ProtocolError::ParseError("Missing INTO clause".to_string()))?;

        let after_into = &query[into_index + 5..];
        let paren_index = after_into
            .find('(')
            .ok_or_else(|| ProtocolError::ParseError("Missing column list".to_string()))?;

        let table = after_into[..paren_index].trim().to_string();

        // Parse column list
        let col_start = paren_index + 1;
        let col_end = after_into[col_start..]
            .find(')')
            .ok_or_else(|| ProtocolError::ParseError("Unclosed column list".to_string()))?;

        let col_str = &after_into[col_start..col_start + col_end];
        let columns: Vec<String> = col_str.split(',').map(|s| s.trim().to_string()).collect();

        // Find VALUES clause
        let values_index = query_upper
            .find("VALUES")
            .ok_or_else(|| ProtocolError::ParseError("Missing VALUES clause".to_string()))?;

        let after_values = &query[values_index + 6..];
        let val_paren_start = after_values
            .find('(')
            .ok_or_else(|| ProtocolError::ParseError("Missing value list".to_string()))?;

        let val_start = val_paren_start + 1;
        let val_end = after_values[val_start..]
            .find(')')
            .ok_or_else(|| ProtocolError::ParseError("Unclosed value list".to_string()))?;

        let val_str = &after_values[val_start..val_start + val_end];

        // Parse values (handle quoted strings and numbers)
        let values: Vec<CqlValue> = self.parse_value_list(val_str)?;

        // Parse TTL if present
        let ttl = if query_upper.contains("USING TTL") {
            let ttl_index = query_upper.find("USING TTL").unwrap();
            let ttl_part = &query[ttl_index + 9..];
            let ttl_value = ttl_part
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<i32>().ok());
            ttl_value
        } else {
            None
        };

        Ok(CqlStatement::Insert {
            table: self.resolve_table_name(&table),
            columns,
            values,
            if_not_exists,
            ttl,
        })
    }

    /// Parse a list of values from a comma-separated string
    fn parse_value_list(&self, val_str: &str) -> ProtocolResult<Vec<CqlValue>> {
        let mut values = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut quote_char = '\0';
        let mut brace_depth = 0;

        for ch in val_str.chars() {
            match ch {
                '\'' | '"' if !in_quotes => {
                    in_quotes = true;
                    quote_char = ch;
                    current.push(ch);
                }
                c if c == quote_char && in_quotes => {
                    in_quotes = false;
                    quote_char = '\0';
                    current.push(ch);
                }
                '{' if !in_quotes => {
                    brace_depth += 1;
                    current.push(ch);
                }
                '}' if !in_quotes => {
                    brace_depth -= 1;
                    current.push(ch);
                }
                ',' if !in_quotes && brace_depth == 0 => {
                    if !current.trim().is_empty() {
                        values.push(self.parse_value(current.trim())?);
                    }
                    current.clear();
                }
                _ => {
                    current.push(ch);
                }
            }
        }

        // Add last value
        if !current.trim().is_empty() {
            values.push(self.parse_value(current.trim())?);
        }

        Ok(values)
    }

    /// Parse UPDATE statement
    fn parse_update(&self, query: &str) -> ProtocolResult<CqlStatement> {
        // UPDATE table SET col1 = val1, col2 = val2 WHERE ... [IF ...] [USING TTL seconds]
        let query_upper = query.to_uppercase();

        // Find table name (after UPDATE)
        let parts: Vec<&str> = query.split_whitespace().collect();
        if parts.len() < 2 {
            return Err(ProtocolError::ParseError("Missing table name".to_string()));
        }
        let table = self.resolve_table_name(parts[1]);

        // Find SET clause
        let set_index = parts
            .iter()
            .position(|&p| p.to_uppercase() == "SET")
            .ok_or_else(|| ProtocolError::ParseError("Missing SET clause".to_string()))?;

        // Parse assignments (col1 = val1, col2 = val2)
        let where_index = parts
            .iter()
            .position(|&p| p.to_uppercase() == "WHERE")
            .unwrap_or(parts.len());

        let set_part = &parts[set_index + 1..where_index];
        let set_str = set_part.join(" ");
        let assignments = self.parse_assignments(&set_str)?;

        // Parse WHERE clause
        let where_clause = if where_index < parts.len() {
            self.parse_where_clause(&parts[where_index + 1..])?
        } else {
            vec![]
        };

        // Parse IF clause (lightweight transaction)
        let if_clause = if query_upper.contains(" IF ") {
            let if_index = parts
                .iter()
                .position(|&p| p.to_uppercase() == "IF")
                .unwrap();
            Some(self.parse_where_clause(&parts[if_index + 1..])?)
        } else {
            None
        };

        // Parse TTL
        let ttl = if query_upper.contains("USING TTL") {
            let ttl_index = query_upper.find("USING TTL").unwrap();
            let ttl_part = &query[ttl_index + 9..];
            ttl_part
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<i32>().ok())
        } else {
            None
        };

        Ok(CqlStatement::Update {
            table,
            assignments,
            where_clause,
            if_clause,
            ttl,
        })
    }

    /// Parse SET assignments (col1 = val1, col2 = val2)
    fn parse_assignments(&self, set_str: &str) -> ProtocolResult<HashMap<String, CqlValue>> {
        let mut assignments = HashMap::new();
        let parts: Vec<&str> = set_str.split(',').collect();

        for part in parts {
            let trimmed = part.trim();
            if let Some(eq_index) = trimmed.find('=') {
                let column = trimmed[..eq_index].trim().to_string();
                let value_str = trimmed[eq_index + 1..].trim();
                let value = self.parse_value(value_str)?;
                assignments.insert(column, value);
            }
        }

        Ok(assignments)
    }

    /// Parse DELETE statement
    fn parse_delete(&self, query: &str) -> ProtocolResult<CqlStatement> {
        // DELETE [col1, col2] FROM table WHERE ... [IF ...]
        let query_upper = query.to_uppercase();
        let parts: Vec<&str> = query.split_whitespace().collect();

        // Check if columns are specified
        let columns = if parts.len() > 1 && parts[1].to_uppercase() != "FROM" {
            // DELETE col1, col2 FROM table
            let from_index = parts
                .iter()
                .position(|&p| p.to_uppercase() == "FROM")
                .ok_or_else(|| ProtocolError::ParseError("Missing FROM clause".to_string()))?;

            // Join the parts before FROM, then split by comma
            let columns_str = parts[1..from_index].join(" ");
            columns_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            vec![] // DELETE entire row
        };

        // Find FROM clause
        let from_index = parts
            .iter()
            .position(|&p| p.to_uppercase() == "FROM")
            .ok_or_else(|| ProtocolError::ParseError("Missing FROM clause".to_string()))?;

        if from_index + 1 >= parts.len() {
            return Err(ProtocolError::ParseError("Missing table name".to_string()));
        }

        let table = self.resolve_table_name(parts[from_index + 1]);

        // Parse WHERE clause
        let where_index = parts
            .iter()
            .position(|&p| p.to_uppercase() == "WHERE")
            .unwrap_or(parts.len());

        let where_clause = if where_index < parts.len() {
            self.parse_where_clause(&parts[where_index + 1..])?
        } else {
            vec![]
        };

        // Parse IF clause
        let if_clause = if query_upper.contains(" IF ") {
            let if_index = parts
                .iter()
                .position(|&p| p.to_uppercase() == "IF")
                .unwrap();
            Some(self.parse_where_clause(&parts[if_index + 1..])?)
        } else {
            None
        };

        Ok(CqlStatement::Delete {
            columns,
            table,
            where_clause,
            if_clause,
        })
    }

    /// Parse CREATE KEYSPACE statement
    fn parse_create_keyspace(&self, query: &str) -> ProtocolResult<CqlStatement> {
        let if_not_exists = query.to_uppercase().contains("IF NOT EXISTS");

        // Extract keyspace name
        let parts: Vec<&str> = query.split_whitespace().collect();
        let name_index = if if_not_exists { 5 } else { 3 };
        let name = parts
            .get(name_index)
            .ok_or_else(|| ProtocolError::ParseError("Missing keyspace name".to_string()))?
            .to_string();

        Ok(CqlStatement::CreateKeyspace {
            name,
            if_not_exists,
            replication: HashMap::new(),
            durable_writes: true,
        })
    }

    /// Parse CREATE TABLE statement
    /// Example: CREATE TABLE IF NOT EXISTS users (user_id UUID PRIMARY KEY, first_name text)
    fn parse_create_table(&self, query: &str) -> ProtocolResult<CqlStatement> {
        let query_upper = query.to_uppercase();
        let if_not_exists = query_upper.contains("IF NOT EXISTS");

        // Parse table name - format: CREATE TABLE [IF NOT EXISTS] table_name (columns...)
        let parts: Vec<&str> = query.split_whitespace().collect();

        // Find table name (after CREATE TABLE or CREATE TABLE IF NOT EXISTS)
        let name_index = if if_not_exists { 5 } else { 2 };
        let raw_table_name = parts
            .get(name_index)
            .map(|s| {
                // Remove trailing parenthesis if present
                if let Some(idx) = s.find('(') {
                    &s[..idx]
                } else {
                    *s
                }
            })
            .unwrap_or("unknown_table");

        // Resolve table name with current keyspace
        let table_name = self.resolve_table_name(raw_table_name);

        Ok(CqlStatement::CreateTable {
            name: table_name,
            if_not_exists,
            columns: vec![],
            primary_key: vec![],
            clustering_key: vec![],
            options: HashMap::new(),
        })
    }

    /// Parse CREATE INDEX statement
    /// Example: CREATE INDEX IF NOT EXISTS user_last_name ON users (last_name)
    fn parse_create_index(&self, query: &str) -> ProtocolResult<CqlStatement> {
        let query_upper = query.to_uppercase();
        let if_not_exists = query_upper.contains("IF NOT EXISTS");

        // Parse index name and table/column
        // Format: CREATE INDEX [IF NOT EXISTS] index_name ON table_name (column_name)
        let parts: Vec<&str> = query.split_whitespace().collect();

        // Find the index name (after CREATE INDEX or CREATE INDEX IF NOT EXISTS)
        let name_index = if if_not_exists { 5 } else { 2 };
        let name = parts
            .get(name_index)
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unnamed_index".to_string());

        // Find ON keyword to get table name
        let on_index = parts
            .iter()
            .position(|&p| p.to_uppercase() == "ON")
            .unwrap_or(parts.len());

        let table = parts
            .get(on_index + 1)
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown_table".to_string());

        // Extract column name from parentheses
        let column = if let Some(start) = query.find('(') {
            if let Some(end) = query.find(')') {
                query[start + 1..end].trim().to_string()
            } else {
                "unknown_column".to_string()
            }
        } else {
            "unknown_column".to_string()
        };

        Ok(CqlStatement::CreateIndex {
            name,
            if_not_exists,
            table: self.resolve_table_name(&table),
            column,
        })
    }

    /// Parse CREATE TYPE statement (User-Defined Types)
    /// Example: CREATE TYPE IF NOT EXISTS address (street text, city text, zip int)
    fn parse_create_type(&self, query: &str) -> ProtocolResult<CqlStatement> {
        let query_upper = query.to_uppercase();
        let if_not_exists = query_upper.contains("IF NOT EXISTS");

        let parts: Vec<&str> = query.split_whitespace().collect();

        // Find the type name
        let name_index = if if_not_exists { 5 } else { 2 };
        let name = parts
            .get(name_index)
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unnamed_type".to_string());

        // Parse fields from parentheses - simplified parsing
        let fields = if let Some(start) = query.find('(') {
            if let Some(end) = query.rfind(')') {
                let fields_str = &query[start + 1..end];
                self.parse_type_fields(fields_str)
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        Ok(CqlStatement::CreateType {
            name: self.resolve_table_name(&name),
            if_not_exists,
            fields,
        })
    }

    /// Parse type fields for CREATE TYPE
    fn parse_type_fields(&self, fields_str: &str) -> Vec<(String, CqlType)> {
        let mut fields = Vec::new();
        for field in fields_str.split(',') {
            let parts: Vec<&str> = field.trim().split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[0].to_string();
                let type_str = parts[1].to_uppercase();
                let cql_type = match type_str.as_str() {
                    "TEXT" | "VARCHAR" => CqlType::Text,
                    "INT" => CqlType::Int,
                    "BIGINT" => CqlType::Bigint,
                    "BOOLEAN" => CqlType::Boolean,
                    "FLOAT" => CqlType::Float,
                    "DOUBLE" => CqlType::Double,
                    "TIMESTAMP" => CqlType::Timestamp,
                    "UUID" => CqlType::Uuid,
                    _ => CqlType::Text, // Default to text
                };
                fields.push((name, cql_type));
            }
        }
        fields
    }

    /// Parse CREATE MATERIALIZED VIEW statement
    /// Example: CREATE MATERIALIZED VIEW IF NOT EXISTS users_by_email AS
    ///          SELECT * FROM users WHERE emails IS NOT NULL AND user_id IS NOT NULL
    ///          PRIMARY KEY (emails, user_id)
    fn parse_create_materialized_view(&self, query: &str) -> ProtocolResult<CqlStatement> {
        let query_upper = query.to_uppercase();
        let if_not_exists = query_upper.contains("IF NOT EXISTS");

        let parts: Vec<&str> = query.split_whitespace().collect();

        // Find view name (after CREATE MATERIALIZED VIEW or CREATE MATERIALIZED VIEW IF NOT EXISTS)
        let name_index = if if_not_exists { 6 } else { 3 };
        let name = parts
            .get(name_index)
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unnamed_view".to_string());

        // Find AS keyword to get SELECT clause
        let as_index = parts
            .iter()
            .position(|&p| p.to_uppercase() == "AS")
            .unwrap_or(parts.len());

        // Find FROM keyword to get source table
        let from_index = parts
            .iter()
            .position(|&p| p.to_uppercase() == "FROM")
            .unwrap_or(parts.len());

        let source_table = parts
            .get(from_index + 1)
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown_table".to_string());

        // Extract columns (simplified - just support * for now)
        let columns = if as_index + 2 < from_index {
            parts[as_index + 2..from_index]
                .iter()
                .map(|s| s.replace(',', "").to_string())
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            vec!["*".to_string()]
        };

        // Extract WHERE clause (simplified)
        let where_clause = if let Some(where_pos) = query_upper.find(" WHERE ") {
            if let Some(pk_pos) = query_upper.find("PRIMARY KEY") {
                Some(query[where_pos + 7..pk_pos].trim().to_string())
            } else {
                Some(query[where_pos + 7..].trim().to_string())
            }
        } else {
            None
        };

        // Extract PRIMARY KEY
        let primary_key = if let Some(pk_start) = query.find("PRIMARY KEY") {
            let pk_part = &query[pk_start..];
            if let Some(start) = pk_part.find('(') {
                if let Some(end) = pk_part.find(')') {
                    pk_part[start + 1..end]
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect()
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        Ok(CqlStatement::CreateMaterializedView {
            name: self.resolve_table_name(&name),
            if_not_exists,
            source_table: self.resolve_table_name(&source_table),
            columns,
            where_clause,
            primary_key,
        })
    }

    /// Parse DROP KEYSPACE statement
    fn parse_drop_keyspace(&self, query: &str) -> ProtocolResult<CqlStatement> {
        let if_exists = query.to_uppercase().contains("IF EXISTS");
        let parts: Vec<&str> = query.split_whitespace().collect();
        let name_index = if if_exists { 4 } else { 2 };
        let name = parts
            .get(name_index)
            .ok_or_else(|| ProtocolError::ParseError("Missing keyspace name".to_string()))?
            .to_string();

        Ok(CqlStatement::DropKeyspace { name, if_exists })
    }

    /// Parse DROP TABLE statement
    fn parse_drop_table(&self, query: &str) -> ProtocolResult<CqlStatement> {
        let if_exists = query.to_uppercase().contains("IF EXISTS");
        let parts: Vec<&str> = query.split_whitespace().collect();
        let name_index = if if_exists { 4 } else { 2 };
        let name = parts
            .get(name_index)
            .ok_or_else(|| ProtocolError::ParseError("Missing table name".to_string()))?
            .to_string();

        Ok(CqlStatement::DropTable {
            name: self.resolve_table_name(&name),
            if_exists,
        })
    }

    /// Parse USE statement
    fn parse_use(&self, query: &str) -> ProtocolResult<CqlStatement> {
        let parts: Vec<&str> = query.split_whitespace().collect();
        let raw_keyspace = parts
            .get(1)
            .ok_or_else(|| ProtocolError::ParseError("Missing keyspace name".to_string()))?;

        // Strip surrounding quotes if present
        let keyspace = raw_keyspace
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();

        Ok(CqlStatement::Use { keyspace })
    }

    /// Parse BATCH statement
    fn parse_batch(&self, _query: &str) -> ProtocolResult<CqlStatement> {
        // Simplified implementation
        Ok(CqlStatement::Batch {
            batch_type: BatchType::Logged,
            statements: vec![],
        })
    }

    /// Parse TRUNCATE statement
    fn parse_truncate(&self, query: &str) -> ProtocolResult<CqlStatement> {
        let parts: Vec<&str> = query.split_whitespace().collect();
        let table = parts
            .get(1)
            .ok_or_else(|| ProtocolError::ParseError("Missing table name".to_string()))?
            .to_string();

        Ok(CqlStatement::Truncate {
            table: self.resolve_table_name(&table),
        })
    }

    /// Parse WHERE clause
    fn parse_where_clause(&self, parts: &[&str]) -> ProtocolResult<Vec<WhereCondition>> {
        let mut conditions = Vec::new();
        let mut i = 0;

        while i < parts.len() {
            // Skip AND/OR keywords for now (we'll support them later)
            if parts[i].to_uppercase() == "AND" || parts[i].to_uppercase() == "OR" {
                i += 1;
                continue;
            }

            // Column name
            let column = parts[i].to_string();
            i += 1;

            if i >= parts.len() {
                break;
            }

            // Operator
            let operator = match parts[i] {
                "=" => ComparisonOperator::Equal,
                ">" => ComparisonOperator::GreaterThan,
                ">=" => ComparisonOperator::GreaterThanOrEqual,
                "<" => ComparisonOperator::LessThan,
                "<=" => ComparisonOperator::LessThanOrEqual,
                "!=" | "<>" => ComparisonOperator::NotEqual,
                "IN" => {
                    i += 1;
                    // Parse IN (value1, value2, ...)
                    // Handle case where "(" might be in the same token or separate
                    let mut values = Vec::new();
                    if i < parts.len() {
                        let mut current_part = parts[i];
                        // Check if current part starts with "("
                        if current_part.starts_with('(') {
                            // Remove opening paren
                            current_part = &current_part[1..];
                        } else if current_part != "(" {
                            return Err(ProtocolError::ParseError(
                                "Expected '(' after IN".to_string(),
                            ));
                        }

                        // Parse values
                        loop {
                            // Remove any leading/trailing commas or parens
                            let cleaned = current_part
                                .trim_matches(|c: char| c == ',' || c == '(' || c == ')');
                            if !cleaned.is_empty() {
                                // Try to parse as number first, then text
                                let value = if let Ok(int_val) = cleaned.parse::<i32>() {
                                    CqlValue::Int(int_val)
                                } else if let Ok(bigint_val) = cleaned.parse::<i64>() {
                                    CqlValue::Bigint(bigint_val)
                                } else {
                                    let text_val = cleaned.trim_matches('\'').to_string();
                                    CqlValue::Text(text_val)
                                };
                                values.push(value);
                            }

                            // Check if we've reached the end
                            if current_part.contains(')') {
                                break;
                            }

                            i += 1;
                            if i >= parts.len() {
                                break;
                            }
                            current_part = parts[i];
                        }
                    }

                    // For IN, we'll create a condition with the first value
                    // In a full implementation, we'd handle IN properly with all values
                    conditions.push(WhereCondition {
                        column,
                        operator: ComparisonOperator::In,
                        value: values.first().cloned().unwrap_or(CqlValue::Null),
                    });
                    continue;
                }
                "CONTAINS" => ComparisonOperator::Contains,
                "CONTAINS KEY" => {
                    i += 1; // Skip KEY
                    ComparisonOperator::ContainsKey
                }
                _ => {
                    return Err(ProtocolError::ParseError(format!(
                        "Unknown operator: {}",
                        parts[i]
                    )));
                }
            };
            i += 1;

            if i >= parts.len() {
                return Err(ProtocolError::ParseError(
                    "Missing value after operator".to_string(),
                ));
            }

            // Value
            let value_str = parts[i];
            let value = self.parse_value(value_str)?;
            i += 1;

            conditions.push(WhereCondition {
                column,
                operator,
                value,
            });
        }

        Ok(conditions)
    }

    /// Parse a CQL value from string
    fn parse_value(&self, value_str: &str) -> ProtocolResult<CqlValue> {
        let trimmed = value_str.trim();

        // Handle NULL
        if trimmed.to_uppercase() == "NULL" {
            return Ok(CqlValue::Null);
        }

        // Handle boolean
        if trimmed.to_uppercase() == "TRUE" {
            return Ok(CqlValue::Boolean(true));
        }
        if trimmed.to_uppercase() == "FALSE" {
            return Ok(CqlValue::Boolean(false));
        }

        // Handle set/list literals like {'value1', 'value2'} or ['value1', 'value2']
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            let inner = &trimmed[1..trimmed.len() - 1];
            if inner.is_empty() {
                return Ok(CqlValue::Set(vec![]));
            }
            let items = self.parse_value_list(inner)?;
            return Ok(CqlValue::Set(items));
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let inner = &trimmed[1..trimmed.len() - 1];
            if inner.is_empty() {
                return Ok(CqlValue::List(vec![]));
            }
            let items = self.parse_value_list(inner)?;
            return Ok(CqlValue::List(items));
        }

        // Handle CQL functions like uuid(), now(), etc.
        let trimmed_lower = trimmed.to_lowercase();
        if trimmed_lower == "uuid()" {
            // Generate a new UUID
            let uuid = uuid::Uuid::new_v4();
            return Ok(CqlValue::Uuid(uuid.to_string()));
        }
        if trimmed_lower == "now()" || trimmed_lower == "currenttimestamp()" {
            // Return current timestamp in milliseconds
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            return Ok(CqlValue::Timestamp(now));
        }
        if trimmed_lower == "timeuuid()" || trimmed_lower == "currenttimeuuid()" {
            // For timeuuid, we'll generate a regular UUID for now
            // A proper implementation would use time-based UUID (v1)
            let uuid = uuid::Uuid::new_v4();
            return Ok(CqlValue::Uuid(uuid.to_string()));
        }

        // Handle quoted strings
        if (trimmed.starts_with('\'') && trimmed.ends_with('\''))
            || (trimmed.starts_with('"') && trimmed.ends_with('"'))
        {
            let unquoted = &trimmed[1..trimmed.len() - 1];
            return Ok(CqlValue::Text(unquoted.to_string()));
        }

        // Handle numbers
        if let Ok(int_val) = trimmed.parse::<i32>() {
            return Ok(CqlValue::Int(int_val));
        }
        if let Ok(bigint_val) = trimmed.parse::<i64>() {
            return Ok(CqlValue::Bigint(bigint_val));
        }
        if let Ok(float_val) = trimmed.parse::<f32>() {
            return Ok(CqlValue::Float(float_val));
        }
        if let Ok(double_val) = trimmed.parse::<f64>() {
            return Ok(CqlValue::Double(double_val));
        }

        // Default to text
        Ok(CqlValue::Text(trimmed.to_string()))
    }

    /// Resolve table name with current keyspace
    fn resolve_table_name(&self, table: &str) -> String {
        if table.contains('.') {
            table.to_string()
        } else if let Some(keyspace) = &self.current_keyspace {
            format!("{}.{}", keyspace, table)
        } else {
            table.to_string()
        }
    }
}

impl Default for CqlParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_select() {
        let parser = CqlParser::new();
        let stmt = parser.parse("SELECT * FROM users").unwrap();

        match stmt {
            CqlStatement::Select { columns, table, .. } => {
                assert_eq!(columns, vec!["*"]);
                assert_eq!(table, "users");
            }
            _ => panic!("Expected SELECT statement"),
        }
    }

    #[test]
    fn test_parse_use() {
        let parser = CqlParser::new();
        let stmt = parser.parse("USE my_keyspace").unwrap();

        match stmt {
            CqlStatement::Use { keyspace } => {
                assert_eq!(keyspace, "my_keyspace");
            }
            _ => panic!("Expected USE statement"),
        }
    }

    #[test]
    fn test_parse_create_keyspace() {
        let parser = CqlParser::new();
        let stmt = parser
            .parse("CREATE KEYSPACE IF NOT EXISTS my_ks WITH REPLICATION = {'class': 'SimpleStrategy'}")
            .unwrap();

        match stmt {
            CqlStatement::CreateKeyspace {
                name,
                if_not_exists,
                ..
            } => {
                assert_eq!(name, "my_ks");
                assert!(if_not_exists);
            }
            _ => panic!("Expected CREATE KEYSPACE statement"),
        }
    }
}
