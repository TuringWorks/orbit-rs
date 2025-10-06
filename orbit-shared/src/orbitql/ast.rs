//! Abstract Syntax Tree (AST) definitions for OrbitQL
//!
//! This module defines the data structures that represent parsed OrbitQL queries.
//! The AST is designed to support multi-model operations across documents, graphs,
//! time-series, and key-value data.

use crate::orbitql::QueryValue;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Root AST node for OrbitQL statements
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Statement {
    Select(SelectStatement),
    Insert(InsertStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement),
    Relate(RelateStatement),
    Create(CreateStatement),
    Drop(DropStatement),
    Transaction(TransactionStatement),
    Live(LiveStatement),
}

/// SELECT statement for querying data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectStatement {
    pub fields: Vec<SelectField>,
    pub from: Vec<FromClause>,
    pub where_clause: Option<Expression>,
    pub join_clauses: Vec<JoinClause>,
    pub group_by: Vec<Expression>,
    pub having: Option<Expression>,
    pub order_by: Vec<OrderByClause>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub fetch: Vec<FetchClause>,
    pub timeout: Option<std::time::Duration>,
}

/// Fields to select in a query
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SelectField {
    All,             // *
    AllFrom(String), // table.*
    Expression {
        expr: Expression,
        alias: Option<String>,
    },
    Graph {
        path: GraphPath,
        alias: Option<String>,
    },
    TimeSeries {
        metric: String,
        aggregation: TimeSeriesAggregation,
        window: Option<TimeWindow>,
        alias: Option<String>,
    },
}

/// FROM clause specifying data sources
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FromClause {
    Table {
        name: String,
        alias: Option<String>,
    },
    Graph {
        pattern: GraphPattern,
        alias: Option<String>,
    },
    TimeSeries {
        metric: String,
        range: TimeRange,
        alias: Option<String>,
    },
    Subquery {
        query: Box<SelectStatement>,
        alias: String,
    },
}

/// JOIN operations between different data models
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JoinClause {
    pub join_type: JoinType,
    pub target: FromClause,
    pub condition: Expression,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
    Cross,
    Graph, // Special join for graph traversals
}

/// Graph path expressions for graph traversals
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphPath {
    pub steps: Vec<GraphStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GraphStep {
    Node {
        label: Option<String>,
        properties: Option<Expression>,
    },
    Edge {
        direction: EdgeDirection,
        label: Option<String>,
        properties: Option<Expression>,
    },
    Recursive {
        min_depth: Option<u32>,
        max_depth: Option<u32>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EdgeDirection {
    Outgoing, // ->
    Incoming, // <-
    Both,     // <>
}

/// Graph pattern for matching in FROM clauses
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphPattern {
    pub path: GraphPath,
    pub where_clause: Option<Expression>,
}

/// Time-series specific structures
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: TimeExpression,
    pub end: TimeExpression,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TimeExpression {
    Now,
    Literal(DateTime<Utc>),
    Relative {
        base: Box<TimeExpression>,
        offset: TimeDuration,
        direction: TimeDirection,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TimeDirection {
    Plus,
    Minus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeDuration {
    pub value: u64,
    pub unit: TimeUnit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TimeUnit {
    Millisecond,
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeWindow {
    pub size: TimeDuration,
    pub step: Option<TimeDuration>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TimeSeriesAggregation {
    Avg,
    Sum,
    Count,
    Min,
    Max,
    First,
    Last,
    StdDev,
    Percentile(f64),
}

/// ORDER BY clause
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderByClause {
    pub expression: Expression,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SortDirection {
    Asc,
    Desc,
}

/// FETCH clause for eager loading of related data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FetchClause {
    pub fields: Vec<String>,
}

/// Generic expression type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expression {
    // Literals
    Literal(QueryValue),
    Parameter(String),

    // Identifiers
    Identifier(String),
    FieldAccess {
        object: Box<Expression>,
        field: String,
    },
    IndexAccess {
        object: Box<Expression>,
        index: Box<Expression>,
    },

    // Binary operations
    Binary {
        left: Box<Expression>,
        operator: BinaryOperator,
        right: Box<Expression>,
    },

    // Unary operations
    Unary {
        operator: UnaryOperator,
        operand: Box<Expression>,
    },

    // Function calls
    Function {
        name: String,
        args: Vec<Expression>,
    },

    // Aggregations
    Aggregate {
        function: AggregateFunction,
        expression: Option<Box<Expression>>,
        distinct: bool,
    },

    // Conditional expressions
    Case {
        when_clauses: Vec<WhenClause>,
        else_clause: Option<Box<Expression>>,
    },

    // Subqueries
    Subquery(Box<SelectStatement>),

    // Existence checks
    Exists(Box<SelectStatement>),

    // Array/Object operations
    Array(Vec<Expression>),
    Object(HashMap<String, Expression>),

    // Graph-specific expressions
    Graph(GraphPath),

    // Time-series expressions
    TimeSeries {
        metric: String,
        filters: Vec<Expression>,
        aggregation: Option<TimeSeriesAggregation>,
        window: Option<TimeWindow>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BinaryOperator {
    // Arithmetic
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,

    // Comparison
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,

    // Logical
    And,
    Or,

    // Pattern matching
    Like,
    ILike,
    NotLike,
    NotILike,
    Match,
    NotMatch,

    // Set operations
    In,
    NotIn,

    // String operations
    Contains,
    StartsWith,
    EndsWith,

    // JSON operations
    JsonExtract,
    JsonContains,

    // Graph operations
    Connected,
    NotConnected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UnaryOperator {
    Not,
    Minus,
    Plus,
    IsNull,
    IsNotNull,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AggregateFunction {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    First,
    Last,
    StdDev,
    Variance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhenClause {
    pub condition: Expression,
    pub result: Expression,
}

/// INSERT statement
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InsertStatement {
    pub into: String,
    pub fields: Option<Vec<String>>,
    pub values: InsertValues,
    pub on_conflict: Option<ConflictResolution>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InsertValues {
    Values(Vec<Vec<Expression>>),
    Select(Box<SelectStatement>),
    Object(HashMap<String, Expression>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConflictResolution {
    DoNothing,
    DoUpdate(Vec<UpdateAssignment>),
    Replace,
}

/// UPDATE statement
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateStatement {
    pub table: String,
    pub assignments: Vec<UpdateAssignment>,
    pub where_clause: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateAssignment {
    pub field: String,
    pub value: Expression,
    pub operator: Option<UpdateOperator>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UpdateOperator {
    Set,      // =
    Add,      // +=
    Subtract, // -=
    Multiply, // *=
    Divide,   // /=
    Append,   // Push to array
    Remove,   // Remove from array
}

/// DELETE statement
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteStatement {
    pub from: String,
    pub where_clause: Option<Expression>,
}

/// RELATE statement for creating graph relationships
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelateStatement {
    pub from: Expression,
    pub edge_type: String,
    pub to: Expression,
    pub properties: Option<HashMap<String, Expression>>,
}

/// CREATE statement for schema definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateStatement {
    pub object_type: CreateObjectType,
    pub name: String,
    pub definition: CreateDefinition,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CreateObjectType {
    Table,
    Index,
    View,
    Function,
    Trigger,
    Schema,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CreateDefinition {
    Table {
        fields: Vec<FieldDefinition>,
        constraints: Vec<Constraint>,
    },
    Index {
        on: String,
        fields: Vec<String>,
        unique: bool,
    },
    View {
        query: Box<SelectStatement>,
    },
    Function {
        parameters: Vec<Parameter>,
        return_type: DataType,
        body: Vec<Statement>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldDefinition {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub default: Option<Expression>,
    pub constraints: Vec<FieldConstraint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataType {
    Boolean,
    Integer,
    Float,
    String { max_length: Option<u32> },
    DateTime,
    Duration,
    Uuid,
    Array(Box<DataType>),
    Object,
    Json,
    Any,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldConstraint {
    NotNull,
    Unique,
    Check(Expression),
    ForeignKey { table: String, field: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Constraint {
    PrimaryKey(Vec<String>),
    Unique(Vec<String>),
    Check(Expression),
    ForeignKey {
        fields: Vec<String>,
        references_table: String,
        references_fields: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub data_type: DataType,
    pub default: Option<Expression>,
}

/// DROP statement
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DropStatement {
    pub object_type: CreateObjectType,
    pub name: String,
    pub if_exists: bool,
    pub cascade: bool,
}

/// Transaction statement
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransactionStatement {
    Begin,
    Commit,
    Rollback,
}

/// LIVE statement for real-time subscriptions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveStatement {
    pub query: Box<SelectStatement>,
    pub diff: bool, // Whether to return diffs or full results
}

impl Statement {
    /// Returns true if this statement modifies data
    pub fn is_mutating(&self) -> bool {
        matches!(
            self,
            Statement::Insert(_)
                | Statement::Update(_)
                | Statement::Delete(_)
                | Statement::Relate(_)
                | Statement::Create(_)
                | Statement::Drop(_)
        )
    }

    /// Returns true if this statement requires a transaction
    pub fn requires_transaction(&self) -> bool {
        self.is_mutating() || matches!(self, Statement::Transaction(_))
    }
}

impl SelectStatement {
    /// Creates a new empty SELECT statement
    pub fn new() -> Self {
        Self {
            fields: vec![SelectField::All],
            from: Vec::new(),
            where_clause: None,
            join_clauses: Vec::new(),
            group_by: Vec::new(),
            having: None,
            order_by: Vec::new(),
            limit: None,
            offset: None,
            fetch: Vec::new(),
            timeout: None,
        }
    }

    /// Adds a table to the FROM clause
    pub fn from_table(mut self, table: &str) -> Self {
        self.from.push(FromClause::Table {
            name: table.to_string(),
            alias: None,
        });
        self
    }

    /// Adds a WHERE condition
    pub fn where_expr(mut self, expr: Expression) -> Self {
        self.where_clause = Some(expr);
        self
    }

    /// Adds a LIMIT clause
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }
}

impl Default for SelectStatement {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_statement_builder() {
        let stmt = SelectStatement::new()
            .from_table("users")
            .where_expr(Expression::Binary {
                left: Box::new(Expression::Identifier("age".to_string())),
                operator: BinaryOperator::GreaterThan,
                right: Box::new(Expression::Literal(QueryValue::Integer(18))),
            })
            .limit(10);

        assert_eq!(stmt.from.len(), 1);
        assert!(stmt.where_clause.is_some());
        assert_eq!(stmt.limit, Some(10));
    }

    #[test]
    fn test_statement_classification() {
        let select = Statement::Select(SelectStatement::new());
        let insert = Statement::Insert(InsertStatement {
            into: "users".to_string(),
            fields: None,
            values: InsertValues::Object(HashMap::new()),
            on_conflict: None,
        });

        assert!(!select.is_mutating());
        assert!(insert.is_mutating());
        assert!(insert.requires_transaction());
    }
}
