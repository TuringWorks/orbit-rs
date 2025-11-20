//! CQL statement parser
//!
//! This module provides a parser for CQL (Cassandra Query Language) statements.

use super::types::{CqlType, CqlValue};
use crate::error::{ProtocolError, ProtocolResult};
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
        let from_index = parts.iter().position(|&p| p.to_uppercase() == "FROM")
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
        let table = parts.get(from_index + 1)
            .ok_or_else(|| ProtocolError::ParseError("Missing table name".to_string()))?
            .to_string();

        // Look for WHERE clause
        let where_clause = if let Some(where_index) = parts.iter().position(|&p| p.to_uppercase() == "WHERE") {
            Some(self.parse_where_clause(&parts[where_index + 1..])?)
        } else {
            None
        };

        // Look for LIMIT
        let limit = if let Some(limit_index) = parts.iter().position(|&p| p.to_uppercase() == "LIMIT") {
            parts.get(limit_index + 1)
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
        // Simplified: INSERT INTO table (col1, col2) VALUES (val1, val2)
        let if_not_exists = query.to_uppercase().contains("IF NOT EXISTS");

        // Extract table name
        let into_index = query.to_uppercase().find("INTO ")
            .ok_or_else(|| ProtocolError::ParseError("Missing INTO clause".to_string()))?;

        let after_into = &query[into_index + 5..];
        let paren_index = after_into.find('(')
            .ok_or_else(|| ProtocolError::ParseError("Missing column list".to_string()))?;

        let table = after_into[..paren_index].trim().to_string();

        // For now, return a simple INSERT structure
        // In production, properly parse column names and values
        Ok(CqlStatement::Insert {
            table: self.resolve_table_name(&table),
            columns: vec![],
            values: vec![],
            if_not_exists,
            ttl: None,
        })
    }

    /// Parse UPDATE statement
    fn parse_update(&self, _query: &str) -> ProtocolResult<CqlStatement> {
        // Simplified implementation
        Ok(CqlStatement::Update {
            table: "temp".to_string(),
            assignments: HashMap::new(),
            where_clause: vec![],
            if_clause: None,
            ttl: None,
        })
    }

    /// Parse DELETE statement
    fn parse_delete(&self, _query: &str) -> ProtocolResult<CqlStatement> {
        // Simplified implementation
        Ok(CqlStatement::Delete {
            columns: vec![],
            table: "temp".to_string(),
            where_clause: vec![],
            if_clause: None,
        })
    }

    /// Parse CREATE KEYSPACE statement
    fn parse_create_keyspace(&self, query: &str) -> ProtocolResult<CqlStatement> {
        let if_not_exists = query.to_uppercase().contains("IF NOT EXISTS");

        // Extract keyspace name
        let parts: Vec<&str> = query.split_whitespace().collect();
        let name_index = if if_not_exists { 5 } else { 3 };
        let name = parts.get(name_index)
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
    fn parse_create_table(&self, query: &str) -> ProtocolResult<CqlStatement> {
        let if_not_exists = query.to_uppercase().contains("IF NOT EXISTS");

        // Simplified implementation
        Ok(CqlStatement::CreateTable {
            name: "temp_table".to_string(),
            if_not_exists,
            columns: vec![],
            primary_key: vec![],
            clustering_key: vec![],
            options: HashMap::new(),
        })
    }

    /// Parse DROP KEYSPACE statement
    fn parse_drop_keyspace(&self, query: &str) -> ProtocolResult<CqlStatement> {
        let if_exists = query.to_uppercase().contains("IF EXISTS");
        let parts: Vec<&str> = query.split_whitespace().collect();
        let name_index = if if_exists { 4 } else { 2 };
        let name = parts.get(name_index)
            .ok_or_else(|| ProtocolError::ParseError("Missing keyspace name".to_string()))?
            .to_string();

        Ok(CqlStatement::DropKeyspace { name, if_exists })
    }

    /// Parse DROP TABLE statement
    fn parse_drop_table(&self, query: &str) -> ProtocolResult<CqlStatement> {
        let if_exists = query.to_uppercase().contains("IF EXISTS");
        let parts: Vec<&str> = query.split_whitespace().collect();
        let name_index = if if_exists { 4 } else { 2 };
        let name = parts.get(name_index)
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
        let keyspace = parts.get(1)
            .ok_or_else(|| ProtocolError::ParseError("Missing keyspace name".to_string()))?
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
        let table = parts.get(1)
            .ok_or_else(|| ProtocolError::ParseError("Missing table name".to_string()))?
            .to_string();

        Ok(CqlStatement::Truncate {
            table: self.resolve_table_name(&table),
        })
    }

    /// Parse WHERE clause
    fn parse_where_clause(&self, _parts: &[&str]) -> ProtocolResult<Vec<WhereCondition>> {
        // Simplified implementation
        Ok(vec![])
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
            CqlStatement::CreateKeyspace { name, if_not_exists, .. } => {
                assert_eq!(name, "my_ks");
                assert!(if_not_exists);
            }
            _ => panic!("Expected CREATE KEYSPACE statement"),
        }
    }
}
