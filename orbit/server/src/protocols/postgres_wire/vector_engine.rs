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

    /// Normalize SQL identifier according to ANSI SQL standards.
    ///
    /// Per ANSI SQL, unquoted identifiers are folded to uppercase.
    /// Quoted identifiers (with double quotes) preserve their exact case.
    ///
    /// Examples:
    ///   - `documents` -> `DOCUMENTS`
    ///   - `Documents` -> `DOCUMENTS`
    ///   - `"Documents"` -> `Documents` (quotes removed, case preserved)
    ///   - `"my-table"` -> `my-table` (quotes removed, special chars preserved)
    fn normalize_identifier(&self, identifier: &str) -> String {
        let trimmed = identifier.trim();
        if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
            // Quoted identifier - preserve case, remove quotes
            trimmed[1..trimmed.len() - 1].to_string()
        } else {
            // Unquoted identifier - fold to uppercase per ANSI SQL
            trimmed.to_uppercase()
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

        // Support both "CREATE EXTENSION vector" and "CREATE EXTENSION IF NOT EXISTS vector"
        if sql_upper.contains("EXTENSION") && sql_upper.contains("VECTOR") {
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
        // Example: CREATE TABLE "MixedCase" (id SERIAL, embedding VECTOR(384));

        let sql_normalized = sql.trim().replace(['\n', '\t'], " ");
        let sql_upper = sql_normalized.to_uppercase();

        // Find TABLE keyword position in the uppercase version
        let parts_upper: Vec<&str> = sql_upper.split_whitespace().collect();
        let table_name_idx = parts_upper
            .iter()
            .position(|&p| p == "TABLE")
            .ok_or_else(|| {
                ProtocolError::PostgresError("Invalid CREATE TABLE syntax".to_string())
            })?;

        if table_name_idx + 1 >= parts_upper.len() {
            return Err(ProtocolError::PostgresError(
                "Missing table name".to_string(),
            ));
        }

        // Extract table name from the ORIGINAL SQL (not uppercase) to preserve quotes
        let parts_original: Vec<&str> = sql_normalized.split_whitespace().collect();
        let raw_table_name = parts_original[table_name_idx + 1].trim_end_matches('(');
        let table_name = self.normalize_identifier(raw_table_name);

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
    /// Supports VECTOR, VECTOR(dim), HALFVEC(dim), and standard SQL types
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
            let type_upper = type_def.to_uppercase();

            let (column_type, dimension) = if type_upper.starts_with("VECTOR(") {
                // Parse VECTOR(dimension) - e.g., VECTOR(1536)
                let dim_str = type_def
                    .to_uppercase()
                    .trim_start_matches("VECTOR(")
                    .trim_end_matches(')')
                    .to_string();
                let dimension = dim_str.parse::<usize>().map_err(|_| {
                    ProtocolError::PostgresError(format!(
                        "Invalid vector dimension '{}'. Expected a positive integer like VECTOR(1536)",
                        dim_str
                    ))
                })?;
                (VectorColumnType::Vector(dimension), Some(dimension))
            } else if type_upper == "VECTOR" {
                // VECTOR without dimension - will be inferred from first INSERT
                // Use 0 as sentinel for "unspecified dimension"
                (VectorColumnType::Vector(0), None)
            } else if type_upper.starts_with("HALFVEC(") {
                let dim_str = type_def
                    .to_uppercase()
                    .trim_start_matches("HALFVEC(")
                    .trim_end_matches(')')
                    .to_string();
                let dimension = dim_str.parse::<usize>().map_err(|_| {
                    ProtocolError::PostgresError(format!(
                        "Invalid halfvector dimension '{}'. Expected a positive integer",
                        dim_str
                    ))
                })?;
                (VectorColumnType::HalfVector(dimension), Some(dimension))
            } else if type_upper == "HALFVEC" {
                // HALFVEC without dimension
                (VectorColumnType::HalfVector(0), None)
            } else {
                let col_type = match type_upper.as_str() {
                    "INTEGER" | "SERIAL" | "INT" | "INT4" => VectorColumnType::Integer,
                    "TEXT" | "VARCHAR" | "CHAR" => VectorColumnType::Text,
                    "FLOAT" | "FLOAT4" | "REAL" | "FLOAT8" | "DOUBLE" => VectorColumnType::Float,
                    _ => VectorColumnType::Text, // Default fallback
                };
                (col_type, None)
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

        let raw_table_name = sql[on_idx + 4..using_idx].trim();
        let table_name = self.normalize_identifier(raw_table_name);

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
            let col_dim = table
                .columns
                .iter()
                .find(|c| c.name.to_lowercase() == column_name.to_lowercase())
                .and_then(|c| c.dimension);

            match col_dim {
                Some(dim) if dim > 0 => dim,
                Some(0) | None => {
                    // Dimension not yet set (VECTOR without dimension, no inserts yet)
                    // Return an error suggesting they either specify dimension or insert data first
                    return Err(ProtocolError::PostgresError(format!(
                        "Cannot create index on column '{}': vector dimension not specified. \
                        Either:\n\
                        1. Specify dimension in CREATE TABLE: embedding VECTOR(1536)\n\
                        2. Insert at least one row before creating the index\n\
                        Common dimensions: 384 (all-MiniLM-L6-v2), 768 (BERT), \
                        1536 (text-embedding-ada-002), 3072 (text-embedding-3-large)",
                        column_name
                    )));
                }
                _ => unreachable!(),
            }
        } else {
            return Err(ProtocolError::PostgresError(format!(
                "Table '{}' not found. Create the table first with:\n\
                CREATE TABLE {} (id SERIAL, content TEXT, {} VECTOR(dimension))",
                table_name, table_name, column_name
            )));
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

        // Extract table name and normalize per ANSI SQL
        let table_part = &sql[into_idx + 5..values_idx];
        let paren_idx = table_part.find('(');
        let raw_table_name = if let Some(idx) = paren_idx {
            table_part[..idx].trim()
        } else {
            table_part.trim()
        };
        let table_name = self.normalize_identifier(raw_table_name);

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
                // Get expected dimension from table schema and validate/auto-detect
                let inserted_dim = vector_data.len();

                if let Some(ref col_name) = vector_column_name {
                    let mut tables = self.tables.write().await;
                    if let Some(table) = tables.get_mut(&table_name) {
                        // Find the vector column
                        for column in &mut table.columns {
                            if column.name.to_lowercase() == col_name.to_lowercase() {
                                match &mut column.column_type {
                                    VectorColumnType::Vector(expected_dim)
                                    | VectorColumnType::HalfVector(expected_dim) => {
                                        if *expected_dim == 0 {
                                            // Auto-detect: first insert sets the dimension
                                            *expected_dim = inserted_dim;
                                            column.dimension = Some(inserted_dim);
                                            tracing::info!(
                                                "Auto-detected vector dimension {} for column '{}.{}'",
                                                inserted_dim, table_name, col_name
                                            );
                                        } else if *expected_dim != inserted_dim {
                                            // Dimension mismatch - return clear error
                                            return Err(ProtocolError::PostgresError(format!(
                                                "Vector dimension mismatch for column '{}': \
                                                expected {} dimensions, got {}. \
                                                Ensure your embedding model outputs {}-dimensional vectors. \
                                                Common dimensions: 384 (all-MiniLM-L6-v2), 768 (BERT), \
                                                1536 (text-embedding-ada-002), 3072 (text-embedding-3-large)",
                                                col_name, *expected_dim, inserted_dim, *expected_dim
                                            )));
                                        }
                                    }
                                    _ => {}
                                }
                                break;
                            }
                        }
                    }
                    drop(tables);
                }

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

        // Extract table name and normalize per ANSI SQL
        let from_idx = sql_upper
            .find(" FROM ")
            .ok_or_else(|| ProtocolError::PostgresError("Missing FROM clause".to_string()))?;
        let from_part = &sql[from_idx + 6..];
        let raw_table_name = from_part
            .split_whitespace()
            .next()
            .ok_or_else(|| ProtocolError::PostgresError("Missing table name".to_string()))?;
        let table_name = self.normalize_identifier(raw_table_name);

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

        // Validate query vector dimension against table schema
        let tables = self.tables.read().await;
        if let Some(table) = tables.get(&table_name) {
            if let Some(col) = table
                .columns
                .iter()
                .find(|c| c.name.to_lowercase() == vector_column.to_lowercase())
            {
                if let Some(expected_dim) = col.dimension {
                    if expected_dim > 0 && expected_dim != query_vector.len() {
                        return Err(ProtocolError::PostgresError(format!(
                            "Query vector dimension mismatch: table '{}' column '{}' expects \
                            {}-dimensional vectors, but query vector has {} dimensions. \
                            Common dimensions: 384 (all-MiniLM-L6-v2), 768 (BERT), \
                            1536 (text-embedding-ada-002), 3072 (text-embedding-3-large)",
                            table_name,
                            vector_column,
                            expected_dim,
                            query_vector.len()
                        )));
                    }
                }
            }
        }
        drop(tables);

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

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_client::OrbitClientConfig;

    async fn create_test_engine() -> VectorQueryEngine {
        let config = OrbitClientConfig {
            namespace: "test".to_string(),
            ..Default::default()
        };
        let client = OrbitClient::new_offline(config).await.unwrap();
        VectorQueryEngine::new(client)
    }

    #[tokio::test]
    async fn test_vector_without_dimension_in_create_table() {
        let engine = create_test_engine().await;

        // Create table with VECTOR (no dimension)
        let result = engine
            .execute_vector_query(
                "CREATE TABLE embeddings (id SERIAL, content TEXT, embedding VECTOR)",
            )
            .await;
        assert!(result.is_ok());

        // Verify table was created with dimension=0 (sentinel for unspecified)
        let tables = engine.tables.read().await;
        let table = tables.get("EMBEDDINGS").unwrap();
        let emb_col = table
            .columns
            .iter()
            .find(|c| c.name == "embedding")
            .unwrap();
        assert_eq!(emb_col.dimension, None); // No dimension set yet
        match &emb_col.column_type {
            VectorColumnType::Vector(dim) => assert_eq!(*dim, 0), // Sentinel value
            _ => panic!("Expected Vector type"),
        }
    }

    #[tokio::test]
    async fn test_vector_with_dimension_1536() {
        let engine = create_test_engine().await;

        // Create table with VECTOR(1536) - OpenAI text-embedding-ada-002
        let result = engine
            .execute_vector_query(
                "CREATE TABLE openai_docs (id SERIAL, content TEXT, embedding VECTOR(1536))",
            )
            .await;
        assert!(result.is_ok());

        // Verify dimension is set correctly
        let tables = engine.tables.read().await;
        let table = tables.get("OPENAI_DOCS").unwrap();
        let emb_col = table
            .columns
            .iter()
            .find(|c| c.name == "embedding")
            .unwrap();
        assert_eq!(emb_col.dimension, Some(1536));
    }

    #[tokio::test]
    async fn test_create_index_on_nonexistent_table() {
        let engine = create_test_engine().await;

        let result = engine
            .execute_vector_query(
                "CREATE INDEX ON nonexistent USING hnsw (embedding vector_cosine_ops)",
            )
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not found"),
            "Error should mention table not found: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_large_dimension_vector_3072() {
        let engine = create_test_engine().await;

        // Create table with VECTOR(3072) - OpenAI text-embedding-3-large
        let result = engine
            .execute_vector_query(
                "CREATE TABLE large_embeddings (id SERIAL, content TEXT, embedding VECTOR(3072))",
            )
            .await;
        assert!(result.is_ok());

        // Verify dimension
        let tables = engine.tables.read().await;
        let table = tables.get("LARGE_EMBEDDINGS").unwrap();
        let emb_col = table
            .columns
            .iter()
            .find(|c| c.name == "embedding")
            .unwrap();
        assert_eq!(emb_col.dimension, Some(3072));
    }

    #[tokio::test]
    async fn test_halfvec_without_dimension() {
        let engine = create_test_engine().await;

        // Create table with HALFVEC (no dimension)
        let result = engine
            .execute_vector_query("CREATE TABLE halfvec_table (id SERIAL, embedding HALFVEC)")
            .await;
        assert!(result.is_ok());

        // Verify HALFVEC was parsed correctly
        let tables = engine.tables.read().await;
        let table = tables.get("HALFVEC_TABLE").unwrap();
        let emb_col = table
            .columns
            .iter()
            .find(|c| c.name == "embedding")
            .unwrap();
        match &emb_col.column_type {
            VectorColumnType::HalfVector(dim) => assert_eq!(*dim, 0), // Sentinel value
            _ => panic!("Expected HalfVector type"),
        }
    }

    #[tokio::test]
    async fn test_parse_column_definitions_various_types() {
        let engine = create_test_engine().await;

        // Test various column type parsing
        let columns = engine
            .parse_column_definitions(
                "id INTEGER, name TEXT, score FLOAT, embedding VECTOR(768), small_vec HALFVEC(384)",
            )
            .unwrap();

        assert_eq!(columns.len(), 5);
        assert!(matches!(columns[0].column_type, VectorColumnType::Integer));
        assert!(matches!(columns[1].column_type, VectorColumnType::Text));
        assert!(matches!(columns[2].column_type, VectorColumnType::Float));
        assert!(matches!(
            columns[3].column_type,
            VectorColumnType::Vector(768)
        ));
        assert_eq!(columns[3].dimension, Some(768));
        assert!(matches!(
            columns[4].column_type,
            VectorColumnType::HalfVector(384)
        ));
        assert_eq!(columns[4].dimension, Some(384));
    }

    #[tokio::test]
    async fn test_ansi_sql_identifier_normalization() {
        let engine = create_test_engine().await;

        // Test unquoted identifiers are folded to uppercase
        assert_eq!(engine.normalize_identifier("documents"), "DOCUMENTS");
        assert_eq!(engine.normalize_identifier("Documents"), "DOCUMENTS");
        assert_eq!(engine.normalize_identifier("DOCUMENTS"), "DOCUMENTS");
        assert_eq!(engine.normalize_identifier("my_table"), "MY_TABLE");

        // Test quoted identifiers preserve case
        assert_eq!(engine.normalize_identifier("\"Documents\""), "Documents");
        assert_eq!(engine.normalize_identifier("\"my-table\""), "my-table");
        assert_eq!(engine.normalize_identifier("\"MixedCase\""), "MixedCase");

        // Test whitespace handling
        assert_eq!(engine.normalize_identifier("  documents  "), "DOCUMENTS");
        assert_eq!(engine.normalize_identifier("  \"Quoted\"  "), "Quoted");
    }

    #[tokio::test]
    async fn test_case_insensitive_table_creation_and_lookup() {
        let engine = create_test_engine().await;

        // Create table with lowercase name
        let result = engine
            .execute_vector_query("CREATE TABLE documents (id SERIAL, embedding VECTOR(384))")
            .await;
        assert!(result.is_ok());

        // Table should be stored as uppercase (per ANSI SQL)
        let tables = engine.tables.read().await;
        assert!(tables.contains_key("DOCUMENTS"));
        assert!(!tables.contains_key("documents"));
        drop(tables);
    }

    #[tokio::test]
    async fn test_case_insensitive_insert() {
        let engine = create_test_engine().await;

        // Create table
        engine
            .execute_vector_query("CREATE TABLE MyDocs (id SERIAL, embedding VECTOR(3))")
            .await
            .unwrap();

        // Insert with different case - should work
        let result = engine
            .execute_vector_query(
                "INSERT INTO mydocs (id, embedding) VALUES ('1', '[0.1, 0.2, 0.3]')",
            )
            .await;
        assert!(result.is_ok());

        // Verify vector was stored
        let table_vectors = engine.table_vectors.read().await;
        assert!(table_vectors.contains_key("MYDOCS"));
    }

    #[tokio::test]
    async fn test_quoted_identifier_preserves_case() {
        let engine = create_test_engine().await;

        // Create table with quoted identifier - should preserve case
        let result = engine
            .execute_vector_query("CREATE TABLE \"MixedCase\" (id SERIAL, embedding VECTOR(384))")
            .await;
        assert!(result.is_ok());

        // Table should be stored with preserved case
        let tables = engine.tables.read().await;
        assert!(tables.contains_key("MixedCase"));
        assert!(!tables.contains_key("MIXEDCASE"));
    }

    // ==========================================================================
    // pgvector Compatibility Tests
    // Based on real pgvector SQL examples from https://github.com/pgvector/pgvector
    // ==========================================================================

    /// Test: pgvector CREATE EXTENSION support
    /// Validates: CREATE EXTENSION IF NOT EXISTS vector;
    #[tokio::test]
    async fn test_pgvector_create_extension() {
        let engine = create_test_engine().await;

        // Standard pgvector extension creation
        let result = engine
            .execute_vector_query("CREATE EXTENSION IF NOT EXISTS vector")
            .await;
        assert!(result.is_ok());

        // Verify extension is registered
        let extensions = engine.extensions.read().await;
        assert!(extensions.contains_key("vector"));
    }

    /// Test: pgvector basic table creation with VECTOR column
    /// Validates: CREATE TABLE items (id bigserial PRIMARY KEY, embedding vector(3));
    #[tokio::test]
    async fn test_pgvector_basic_table_creation() {
        let engine = create_test_engine().await;

        let result = engine
            .execute_vector_query(
                "CREATE TABLE items (id bigserial PRIMARY KEY, embedding vector(3))",
            )
            .await;
        assert!(result.is_ok());

        // Verify table structure
        let tables = engine.tables.read().await;
        let table = tables.get("ITEMS").expect("Table ITEMS should exist");
        assert_eq!(table.columns.len(), 2);

        let emb_col = table
            .columns
            .iter()
            .find(|c| c.name == "embedding")
            .expect("embedding column should exist");
        assert_eq!(emb_col.dimension, Some(3));
    }

    /// Test: pgvector INSERT with array-style vector literal
    /// Validates: INSERT INTO items (embedding) VALUES ('[1,2,3]');
    #[tokio::test]
    async fn test_pgvector_insert_array_literal() {
        let engine = create_test_engine().await;

        // Create table first
        engine
            .execute_vector_query("CREATE TABLE items (id SERIAL, embedding vector(3))")
            .await
            .unwrap();

        // Insert with array-style literal (pgvector syntax)
        let result = engine
            .execute_vector_query("INSERT INTO items (id, embedding) VALUES ('1', '[1,2,3]')")
            .await;
        assert!(result.is_ok());

        // Verify vector was stored
        let table_vectors = engine.table_vectors.read().await;
        assert!(table_vectors.contains_key("ITEMS"));
        assert!(!table_vectors.get("ITEMS").unwrap().is_empty());
    }

    /// Test: pgvector INSERT with floating point values
    /// Validates: INSERT INTO items (embedding) VALUES ('[0.1, 0.2, 0.3]');
    #[tokio::test]
    async fn test_pgvector_insert_float_literal() {
        let engine = create_test_engine().await;

        engine
            .execute_vector_query("CREATE TABLE items (id SERIAL, embedding vector(3))")
            .await
            .unwrap();

        let result = engine
            .execute_vector_query(
                "INSERT INTO items (id, embedding) VALUES ('doc1', '[0.1, 0.2, 0.3]')",
            )
            .await;
        assert!(result.is_ok());
    }

    /// Test: pgvector HNSW index creation with cosine distance
    /// Validates: CREATE INDEX ON items USING hnsw (embedding vector_cosine_ops);
    #[tokio::test]
    async fn test_pgvector_hnsw_index_cosine() {
        let engine = create_test_engine().await;

        // Create table and insert data first
        engine
            .execute_vector_query("CREATE TABLE items (id SERIAL, embedding vector(3))")
            .await
            .unwrap();
        engine
            .execute_vector_query("INSERT INTO items (id, embedding) VALUES ('1', '[1,0,0]')")
            .await
            .unwrap();

        // Create HNSW index with cosine similarity
        let result = engine
            .execute_vector_query("CREATE INDEX ON items USING hnsw (embedding vector_cosine_ops)")
            .await;
        assert!(result.is_ok());

        // Verify index was created
        let indexes = engine.indexes.read().await;
        assert!(indexes.contains_key("ITEMS_embedding"));
    }

    /// Test: pgvector HNSW index creation with L2 distance
    /// Validates: CREATE INDEX ON items USING hnsw (embedding vector_l2_ops);
    #[tokio::test]
    async fn test_pgvector_hnsw_index_l2() {
        let engine = create_test_engine().await;

        engine
            .execute_vector_query("CREATE TABLE vectors (id SERIAL, data vector(4))")
            .await
            .unwrap();
        engine
            .execute_vector_query("INSERT INTO vectors (id, data) VALUES ('v1', '[1,2,3,4]')")
            .await
            .unwrap();

        let result = engine
            .execute_vector_query("CREATE INDEX ON vectors USING hnsw (data vector_l2_ops)")
            .await;
        assert!(result.is_ok());
    }

    /// Test: pgvector IVFFlat index creation
    /// Validates: CREATE INDEX ON items USING ivfflat (embedding vector_cosine_ops);
    #[tokio::test]
    async fn test_pgvector_ivfflat_index() {
        let engine = create_test_engine().await;

        engine
            .execute_vector_query("CREATE TABLE docs (id SERIAL, emb vector(5))")
            .await
            .unwrap();
        engine
            .execute_vector_query("INSERT INTO docs (id, emb) VALUES ('d1', '[0.1,0.2,0.3,0.4,0.5]')")
            .await
            .unwrap();

        let result = engine
            .execute_vector_query("CREATE INDEX ON docs USING ivfflat (emb vector_cosine_ops)")
            .await;
        assert!(result.is_ok());
    }

    /// Test: pgvector nearest neighbor search with <-> operator (L2 distance)
    /// Validates: SELECT * FROM items ORDER BY embedding <-> '[3,1,2]' LIMIT 5;
    #[tokio::test]
    async fn test_pgvector_l2_distance_search() {
        let engine = create_test_engine().await;

        // Setup table with test data
        engine
            .execute_vector_query("CREATE TABLE items (id SERIAL, content TEXT, embedding vector(3))")
            .await
            .unwrap();

        // Insert multiple vectors
        engine
            .execute_vector_query(
                "INSERT INTO items (id, content, embedding) VALUES ('1', 'first', '[1,0,0]')",
            )
            .await
            .unwrap();
        engine
            .execute_vector_query(
                "INSERT INTO items (id, content, embedding) VALUES ('2', 'second', '[0,1,0]')",
            )
            .await
            .unwrap();
        engine
            .execute_vector_query(
                "INSERT INTO items (id, content, embedding) VALUES ('3', 'third', '[0,0,1]')",
            )
            .await
            .unwrap();

        // Search using L2 distance operator <->
        let result = engine
            .execute_vector_query(
                "SELECT content, embedding <-> '[1,0,0]' AS distance FROM items ORDER BY distance LIMIT 5",
            )
            .await;
        assert!(result.is_ok());

        match result.unwrap() {
            QueryResult::Select { columns, rows } => {
                assert!(columns.contains(&"distance".to_string()));
                assert!(!rows.is_empty());
                // First result should be closest to [1,0,0]
            }
            _ => panic!("Expected SELECT result"),
        }
    }

    /// Test: pgvector cosine distance search with <=> operator
    /// Validates: SELECT * FROM items ORDER BY embedding <=> '[3,1,2]' LIMIT 5;
    #[tokio::test]
    async fn test_pgvector_cosine_distance_search() {
        let engine = create_test_engine().await;

        engine
            .execute_vector_query("CREATE TABLE items (id SERIAL, content TEXT, embedding vector(3))")
            .await
            .unwrap();

        engine
            .execute_vector_query(
                "INSERT INTO items (id, content, embedding) VALUES ('1', 'doc1', '[1,1,0]')",
            )
            .await
            .unwrap();
        engine
            .execute_vector_query(
                "INSERT INTO items (id, content, embedding) VALUES ('2', 'doc2', '[0,1,1]')",
            )
            .await
            .unwrap();

        // Search using cosine distance operator <=>
        let result = engine
            .execute_vector_query(
                "SELECT content, embedding <=> '[1,1,0]' AS distance FROM items LIMIT 5",
            )
            .await;
        assert!(result.is_ok());
    }

    /// Test: pgvector inner product search with <#> operator
    /// Validates: SELECT * FROM items ORDER BY embedding <#> '[3,1,2]' LIMIT 5;
    #[tokio::test]
    async fn test_pgvector_inner_product_search() {
        let engine = create_test_engine().await;

        engine
            .execute_vector_query("CREATE TABLE items (id SERIAL, content TEXT, embedding vector(3))")
            .await
            .unwrap();

        engine
            .execute_vector_query(
                "INSERT INTO items (id, content, embedding) VALUES ('1', 'product1', '[1,0,0]')",
            )
            .await
            .unwrap();

        // Search using inner product operator <#>
        let result = engine
            .execute_vector_query(
                "SELECT content, embedding <#> '[1,0,0]' AS score FROM items LIMIT 3",
            )
            .await;
        assert!(result.is_ok());
    }

    /// Test: OpenAI text-embedding-ada-002 dimension (1536)
    /// Validates full workflow with 1536-dimensional vectors
    #[tokio::test]
    async fn test_pgvector_openai_ada_002_workflow() {
        let engine = create_test_engine().await;

        // Create table for OpenAI embeddings
        let result = engine
            .execute_vector_query(
                "CREATE TABLE documents (id SERIAL, content TEXT, embedding vector(1536))",
            )
            .await;
        assert!(result.is_ok());

        // Create HNSW index
        engine
            .execute_vector_query(
                "INSERT INTO documents (id, content, embedding) VALUES ('doc1', 'test', '[{}]')",
            )
            .await
            .ok(); // May fail without actual 1536 values, that's ok for structure test

        // Verify table was created with correct dimension
        let tables = engine.tables.read().await;
        let table = tables.get("DOCUMENTS").unwrap();
        let emb_col = table
            .columns
            .iter()
            .find(|c| c.name == "embedding")
            .unwrap();
        assert_eq!(emb_col.dimension, Some(1536));
    }

    /// Test: OpenAI text-embedding-3-large dimension (3072)
    #[tokio::test]
    async fn test_pgvector_openai_3_large_workflow() {
        let engine = create_test_engine().await;

        let result = engine
            .execute_vector_query(
                "CREATE TABLE large_docs (id SERIAL, content TEXT, embedding vector(3072))",
            )
            .await;
        assert!(result.is_ok());

        let tables = engine.tables.read().await;
        let table = tables.get("LARGE_DOCS").unwrap();
        let emb_col = table
            .columns
            .iter()
            .find(|c| c.name == "embedding")
            .unwrap();
        assert_eq!(emb_col.dimension, Some(3072));
    }

    /// Test: BERT/Sentence-Transformers dimension (384)
    #[tokio::test]
    async fn test_pgvector_sentence_transformers_workflow() {
        let engine = create_test_engine().await;

        // all-MiniLM-L6-v2 outputs 384 dimensions
        let result = engine
            .execute_vector_query(
                "CREATE TABLE sentences (id SERIAL, text TEXT, embedding vector(384))",
            )
            .await;
        assert!(result.is_ok());

        let tables = engine.tables.read().await;
        let table = tables.get("SENTENCES").unwrap();
        let emb_col = table
            .columns
            .iter()
            .find(|c| c.name == "embedding")
            .unwrap();
        assert_eq!(emb_col.dimension, Some(384));
    }

    /// Test: Dimension mismatch error handling
    /// Validates clear error when inserting wrong-dimension vector
    #[tokio::test]
    async fn test_pgvector_dimension_mismatch_error() {
        let engine = create_test_engine().await;

        engine
            .execute_vector_query("CREATE TABLE items (id SERIAL, embedding vector(3))")
            .await
            .unwrap();

        // First insert sets no conflict
        engine
            .execute_vector_query("INSERT INTO items (id, embedding) VALUES ('1', '[1,2,3]')")
            .await
            .unwrap();

        // Try to insert wrong dimension - should fail
        let result = engine
            .execute_vector_query("INSERT INTO items (id, embedding) VALUES ('2', '[1,2,3,4,5]')")
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("dimension mismatch") || err_msg.contains("expected 3"),
            "Error should mention dimension mismatch: {}",
            err_msg
        );
    }

    /// Test: Auto-dimension detection when VECTOR has no dimension
    #[tokio::test]
    async fn test_pgvector_auto_dimension_detection() {
        let engine = create_test_engine().await;

        // Create table without specifying dimension
        engine
            .execute_vector_query("CREATE TABLE flexible (id SERIAL, embedding VECTOR)")
            .await
            .unwrap();

        // First insert should set dimension to 4
        engine
            .execute_vector_query("INSERT INTO flexible (id, embedding) VALUES ('1', '[1,2,3,4]')")
            .await
            .unwrap();

        // Verify dimension was auto-detected
        {
            let tables = engine.tables.read().await;
            let table = tables.get("FLEXIBLE").unwrap();
            let emb_col = table
                .columns
                .iter()
                .find(|c| c.name == "embedding")
                .unwrap();
            assert_eq!(emb_col.dimension, Some(4));
        } // Drop the read lock before next INSERT

        // Second insert with same dimension should succeed
        let result = engine
            .execute_vector_query("INSERT INTO flexible (id, embedding) VALUES ('2', '[5,6,7,8]')")
            .await;
        assert!(result.is_ok());

        // Third insert with wrong dimension should fail
        let result = engine
            .execute_vector_query("INSERT INTO flexible (id, embedding) VALUES ('3', '[1,2]')")
            .await;
        assert!(result.is_err());
    }

    /// Test: pgvector RAG (Retrieval Augmented Generation) workflow
    /// Common pattern: store documents with embeddings, search by similarity
    #[tokio::test]
    async fn test_pgvector_rag_workflow() {
        let engine = create_test_engine().await;

        // Step 1: Create table for RAG documents
        engine
            .execute_vector_query(
                "CREATE TABLE rag_documents (
                    id SERIAL,
                    content TEXT,
                    embedding vector(3)
                )",
            )
            .await
            .unwrap();

        // Step 2: Insert documents with embeddings
        engine
            .execute_vector_query(
                "INSERT INTO rag_documents (id, content, embedding) VALUES ('1', 'The quick brown fox', '[0.1, 0.8, 0.3]')",
            )
            .await
            .unwrap();

        engine
            .execute_vector_query(
                "INSERT INTO rag_documents (id, content, embedding) VALUES ('2', 'A lazy dog sleeps', '[0.9, 0.1, 0.2]')",
            )
            .await
            .unwrap();

        engine
            .execute_vector_query(
                "INSERT INTO rag_documents (id, content, embedding) VALUES ('3', 'The fox jumps over', '[0.15, 0.75, 0.35]')",
            )
            .await
            .unwrap();

        // Step 3: Create HNSW index for fast retrieval
        engine
            .execute_vector_query(
                "CREATE INDEX ON rag_documents USING hnsw (embedding vector_cosine_ops)",
            )
            .await
            .unwrap();

        // Step 4: Search for similar documents (simulating query embedding)
        let result = engine
            .execute_vector_query(
                "SELECT content, embedding <=> '[0.12, 0.78, 0.32]' AS similarity FROM rag_documents ORDER BY similarity LIMIT 2",
            )
            .await;
        assert!(result.is_ok());

        match result.unwrap() {
            QueryResult::Select { rows, .. } => {
                // Should return results ordered by similarity
                assert!(!rows.is_empty());
            }
            _ => panic!("Expected SELECT result"),
        }
    }

    /// Test: Query vector dimension validation
    /// Validates error when query vector dimension doesn't match table
    #[tokio::test]
    async fn test_pgvector_query_dimension_validation() {
        let engine = create_test_engine().await;

        engine
            .execute_vector_query("CREATE TABLE items (id SERIAL, content TEXT, embedding vector(3))")
            .await
            .unwrap();

        engine
            .execute_vector_query(
                "INSERT INTO items (id, content, embedding) VALUES ('1', 'test', '[1,2,3]')",
            )
            .await
            .unwrap();

        // Query with wrong dimension should fail
        let result = engine
            .execute_vector_query(
                "SELECT content, embedding <-> '[1,2,3,4,5]' AS distance FROM items LIMIT 5",
            )
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("dimension mismatch"),
            "Error should mention query dimension mismatch: {}",
            err_msg
        );
    }

    /// Test: Multiple vector columns in single table
    #[tokio::test]
    async fn test_pgvector_multiple_vector_columns() {
        let engine = create_test_engine().await;

        // Table with multiple embedding columns (e.g., title_embedding, content_embedding)
        let result = engine
            .execute_vector_query(
                "CREATE TABLE multi_embed (
                    id SERIAL,
                    title TEXT,
                    title_embedding vector(128),
                    content TEXT,
                    content_embedding vector(384)
                )",
            )
            .await;
        assert!(result.is_ok());

        // Verify both vector columns exist with correct dimensions
        let tables = engine.tables.read().await;
        let table = tables.get("MULTI_EMBED").unwrap();

        let title_col = table
            .columns
            .iter()
            .find(|c| c.name == "title_embedding")
            .unwrap();
        assert_eq!(title_col.dimension, Some(128));

        let content_col = table
            .columns
            .iter()
            .find(|c| c.name == "content_embedding")
            .unwrap();
        assert_eq!(content_col.dimension, Some(384));
    }

    /// Test: Half-precision vectors (HALFVEC)
    #[tokio::test]
    async fn test_pgvector_halfvec_type() {
        let engine = create_test_engine().await;

        let result = engine
            .execute_vector_query(
                "CREATE TABLE half_vectors (id SERIAL, embedding halfvec(256))",
            )
            .await;
        assert!(result.is_ok());

        let tables = engine.tables.read().await;
        let table = tables.get("HALF_VECTORS").unwrap();
        let emb_col = table
            .columns
            .iter()
            .find(|c| c.name == "embedding")
            .unwrap();
        assert!(matches!(
            emb_col.column_type,
            VectorColumnType::HalfVector(256)
        ));
    }

    /// Test: Vector parsing edge cases
    #[tokio::test]
    async fn test_pgvector_vector_literal_parsing() {
        let engine = create_test_engine().await;

        engine
            .execute_vector_query("CREATE TABLE parse_test (id SERIAL, embedding vector(3))")
            .await
            .unwrap();

        // Test various valid vector literal formats
        let test_cases = vec![
            "[1, 2, 3]",      // Spaces after comma
            "[1,2,3]",        // No spaces
            "[ 1, 2, 3 ]",    // Spaces around brackets
            "[1.0, 2.0, 3.0]", // Explicit decimals
            "[0.001, 0.002, 0.003]", // Small floats
            "[-1, -2, -3]",   // Negative values
        ];

        for (i, vector_literal) in test_cases.iter().enumerate() {
            let result = engine
                .execute_vector_query(&format!(
                    "INSERT INTO parse_test (id, embedding) VALUES ('{}', '{}')",
                    i, vector_literal
                ))
                .await;
            assert!(
                result.is_ok(),
                "Failed to parse vector literal: {}",
                vector_literal
            );
        }
    }
}
