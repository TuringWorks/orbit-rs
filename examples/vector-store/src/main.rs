//! Vector Store Example
//!
//! Demonstrates using Orbit-RS as a vector database with Redis protocol compatibility.
//! This example shows how to:
//! - Start a Redis-compatible vector store server
//! - Add high-dimensional vectors with metadata
//! - Perform similarity searches and k-nearest neighbors queries
//! - Use both Redis FT.* commands and simplified VECTOR.* commands

use orbit_client::OrbitClientBuilder;
use orbit_protocols::{
    resp::server::RespServer,
    vector_store::{SimilarityMetric, Vector, VectorActor, VectorIndexConfig, VectorStats},
};
use orbit_shared::Key;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("vector_store=debug,orbit=info")
        .init();

    info!("🚀 Starting Orbit Vector Store Example");

    // Create OrbitClient
    let orbit_client = OrbitClientBuilder::new()
        .with_server_urls(vec!["http://127.0.0.1:50051".to_string()])
        .build()
        .await
        .map_err(|e| format!("Failed to create OrbitClient: {}", e))?;

    info!("✅ Connected to Orbit cluster");

    // Set up vector store with sample data
    setup_vector_data(&orbit_client).await?;

    // Start RESP server for Redis compatibility
    let addr = "127.0.0.1:6381"; // Different port to avoid conflict
    info!("🎯 Vector Store server listening on {}", addr);
    info!("🔗 Connect with: redis-cli -h 127.0.0.1 -p 6381");

    // Create and start server
    let server = RespServer::new(addr, orbit_client);

    // Print usage examples
    print_usage_examples();

    // Start server
    server.run().await?;

    Ok(())
}

async fn setup_vector_data(
    orbit_client: &orbit_client::OrbitClient,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("📊 Setting up sample vector data...");

    // Get a VectorActor for our examples
    let vector_actor_ref = orbit_client
        .actor_reference::<VectorActor>(Key::StringKey {
            key: "ml-embeddings".to_string(),
        })
        .await
        .map_err(|e| format!("Failed to get actor reference: {}", e))?;

    // Create a vector index for ML embeddings (384 dimensions - typical for sentence transformers)
    let index_config =
        VectorIndexConfig::new("ml-embeddings".to_string(), 384, SimilarityMetric::Cosine);

    let config_value = serde_json::to_value(index_config)?;
    vector_actor_ref
        .invoke::<()>("create_index", vec![config_value])
        .await
        .map_err(|e| format!("Failed to create index: {}", e))?;

    // Add some sample vectors (simulated embeddings)
    let sample_vectors = vec![
        (
            "doc1",
            create_sample_vector(384, 0.1),
            HashMap::from([
                (
                    "title".to_string(),
                    "Introduction to Machine Learning".to_string(),
                ),
                ("category".to_string(), "AI".to_string()),
                ("author".to_string(), "John Doe".to_string()),
            ]),
        ),
        (
            "doc2",
            create_sample_vector(384, 0.2),
            HashMap::from([
                (
                    "title".to_string(),
                    "Deep Learning Fundamentals".to_string(),
                ),
                ("category".to_string(), "AI".to_string()),
                ("author".to_string(), "Jane Smith".to_string()),
            ]),
        ),
        (
            "doc3",
            create_sample_vector(384, 0.3),
            HashMap::from([
                (
                    "title".to_string(),
                    "Neural Network Architectures".to_string(),
                ),
                ("category".to_string(), "AI".to_string()),
                ("author".to_string(), "Bob Johnson".to_string()),
            ]),
        ),
        (
            "doc4",
            create_sample_vector(384, 0.4),
            HashMap::from([
                (
                    "title".to_string(),
                    "Computer Vision Applications".to_string(),
                ),
                ("category".to_string(), "CV".to_string()),
                ("author".to_string(), "Alice Brown".to_string()),
            ]),
        ),
        (
            "doc5",
            create_sample_vector(384, 0.5),
            HashMap::from([
                (
                    "title".to_string(),
                    "Natural Language Processing".to_string(),
                ),
                ("category".to_string(), "NLP".to_string()),
                ("author".to_string(), "Charlie Wilson".to_string()),
            ]),
        ),
    ];

    // Add vectors to the store
    for (id, data, metadata) in sample_vectors {
        let vector = Vector::with_metadata(id.to_string(), data, metadata);
        let vector_value = serde_json::to_value(vector)?;

        vector_actor_ref
            .invoke::<()>("add_vector", vec![vector_value])
            .await
            .map_err(|e| format!("Failed to add vector {}: {}", id, e))?;

        info!("📝 Added vector: {}", id);
    }

    // Sleep briefly to ensure vectors are stored
    sleep(Duration::from_millis(100)).await;

    // Get vector statistics
    let stats: VectorStats = vector_actor_ref
        .invoke("get_stats", vec![])
        .await
        .map_err(|e| format!("Failed to get stats: {}", e))?;

    info!("📈 Vector store statistics:");
    info!("   • Vector count: {}", stats.vector_count);
    info!("   • Index count: {}", stats.index_count);
    info!("   • Average dimension: {:.1}", stats.avg_dimension);
    info!(
        "   • Dimension range: {} - {}",
        stats.min_dimension, stats.max_dimension
    );

    Ok(())
}

fn create_sample_vector(dimension: usize, seed: f32) -> Vec<f32> {
    // Create a simple deterministic vector for demonstration
    (0..dimension)
        .map(|i| (seed + (i as f32 * 0.01)).sin() * 0.5)
        .collect()
}

fn print_usage_examples() {
    info!("🎯 Vector Store Ready! Try these Redis commands:");
    info!("");
    info!("📋 Basic Vector Operations:");
    info!("   VECTOR.ADD my-vectors doc6 \"0.1,0.2,0.3,...\"");
    info!("   VECTOR.GET my-vectors doc1");
    info!("   VECTOR.DEL my-vectors doc1");
    info!("   VECTOR.STATS my-vectors");
    info!("");
    info!("🔍 Vector Search:");
    info!("   VECTOR.SEARCH ml-embeddings \"0.1,0.2,0.3,...\" 5");
    info!("   VECTOR.KNN ml-embeddings \"0.1,0.2,0.3,...\" 3");
    info!("");
    info!("🚀 Redis FT Commands (Advanced):");
    info!("   FT.CREATE my-index DIM 384 DISTANCE_METRIC COSINE");
    info!("   FT.ADD my-index vec1 \"0.1,0.2,0.3,...\" title \"My Document\"");
    info!("   FT.SEARCH my-index \"0.1,0.2,0.3,...\" 10 DISTANCE_METRIC COSINE");
    info!("   FT.INFO my-index");
    info!("   FT.DEL my-index vec1");
    info!("");
    info!("💡 Sample query with pre-loaded data:");
    info!(
        "   VECTOR.SEARCH ml-embeddings \"{}\" 3",
        create_sample_vector(384, 0.15)
            .iter()
            .take(10)
            .map(|f| format!("{:.3}", f))
            .collect::<Vec<_>>()
            .join(",")
    );
    info!("");
    info!("📊 Distance Metrics:");
    info!("   • COSINE: Cosine similarity (default)");
    info!("   • L2: Euclidean distance");
    info!("   • IP: Inner product (dot product)");
    info!("   • L1: Manhattan distance");
    info!("");
}
