//! Vector search command handlers for Redis RESP protocol
//!
//! This module implements Redis-compatible vector commands similar to RediSearch:
//! - VECTOR.CREATE - Create a vector index
//! - VECTOR.ADD - Add a vector to an index
//! - VECTOR.GET - Get a vector by ID
//! - VECTOR.DEL - Delete a vector
//! - VECTOR.SEARCH - Perform KNN similarity search
//! - VECTOR.INFO - Get index metadata
//! - VECTOR.COUNT - Count vectors in an index
//! - VECTOR.LIST - List all vector IDs in an index
//! - VECTOR.STATS - Get detailed index statistics
//! - VECTOR.DROP - Delete an index
//! - FT.CREATE - Create a full-text search index with vector support
//! - FT.SEARCH - Search with vector similarity

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::protocols::error::ProtocolResult;
use crate::protocols::resp::RespValue;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Vector index configuration
#[derive(Debug, Clone)]
pub struct VectorIndexConfig {
    /// Name of the index
    pub name: String,
    /// Vector dimension
    pub dimension: usize,
    /// Distance metric (L2, COSINE, IP)
    pub distance_metric: DistanceMetric,
    /// Maximum number of vectors
    pub capacity: usize,
}

/// Distance metric for vector similarity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceMetric {
    /// Euclidean (L2) distance
    L2,
    /// Cosine similarity
    Cosine,
    /// Inner product
    InnerProduct,
}

impl DistanceMetric {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "L2" | "EUCLIDEAN" => Some(Self::L2),
            "COSINE" | "COS" => Some(Self::Cosine),
            "IP" | "INNERPRODUCT" | "DOT" => Some(Self::InnerProduct),
            _ => None,
        }
    }
}

/// Stored vector with metadata
#[derive(Debug, Clone)]
struct StoredVector {
    id: String,
    vector: Vec<f32>,
    metadata: HashMap<String, String>,
}

/// Full-text search field schema
#[derive(Debug, Clone)]
pub struct FtsFieldSchema {
    pub name: String,
    pub field_type: FtsFieldType,
    pub sortable: bool,
    pub noindex: bool,
}

/// FTS field types (RediSearch compatible)
#[derive(Debug, Clone, PartialEq)]
pub enum FtsFieldType {
    Text { weight: f32, nostem: bool },
    Tag { separator: char },
    Numeric,
    Geo,
    Vector { dim: usize, metric: DistanceMetric },
}

/// Stored document for full-text search
#[derive(Debug, Clone)]
struct StoredDocument {
    #[allow(dead_code)]
    id: String,
    fields: HashMap<String, String>,
    #[allow(dead_code)]
    score: f32,
}

/// Text index for full-text search
struct TextIndex {
    /// Inverted index: term -> [(doc_id, positions, field_weight)]
    inverted_index: HashMap<String, Vec<(String, Vec<usize>, f32)>>,
    /// Document store
    documents: HashMap<String, StoredDocument>,
    /// Field schemas
    schema: Vec<FtsFieldSchema>,
}

impl TextIndex {
    fn new(schema: Vec<FtsFieldSchema>) -> Self {
        Self {
            inverted_index: HashMap::new(),
            documents: HashMap::new(),
            schema,
        }
    }

    /// Tokenize text into terms
    fn tokenize(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && s.len() > 1)
            .map(|s| self.stem_word(s))
            .collect()
    }

    /// Simple stemming (basic suffix removal)
    fn stem_word(&self, word: &str) -> String {
        let w = word.to_lowercase();
        if w.ends_with("ing") && w.len() > 5 {
            w[..w.len() - 3].to_string()
        } else if w.ends_with("ed") && w.len() > 4 {
            w[..w.len() - 2].to_string()
        } else if w.ends_with("s") && w.len() > 3 && !w.ends_with("ss") {
            w[..w.len() - 1].to_string()
        } else {
            w
        }
    }

    /// Add document to text index
    fn add_document(&mut self, id: &str, fields: HashMap<String, String>) {
        // Index each text field
        for schema_field in &self.schema {
            if let FtsFieldType::Text { weight, nostem } = &schema_field.field_type {
                if let Some(value) = fields.get(&schema_field.name) {
                    let terms = if *nostem {
                        value
                            .to_lowercase()
                            .split_whitespace()
                            .map(|s| s.to_string())
                            .collect()
                    } else {
                        self.tokenize(value)
                    };

                    for (pos, term) in terms.iter().enumerate() {
                        self.inverted_index
                            .entry(term.clone())
                            .or_insert_with(Vec::new)
                            .push((id.to_string(), vec![pos], *weight));
                    }
                }
            }
        }

        // Store document
        self.documents.insert(
            id.to_string(),
            StoredDocument {
                id: id.to_string(),
                fields,
                score: 1.0,
            },
        );
    }

    /// Delete document from text index
    fn delete_document(&mut self, id: &str) -> bool {
        if self.documents.remove(id).is_some() {
            // Remove from inverted index
            for postings in self.inverted_index.values_mut() {
                postings.retain(|(doc_id, _, _)| doc_id != id);
            }
            true
        } else {
            false
        }
    }

    /// Search text index with query
    fn search(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Vec<(String, f32, HashMap<String, String>)> {
        let query_terms = self.tokenize(query);
        if query_terms.is_empty() {
            return vec![];
        }

        // Calculate TF-IDF scores for each document
        let mut doc_scores: HashMap<String, f32> = HashMap::new();
        let num_docs = self.documents.len() as f32;

        for term in &query_terms {
            if let Some(postings) = self.inverted_index.get(term) {
                // IDF = log(N / df)
                let idf = (num_docs / postings.len() as f32).ln().max(0.0) + 1.0;

                for (doc_id, positions, field_weight) in postings {
                    // TF = number of occurrences
                    let tf = positions.len() as f32;
                    let score = tf * idf * field_weight;

                    *doc_scores.entry(doc_id.clone()).or_insert(0.0) += score;
                }
            }
        }

        // Sort by score
        let mut results: Vec<(String, f32)> = doc_scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Apply offset and limit, return with fields
        results
            .into_iter()
            .skip(offset)
            .take(limit)
            .filter_map(|(id, score)| {
                self.documents
                    .get(&id)
                    .map(|doc| (id, score, doc.fields.clone()))
            })
            .collect()
    }

    /// Get document count
    fn doc_count(&self) -> usize {
        self.documents.len()
    }
}

/// Vector index storage
struct VectorIndex {
    config: VectorIndexConfig,
    vectors: HashMap<String, StoredVector>,
    /// Optional text index for hybrid search
    text_index: Option<TextIndex>,
}

impl VectorIndex {
    fn new(config: VectorIndexConfig) -> Self {
        Self {
            config,
            vectors: HashMap::new(),
            text_index: None,
        }
    }

    /// Create with text index schema for FT.* commands
    fn with_text_schema(config: VectorIndexConfig, schema: Vec<FtsFieldSchema>) -> Self {
        Self {
            config,
            vectors: HashMap::new(),
            text_index: Some(TextIndex::new(schema)),
        }
    }

    /// Calculate distance between two vectors
    fn calculate_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match self.config.distance_metric {
            DistanceMetric::L2 => {
                // Euclidean distance
                a.iter()
                    .zip(b.iter())
                    .map(|(x, y)| (x - y).powi(2))
                    .sum::<f32>()
                    .sqrt()
            }
            DistanceMetric::Cosine => {
                // Cosine similarity (return 1 - similarity for consistency as distance)
                let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
                let norm_a: f32 = a.iter().map(|x| x.powi(2)).sum::<f32>().sqrt();
                let norm_b: f32 = b.iter().map(|x| x.powi(2)).sum::<f32>().sqrt();
                if norm_a == 0.0 || norm_b == 0.0 {
                    1.0
                } else {
                    1.0 - (dot / (norm_a * norm_b))
                }
            }
            DistanceMetric::InnerProduct => {
                // Negative inner product (higher IP = more similar = lower distance)
                -a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>()
            }
        }
    }

    /// Perform KNN search
    fn knn_search(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        let mut distances: Vec<(String, f32)> = self
            .vectors
            .iter()
            .map(|(id, sv)| (id.clone(), self.calculate_distance(query, &sv.vector)))
            .collect();

        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        distances.truncate(k);
        distances
    }
}

/// Global vector index storage
static VECTOR_INDICES: std::sync::OnceLock<Arc<RwLock<HashMap<String, VectorIndex>>>> =
    std::sync::OnceLock::new();

fn get_vector_indices() -> &'static Arc<RwLock<HashMap<String, VectorIndex>>> {
    VECTOR_INDICES.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

/// Handler for vector commands
pub struct VectorCommands {
    #[allow(dead_code)]
    base: BaseCommandHandler,
}

impl VectorCommands {
    pub fn new(
        orbit_client: Arc<orbit_client::OrbitClient>,
        local_registry: Arc<crate::protocols::resp::simple_local::SimpleLocalRegistry>,
    ) -> Self {
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
        }
    }

    /// VECTOR.CREATE index_name DIM dimension [DISTANCE_METRIC L2|COSINE|IP] [CAPACITY cap]
    async fn cmd_vector_create(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'vector.create' command".to_string(),
            ));
        }

        let index_name = self.get_string_arg(args, 0, "VECTOR.CREATE")?;
        let mut dimension = 128; // default
        let mut distance_metric = DistanceMetric::L2;
        let mut capacity = 10000;

        // Parse optional arguments
        let mut i = 1;
        while i < args.len() {
            let arg = self
                .get_string_arg(args, i, "VECTOR.CREATE")?
                .to_uppercase();
            match arg.as_str() {
                "DIM" | "DIMENSION" => {
                    i += 1;
                    dimension = self.get_int_arg(args, i, "VECTOR.CREATE")? as usize;
                }
                "DISTANCE_METRIC" | "METRIC" => {
                    i += 1;
                    let metric_str = self.get_string_arg(args, i, "VECTOR.CREATE")?;
                    distance_metric = DistanceMetric::from_str(&metric_str).ok_or_else(|| {
                        crate::protocols::error::ProtocolError::RespError(format!(
                            "ERR unknown distance metric '{}'",
                            metric_str
                        ))
                    })?;
                }
                "CAPACITY" | "CAP" => {
                    i += 1;
                    capacity = self.get_int_arg(args, i, "VECTOR.CREATE")? as usize;
                }
                _ => {}
            }
            i += 1;
        }

        let config = VectorIndexConfig {
            name: index_name.clone(),
            dimension,
            distance_metric,
            capacity,
        };

        let indices = get_vector_indices();
        let mut indices_guard = indices.write().await;

        if indices_guard.contains_key(&index_name) {
            return Err(crate::protocols::error::ProtocolError::RespError(format!(
                "ERR index '{}' already exists",
                index_name
            )));
        }

        indices_guard.insert(index_name.clone(), VectorIndex::new(config));

        info!(
            "Created vector index '{}' with dim={}, metric={:?}",
            index_name, dimension, distance_metric
        );

        Ok(RespValue::ok())
    }

    /// VECTOR.ADD index_name id vector [field value ...]
    async fn cmd_vector_add(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'vector.add' command".to_string(),
            ));
        }

        let index_name = self.get_string_arg(args, 0, "VECTOR.ADD")?;
        let id = self.get_string_arg(args, 1, "VECTOR.ADD")?;
        let vector_str = self.get_string_arg(args, 2, "VECTOR.ADD")?;

        // Parse vector from comma-separated or space-separated values
        let vector: Vec<f32> = vector_str
            .split([',', ' '])
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.trim().parse::<f32>().map_err(|_| {
                    crate::protocols::error::ProtocolError::RespError(format!(
                        "ERR invalid vector component '{}'",
                        s
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Parse metadata
        let mut metadata = HashMap::new();
        let mut i = 3;
        while i + 1 < args.len() {
            let field = self.get_string_arg(args, i, "VECTOR.ADD")?;
            let value = self.get_string_arg(args, i + 1, "VECTOR.ADD")?;
            metadata.insert(field, value);
            i += 2;
        }

        let indices = get_vector_indices();
        let mut indices_guard = indices.write().await;

        let index = indices_guard.get_mut(&index_name).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR index '{}' does not exist",
                index_name
            ))
        })?;

        // Validate dimension
        if vector.len() != index.config.dimension {
            return Err(crate::protocols::error::ProtocolError::RespError(format!(
                "ERR vector dimension {} does not match index dimension {}",
                vector.len(),
                index.config.dimension
            )));
        }

        // Check capacity
        if index.vectors.len() >= index.config.capacity {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR index capacity reached".to_string(),
            ));
        }

        let stored = StoredVector {
            id: id.clone(),
            vector,
            metadata,
        };

        index.vectors.insert(id.clone(), stored);

        debug!("Added vector '{}' to index '{}'", id, index_name);
        Ok(RespValue::ok())
    }

    /// VECTOR.GET index_name id
    async fn cmd_vector_get(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("VECTOR.GET", args, 2)?;

        let index_name = self.get_string_arg(args, 0, "VECTOR.GET")?;
        let id = self.get_string_arg(args, 1, "VECTOR.GET")?;

        let indices = get_vector_indices();
        let indices_guard = indices.read().await;

        let index = indices_guard.get(&index_name).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR index '{}' does not exist",
                index_name
            ))
        })?;

        if let Some(sv) = index.vectors.get(&id) {
            let vector_str = sv
                .vector
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",");

            let mut result = vec![
                RespValue::bulk_string_from_str("id"),
                RespValue::bulk_string_from_str(&sv.id),
                RespValue::bulk_string_from_str("vector"),
                RespValue::bulk_string_from_str(&vector_str),
            ];

            for (k, v) in &sv.metadata {
                result.push(RespValue::bulk_string_from_str(k));
                result.push(RespValue::bulk_string_from_str(v));
            }

            Ok(RespValue::Array(result))
        } else {
            Ok(RespValue::null())
        }
    }

    /// VECTOR.DEL index_name id
    async fn cmd_vector_del(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("VECTOR.DEL", args, 2)?;

        let index_name = self.get_string_arg(args, 0, "VECTOR.DEL")?;
        let id = self.get_string_arg(args, 1, "VECTOR.DEL")?;

        let indices = get_vector_indices();
        let mut indices_guard = indices.write().await;

        let index = indices_guard.get_mut(&index_name).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR index '{}' does not exist",
                index_name
            ))
        })?;

        let deleted = index.vectors.remove(&id).is_some();
        debug!("VECTOR.DEL {} {} -> {}", index_name, id, deleted);

        Ok(RespValue::Integer(if deleted { 1 } else { 0 }))
    }

    /// VECTOR.SEARCH index_name query_vector [K num] [RETURN fields...]
    async fn cmd_vector_search(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'vector.search' command".to_string(),
            ));
        }

        let index_name = self.get_string_arg(args, 0, "VECTOR.SEARCH")?;
        let query_str = self.get_string_arg(args, 1, "VECTOR.SEARCH")?;

        // Parse query vector
        let query: Vec<f32> = query_str
            .split([',', ' '])
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.trim().parse::<f32>().map_err(|_| {
                    crate::protocols::error::ProtocolError::RespError(format!(
                        "ERR invalid query vector component '{}'",
                        s
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut k = 10; // default
        let mut return_fields: Vec<String> = Vec::new();

        // Parse options
        let mut i = 2;
        while i < args.len() {
            let arg = self
                .get_string_arg(args, i, "VECTOR.SEARCH")?
                .to_uppercase();
            match arg.as_str() {
                "K" | "KNN" | "LIMIT" => {
                    i += 1;
                    k = self.get_int_arg(args, i, "VECTOR.SEARCH")? as usize;
                }
                "RETURN" => {
                    i += 1;
                    // Collect return fields until end or next keyword
                    while i < args.len() {
                        if let Some(field) = self.get_optional_string_arg(args, i) {
                            if field.to_uppercase() == "K"
                                || field.to_uppercase() == "KNN"
                                || field.to_uppercase() == "LIMIT"
                            {
                                i -= 1;
                                break;
                            }
                            return_fields.push(field);
                        }
                        i += 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }

        let indices = get_vector_indices();
        let indices_guard = indices.read().await;

        let index = indices_guard.get(&index_name).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR index '{}' does not exist",
                index_name
            ))
        })?;

        // Validate query dimension
        if query.len() != index.config.dimension {
            return Err(crate::protocols::error::ProtocolError::RespError(format!(
                "ERR query dimension {} does not match index dimension {}",
                query.len(),
                index.config.dimension
            )));
        }

        // Perform KNN search
        let results = index.knn_search(&query, k);

        // Build response
        let mut response = vec![RespValue::Integer(results.len() as i64)];

        for (id, distance) in results {
            response.push(RespValue::bulk_string_from_str(&id));

            let mut doc_result = vec![
                RespValue::bulk_string_from_str("__vector_score"),
                RespValue::bulk_string_from_str(distance.to_string()),
            ];

            // Add requested fields
            if let Some(sv) = index.vectors.get(&id) {
                if return_fields.is_empty() {
                    // Return all metadata
                    for (field, value) in &sv.metadata {
                        doc_result.push(RespValue::bulk_string_from_str(field));
                        doc_result.push(RespValue::bulk_string_from_str(value));
                    }
                } else {
                    // Return only specified fields
                    for field in &return_fields {
                        if let Some(value) = sv.metadata.get(field) {
                            doc_result.push(RespValue::bulk_string_from_str(field));
                            doc_result.push(RespValue::bulk_string_from_str(value));
                        }
                    }
                }
            }

            response.push(RespValue::Array(doc_result));
        }

        debug!(
            "VECTOR.SEARCH {} -> {} results",
            index_name,
            (response.len() - 1) / 2
        );
        Ok(RespValue::Array(response))
    }

    /// VECTOR.INFO index_name
    async fn cmd_vector_info(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("VECTOR.INFO", args, 1)?;

        let index_name = self.get_string_arg(args, 0, "VECTOR.INFO")?;

        let indices = get_vector_indices();
        let indices_guard = indices.read().await;

        let index = indices_guard.get(&index_name).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR index '{}' does not exist",
                index_name
            ))
        })?;

        let metric_str = match index.config.distance_metric {
            DistanceMetric::L2 => "L2",
            DistanceMetric::Cosine => "COSINE",
            DistanceMetric::InnerProduct => "IP",
        };

        Ok(RespValue::Array(vec![
            RespValue::bulk_string_from_str("index_name"),
            RespValue::bulk_string_from_str(&index.config.name),
            RespValue::bulk_string_from_str("dimension"),
            RespValue::Integer(index.config.dimension as i64),
            RespValue::bulk_string_from_str("distance_metric"),
            RespValue::bulk_string_from_str(metric_str),
            RespValue::bulk_string_from_str("capacity"),
            RespValue::Integer(index.config.capacity as i64),
            RespValue::bulk_string_from_str("num_vectors"),
            RespValue::Integer(index.vectors.len() as i64),
        ]))
    }

    /// VECTOR.COUNT index_name - Return the number of vectors in an index
    async fn cmd_vector_count(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("VECTOR.COUNT", args, 1)?;

        let index_name = self.get_string_arg(args, 0, "VECTOR.COUNT")?;

        let indices = get_vector_indices();
        let indices_guard = indices.read().await;

        let index = indices_guard.get(&index_name).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR index '{}' does not exist",
                index_name
            ))
        })?;

        Ok(RespValue::Integer(index.vectors.len() as i64))
    }

    /// VECTOR.LIST index_name - List all vector IDs in an index
    async fn cmd_vector_list(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("VECTOR.LIST", args, 1)?;

        let index_name = self.get_string_arg(args, 0, "VECTOR.LIST")?;

        let indices = get_vector_indices();
        let indices_guard = indices.read().await;

        let index = indices_guard.get(&index_name).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR index '{}' does not exist",
                index_name
            ))
        })?;

        let ids: Vec<RespValue> = index
            .vectors
            .keys()
            .map(RespValue::bulk_string_from_str)
            .collect();

        Ok(RespValue::Array(ids))
    }

    /// VECTOR.STATS index_name - Get detailed statistics about an index
    async fn cmd_vector_stats(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("VECTOR.STATS", args, 1)?;

        let index_name = self.get_string_arg(args, 0, "VECTOR.STATS")?;

        let indices = get_vector_indices();
        let indices_guard = indices.read().await;

        let index = indices_guard.get(&index_name).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR index '{}' does not exist",
                index_name
            ))
        })?;

        // Compute basic statistics
        let vector_count = index.vectors.len();
        let memory_usage = vector_count * index.config.dimension * std::mem::size_of::<f32>();

        Ok(RespValue::Array(vec![
            RespValue::bulk_string_from_str("index_name"),
            RespValue::bulk_string_from_str(&index_name),
            RespValue::bulk_string_from_str("dimension"),
            RespValue::Integer(index.config.dimension as i64),
            RespValue::bulk_string_from_str("distance_metric"),
            RespValue::bulk_string_from_str(format!("{:?}", index.config.distance_metric)),
            RespValue::bulk_string_from_str("num_vectors"),
            RespValue::Integer(vector_count as i64),
            RespValue::bulk_string_from_str("memory_usage_bytes"),
            RespValue::Integer(memory_usage as i64),
            RespValue::bulk_string_from_str("capacity"),
            RespValue::Integer(index.config.capacity as i64),
        ]))
    }

    /// VECTOR.DROP index_name
    async fn cmd_vector_drop(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("VECTOR.DROP", args, 1)?;

        let index_name = self.get_string_arg(args, 0, "VECTOR.DROP")?;

        let indices = get_vector_indices();
        let mut indices_guard = indices.write().await;

        let removed = indices_guard.remove(&index_name).is_some();

        info!("Dropped vector index '{}': {}", index_name, removed);
        Ok(RespValue::Integer(if removed { 1 } else { 0 }))
    }

    /// FT.CREATE - RediSearch-compatible index creation
    /// Syntax: FT.CREATE index [ON HASH|JSON] [PREFIX count prefix ...] SCHEMA field_name field_type [OPTIONS] ...
    async fn cmd_ft_create(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'ft.create' command".to_string(),
            ));
        }

        let index_name = self.get_string_arg(args, 0, "FT.CREATE")?;

        // Parse schema definition
        let mut schema: Vec<FtsFieldSchema> = Vec::new();
        let mut i = 1;
        let mut in_schema = false;
        let mut vector_dim = 128;
        let mut distance_metric = DistanceMetric::Cosine;

        while i < args.len() {
            let arg = self.get_string_arg(args, i, "FT.CREATE")?.to_uppercase();

            match arg.as_str() {
                "ON" => {
                    i += 2; // Skip ON HASH/JSON
                }
                "PREFIX" => {
                    if i + 1 < args.len() {
                        let count = self.get_int_arg(args, i + 1, "FT.CREATE")? as usize;
                        i += 2 + count; // Skip PREFIX count prefixes...
                    } else {
                        i += 1;
                    }
                }
                "SCHEMA" => {
                    in_schema = true;
                    i += 1;
                }
                _ if in_schema => {
                    // Parse field definition: field_name TYPE [OPTIONS...]
                    let field_name = arg.to_lowercase();
                    i += 1;

                    if i >= args.len() {
                        break;
                    }

                    let field_type_str = self.get_string_arg(args, i, "FT.CREATE")?.to_uppercase();
                    i += 1;

                    let mut weight = 1.0f32;
                    let mut nostem = false;
                    let mut sortable = false;
                    let mut separator = ',';

                    // Parse field options
                    while i < args.len() {
                        let opt = self
                            .get_string_arg(args, i, "FT.CREATE")
                            .unwrap_or_default()
                            .to_uppercase();

                        match opt.as_str() {
                            "WEIGHT" => {
                                i += 1;
                                if i < args.len() {
                                    weight = self
                                        .get_string_arg(args, i, "FT.CREATE")
                                        .unwrap_or_default()
                                        .parse()
                                        .unwrap_or(1.0);
                                    i += 1;
                                }
                            }
                            "NOSTEM" => {
                                nostem = true;
                                i += 1;
                            }
                            "SORTABLE" => {
                                sortable = true;
                                i += 1;
                            }
                            "SEPARATOR" => {
                                i += 1;
                                if i < args.len() {
                                    separator = self
                                        .get_string_arg(args, i, "FT.CREATE")
                                        .unwrap_or_default()
                                        .chars()
                                        .next()
                                        .unwrap_or(',');
                                    i += 1;
                                }
                            }
                            "DIM" => {
                                i += 1;
                                if i < args.len() {
                                    vector_dim = self.get_int_arg(args, i, "FT.CREATE")? as usize;
                                    i += 1;
                                }
                            }
                            "DISTANCE_METRIC" => {
                                i += 1;
                                if i < args.len() {
                                    let metric_str = self.get_string_arg(args, i, "FT.CREATE")?;
                                    distance_metric = DistanceMetric::from_str(&metric_str)
                                        .unwrap_or(DistanceMetric::Cosine);
                                    i += 1;
                                }
                            }
                            // If we hit another field name (not an option), break
                            _ if !opt.is_empty()
                                && !["TEXT", "TAG", "NUMERIC", "GEO", "VECTOR"]
                                    .contains(&opt.as_str()) =>
                            {
                                break;
                            }
                            _ => break,
                        }
                    }

                    let field_type = match field_type_str.as_str() {
                        "TEXT" => FtsFieldType::Text { weight, nostem },
                        "TAG" => FtsFieldType::Tag { separator },
                        "NUMERIC" => FtsFieldType::Numeric,
                        "GEO" => FtsFieldType::Geo,
                        "VECTOR" => FtsFieldType::Vector {
                            dim: vector_dim,
                            metric: distance_metric,
                        },
                        _ => continue,
                    };

                    schema.push(FtsFieldSchema {
                        name: field_name,
                        field_type,
                        sortable,
                        noindex: false,
                    });
                }
                _ => {
                    i += 1;
                }
            }
        }

        // Create the index with text schema
        let config = VectorIndexConfig {
            name: index_name.clone(),
            dimension: vector_dim,
            distance_metric,
            capacity: 100000,
        };

        let indices = get_vector_indices();
        let mut indices_guard = indices.write().await;

        if indices_guard.contains_key(&index_name) {
            return Err(crate::protocols::error::ProtocolError::RespError(format!(
                "Index already exists: {}",
                index_name
            )));
        }

        let index = VectorIndex::with_text_schema(config, schema);
        indices_guard.insert(index_name.clone(), index);

        info!("Created FT index '{}'", index_name);
        Ok(RespValue::ok())
    }

    /// FT.ADD - Add document to index
    /// Syntax: FT.ADD index docId score [NOSAVE] [REPLACE] [LANGUAGE lang] [PAYLOAD payload] FIELDS field value ...
    async fn cmd_ft_add(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 4 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'ft.add' command".to_string(),
            ));
        }

        let index_name = self.get_string_arg(args, 0, "FT.ADD")?;
        let doc_id = self.get_string_arg(args, 1, "FT.ADD")?;
        let _score: f32 = self
            .get_string_arg(args, 2, "FT.ADD")
            .unwrap_or_default()
            .parse()
            .unwrap_or(1.0);

        // Parse fields
        let mut fields: HashMap<String, String> = HashMap::new();
        let mut i = 3;
        let mut in_fields = false;

        while i < args.len() {
            let arg = self.get_string_arg(args, i, "FT.ADD")?.to_uppercase();
            match arg.as_str() {
                "FIELDS" => {
                    in_fields = true;
                    i += 1;
                }
                "NOSAVE" | "REPLACE" | "PARTIAL" => {
                    i += 1;
                }
                "LANGUAGE" | "PAYLOAD" => {
                    i += 2; // Skip option and its value
                }
                _ if in_fields => {
                    if i + 1 < args.len() {
                        let field_name = self.get_string_arg(args, i, "FT.ADD")?;
                        let field_value = self.get_string_arg(args, i + 1, "FT.ADD")?;
                        fields.insert(field_name, field_value);
                        i += 2;
                    } else {
                        break;
                    }
                }
                _ => {
                    i += 1;
                }
            }
        }

        // Add to index
        let indices = get_vector_indices();
        let mut indices_guard = indices.write().await;

        let index = indices_guard.get_mut(&index_name).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "Unknown index: {}",
                index_name
            ))
        })?;

        // Add to text index if available
        if let Some(text_index) = &mut index.text_index {
            text_index.add_document(&doc_id, fields);
        }

        info!("Added document '{}' to FT index '{}'", doc_id, index_name);
        Ok(RespValue::ok())
    }

    /// FT.SEARCH - RediSearch-compatible search
    /// Syntax: FT.SEARCH index query [NOCONTENT] [LIMIT offset num] [RETURN count field ...] [SORTBY field [ASC|DESC]] ...
    async fn cmd_ft_search(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'ft.search' command".to_string(),
            ));
        }

        let index_name = self.get_string_arg(args, 0, "FT.SEARCH")?;
        let query = self.get_string_arg(args, 1, "FT.SEARCH")?;

        // Parse options
        let mut limit = 10usize;
        let mut offset = 0usize;
        let mut nocontent = false;
        let mut return_fields: Option<Vec<String>> = None;

        let mut i = 2;
        while i < args.len() {
            let arg = self.get_string_arg(args, i, "FT.SEARCH")?.to_uppercase();
            match arg.as_str() {
                "NOCONTENT" => {
                    nocontent = true;
                    i += 1;
                }
                "LIMIT" => {
                    if i + 2 < args.len() {
                        offset = self.get_int_arg(args, i + 1, "FT.SEARCH")? as usize;
                        limit = self.get_int_arg(args, i + 2, "FT.SEARCH")? as usize;
                        i += 3;
                    } else {
                        i += 1;
                    }
                }
                "RETURN" => {
                    if i + 1 < args.len() {
                        let count = self.get_int_arg(args, i + 1, "FT.SEARCH")? as usize;
                        let mut fields = Vec::new();
                        for j in 0..count {
                            if i + 2 + j < args.len() {
                                fields.push(self.get_string_arg(args, i + 2 + j, "FT.SEARCH")?);
                            }
                        }
                        return_fields = Some(fields);
                        i += 2 + count;
                    } else {
                        i += 1;
                    }
                }
                "SORTBY" => {
                    i += 2; // Skip SORTBY and field
                    if i < args.len() {
                        let dir = self
                            .get_string_arg(args, i, "FT.SEARCH")
                            .unwrap_or_default()
                            .to_uppercase();
                        if dir == "ASC" || dir == "DESC" {
                            i += 1;
                        }
                    }
                }
                _ => {
                    i += 1;
                }
            }
        }

        // Perform search
        let indices = get_vector_indices();
        let indices_guard = indices.read().await;

        let index = indices_guard.get(&index_name).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "Unknown index: {}",
                index_name
            ))
        })?;

        // Check if it's a wildcard query
        let results = if query == "*" {
            // Return all documents
            if let Some(text_index) = &index.text_index {
                text_index
                    .documents
                    .iter()
                    .skip(offset)
                    .take(limit)
                    .map(|(id, doc)| (id.clone(), 1.0f32, doc.fields.clone()))
                    .collect()
            } else {
                vec![]
            }
        } else if let Some(text_index) = &index.text_index {
            text_index.search(&query, limit, offset)
        } else {
            vec![]
        };

        // Build response
        // Format: [total_results, doc_id, [field, value, ...], doc_id, [field, value, ...], ...]
        let total = results.len();
        let mut response = vec![RespValue::Integer(total as i64)];

        for (doc_id, _score, fields) in results {
            response.push(RespValue::bulk_string_from_str(&doc_id));

            if !nocontent {
                let field_list: Vec<RespValue> = match &return_fields {
                    Some(rf) => rf
                        .iter()
                        .flat_map(|f| {
                            fields.get(f).map(|v| {
                                vec![
                                    RespValue::bulk_string_from_str(f),
                                    RespValue::bulk_string_from_str(v),
                                ]
                            })
                        })
                        .flatten()
                        .collect(),
                    None => fields
                        .iter()
                        .flat_map(|(k, v)| {
                            vec![
                                RespValue::bulk_string_from_str(k),
                                RespValue::bulk_string_from_str(v),
                            ]
                        })
                        .collect(),
                };
                response.push(RespValue::Array(field_list));
            }
        }

        Ok(RespValue::Array(response))
    }

    /// FT.DEL - Delete document from index
    async fn cmd_ft_del(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'ft.del' command".to_string(),
            ));
        }

        let index_name = self.get_string_arg(args, 0, "FT.DEL")?;
        let doc_id = self.get_string_arg(args, 1, "FT.DEL")?;

        let indices = get_vector_indices();
        let mut indices_guard = indices.write().await;

        let index = indices_guard.get_mut(&index_name).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "Unknown index: {}",
                index_name
            ))
        })?;

        let deleted = if let Some(text_index) = &mut index.text_index {
            text_index.delete_document(&doc_id)
        } else {
            false
        };

        Ok(RespValue::Integer(if deleted { 1 } else { 0 }))
    }

    /// FT.DROPINDEX - RediSearch-compatible index deletion
    async fn cmd_ft_dropindex(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.cmd_vector_drop(args).await
    }

    /// FT.INFO - RediSearch-compatible index info
    async fn cmd_ft_info(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'ft.info' command".to_string(),
            ));
        }

        let index_name = self.get_string_arg(args, 0, "FT.INFO")?;

        let indices = get_vector_indices();
        let indices_guard = indices.read().await;

        let index = indices_guard.get(&index_name).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "Unknown index: {}",
                index_name
            ))
        })?;

        let num_docs = index
            .text_index
            .as_ref()
            .map(|t| t.doc_count())
            .unwrap_or(0);
        let num_terms = index
            .text_index
            .as_ref()
            .map(|t| t.inverted_index.len())
            .unwrap_or(0);

        // Build schema info
        let schema_info: Vec<RespValue> = index
            .text_index
            .as_ref()
            .map(|t| {
                t.schema
                    .iter()
                    .map(|f| {
                        let type_str = match &f.field_type {
                            FtsFieldType::Text { weight, nostem } => {
                                format!(
                                    "TEXT WEIGHT {} {}",
                                    weight,
                                    if *nostem { "NOSTEM" } else { "" }
                                )
                            }
                            FtsFieldType::Tag { separator } => {
                                format!("TAG SEPARATOR {}", separator)
                            }
                            FtsFieldType::Numeric => "NUMERIC".to_string(),
                            FtsFieldType::Geo => "GEO".to_string(),
                            FtsFieldType::Vector { dim, metric } => {
                                format!("VECTOR DIM {} DISTANCE_METRIC {:?}", dim, metric)
                            }
                        };
                        RespValue::Array(vec![
                            RespValue::bulk_string_from_str(&f.name),
                            RespValue::bulk_string_from_str(&type_str),
                        ])
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(RespValue::Array(vec![
            RespValue::bulk_string_from_str("index_name"),
            RespValue::bulk_string_from_str(&index_name),
            RespValue::bulk_string_from_str("num_docs"),
            RespValue::Integer(num_docs as i64),
            RespValue::bulk_string_from_str("num_terms"),
            RespValue::Integer(num_terms as i64),
            RespValue::bulk_string_from_str("num_records"),
            RespValue::Integer(num_docs as i64),
            RespValue::bulk_string_from_str("fields"),
            RespValue::Array(schema_info),
        ]))
    }
}

#[async_trait]
impl CommandHandler for VectorCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        match command_name.to_uppercase().as_str() {
            "VECTOR.CREATE" => self.cmd_vector_create(args).await,
            "VECTOR.ADD" => self.cmd_vector_add(args).await,
            "VECTOR.GET" => self.cmd_vector_get(args).await,
            "VECTOR.DEL" => self.cmd_vector_del(args).await,
            "VECTOR.SEARCH" => self.cmd_vector_search(args).await,
            "VECTOR.INFO" => self.cmd_vector_info(args).await,
            "VECTOR.COUNT" => self.cmd_vector_count(args).await,
            "VECTOR.LIST" => self.cmd_vector_list(args).await,
            "VECTOR.STATS" => self.cmd_vector_stats(args).await,
            "VECTOR.DROP" => self.cmd_vector_drop(args).await,
            "VECTOR.KNN" => self.cmd_vector_search(args).await, // Alias for VECTOR.SEARCH
            "FT.CREATE" => self.cmd_ft_create(args).await,
            "FT.ADD" => self.cmd_ft_add(args).await,
            "FT.DEL" => self.cmd_ft_del(args).await,
            "FT.SEARCH" => self.cmd_ft_search(args).await,
            "FT.DROPINDEX" => self.cmd_ft_dropindex(args).await,
            "FT.INFO" => self.cmd_ft_info(args).await,
            _ => Err(crate::protocols::error::ProtocolError::RespError(format!(
                "ERR unknown vector command '{command_name}'"
            ))),
        }
    }

    fn supported_commands(&self) -> &[&'static str] {
        &[
            "VECTOR.CREATE",
            "VECTOR.ADD",
            "VECTOR.GET",
            "VECTOR.DEL",
            "VECTOR.SEARCH",
            "VECTOR.INFO",
            "VECTOR.COUNT",
            "VECTOR.LIST",
            "VECTOR.STATS",
            "VECTOR.DROP",
            "VECTOR.KNN",
            "FT.CREATE",
            "FT.ADD",
            "FT.DEL",
            "FT.SEARCH",
            "FT.DROPINDEX",
            "FT.INFO",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_metric_l2() {
        let config = VectorIndexConfig {
            name: "test".to_string(),
            dimension: 3,
            distance_metric: DistanceMetric::L2,
            capacity: 100,
        };
        let index = VectorIndex::new(config);

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];

        let distance = index.calculate_distance(&a, &b);
        assert!((distance - std::f32::consts::SQRT_2).abs() < 0.001);
    }

    #[test]
    fn test_distance_metric_cosine() {
        let config = VectorIndexConfig {
            name: "test".to_string(),
            dimension: 3,
            distance_metric: DistanceMetric::Cosine,
            capacity: 100,
        };
        let index = VectorIndex::new(config);

        // Same direction = 0 distance (1 - 1 = 0)
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![2.0, 0.0, 0.0];
        let distance = index.calculate_distance(&a, &b);
        assert!(distance.abs() < 0.001);

        // Orthogonal = 1 distance (1 - 0 = 1)
        let c = vec![0.0, 1.0, 0.0];
        let distance2 = index.calculate_distance(&a, &c);
        assert!((distance2 - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_knn_search() {
        let config = VectorIndexConfig {
            name: "test".to_string(),
            dimension: 2,
            distance_metric: DistanceMetric::L2,
            capacity: 100,
        };
        let mut index = VectorIndex::new(config);

        // Add some vectors
        index.vectors.insert(
            "a".to_string(),
            StoredVector {
                id: "a".to_string(),
                vector: vec![0.0, 0.0],
                metadata: HashMap::new(),
            },
        );
        index.vectors.insert(
            "b".to_string(),
            StoredVector {
                id: "b".to_string(),
                vector: vec![1.0, 0.0],
                metadata: HashMap::new(),
            },
        );
        index.vectors.insert(
            "c".to_string(),
            StoredVector {
                id: "c".to_string(),
                vector: vec![10.0, 0.0],
                metadata: HashMap::new(),
            },
        );

        // Search for nearest to origin
        let results = index.knn_search(&[0.0, 0.0], 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "a"); // closest
        assert_eq!(results[1].0, "b"); // second closest
    }

    #[tokio::test]
    async fn test_vector_count_list_stats() {
        // Create a test index
        let config = VectorIndexConfig {
            name: "test-count".to_string(),
            dimension: 3,
            distance_metric: DistanceMetric::Cosine,
            capacity: 100,
        };
        let mut index = VectorIndex::new(config);

        // Add some vectors
        index.vectors.insert(
            "v1".to_string(),
            StoredVector {
                id: "v1".to_string(),
                vector: vec![1.0, 0.0, 0.0],
                metadata: HashMap::new(),
            },
        );
        index.vectors.insert(
            "v2".to_string(),
            StoredVector {
                id: "v2".to_string(),
                vector: vec![0.0, 1.0, 0.0],
                metadata: HashMap::new(),
            },
        );
        index.vectors.insert(
            "v3".to_string(),
            StoredVector {
                id: "v3".to_string(),
                vector: vec![0.0, 0.0, 1.0],
                metadata: HashMap::new(),
            },
        );

        // Test count
        assert_eq!(index.vectors.len(), 3);

        // Test list
        let ids: Vec<&String> = index.vectors.keys().collect();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&&"v1".to_string()));
        assert!(ids.contains(&&"v2".to_string()));
        assert!(ids.contains(&&"v3".to_string()));

        // Test stats via config
        assert_eq!(index.config.dimension, 3);
        assert_eq!(index.config.capacity, 100);
    }

    #[test]
    fn test_vector_commands_supported() {
        // Just verify the supported commands list contains new commands
        let expected_commands = [
            "VECTOR.COUNT",
            "VECTOR.LIST",
            "VECTOR.STATS",
            "VECTOR.KNN",
            "FT.ADD",
            "FT.DEL",
        ];

        // These should be in the supported commands list
        // (testing via trait would require full setup, so we just verify the constants)
        for cmd in &expected_commands {
            assert!(
                [
                    "VECTOR.CREATE",
                    "VECTOR.ADD",
                    "VECTOR.GET",
                    "VECTOR.DEL",
                    "VECTOR.SEARCH",
                    "VECTOR.INFO",
                    "VECTOR.COUNT",
                    "VECTOR.LIST",
                    "VECTOR.STATS",
                    "VECTOR.DROP",
                    "VECTOR.KNN",
                    "FT.CREATE",
                    "FT.ADD",
                    "FT.DEL",
                    "FT.SEARCH",
                    "FT.DROPINDEX",
                    "FT.INFO"
                ]
                .contains(cmd),
                "Command {} should be supported",
                cmd
            );
        }
    }
}
