//! Vector search command handlers for Redis RESP protocol
//!
//! This module implements Redis-compatible vector commands similar to RediSearch:
//! - VECTOR.CREATE - Create a vector index
//! - VECTOR.ADD - Add a vector to an index
//! - VECTOR.GET - Get a vector by ID
//! - VECTOR.DEL - Delete a vector
//! - VECTOR.SEARCH - Perform KNN similarity search
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

/// Vector index storage
struct VectorIndex {
    config: VectorIndexConfig,
    vectors: HashMap<String, StoredVector>,
}

impl VectorIndex {
    fn new(config: VectorIndexConfig) -> Self {
        Self {
            config,
            vectors: HashMap::new(),
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
            .split(|c| c == ',' || c == ' ')
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
            .split(|c| c == ',' || c == ' ')
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
                RespValue::bulk_string_from_str(&distance.to_string()),
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
    async fn cmd_ft_create(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'ft.create' command".to_string(),
            ));
        }

        // For now, delegate to VECTOR.CREATE with simplified parsing
        // Full FT.CREATE would support more schema options
        self.cmd_vector_create(args).await
    }

    /// FT.SEARCH - RediSearch-compatible search
    async fn cmd_ft_search(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'ft.search' command".to_string(),
            ));
        }

        // For now, delegate to VECTOR.SEARCH
        // Full FT.SEARCH would support text queries + vector queries
        self.cmd_vector_search(args).await
    }

    /// FT.DROPINDEX - RediSearch-compatible index deletion
    async fn cmd_ft_dropindex(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.cmd_vector_drop(args).await
    }

    /// FT.INFO - RediSearch-compatible index info
    async fn cmd_ft_info(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.cmd_vector_info(args).await
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
            "VECTOR.DROP" => self.cmd_vector_drop(args).await,
            "FT.CREATE" => self.cmd_ft_create(args).await,
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
            "VECTOR.DROP",
            "FT.CREATE",
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
}
