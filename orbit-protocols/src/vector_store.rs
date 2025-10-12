//! Vector Store Module
//!
//! Provides vector storage and similarity search capabilities similar to Redis vector database
//! and PostgreSQL pgvector extensions. Supports high-dimensional vector operations for
//! machine learning embeddings and semantic search applications.
//!
//! Now with SIMD-optimized vector distance calculations for improved performance.

use async_trait::async_trait;
use orbit_shared::orbitql::simd_ops::SimdVectorOps;
use orbit_shared::{Addressable, OrbitResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// High-dimensional vector with metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vector {
    /// Unique identifier for the vector
    pub id: String,
    /// Vector data as f32 array
    pub data: Vec<f32>,
    /// Optional metadata associated with the vector
    pub metadata: HashMap<String, String>,
    /// Timestamp when vector was created/updated
    pub timestamp: u64,
}

impl Vector {
    /// Create a new vector with given id and data
    pub fn new(id: String, data: Vec<f32>) -> Self {
        Self {
            id,
            data,
            metadata: HashMap::new(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Create a vector with metadata
    pub fn with_metadata(id: String, data: Vec<f32>, metadata: HashMap<String, String>) -> Self {
        Self {
            id,
            data,
            metadata,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Get vector dimension
    pub fn dimension(&self) -> usize {
        self.data.len()
    }

    /// Normalize vector to unit length for cosine similarity
    pub fn normalize(&mut self) {
        let magnitude = self.magnitude();
        if magnitude > 0.0 {
            for value in &mut self.data {
                *value /= magnitude;
            }
        }
    }

    /// Get vector magnitude (L2 norm)
    pub fn magnitude(&self) -> f32 {
        self.data.iter().map(|x| x * x).sum::<f32>().sqrt()
    }
}

/// Vector similarity search result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorSearchResult {
    /// The matching vector
    pub vector: Vector,
    /// Similarity score (higher = more similar)
    pub score: f32,
    /// Distance metric used
    pub metric: SimilarityMetric,
}

/// Supported vector similarity metrics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimilarityMetric {
    /// Cosine similarity (normalized dot product)
    Cosine,
    /// Euclidean distance (L2 norm)
    Euclidean,
    /// Dot product similarity
    DotProduct,
    /// Manhattan distance (L1 norm)
    Manhattan,
}

/// Vector search parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchParams {
    /// Query vector to search for
    pub query_vector: Vec<f32>,
    /// Similarity metric to use
    pub metric: SimilarityMetric,
    /// Maximum number of results to return
    pub limit: usize,
    /// Minimum similarity threshold
    pub threshold: Option<f32>,
    /// Metadata filters to apply
    pub metadata_filters: HashMap<String, String>,
}

impl VectorSearchParams {
    /// Create new search parameters
    pub fn new(query_vector: Vec<f32>, metric: SimilarityMetric, limit: usize) -> Self {
        Self {
            query_vector,
            metric,
            limit,
            threshold: None,
            metadata_filters: HashMap::new(),
        }
    }

    /// Add metadata filter
    pub fn with_metadata_filter(mut self, key: String, value: String) -> Self {
        self.metadata_filters.insert(key, value);
        self
    }

    /// Set similarity threshold
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = Some(threshold);
        self
    }
}

/// Vector index configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorIndexConfig {
    /// Index name
    pub name: String,
    /// Vector dimension
    pub dimension: usize,
    /// Distance metric for the index
    pub metric: SimilarityMetric,
    /// Index algorithm (for future extensibility)
    pub algorithm: String,
    /// Index-specific parameters
    pub parameters: HashMap<String, String>,
}

impl VectorIndexConfig {
    /// Create a new index configuration
    pub fn new(name: String, dimension: usize, metric: SimilarityMetric) -> Self {
        Self {
            name,
            dimension,
            metric,
            algorithm: "brute_force".to_string(),
            parameters: HashMap::new(),
        }
    }
}

/// Vector similarity calculation utilities
pub struct VectorSimilarity;

impl VectorSimilarity {
    /// Calculate cosine similarity between two vectors using SIMD
    /// Returns value between -1 and 1, where 1 means identical direction
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        // Use SIMD-optimized implementation from simd_ops module
        SimdVectorOps::cosine_similarity_f32(a, b)
    }

    /// Calculate euclidean distance between two vectors using SIMD
    /// Returns distance (lower = more similar)
    pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
        // Use SIMD-optimized implementation from simd_ops module
        SimdVectorOps::euclidean_distance_f32(a, b)
    }

    /// Calculate dot product between two vectors using SIMD
    pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
        // Use SIMD-optimized implementation from simd_ops module
        SimdVectorOps::dot_product_f32(a, b)
    }

    /// Calculate Manhattan (L1) distance between two vectors using SIMD
    pub fn manhattan_distance(a: &[f32], b: &[f32]) -> f32 {
        // Use SIMD-optimized implementation from simd_ops module
        SimdVectorOps::manhattan_distance_f32(a, b)
    }

    /// Calculate similarity based on metric type
    pub fn calculate_similarity(a: &[f32], b: &[f32], metric: SimilarityMetric) -> f32 {
        match metric {
            SimilarityMetric::Cosine => Self::cosine_similarity(a, b),
            SimilarityMetric::Euclidean => {
                // Convert distance to similarity (higher = more similar)
                let distance = Self::euclidean_distance(a, b);
                if distance == 0.0 {
                    1.0
                } else {
                    1.0 / (1.0 + distance)
                }
            }
            SimilarityMetric::DotProduct => Self::dot_product(a, b),
            SimilarityMetric::Manhattan => {
                // Convert distance to similarity
                let distance = Self::manhattan_distance(a, b);
                if distance == 0.0 {
                    1.0
                } else {
                    1.0 / (1.0 + distance)
                }
            }
        }
    }
}

/// Vector store actor for managing vectors and performing similarity searches
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorActor {
    /// Collection of stored vectors
    pub vectors: HashMap<String, Vector>,
    /// Vector indices for fast search
    pub indices: HashMap<String, VectorIndexConfig>,
    /// Default dimension for new vectors
    pub default_dimension: Option<usize>,
    /// Default similarity metric
    pub default_metric: SimilarityMetric,
}

impl Default for VectorActor {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorActor {
    /// Create a new empty vector actor
    pub fn new() -> Self {
        Self {
            vectors: HashMap::new(),
            indices: HashMap::new(),
            default_dimension: None,
            default_metric: SimilarityMetric::Cosine,
        }
    }

    /// Set default dimension for vectors
    pub fn with_default_dimension(mut self, dimension: usize) -> Self {
        self.default_dimension = Some(dimension);
        self
    }

    /// Set default similarity metric
    pub fn with_default_metric(mut self, metric: SimilarityMetric) -> Self {
        self.default_metric = metric;
        self
    }

    /// Add or update a vector
    pub fn add_vector(&mut self, vector: Vector) -> Result<(), String> {
        // Validate dimension consistency
        if let Some(expected_dim) = self.default_dimension {
            if vector.dimension() != expected_dim {
                return Err(format!(
                    "Vector dimension {} does not match expected dimension {}",
                    vector.dimension(),
                    expected_dim
                ));
            }
        }

        // If this is the first vector, set default dimension
        if self.default_dimension.is_none() && !self.vectors.is_empty() {
            self.default_dimension = Some(vector.dimension());
        }

        self.vectors.insert(vector.id.clone(), vector);
        Ok(())
    }

    /// Get a vector by ID
    pub fn get_vector(&self, id: &str) -> Option<&Vector> {
        self.vectors.get(id)
    }

    /// Remove a vector by ID
    pub fn remove_vector(&mut self, id: &str) -> Option<Vector> {
        self.vectors.remove(id)
    }

    /// Get all vector IDs
    pub fn list_vector_ids(&self) -> Vec<String> {
        self.vectors.keys().cloned().collect()
    }

    /// Get vector count
    pub fn vector_count(&self) -> usize {
        self.vectors.len()
    }

    /// Create a vector index
    pub fn create_index(&mut self, config: VectorIndexConfig) -> Result<(), String> {
        if let Some(expected_dim) = self.default_dimension {
            if config.dimension != expected_dim {
                return Err(format!(
                    "Index dimension {} does not match vector dimension {}",
                    config.dimension, expected_dim
                ));
            }
        }

        self.indices.insert(config.name.clone(), config);
        Ok(())
    }

    /// Drop a vector index
    pub fn drop_index(&mut self, name: &str) -> bool {
        self.indices.remove(name).is_some()
    }

    /// List all indices
    pub fn list_indices(&self) -> Vec<&VectorIndexConfig> {
        self.indices.values().collect()
    }

    /// Perform vector similarity search
    pub fn search_vectors(&self, params: VectorSearchParams) -> Vec<VectorSearchResult> {
        let mut results = Vec::new();

        for vector in self.vectors.values() {
            // Apply metadata filters
            let mut matches_filters = true;
            for (key, expected_value) in &params.metadata_filters {
                if let Some(actual_value) = vector.metadata.get(key) {
                    if actual_value != expected_value {
                        matches_filters = false;
                        break;
                    }
                } else {
                    matches_filters = false;
                    break;
                }
            }

            if !matches_filters {
                continue;
            }

            // Calculate similarity
            let score = VectorSimilarity::calculate_similarity(
                &params.query_vector,
                &vector.data,
                params.metric,
            );

            // Apply threshold filter
            if let Some(threshold) = params.threshold {
                if score < threshold {
                    continue;
                }
            }

            results.push(VectorSearchResult {
                vector: vector.clone(),
                score,
                metric: params.metric,
            });
        }

        // Sort by score (descending - higher scores first)
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Apply limit
        results.truncate(params.limit);

        results
    }

    /// Find k nearest neighbors
    pub fn knn_search(
        &self,
        query_vector: Vec<f32>,
        k: usize,
        metric: Option<SimilarityMetric>,
    ) -> Vec<VectorSearchResult> {
        let metric = metric.unwrap_or(self.default_metric);

        let params = VectorSearchParams {
            query_vector,
            metric,
            limit: k,
            threshold: None,
            metadata_filters: HashMap::new(),
        };

        self.search_vectors(params)
    }

    /// Calculate statistics about stored vectors
    pub fn get_stats(&self) -> VectorStats {
        if self.vectors.is_empty() {
            return VectorStats::default();
        }

        let mut total_dimension = 0;
        let mut min_dimension = usize::MAX;
        let mut max_dimension = 0;
        let mut total_metadata_keys = 0;

        for vector in self.vectors.values() {
            let dim = vector.dimension();
            total_dimension += dim;
            min_dimension = min_dimension.min(dim);
            max_dimension = max_dimension.max(dim);
            total_metadata_keys += vector.metadata.len();
        }

        let count = self.vectors.len();
        VectorStats {
            vector_count: count,
            index_count: self.indices.len(),
            avg_dimension: total_dimension as f64 / count as f64,
            min_dimension,
            max_dimension,
            avg_metadata_keys: total_metadata_keys as f64 / count as f64,
        }
    }
}

/// Vector store statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStats {
    /// Total number of vectors
    pub vector_count: usize,
    /// Number of indices
    pub index_count: usize,
    /// Average vector dimension
    pub avg_dimension: f64,
    /// Minimum vector dimension
    pub min_dimension: usize,
    /// Maximum vector dimension  
    pub max_dimension: usize,
    /// Average metadata keys per vector
    pub avg_metadata_keys: f64,
}

impl Default for VectorStats {
    fn default() -> Self {
        Self {
            vector_count: 0,
            index_count: 0,
            avg_dimension: 0.0,
            min_dimension: 0,
            max_dimension: 0,
            avg_metadata_keys: 0.0,
        }
    }
}

impl Addressable for VectorActor {
    fn addressable_type() -> &'static str {
        "VectorActor"
    }
}

/// Async trait defining vector actor operations
#[async_trait]
pub trait VectorActorMethods {
    /// Add a vector to the store
    async fn add_vector(&mut self, vector: Vector) -> OrbitResult<()>;

    /// Get a vector by ID
    async fn get_vector(&self, id: String) -> OrbitResult<Option<Vector>>;

    /// Remove a vector by ID
    async fn remove_vector(&mut self, id: String) -> OrbitResult<bool>;

    /// Search for similar vectors
    async fn search_vectors(
        &self,
        params: VectorSearchParams,
    ) -> OrbitResult<Vec<VectorSearchResult>>;

    /// Perform k-nearest neighbors search
    async fn knn_search(
        &self,
        query_vector: Vec<f32>,
        k: usize,
        metric: Option<SimilarityMetric>,
    ) -> OrbitResult<Vec<VectorSearchResult>>;

    /// Create a vector index
    async fn create_index(&mut self, config: VectorIndexConfig) -> OrbitResult<()>;

    /// Drop a vector index
    async fn drop_index(&mut self, name: String) -> OrbitResult<bool>;

    /// List all vector indices
    async fn list_indices(&self) -> OrbitResult<Vec<VectorIndexConfig>>;

    /// Get vector store statistics
    async fn get_stats(&self) -> OrbitResult<VectorStats>;

    /// List all vector IDs
    async fn list_vector_ids(&self) -> OrbitResult<Vec<String>>;

    /// Get vector count
    async fn vector_count(&self) -> OrbitResult<usize>;
}

/// Implementation of vector actor methods
#[async_trait]
impl VectorActorMethods for VectorActor {
    async fn add_vector(&mut self, vector: Vector) -> OrbitResult<()> {
        self.add_vector(vector)
            .map_err(orbit_shared::OrbitError::internal)
    }

    async fn get_vector(&self, id: String) -> OrbitResult<Option<Vector>> {
        Ok(self.get_vector(&id).cloned())
    }

    async fn remove_vector(&mut self, id: String) -> OrbitResult<bool> {
        Ok(self.remove_vector(&id).is_some())
    }

    async fn search_vectors(
        &self,
        params: VectorSearchParams,
    ) -> OrbitResult<Vec<VectorSearchResult>> {
        Ok(self.search_vectors(params))
    }

    async fn knn_search(
        &self,
        query_vector: Vec<f32>,
        k: usize,
        metric: Option<SimilarityMetric>,
    ) -> OrbitResult<Vec<VectorSearchResult>> {
        Ok(self.knn_search(query_vector, k, metric))
    }

    async fn create_index(&mut self, config: VectorIndexConfig) -> OrbitResult<()> {
        self.create_index(config)
            .map_err(orbit_shared::OrbitError::internal)
    }

    async fn drop_index(&mut self, name: String) -> OrbitResult<bool> {
        Ok(self.drop_index(&name))
    }

    async fn list_indices(&self) -> OrbitResult<Vec<VectorIndexConfig>> {
        Ok(self.list_indices().into_iter().cloned().collect())
    }

    async fn get_stats(&self) -> OrbitResult<VectorStats> {
        Ok(self.get_stats())
    }

    async fn list_vector_ids(&self) -> OrbitResult<Vec<String>> {
        Ok(self.list_vector_ids())
    }

    async fn vector_count(&self) -> OrbitResult<usize> {
        Ok(self.vector_count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_creation() {
        let vector = Vector::new("test1".to_string(), vec![1.0, 2.0, 3.0]);
        assert_eq!(vector.id, "test1");
        assert_eq!(vector.data, vec![1.0, 2.0, 3.0]);
        assert_eq!(vector.dimension(), 3);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let similarity = VectorSimilarity::cosine_similarity(&a, &b);
        assert!((similarity - 1.0).abs() < f32::EPSILON);

        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let similarity = VectorSimilarity::cosine_similarity(&a, &b);
        assert!((similarity - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_euclidean_distance() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        let distance = VectorSimilarity::euclidean_distance(&a, &b);
        assert!((distance - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_vector_actor_operations() {
        let mut actor = VectorActor::new();

        // Add vectors
        let vec1 = Vector::new("v1".to_string(), vec![1.0, 2.0, 3.0]);
        let vec2 = Vector::new("v2".to_string(), vec![2.0, 3.0, 4.0]);

        assert!(actor.add_vector(vec1.clone()).is_ok());
        assert!(actor.add_vector(vec2.clone()).is_ok());
        assert_eq!(actor.vector_count(), 2);

        // Get vector
        let retrieved = actor.get_vector("v1");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "v1");

        // Search vectors
        let params = VectorSearchParams::new(vec![1.0, 2.0, 3.0], SimilarityMetric::Cosine, 10);
        let results = actor.search_vectors(params);
        assert!(!results.is_empty());
        assert_eq!(results[0].vector.id, "v1"); // Should be exact match
    }
}
