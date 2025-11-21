//! Natural Language Query Processor
//!
//! This module provides natural language understanding capabilities for converting
//! user queries into structured SQL operations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Natural Language Query Processor
pub struct NlpQueryProcessor {
    /// Schema analyzer for understanding database structure
    pub schema_analyzer: SchemaAnalyzer,
    /// Intent classifier for determining query type
    pub intent_classifier: IntentClassifier,
    /// Entity extractor for identifying tables, columns, values
    pub entity_extractor: EntityExtractor,
}

impl NlpQueryProcessor {
    /// Create a new NLP query processor
    pub fn new(schema_analyzer: SchemaAnalyzer) -> Self {
        Self {
            schema_analyzer,
            intent_classifier: IntentClassifier::new(),
            entity_extractor: EntityExtractor::new(),
        }
    }

    /// Process a natural language query and extract intent
    pub async fn process_query(&self, query: &str) -> Result<QueryIntent, NlpError> {
        // Step 1: Classify intent
        let operation = self.intent_classifier.classify(query)?;

        // Step 2: Extract entities (tables, columns, values)
        let entities = self.entity_extractor.extract(query)?;

        // Step 3: Extract conditions
        let conditions = self.entity_extractor.extract_conditions(query)?;

        // Step 4: Extract projections (columns to select)
        let projections = self.entity_extractor.extract_projections(query)?;

        // Step 5: Calculate confidence based on entity recognition
        let confidence = self.calculate_confidence(&entities, &conditions);

        Ok(QueryIntent {
            operation,
            entities,
            conditions,
            projections,
            confidence,
        })
    }

    /// Calculate confidence score for the extracted intent
    fn calculate_confidence(
        &self,
        entities: &[RecognizedEntity],
        conditions: &[QueryCondition],
    ) -> f64 {
        // Base confidence
        let mut confidence: f64 = 0.5;

        // Increase confidence if we found table names
        if entities.iter().any(|e| matches!(e.entity_type, EntityType::Table)) {
            confidence += 0.2;
        }

        // Increase confidence if we found column names
        if entities.iter().any(|e| matches!(e.entity_type, EntityType::Column)) {
            confidence += 0.2;
        }

        // Increase confidence if we have conditions
        if !conditions.is_empty() {
            confidence += 0.1;
        }

        confidence.min(1.0_f64)
    }
}

/// Query intent extracted from natural language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryIntent {
    /// SQL operation type
    pub operation: SqlOperation,
    /// Recognized entities (tables, columns, values)
    pub entities: Vec<RecognizedEntity>,
    /// Query conditions (WHERE clauses)
    pub conditions: Vec<QueryCondition>,
    /// Projection columns (SELECT list)
    pub projections: Vec<ProjectionColumn>,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
}

/// SQL operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SqlOperation {
    /// SELECT query
    Select {
        /// Aggregation type if present
        aggregation: Option<AggregationType>,
        /// Result limit
        limit: Option<u64>,
        /// Ordering specification
        ordering: Option<OrderingSpec>,
    },
    /// INSERT operation
    Insert {
        /// Insert mode
        mode: InsertMode,
    },
    /// UPDATE operation
    Update {
        /// Whether update has conditions
        conditional: bool,
    },
    /// DELETE operation
    Delete {
        /// Whether delete has conditions
        conditional: bool,
    },
    /// ANALYZE operation
    Analyze {
        /// Analysis type
        analysis_type: AnalysisType,
    },
}

/// Aggregation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationType {
    Count,
    Sum,
    Average,
    Min,
    Max,
    GroupBy,
}

/// Ordering specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderingSpec {
    /// Column to order by
    pub column: String,
    /// Sort direction
    pub direction: SortDirection,
}

/// Sort direction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Insert mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InsertMode {
    /// Single row insert
    Single,
    /// Batch insert
    Batch,
    /// Insert from SELECT
    FromSelect,
}

/// Analysis type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalysisType {
    /// Summary statistics
    Summary,
    /// Distribution analysis
    Distribution,
    /// Trend analysis
    Trends,
    /// Correlation analysis
    Correlation,
}

/// Recognized entity from natural language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecognizedEntity {
    /// Entity type
    pub entity_type: EntityType,
    /// Entity name/value
    pub value: String,
    /// Original text position
    pub position: usize,
    /// Confidence in recognition
    pub confidence: f64,
}

/// Entity types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityType {
    /// Table name
    Table,
    /// Column name
    Column,
    /// Value (string, number, date)
    Value,
    /// Function name
    Function,
}

/// Query condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCondition {
    /// Column name
    pub column: String,
    /// Comparison operator
    pub operator: ComparisonOperator,
    /// Value to compare
    pub value: ConditionValue,
}

/// Comparison operator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    GreaterThan,
    GreaterOrEqual,
    LessThan,
    LessOrEqual,
    Like,
    In,
    Between,
    Similar, // For vector similarity
}

/// Condition value
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConditionValue {
    String(String),
    Number(f64),
    Integer(i64),
    Boolean(bool),
    Vector(Vec<f32>),
    List(Vec<ConditionValue>),
}

/// Projection column
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectionColumn {
    /// Column name
    pub name: String,
    /// Optional alias
    pub alias: Option<String>,
    /// Aggregation function if present
    pub aggregation: Option<AggregationType>,
}

/// Intent Classifier
pub struct IntentClassifier {
    /// Keywords for SELECT operations
    #[allow(dead_code)]
    select_keywords: Vec<&'static str>,
    /// Keywords for INSERT operations
    insert_keywords: Vec<&'static str>,
    /// Keywords for UPDATE operations
    update_keywords: Vec<&'static str>,
    /// Keywords for DELETE operations
    delete_keywords: Vec<&'static str>,
    /// Keywords for ANALYZE operations
    analyze_keywords: Vec<&'static str>,
}

impl IntentClassifier {
    /// Create a new intent classifier
    pub fn new() -> Self {
        Self {
            select_keywords: vec![
                "show", "display", "list", "get", "find", "search", "select", "fetch",
                "retrieve", "query", "what", "which", "who", "where", "when", "how many",
            ],
            insert_keywords: vec!["add", "insert", "create", "new", "add new"],
            update_keywords: vec!["update", "modify", "change", "set", "edit"],
            delete_keywords: vec!["delete", "remove", "drop", "clear"],
            analyze_keywords: vec![
                "analyze", "analysis", "statistics", "stats", "summary", "summarize",
                "distribution", "trend", "correlation", "outlier",
            ],
        }
    }

    /// Classify the intent of a natural language query
    pub fn classify(&self, query: &str) -> Result<SqlOperation, NlpError> {
        let query_lower = query.to_lowercase();

        // Check for ANALYZE intent first (more specific)
        if self.analyze_keywords.iter().any(|kw| query_lower.contains(kw)) {
            let analysis_type = if query_lower.contains("distribution") {
                AnalysisType::Distribution
            } else if query_lower.contains("trend") {
                AnalysisType::Trends
            } else if query_lower.contains("correlation") {
                AnalysisType::Correlation
            } else {
                AnalysisType::Summary
            };

            return Ok(SqlOperation::Analyze { analysis_type });
        }

        // Check for DELETE intent
        if self.delete_keywords.iter().any(|kw| query_lower.contains(kw)) {
            let conditional = query_lower.contains("where") || query_lower.contains("from");
            return Ok(SqlOperation::Delete { conditional });
        }

        // Check for UPDATE intent
        if self.update_keywords.iter().any(|kw| query_lower.contains(kw)) {
            let conditional = query_lower.contains("where");
            return Ok(SqlOperation::Update { conditional });
        }

        // Check for INSERT intent
        if self.insert_keywords.iter().any(|kw| query_lower.contains(kw)) {
            let mode = if query_lower.contains("batch") || query_lower.contains("multiple") {
                InsertMode::Batch
            } else if query_lower.contains("from") && query_lower.contains("select") {
                InsertMode::FromSelect
            } else {
                InsertMode::Single
            };

            return Ok(SqlOperation::Insert { mode });
        }

        // Default to SELECT (most common)
        let aggregation = self.extract_aggregation(&query_lower);
        let limit = self.extract_limit(&query_lower);
        let ordering = self.extract_ordering(&query_lower);

        Ok(SqlOperation::Select {
            aggregation,
            limit,
            ordering,
        })
    }

    /// Extract aggregation type from query
    fn extract_aggregation(&self, query: &str) -> Option<AggregationType> {
        if query.contains("count") || query.contains("how many") {
            Some(AggregationType::Count)
        } else if query.contains("sum") || query.contains("total") {
            Some(AggregationType::Sum)
        } else if query.contains("average") || query.contains("avg") || query.contains("mean") {
            Some(AggregationType::Average)
        } else if query.contains("minimum") || query.contains("min") || query.contains("lowest") {
            Some(AggregationType::Min)
        } else if query.contains("maximum") || query.contains("max") || query.contains("highest") {
            Some(AggregationType::Max)
        } else if query.contains("group by") || query.contains("by") {
            Some(AggregationType::GroupBy)
        } else {
            None
        }
    }

    /// Extract limit from query
    fn extract_limit(&self, query: &str) -> Option<u64> {
        // Look for patterns like "top 10", "first 5", "limit 20"
        let patterns = vec![
            (r"top\s+(\d+)", 1),
            (r"first\s+(\d+)", 1),
            (r"limit\s+(\d+)", 1),
            (r"(\d+)\s+results", 1),
        ];

        for (pattern, group) in patterns {
            if let Some(captures) = regex::Regex::new(pattern)
                .ok()
                .and_then(|re| re.captures(query))
            {
                if let Some(matched) = captures.get(group) {
                    if let Ok(num) = matched.as_str().parse::<u64>() {
                        return Some(num);
                    }
                }
            }
        }

        None
    }

    /// Extract ordering from query
    fn extract_ordering(&self, query: &str) -> Option<OrderingSpec> {
        // Look for patterns like "order by X", "sort by X", "sorted by X"
        let patterns = vec![
            r"order\s+by\s+(\w+)",
            r"sort\s+by\s+(\w+)",
            r"sorted\s+by\s+(\w+)",
        ];

        for pattern in patterns {
            if let Some(captures) = regex::Regex::new(pattern)
                .ok()
                .and_then(|re| re.captures(query))
            {
                if let Some(column_match) = captures.get(1) {
                    let column = column_match.as_str().to_string();
                    let direction = if query.contains("desc") || query.contains("descending") {
                        SortDirection::Descending
                    } else {
                        SortDirection::Ascending
                    };

                    return Some(OrderingSpec { column, direction });
                }
            }
        }

        None
    }
}

impl Default for IntentClassifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Entity Extractor
pub struct EntityExtractor {
    /// Common table name patterns
    #[allow(dead_code)]
    table_patterns: Vec<&'static str>,
    /// Common column name patterns
    #[allow(dead_code)]
    column_patterns: Vec<&'static str>,
}

impl EntityExtractor {
    /// Create a new entity extractor
    pub fn new() -> Self {
        Self {
            table_patterns: vec!["table", "from", "in"],
            column_patterns: vec!["column", "field", "attribute"],
        }
    }

    /// Extract entities from natural language query
    pub fn extract(&self, query: &str) -> Result<Vec<RecognizedEntity>, NlpError> {
        let mut entities = Vec::new();

        // Extract table names (after "from", "in", "table")
        let table_patterns = vec![
            r"from\s+(\w+)",
            r"in\s+the\s+(\w+)\s+table",
            r"table\s+(\w+)",
            r"(\w+)\s+table",
        ];

        let query_lower = query.to_lowercase();
        for pattern in table_patterns {
            if let Some(captures) = regex::Regex::new(pattern)
                .ok()
                .and_then(|re| re.captures(&query_lower))
            {
                if let Some(matched) = captures.get(1) {
                    entities.push(RecognizedEntity {
                        entity_type: EntityType::Table,
                        value: matched.as_str().to_string(),
                        position: matched.start(),
                        confidence: 0.8,
                    });
                }
            }
        }

        // Extract column names (after "where", "select", "by")
        let column_patterns = vec![
            r"where\s+(\w+)\s*[=<>]",
            r"select\s+(\w+)",
            r"by\s+(\w+)",
            r"column\s+(\w+)",
        ];

        for pattern in column_patterns {
            if let Some(captures) = regex::Regex::new(pattern)
                .ok()
                .and_then(|re| re.captures(&query_lower))
            {
                if let Some(matched) = captures.get(1) {
                    entities.push(RecognizedEntity {
                        entity_type: EntityType::Column,
                        value: matched.as_str().to_string(),
                        position: matched.start(),
                        confidence: 0.7,
                    });
                }
            }
        }

        Ok(entities)
    }

    /// Extract conditions from query
    pub fn extract_conditions(&self, query: &str) -> Result<Vec<QueryCondition>, NlpError> {
        let mut conditions = Vec::new();
        let query_lower = query.to_lowercase();

        // Simple pattern matching for common conditions
        // Pattern: "where X = Y", "X is Y", "X greater than Y", etc.
        let patterns = vec![
            (r"where\s+(\w+)\s*=\s*([^\s]+)", ComparisonOperator::Equal),
            (r"(\w+)\s+is\s+(.+)", ComparisonOperator::Equal),
            (r"(\w+)\s+greater\s+than\s+(\d+)", ComparisonOperator::GreaterThan),
            (r"(\w+)\s+less\s+than\s+(\d+)", ComparisonOperator::LessThan),
        ];

        for (pattern, operator) in patterns {
            if let Some(captures) = regex::Regex::new(pattern)
                .ok()
                .and_then(|re| re.captures(&query_lower))
            {
                if let (Some(column_match), Some(value_match)) =
                    (captures.get(1), captures.get(2))
                {
                    let column = column_match.as_str().to_string();
                    let value_str = value_match.as_str().trim();

                    let value = if let Ok(num) = value_str.parse::<f64>() {
                        ConditionValue::Number(num)
                    } else if let Ok(int) = value_str.parse::<i64>() {
                        ConditionValue::Integer(int)
                    } else {
                        ConditionValue::String(value_str.to_string())
                    };

                    conditions.push(QueryCondition {
                        column,
                        operator: operator.clone(),
                        value,
                    });
                }
            }
        }

        Ok(conditions)
    }

    /// Extract projection columns from query
    pub fn extract_projections(&self, query: &str) -> Result<Vec<ProjectionColumn>, NlpError> {
        let mut projections = Vec::new();
        let query_lower = query.to_lowercase();

        // Look for "select X", "show X", "display X"
        let patterns = vec![
            r"select\s+(\w+)",
            r"show\s+(\w+)",
            r"display\s+(\w+)",
            r"get\s+(\w+)",
        ];

        for pattern in patterns {
            if let Some(captures) = regex::Regex::new(pattern)
                .ok()
                .and_then(|re| re.captures(&query_lower))
            {
                if let Some(matched) = captures.get(1) {
                    projections.push(ProjectionColumn {
                        name: matched.as_str().to_string(),
                        alias: None,
                        aggregation: None,
                    });
                }
            }
        }

        // If no explicit projections, default to all columns
        if projections.is_empty() {
            projections.push(ProjectionColumn {
                name: "*".to_string(),
                alias: None,
                aggregation: None,
            });
        }

        Ok(projections)
    }
}

impl Default for EntityExtractor {
    fn default() -> Self {
        Self::new()
    }
}

/// Schema Analyzer (placeholder - will be implemented with actual schema discovery)
pub struct SchemaAnalyzer {
    /// Cached schema information
    schema_cache: HashMap<String, crate::mcp::sql_generator::TableSchema>,
}

impl SchemaAnalyzer {
    /// Create a new schema analyzer
    pub fn new() -> Self {
        Self {
            schema_cache: HashMap::new(),
        }
    }

    /// Get schema for a table
    pub async fn get_table_schema(
        &self,
        table_name: &str,
    ) -> Option<&crate::mcp::sql_generator::TableSchema> {
        self.schema_cache.get(table_name)
    }
}

impl Default for SchemaAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// NLP processing errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NlpError {
    /// Failed to classify intent
    IntentClassificationFailed(String),
    /// Failed to extract entities
    EntityExtractionFailed(String),
    /// Ambiguous query
    AmbiguousQuery(String),
    /// Invalid query format
    InvalidQuery(String),
}

impl std::fmt::Display for NlpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NlpError::IntentClassificationFailed(msg) => {
                write!(f, "Intent classification failed: {}", msg)
            }
            NlpError::EntityExtractionFailed(msg) => {
                write!(f, "Entity extraction failed: {}", msg)
            }
            NlpError::AmbiguousQuery(msg) => write!(f, "Ambiguous query: {}", msg),
            NlpError::InvalidQuery(msg) => write!(f, "Invalid query: {}", msg),
        }
    }
}

impl std::error::Error for NlpError {}

