//! Abstract Syntax Tree (AST) definitions for SQL statements
//!
//! This module defines the complete AST for ANSI SQL statements including
//! DDL, DML, DCL, and TCL operations, with extensions for vector operations.

use crate::postgres_wire::sql::types::{SqlType, SqlValue};

/// Top-level SQL statement
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    // Data Definition Language (DDL)
    CreateTable(CreateTableStatement),
    CreateIndex(CreateIndexStatement),
    CreateView(CreateViewStatement),
    CreateSchema(CreateSchemaStatement),
    AlterTable(AlterTableStatement),
    DropTable(DropTableStatement),
    DropIndex(DropIndexStatement),
    DropView(DropViewStatement),
    DropSchema(DropSchemaStatement),

    // Data Manipulation Language (DML)
    Select(Box<SelectStatement>),
    Insert(InsertStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement),

    // Data Control Language (DCL)
    Grant(GrantStatement),
    Revoke(RevokeStatement),

    // Transaction Control Language (TCL)
    Begin(BeginStatement),
    Commit(CommitStatement),
    Rollback(RollbackStatement),
    Savepoint(SavepointStatement),
    ReleaseSavepoint(ReleaseSavepointStatement),

    // Utility statements
    Explain(ExplainStatement),
    Show(ShowStatement),
    Use(UseStatement),
    Describe(DescribeStatement),

    // Extensions
    CreateExtension(CreateExtensionStatement),
    DropExtension(DropExtensionStatement),
}

// ===== DDL Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTableStatement {
    pub if_not_exists: bool,
    pub name: TableName,
    pub columns: Vec<ColumnDefinition>,
    pub constraints: Vec<TableConstraint>,
    pub options: Vec<TableOption>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateIndexStatement {
    pub if_not_exists: bool,
    pub name: Option<String>,
    pub table: TableName,
    pub columns: Vec<IndexColumn>,
    pub index_type: IndexType,
    pub where_clause: Option<Expression>,
    pub options: Vec<IndexOption>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateViewStatement {
    pub if_not_exists: bool,
    pub name: TableName,
    pub columns: Option<Vec<String>>,
    pub query: Box<SelectStatement>,
    pub materialized: bool,
    pub replace: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateSchemaStatement {
    pub if_not_exists: bool,
    pub name: String,
    pub authorization: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterTableStatement {
    pub name: TableName,
    pub actions: Vec<AlterTableAction>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterTableAction {
    AddColumn(ColumnDefinition),
    DropColumn {
        name: String,
        cascade: bool,
    },
    AlterColumn {
        name: String,
        action: AlterColumnAction,
    },
    AddConstraint(TableConstraint),
    DropConstraint {
        name: String,
        cascade: bool,
    },
    RenameColumn {
        old_name: String,
        new_name: String,
    },
    RenameTable(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterColumnAction {
    SetType(SqlType),
    SetDefault(Expression),
    DropDefault,
    SetNotNull,
    DropNotNull,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropTableStatement {
    pub if_exists: bool,
    pub names: Vec<TableName>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropIndexStatement {
    pub if_exists: bool,
    pub names: Vec<String>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropViewStatement {
    pub if_exists: bool,
    pub names: Vec<TableName>,
    pub cascade: bool,
    pub materialized: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropSchemaStatement {
    pub if_exists: bool,
    pub names: Vec<String>,
    pub cascade: bool,
}

// ===== Column and Constraint Definitions =====

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDefinition {
    pub name: String,
    pub data_type: SqlType,
    pub constraints: Vec<ColumnConstraint>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ColumnConstraint {
    NotNull,
    Null,
    Default(Expression),
    PrimaryKey,
    Unique,
    References {
        table: TableName,
        columns: Option<Vec<String>>,
        on_delete: Option<ReferentialAction>,
        on_update: Option<ReferentialAction>,
    },
    Check(Expression),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TableConstraint {
    PrimaryKey {
        name: Option<String>,
        columns: Vec<String>,
    },
    Unique {
        name: Option<String>,
        columns: Vec<String>,
    },
    ForeignKey {
        name: Option<String>,
        columns: Vec<String>,
        references_table: TableName,
        references_columns: Vec<String>,
        on_delete: Option<ReferentialAction>,
        on_update: Option<ReferentialAction>,
    },
    Check {
        name: Option<String>,
        expression: Expression,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReferentialAction {
    Cascade,
    SetNull,
    SetDefault,
    Restrict,
    NoAction,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableOption {
    pub name: String,
    pub value: Option<SqlValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndexColumn {
    pub name: String,
    pub direction: Option<SortDirection>,
    pub nulls: Option<NullsOrder>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IndexType {
    BTree,
    Hash,
    Gist,
    Gin,
    // Vector indexes
    IvfFlat {
        lists: Option<i32>,
    },
    Hnsw {
        m: Option<i32>,
        ef_construction: Option<i32>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndexOption {
    pub name: String,
    pub value: SqlValue,
}

// ===== DML Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct SelectStatement {
    pub with: Option<WithClause>,
    pub select_list: Vec<SelectItem>,
    pub distinct: Option<DistinctClause>,
    pub from_clause: Option<FromClause>,
    pub where_clause: Option<Expression>,
    pub group_by: Option<Vec<Expression>>,
    pub having: Option<Expression>,
    pub order_by: Option<Vec<OrderByItem>>,
    pub limit: Option<LimitClause>,
    pub offset: Option<u64>,
    pub for_clause: Option<ForClause>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WithClause {
    pub recursive: bool,
    pub ctes: Vec<CommonTableExpression>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommonTableExpression {
    pub name: String,
    pub columns: Option<Vec<String>>,
    pub query: Box<SelectStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectItem {
    Wildcard,
    QualifiedWildcard {
        qualifier: String,
    },
    Expression {
        expr: Expression,
        alias: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DistinctClause {
    Distinct,
    DistinctOn(Vec<Expression>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum FromClause {
    Table {
        name: TableName,
        alias: Option<TableAlias>,
    },
    Join {
        left: Box<FromClause>,
        join_type: JoinType,
        right: Box<FromClause>,
        condition: JoinCondition,
    },
    Subquery {
        query: Box<SelectStatement>,
        alias: TableAlias,
    },
    Values {
        values: Vec<Vec<Expression>>,
        alias: Option<TableAlias>,
    },
    TableFunction {
        function: FunctionCall,
        alias: Option<TableAlias>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableAlias {
    pub name: String,
    pub columns: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JoinType {
    Inner,
    LeftOuter,
    RightOuter,
    FullOuter,
    Cross,
    LeftSemi,
    LeftAnti,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JoinCondition {
    On(Expression),
    Using(Vec<String>),
    Natural,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderByItem {
    pub expression: Expression,
    pub direction: Option<SortDirection>,
    pub nulls: Option<NullsOrder>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NullsOrder {
    First,
    Last,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LimitClause {
    pub count: Option<Expression>,
    pub with_ties: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ForClause {
    Update { nowait: bool, skip_locked: bool },
    Share { nowait: bool, skip_locked: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub struct InsertStatement {
    pub table: TableName,
    pub columns: Option<Vec<String>>,
    pub source: InsertSource,
    pub on_conflict: Option<OnConflictClause>,
    pub returning: Option<Vec<SelectItem>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InsertSource {
    Values(Vec<Vec<Expression>>),
    Query(Box<SelectStatement>),
    DefaultValues,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OnConflictClause {
    pub target: Option<ConflictTarget>,
    pub action: ConflictAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConflictTarget {
    Columns(Vec<String>),
    Constraint(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConflictAction {
    DoNothing,
    DoUpdate {
        set: Vec<Assignment>,
        where_clause: Option<Expression>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateStatement {
    pub table: TableName,
    pub alias: Option<String>,
    pub set: Vec<Assignment>,
    pub from: Option<Vec<FromClause>>,
    pub where_clause: Option<Expression>,
    pub returning: Option<Vec<SelectItem>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub target: AssignmentTarget,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AssignmentTarget {
    Column(String),
    Columns(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeleteStatement {
    pub table: TableName,
    pub alias: Option<String>,
    pub using: Option<Vec<FromClause>>,
    pub where_clause: Option<Expression>,
    pub returning: Option<Vec<SelectItem>>,
}

// ===== Expressions =====

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    // Literals and identifiers
    Literal(SqlValue),
    Column(ColumnRef),
    Parameter(u32),

    // Operators
    Binary {
        left: Box<Expression>,
        operator: BinaryOperator,
        right: Box<Expression>,
    },
    Unary {
        operator: UnaryOperator,
        operand: Box<Expression>,
    },

    // Functions and aggregates
    Function(Box<FunctionCall>),
    WindowFunction {
        function: WindowFunctionType,
        partition_by: Vec<Expression>,
        order_by: Vec<OrderByItem>,
        frame: Option<WindowFrame>,
    },

    // Conditional expressions
    Case(CaseExpression),

    // Subqueries and lists
    Subquery(Box<SelectStatement>),
    Exists(Box<SelectStatement>),
    In {
        expr: Box<Expression>,
        list: InList,
        negated: bool,
    },

    // Range conditions
    Between {
        expr: Box<Expression>,
        low: Box<Expression>,
        high: Box<Expression>,
        negated: bool,
    },

    // Pattern matching
    Like {
        expr: Box<Expression>,
        pattern: Box<Expression>,
        escape: Option<Box<Expression>>,
        case_insensitive: bool,
        negated: bool,
    },

    // Null checks
    IsNull {
        expr: Box<Expression>,
        negated: bool,
    },

    // Type casting
    Cast {
        expr: Box<Expression>,
        target_type: SqlType,
    },

    // Array and row operations
    Array(Vec<Expression>),
    Row(Vec<Expression>),
    ArrayIndex {
        array: Box<Expression>,
        index: Box<Expression>,
    },
    ArraySlice {
        array: Box<Expression>,
        start: Option<Box<Expression>>,
        end: Option<Box<Expression>>,
    },

    // Vector operations
    VectorSimilarity {
        left: Box<Expression>,
        operator: VectorOperator,
        right: Box<Expression>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnRef {
    pub table: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOperator {
    // Arithmetic
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
    Power,

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

    // String
    Concat,
    Like,
    ILike,
    Similar,

    // Array
    Contains,
    ContainedBy,
    Overlap,

    // JSON
    JsonExtract,
    JsonExtractText,
    JsonContains,
    JsonContainedBy,

    // Pattern matching
    Match,
    NotMatch,

    // Null tests
    Is,
    IsNot,

    // Set operations
    In,
    NotIn,

    // Vector operations
    VectorDistance,
    VectorInnerProduct,
    VectorCosineDistance,

    // Bitwise
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    LeftShift,
    RightShift,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOperator {
    Plus,
    Minus,
    Not,
    BitwiseNot,
    IsNull,
    IsNotNull,
    IsTrue,
    IsNotTrue,
    IsFalse,
    IsNotFalse,
    IsUnknown,
    IsNotUnknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VectorOperator {
    L2Distance,      // <->
    InnerProduct,    // <#>
    CosineDistance,  // <=>
    L1Distance,      // Custom extension
    HammingDistance, // Custom extension
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionCall {
    pub name: FunctionName,
    pub args: Vec<Expression>,
    pub distinct: bool,
    pub order_by: Option<Vec<OrderByItem>>,
    pub filter: Option<Box<Expression>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionName {
    Simple(String),
    Qualified { schema: String, name: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum WindowFunctionType {
    RowNumber,
    Rank,
    DenseRank,
    PercentRank,
    CumeDist,
    Ntile(Box<Expression>),
    Lag {
        expr: Box<Expression>,
        offset: Option<Box<Expression>>,
        default: Option<Box<Expression>>,
    },
    Lead {
        expr: Box<Expression>,
        offset: Option<Box<Expression>>,
        default: Option<Box<Expression>>,
    },
    FirstValue(Box<Expression>),
    LastValue(Box<Expression>),
    NthValue {
        expr: Box<Expression>,
        n: Box<Expression>,
    },
    Aggregate(Box<FunctionCall>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowFrame {
    pub start_bound: FrameBound,
    pub end_bound: Option<FrameBound>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FrameBound {
    UnboundedPreceding,
    Preceding(Box<Expression>),
    CurrentRow,
    Following(Box<Expression>),
    UnboundedFollowing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaseExpression {
    pub operand: Option<Box<Expression>>,
    pub when_clauses: Vec<WhenClause>,
    pub else_clause: Option<Box<Expression>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhenClause {
    pub condition: Box<Expression>,
    pub result: Box<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InList {
    Expressions(Vec<Expression>),
    Subquery(Box<SelectStatement>),
}

// ===== Table and Schema Names =====

#[derive(Debug, Clone, PartialEq)]
pub struct TableName {
    pub schema: Option<String>,
    pub name: String,
}

impl TableName {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            schema: None,
            name: name.into(),
        }
    }

    pub fn with_schema(schema: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            schema: Some(schema.into()),
            name: name.into(),
        }
    }

    pub fn full_name(&self) -> String {
        match &self.schema {
            Some(schema) => format!("{}.{}", schema, self.name),
            None => self.name.clone(),
        }
    }
}

// ===== DCL Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct GrantStatement {
    pub privileges: Vec<Privilege>,
    pub object_type: ObjectType,
    pub object_name: String,
    pub grantees: Vec<String>,
    pub with_grant_option: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RevokeStatement {
    pub grant_option_for: bool,
    pub privileges: Vec<Privilege>,
    pub object_type: ObjectType,
    pub object_name: String,
    pub grantees: Vec<String>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Privilege {
    All,
    Select,
    Insert,
    Update,
    Delete,
    Create,
    Drop,
    Alter,
    Execute,
    Usage,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectType {
    Table,
    View,
    Schema,
    Function,
    Sequence,
    Database,
}

// ===== TCL Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct BeginStatement {
    pub isolation_level: Option<IsolationLevel>,
    pub access_mode: Option<AccessMode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommitStatement {
    pub chain: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RollbackStatement {
    pub to_savepoint: Option<String>,
    pub chain: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SavepointStatement {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseSavepointStatement {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccessMode {
    ReadOnly,
    ReadWrite,
}

// ===== Utility Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct ExplainStatement {
    pub analyze: bool,
    pub verbose: bool,
    pub costs: bool,
    pub buffers: bool,
    pub timing: bool,
    pub format: ExplainFormat,
    pub statement: Box<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExplainFormat {
    Text,
    Json,
    Xml,
    Yaml,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShowStatement {
    pub variable: ShowVariable,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShowVariable {
    All,
    Variable(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct UseStatement {
    pub schema: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DescribeStatement {
    pub object_type: ObjectType,
    pub name: String,
}

// ===== Extension Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateExtensionStatement {
    pub if_not_exists: bool,
    pub name: String,
    pub schema: Option<String>,
    pub version: Option<String>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropExtensionStatement {
    pub if_exists: bool,
    pub names: Vec<String>,
    pub cascade: bool,
}

// Helper implementations

impl std::fmt::Display for TableName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.full_name())
    }
}

impl From<&str> for TableName {
    fn from(name: &str) -> Self {
        if name.contains('.') {
            let parts: Vec<&str> = name.split('.').collect();
            Self::with_schema(parts[0], parts[1])
        } else {
            Self::new(name)
        }
    }
}

impl From<String> for TableName {
    fn from(name: String) -> Self {
        name.as_str().into()
    }
}
