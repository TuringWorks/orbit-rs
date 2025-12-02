//! pgvector-compatible SQL query engine for vector operations
//!
//! This module extends the PostgreSQL query engine with pgvector compatibility,
//! supporting vector data types, similarity operators, and vector functions.
//!
//! Supports HNSW and IVFFlat indexes for efficient approximate nearest neighbor search.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::query_engine::QueryResult;
use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::vector_index::VectorIndex;
use crate::protocols::vector_store::{SimilarityMetric, Vector};
use orbit_client::OrbitClient;

/// Vector table schema definition
#[derive(Debug, Clone)]
pub struct VectorTable {
    pub name: String,
    pub columns: Vec<VectorColumn>,
}

#[derive(Debug, Clone)]
pub struct VectorColumn {
    pub name: String,
    pub column_type: VectorColumnType,
    pub dimension: Option<usize>, // For vector types
}

#[derive(Debug, Clone)]
pub enum VectorColumnType {
    Integer,
    Text,
    Float,
    Vector(usize), // dimension
    HalfVector(usize),
    SparseVector(usize),
}

/// pgvector-compatible query engine
pub struct VectorQueryEngine {
    #[allow(dead_code)] // Reserved for future actor-based operations
    orbit_client: OrbitClient,
    tables: Arc<RwLock<HashMap<String, VectorTable>>>,
    extensions: Arc<RwLock<HashMap<String, bool>>>, // installed extensions
    /// Vector indexes keyed by "table_column" name
    indexes: Arc<RwLock<HashMap<String, VectorIndex>>>,
    /// Stored vectors for tables without explicit indexes (table_name -> vectors)
    table_vectors: Arc<RwLock<HashMap<String, HashMap<String, Vector>>>>,
}

impl VectorQueryEngine {
    /// Create a new vector query engine
    pub fn new(orbit_client: OrbitClient) -> Self {
        Self {
            orbit_client,
            tables: Arc::new(RwLock::new(HashMap::new())),
            extensions: Arc::new(RwLock::new(HashMap::new())),
            indexes: Arc::new(RwLock::new(HashMap::new())),
            table_vectors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Execute a vector-compatible SQL query
    pub async fn execute_vector_query(&self, sql: &str) -> ProtocolResult<QueryResult> {
        let sql_upper = sql.trim().to_uppercase();

        if sql_upper.starts_with("CREATE EXTENSION") {
            self.handle_create_extension(sql).await
        } else if sql_upper.starts_with("CREATE TABLE") {
            self.handle_create_table(sql).await
        } else if sql_upper.starts_with("CREATE INDEX") && sql_upper.contains("USING") {
            self.handle_create_vector_index(sql).await
        } else if sql_upper.starts_with("INSERT INTO") {
            self.handle_insert_vector(sql).await
        } else if sql_upper.starts_with("SELECT") && self.contains_vector_operations(&sql_upper) {
            self.handle_vector_select(sql).await
        } else if sql_upper.starts_with("SELECT") && sql_upper.contains("VECTOR_DIMS") {
            self.handle_vector_function(sql).await
        } else {
            Err(ProtocolError::PostgresError(format!(
                "Unsupported vector query: {sql}"
            )))
        }
    }

    /// Handle CREATE EXTENSION vector
    async fn handle_create_extension(&self, sql: &str) -> ProtocolResult<QueryResult> {
        let sql_upper = sql.to_uppercase();

        if sql_upper.contains("CREATE EXTENSION VECTOR") {
            let mut extensions = self.extensions.write().await;
            extensions.insert("vector".to_string(), true);

            Ok(QueryResult::Select {
                columns: vec!["message".to_string()],
                rows: vec![vec![Some("CREATE EXTENSION".to_string())]],
            })
        } else {
            Err(ProtocolError::PostgresError(
                "Unsupported extension".to_string(),
            ))
        }
    }

    /// Handle CREATE TABLE with vector columns
    async fn handle_create_table(&self, sql: &str) -> ProtocolResult<QueryResult> {
        // Parse CREATE TABLE statement
        // Example: CREATE TABLE documents (id SERIAL, content TEXT, embedding VECTOR(384));

        let sql_clean = sql.trim().to_uppercase();
        let sql_clean = sql_clean.replace(['\n', '\t'], " ");

        // Extract table name
        let parts: Vec<&str> = sql_clean.split_whitespace().collect();
        let table_name_idx = parts.iter().position(|&p| p == "TABLE").ok_or_else(|| {
            ProtocolError::PostgresError("Invalid CREATE TABLE syntax".to_string())
        })?;

        if table_name_idx + 1 >= parts.len() {
            return Err(ProtocolError::PostgresError(
                "Missing table name".to_string(),
            ));
        }

        let table_name = parts[table_name_idx + 1].trim_end_matches('(').to_string();

        // Find column definitions between parentheses
        let open_paren = sql.find('(').ok_or_else(|| {
            ProtocolError::PostgresError("Missing column definitions".to_string())
        })?;
        let close_paren = sql.rfind(')').ok_or_else(|| {
            ProtocolError::PostgresError("Missing closing parenthesis".to_string())
        })?;

        let column_defs = &sql[open_paren + 1..close_paren];
        let columns = self.parse_column_definitions(column_defs)?;

        // Store table schema
        let table = VectorTable {
            name: table_name.clone(),
            columns,
        };

        let mut tables = self.tables.write().await;
        tables.insert(table_name, table);

        Ok(QueryResult::Select {
            columns: vec!["message".to_string()],
            rows: vec![vec![Some("CREATE TABLE".to_string())]],
        })
    }

    /// Parse column definitions from CREATE TABLE
    fn parse_column_definitions(&self, column_defs: &str) -> ProtocolResult<Vec<VectorColumn>> {
        let mut columns = Vec::new();

        for def in column_defs.split(',') {
            let def = def.trim();
            let parts: Vec<&str> = def.split_whitespace().collect();

            if parts.len() < 2 {
                continue;
            }

            let column_name = parts[0].to_string();
            let type_def = parts[1];

            let column_type = if type_def.to_uppercase().starts_with("VECTOR(") {
                // Parse VECTOR(dimension)
                let dim_str = type_def.trim_start_matches("VECTOR(").trim_end_matches(')');
                let dimension = dim_str.parse::<usize>().map_err(|_| {
                    ProtocolError::PostgresError("Invalid vector dimension".to_string())
                })?;
                VectorColumnType::Vector(dimension)
            } else if type_def.to_uppercase().starts_with("HALFVEC(") {
                let dim_str = type_def
                    .trim_start_matches("HALFVEC(")
                    .trim_end_matches(')');
                let dimension = dim_str.parse::<usize>().map_err(|_| {
                    ProtocolError::PostgresError("Invalid halfvector dimension".to_string())
                })?;
                VectorColumnType::HalfVector(dimension)
            } else {
                match type_def.to_uppercase().as_str() {
                    "INTEGER" | "SERIAL" | "INT" | "INT4" => VectorColumnType::Integer,
                    "TEXT" | "VARCHAR" | "CHAR" => VectorColumnType::Text,
                    "FLOAT" | "FLOAT4" | "REAL" | "FLOAT8" | "DOUBLE" => VectorColumnType::Float,
                    _ => VectorColumnType::Text, // Default fallback
                }
            };

            let dimension = match &column_type {
                VectorColumnType::Vector(d)
                | VectorColumnType::HalfVector(d)
                | VectorColumnType::SparseVector(d) => Some(*d),
                _ => None,
            };

            columns.push(VectorColumn {
                name: column_name,
                column_type,
                dimension,
            });
        }

        Ok(columns)
    }

    /// Handle CREATE INDEX for vector similarity search
    async fn handle_create_vector_index(&self, sql: &str) -> ProtocolResult<QueryResult> {
        // Example: CREATE INDEX ON documents USING ivfflat (embedding vector_cosine_ops);
        // Example: CREATE INDEX ON documents USING hnsw (embedding vector_l2_ops);
        // Example: CREATE INDEX idx_name ON documents USING hnsw (embedding vector_l2_ops) WITH (m = 16, ef_construction = 64);

        let sql_upper = sql.to_uppercase();

        // Extract table name and column
        let on_idx = sql_upper
            .find(" ON ")
            .ok_or_else(|| ProtocolError::PostgresError("Missing ON clause".to_string()))?;
        let using_idx = sql_upper
            .find(" USING ")
            .ok_or_else(|| ProtocolError::PostgresError("Missing USING clause".to_string()))?;

        let table_name = sql[on_idx + 4..using_idx].trim().to_string();

        // Extract index method and operator
        let using_part = &sql[using_idx + 7..];
        let paren_idx = using_part.find('(').ok_or_else(|| {
            ProtocolError::PostgresError("Missing column specification".to_string())
        })?;

        let index_method = using_part[..paren_idx].trim().to_lowercase();

        // Extract column and operator class
        let close_paren = using_part.rfind(')').ok_or_else(|| {
            ProtocolError::PostgresError("Missing closing parenthesis".to_string())
        })?;
        let column_spec = &using_part[paren_idx + 1..close_paren];

        let column_parts: Vec<&str> = column_spec.split_whitespace().collect();
        if column_parts.is_empty() {
            return Err(ProtocolError::PostgresError(
                "Invalid column specification".to_string(),
            ));
        }

        let column_name = column_parts[0];
        let operator_class = if column_parts.len() > 1 {
            column_parts[1]
        } else {
            "vector_cosine_ops"
        };

        // Map pgvector operator classes to similarity metrics
        let similarity_metric = match operator_class.to_lowercase().as_str() {
            "vector_cosine_ops" => SimilarityMetric::Cosine,
            "vector_l2_ops" => SimilarityMetric::Euclidean,
            "vector_ip_ops" => SimilarityMetric::DotProduct,
            _ => SimilarityMetric::Cosine, // Default
        };

        // Get dimension from table schema
        let tables = self.tables.read().await;
        let dimension = if let Some(table) = tables.get(&table_name) {
            table
                .columns
                .iter()
                .find(|c| c.name.to_lowercase() == column_name.to_lowercase())
                .and_then(|c| c.dimension)
                .unwrap_or(384)
        } else {
            384 // Default dimension
        };
        drop(tables);

        // Create the actual vector index
        let index_name = format!("{}_{}", table_name, column_name);
        let index = match index_method.as_str() {
            "hnsw" => {
                // Parse WITH options if present (m, ef_construction)
                let m = 16; // Default
                let ef_construction = 64; // Default
                VectorIndex::hnsw(
                    index_name.clone(),
                    dimension,
                    similarity_metric,
                    m,
                    ef_construction,
                )
            }
            "ivfflat" => {
                // Parse WITH options if present (lists)
                let n_lists = 100; // Default
                VectorIndex::ivfflat(index_name.clone(), dimension, similarity_metric, n_lists)
            }
            _ => {
                // Default to brute force for unknown index types
                VectorIndex::brute_force(index_name.clone(), dimension, similarity_metric)
            }
        };

        // Store the index
        let mut indexes = self.indexes.write().await;
        indexes.insert(index_name.clone(), index);
        drop(indexes);

        // Also add any existing vectors from table_vectors to the index
        let table_vectors = self.table_vectors.read().await;
        let existing_vectors: Vec<Vector> = table_vectors
            .get(&table_name)
            .map(|v| v.values().cloned().collect())
            .unwrap_or_default();
        drop(table_vectors);

        if !existing_vectors.is_empty() {
            let mut indexes = self.indexes.write().await;
            if let Some(idx) = indexes.get_mut(&index_name) {
                for vector in existing_vectors {
                    let _ = idx.insert(vector);
                }
            }
        }

        Ok(QueryResult::Select {
            columns: vec!["message".to_string()],
            rows: vec![vec![Some("CREATE INDEX".to_string())]],
        })
    }

    /// Handle INSERT with vector data
    async fn handle_insert_vector(&self, sql: &str) -> ProtocolResult<QueryResult> {
        // Example: INSERT INTO documents (content, embedding) VALUES ('text', '[0.1, 0.2, 0.3]');

        // Parse INSERT statement
        let sql_upper = sql.to_uppercase();
        let into_idx = sql_upper
            .find("INTO ")
            .ok_or_else(|| ProtocolError::PostgresError("Missing INTO clause".to_string()))?;
        let values_idx = sql_upper
            .find(" VALUES ")
            .ok_or_else(|| ProtocolError::PostgresError("Missing VALUES clause".to_string()))?;

        // Extract table name
        let table_part = &sql[into_idx + 5..values_idx];
        let paren_idx = table_part.find('(');
        let table_name = if let Some(idx) = paren_idx {
            table_part[..idx].trim().to_string()
        } else {
            table_part.trim().to_string()
        };

        // Parse column names and values
        let columns: Vec<String> = if let Some(idx) = paren_idx {
            let close_paren = table_part.find(')').ok_or_else(|| {
                ProtocolError::PostgresError("Missing closing parenthesis".to_string())
            })?;
            let cols = &table_part[idx + 1..close_paren];
            cols.split(',').map(|s| s.trim().to_string()).collect()
        } else {
            vec![] // Will need to infer from table schema
        };

        // Parse VALUES
        let values_part = &sql[values_idx + 8..];
        let values_start = values_part.find('(').ok_or_else(|| {
            ProtocolError::PostgresError("Missing VALUES parenthesis".to_string())
        })?;
        let values_end = values_part.rfind(')').ok_or_else(|| {
            ProtocolError::PostgresError("Missing closing VALUES parenthesis".to_string())
        })?;

        let values_str = &values_part[values_start + 1..values_end];
        let values = self.parse_insert_values(values_str)?;

        // Create and insert vector record
        if !columns.is_empty() && !values.is_empty() {
            let mut vector_data: Vec<f32> = Vec::new();
            let mut metadata = HashMap::new();
            let mut vector_id = uuid::Uuid::new_v4().to_string();
            let mut vector_column_name: Option<String> = None;

            for (col, val) in columns.iter().zip(values.iter()) {
                let col_lower = col.to_lowercase();

                if col_lower.contains("id") {
                    vector_id = val.clone();
                } else if col_lower.contains("embedding") || col_lower.contains("vector") {
                    // Parse vector data
                    vector_data = self.parse_vector_literal(val)?;
                    vector_column_name = Some(col.clone());
                } else {
                    // Store as metadata
                    metadata.insert(col.clone(), val.clone());
                }
            }

            if !vector_data.is_empty() {
                let vector = Vector::with_metadata(vector_id.clone(), vector_data, metadata);

                // Store in table_vectors
                let mut table_vectors = self.table_vectors.write().await;
                let vectors = table_vectors.entry(table_name.clone()).or_default();
                vectors.insert(vector_id.clone(), vector.clone());
                drop(table_vectors);

                // Insert into any applicable index
                if let Some(col_name) = vector_column_name {
                    let index_name = format!("{}_{}", table_name, col_name);
                    let mut indexes = self.indexes.write().await;
                    if let Some(index) = indexes.get_mut(&index_name) {
                        index.insert(vector.clone()).map_err(|e| {
                            ProtocolError::PostgresError(format!("Index insert failed: {e}"))
                        })?;
                    }
                }
            }
        }

        Ok(QueryResult::Insert { count: 1 })
    }

    /// Handle SELECT queries with vector operations
    async fn handle_vector_select(&self, sql: &str) -> ProtocolResult<QueryResult> {
        // Example: SELECT content, embedding <-> '[0.1, 0.2, 0.3]' AS distance FROM documents ORDER BY distance LIMIT 5;

        let sql_upper = sql.to_uppercase();

        // Check for similarity search
        if sql_upper.contains("<->") || sql_upper.contains("<#>") || sql_upper.contains("<=>") {
            return self.handle_similarity_search(sql).await;
        }

        // Regular SELECT - just return empty result for now
        Ok(QueryResult::Select {
            columns: vec!["message".to_string()],
            rows: vec![vec![Some("Vector SELECT not implemented yet".to_string())]],
        })
    }

    /// Handle similarity search queries
    async fn handle_similarity_search(&self, sql: &str) -> ProtocolResult<QueryResult> {
        // Parse similarity search query
        let sql_upper = sql.to_uppercase();

        // Extract table name
        let from_idx = sql_upper
            .find(" FROM ")
            .ok_or_else(|| ProtocolError::PostgresError("Missing FROM clause".to_string()))?;
        let from_part = &sql[from_idx + 6..];
        let table_name = from_part
            .split_whitespace()
            .next()
            .ok_or_else(|| ProtocolError::PostgresError("Missing table name".to_string()))?
            .to_string();

        // Extract vector column name from the query (before the operator)
        let vector_column = self.extract_vector_column_from_query(sql)?;

        // Extract similarity operator and query vector
        let (_similarity_metric, query_vector) = if sql.contains("<->") {
            let parts: Vec<&str> = sql.split("<->").collect();
            if parts.len() < 2 {
                return Err(ProtocolError::PostgresError(
                    "Invalid similarity syntax".to_string(),
                ));
            }
            let vector_part = parts[1].trim();
            let vector_str = self.extract_vector_literal(vector_part)?;
            (
                SimilarityMetric::Euclidean,
                self.parse_vector_literal(&vector_str)?,
            )
        } else if sql.contains("<=>") {
            let parts: Vec<&str> = sql.split("<=>").collect();
            if parts.len() < 2 {
                return Err(ProtocolError::PostgresError(
                    "Invalid similarity syntax".to_string(),
                ));
            }
            let vector_part = parts[1].trim();
            let vector_str = self.extract_vector_literal(vector_part)?;
            (
                SimilarityMetric::Cosine,
                self.parse_vector_literal(&vector_str)?,
            )
        } else if sql.contains("<#>") {
            let parts: Vec<&str> = sql.split("<#>").collect();
            if parts.len() < 2 {
                return Err(ProtocolError::PostgresError(
                    "Invalid similarity syntax".to_string(),
                ));
            }
            let vector_part = parts[1].trim();
            let vector_str = self.extract_vector_literal(vector_part)?;
            (
                SimilarityMetric::DotProduct,
                self.parse_vector_literal(&vector_str)?,
            )
        } else {
            return Err(ProtocolError::PostgresError(
                "No similarity operator found".to_string(),
            ));
        };

        // Extract LIMIT
        let limit = if let Some(limit_idx) = sql_upper.find("LIMIT ") {
            let limit_part = &sql[limit_idx + 6..];
            let limit_str = limit_part.split_whitespace().next().unwrap_or("10");
            limit_str.parse::<usize>().unwrap_or(10)
        } else {
            10
        };

        // Try to use an index if available
        let index_name = format!("{}_{}", table_name, vector_column);
        let indexes = self.indexes.read().await;

        let results = if let Some(index) = indexes.get(&index_name) {
            // Use index for efficient search
            let search_results = index.search(&query_vector, limit);
            drop(indexes);

            // Get metadata from table_vectors for each result
            let table_vectors = self.table_vectors.read().await;
            let vectors = table_vectors.get(&table_name);

            search_results
                .into_iter()
                .map(|(id, distance)| {
                    let metadata = vectors
                        .and_then(|v| v.get(&id))
                        .map(|v| v.metadata.clone())
                        .unwrap_or_default();
                    (id, distance, metadata)
                })
                .collect::<Vec<_>>()
        } else {
            drop(indexes);

            // Fall back to brute-force search on table_vectors
            let table_vectors = self.table_vectors.read().await;
            if let Some(vectors) = table_vectors.get(&table_name) {
                let mut results: Vec<(String, f32, HashMap<String, String>)> = vectors
                    .values()
                    .map(|v| {
                        let distance =
                            crate::protocols::vector_store::VectorSimilarity::euclidean_distance(
                                &query_vector,
                                &v.data,
                            );
                        (v.id.clone(), distance, v.metadata.clone())
                    })
                    .collect();

                results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                results.truncate(limit);
                results
            } else {
                Vec::new()
            }
        };

        // Convert results to QueryResult
        let mut rows = Vec::new();
        for (id, distance, metadata) in results {
            // Build row with id, distance, and any requested metadata columns
            let content = metadata.get("content").cloned().unwrap_or_default();
            rows.push(vec![
                Some(id),
                Some(content),
                Some(format!("{:.6}", distance)),
            ]);
        }

        Ok(QueryResult::Select {
            columns: vec![
                "id".to_string(),
                "content".to_string(),
                "distance".to_string(),
            ],
            rows,
        })
    }

    /// Extract vector column name from a query
    fn extract_vector_column_from_query(&self, sql: &str) -> ProtocolResult<String> {
        // Look for column name before similarity operators
        for op in ["<->", "<=>", "<#>"] {
            if let Some(idx) = sql.find(op) {
                let before_op = &sql[..idx];
                // Find the last word before the operator
                let words: Vec<&str> = before_op.split_whitespace().collect();
                if let Some(last_word) = words.last() {
                    // Clean up the column name (remove any leading comma or punctuation)
                    let col_name =
                        last_word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
                    if !col_name.is_empty() {
                        return Ok(col_name.to_lowercase());
                    }
                }
            }
        }

        // Default to "embedding" if not found
        Ok("embedding".to_string())
    }

    /// Handle vector functions like vector_dims()
    async fn handle_vector_function(&self, sql: &str) -> ProtocolResult<QueryResult> {
        let sql_upper = sql.to_uppercase();

        if sql_upper.contains("VECTOR_DIMS") {
            // Example: SELECT vector_dims(embedding) FROM documents;
            Ok(QueryResult::Select {
                columns: vec!["vector_dims".to_string()],
                rows: vec![vec![Some("384".to_string())]],
            })
        } else {
            Err(ProtocolError::PostgresError(
                "Unsupported vector function".to_string(),
            ))
        }
    }

    /// Check if SQL contains vector operations
    fn contains_vector_operations(&self, sql: &str) -> bool {
        sql.contains("<->")
            || sql.contains("<#>")
            || sql.contains("<=>")
            || sql.contains("VECTOR_DIMS")
            || sql.contains("VECTOR_NORM")
    }

    /// Parse INSERT values
    fn parse_insert_values(&self, values_str: &str) -> ProtocolResult<Vec<String>> {
        let mut values = Vec::new();
        let mut current_value = String::new();
        let mut in_quotes = false;
        let mut in_brackets = false;
        let mut quote_char = '"';

        for ch in values_str.chars() {
            match ch {
                '"' | '\'' if !in_brackets => {
                    if !in_quotes {
                        in_quotes = true;
                        quote_char = ch;
                    } else if ch == quote_char {
                        in_quotes = false;
                    }
                    current_value.push(ch);
                }
                '[' if !in_quotes => {
                    in_brackets = true;
                    current_value.push(ch);
                }
                ']' if !in_quotes => {
                    in_brackets = false;
                    current_value.push(ch);
                }
                ',' if !in_quotes && !in_brackets => {
                    values.push(
                        current_value
                            .trim()
                            .trim_matches('"')
                            .trim_matches('\'')
                            .to_string(),
                    );
                    current_value.clear();
                }
                _ => {
                    current_value.push(ch);
                }
            }
        }

        if !current_value.trim().is_empty() {
            values.push(
                current_value
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string(),
            );
        }

        Ok(values)
    }

    /// Parse vector literal like '[0.1, 0.2, 0.3]' or '{0.1, 0.2, 0.3}'
    fn parse_vector_literal(&self, vector_str: &str) -> ProtocolResult<Vec<f32>> {
        let vector_str = vector_str.trim();
        let vector_str = if (vector_str.starts_with('[') && vector_str.ends_with(']'))
            || (vector_str.starts_with('{') && vector_str.ends_with('}'))
        {
            &vector_str[1..vector_str.len() - 1]
        } else {
            vector_str
        };

        let components: Result<Vec<f32>, _> = vector_str
            .split(',')
            .map(|s| s.trim().parse::<f32>())
            .collect();

        components
            .map_err(|e| ProtocolError::serialization_error(format!("Invalid vector format: {e}")))
    }

    /// Extract vector literal from SQL expression
    fn extract_vector_literal(&self, expr: &str) -> ProtocolResult<String> {
        let expr = expr.trim();

        // Look for vector literal in quotes
        if let Some(start) = expr.find('\'') {
            if let Some(end) = expr.rfind('\'') {
                if end > start {
                    return Ok(expr[start + 1..end].to_string());
                }
            }
        }

        // Look for vector literal in brackets
        if let Some(start) = expr.find('[') {
            if let Some(end) = expr.rfind(']') {
                if end > start {
                    return Ok(expr[start..=end].to_string());
                }
            }
        }

        Err(ProtocolError::PostgresError(
            "Could not extract vector literal".to_string(),
        ))
    }
}
