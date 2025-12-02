//! Vector Index Implementations
//!
//! Provides HNSW and IVFFlat index implementations for efficient
//! approximate nearest neighbor (ANN) search in high-dimensional spaces.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use super::vector_store::{SimilarityMetric, Vector, VectorSimilarity};

/// HNSW (Hierarchical Navigable Small World) Index
///
/// A graph-based index for approximate nearest neighbor search.
/// Provides logarithmic search time with high recall.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswIndex {
    /// Index name
    pub name: String,
    /// Vector dimension
    pub dimension: usize,
    /// Distance metric
    pub metric: SimilarityMetric,
    /// Maximum number of connections per node at each layer
    pub m: usize,
    /// Maximum number of connections for the first layer (typically 2*m)
    pub m_max0: usize,
    /// Size of the dynamic candidate list during construction
    pub ef_construction: usize,
    /// Size of the dynamic candidate list during search
    pub ef_search: usize,
    /// Maximum level in the graph
    pub max_level: usize,
    /// Entry point node ID
    pub entry_point: Option<String>,
    /// Stored vectors
    vectors: HashMap<String, Vector>,
    /// Graph structure: node_id -> (level -> neighbors)
    graph: HashMap<String, Vec<HashSet<String>>>,
    /// Node levels
    levels: HashMap<String, usize>,
    /// Level multiplier for random level generation
    ml: f64,
}

impl Default for HnswIndex {
    fn default() -> Self {
        Self::new("default".to_string(), 384, SimilarityMetric::Cosine)
    }
}

impl HnswIndex {
    /// Create a new HNSW index
    pub fn new(name: String, dimension: usize, metric: SimilarityMetric) -> Self {
        let m = 16;
        Self {
            name,
            dimension,
            metric,
            m,
            m_max0: m * 2,
            ef_construction: 64,
            ef_search: 50,
            max_level: 0,
            entry_point: None,
            vectors: HashMap::new(),
            graph: HashMap::new(),
            levels: HashMap::new(),
            ml: 1.0 / (m as f64).ln(),
        }
    }

    /// Create with custom parameters
    pub fn with_params(
        name: String,
        dimension: usize,
        metric: SimilarityMetric,
        m: usize,
        ef_construction: usize,
    ) -> Self {
        Self {
            name,
            dimension,
            metric,
            m,
            m_max0: m * 2,
            ef_construction,
            ef_search: 50,
            max_level: 0,
            entry_point: None,
            vectors: HashMap::new(),
            graph: HashMap::new(),
            levels: HashMap::new(),
            ml: 1.0 / (m as f64).ln(),
        }
    }

    /// Set ef_search parameter for query-time accuracy/speed tradeoff
    pub fn set_ef_search(&mut self, ef_search: usize) {
        self.ef_search = ef_search;
    }

    /// Generate random level for a new node
    fn random_level(&self) -> usize {
        let r: f64 = rand::random();
        (-r.ln() * self.ml).floor() as usize
    }

    /// Calculate distance between two vectors
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match self.metric {
            SimilarityMetric::Euclidean => VectorSimilarity::euclidean_distance(a, b),
            SimilarityMetric::Cosine => {
                // For cosine, we use 1 - cosine_similarity as distance
                1.0 - VectorSimilarity::cosine_similarity(a, b)
            }
            SimilarityMetric::DotProduct => {
                // Negative dot product (higher dot product = smaller distance)
                -VectorSimilarity::dot_product(a, b)
            }
            SimilarityMetric::Manhattan => VectorSimilarity::manhattan_distance(a, b),
        }
    }

    /// Insert a vector into the index
    pub fn insert(&mut self, vector: Vector) -> Result<(), String> {
        if vector.data.len() != self.dimension {
            return Err(format!(
                "Vector dimension {} doesn't match index dimension {}",
                vector.data.len(),
                self.dimension
            ));
        }

        let id = vector.id.clone();
        let level = self.random_level();

        // Store the vector
        self.vectors.insert(id.clone(), vector.clone());
        self.levels.insert(id.clone(), level);

        // Initialize graph structure for this node
        let mut node_neighbors: Vec<HashSet<String>> = Vec::new();
        for _ in 0..=level {
            node_neighbors.push(HashSet::new());
        }
        self.graph.insert(id.clone(), node_neighbors);

        // If this is the first node, set as entry point
        if self.entry_point.is_none() {
            self.entry_point = Some(id.clone());
            self.max_level = level;
            return Ok(());
        }

        let entry_point = self.entry_point.clone().unwrap();

        // Search from top level to level+1
        let mut ep = entry_point.clone();
        for lc in (level + 1..=self.max_level).rev() {
            let changed = self.search_layer_single(&vector.data, &ep, lc);
            if let Some(new_ep) = changed {
                ep = new_ep;
            }
        }

        // Insert at levels from min(level, max_level) down to 0
        let insert_level = level.min(self.max_level);
        for lc in (0..=insert_level).rev() {
            // Find ef_construction nearest neighbors at this level
            let neighbors = self.search_layer(&vector.data, &ep, self.ef_construction, lc);

            // Select M best neighbors
            let m = if lc == 0 { self.m_max0 } else { self.m };
            let selected: Vec<String> = neighbors.into_iter().take(m).map(|(id, _)| id).collect();

            // Add bidirectional connections
            if let Some(node_neighbors) = self.graph.get_mut(&id) {
                if lc < node_neighbors.len() {
                    for neighbor_id in &selected {
                        node_neighbors[lc].insert(neighbor_id.clone());
                    }
                }
            }

            for neighbor_id in &selected {
                if let Some(neighbor_neighbors) = self.graph.get_mut(neighbor_id) {
                    if lc < neighbor_neighbors.len() {
                        neighbor_neighbors[lc].insert(id.clone());

                        // Prune if too many connections
                        let max_conn = if lc == 0 { self.m_max0 } else { self.m };
                        if neighbor_neighbors[lc].len() > max_conn {
                            self.prune_connections(neighbor_id, lc, max_conn);
                        }
                    }
                }
            }

            // Update ep for next level
            if !selected.is_empty() {
                ep = selected[0].clone();
            }
        }

        // Update entry point if new node has higher level
        if level > self.max_level {
            self.max_level = level;
            self.entry_point = Some(id);
        }

        Ok(())
    }

    /// Search for single nearest neighbor at a layer
    fn search_layer_single(&self, query: &[f32], entry: &str, level: usize) -> Option<String> {
        let mut current = entry.to_string();
        let entry_vec = self.vectors.get(entry)?;
        let mut current_dist = self.distance(query, &entry_vec.data);

        loop {
            let mut changed = false;

            if let Some(node_neighbors) = self.graph.get(&current) {
                if level < node_neighbors.len() {
                    for neighbor_id in &node_neighbors[level] {
                        if let Some(neighbor_vec) = self.vectors.get(neighbor_id) {
                            let dist = self.distance(query, &neighbor_vec.data);
                            if dist < current_dist {
                                current = neighbor_id.clone();
                                current_dist = dist;
                                changed = true;
                            }
                        }
                    }
                }
            }

            if !changed {
                break;
            }
        }

        Some(current)
    }

    /// Search layer for k nearest neighbors
    fn search_layer(
        &self,
        query: &[f32],
        entry: &str,
        ef: usize,
        level: usize,
    ) -> Vec<(String, f32)> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut candidates: BinaryHeap<DistanceNode> = BinaryHeap::new();
        let mut results: BinaryHeap<DistanceNode> = BinaryHeap::new();

        let entry_vec = match self.vectors.get(entry) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let entry_dist = self.distance(query, &entry_vec.data);

        visited.insert(entry.to_string());
        candidates.push(DistanceNode {
            id: entry.to_string(),
            distance: -entry_dist, // Min-heap behavior
        });
        results.push(DistanceNode {
            id: entry.to_string(),
            distance: entry_dist, // Max-heap for furthest
        });

        while let Some(DistanceNode { id, distance }) = candidates.pop() {
            let current_dist = -distance;

            // Get furthest result distance
            let furthest_dist = results.peek().map(|n| n.distance).unwrap_or(f32::INFINITY);

            if current_dist > furthest_dist && results.len() >= ef {
                break;
            }

            if let Some(node_neighbors) = self.graph.get(&id) {
                if level < node_neighbors.len() {
                    for neighbor_id in &node_neighbors[level] {
                        if visited.contains(neighbor_id) {
                            continue;
                        }
                        visited.insert(neighbor_id.clone());

                        if let Some(neighbor_vec) = self.vectors.get(neighbor_id) {
                            let dist = self.distance(query, &neighbor_vec.data);
                            let furthest =
                                results.peek().map(|n| n.distance).unwrap_or(f32::INFINITY);

                            if dist < furthest || results.len() < ef {
                                candidates.push(DistanceNode {
                                    id: neighbor_id.clone(),
                                    distance: -dist,
                                });
                                results.push(DistanceNode {
                                    id: neighbor_id.clone(),
                                    distance: dist,
                                });

                                if results.len() > ef {
                                    results.pop();
                                }
                            }
                        }
                    }
                }
            }
        }

        // Convert to sorted results (closest first)
        let mut result_vec: Vec<(String, f32)> =
            results.into_iter().map(|n| (n.id, n.distance)).collect();
        result_vec.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        result_vec
    }

    /// Prune connections to keep only the best M
    fn prune_connections(&mut self, node_id: &str, level: usize, max_conn: usize) {
        let node_vec = match self.vectors.get(node_id) {
            Some(v) => v.clone(),
            None => return,
        };

        let neighbors = match self.graph.get(node_id) {
            Some(n) if level < n.len() => n[level].clone(),
            _ => return,
        };

        // Calculate distances and sort
        let mut distances: Vec<(String, f32)> = neighbors
            .iter()
            .filter_map(|neighbor_id| {
                self.vectors
                    .get(neighbor_id)
                    .map(|v| (neighbor_id.clone(), self.distance(&node_vec.data, &v.data)))
            })
            .collect();

        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

        // Keep only best max_conn neighbors
        let keep: HashSet<String> = distances
            .into_iter()
            .take(max_conn)
            .map(|(id, _)| id)
            .collect();

        if let Some(node_neighbors) = self.graph.get_mut(node_id) {
            if level < node_neighbors.len() {
                node_neighbors[level] = keep;
            }
        }
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        if query.len() != self.dimension {
            return Vec::new();
        }

        let entry_point = match &self.entry_point {
            Some(ep) => ep.clone(),
            None => return Vec::new(),
        };

        // Search from top level to level 1
        let mut ep = entry_point;
        for lc in (1..=self.max_level).rev() {
            if let Some(new_ep) = self.search_layer_single(query, &ep, lc) {
                ep = new_ep;
            }
        }

        // Search level 0 with ef_search
        let results = self.search_layer(query, &ep, self.ef_search.max(k), 0);

        // Return top k
        results.into_iter().take(k).collect()
    }

    /// Get a vector by ID
    pub fn get(&self, id: &str) -> Option<&Vector> {
        self.vectors.get(id)
    }

    /// Remove a vector from the index
    pub fn remove(&mut self, id: &str) -> Option<Vector> {
        // Remove from graph
        if let Some(node_neighbors) = self.graph.remove(id) {
            // Remove all references to this node from neighbors
            for (level, neighbors) in node_neighbors.iter().enumerate() {
                for neighbor_id in neighbors {
                    if let Some(neighbor_graph) = self.graph.get_mut(neighbor_id) {
                        if level < neighbor_graph.len() {
                            neighbor_graph[level].remove(id);
                        }
                    }
                }
            }
        }

        self.levels.remove(id);

        // Update entry point if necessary
        if self.entry_point.as_deref() == Some(id) {
            self.entry_point = self.vectors.keys().find(|k| *k != id).cloned();
            if let Some(ref new_ep) = self.entry_point {
                self.max_level = *self.levels.get(new_ep).unwrap_or(&0);
            } else {
                self.max_level = 0;
            }
        }

        self.vectors.remove(id)
    }

    /// Get the number of vectors in the index
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }
}

/// Helper struct for priority queue operations
#[derive(Clone)]
struct DistanceNode {
    id: String,
    distance: f32,
}

impl PartialEq for DistanceNode {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for DistanceNode {}

impl PartialOrd for DistanceNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DistanceNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // For max-heap behavior (furthest first)
        self.distance
            .partial_cmp(&other.distance)
            .unwrap_or(Ordering::Equal)
    }
}

/// IVFFlat (Inverted File with Flat quantization) Index
///
/// A partition-based index that divides the vector space into clusters
/// and only searches relevant clusters during query time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IvfFlatIndex {
    /// Index name
    pub name: String,
    /// Vector dimension
    pub dimension: usize,
    /// Distance metric
    pub metric: SimilarityMetric,
    /// Number of clusters (lists)
    pub n_lists: usize,
    /// Number of clusters to probe during search
    pub n_probe: usize,
    /// Cluster centroids
    centroids: Vec<Vec<f32>>,
    /// Inverted lists: cluster_id -> list of vector IDs
    inverted_lists: Vec<Vec<String>>,
    /// Stored vectors
    vectors: HashMap<String, Vector>,
    /// Vector to cluster assignment
    assignments: HashMap<String, usize>,
    /// Whether the index has been trained
    trained: bool,
}

impl IvfFlatIndex {
    /// Create a new IVFFlat index
    pub fn new(name: String, dimension: usize, metric: SimilarityMetric, n_lists: usize) -> Self {
        Self {
            name,
            dimension,
            metric,
            n_lists,
            n_probe: 1,
            centroids: Vec::new(),
            inverted_lists: vec![Vec::new(); n_lists],
            vectors: HashMap::new(),
            assignments: HashMap::new(),
            trained: false,
        }
    }

    /// Set the number of clusters to probe during search
    pub fn set_n_probe(&mut self, n_probe: usize) {
        self.n_probe = n_probe.min(self.n_lists);
    }

    /// Calculate distance between two vectors
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match self.metric {
            SimilarityMetric::Euclidean => VectorSimilarity::euclidean_distance(a, b),
            SimilarityMetric::Cosine => 1.0 - VectorSimilarity::cosine_similarity(a, b),
            SimilarityMetric::DotProduct => -VectorSimilarity::dot_product(a, b),
            SimilarityMetric::Manhattan => VectorSimilarity::manhattan_distance(a, b),
        }
    }

    /// Train the index using k-means clustering
    pub fn train(&mut self, training_vectors: &[Vector]) -> Result<(), String> {
        if training_vectors.len() < self.n_lists {
            return Err(format!(
                "Need at least {} vectors to train {} clusters",
                self.n_lists, self.n_lists
            ));
        }

        // Initialize centroids using k-means++
        self.centroids = self.kmeans_plusplus_init(training_vectors);

        // Run k-means iterations
        let max_iterations = 20;
        for _ in 0..max_iterations {
            // Assign vectors to clusters
            let mut cluster_sums: Vec<Vec<f64>> = vec![vec![0.0; self.dimension]; self.n_lists];
            let mut cluster_counts: Vec<usize> = vec![0; self.n_lists];

            for vector in training_vectors {
                let cluster = self.find_nearest_centroid(&vector.data);
                cluster_counts[cluster] += 1;
                for (i, v) in vector.data.iter().enumerate() {
                    cluster_sums[cluster][i] += *v as f64;
                }
            }

            // Update centroids
            let mut converged = true;
            for (i, (sum, count)) in cluster_sums.iter().zip(cluster_counts.iter()).enumerate() {
                if *count > 0 {
                    let new_centroid: Vec<f32> =
                        sum.iter().map(|s| (*s / *count as f64) as f32).collect();

                    let dist =
                        VectorSimilarity::euclidean_distance(&self.centroids[i], &new_centroid);
                    if dist > 1e-6 {
                        converged = false;
                    }
                    self.centroids[i] = new_centroid;
                }
            }

            if converged {
                break;
            }
        }

        self.trained = true;
        Ok(())
    }

    /// Initialize centroids using k-means++
    fn kmeans_plusplus_init(&self, vectors: &[Vector]) -> Vec<Vec<f32>> {
        let mut centroids: Vec<Vec<f32>> = Vec::with_capacity(self.n_lists);

        // Choose first centroid randomly
        let first_idx = (rand::random::<f64>() * vectors.len() as f64) as usize;
        centroids.push(vectors[first_idx].data.clone());

        // Choose remaining centroids
        for _ in 1..self.n_lists {
            let mut distances: Vec<f32> = Vec::with_capacity(vectors.len());
            let mut total_dist = 0.0f32;

            for vector in vectors {
                let min_dist = centroids
                    .iter()
                    .map(|c| self.distance(&vector.data, c))
                    .fold(f32::INFINITY, f32::min);
                let dist_sq = min_dist * min_dist;
                distances.push(dist_sq);
                total_dist += dist_sq;
            }

            // Choose next centroid with probability proportional to distance squared
            let threshold = rand::random::<f32>() * total_dist;
            let mut cumsum = 0.0f32;
            let mut chosen_idx = 0;

            for (i, dist) in distances.iter().enumerate() {
                cumsum += dist;
                if cumsum >= threshold {
                    chosen_idx = i;
                    break;
                }
            }

            centroids.push(vectors[chosen_idx].data.clone());
        }

        centroids
    }

    /// Find the nearest centroid for a vector
    fn find_nearest_centroid(&self, vector: &[f32]) -> usize {
        self.centroids
            .iter()
            .enumerate()
            .map(|(i, c)| (i, self.distance(vector, c)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// Find k nearest centroids for a vector
    fn find_k_nearest_centroids(&self, vector: &[f32], k: usize) -> Vec<usize> {
        let mut distances: Vec<(usize, f32)> = self
            .centroids
            .iter()
            .enumerate()
            .map(|(i, c)| (i, self.distance(vector, c)))
            .collect();

        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        distances.into_iter().take(k).map(|(i, _)| i).collect()
    }

    /// Insert a vector into the index
    pub fn insert(&mut self, vector: Vector) -> Result<(), String> {
        if vector.data.len() != self.dimension {
            return Err(format!(
                "Vector dimension {} doesn't match index dimension {}",
                vector.data.len(),
                self.dimension
            ));
        }

        // Auto-train if not trained and we have enough vectors
        if !self.trained {
            // For now, just initialize with this vector as a centroid if needed
            if self.centroids.is_empty() {
                self.centroids = vec![vec![0.0; self.dimension]; self.n_lists];
                for (i, c) in self.centroids.iter_mut().enumerate() {
                    // Initialize centroids with slight variations
                    for (j, v) in c.iter_mut().enumerate() {
                        *v = vector.data.get(j).copied().unwrap_or(0.0) + (i as f32 * 0.01);
                    }
                }
                self.trained = true;
            }
        }

        let id = vector.id.clone();
        let cluster = self.find_nearest_centroid(&vector.data);

        // Remove from old cluster if reassigning
        if let Some(old_cluster) = self.assignments.get(&id) {
            self.inverted_lists[*old_cluster].retain(|x| x != &id);
        }

        // Add to new cluster
        self.inverted_lists[cluster].push(id.clone());
        self.assignments.insert(id.clone(), cluster);
        self.vectors.insert(id, vector);

        Ok(())
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        if query.len() != self.dimension || !self.trained {
            return Vec::new();
        }

        // Find n_probe nearest clusters
        let clusters = self.find_k_nearest_centroids(query, self.n_probe);

        // Search within selected clusters
        let mut results: Vec<(String, f32)> = Vec::new();

        for cluster_id in clusters {
            for vector_id in &self.inverted_lists[cluster_id] {
                if let Some(vector) = self.vectors.get(vector_id) {
                    let dist = self.distance(query, &vector.data);
                    results.push((vector_id.clone(), dist));
                }
            }
        }

        // Sort by distance and return top k
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        results.truncate(k);
        results
    }

    /// Get a vector by ID
    pub fn get(&self, id: &str) -> Option<&Vector> {
        self.vectors.get(id)
    }

    /// Remove a vector from the index
    pub fn remove(&mut self, id: &str) -> Option<Vector> {
        if let Some(cluster) = self.assignments.remove(id) {
            self.inverted_lists[cluster].retain(|x| x != id);
        }
        self.vectors.remove(id)
    }

    /// Get the number of vectors in the index
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }
}

/// Unified vector index that can use different algorithms
#[derive(Debug, Clone)]
pub enum VectorIndex {
    Hnsw(HnswIndex),
    IvfFlat(IvfFlatIndex),
    BruteForce(BruteForceIndex),
}

impl VectorIndex {
    /// Create an HNSW index
    pub fn hnsw(
        name: String,
        dimension: usize,
        metric: SimilarityMetric,
        m: usize,
        ef_construction: usize,
    ) -> Self {
        VectorIndex::Hnsw(HnswIndex::with_params(
            name,
            dimension,
            metric,
            m,
            ef_construction,
        ))
    }

    /// Create an IVFFlat index
    pub fn ivfflat(
        name: String,
        dimension: usize,
        metric: SimilarityMetric,
        n_lists: usize,
    ) -> Self {
        VectorIndex::IvfFlat(IvfFlatIndex::new(name, dimension, metric, n_lists))
    }

    /// Create a brute force index (exact search)
    pub fn brute_force(name: String, dimension: usize, metric: SimilarityMetric) -> Self {
        VectorIndex::BruteForce(BruteForceIndex::new(name, dimension, metric))
    }

    /// Insert a vector
    pub fn insert(&mut self, vector: Vector) -> Result<(), String> {
        match self {
            VectorIndex::Hnsw(idx) => idx.insert(vector),
            VectorIndex::IvfFlat(idx) => idx.insert(vector),
            VectorIndex::BruteForce(idx) => idx.insert(vector),
        }
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        match self {
            VectorIndex::Hnsw(idx) => idx.search(query, k),
            VectorIndex::IvfFlat(idx) => idx.search(query, k),
            VectorIndex::BruteForce(idx) => idx.search(query, k),
        }
    }

    /// Get a vector by ID
    pub fn get(&self, id: &str) -> Option<&Vector> {
        match self {
            VectorIndex::Hnsw(idx) => idx.get(id),
            VectorIndex::IvfFlat(idx) => idx.get(id),
            VectorIndex::BruteForce(idx) => idx.get(id),
        }
    }

    /// Remove a vector
    pub fn remove(&mut self, id: &str) -> Option<Vector> {
        match self {
            VectorIndex::Hnsw(idx) => idx.remove(id),
            VectorIndex::IvfFlat(idx) => idx.remove(id),
            VectorIndex::BruteForce(idx) => idx.remove(id),
        }
    }

    /// Get the number of vectors
    pub fn len(&self) -> usize {
        match self {
            VectorIndex::Hnsw(idx) => idx.len(),
            VectorIndex::IvfFlat(idx) => idx.len(),
            VectorIndex::BruteForce(idx) => idx.len(),
        }
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        match self {
            VectorIndex::Hnsw(idx) => idx.is_empty(),
            VectorIndex::IvfFlat(idx) => idx.is_empty(),
            VectorIndex::BruteForce(idx) => idx.is_empty(),
        }
    }
}

/// Brute force index for exact nearest neighbor search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BruteForceIndex {
    /// Index name
    pub name: String,
    /// Vector dimension
    pub dimension: usize,
    /// Distance metric
    pub metric: SimilarityMetric,
    /// Stored vectors
    vectors: HashMap<String, Vector>,
}

impl BruteForceIndex {
    /// Create a new brute force index
    pub fn new(name: String, dimension: usize, metric: SimilarityMetric) -> Self {
        Self {
            name,
            dimension,
            metric,
            vectors: HashMap::new(),
        }
    }

    /// Calculate distance between two vectors
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match self.metric {
            SimilarityMetric::Euclidean => VectorSimilarity::euclidean_distance(a, b),
            SimilarityMetric::Cosine => 1.0 - VectorSimilarity::cosine_similarity(a, b),
            SimilarityMetric::DotProduct => -VectorSimilarity::dot_product(a, b),
            SimilarityMetric::Manhattan => VectorSimilarity::manhattan_distance(a, b),
        }
    }

    /// Insert a vector
    pub fn insert(&mut self, vector: Vector) -> Result<(), String> {
        if vector.data.len() != self.dimension {
            return Err(format!(
                "Vector dimension {} doesn't match index dimension {}",
                vector.data.len(),
                self.dimension
            ));
        }
        self.vectors.insert(vector.id.clone(), vector);
        Ok(())
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        if query.len() != self.dimension {
            return Vec::new();
        }

        let mut results: Vec<(String, f32)> = self
            .vectors
            .iter()
            .map(|(id, v)| (id.clone(), self.distance(query, &v.data)))
            .collect();

        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        results.truncate(k);
        results
    }

    /// Get a vector by ID
    pub fn get(&self, id: &str) -> Option<&Vector> {
        self.vectors.get(id)
    }

    /// Remove a vector
    pub fn remove(&mut self, id: &str) -> Option<Vector> {
        self.vectors.remove(id)
    }

    /// Get the number of vectors
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_vectors() -> Vec<Vector> {
        vec![
            Vector::new("v1".to_string(), vec![1.0, 0.0, 0.0]),
            Vector::new("v2".to_string(), vec![0.0, 1.0, 0.0]),
            Vector::new("v3".to_string(), vec![0.0, 0.0, 1.0]),
            Vector::new("v4".to_string(), vec![0.5, 0.5, 0.0]),
            Vector::new("v5".to_string(), vec![0.5, 0.0, 0.5]),
        ]
    }

    #[test]
    fn test_hnsw_insert_and_search() {
        let mut index = HnswIndex::new("test".to_string(), 3, SimilarityMetric::Euclidean);

        for v in create_test_vectors() {
            index.insert(v).unwrap();
        }

        assert_eq!(index.len(), 5);

        // Search for vector closest to [1, 0, 0]
        let results = index.search(&[1.0, 0.0, 0.0], 2);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "v1"); // Should find exact match
    }

    #[test]
    fn test_ivfflat_insert_and_search() {
        let mut index = IvfFlatIndex::new("test".to_string(), 3, SimilarityMetric::Euclidean, 2);

        let vectors = create_test_vectors();
        index.train(&vectors).unwrap();

        for v in vectors {
            index.insert(v).unwrap();
        }

        assert_eq!(index.len(), 5);

        // Search with n_probe = 2 (all clusters)
        index.set_n_probe(2);
        let results = index.search(&[1.0, 0.0, 0.0], 2);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "v1"); // Should find exact match
    }

    #[test]
    fn test_brute_force_search() {
        let mut index = BruteForceIndex::new("test".to_string(), 3, SimilarityMetric::Euclidean);

        for v in create_test_vectors() {
            index.insert(v).unwrap();
        }

        let results = index.search(&[1.0, 0.0, 0.0], 2);
        assert_eq!(results[0].0, "v1"); // Exact match
    }

    #[test]
    fn test_cosine_similarity_search() {
        let mut index = HnswIndex::new("test".to_string(), 3, SimilarityMetric::Cosine);

        index
            .insert(Vector::new("a".to_string(), vec![1.0, 0.0, 0.0]))
            .unwrap();
        index
            .insert(Vector::new("b".to_string(), vec![0.0, 1.0, 0.0]))
            .unwrap();
        index
            .insert(Vector::new("c".to_string(), vec![0.707, 0.707, 0.0]))
            .unwrap();

        // Query direction similar to [1, 0, 0]
        let results = index.search(&[0.9, 0.1, 0.0], 2);
        assert!(!results.is_empty());
        // Should find "a" or "c" as closest
    }

    #[test]
    fn test_remove_vector() {
        let mut index = HnswIndex::new("test".to_string(), 3, SimilarityMetric::Euclidean);

        for v in create_test_vectors() {
            index.insert(v).unwrap();
        }

        assert_eq!(index.len(), 5);

        let removed = index.remove("v1");
        assert!(removed.is_some());
        assert_eq!(index.len(), 4);
        assert!(index.get("v1").is_none());
    }

    #[test]
    fn test_unified_index() {
        let mut hnsw =
            VectorIndex::hnsw("hnsw".to_string(), 3, SimilarityMetric::Euclidean, 16, 64);
        let mut ivf = VectorIndex::ivfflat("ivf".to_string(), 3, SimilarityMetric::Euclidean, 2);
        let mut bf = VectorIndex::brute_force("bf".to_string(), 3, SimilarityMetric::Euclidean);

        let vectors = create_test_vectors();
        for v in vectors {
            hnsw.insert(v.clone()).unwrap();
            ivf.insert(v.clone()).unwrap();
            bf.insert(v).unwrap();
        }

        // All should return v1 as closest to [1, 0, 0]
        let query = vec![1.0, 0.0, 0.0];

        let hnsw_results = hnsw.search(&query, 1);
        let bf_results = bf.search(&query, 1);

        assert_eq!(hnsw_results[0].0, "v1");
        assert_eq!(bf_results[0].0, "v1");
    }
}
