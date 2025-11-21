//! Abstract Syntax Tree (AST) definitions for OrbitQL
//!
//! This module defines the data structures that represent parsed OrbitQL queries.
//! The AST is designed to support multi-model operations across documents, graphs,
//! time-series, and key-value data.

use crate::orbitql::QueryValue;
use crate::spatial::{BoundingBox, Point};
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
    // Graph traversal
    Traverse(TraverseStatement),
    // GraphRAG statements
    GraphRAG(GraphRAGStatement),
}

/// TRAVERSE statement for graph traversal
/// Syntax: TRAVERSE <edge_type> FROM <node_id> [MAX_DEPTH <n>] [WHERE <condition>]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraverseStatement {
    /// Edge type/relationship to traverse
    pub edge_type: String,
    /// Starting node expression (e.g., user:user1)
    pub from_node: Expression,
    /// Maximum traversal depth (optional)
    pub max_depth: Option<u32>,
    /// Optional filter condition
    pub where_clause: Option<Expression>,
}

/// Common Table Expression (CTE) clause
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WithClause {
    pub name: String,
    pub columns: Option<Vec<String>>,
    pub query: Box<SelectStatement>,
    pub recursive: bool,
}

/// SELECT statement for querying data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectStatement {
    pub with_clauses: Vec<WithClause>,
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

    // Spatial expressions
    Geometry(GeometryLiteral),
    SpatialFunction {
        name: String,
        args: Vec<Expression>,
        srid: Option<i32>,
    },
    BoundingBox(BoundingBox),
    Buffer {
        geometry: Box<Expression>,
        distance: f64,
    },
    Transform {
        geometry: Box<Expression>,
        srid: i32,
    },

    // Machine Learning expressions
    MLFunction {
        function: MLFunction,
        args: Vec<Expression>,
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
    Between,

    // Null checking
    Is,
    IsNot,

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

    // Spatial operations
    SpatialContains,
    SpatialWithin,
    SpatialIntersects,
    SpatialOverlaps,
    SpatialTouches,
    SpatialCrosses,
    SpatialDisjoint,
    SpatialEquals,
    SpatialDWithin(f64),
    SpatialBeyond(f64),
    SpatialKNN(i32),
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

/// Machine Learning function definitions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MLFunction {
    // Model Management
    TrainModel {
        name: String,
        algorithm: MLAlgorithm,
        features: Vec<Expression>,
        target: Box<Expression>,
        parameters: HashMap<String, Expression>,
    },
    Predict {
        model_name: String,
        features: Vec<Expression>,
    },
    EvaluateModel {
        model_name: String,
        test_features: Vec<Expression>,
        test_target: Box<Expression>,
        metrics: Vec<String>,
    },
    UpdateModel {
        model_name: String,
        features: Vec<Expression>,
        target: Box<Expression>,
    },
    DropModel {
        model_name: String,
    },
    ListModels,
    ModelInfo {
        model_name: String,
    },

    // Statistical Functions
    LinearRegression {
        features: Vec<Expression>,
        target: Box<Expression>,
    },
    LogisticRegression {
        features: Vec<Expression>,
        target: Box<Expression>,
    },
    Correlation {
        x: Box<Expression>,
        y: Box<Expression>,
    },
    Covariance {
        x: Box<Expression>,
        y: Box<Expression>,
    },
    ZScore {
        value: Box<Expression>,
        mean: Box<Expression>,
        std: Box<Expression>,
    },

    // Supervised Learning
    KMeans {
        features: Vec<Expression>,
        k: u32,
    },
    SVM {
        features: Vec<Expression>,
        target: Box<Expression>,
        kernel: String,
    },
    DecisionTree {
        features: Vec<Expression>,
        target: Box<Expression>,
        max_depth: Option<u32>,
    },
    RandomForest {
        features: Vec<Expression>,
        target: Box<Expression>,
        n_estimators: u32,
    },
    NeuralNetwork {
        features: Vec<Expression>,
        target: Box<Expression>,
        layers: Vec<u32>,
    },

    // Boosting Algorithms
    GradientBoosting {
        features: Vec<Expression>,
        target: Box<Expression>,
        n_estimators: u32,
        learning_rate: f64,
    },
    AdaBoost {
        features: Vec<Expression>,
        target: Box<Expression>,
        n_estimators: u32,
    },
    XGBoost {
        features: Vec<Expression>,
        target: Box<Expression>,
        parameters: HashMap<String, Expression>,
    },
    LightGBM {
        features: Vec<Expression>,
        target: Box<Expression>,
        parameters: HashMap<String, Expression>,
    },
    CatBoost {
        features: Vec<Expression>,
        target: Box<Expression>,
        parameters: HashMap<String, Expression>,
    },

    // Feature Engineering
    Normalize {
        values: Vec<Expression>,
        method: NormalizationMethod,
    },
    EncodeCategorical {
        category: Box<Expression>,
        method: EncodingMethod,
    },
    PolynomialFeatures {
        features: Vec<Expression>,
        degree: u32,
    },
    PCA {
        features: Vec<Expression>,
        components: u32,
    },
    FeatureSelection {
        features: Vec<Expression>,
        target: Box<Expression>,
        method: String,
    },

    // Vector Operations
    EmbedText {
        text: Box<Expression>,
        model: String,
    },
    EmbedImage {
        image_url: Box<Expression>,
        model: String,
    },
    SimilaritySearch {
        query_vector: Box<Expression>,
        target_vectors: Box<Expression>,
        k: u32,
    },
    VectorCluster {
        vectors: Vec<Expression>,
        k: u32,
    },
    DimensionalityReduction {
        vectors: Vec<Expression>,
        method: String,
        dimensions: u32,
    },

    // Time Series ML
    Forecast {
        timeseries: Box<Expression>,
        periods: u32,
    },
    SeasonalityDecompose {
        timeseries: Box<Expression>,
    },
    AnomalyDetection {
        timeseries: Box<Expression>,
    },
    ChangepointDetection {
        timeseries: Box<Expression>,
    },

    // NLP Functions
    SentimentAnalysis {
        text: Box<Expression>,
    },
    ExtractEntities {
        text: Box<Expression>,
    },
    SummarizeText {
        text: Box<Expression>,
        max_length: u32,
    },
    Translate {
        text: Box<Expression>,
        source_lang: String,
        target_lang: String,
    },
}

/// ML algorithms available for training
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MLAlgorithm {
    LinearRegression,
    LogisticRegression,
    KMeans,
    SVM,
    DecisionTree,
    RandomForest,
    NeuralNetwork,
    GradientBoosting,
    AdaBoost,
    XGBoost,
    LightGBM,
    CatBoost,
}

/// Normalization methods for feature scaling
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NormalizationMethod {
    MinMax,
    ZScore,
    Robust,
    UnitVector,
}

/// Categorical encoding methods
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EncodingMethod {
    OneHot,
    Label,
    Target,
    Binary,
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
    // Spatial data types
    Geometry,
    Point,
    LineString,
    Polygon,
    MultiPoint,
    MultiLineString,
    MultiPolygon,
    GeometryCollection,
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

/// GraphRAG statement for knowledge graph operations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GraphRAGStatement {
    /// Build knowledge graph from document
    Build {
        kg_name: String,
        document_id: String,
        text: String,
        metadata: Option<HashMap<String, Expression>>,
        build_graph: bool,
        generate_embeddings: bool,
        extractors: Option<Vec<String>>,
    },
    /// Query knowledge graph with RAG
    Query {
        kg_name: String,
        query_text: String,
        max_hops: Option<u32>,
        context_size: Option<usize>,
        llm_provider: Option<String>,
        search_strategy: Option<String>,
        include_explanation: bool,
        max_results: Option<usize>,
    },
    /// Extract entities and relationships only
    Extract {
        kg_name: String,
        document_id: String,
        text: String,
        extractors: Option<Vec<String>>,
        confidence_threshold: Option<f32>,
    },
    /// Find reasoning paths between entities
    Reason {
        kg_name: String,
        from_entity: String,
        to_entity: String,
        max_hops: Option<u32>,
        relationship_types: Option<Vec<String>>,
        include_explanation: bool,
        max_results: Option<usize>,
    },
    /// Get knowledge graph statistics
    Stats { kg_name: String },
    /// List entities in knowledge graph
    Entities {
        kg_name: String,
        entity_type: Option<String>,
        limit: Option<usize>,
    },
    /// Find similar entities
    Similar {
        kg_name: String,
        entity_name: String,
        limit: Option<usize>,
        threshold: Option<f32>,
    },
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
                | Statement::GraphRAG(GraphRAGStatement::Build { .. })
        )
    }

    /// Returns true if this statement requires a transaction
    pub fn requires_transaction(&self) -> bool {
        self.is_mutating() || matches!(self, Statement::Transaction(_))
    }
}

impl std::fmt::Display for Statement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Statement::Select(_) => write!(f, "SELECT"),
            Statement::Insert(_) => write!(f, "INSERT"),
            Statement::Update(_) => write!(f, "UPDATE"),
            Statement::Delete(_) => write!(f, "DELETE"),
            Statement::Relate(_) => write!(f, "RELATE"),
            Statement::Create(_) => write!(f, "CREATE"),
            Statement::Drop(_) => write!(f, "DROP"),
            Statement::Transaction(_) => write!(f, "TRANSACTION"),
            Statement::Live(_) => write!(f, "LIVE"),
            Statement::Traverse(_) => write!(f, "TRAVERSE"),
            Statement::GraphRAG(_) => write!(f, "GRAPH_RAG"),
        }
    }
}

impl SelectStatement {
    /// Creates a new empty SELECT statement
    pub fn new() -> Self {
        Self {
            with_clauses: Vec::new(),
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

/// Geometry literals in various formats
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GeometryLiteral {
    Point(Point),
    WKT(String),
    GeoJSON(serde_json::Value),
    EWKT(String),    // Extended WKT with SRID
    Binary(Vec<u8>), // WKB
}

/// Spatial filter for queries
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpatialFilter {
    pub geometry_expr: Expression,
    pub operator: SpatialOperator,
    pub reference_expr: Expression,
}

/// Spatial operators for geometric relationships
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SpatialOperator {
    // Topological relationships
    Contains,
    Within,
    Intersects,
    Overlaps,
    Touches,
    Crosses,
    Disjoint,
    Equals,

    // Distance relationships
    DWithin(f64), // Distance within threshold
    Beyond(f64),  // Distance beyond threshold

    // Directional relationships
    North,
    South,
    East,
    West,
    Northeast,
    Northwest,
    Southeast,
    Southwest,

    // Custom operators
    KNN(i32), // K nearest neighbors
    BBox,     // Bounding box intersection
}

/// Spatial index types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SpatialIndexType {
    RTree,
    QuadTree,
    Geohash,
    BTree,
    Hash,
}

/// Spatial index configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpatialIndexConfig {
    pub max_entries: Option<usize>,
    pub precision: Option<u8>,
    pub fill_factor: Option<f64>,
    pub srid: Option<i32>,
}

/// Window specification for streaming queries
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowSpec {
    pub size: std::time::Duration,
    pub slide: Option<std::time::Duration>,
    pub watermark: Option<std::time::Duration>,
}

/// Streaming clause for real-time queries
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamingClause {
    pub window: WindowSpec,
    pub trigger: Option<StreamTrigger>,
}

/// Stream triggers
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StreamTrigger {
    ProcessingTime(std::time::Duration),
    EventTime,
    Count(usize),
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
