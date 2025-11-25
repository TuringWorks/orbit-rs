//! Integration tests for GPU-accelerated vector similarity search
//!
//! Tests the integration of GPU-accelerated vector similarity with VectorActor
//! and verifies correctness and performance routing.

use orbit_server::protocols::vector_store::{
    SimilarityMetric, Vector, VectorActor, VectorSearchParams,
};

#[tokio::test]
async fn test_vector_similarity_cpu_sequential() {
    let mut actor = VectorActor::new();

    // Add test vectors
    let vec1 = Vector::new("v1".to_string(), vec![1.0, 0.0, 0.0]);
    let vec2 = Vector::new("v2".to_string(), vec![0.0, 1.0, 0.0]);
    let vec3 = Vector::new("v3".to_string(), vec![0.707, 0.707, 0.0]);

    actor.add_vector(vec1.clone()).unwrap();
    actor.add_vector(vec2.clone()).unwrap();
    actor.add_vector(vec3.clone()).unwrap();

    // Search for similar vectors
    let params = VectorSearchParams::new(vec![1.0, 0.0, 0.0], SimilarityMetric::Cosine, 10);
    let results = actor.search_vectors(params);

    assert!(!results.is_empty());
    assert_eq!(results[0].vector.id, "v1"); // Should be exact match
    assert!((results[0].score - 1.0).abs() < 0.01);
}

#[tokio::test]
async fn test_vector_similarity_cpu_parallel() {
    // Create a larger dataset to trigger parallel processing
    let mut actor = VectorActor::new();

    // Add 200 vectors to trigger parallel processing
    for i in 0..200 {
        let data: Vec<f32> = (0..128)
            .map(|j| ((i + j) as f32) * 0.01)
            .collect();
        let vector = Vector::new(format!("v{}", i), data);
        actor.add_vector(vector).unwrap();
    }

    // Create query vector
    let query: Vec<f32> = (0..128).map(|i| (i as f32) * 0.01).collect();

    let params = VectorSearchParams::new(query, SimilarityMetric::Cosine, 10);
    let results = actor.search_vectors(params);

    assert_eq!(results.len(), 10);
    // Results should be sorted by score (descending)
    for i in 1..results.len() {
        assert!(results[i - 1].score >= results[i].score);
    }
}

#[tokio::test]
async fn test_vector_similarity_metrics() {
    let mut actor = VectorActor::new();

    let vec1 = Vector::new("v1".to_string(), vec![1.0, 0.0]);
    let vec2 = Vector::new("v2".to_string(), vec![0.0, 1.0]);
    let vec3 = Vector::new("v3".to_string(), vec![1.0, 1.0]);

    actor.add_vector(vec1).unwrap();
    actor.add_vector(vec2).unwrap();
    actor.add_vector(vec3).unwrap();

    let query = vec![1.0, 0.0];

    // Test cosine similarity
    let params = VectorSearchParams::new(query.clone(), SimilarityMetric::Cosine, 10);
    let results = actor.search_vectors(params);
    assert_eq!(results[0].vector.id, "v1"); // Should match v1 exactly

    // Test euclidean distance
    let params = VectorSearchParams::new(query.clone(), SimilarityMetric::Euclidean, 10);
    let results = actor.search_vectors(params);
    assert_eq!(results[0].vector.id, "v1"); // Should match v1 (distance = 0)

    // Test dot product
    let params = VectorSearchParams::new(query.clone(), SimilarityMetric::DotProduct, 10);
    let results = actor.search_vectors(params);
    assert_eq!(results[0].vector.id, "v1"); // Should match v1 (dot product = 1.0)

    // Test manhattan distance
    let params = VectorSearchParams::new(query, SimilarityMetric::Manhattan, 10);
    let results = actor.search_vectors(params);
    assert_eq!(results[0].vector.id, "v1"); // Should match v1 (distance = 0)
}

#[tokio::test]
async fn test_vector_similarity_metadata_filters() {
    let mut actor = VectorActor::new();

    let mut vec1 = Vector::new("v1".to_string(), vec![1.0, 0.0, 0.0]);
    vec1.metadata.insert("category".to_string(), "A".to_string());
    let mut vec2 = Vector::new("v2".to_string(), vec![1.0, 0.0, 0.0]);
    vec2.metadata.insert("category".to_string(), "B".to_string());

    actor.add_vector(vec1).unwrap();
    actor.add_vector(vec2).unwrap();

    let mut params = VectorSearchParams::new(vec![1.0, 0.0, 0.0], SimilarityMetric::Cosine, 10);
    params.metadata_filters.insert("category".to_string(), "A".to_string());

    let results = actor.search_vectors(params);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].vector.id, "v1");
}

#[tokio::test]
async fn test_vector_similarity_threshold() {
    let mut actor = VectorActor::new();

    let vec1 = Vector::new("v1".to_string(), vec![1.0, 0.0, 0.0]);
    let vec2 = Vector::new("v2".to_string(), vec![0.0, 1.0, 0.0]);

    actor.add_vector(vec1).unwrap();
    actor.add_vector(vec2).unwrap();

    let mut params = VectorSearchParams::new(vec![1.0, 0.0, 0.0], SimilarityMetric::Cosine, 10);
    params.threshold = Some(0.5);

    let results = actor.search_vectors(params);
    // Only v1 should match (cosine similarity = 1.0 > 0.5)
    // v2 should be filtered out (cosine similarity = 0.0 < 0.5)
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].vector.id, "v1");
}

#[tokio::test]
async fn test_vector_similarity_limit() {
    let mut actor = VectorActor::new();

    // Add 100 vectors
    for i in 0..100 {
        let data: Vec<f32> = (0..64)
            .map(|j| ((i + j) as f32) * 0.01)
            .collect();
        let vector = Vector::new(format!("v{}", i), data);
        actor.add_vector(vector).unwrap();
    }

    let query: Vec<f32> = (0..64).map(|i| (i as f32) * 0.01).collect();
    let params = VectorSearchParams::new(query, SimilarityMetric::Cosine, 5);

    let results = actor.search_vectors(params);
    assert_eq!(results.len(), 5); // Should respect limit
}

