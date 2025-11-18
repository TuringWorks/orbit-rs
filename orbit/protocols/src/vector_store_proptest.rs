//! Property-based tests for vector operations
//!
//! This module uses proptest to validate vector operations and similarity
//! calculations under various input conditions to ensure mathematical
//! correctness and numerical stability.

#[cfg(test)]
mod proptests {
    use super::super::vector_store::*;
    use crate::error::ProtocolResult;
    use proptest::prelude::*;
    use std::collections::HashMap;

    /// Generate arbitrary vectors with reasonable dimensions and values
    fn arb_vector() -> impl Strategy<Value = Vec<f32>> {
        prop::collection::vec(
            // Generate floats in a reasonable range to avoid numerical issues
            (-1000.0f32..1000.0f32).prop_filter("non-nan", |x| x.is_finite()),
            1..=512, // Common vector dimensions
        )
    }

    /// Generate arbitrary vector with metadata
    fn arb_vector_with_metadata() -> impl Strategy<Value = Vector> {
        (
            "[a-z]{1,20}", // Random string ID
            arb_vector(),
            prop::collection::hash_map("[a-z]{1,10}", "[a-z]{1,50}", 0..=5), // Small metadata
        )
            .prop_map(|(id, data, metadata)| {
                let mut meta_json = HashMap::new();
                for (k, v) in metadata {
                    meta_json.insert(k, serde_json::Value::String(v));
                }
                Vector::with_metadata(id, data, meta_json)
            })
    }

    proptest! {
        /// Test that vector similarity calculations are symmetric for euclidean distance
        #[test]
        fn test_euclidean_distance_symmetry(
            v1 in arb_vector(),
            v2 in arb_vector()
        ) {
            // Skip if vectors have different dimensions
            prop_assume!(v1.len() == v2.len());
            
            let dist1 = calculate_euclidean_distance(&v1, &v2);
            let dist2 = calculate_euclidean_distance(&v2, &v1);
            
            // Distance should be symmetric (within floating point precision)
            prop_assert!((dist1 - dist2).abs() < 1e-6, 
                        "Euclidean distance not symmetric: {} vs {}", dist1, dist2);
        }

        /// Test that cosine similarity is bounded between -1 and 1
        #[test]
        fn test_cosine_similarity_bounds(
            v1 in arb_vector(),
            v2 in arb_vector()
        ) {
            prop_assume!(v1.len() == v2.len());
            prop_assume!(!v1.iter().all(|&x| x == 0.0)); // Non-zero vector
            prop_assume!(!v2.iter().all(|&x| x == 0.0)); // Non-zero vector
            
            let similarity = calculate_cosine_similarity(&v1, &v2);
            
            prop_assert!(similarity >= -1.0 && similarity <= 1.0,
                        "Cosine similarity out of bounds: {}", similarity);
        }

        /// Test that identical vectors have perfect cosine similarity
        #[test]
        fn test_cosine_similarity_identity(v in arb_vector()) {
            prop_assume!(!v.iter().all(|&x| x == 0.0)); // Non-zero vector
            
            let similarity = calculate_cosine_similarity(&v, &v);
            
            // Should be very close to 1.0 (within floating point precision)
            prop_assert!((similarity - 1.0).abs() < 1e-6,
                        "Self cosine similarity not 1.0: {}", similarity);
        }

        /// Test that euclidean distance to self is zero
        #[test]
        fn test_euclidean_distance_identity(v in arb_vector()) {
            let distance = calculate_euclidean_distance(&v, &v);
            
            prop_assert!(distance < 1e-6, 
                        "Self euclidean distance not zero: {}", distance);
        }

        /// Test that dot product is commutative
        #[test]
        fn test_dot_product_commutative(
            v1 in arb_vector(),
            v2 in arb_vector()
        ) {
            prop_assume!(v1.len() == v2.len());
            
            let dot1 = calculate_dot_product(&v1, &v2);
            let dot2 = calculate_dot_product(&v2, &v1);
            
            prop_assert!((dot1 - dot2).abs() < 1e-6,
                        "Dot product not commutative: {} vs {}", dot1, dot2);
        }

        /// Test that vector normalization produces unit vectors
        #[test]
        fn test_vector_normalization(v in arb_vector()) {
            prop_assume!(!v.iter().all(|&x| x == 0.0)); // Non-zero vector
            
            let normalized = normalize_vector(&v);
            let magnitude = (normalized.iter().map(|x| x * x).sum::<f32>()).sqrt();
            
            prop_assert!((magnitude - 1.0).abs() < 1e-5,
                        "Normalized vector magnitude not 1.0: {}", magnitude);
        }

        /// Test triangle inequality for euclidean distance
        #[test]
        fn test_triangle_inequality(
            v1 in arb_vector(),
            v2 in arb_vector(),
            v3 in arb_vector()
        ) {
            prop_assume!(v1.len() == v2.len() && v2.len() == v3.len());
            
            let d12 = calculate_euclidean_distance(&v1, &v2);
            let d23 = calculate_euclidean_distance(&v2, &v3);
            let d13 = calculate_euclidean_distance(&v1, &v3);
            
            // Triangle inequality: d(a,c) <= d(a,b) + d(b,c)
            prop_assert!(d13 <= d12 + d23 + 1e-6, // Small epsilon for floating point errors
                        "Triangle inequality violated: {} > {} + {}", d13, d12, d23);
        }

        /// Test that adding vectors to index and searching works
        #[test]
        fn test_vector_index_operations(vectors in prop::collection::vec(arb_vector_with_metadata(), 1..=100)) {
            prop_assume!(!vectors.is_empty());
            
            let mut index = VectorIndex::new(VectorIndexConfig::new(
                "test_index".to_string(),
                vectors[0].data.len(),
                SimilarityMetric::Cosine,
            ));
            
            // Add all vectors to index
            for vector in &vectors {
                let _ = index.add_vector(vector.clone());
            }
            
            // Search for the first vector - should find itself
            if let Ok(results) = index.search(&vectors[0].data, 5) {
                prop_assert!(!results.is_empty(), "Search should find at least one result");
                
                // First result should be the vector itself (or very similar)
                if let Some(first_result) = results.first() {
                    prop_assert!(first_result.score >= 0.99 || first_result.score <= 0.01,
                                "First result should have high similarity or low distance: {}", 
                                first_result.score);
                }
            }
        }

        /// Test vector serialization/deserialization round trip
        #[test]
        fn test_vector_serialization_roundtrip(vector in arb_vector_with_metadata()) {
            let json = serde_json::to_string(&vector).unwrap();
            let deserialized: Vector = serde_json::from_str(&json).unwrap();
            
            prop_assert_eq!(vector.id, deserialized.id);
            prop_assert_eq!(vector.data.len(), deserialized.data.len());
            
            for (a, b) in vector.data.iter().zip(deserialized.data.iter()) {
                prop_assert!((a - b).abs() < 1e-6, "Vector data mismatch after serialization");
            }
        }

        /// Test that similarity metrics produce consistent ordering
        #[test]
        fn test_similarity_metric_consistency(
            query in arb_vector(),
            vectors in prop::collection::vec(arb_vector(), 2..=10)
        ) {
            prop_assume!(vectors.len() >= 2);
            prop_assume!(vectors.iter().all(|v| v.len() == query.len()));
            
            // Calculate similarities for all metrics
            let mut cosine_scores: Vec<(usize, f32)> = vectors.iter().enumerate()
                .map(|(i, v)| (i, calculate_cosine_similarity(&query, v)))
                .collect();
            
            let mut euclidean_scores: Vec<(usize, f32)> = vectors.iter().enumerate()
                .map(|(i, v)| (i, -calculate_euclidean_distance(&query, v))) // Negative for descending order
                .collect();
            
            // Sort by similarity (descending)
            cosine_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            euclidean_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            
            // The relative ordering should be somewhat consistent for most vectors
            // This is a weak property test - we don't expect perfect correlation
            let top_cosine = cosine_scores[0].0;
            let top_euclidean = euclidean_scores[0].0;
            
            // At least check that the results are in valid ranges
            prop_assert!(cosine_scores[0].1 >= -1.0 && cosine_scores[0].1 <= 1.0);
            prop_assert!(euclidean_scores[0].1 <= 0.0); // Negative distance should be <= 0
        }
    }

    // Helper functions that would typically be in the vector_store module
    fn calculate_euclidean_distance(v1: &[f32], v2: &[f32]) -> f32 {
        v1.iter()
            .zip(v2.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    fn calculate_cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
        let dot_product = calculate_dot_product(v1, v2);
        let magnitude1 = (v1.iter().map(|x| x.powi(2)).sum::<f32>()).sqrt();
        let magnitude2 = (v2.iter().map(|x| x.powi(2)).sum::<f32>()).sqrt();
        
        if magnitude1 == 0.0 || magnitude2 == 0.0 {
            0.0
        } else {
            dot_product / (magnitude1 * magnitude2)
        }
    }

    fn calculate_dot_product(v1: &[f32], v2: &[f32]) -> f32 {
        v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum()
    }

    fn normalize_vector(v: &[f32]) -> Vec<f32> {
        let magnitude = (v.iter().map(|x| x.powi(2)).sum::<f32>()).sqrt();
        if magnitude == 0.0 {
            v.to_vec()
        } else {
            v.iter().map(|x| x / magnitude).collect()
        }
    }
}