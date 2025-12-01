//! Universal operations for cross-protocol storage
//!
//! This module defines the operations that any protocol can express,
//! providing a common language for data manipulation across Redis, SQL,
//! Cassandra, Cypher, AQL, and REST protocols.

use super::types::{RecordId, UniversalValue};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Operations that any protocol can express
///
/// This enum represents all possible data operations in a protocol-agnostic way.
/// Each protocol adapter translates its native commands into these universal operations.
#[derive(Debug, Clone)]
pub enum UniversalOperation {
    // ============================================================================
    // Basic CRUD Operations
    // ============================================================================
    /// Get a single record by key
    Get { namespace: String, key: String },

    /// Insert or update a record
    Put {
        namespace: String,
        key: String,
        value: UniversalValue,
        /// Optional TTL for the record
        ttl: Option<Duration>,
        /// If true, only insert if key doesn't exist (like Redis SETNX)
        if_not_exists: bool,
        /// If Some, only update if version matches (optimistic locking)
        if_version: Option<u64>,
    },

    /// Delete a record
    Delete { namespace: String, key: String },

    /// Check if a key exists
    Exists { namespace: String, key: String },

    // ============================================================================
    // Batch Operations
    // ============================================================================
    /// Get multiple records by keys
    MultiGet {
        namespace: String,
        keys: Vec<String>,
    },

    /// Insert or update multiple records atomically
    MultiPut {
        records: Vec<(RecordId, UniversalValue)>,
    },

    /// Delete multiple records
    MultiDelete {
        namespace: String,
        keys: Vec<String>,
    },

    // ============================================================================
    // Scan/Query Operations
    // ============================================================================
    /// Scan records with optional filtering
    Scan {
        namespace: String,
        /// Filter expression (WHERE clause equivalent)
        filter: Option<FilterExpression>,
        /// Maximum number of records to return
        limit: Option<usize>,
        /// Number of records to skip
        offset: Option<usize>,
        /// Fields to order by
        order_by: Option<Vec<(String, SortOrder)>>,
        /// Fields to return (empty = all fields)
        projection: Vec<String>,
    },

    /// Scan keys matching a pattern (like Redis KEYS/SCAN)
    ScanKeys {
        namespace: String,
        /// Pattern with wildcards (e.g., "user:*")
        pattern: Option<String>,
        limit: Option<usize>,
        cursor: Option<String>,
    },

    // ============================================================================
    // Aggregation Operations
    // ============================================================================
    /// Aggregate records with grouping
    Aggregate {
        namespace: String,
        filter: Option<FilterExpression>,
        group_by: Vec<String>,
        aggregations: Vec<AggregateOp>,
        having: Option<FilterExpression>,
        order_by: Option<Vec<(String, SortOrder)>>,
        limit: Option<usize>,
    },

    /// Count records matching a filter
    Count {
        namespace: String,
        filter: Option<FilterExpression>,
    },

    // ============================================================================
    // Field-Level Operations (for Map values)
    // ============================================================================
    /// Get a specific field from a record (like Redis HGET)
    GetField {
        namespace: String,
        key: String,
        field: String,
    },

    /// Set a specific field in a record (like Redis HSET)
    SetField {
        namespace: String,
        key: String,
        field: String,
        value: UniversalValue,
    },

    /// Delete a field from a record (like Redis HDEL)
    DeleteField {
        namespace: String,
        key: String,
        field: String,
    },

    /// Increment a numeric field (like Redis HINCRBY)
    IncrementField {
        namespace: String,
        key: String,
        field: String,
        delta: i64,
    },

    // ============================================================================
    // List Operations (for List values)
    // ============================================================================
    /// Push to the front of a list (like Redis LPUSH)
    ListPushFront {
        namespace: String,
        key: String,
        values: Vec<UniversalValue>,
    },

    /// Push to the back of a list (like Redis RPUSH)
    ListPushBack {
        namespace: String,
        key: String,
        values: Vec<UniversalValue>,
    },

    /// Pop from the front of a list (like Redis LPOP)
    ListPopFront {
        namespace: String,
        key: String,
        count: Option<usize>,
    },

    /// Pop from the back of a list (like Redis RPOP)
    ListPopBack {
        namespace: String,
        key: String,
        count: Option<usize>,
    },

    /// Get a range of list elements (like Redis LRANGE)
    ListRange {
        namespace: String,
        key: String,
        start: i64,
        stop: i64,
    },

    /// Get list length (like Redis LLEN)
    ListLength { namespace: String, key: String },

    // ============================================================================
    // Set Operations (for Set values)
    // ============================================================================
    /// Add members to a set (like Redis SADD)
    SetAdd {
        namespace: String,
        key: String,
        members: Vec<UniversalValue>,
    },

    /// Remove members from a set (like Redis SREM)
    SetRemove {
        namespace: String,
        key: String,
        members: Vec<UniversalValue>,
    },

    /// Check if a member is in a set (like Redis SISMEMBER)
    SetIsMember {
        namespace: String,
        key: String,
        member: UniversalValue,
    },

    /// Get all members of a set (like Redis SMEMBERS)
    SetMembers { namespace: String, key: String },

    // ============================================================================
    // Sorted Set Operations (for SortedSet values)
    // ============================================================================
    /// Add members with scores to a sorted set (like Redis ZADD)
    SortedSetAdd {
        namespace: String,
        key: String,
        members: Vec<(UniversalValue, f64)>,
    },

    /// Get members by score range (like Redis ZRANGEBYSCORE)
    SortedSetRangeByScore {
        namespace: String,
        key: String,
        min: f64,
        max: f64,
        limit: Option<usize>,
        offset: Option<usize>,
    },

    /// Get members by rank range (like Redis ZRANGE)
    SortedSetRangeByRank {
        namespace: String,
        key: String,
        start: i64,
        stop: i64,
        with_scores: bool,
    },

    // ============================================================================
    // Graph Operations (for Cypher/AQL)
    // ============================================================================
    /// Create a graph node
    CreateNode {
        labels: Vec<String>,
        properties: std::collections::BTreeMap<String, UniversalValue>,
    },

    /// Create a relationship between nodes
    CreateRelationship {
        start_node: String,
        end_node: String,
        rel_type: String,
        properties: std::collections::BTreeMap<String, UniversalValue>,
    },

    /// Traverse the graph following a pattern
    TraverseGraph {
        /// Starting node IDs
        start_nodes: Vec<String>,
        /// Graph pattern to match
        pattern: GraphPattern,
        /// Maximum traversal depth
        max_depth: Option<usize>,
    },

    /// Find shortest path between nodes
    ShortestPath {
        start_node: String,
        end_node: String,
        relationship_types: Option<Vec<String>>,
        max_depth: Option<usize>,
    },

    // ============================================================================
    // Transaction Operations
    // ============================================================================
    /// Begin a new transaction
    BeginTransaction { isolation_level: IsolationLevel },

    /// Commit a transaction
    Commit { tx_id: String },

    /// Rollback a transaction
    Rollback { tx_id: String },

    // ============================================================================
    // Schema Operations
    // ============================================================================
    /// Create a new namespace/table
    CreateNamespace {
        namespace: String,
        schema: Option<NamespaceSchema>,
    },

    /// Drop a namespace/table
    DropNamespace { namespace: String, if_exists: bool },

    /// Create an index
    CreateIndex {
        namespace: String,
        index_name: String,
        fields: Vec<String>,
        unique: bool,
    },

    /// Drop an index
    DropIndex {
        namespace: String,
        index_name: String,
    },

    // ============================================================================
    // TTL Operations
    // ============================================================================
    /// Set TTL on a key (like Redis EXPIRE)
    SetTTL {
        namespace: String,
        key: String,
        ttl: Duration,
    },

    /// Get remaining TTL (like Redis TTL)
    GetTTL { namespace: String, key: String },

    /// Remove TTL from a key (like Redis PERSIST)
    RemoveTTL { namespace: String, key: String },
}

/// Filter expression for queries (WHERE clause equivalent)
#[derive(Debug, Clone)]
pub enum FilterExpression {
    // Comparison operators
    /// Equals: field = value
    Eq(String, UniversalValue),
    /// Not equals: field != value
    Ne(String, UniversalValue),
    /// Greater than: field > value
    Gt(String, UniversalValue),
    /// Greater than or equal: field >= value
    Gte(String, UniversalValue),
    /// Less than: field < value
    Lt(String, UniversalValue),
    /// Less than or equal: field <= value
    Lte(String, UniversalValue),

    // Set operators
    /// IN: field IN (value1, value2, ...)
    In(String, Vec<UniversalValue>),
    /// NOT IN: field NOT IN (value1, value2, ...)
    NotIn(String, Vec<UniversalValue>),
    /// BETWEEN: field BETWEEN min AND max
    Between(String, UniversalValue, UniversalValue),

    // String operators
    /// LIKE: field LIKE pattern (SQL-style pattern matching)
    Like(String, String),
    /// ILIKE: case-insensitive LIKE
    ILike(String, String),
    /// Starts with prefix
    StartsWith(String, String),
    /// Ends with suffix
    EndsWith(String, String),
    /// Contains substring
    Contains(String, String),
    /// Matches regex
    Regex(String, String),

    // Null checks
    /// IS NULL
    IsNull(String),
    /// IS NOT NULL
    IsNotNull(String),

    // Array/List operators
    /// Array contains element
    ArrayContains(String, UniversalValue),
    /// Array contains all elements
    ArrayContainsAll(String, Vec<UniversalValue>),
    /// Array contains any element
    ArrayContainsAny(String, Vec<UniversalValue>),
    /// Array length equals
    ArrayLength(String, usize),

    // Logical operators
    /// AND: expr1 AND expr2
    And(Box<FilterExpression>, Box<FilterExpression>),
    /// OR: expr1 OR expr2
    Or(Box<FilterExpression>, Box<FilterExpression>),
    /// NOT: NOT expr
    Not(Box<FilterExpression>),

    // Geospatial operators
    /// Within distance of a point
    GeoWithinDistance {
        field: String,
        lat: f64,
        lon: f64,
        distance_meters: f64,
    },
    /// Within a bounding box
    GeoWithinBox {
        field: String,
        min_lat: f64,
        min_lon: f64,
        max_lat: f64,
        max_lon: f64,
    },

    // Full-text search
    /// Full-text search match
    FullTextMatch(String, String),
}

impl FilterExpression {
    /// Create an AND expression
    pub fn and(self, other: FilterExpression) -> FilterExpression {
        FilterExpression::And(Box::new(self), Box::new(other))
    }

    /// Create an OR expression
    pub fn or(self, other: FilterExpression) -> FilterExpression {
        FilterExpression::Or(Box::new(self), Box::new(other))
    }

    /// Create a NOT expression
    pub fn not(self) -> FilterExpression {
        FilterExpression::Not(Box::new(self))
    }
}

/// Sort order for ORDER BY clauses
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl Default for SortOrder {
    fn default() -> Self {
        SortOrder::Ascending
    }
}

/// Aggregation operations
#[derive(Debug, Clone)]
pub enum AggregateOp {
    /// COUNT(*)
    Count,
    /// COUNT(field)
    CountField(String),
    /// COUNT(DISTINCT field)
    CountDistinct(String),
    /// SUM(field)
    Sum(String),
    /// AVG(field)
    Avg(String),
    /// MIN(field)
    Min(String),
    /// MAX(field)
    Max(String),
    /// COLLECT(field) - collect values into a list
    Collect(String),
    /// FIRST(field) - first value in group
    First(String),
    /// LAST(field) - last value in group
    Last(String),
    /// Array aggregation
    ArrayAgg(String),
    /// String concatenation
    StringAgg { field: String, delimiter: String },
}

/// Graph pattern for traversal queries
#[derive(Debug, Clone)]
pub struct GraphPattern {
    /// Node patterns in the match
    pub nodes: Vec<NodePattern>,
    /// Relationship patterns connecting nodes
    pub relationships: Vec<RelationshipPattern>,
    /// Return all paths or just one
    pub all_paths: bool,
}

/// Pattern for matching nodes
#[derive(Debug, Clone)]
pub struct NodePattern {
    /// Variable name for the node
    pub variable: String,
    /// Required labels
    pub labels: Vec<String>,
    /// Property filters
    pub properties: Option<std::collections::BTreeMap<String, UniversalValue>>,
}

/// Pattern for matching relationships
#[derive(Debug, Clone)]
pub struct RelationshipPattern {
    /// Variable name for the relationship
    pub variable: Option<String>,
    /// Relationship types (empty = any type)
    pub types: Vec<String>,
    /// Direction
    pub direction: RelationshipDirection,
    /// Variable-length path bounds (min, max)
    pub variable_length: Option<(usize, Option<usize>)>,
    /// Source node variable
    pub from_node: String,
    /// Target node variable
    pub to_node: String,
}

/// Direction of a relationship
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipDirection {
    /// Outgoing: (a)-[r]->(b)
    Outgoing,
    /// Incoming: (a)<-[r]-(b)
    Incoming,
    /// Both directions: (a)-[r]-(b)
    Both,
}

/// Transaction isolation level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

impl Default for IsolationLevel {
    fn default() -> Self {
        IsolationLevel::ReadCommitted
    }
}

/// Schema definition for a namespace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceSchema {
    /// Field definitions
    pub fields: Vec<FieldDefinition>,
    /// Primary key fields
    pub primary_key: Vec<String>,
    /// Indexes
    pub indexes: Vec<IndexDefinition>,
}

/// Field definition in a schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDefinition {
    pub name: String,
    pub field_type: FieldType,
    pub nullable: bool,
    pub default: Option<UniversalValue>,
}

/// Field type in a schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldType {
    Bool,
    Int,
    Float,
    String,
    Bytes,
    Timestamp,
    Date,
    Time,
    Json,
    List(Box<FieldType>),
    Map(Box<FieldType>, Box<FieldType>),
    Vector(usize),
    Uuid,
}

/// Index definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDefinition {
    pub name: String,
    pub fields: Vec<String>,
    pub unique: bool,
    pub index_type: IndexType,
}

/// Type of index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexType {
    BTree,
    Hash,
    FullText,
    Geospatial,
    Vector,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_expression_composition() {
        let filter = FilterExpression::Eq("name".to_string(), "Alice".into())
            .and(FilterExpression::Gt("age".to_string(), 18i64.into()))
            .or(FilterExpression::IsNull("deleted_at".to_string()));

        match filter {
            FilterExpression::Or(_, _) => {}
            _ => panic!("Expected Or expression"),
        }
    }

    #[test]
    fn test_graph_pattern() {
        let pattern = GraphPattern {
            nodes: vec![
                NodePattern {
                    variable: "a".to_string(),
                    labels: vec!["Person".to_string()],
                    properties: None,
                },
                NodePattern {
                    variable: "b".to_string(),
                    labels: vec!["Person".to_string()],
                    properties: None,
                },
            ],
            relationships: vec![RelationshipPattern {
                variable: Some("r".to_string()),
                types: vec!["KNOWS".to_string()],
                direction: RelationshipDirection::Outgoing,
                variable_length: Some((1, Some(3))),
                from_node: "a".to_string(),
                to_node: "b".to_string(),
            }],
            all_paths: false,
        };

        assert_eq!(pattern.nodes.len(), 2);
        assert_eq!(pattern.relationships.len(), 1);
    }
}
