//! pgvector-Compatible PostgreSQL Interface for Orbit Vector Store
//!
//! This example demonstrates how to use Orbit-RS as a **PostgreSQL-compatible vector database**
//! using the pgvector extension syntax. The example provides full compatibility with pgvector
//! commands, allowing existing PostgreSQL applications to seamlessly use Orbit's vector capabilities.
//!
//! ## Features
//! - 🐘 **Full PostgreSQL wire protocol compatibility**
//! - 📦 **pgvector extension syntax support**  
//! - 🧮 **Vector data types** (VECTOR, HALFVEC)
//! - 🔍 **Similarity search operators** (<->, <#>, <=>)
//! - 📊 **Vector indexes** (ivfflat, hnsw)
//! - 🎯 **Vector functions** (vector_dims, vector_norm)
//! - ⚡ **High-performance similarity search**

use anyhow::Result;
use orbit_client::OrbitClientBuilder;
use orbit_protocols::{
    postgres_wire::{PostgresServer, QueryEngine},
    vector_store::{SimilarityMetric, Vector, VectorActor, VectorIndexConfig},
};
use orbit_shared::Key;
use std::collections::HashMap;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("pgvector_store=debug,orbit=info")
        .init();

    info!("🐘 Starting Orbit pgvector-Compatible PostgreSQL Server");

    // Create OrbitClient
    let orbit_client = OrbitClientBuilder::new()
        .with_server_urls(vec!["http://127.0.0.1:50051".to_string()])
        .build()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create OrbitClient: {}", e))?;

    info!("✅ Connected to Orbit cluster");

    // Set up sample vector data for demonstration
    setup_sample_data(&orbit_client).await?;

    // Create PostgreSQL server with vector support
    let query_engine = QueryEngine::new_with_vector_support(orbit_client);
    let postgres_server = PostgresServer::new_with_query_engine("127.0.0.1:5433", query_engine);

    info!("🎯 pgvector-compatible PostgreSQL server listening on 127.0.0.1:5433");
    info!("🔗 Connect with: psql -h 127.0.0.1 -p 5433 -d postgres -U postgres");

    // Print usage examples
    print_pgvector_examples();

    // Start the PostgreSQL server
    postgres_server.run().await?;

    Ok(())
}

/// Set up sample vector data for demonstration
async fn setup_sample_data(orbit_client: &orbit_client::OrbitClient) -> Result<()> {
    info!("📊 Setting up sample vector data...");

    // Create sample documents table data
    let documents_actor_ref = orbit_client
        .actor_reference::<VectorActor>(Key::StringKey {
            key: "table_documents".to_string(),
        })
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get documents actor: {}", e))?;

    // Create vector index for documents
    let index_config = VectorIndexConfig::new(
        "documents_embedding_idx".to_string(),
        384, // Standard sentence transformer dimension
        SimilarityMetric::Cosine,
    );

    let config_value = serde_json::to_value(index_config)?;
    documents_actor_ref
        .invoke::<()>("create_index", vec![config_value])
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create index: {}", e))?;

    // Add sample document vectors
    let sample_docs = vec![
        (
            "doc_1",
            "PostgreSQL is a powerful, open source object-relational database system.",
            create_sample_embedding(384, 0.1),
        ),
        (
            "doc_2",
            "Vector databases enable semantic search and similarity matching.",
            create_sample_embedding(384, 0.2),
        ),
        (
            "doc_3",
            "Machine learning embeddings represent text as high-dimensional vectors.",
            create_sample_embedding(384, 0.3),
        ),
        (
            "doc_4",
            "Similarity search finds the most relevant documents for a query.",
            create_sample_embedding(384, 0.4),
        ),
        (
            "doc_5",
            "pgvector is a PostgreSQL extension for vector similarity search.",
            create_sample_embedding(384, 0.5),
        ),
    ];

    for (doc_id, content, embedding) in sample_docs {
        let metadata = HashMap::from([
            ("content".to_string(), content.to_string()),
            ("doc_type".to_string(), "article".to_string()),
        ]);

        let vector = Vector::with_metadata(doc_id.to_string(), embedding, metadata);
        let vector_value = serde_json::to_value(vector)?;

        documents_actor_ref
            .invoke::<()>("add_vector", vec![vector_value])
            .await
            .map_err(|e| anyhow::anyhow!("Failed to add vector: {}", e))?;

        info!("📄 Added document: {}", doc_id);
    }

    // Create products table data for e-commerce example
    let products_actor_ref = orbit_client
        .actor_reference::<VectorActor>(Key::StringKey {
            key: "table_products".to_string(),
        })
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get products actor: {}", e))?;

    let products_index_config = VectorIndexConfig::new(
        "products_features_idx".to_string(),
        128, // Product feature vectors
        SimilarityMetric::Euclidean,
    );

    let products_config_value = serde_json::to_value(products_index_config)?;
    products_actor_ref
        .invoke::<()>("create_index", vec![products_config_value])
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create products index: {}", e))?;

    // Add sample product vectors
    let sample_products = vec![
        (
            "prod_1",
            "Wireless Bluetooth Headphones",
            create_sample_embedding(128, 0.7),
        ),
        (
            "prod_2",
            "Noise-Cancelling Earbuds",
            create_sample_embedding(128, 0.8),
        ),
        (
            "prod_3",
            "Gaming Keyboard RGB",
            create_sample_embedding(128, 0.9),
        ),
    ];

    for (prod_id, name, features) in sample_products {
        let metadata = HashMap::from([
            ("name".to_string(), name.to_string()),
            ("category".to_string(), "electronics".to_string()),
        ]);

        let vector = Vector::with_metadata(prod_id.to_string(), features, metadata);
        let vector_value = serde_json::to_value(vector)?;

        products_actor_ref
            .invoke::<()>("add_vector", vec![vector_value])
            .await
            .map_err(|e| anyhow::anyhow!("Failed to add product vector: {}", e))?;
    }

    info!("✅ Sample data setup complete");
    Ok(())
}

/// Create a sample embedding vector with deterministic values
fn create_sample_embedding(dimension: usize, seed: f32) -> Vec<f32> {
    (0..dimension)
        .map(|i| ((seed + i as f32 * 0.01).sin() * 0.5))
        .collect()
}

/// Print comprehensive pgvector usage examples
fn print_pgvector_examples() {
    info!("🐘 pgvector-Compatible PostgreSQL Server Ready!");
    info!("");
    info!("📋 Connect with PostgreSQL clients:");
    info!("   psql -h 127.0.0.1 -p 5433 -d postgres -U postgres");
    info!("   pgAdmin: localhost:5433");
    info!("");
    info!("🚀 pgvector Extension Commands:");
    info!("");
    info!("1️⃣  Enable the vector extension:");
    info!("   CREATE EXTENSION vector;");
    info!("");
    info!("2️⃣  Create tables with vector columns:");
    info!("   CREATE TABLE documents (");
    info!("     id SERIAL PRIMARY KEY,");
    info!("     content TEXT,");
    info!("     embedding VECTOR(384)");
    info!("   );");
    info!("");
    info!("   CREATE TABLE products (");
    info!("     id SERIAL PRIMARY KEY,");
    info!("     name TEXT,");
    info!("     features VECTOR(128)");
    info!("   );");
    info!("");
    info!("3️⃣  Insert vectors:");
    info!("   INSERT INTO documents (content, embedding) VALUES (");
    info!("     'Sample document text',");
    info!("     '[0.1, 0.2, 0.3, ...]'");
    info!("   );");
    info!("");
    info!("4️⃣  Create vector indexes:");
    info!("   CREATE INDEX ON documents USING ivfflat (embedding vector_cosine_ops);");
    info!("   CREATE INDEX ON products USING hnsw (features vector_l2_ops);");
    info!("");
    info!("5️⃣  Similarity search queries:");
    info!("");
    info!("   -- Cosine similarity (best for embeddings)");
    info!("   SELECT content, embedding <=> '[0.1, 0.2, ...]' AS distance");
    info!("   FROM documents ORDER BY distance LIMIT 5;");
    info!("");
    info!("   -- Euclidean distance");
    info!("   SELECT name, features <-> '[0.7, 0.8, ...]' AS distance");
    info!("   FROM products ORDER BY distance LIMIT 3;");
    info!("");
    info!("   -- Inner product similarity");
    info!("   SELECT content, embedding <#> '[0.1, 0.2, ...]' AS similarity");
    info!("   FROM documents ORDER BY similarity DESC LIMIT 5;");
    info!("");
    info!("6️⃣  Vector functions:");
    info!("   SELECT vector_dims(embedding) FROM documents LIMIT 1;");
    info!("   -- Returns: 384");
    info!("");
    info!("🔗 Similarity Operators:");
    info!("   <->  Euclidean distance (L2)");
    info!("   <#>  Inner product (dot product)");
    info!("   <=>  Cosine distance");
    info!("");
    info!("📚 Index Types:");
    info!("   ivfflat  - Inverted File Flat (fast, approximate)");
    info!("   hnsw     - Hierarchical Navigable Small World (high accuracy)");
    info!("");
    info!("💡 Try these sample queries with the pre-loaded data:");

    let sample_query_vector = create_sample_embedding(384, 0.15);
    let sample_vector_str = sample_query_vector
        .iter()
        .take(5)
        .map(|f| format!("{:.3}", f))
        .collect::<Vec<_>>()
        .join(", ");

    info!("   SELECT content FROM table_documents");
    info!(
        "   WHERE embedding <=> '[{}, ...]' < 0.8",
        sample_vector_str
    );
    info!("");
    info!("🎯 Use Cases:");
    info!("   • Semantic document search");
    info!("   • Product recommendations");
    info!("   • Content similarity matching");
    info!("   • Anomaly detection");
    info!("   • RAG (Retrieval-Augmented Generation)");
    info!("");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_sample_embedding() {
        let embedding = create_sample_embedding(10, 0.5);
        assert_eq!(embedding.len(), 10);
        assert!(embedding.iter().all(|&x| x >= -1.0 && x <= 1.0));
    }

    #[tokio::test]
    async fn test_setup_sample_data_structure() {
        // This test would require a running Orbit instance
        // For now, just test the embedding creation
        let embedding = create_sample_embedding(384, 0.1);
        assert_eq!(embedding.len(), 384);
    }
}
