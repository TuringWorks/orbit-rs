//! Abstract Syntax Tree (AST) definitions for SQL statements
//!
//! This module defines the complete AST for ANSI SQL statements including
//! DDL, DML, DCL, and TCL operations, with extensions for vector operations.

// Large enum variants are intentional for AST flexibility
#![allow(clippy::large_enum_variant)]

use crate::protocols::postgres_wire::sql::types::{SqlType, SqlValue};

/// Top-level SQL statement
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    // Data Definition Language (DDL)
    CreateDatabase(CreateDatabaseStatement),
    CreateTable(CreateTableStatement),
    CreateIndex(CreateIndexStatement),
    CreateView(CreateViewStatement),
    CreateSchema(CreateSchemaStatement),
    AlterTable(AlterTableStatement),
    DropDatabase(DropDatabaseStatement),
    DropTable(DropTableStatement),
    DropIndex(DropIndexStatement),
    DropView(DropViewStatement),
    DropSchema(DropSchemaStatement),
    UndropTable(UndropTableStatement),

    // Data Manipulation Language (DML)
    Select(Box<SelectStatement>),
    Insert(InsertStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement),
    Merge(MergeStatement),
    Copy(CopyStatement),

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

    // Session Management
    Set(SetStatement),

    // Functions
    CreateFunction(CreateFunctionStatement),

    // Triggers
    CreateTrigger(CreateTriggerStatement),
    DropTrigger(DropTriggerStatement),

    // Comments
    CommentOn(CommentOnStatement),

    // Sequences
    CreateSequence(CreateSequenceStatement),
    AlterSequence(AlterSequenceStatement),
    DropSequence(DropSequenceStatement),

    // Truncate
    Truncate(TruncateStatement),

    // Extended DDL - Types
    CreateType(CreateTypeStatement),
    DropType(DropTypeStatement),
    AlterType(AlterTypeStatement),

    // Extended DDL - Domains
    CreateDomain(CreateDomainStatement),
    DropDomain(DropDomainStatement),
    AlterDomain(AlterDomainStatement),

    // Extended DDL - Roles/Users
    CreateRole(CreateRoleStatement),
    DropRole(DropRoleStatement),
    AlterRole(AlterRoleStatement),

    // Extended DDL - Policies (Row-Level Security)
    CreatePolicy(CreatePolicyStatement),
    DropPolicy(DropPolicyStatement),
    AlterPolicy(AlterPolicyStatement),

    // Extended DDL - Rules
    CreateRule(CreateRuleStatement),
    DropRule(DropRuleStatement),

    // Extended DDL - Groups
    CreateGroup(CreateGroupStatement),
    DropGroup(DropGroupStatement),
    AlterGroup(AlterGroupStatement),

    // Extended DDL - Tablespaces
    CreateTablespace(CreateTablespaceStatement),
    DropTablespace(DropTablespaceStatement),
    AlterTablespace(AlterTablespaceStatement),

    // Extended DDL - Aggregates
    CreateAggregate(CreateAggregateStatement),
    DropAggregate(DropAggregateStatement),
    AlterAggregate(AlterAggregateStatement),

    // Extended DDL - Operators
    CreateOperator(CreateOperatorStatement),
    DropOperator(DropOperatorStatement),
    AlterOperator(AlterOperatorStatement),

    // Extended DDL - Casts
    CreateCast(CreateCastStatement),
    DropCast(DropCastStatement),

    // Extended DDL - Collations
    CreateCollation(CreateCollationStatement),
    DropCollation(DropCollationStatement),
    AlterCollation(AlterCollationStatement),

    // Extended DDL - Conversions
    CreateConversion(CreateConversionStatement),
    DropConversion(DropConversionStatement),
    AlterConversion(AlterConversionStatement),

    // Extended DDL - Foreign Data Wrappers
    CreateForeignDataWrapper(CreateForeignDataWrapperStatement),
    DropForeignDataWrapper(DropForeignDataWrapperStatement),
    AlterForeignDataWrapper(AlterForeignDataWrapperStatement),

    // Extended DDL - Foreign Tables
    CreateForeignTable(CreateForeignTableStatement),
    DropForeignTable(DropForeignTableStatement),
    AlterForeignTable(AlterForeignTableStatement),

    // Extended DDL - Servers
    CreateServer(CreateServerStatement),
    DropServer(DropServerStatement),
    AlterServer(AlterServerStatement),

    // Extended DDL - User Mappings
    CreateUserMapping(CreateUserMappingStatement),
    DropUserMapping(DropUserMappingStatement),
    AlterUserMapping(AlterUserMappingStatement),

    // Extended DDL - Publications
    CreatePublication(CreatePublicationStatement),
    DropPublication(DropPublicationStatement),
    AlterPublication(AlterPublicationStatement),

    // Extended DDL - Subscriptions
    CreateSubscription(CreateSubscriptionStatement),
    DropSubscription(DropSubscriptionStatement),
    AlterSubscription(AlterSubscriptionStatement),

    // Extended DDL - Event Triggers
    CreateEventTrigger(CreateEventTriggerStatement),
    DropEventTrigger(DropEventTriggerStatement),
    AlterEventTrigger(AlterEventTriggerStatement),

    // Extended DDL - Access Methods
    CreateAccessMethod(CreateAccessMethodStatement),
    DropAccessMethod(DropAccessMethodStatement),

    // Extended DDL - Statistics
    CreateStatistics(CreateStatisticsStatement),
    DropStatistics(DropStatisticsStatement),
    AlterStatistics(AlterStatisticsStatement),

    // Extended DDL - Text Search
    CreateTextSearchConfiguration(CreateTextSearchConfigurationStatement),
    DropTextSearchConfiguration(DropTextSearchConfigurationStatement),
    AlterTextSearchConfiguration(AlterTextSearchConfigurationStatement),
    CreateTextSearchDictionary(CreateTextSearchDictionaryStatement),
    DropTextSearchDictionary(DropTextSearchDictionaryStatement),
    AlterTextSearchDictionary(AlterTextSearchDictionaryStatement),
    CreateTextSearchParser(CreateTextSearchParserStatement),
    DropTextSearchParser(DropTextSearchParserStatement),
    AlterTextSearchParser(AlterTextSearchParserStatement),
    CreateTextSearchTemplate(CreateTextSearchTemplateStatement),
    DropTextSearchTemplate(DropTextSearchTemplateStatement),
    AlterTextSearchTemplate(AlterTextSearchTemplateStatement),

    // Extended DDL - Transforms
    CreateTransform(CreateTransformStatement),
    DropTransform(DropTransformStatement),

    // Extended DDL - Languages
    CreateLanguage(CreateLanguageStatement),
    DropLanguage(DropLanguageStatement),
    AlterLanguage(AlterLanguageStatement),

    // Extended DDL - Operator Classes/Families
    CreateOperatorClass(CreateOperatorClassStatement),
    DropOperatorClass(DropOperatorClassStatement),
    AlterOperatorClass(AlterOperatorClassStatement),
    CreateOperatorFamily(CreateOperatorFamilyStatement),
    DropOperatorFamily(DropOperatorFamilyStatement),
    AlterOperatorFamily(AlterOperatorFamilyStatement),

    // Extended DDL - Routines
    AlterRoutine(AlterRoutineStatement),
    DropRoutine(DropRoutineStatement),

    // Extended DDL - Large Objects
    AlterLargeObject(AlterLargeObjectStatement),

    // Extended DDL - Default Privileges
    AlterDefaultPrivileges(AlterDefaultPrivilegesStatement),

    // Extended DDL - System
    AlterSystem(AlterSystemStatement),

    // Extended DDL - Alter statements for existing objects
    AlterDatabase(AlterDatabaseStatement),
    AlterIndex(AlterIndexStatement),
    AlterView(AlterViewStatement),
    AlterSchema(AlterSchemaStatement),
    AlterFunction(AlterFunctionStatement),
    AlterProcedure(AlterProcedureStatement),
    AlterTrigger(AlterTriggerStatement),
    AlterMaterializedView(AlterMaterializedViewStatement),
    AlterExtension(AlterExtensionStatement),

    // Extended DDL - Drop statements
    DropFunction(DropFunctionStatement),
    DropProcedure(DropProcedureStatement),
    DropOwned(DropOwnedStatement),

    // Utility Commands
    Reset(ResetStatement),
    Discard(DiscardStatement),

    // Cursor Commands
    DeclareCursor(DeclareCursorStatement),
    FetchCursor(FetchCursorStatement),
    MoveCursor(MoveCursorStatement),
    CloseCursor(CloseCursorStatement),

    // Notification Commands
    Listen(ListenStatement),
    Unlisten(UnlistenStatement),
    Notify(NotifyStatement),

    // Prepared Statement Commands
    Prepare(PrepareStatement),
    Execute(ExecuteStatement),
    Deallocate(DeallocateStatement),

    // Maintenance Commands
    Vacuum(VacuumStatement),
    Analyze(AnalyzeStatement),
    Reindex(ReindexStatement),
    Cluster(ClusterStatement),
    Checkpoint,

    // Procedural Commands
    Call(CallStatement),
    Do(DoStatement),

    // Additional TCL Commands
    SetTransaction(SetTransactionStatement),
    SetConstraints(SetConstraintsStatement),
    Lock(LockStatement),

    // Additional Utility Commands
    Load(LoadStatement),
    RefreshMaterializedView(RefreshMaterializedViewStatement),
    ImportForeignSchema(ImportForeignSchemaStatement),

    // Two-Phase Commit Commands
    PrepareTransaction(PrepareTransactionStatement),
    CommitPrepared(CommitPreparedStatement),
    RollbackPrepared(RollbackPreparedStatement),

    // Additional DCL Commands
    ReassignOwned(ReassignOwnedStatement),
    SecurityLabel(SecurityLabelStatement),
}

// ===== DDL Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateDatabaseStatement {
    pub if_not_exists: bool,
    pub name: String,
    pub owner: Option<String>,
    pub template: Option<String>,
    pub encoding: Option<String>,
    pub locale: Option<String>,
    pub connection_limit: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropDatabaseStatement {
    pub if_exists: bool,
    pub names: Vec<String>,
    pub force: bool,
}

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
    pub unique: bool,
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
        if_exists: bool,
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
    SetSchema(String),
    Owner(String),
    AttachPartition {
        partition: TableName,
    },
    DetachPartition {
        partition: TableName,
        concurrently: bool,
        finalize: bool,
    },
    SetLogged,
    SetUnlogged,
    EnableTrigger(String),
    DisableTrigger(String),
    EnableRowLevelSecurity,
    DisableRowLevelSecurity,
    ForceRowLevelSecurity,
    NoForceRowLevelSecurity,
    SetTablespace(String),
    ClusterOn(String),
    SetWithoutCluster,
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

/// UNDROP TABLE statement for restoring dropped tables
///
/// Syntax: `UNDROP TABLE table_name`
///
/// Uses Iceberg snapshot history to restore tables to their state before dropping.
#[derive(Debug, Clone, PartialEq)]
pub struct UndropTableStatement {
    /// Table name to restore
    pub name: TableName,
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

// ===== Sequence Statements =====

/// CREATE SEQUENCE statement
#[derive(Debug, Clone, PartialEq)]
pub struct CreateSequenceStatement {
    pub if_not_exists: bool,
    pub name: TableName,
    pub options: SequenceOptions,
}

/// ALTER SEQUENCE statement
#[derive(Debug, Clone, PartialEq)]
pub struct AlterSequenceStatement {
    pub if_exists: bool,
    pub name: TableName,
    pub options: SequenceOptions,
}

/// DROP SEQUENCE statement
#[derive(Debug, Clone, PartialEq)]
pub struct DropSequenceStatement {
    pub if_exists: bool,
    pub names: Vec<TableName>,
    pub cascade: bool,
}

/// Sequence configuration options
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SequenceOptions {
    /// AS data_type (smallint, integer, bigint)
    pub data_type: Option<SqlType>,
    /// INCREMENT BY value
    pub increment: Option<i64>,
    /// MINVALUE or NO MINVALUE
    pub min_value: Option<SequenceBound>,
    /// MAXVALUE or NO MAXVALUE
    pub max_value: Option<SequenceBound>,
    /// START WITH value
    pub start: Option<i64>,
    /// CACHE value
    pub cache: Option<i64>,
    /// CYCLE or NO CYCLE
    pub cycle: Option<bool>,
    /// OWNED BY table.column or OWNED BY NONE
    pub owned_by: Option<SequenceOwner>,
    /// RESTART (for ALTER SEQUENCE)
    pub restart: Option<Option<i64>>,
}

/// Sequence bound (min/max value)
#[derive(Debug, Clone, PartialEq)]
pub enum SequenceBound {
    /// Explicit value
    Value(i64),
    /// NO MINVALUE / NO MAXVALUE (use type default)
    None,
}

/// Sequence ownership
#[derive(Debug, Clone, PartialEq)]
pub enum SequenceOwner {
    /// OWNED BY table.column
    Column { table: TableName, column: String },
    /// OWNED BY NONE
    None,
}

// ===== Truncate Statement =====

/// TRUNCATE statement
#[derive(Debug, Clone, PartialEq)]
pub struct TruncateStatement {
    /// Tables to truncate
    pub tables: Vec<TableName>,
    /// RESTART IDENTITY or CONTINUE IDENTITY
    pub identity: Option<TruncateIdentity>,
    /// CASCADE or RESTRICT
    pub cascade: Option<bool>,
    /// ONLY (don't truncate child tables)
    pub only: bool,
}

/// TRUNCATE identity handling
#[derive(Debug, Clone, PartialEq)]
pub enum TruncateIdentity {
    /// RESTART IDENTITY - reset sequences
    Restart,
    /// CONTINUE IDENTITY - keep sequence values
    Continue,
}

// ===== Column and Constraint Definitions =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateFunctionStatement {
    pub or_replace: bool,
    pub name: FunctionName,
    pub args: Option<Vec<FunctionParameter>>,
    pub return_type: Option<SqlType>,
    pub language: Option<FunctionLanguage>,
    pub body: String,
    pub volatility: Option<FunctionVolatility>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionParameter {
    pub name: Option<String>,
    pub data_type: SqlType,
    pub mode: Option<ParameterMode>,
    pub default: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParameterMode {
    In,
    Out,
    InOut,
    Variadic,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionLanguage {
    Lua,
    Sql,
    PlPgSql,
    PlJavaScript,
    Other(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionVolatility {
    Immutable,
    Stable,
    Volatile,
}

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
    /// PostgreSQL 12+ GENERATED ALWAYS AS (expression) STORED
    /// PostgreSQL 18+ GENERATED ALWAYS AS (expression) VIRTUAL
    Generated {
        expression: Expression,
        storage: GeneratedColumnStorage,
    },
}

/// Storage type for generated columns
#[derive(Debug, Clone, PartialEq)]
pub enum GeneratedColumnStorage {
    /// STORED - value is computed on INSERT/UPDATE and stored physically
    Stored,
    /// VIRTUAL - value is computed on each read (PostgreSQL 18+)
    Virtual,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TableConstraint {
    PrimaryKey {
        name: Option<String>,
        columns: Vec<String>,
        /// PostgreSQL 18: WITHOUT OVERLAPS for temporal primary keys
        /// The column name that should use WITHOUT OVERLAPS (must be a range type)
        without_overlaps: Option<String>,
    },
    Unique {
        name: Option<String>,
        columns: Vec<String>,
        /// PostgreSQL 18: WITHOUT OVERLAPS for temporal unique constraints
        without_overlaps: Option<String>,
    },
    ForeignKey {
        name: Option<String>,
        columns: Vec<String>,
        references_table: TableName,
        references_columns: Vec<String>,
        on_delete: Option<ReferentialAction>,
        on_update: Option<ReferentialAction>,
        /// PostgreSQL 18: PERIOD for temporal foreign keys
        period_column: Option<String>,
        references_period: Option<String>,
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
    pub traverse: Option<TraverseClause>,
    /// Compound query operation (UNION, INTERSECT, EXCEPT)
    pub set_operation: Option<SetOperation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraverseClause {
    pub direction: TraverseDirection,
    pub min_steps: u32,
    pub max_steps: u32,
    pub edge_collection: String,
    pub target_alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TraverseDirection {
    Outbound,
    Inbound,
    Any,
}

/// Set operations for compound SELECT statements (UNION, INTERSECT, EXCEPT)
#[derive(Debug, Clone, PartialEq)]
pub enum SetOperator {
    Union,
    UnionAll,
    Intersect,
    IntersectAll,
    Except,
    ExceptAll,
}

/// A compound SELECT with a set operation
#[derive(Debug, Clone, PartialEq)]
pub struct SetOperation {
    pub operator: SetOperator,
    pub right: Box<SelectStatement>,
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
        time_travel: Option<TimeTravelClause>,
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
        lateral: bool,
    },
    Values {
        values: Vec<Vec<Expression>>,
        alias: Option<TableAlias>,
    },
    TableFunction {
        function: FunctionCall,
        alias: Option<TableAlias>,
        lateral: bool,
    },
    JsonTable(JsonTable),
}

/// Time travel clause for querying historical data
///
/// Supports multiple syntaxes:
/// - Snowflake-style: `AT(TIMESTAMP => '2025-01-01')`
/// - Snowflake-style: `AT(VERSION => 123456789)`
/// - SQL:2011 temporal: `FOR SYSTEM_TIME AS OF TIMESTAMP '2025-01-01'`
#[derive(Debug, Clone, PartialEq)]
pub enum TimeTravelClause {
    /// Query table as it existed at a specific timestamp
    /// Example: `AT(TIMESTAMP => '2025-01-01 00:00:00')`
    Timestamp(Expression),

    /// Query table at a specific version/snapshot ID
    /// Example: `AT(VERSION => 123456789)`
    Version(Expression),

    /// SQL:2011 FOR SYSTEM_TIME AS OF syntax
    /// Example: `FOR SYSTEM_TIME AS OF TIMESTAMP '2025-01-01'`
    SystemTime(Expression),
}

#[derive(Debug, Clone, PartialEq)]
pub struct JsonTable {
    pub context_item: Expression,
    pub path_expression: Expression,
    pub columns: Vec<JsonTableColumn>,
    pub alias: Option<TableAlias>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JsonTableColumn {
    pub name: String,
    pub data_type: SqlType,
    pub path: Option<String>,
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

#[derive(Debug, Clone, PartialEq)]
pub struct MergeStatement {
    pub table: TableName,
    pub alias: Option<String>,
    pub source: FromClause,
    pub on: Expression,
    pub when_clauses: Vec<MergeWhenClause>,
    pub returning: Option<Vec<SelectItem>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergeWhenClause {
    pub matched: bool,
    pub condition: Option<Expression>,
    pub action: MergeAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MergeAction {
    Update(MergeUpdate),
    Delete,
    Insert(MergeInsert),
    DoNothing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergeUpdate {
    pub assignments: Vec<Assignment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergeInsert {
    pub columns: Option<Vec<String>>,
    pub values: MergeInsertValues,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MergeInsertValues {
    Values(Vec<Expression>),
    DefaultValues,
}

// ===== COPY Statement =====

/// PostgreSQL COPY statement for bulk data transfer
#[derive(Debug, Clone, PartialEq)]
pub struct CopyStatement {
    /// Direction of the copy operation
    pub direction: CopyDirection,
    /// Target table or query
    pub target: CopyTarget,
    /// Specific columns (if not all)
    pub columns: Option<Vec<String>>,
    /// Source/destination specification
    pub source: CopySource,
    /// Copy options
    pub options: Vec<CopyOption>,
}

/// Direction of COPY operation
#[derive(Debug, Clone, PartialEq)]
pub enum CopyDirection {
    /// COPY TO (export data)
    To,
    /// COPY FROM (import data)
    From,
}

/// Target of COPY operation
#[derive(Debug, Clone, PartialEq)]
pub enum CopyTarget {
    /// Table name
    Table(TableName),
    /// Query (only for COPY TO)
    Query(Box<SelectStatement>),
}

/// Source/destination for COPY data
#[derive(Debug, Clone, PartialEq)]
pub enum CopySource {
    /// Standard input/output (STDIN/STDOUT)
    Stdio,
    /// Program to pipe through
    Program(String),
    /// File path
    File(String),
}

/// COPY statement options
#[derive(Debug, Clone, PartialEq)]
pub enum CopyOption {
    /// FORMAT (text, csv, binary)
    Format(CopyFormat),
    /// FREEZE (for initial data load)
    Freeze(bool),
    /// DELIMITER character
    Delimiter(char),
    /// NULL string representation
    Null(String),
    /// HEADER (first line contains headers)
    Header(CopyHeaderOption),
    /// QUOTE character (CSV)
    Quote(char),
    /// ESCAPE character (CSV)
    Escape(char),
    /// FORCE_QUOTE columns (CSV)
    ForceQuote(Vec<String>),
    /// FORCE_NOT_NULL columns (CSV)
    ForceNotNull(Vec<String>),
    /// FORCE_NULL columns (CSV)
    ForceNull(Vec<String>),
    /// ENCODING
    Encoding(String),
    /// Default value for missing columns
    Default(String),
    /// ON_ERROR behavior
    OnError(CopyOnError),
    /// LOG_VERBOSITY
    LogVerbosity(CopyLogVerbosity),
}

/// COPY format types
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CopyFormat {
    #[default]
    Text,
    Csv,
    Binary,
}

/// COPY HEADER option values
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CopyHeaderOption {
    /// No header processing
    #[default]
    Off,
    /// First row is header (HEADER or HEADER true)
    On,
    /// Match header to column names (HEADER MATCH)
    Match,
}

/// COPY ON_ERROR behavior
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CopyOnError {
    /// Stop on error (default)
    #[default]
    Stop,
    /// Skip rows with errors
    Ignore,
}

/// COPY LOG_VERBOSITY option
#[derive(Debug, Clone, PartialEq)]
pub enum CopyLogVerbosity {
    Default,
    Verbose,
}

// ===== Expressions =====

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    // Literals and identifiers
    Literal(SqlValue),
    Column(ColumnRef),
    Parameter(u32),

    // Date/Time functions
    CurrentDate,
    CurrentTime(Option<u32>),
    CurrentTimestamp(Option<u32>),
    LocalTime(Option<u32>),
    LocalTimestamp(Option<u32>),

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
    Any(Box<Expression>),
    All(Box<Expression>),
    Some(Box<Expression>),

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

    // JSON path operators
    /// -> operator: extract JSON object field
    JsonExtract,
    /// ->> operator: extract JSON object field as text
    JsonExtractText,
    /// #> operator: extract JSON sub-object at path
    JsonPathExtract,
    /// #>> operator: extract JSON sub-object at path as text
    JsonPathExtractText,

    // JSON containment operators
    /// @> operator: does left JSON contain right JSON
    JsonContains,
    /// <@ operator: is left JSON contained in right JSON
    JsonContainedBy,
    /// ? operator: does string exist as top-level key
    JsonExists,
    /// ?| operator: do any strings exist as top-level keys
    JsonExistsAny,
    /// ?& operator: do all strings exist as top-level keys
    JsonExistsAll,

    // JSON manipulation
    /// || operator: concatenate JSON values
    JsonConcat,
    /// - operator: delete key or array element
    JsonDelete,
    /// #- operator: delete path
    JsonDeletePath,

    // JSON path operators (PostgreSQL)
    /// @? operator: does JSON path exist
    JsonPathExists,
    /// @@ operator: does JSON path match
    JsonPathMatch,

    // Pattern matching
    Match,
    NotMatch,
    SimilarTo,
    NotSimilarTo,
    NotLike,

    RegexMatch,                   // ~ operator
    RegexMatchCaseInsensitive,    // ~* operator
    RegexNotMatch,                // !~ operator
    RegexNotMatchCaseInsensitive, // !~* operator

    // Null tests
    Is,
    IsNot,
    IsDistinctFrom,    // IS DISTINCT FROM
    IsNotDistinctFrom, // IS NOT DISTINCT FROM

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

    // Range operators (PostgreSQL range types)
    /// @> operator: range contains element/range
    RangeContains,
    /// <@ operator: element/range is contained by range
    RangeContainedBy,
    /// && operator: ranges overlap
    RangeOverlaps,
    /// -|- operator: ranges are adjacent
    RangeAdjacent,
    /// << operator: range is strictly left of range
    RangeStrictlyLeft,
    /// >> operator: range is strictly right of range
    RangeStrictlyRight,
    /// &< operator: range does not extend right of range
    RangeNotExtendRight,
    /// &> operator: range does not extend left of range
    RangeNotExtendLeft,

    // Text Search operators
    /// @@ operator: tsvector matches tsquery
    TextSearchMatch,
    /// @> operator: tsquery contains tsquery
    TextSearchContains,
    /// <@ operator: tsquery is contained by tsquery
    TextSearchContainedBy,
    /// || operator: concatenate tsvectors or tsqueries
    TextSearchConcat,
    /// && operator: AND tsqueries
    TextSearchAnd,
    /// !! operator: negate tsquery
    TextSearchNot,
    /// <-> operator: followed by (phrase search)
    TextSearchFollowedBy,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOperator {
    Plus,
    Minus,
    Not,
    BitwiseNot,
    SquareRoot,    // |/ operator
    CubeRoot,      // ||/ operator
    AbsoluteValue, // @ operator
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
    pub within_group: Option<Vec<OrderByItem>>,
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
    pub mode: WindowFrameMode,
    pub start_bound: FrameBound,
    pub end_bound: Option<FrameBound>,
    pub exclusion: Option<WindowFrameExclusion>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum WindowFrameMode {
    #[default]
    Range,
    Rows,
    Groups,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WindowFrameExclusion {
    CurrentRow,
    Group,
    Ties,
    NoOthers,
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

// ===== Trigger Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTriggerStatement {
    pub or_replace: bool,
    pub name: String,
    pub timing: TriggerTiming,
    pub events: Vec<TriggerEvent>,
    pub table: TableName,
    pub for_each: TriggerForEach,
    pub when_clause: Option<Expression>,
    pub function: FunctionName,
    pub function_args: Vec<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerTiming {
    Before,
    After,
    InsteadOf,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerEvent {
    Insert,
    Update(Option<Vec<String>>), // Optional column list for UPDATE OF
    Delete,
    Truncate,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerForEach {
    Row,
    Statement,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropTriggerStatement {
    pub if_exists: bool,
    pub name: String,
    pub table: TableName,
    pub cascade: bool,
}

// ===== Comment Statement =====

#[derive(Debug, Clone, PartialEq)]
pub struct CommentOnStatement {
    pub object_type: CommentObjectType,
    pub object_name: String,
    pub column_name: Option<String>, // For COMMENT ON COLUMN table.column
    pub comment: Option<String>,     // None means NULL (remove comment)
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommentObjectType {
    Table,
    Column,
    Index,
    View,
    Schema,
    Extension,
    Function,
    Trigger,
    Constraint,
    Database,
}

// Helper implementations

impl std::fmt::Display for TableName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.full_name())
    }
}

impl std::fmt::Display for FunctionName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FunctionName::Simple(name) => write!(f, "{}", name),
            FunctionName::Qualified { schema, name } => write!(f, "{}.{}", schema, name),
        }
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
#[derive(Debug, Clone, PartialEq)]
pub struct SetStatement {
    pub variable: String,
    pub value: Vec<Expression>,
}

// ===== Extended DDL - Type Statements =====

/// CREATE TYPE statement for enum, composite, range, and base types
#[derive(Debug, Clone, PartialEq)]
pub struct CreateTypeStatement {
    pub if_not_exists: bool,
    pub name: TableName, // schema.name support
    pub type_definition: TypeDefinition,
}

/// Type definition variants
#[derive(Debug, Clone, PartialEq)]
pub enum TypeDefinition {
    /// ENUM type: CREATE TYPE name AS ENUM ('value1', 'value2', ...)
    Enum { values: Vec<String> },
    /// Composite type: CREATE TYPE name AS (column1 type1, column2 type2, ...)
    Composite { attributes: Vec<TypeAttribute> },
    /// Range type: CREATE TYPE name AS RANGE (SUBTYPE = subtype, ...)
    Range {
        subtype: SqlType,
        options: Vec<RangeTypeOption>,
    },
    /// Base type: CREATE TYPE name (INPUT = ..., OUTPUT = ..., ...)
    Base { options: Vec<BaseTypeOption> },
    /// Shell type: CREATE TYPE name (placeholder for forward references)
    Shell,
}

/// Attribute for composite types
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAttribute {
    pub name: String,
    pub data_type: SqlType,
    pub collation: Option<String>,
}

/// Options for range types
#[derive(Debug, Clone, PartialEq)]
pub enum RangeTypeOption {
    Subtype(SqlType),
    SubtypeOpClass(String),
    Collation(String),
    Canonical(String),
    SubtypeDiff(String),
    Multirange(String),
}

/// Options for base types
#[derive(Debug, Clone, PartialEq)]
pub enum BaseTypeOption {
    Input(String),
    Output(String),
    Receive(String),
    Send(String),
    TypeModIn(String),
    TypeModOut(String),
    Analyze(String),
    Subscript(String),
    InternalLength(i32),
    PassedByValue,
    Alignment(String),
    Storage(String),
    Like(String),
    Category(char),
    Preferred(bool),
    DefaultValue(String),
    Element(SqlType),
    Delimiter(char),
    Collatable(bool),
}

/// DROP TYPE statement
#[derive(Debug, Clone, PartialEq)]
pub struct DropTypeStatement {
    pub if_exists: bool,
    pub names: Vec<TableName>,
    pub cascade: bool,
}

/// ALTER TYPE statement
#[derive(Debug, Clone, PartialEq)]
pub struct AlterTypeStatement {
    pub name: TableName,
    pub action: AlterTypeAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterTypeAction {
    /// ADD VALUE 'new_value' [BEFORE|AFTER 'existing_value']
    AddValue {
        if_not_exists: bool,
        value: String,
        position: Option<EnumValuePosition>,
    },
    /// RENAME VALUE 'old_value' TO 'new_value'
    RenameValue {
        old_value: String,
        new_value: String,
    },
    /// RENAME TO new_name
    Rename(String),
    /// SET SCHEMA new_schema
    SetSchema(String),
    /// ADD ATTRIBUTE name data_type
    AddAttribute { name: String, data_type: SqlType },
    /// DROP ATTRIBUTE name
    DropAttribute { name: String, cascade: bool },
    /// ALTER ATTRIBUTE name SET DATA TYPE data_type
    AlterAttribute { name: String, data_type: SqlType },
    /// OWNER TO new_owner
    Owner(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum EnumValuePosition {
    Before(String),
    After(String),
}

// ===== Extended DDL - Domain Statements =====

/// CREATE DOMAIN statement
#[derive(Debug, Clone, PartialEq)]
pub struct CreateDomainStatement {
    pub if_not_exists: bool,
    pub name: TableName,
    pub data_type: SqlType,
    pub collation: Option<String>,
    pub default: Option<Expression>,
    pub constraints: Vec<DomainConstraint>,
}

/// Domain constraint
#[derive(Debug, Clone, PartialEq)]
pub struct DomainConstraint {
    pub name: Option<String>,
    pub constraint_type: DomainConstraintType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DomainConstraintType {
    NotNull,
    Null,
    Check(Expression),
}

/// DROP DOMAIN statement
#[derive(Debug, Clone, PartialEq)]
pub struct DropDomainStatement {
    pub if_exists: bool,
    pub names: Vec<TableName>,
    pub cascade: bool,
}

/// ALTER DOMAIN statement
#[derive(Debug, Clone, PartialEq)]
pub struct AlterDomainStatement {
    pub name: TableName,
    pub action: AlterDomainAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterDomainAction {
    SetDefault(Expression),
    DropDefault,
    SetNotNull,
    DropNotNull,
    AddConstraint(DomainConstraint),
    DropConstraint { name: String, cascade: bool },
    RenameConstraint { old_name: String, new_name: String },
    ValidateConstraint(String),
    Owner(String),
    Rename(String),
    SetSchema(String),
}

// ===== Extended DDL - Role/User Statements =====

/// CREATE ROLE/USER statement
#[derive(Debug, Clone, PartialEq)]
pub struct CreateRoleStatement {
    pub if_not_exists: bool,
    pub name: String,
    pub is_user: bool, // true for CREATE USER (implies LOGIN)
    pub options: Vec<RoleOption>,
}

/// Role options
#[derive(Debug, Clone, PartialEq)]
pub enum RoleOption {
    SuperUser(bool),           // SUPERUSER / NOSUPERUSER
    CreateDb(bool),            // CREATEDB / NOCREATEDB
    CreateRole(bool),          // CREATEROLE / NOCREATEROLE
    Inherit(bool),             // INHERIT / NOINHERIT
    Login(bool),               // LOGIN / NOLOGIN
    Replication(bool),         // REPLICATION / NOREPLICATION
    BypassRls(bool),           // BYPASSRLS / NOBYPASSRLS
    ConnectionLimit(i32),      // CONNECTION LIMIT n
    Password(Option<String>),  // PASSWORD 'password' / PASSWORD NULL
    EncryptedPassword(String), // ENCRYPTED PASSWORD 'password'
    ValidUntil(String),        // VALID UNTIL 'timestamp'
    InRole(Vec<String>),       // IN ROLE role1, role2
    Role(Vec<String>),         // ROLE role1, role2 (this role can be granted)
    Admin(Vec<String>),        // ADMIN role1, role2 (can grant this role)
}

/// DROP ROLE/USER statement
#[derive(Debug, Clone, PartialEq)]
pub struct DropRoleStatement {
    pub if_exists: bool,
    pub names: Vec<String>,
    pub is_user: bool, // true for DROP USER
}

/// ALTER ROLE/USER statement
#[derive(Debug, Clone, PartialEq)]
pub struct AlterRoleStatement {
    pub name: String,
    pub is_user: bool,
    pub action: AlterRoleAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterRoleAction {
    /// ALTER ROLE name WITH options
    SetOptions(Vec<RoleOption>),
    /// ALTER ROLE name RENAME TO new_name
    Rename(String),
    /// ALTER ROLE name SET parameter TO value
    SetConfig {
        parameter: String,
        value: Expression,
    },
    /// ALTER ROLE name RESET parameter
    ResetConfig(String),
    /// ALTER ROLE name RESET ALL
    ResetAllConfig,
}

// ===== Extended DDL - Policy Statements (Row-Level Security) =====

/// CREATE POLICY statement
#[derive(Debug, Clone, PartialEq)]
pub struct CreatePolicyStatement {
    pub name: String,
    pub table: TableName,
    pub permissive: bool, // true for PERMISSIVE (default), false for RESTRICTIVE
    pub command: PolicyCommand,
    pub roles: Vec<String>,             // TO roles
    pub using_expr: Option<Expression>, // USING expression
    pub check_expr: Option<Expression>, // WITH CHECK expression
}

/// Policy command type
#[derive(Debug, Clone, PartialEq)]
pub enum PolicyCommand {
    All,
    Select,
    Insert,
    Update,
    Delete,
}

/// DROP POLICY statement
#[derive(Debug, Clone, PartialEq)]
pub struct DropPolicyStatement {
    pub if_exists: bool,
    pub name: String,
    pub table: TableName,
    pub cascade: bool,
}

/// ALTER POLICY statement
#[derive(Debug, Clone, PartialEq)]
pub struct AlterPolicyStatement {
    pub name: String,
    pub table: TableName,
    pub action: AlterPolicyAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterPolicyAction {
    Rename(String),
    SetRoles(Vec<String>),
    SetUsing(Option<Expression>),
    SetCheck(Option<Expression>),
}

// ===== Extended DDL - Rule Statements =====

/// CREATE RULE statement
#[derive(Debug, Clone, PartialEq)]
pub struct CreateRuleStatement {
    pub or_replace: bool,
    pub name: String,
    pub table: TableName,
    pub event: RuleEvent,
    pub where_clause: Option<Expression>,
    pub action: RuleAction,
}

/// Rule event type
#[derive(Debug, Clone, PartialEq)]
pub enum RuleEvent {
    Select,
    Insert,
    Update,
    Delete,
}

/// Rule action
#[derive(Debug, Clone, PartialEq)]
pub enum RuleAction {
    Nothing,
    Instead(Vec<Statement>),
    Also(Vec<Statement>),
}

/// DROP RULE statement
#[derive(Debug, Clone, PartialEq)]
pub struct DropRuleStatement {
    pub if_exists: bool,
    pub name: String,
    pub table: TableName,
    pub cascade: bool,
}

// ===== Extended DDL - Group Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateGroupStatement {
    pub name: String,
    pub with_options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropGroupStatement {
    pub if_exists: bool,
    pub names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterGroupStatement {
    pub name: String,
    pub action: AlterGroupAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterGroupAction {
    AddUser(Vec<String>),
    DropUser(Vec<String>),
    Rename(String),
}

// ===== Extended DDL - Tablespace Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTablespaceStatement {
    pub if_not_exists: bool,
    pub name: String,
    pub owner: Option<String>,
    pub location: String,
    pub options: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropTablespaceStatement {
    pub if_exists: bool,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterTablespaceStatement {
    pub name: String,
    pub action: AlterTablespaceAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterTablespaceAction {
    Rename(String),
    Owner(String),
    SetOptions(Vec<(String, String)>),
    ResetOptions(Vec<String>),
}

// ===== Extended DDL - Aggregate Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateAggregateStatement {
    pub or_replace: bool,
    pub name: TableName,
    pub args: Vec<SqlType>,
    pub sfunc: String,
    pub stype: SqlType,
    pub options: Vec<AggregateOption>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AggregateOption {
    SFunc(String),
    SType(SqlType),
    SSpace(i64),
    FinalFunc(String),
    FinalFuncExtra,
    FinalFuncModify(String),
    CombineFunc(String),
    SerialFunc(String),
    DeserialFunc(String),
    InitCond(String),
    MInitCond(String),
    SortOp(String),
    Parallel(String),
    // Moving-aggregate options (PostgreSQL)
    MSFunc(String),     // Moving-aggregate state function
    MInvFunc(String),   // Moving-aggregate inverse function
    MSType(SqlType),    // Moving-aggregate state type
    MSSpace(i64),       // Moving-aggregate state size
    MFinalFunc(String), // Moving-aggregate final function
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropAggregateStatement {
    pub if_exists: bool,
    pub names: Vec<(TableName, Vec<SqlType>)>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterAggregateStatement {
    pub name: TableName,
    pub args: Vec<SqlType>,
    pub action: AlterAggregateAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterAggregateAction {
    Rename(String),
    Owner(String),
    SetSchema(String),
}

// ===== Extended DDL - Operator Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateOperatorStatement {
    pub name: String,
    pub left_type: Option<SqlType>,
    pub right_type: Option<SqlType>,
    pub procedure: String,
    pub options: Vec<OperatorOption>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OperatorOption {
    Commutator(String),
    Negator(String),
    Restrict(String),
    Join(String),
    Hashes,
    Merges,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropOperatorStatement {
    pub if_exists: bool,
    pub operators: Vec<(String, Option<SqlType>, Option<SqlType>)>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterOperatorStatement {
    pub name: String,
    pub left_type: Option<SqlType>,
    pub right_type: Option<SqlType>,
    pub action: AlterOperatorAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterOperatorAction {
    Owner(String),
    SetSchema(String),
    SetRestrict(String),
    SetJoin(String),
}

// ===== Extended DDL - Cast Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateCastStatement {
    pub source_type: SqlType,
    pub target_type: SqlType,
    pub function: Option<String>,
    pub context: CastContext,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CastContext {
    Implicit,
    Assignment,
    Explicit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropCastStatement {
    pub if_exists: bool,
    pub source_type: SqlType,
    pub target_type: SqlType,
    pub cascade: bool,
}

// ===== Extended DDL - Collation Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateCollationStatement {
    pub if_not_exists: bool,
    pub name: TableName,
    pub options: CollationOptions,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CollationOptions {
    From(TableName),
    Definition {
        locale: Option<String>,
        lc_collate: Option<String>,
        lc_ctype: Option<String>,
        provider: Option<String>,
        deterministic: Option<bool>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropCollationStatement {
    pub if_exists: bool,
    pub names: Vec<TableName>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterCollationStatement {
    pub name: TableName,
    pub action: AlterCollationAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterCollationAction {
    RefreshVersion,
    Rename(String),
    Owner(String),
    SetSchema(String),
}

// ===== Extended DDL - Conversion Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateConversionStatement {
    pub default: bool,
    pub name: TableName,
    pub source_encoding: String,
    pub dest_encoding: String,
    pub function: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropConversionStatement {
    pub if_exists: bool,
    pub names: Vec<TableName>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterConversionStatement {
    pub name: TableName,
    pub action: AlterConversionAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterConversionAction {
    Rename(String),
    Owner(String),
    SetSchema(String),
}

// ===== Extended DDL - Foreign Data Wrapper Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateForeignDataWrapperStatement {
    pub if_not_exists: bool,
    pub name: String,
    pub handler: Option<String>,
    pub validator: Option<String>,
    pub options: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropForeignDataWrapperStatement {
    pub if_exists: bool,
    pub names: Vec<String>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterForeignDataWrapperStatement {
    pub name: String,
    pub action: AlterForeignDataWrapperAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterForeignDataWrapperAction {
    SetHandler(Option<String>),
    SetValidator(Option<String>),
    SetOptions(Vec<(String, String)>),
    Owner(String),
    Rename(String),
}

// ===== Extended DDL - Foreign Table Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateForeignTableStatement {
    pub if_not_exists: bool,
    pub name: TableName,
    pub columns: Vec<ColumnDefinition>,
    pub server: String,
    pub options: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropForeignTableStatement {
    pub if_exists: bool,
    pub names: Vec<TableName>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterForeignTableStatement {
    pub name: TableName,
    pub action: AlterForeignTableAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterForeignTableAction {
    AddColumn(ColumnDefinition),
    DropColumn { name: String, cascade: bool },
    SetOptions(Vec<(String, String)>),
    Owner(String),
    Rename(String),
    SetSchema(String),
}

// ===== Extended DDL - Server Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateServerStatement {
    pub if_not_exists: bool,
    pub name: String,
    pub server_type: Option<String>,
    pub version: Option<String>,
    pub foreign_data_wrapper: String,
    pub options: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropServerStatement {
    pub if_exists: bool,
    pub names: Vec<String>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterServerStatement {
    pub name: String,
    pub action: AlterServerAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterServerAction {
    SetVersion(Option<String>),
    SetOptions(Vec<(String, String)>),
    Owner(String),
    Rename(String),
}

// ===== Extended DDL - User Mapping Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateUserMappingStatement {
    pub if_not_exists: bool,
    pub user: String,
    pub server: String,
    pub options: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropUserMappingStatement {
    pub if_exists: bool,
    pub user: String,
    pub server: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterUserMappingStatement {
    pub user: String,
    pub server: String,
    pub options: Vec<(String, String)>,
}

// ===== Extended DDL - Publication Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreatePublicationStatement {
    pub if_not_exists: bool,
    pub name: String,
    pub for_all_tables: bool,
    pub tables: Vec<TableName>,
    pub options: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropPublicationStatement {
    pub if_exists: bool,
    pub names: Vec<String>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterPublicationStatement {
    pub name: String,
    pub action: AlterPublicationAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterPublicationAction {
    AddTable(Vec<TableName>),
    DropTable(Vec<TableName>),
    SetTable(Vec<TableName>),
    SetOptions(Vec<(String, String)>),
    Owner(String),
    Rename(String),
}

// ===== Extended DDL - Subscription Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateSubscriptionStatement {
    pub if_not_exists: bool,
    pub name: String,
    pub connection: String,
    pub publication: Vec<String>,
    pub options: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropSubscriptionStatement {
    pub if_exists: bool,
    pub name: String,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterSubscriptionStatement {
    pub name: String,
    pub action: AlterSubscriptionAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterSubscriptionAction {
    SetConnection(String),
    SetPublication(Vec<String>),
    AddPublication(Vec<String>),
    DropPublication(Vec<String>),
    SetOptions(Vec<(String, String)>),
    Enable,
    Disable,
    Refresh,
    Owner(String),
    Rename(String),
}

// ===== Extended DDL - Event Trigger Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateEventTriggerStatement {
    pub name: String,
    pub event: String,
    pub when_clause: Option<Vec<(String, Vec<String>)>>,
    pub function: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropEventTriggerStatement {
    pub if_exists: bool,
    pub name: String,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterEventTriggerStatement {
    pub name: String,
    pub action: AlterEventTriggerAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterEventTriggerAction {
    Enable,
    EnableReplica,
    EnableAlways,
    Disable,
    Owner(String),
    Rename(String),
}

// ===== Extended DDL - Access Method Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateAccessMethodStatement {
    pub name: String,
    pub method_type: AccessMethodType,
    pub handler: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccessMethodType {
    Index,
    Table,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropAccessMethodStatement {
    pub if_exists: bool,
    pub name: String,
    pub cascade: bool,
}

// ===== Extended DDL - Statistics Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateStatisticsStatement {
    pub if_not_exists: bool,
    pub name: TableName,
    pub kinds: Vec<String>,
    pub table: TableName,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropStatisticsStatement {
    pub if_exists: bool,
    pub names: Vec<TableName>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterStatisticsStatement {
    pub name: TableName,
    pub action: AlterStatisticsAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterStatisticsAction {
    Owner(String),
    Rename(String),
    SetSchema(String),
    SetStatisticsTarget(i32),
}

// ===== Extended DDL - Text Search Configuration Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTextSearchConfigurationStatement {
    pub if_not_exists: bool,
    pub name: TableName,
    pub source: Option<TableName>,
    pub parser: Option<TableName>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropTextSearchConfigurationStatement {
    pub if_exists: bool,
    pub names: Vec<TableName>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterTextSearchConfigurationStatement {
    pub name: TableName,
    pub action: AlterTextSearchConfigurationAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterTextSearchConfigurationAction {
    AddMapping {
        token_type: String,
        dictionaries: Vec<TableName>,
    },
    AlterMapping {
        token_type: String,
        dictionaries: Vec<TableName>,
    },
    DropMapping {
        if_exists: bool,
        token_type: String,
    },
    Owner(String),
    Rename(String),
    SetSchema(String),
}

// ===== Extended DDL - Text Search Dictionary Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTextSearchDictionaryStatement {
    pub if_not_exists: bool,
    pub name: TableName,
    pub template: TableName,
    pub options: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropTextSearchDictionaryStatement {
    pub if_exists: bool,
    pub names: Vec<TableName>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterTextSearchDictionaryStatement {
    pub name: TableName,
    pub action: AlterTextSearchDictionaryAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterTextSearchDictionaryAction {
    SetOptions(Vec<(String, String)>),
    Owner(String),
    Rename(String),
    SetSchema(String),
}

// ===== Extended DDL - Text Search Parser Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTextSearchParserStatement {
    pub if_not_exists: bool,
    pub name: TableName,
    pub start_func: String,
    pub gettoken_func: String,
    pub end_func: String,
    pub lextypes_func: String,
    pub headline_func: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropTextSearchParserStatement {
    pub if_exists: bool,
    pub names: Vec<TableName>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterTextSearchParserStatement {
    pub name: TableName,
    pub action: AlterTextSearchParserAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterTextSearchParserAction {
    Rename(String),
    SetSchema(String),
}

// ===== Extended DDL - Text Search Template Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTextSearchTemplateStatement {
    pub if_not_exists: bool,
    pub name: TableName,
    pub init_func: Option<String>,
    pub lexize_func: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropTextSearchTemplateStatement {
    pub if_exists: bool,
    pub names: Vec<TableName>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterTextSearchTemplateStatement {
    pub name: TableName,
    pub action: AlterTextSearchTemplateAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterTextSearchTemplateAction {
    Rename(String),
    SetSchema(String),
}

// ===== Extended DDL - Transform Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTransformStatement {
    pub or_replace: bool,
    pub type_name: SqlType,
    pub language: String,
    pub from_sql: Option<String>,
    pub to_sql: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropTransformStatement {
    pub if_exists: bool,
    pub type_name: SqlType,
    pub language: String,
    pub cascade: bool,
}

// ===== Extended DDL - Language Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateLanguageStatement {
    pub or_replace: bool,
    pub trusted: bool,
    pub procedural: bool,
    pub name: String,
    pub handler: Option<String>,
    pub inline_handler: Option<String>,
    pub validator: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropLanguageStatement {
    pub if_exists: bool,
    pub name: String,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterLanguageStatement {
    pub name: String,
    pub action: AlterLanguageAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterLanguageAction {
    Rename(String),
    Owner(String),
}

// ===== Extended DDL - Operator Class Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateOperatorClassStatement {
    pub default: bool,
    pub name: TableName,
    pub data_type: SqlType,
    pub index_method: String,
    pub family: Option<TableName>,
    pub operators: Vec<OperatorClassItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OperatorClassItem {
    Operator {
        strategy: i32,
        name: String,
        for_search: bool,
    },
    Function {
        support: i32,
        name: String,
    },
    Storage(SqlType),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropOperatorClassStatement {
    pub if_exists: bool,
    pub name: TableName,
    pub index_method: String,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterOperatorClassStatement {
    pub name: TableName,
    pub index_method: String,
    pub action: AlterOperatorClassAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterOperatorClassAction {
    Rename(String),
    Owner(String),
    SetSchema(String),
}

// ===== Extended DDL - Operator Family Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct CreateOperatorFamilyStatement {
    pub if_not_exists: bool,
    pub name: TableName,
    pub index_method: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropOperatorFamilyStatement {
    pub if_exists: bool,
    pub name: TableName,
    pub index_method: String,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterOperatorFamilyStatement {
    pub name: TableName,
    pub index_method: String,
    pub action: AlterOperatorFamilyAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterOperatorFamilyAction {
    Add(Vec<OperatorClassItem>),
    Drop(Vec<OperatorFamilyDropItem>),
    Rename(String),
    Owner(String),
    SetSchema(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum OperatorFamilyDropItem {
    Operator {
        strategy: i32,
        left_type: SqlType,
        right_type: SqlType,
    },
    Function {
        support: i32,
        left_type: SqlType,
        right_type: SqlType,
    },
}

// ===== Extended DDL - Routine Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct AlterRoutineStatement {
    pub name: TableName,
    pub args: Vec<SqlType>,
    pub action: AlterRoutineAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterRoutineAction {
    Rename(String),
    Owner(String),
    SetSchema(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropRoutineStatement {
    pub if_exists: bool,
    pub routines: Vec<(TableName, Vec<SqlType>)>,
    pub cascade: bool,
}

// ===== Extended DDL - Large Object Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct AlterLargeObjectStatement {
    pub oid: i64,
    pub owner: String,
}

// ===== Extended DDL - Default Privileges =====

#[derive(Debug, Clone, PartialEq)]
pub struct AlterDefaultPrivilegesStatement {
    pub target: Option<DefaultPrivilegesTarget>,
    pub action: DefaultPrivilegesAction,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DefaultPrivilegesTarget {
    pub for_role: Option<Vec<String>>,
    pub in_schema: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DefaultPrivilegesAction {
    Grant {
        privileges: Vec<Privilege>,
        object_type: String,
        grantees: Vec<String>,
        with_grant_option: bool,
    },
    Revoke {
        privileges: Vec<Privilege>,
        object_type: String,
        grantees: Vec<String>,
        cascade: bool,
    },
}

// ===== Extended DDL - System Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct AlterSystemStatement {
    pub action: AlterSystemAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterSystemAction {
    Set { parameter: String, value: String },
    Reset(String),
    ResetAll,
}

// ===== Extended DDL - Alter Statements for Existing Objects =====

#[derive(Debug, Clone, PartialEq)]
pub struct AlterDatabaseStatement {
    pub name: String,
    pub action: AlterDatabaseAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterDatabaseAction {
    Rename(String),
    Owner(String),
    SetTablespace(String),
    SetConfig { parameter: String, value: String },
    ResetConfig(String),
    ResetAllConfig,
    RefreshCollationVersion,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterIndexStatement {
    pub if_exists: bool,
    pub name: TableName,
    pub action: AlterIndexAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterIndexAction {
    Rename(String),
    SetTablespace(String),
    AttachPartition(TableName),
    SetOptions(Vec<(String, String)>),
    ResetOptions(Vec<String>),
    AlterColumn { column: i32, set_statistics: i32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterViewStatement {
    pub if_exists: bool,
    pub name: TableName,
    pub action: AlterViewAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterViewAction {
    Rename(String),
    Owner(String),
    SetSchema(String),
    SetOptions(Vec<(String, String)>),
    ResetOptions(Vec<String>),
    AlterColumn {
        column: String,
        action: AlterColumnAction,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterSchemaStatement {
    pub name: String,
    pub action: AlterSchemaAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterSchemaAction {
    Rename(String),
    Owner(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterFunctionStatement {
    pub name: TableName,
    pub args: Vec<SqlType>,
    pub action: AlterFunctionAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterFunctionAction {
    Rename(String),
    Owner(String),
    SetSchema(String),
    SetConfig { parameter: String, value: String },
    ResetConfig(String),
    ResetAllConfig,
    SecurityDefiner(bool),
    Parallel(String),
    Cost(f64),
    Rows(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterProcedureStatement {
    pub name: TableName,
    pub args: Vec<SqlType>,
    pub action: AlterProcedureAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterProcedureAction {
    Rename(String),
    Owner(String),
    SetSchema(String),
    SetConfig { parameter: String, value: String },
    ResetConfig(String),
    ResetAllConfig,
    SecurityDefiner(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterTriggerStatement {
    pub name: String,
    pub table: TableName,
    pub action: AlterTriggerAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterTriggerAction {
    Rename(String),
    DependsOnExtension(String),
    NoDependsOnExtension(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterMaterializedViewStatement {
    pub if_exists: bool,
    pub name: TableName,
    pub action: AlterMaterializedViewAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterMaterializedViewAction {
    Rename(String),
    Owner(String),
    SetSchema(String),
    SetTablespace(String),
    SetOptions(Vec<(String, String)>),
    ResetOptions(Vec<String>),
    AlterColumn {
        column: String,
        action: AlterColumnAction,
    },
    ClusterOn(String),
    SetWithoutCluster,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterExtensionStatement {
    pub name: String,
    pub action: AlterExtensionAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterExtensionAction {
    UpdateTo(Option<String>),
    SetSchema(String),
    AddMember {
        object_type: String,
        object_name: String,
    },
    DropMember {
        object_type: String,
        object_name: String,
    },
}

// ===== Extended DDL - Additional Drop Statements =====

#[derive(Debug, Clone, PartialEq)]
pub struct DropFunctionStatement {
    pub if_exists: bool,
    pub functions: Vec<(TableName, Vec<SqlType>)>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropProcedureStatement {
    pub if_exists: bool,
    pub procedures: Vec<(TableName, Vec<SqlType>)>,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropOwnedStatement {
    pub roles: Vec<String>,
    pub cascade: bool,
}

// ===== Utility Commands =====

#[derive(Debug, Clone, PartialEq)]
pub struct ResetStatement {
    pub target: ResetTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResetTarget {
    Parameter(String),
    All,
    TimeZone,
    Role,
    SessionAuthorization,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiscardStatement {
    pub target: DiscardTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiscardTarget {
    All,
    Plans,
    Sequences,
    Temporary,
    Temp,
}

// ===== Cursor Commands =====

#[derive(Debug, Clone, PartialEq)]
pub struct DeclareCursorStatement {
    pub name: String,
    pub binary: bool,
    pub insensitive: bool,
    pub scroll: Option<bool>, // None = default, Some(true) = SCROLL, Some(false) = NO SCROLL
    pub hold: bool,           // WITH HOLD
    pub query: Box<SelectStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FetchCursorStatement {
    pub direction: FetchDirection,
    pub cursor_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FetchDirection {
    Next,
    Prior,
    First,
    Last,
    Absolute(i64),
    Relative(i64),
    Count(i64),
    All,
    Forward,
    ForwardCount(i64),
    ForwardAll,
    Backward,
    BackwardCount(i64),
    BackwardAll,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoveCursorStatement {
    pub direction: FetchDirection,
    pub cursor_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CloseCursorStatement {
    pub cursor_name: CloseCursorTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CloseCursorTarget {
    Named(String),
    All,
}

// ===== Notification Commands =====

#[derive(Debug, Clone, PartialEq)]
pub struct ListenStatement {
    pub channel: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnlistenStatement {
    pub channel: UnlistenTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnlistenTarget {
    Channel(String),
    All,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NotifyStatement {
    pub channel: String,
    pub payload: Option<String>,
}

// ===== Prepared Statement Commands =====

#[derive(Debug, Clone, PartialEq)]
pub struct PrepareStatement {
    pub name: String,
    pub data_types: Vec<SqlType>,
    pub statement: Box<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecuteStatement {
    pub name: String,
    pub parameters: Vec<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeallocateStatement {
    pub target: DeallocateTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeallocateTarget {
    Named(String),
    All,
}

// ===== Maintenance Commands =====

#[derive(Debug, Clone, PartialEq)]
pub struct VacuumStatement {
    pub full: bool,
    pub freeze: bool,
    pub verbose: bool,
    pub analyze: bool,
    pub disable_page_skipping: bool,
    pub skip_locked: bool,
    pub index_cleanup: Option<bool>,
    pub truncate: Option<bool>,
    pub parallel: Option<i32>,
    pub tables: Vec<VacuumTable>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VacuumTable {
    pub name: TableName,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyzeStatement {
    pub verbose: bool,
    pub skip_locked: bool,
    pub tables: Vec<VacuumTable>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReindexStatement {
    pub target_type: ReindexTarget,
    pub concurrently: bool,
    pub verbose: bool,
    pub name: Option<TableName>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReindexTarget {
    Index,
    Table,
    Schema,
    Database,
    System,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClusterStatement {
    pub verbose: bool,
    pub table_name: Option<TableName>,
    pub index_name: Option<String>,
}

// ===== Procedural Commands =====

#[derive(Debug, Clone, PartialEq)]
pub struct CallStatement {
    pub procedure_name: TableName,
    pub arguments: Vec<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DoStatement {
    pub language: Option<String>,
    pub code: String,
}

// ===== Additional TCL Commands =====

#[derive(Debug, Clone, PartialEq)]
pub struct SetTransactionStatement {
    pub isolation_level: Option<TransactionIsolationLevel>,
    pub read_only: Option<bool>,
    pub deferrable: Option<bool>,
    pub session_characteristics: bool, // SET SESSION CHARACTERISTICS AS TRANSACTION
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransactionIsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetConstraintsStatement {
    pub constraints: ConstraintTarget,
    pub mode: ConstraintMode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstraintTarget {
    All,
    Named(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstraintMode {
    Deferred,
    Immediate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LockStatement {
    pub tables: Vec<LockTarget>,
    pub mode: LockMode,
    pub nowait: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LockTarget {
    pub table_name: TableName,
    pub only: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LockMode {
    AccessShare,
    RowShare,
    RowExclusive,
    ShareUpdateExclusive,
    Share,
    ShareRowExclusive,
    Exclusive,
    AccessExclusive,
}

// ===== Additional Utility Commands =====

#[derive(Debug, Clone, PartialEq)]
pub struct LoadStatement {
    pub filename: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RefreshMaterializedViewStatement {
    pub concurrently: bool,
    pub view_name: TableName,
    pub with_data: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportForeignSchemaStatement {
    pub remote_schema: String,
    pub import_type: ImportForeignSchemaType,
    pub server_name: String,
    pub local_schema: String,
    pub options: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImportForeignSchemaType {
    All,
    LimitTo(Vec<String>),
    Except(Vec<String>),
}

// ===== Two-Phase Commit Commands =====

#[derive(Debug, Clone, PartialEq)]
pub struct PrepareTransactionStatement {
    pub transaction_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommitPreparedStatement {
    pub transaction_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RollbackPreparedStatement {
    pub transaction_id: String,
}

// ===== Additional DCL Commands =====

#[derive(Debug, Clone, PartialEq)]
pub struct ReassignOwnedStatement {
    pub old_roles: Vec<String>,
    pub new_role: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SecurityLabelStatement {
    pub provider: Option<String>,
    pub object_type: SecurityLabelObjectType,
    pub object_name: TableName,
    pub column_name: Option<String>,
    pub label: Option<String>, // None means remove label
}

#[derive(Debug, Clone, PartialEq)]
pub enum SecurityLabelObjectType {
    Table,
    Column,
    Aggregate,
    Database,
    Domain,
    EventTrigger,
    ForeignTable,
    Function,
    Index,
    Language,
    LargeObject,
    MaterializedView,
    Procedure,
    Publication,
    Role,
    Routine,
    Schema,
    Sequence,
    Subscription,
    Tablespace,
    Type,
    View,
}
