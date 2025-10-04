//! pgvector-compatible SQL query engine for vector operations
//!
//! This module extends the PostgreSQL query engine with pgvector compatibility,
//! supporting vector data types, similarity operators, and vector functions.

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;

use crate::error::{ProtocolError, ProtocolResult};
use crate::vector_store::{Vector, VectorActor, VectorIndexConfig, SimilarityMetric, VectorSearchParams};
use super::query_engine::QueryResult;
use orbit_client::OrbitClient;
use orbit_shared::Key;

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
    orbit_client: OrbitClient,
    tables: Arc<RwLock<HashMap<String, VectorTable>>>,
    extensions: Arc<RwLock<HashMap<String, bool>>>, // installed extensions
}

impl VectorQueryEngine {
    /// Create a new vector query engine
    pub fn new(orbit_client: OrbitClient) -> Self {
        Self {
            orbit_client,
            tables: Arc::new(RwLock::new(HashMap::new())),
            extensions: Arc::new(RwLock::new(HashMap::new())),
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
                "Unsupported vector query: {}",
                sql
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
            Err(ProtocolError::PostgresError("Unsupported extension".to_string()))
        }
    }

    /// Handle CREATE TABLE with vector columns
    async fn handle_create_table(&self, sql: &str) -> ProtocolResult<QueryResult> {
        // Parse CREATE TABLE statement
        // Example: CREATE TABLE documents (id SERIAL, content TEXT, embedding VECTOR(384));
        
        let sql_clean = sql.trim().to_uppercase();
        let sql_clean = sql_clean.replace('\n', " ").replace('\t', " ");
        
        // Extract table name
        let parts: Vec<&str> = sql_clean.split_whitespace().collect();
        let table_name_idx = parts.iter().position(|&p| p == "TABLE")
            .ok_or_else(|| ProtocolError::PostgresError("Invalid CREATE TABLE syntax".to_string()))?;
            
        if table_name_idx + 1 >= parts.len() {
            return Err(ProtocolError::PostgresError("Missing table name".to_string()));
        }
        
        let table_name = parts[table_name_idx + 1].trim_end_matches('(').to_string();
        
        // Find column definitions between parentheses
        let open_paren = sql.find('(')
            .ok_or_else(|| ProtocolError::PostgresError("Missing column definitions".to_string()))?;
        let close_paren = sql.rfind(')')
            .ok_or_else(|| ProtocolError::PostgresError("Missing closing parenthesis".to_string()))?;
            
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
                let dim_str = type_def.trim_start_matches("VECTOR(")
                    .trim_end_matches(')');
                let dimension = dim_str.parse::<usize>()
                    .map_err(|_| ProtocolError::PostgresError("Invalid vector dimension".to_string()))?;
                VectorColumnType::Vector(dimension)
            } else if type_def.to_uppercase().starts_with("HALFVEC(") {
                let dim_str = type_def.trim_start_matches("HALFVEC(")
                    .trim_end_matches(')');
                let dimension = dim_str.parse::<usize>()
                    .map_err(|_| ProtocolError::PostgresError("Invalid halfvector dimension".to_string()))?;
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
                VectorColumnType::Vector(d) | VectorColumnType::HalfVector(d) | VectorColumnType::SparseVector(d) => Some(*d),
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
        
        let sql_upper = sql.to_uppercase();
        
        // Extract table name and column
        let on_idx = sql_upper.find(" ON ")
            .ok_or_else(|| ProtocolError::PostgresError("Missing ON clause".to_string()))?;
        let using_idx = sql_upper.find(" USING ")
            .ok_or_else(|| ProtocolError::PostgresError("Missing USING clause".to_string()))?;
            
        let table_name = sql[on_idx + 4..using_idx].trim().to_string();
        
        // Extract index method and operator
        let using_part = &sql[using_idx + 7..];
        let paren_idx = using_part.find('(')
            .ok_or_else(|| ProtocolError::PostgresError("Missing column specification".to_string()))?;
            
        let _index_method = using_part[..paren_idx].trim();
        
        // Extract column and operator class
        let close_paren = using_part.rfind(')')
            .ok_or_else(|| ProtocolError::PostgresError("Missing closing parenthesis".to_string()))?;
        let column_spec = &using_part[paren_idx + 1..close_paren];
        
        let column_parts: Vec<&str> = column_spec.split_whitespace().collect();
        if column_parts.len() < 2 {
            return Err(ProtocolError::PostgresError("Invalid column specification".to_string()));
        }
        
        let column_name = column_parts[0];
        let operator_class = column_parts[1];
        
        // Map pgvector operator classes to similarity metrics
        let similarity_metric = match operator_class.to_lowercase().as_str() {
            "vector_cosine_ops" => SimilarityMetric::Cosine,
            "vector_l2_ops" => SimilarityMetric::Euclidean,
            "vector_ip_ops" => SimilarityMetric::DotProduct,
            _ => SimilarityMetric::Cosine, // Default
        };
        
        // Get vector actor for the table
        let vector_actor_ref = self.orbit_client.actor_reference::<VectorActor>(
            Key::StringKey {
                key: format!("table_{}", table_name),
            }
        ).await.map_err(|e| ProtocolError::PostgresError(format!("Failed to get actor: {}", e)))?;
        
        // Create vector index
        let index_name = format!("{}_{}_idx", table_name, column_name);
        let index_config = VectorIndexConfig::new(
            index_name,
            384, // Default dimension - should be extracted from table schema
            similarity_metric,
        );
        
        let config_value = serde_json::to_value(index_config)
            .map_err(|e| ProtocolError::PostgresError(format!("Serialization error: {}", e)))?;
            
        vector_actor_ref.invoke::<()>("create_index", vec![config_value])
            .await
            .map_err(|e| ProtocolError::PostgresError(format!("Index creation failed: {}", e)))?;
        
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
        let into_idx = sql_upper.find("INTO ")
            .ok_or_else(|| ProtocolError::PostgresError("Missing INTO clause".to_string()))?;
        let values_idx = sql_upper.find(" VALUES ")
            .ok_or_else(|| ProtocolError::PostgresError("Missing VALUES clause".to_string()))?;
            
        // Extract table name
        let table_part = &sql[into_idx + 5..values_idx];
        let paren_idx = table_part.find('(');
        let table_name = if let Some(idx) = paren_idx {
            table_part[..idx].trim().to_string()
        } else {
            table_part.trim().to_string()
        };
        
        // Get vector actor for the table
        let vector_actor_ref = self.orbit_client.actor_reference::<VectorActor>(
            Key::StringKey {
                key: format!("table_{}", table_name),
            }
        ).await.map_err(|e| ProtocolError::PostgresError(format!("Failed to get actor: {}", e)))?;
        
        // Parse column names and values
        let columns = if let Some(idx) = paren_idx {
            let close_paren = table_part.find(')')
                .ok_or_else(|| ProtocolError::PostgresError("Missing closing parenthesis".to_string()))?;
            let cols = &table_part[idx + 1..close_paren];
            cols.split(',').map(|s| s.trim().to_string()).collect()
        } else {
            vec![] // Will need to infer from table schema
        };
        
        // Parse VALUES
        let values_part = &sql[values_idx + 8..];
        let values_start = values_part.find('(')
            .ok_or_else(|| ProtocolError::PostgresError("Missing VALUES parenthesis".to_string()))?;
        let values_end = values_part.rfind(')')
            .ok_or_else(|| ProtocolError::PostgresError("Missing closing VALUES parenthesis".to_string()))?;
            
        let values_str = &values_part[values_start + 1..values_end];
        let values = self.parse_insert_values(values_str)?;
        
        // Create and insert vector record
        if !columns.is_empty() && !values.is_empty() {
            let mut vector_data: Vec<f32> = Vec::new();
            let mut metadata = HashMap::new();
            let mut vector_id = uuid::Uuid::new_v4().to_string();
            
            for (col, val) in columns.iter().zip(values.iter()) {
                let col_lower = col.to_lowercase();
                
                if col_lower.contains("id") {
                    vector_id = val.clone();
                } else if col_lower.contains("embedding") || col_lower.contains("vector") {
                    // Parse vector data
                    vector_data = self.parse_vector_literal(val)?;
                } else {
                    // Store as metadata
                    metadata.insert(col.clone(), val.clone());
                }
            }
            
            if !vector_data.is_empty() {
                let vector = Vector::with_metadata(vector_id, vector_data, metadata);
                let vector_value = serde_json::to_value(vector)
                    .map_err(|e| ProtocolError::PostgresError(format!("Serialization error: {}", e)))?;
                    
                vector_actor_ref.invoke::<()>("add_vector", vec![vector_value])
                    .await
                    .map_err(|e| ProtocolError::PostgresError(format!("Vector insertion failed: {}", e)))?;
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
        let from_idx = sql_upper.find(" FROM ")
            .ok_or_else(|| ProtocolError::PostgresError("Missing FROM clause".to_string()))?;
        let from_part = &sql[from_idx + 6..];
        let table_name = from_part.split_whitespace().next()
            .ok_or_else(|| ProtocolError::PostgresError("Missing table name".to_string()))?;
        
        // Extract similarity operator and query vector
        let (similarity_metric, query_vector) = if sql.contains("<->") {
            let parts: Vec<&str> = sql.split("<->").collect();
            if parts.len() < 2 {
                return Err(ProtocolError::PostgresError("Invalid similarity syntax".to_string()));
            }
            let vector_part = parts[1].trim();
            let vector_str = self.extract_vector_literal(vector_part)?;
            (SimilarityMetric::Euclidean, self.parse_vector_literal(&vector_str)?)
        } else if sql.contains("<=>") {
            let parts: Vec<&str> = sql.split("<=>").collect();
            if parts.len() < 2 {
                return Err(ProtocolError::PostgresError("Invalid similarity syntax".to_string()));
            }
            let vector_part = parts[1].trim();
            let vector_str = self.extract_vector_literal(vector_part)?;
            (SimilarityMetric::Cosine, self.parse_vector_literal(&vector_str)?)
        } else if sql.contains("<#>") {
            let parts: Vec<&str> = sql.split("<#>").collect();
            if parts.len() < 2 {
                return Err(ProtocolError::PostgresError("Invalid similarity syntax".to_string()));
            }
            let vector_part = parts[1].trim();
            let vector_str = self.extract_vector_literal(vector_part)?;
            (SimilarityMetric::DotProduct, self.parse_vector_literal(&vector_str)?)
        } else {
            return Err(ProtocolError::PostgresError("No similarity operator found".to_string()));
        };
        
        // Extract LIMIT
        let limit = if let Some(limit_idx) = sql_upper.find("LIMIT ") {
            let limit_part = &sql[limit_idx + 6..];
            let limit_str = limit_part.split_whitespace().next().unwrap_or("10");
            limit_str.parse::<usize>().unwrap_or(10)
        } else {
            10
        };
        
        // Get vector actor for the table
        let vector_actor_ref = self.orbit_client.actor_reference::<VectorActor>(
            Key::StringKey {
                key: format!("table_{}", table_name),
            }
        ).await.map_err(|e| ProtocolError::PostgresError(format!("Failed to get actor: {}", e)))?;
        
        // Perform similarity search
        let search_params = VectorSearchParams::new(query_vector, similarity_metric, limit);
        let search_params_value = serde_json::to_value(search_params)
            .map_err(|e| ProtocolError::PostgresError(format!("Serialization error: {}", e)))?;
            
        let results: serde_json::Value = vector_actor_ref.invoke("search_vectors", vec![search_params_value])
            .await
            .map_err(|e| ProtocolError::PostgresError(format!("Search failed: {}", e)))?;
        
        // Convert results to QueryResult
        let mut rows = Vec::new();
        if let Some(results_array) = results.as_array() {
            for result in results_array {
                if let Some(result_obj) = result.as_object() {
                    let vector_id = result_obj.get("vector")
                        .and_then(|v| v.get("id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let score = result_obj.get("score")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    
                    rows.push(vec![
                        Some(vector_id.to_string()),
                        Some(score.to_string()),
                    ]);
                }
            }
        }
        
        Ok(QueryResult::Select {
            columns: vec!["id".to_string(), "distance".to_string()],
            rows,
        })
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
            Err(ProtocolError::PostgresError("Unsupported vector function".to_string()))
        }
    }

    /// Check if SQL contains vector operations
    fn contains_vector_operations(&self, sql: &str) -> bool {
        sql.contains("<->") || sql.contains("<#>") || sql.contains("<=>") ||
        sql.contains("VECTOR_DIMS") || sql.contains("VECTOR_NORM")
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
                    values.push(current_value.trim().trim_matches('"').trim_matches('\'').to_string());
                    current_value.clear();
                }
                _ => {
                    current_value.push(ch);
                }
            }
        }
        
        if !current_value.trim().is_empty() {
            values.push(current_value.trim().trim_matches('"').trim_matches('\'').to_string());
        }
        
        Ok(values)
    }

    /// Parse vector literal like '[0.1, 0.2, 0.3]' or '{0.1, 0.2, 0.3}'
    fn parse_vector_literal(&self, vector_str: &str) -> ProtocolResult<Vec<f32>> {
        let vector_str = vector_str.trim();
        let vector_str = if vector_str.starts_with('[') && vector_str.ends_with(']') {
            &vector_str[1..vector_str.len() - 1]
        } else if vector_str.starts_with('{') && vector_str.ends_with('}') {
            &vector_str[1..vector_str.len() - 1]
        } else {
            vector_str
        };
        
        let components: Result<Vec<f32>, _> = vector_str
            .split(',')
            .map(|s| s.trim().parse::<f32>())
            .collect();
            
        components.map_err(|e| ProtocolError::PostgresError(format!("Invalid vector format: {}", e)))
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
        
        Err(ProtocolError::PostgresError("Could not extract vector literal".to_string()))
    }
}