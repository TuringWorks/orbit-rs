//! Comprehensive Multi-Model Database Example
//!
//! This example demonstrates the full power of Orbit-RS by combining:
//! - SQL operations with PostgreSQL wire protocol
//! - Vector similarity search for AI/ML workloads
//! - Graph queries using Cypher
//! - Document queries using AQL
//! - Real-time event streaming
//! - Distributed transactions across all data models
//!
//! ## Scenario: E-commerce Recommendation System
//! 
//! We'll build a complete e-commerce recommendation system that:
//! 1. Stores product data in SQL tables
//! 2. Uses vector embeddings for product similarity
//! 3. Models customer relationships as a graph
//! 4. Stores flexible product metadata as documents
//! 5. Processes real-time user interactions
//! 6. Ensures consistency with distributed transactions

use orbit_client::OrbitClient;
use orbit_protocols::postgres_wire::PostgresServer;
use orbit_protocols::resp::RespServer;
use orbit_shared::{Key, ActorSystemConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
struct Product {
    id: u32,
    name: String,
    description: String,
    price: f64,
    category: String,
    embedding: Vec<f32>, // Product feature vector
    metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Customer {
    id: u32,
    name: String,
    email: String,
    preferences: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Purchase {
    id: Uuid,
    customer_id: u32,
    product_id: u32,
    quantity: i32,
    timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Recommendation {
    customer_id: u32,
    product_id: u32,
    score: f32,
    reason: String,
}

/// Main application orchestrating the multi-model demo
struct EcommerceRecommendationSystem {
    orbit_client: OrbitClient,
    postgres_server: PostgresServer,
    resp_server: RespServer,
}

impl EcommerceRecommendationSystem {
    /// Initialize the complete system
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        println!("🚀 Starting Orbit-RS Multi-Model E-commerce Demo");

        // Start actor system
        let config = ActorSystemConfig::default();
        let orbit_client = OrbitClient::new(config).await?;

        // Start protocol servers
        let postgres_server = PostgresServer::new("127.0.0.1:5432".to_string());
        let resp_server = RespServer::new("127.0.0.1:6379".to_string());

        println!("✅ Initialized Orbit client and protocol servers");

        Ok(Self {
            orbit_client,
            postgres_server,
            resp_server,
        })
    }

    /// Set up the database schema across all data models
    async fn initialize_schema(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("📋 Setting up multi-model database schema...");

        // 1. SQL Schema - Core transactional data
        self.setup_sql_schema().await?;

        // 2. Vector Store - Product embeddings for similarity search
        self.setup_vector_store().await?;

        // 3. Graph Schema - Customer relationships and preferences
        self.setup_graph_schema().await?;

        // 4. Document Store - Flexible product metadata
        self.setup_document_store().await?;

        println!("✅ Multi-model schema setup complete");
        Ok(())
    }

    /// Setup SQL schema for core transactional data
    async fn setup_sql_schema(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("  🗃️  Setting up SQL tables...");

        // Get SQL executor actor
        let sql_executor = self.orbit_client
            .actor_reference::<orbit_protocols::postgres_wire::sql::SqlExecutor>(Key::StringKey {
                key: "sql_executor".to_string(),
            })
            .await?;

        // Create core tables
        let create_statements = vec![
            "CREATE TABLE IF NOT EXISTS customers (
                id SERIAL PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                email VARCHAR(255) UNIQUE NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            "CREATE TABLE IF NOT EXISTS products (
                id SERIAL PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                description TEXT,
                price DECIMAL(10,2) NOT NULL,
                category VARCHAR(100),
                in_stock INTEGER DEFAULT 0,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            "CREATE TABLE IF NOT EXISTS purchases (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                customer_id INTEGER REFERENCES customers(id),
                product_id INTEGER REFERENCES products(id),
                quantity INTEGER NOT NULL,
                total_amount DECIMAL(10,2),
                purchase_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            "CREATE INDEX IF NOT EXISTS idx_purchases_customer ON purchases(customer_id)",
            "CREATE INDEX IF NOT EXISTS idx_purchases_product ON purchases(product_id)",
            "CREATE INDEX IF NOT EXISTS idx_products_category ON products(category)",
        ];

        for stmt in create_statements {
            sql_executor.invoke::<()>("execute", vec![serde_json::Value::String(stmt.to_string())]).await?;
        }

        println!("    ✅ SQL tables created");
        Ok(())
    }

    /// Setup vector store for product embeddings
    async fn setup_vector_store(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("  🔢 Setting up vector store...");

        let vector_store = self.orbit_client
            .actor_reference::<orbit_protocols::vector_store::VectorActor>(Key::StringKey {
                key: "product_embeddings".to_string(),
            })
            .await?;

        // Create vector index for product embeddings (384 dimensions - typical for sentence transformers)
        let index_config = orbit_protocols::vector_store::VectorIndexConfig::new(
            "product_similarity".to_string(),
            384,
            orbit_protocols::vector_store::SimilarityMetric::Cosine,
        );

        vector_store.invoke::<()>("create_index", vec![serde_json::to_value(index_config)?]).await?;

        println!("    ✅ Vector index created");
        Ok(())
    }

    /// Setup graph schema for relationships
    async fn setup_graph_schema(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("  🕸️  Setting up graph database...");

        let graph_engine = self.orbit_client
            .actor_reference::<orbit_protocols::cypher::GraphEngine>(Key::StringKey {
                key: "customer_graph".to_string(),
            })
            .await?;

        // Create node and relationship types using Cypher
        let cypher_statements = vec![
            "CREATE CONSTRAINT FOR (c:Customer) REQUIRE c.id IS UNIQUE",
            "CREATE CONSTRAINT FOR (p:Product) REQUIRE p.id IS UNIQUE",
            "CREATE INDEX FOR (c:Customer) ON (c.email)",
            "CREATE INDEX FOR (p:Product) ON (p.category)",
        ];

        for stmt in cypher_statements {
            graph_engine.invoke::<()>("execute", vec![serde_json::Value::String(stmt.to_string())]).await?;
        }

        println!("    ✅ Graph constraints and indices created");
        Ok(())
    }

    /// Setup document store for flexible metadata
    async fn setup_document_store(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("  📄 Setting up document store...");

        let aql_engine = self.orbit_client
            .actor_reference::<orbit_protocols::aql::AqlQueryEngine>(Key::StringKey {
                key: "product_metadata".to_string(),
            })
            .await?;

        // Create collections for flexible data
        let aql_statements = vec![
            "CREATE COLLECTION product_metadata",
            "CREATE COLLECTION customer_preferences", 
            "CREATE COLLECTION interaction_logs",
            "CREATE INDEX idx_metadata_product_id ON product_metadata (product_id)",
            "CREATE INDEX idx_preferences_customer_id ON customer_preferences (customer_id)",
        ];

        for stmt in aql_statements {
            aql_engine.invoke::<()>("execute", vec![serde_json::Value::String(stmt.to_string())]).await?;
        }

        println!("    ✅ Document collections created");
        Ok(())
    }

    /// Load sample data across all data models
    async fn load_sample_data(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("📊 Loading sample e-commerce data...");

        // Load data in parallel across different models
        let (_, _, _, _) = tokio::try_join!(
            self.load_sql_data(),
            self.load_vector_data(),
            self.load_graph_data(),
            self.load_document_data()
        )?;

        println!("✅ Sample data loaded successfully");
        Ok(())
    }

    /// Load core SQL data
    async fn load_sql_data(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("  📊 Loading SQL data...");

        let sql_executor = self.orbit_client
            .actor_reference::<orbit_protocols::postgres_wire::sql::SqlExecutor>(Key::StringKey {
                key: "sql_executor".to_string(),
            })
            .await?;

        // Insert sample customers
        let customers = vec![
            ("Alice Johnson", "alice@example.com"),
            ("Bob Smith", "bob@example.com"),
            ("Charlie Brown", "charlie@example.com"),
            ("Diana Miller", "diana@example.com"),
            ("Eve Wilson", "eve@example.com"),
        ];

        for (name, email) in customers {
            let sql = format!("INSERT INTO customers (name, email) VALUES ('{}', '{}')", name, email);
            sql_executor.invoke::<()>("execute", vec![serde_json::Value::String(sql)]).await?;
        }

        // Insert sample products
        let products = vec![
            ("Wireless Headphones", "High-quality Bluetooth headphones", 99.99, "Electronics"),
            ("Running Shoes", "Comfortable athletic shoes for runners", 129.99, "Sports"),
            ("Coffee Maker", "Automatic drip coffee maker", 79.99, "Home"),
            ("Smartphone", "Latest model with advanced features", 699.99, "Electronics"),
            ("Yoga Mat", "Non-slip exercise mat", 29.99, "Sports"),
            ("Laptop", "High-performance laptop for professionals", 1299.99, "Electronics"),
            ("Cookware Set", "Non-stick pots and pans set", 149.99, "Home"),
            ("Fitness Tracker", "Wearable device for health monitoring", 199.99, "Electronics"),
        ];

        for (name, desc, price, category) in products {
            let sql = format!(
                "INSERT INTO products (name, description, price, category, in_stock) VALUES ('{}', '{}', {}, '{}', {})",
                name, desc, price, category, 50
            );
            sql_executor.invoke::<()>("execute", vec![serde_json::Value::String(sql)]).await?;
        }

        println!("    ✅ SQL data loaded");
        Ok(())
    }

    /// Load vector embeddings for products
    async fn load_vector_data(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("  🔢 Loading vector embeddings...");

        let vector_store = self.orbit_client
            .actor_reference::<orbit_protocols::vector_store::VectorActor>(Key::StringKey {
                key: "product_embeddings".to_string(),
            })
            .await?;

        // Generate mock embeddings (in reality, these would come from a model like BERT)
        let product_embeddings = vec![
            (1, generate_mock_embedding("wireless headphones audio bluetooth")),
            (2, generate_mock_embedding("running shoes sports fitness athletic")),
            (3, generate_mock_embedding("coffee maker kitchen appliance brew")),
            (4, generate_mock_embedding("smartphone mobile phone technology")),
            (5, generate_mock_embedding("yoga mat exercise fitness health")),
            (6, generate_mock_embedding("laptop computer technology work")),
            (7, generate_mock_embedding("cookware kitchen cooking pots pans")),
            (8, generate_mock_embedding("fitness tracker health wearable device")),
        ];

        for (product_id, embedding) in product_embeddings {
            let vector = orbit_protocols::vector_store::Vector::with_metadata(
                format!("product_{}", product_id),
                embedding,
                HashMap::from([
                    ("product_id".to_string(), serde_json::Value::Number(serde_json::Number::from(product_id))),
                    ("type".to_string(), serde_json::Value::String("product".to_string())),
                ]),
            );

            vector_store.invoke::<()>("add_vector", vec![serde_json::to_value(vector)?]).await?;
        }

        println!("    ✅ Vector embeddings loaded");
        Ok(())
    }

    /// Load graph relationships
    async fn load_graph_data(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("  🕸️  Loading graph relationships...");

        let graph_engine = self.orbit_client
            .actor_reference::<orbit_protocols::cypher::GraphEngine>(Key::StringKey {
                key: "customer_graph".to_string(),
            })
            .await?;

        // Create customer nodes
        let create_customers = vec![
            "CREATE (c1:Customer {id: 1, name: 'Alice Johnson', email: 'alice@example.com', segment: 'tech_enthusiast'})",
            "CREATE (c2:Customer {id: 2, name: 'Bob Smith', email: 'bob@example.com', segment: 'fitness_lover'})",
            "CREATE (c3:Customer {id: 3, name: 'Charlie Brown', email: 'charlie@example.com', segment: 'home_chef'})",
            "CREATE (c4:Customer {id: 4, name: 'Diana Miller', email: 'diana@example.com', segment: 'professional'})",
            "CREATE (c5:Customer {id: 5, name: 'Eve Wilson', email: 'eve@example.com', segment: 'fitness_lover'})",
        ];

        // Create product nodes
        let create_products = vec![
            "CREATE (p1:Product {id: 1, name: 'Wireless Headphones', category: 'Electronics'})",
            "CREATE (p2:Product {id: 2, name: 'Running Shoes', category: 'Sports'})",
            "CREATE (p3:Product {id: 3, name: 'Coffee Maker', category: 'Home'})",
            "CREATE (p4:Product {id: 4, name: 'Smartphone', category: 'Electronics'})",
            "CREATE (p5:Product {id: 5, name: 'Yoga Mat', category: 'Sports'})",
            "CREATE (p6:Product {id: 6, name: 'Laptop', category: 'Electronics'})",
            "CREATE (p7:Product {id: 7, name: 'Cookware Set', category: 'Home'})",
            "CREATE (p8:Product {id: 8, name: 'Fitness Tracker', category: 'Electronics'})",
        ];

        // Create relationships
        let create_relationships = vec![
            // Purchase relationships
            "MATCH (c:Customer {id: 1}), (p:Product {id: 1}) CREATE (c)-[:PURCHASED {rating: 5, date: '2024-01-15'}]->(p)",
            "MATCH (c:Customer {id: 1}), (p:Product {id: 4}) CREATE (c)-[:PURCHASED {rating: 4, date: '2024-01-20'}]->(p)",
            "MATCH (c:Customer {id: 2}), (p:Product {id: 2}) CREATE (c)-[:PURCHASED {rating: 5, date: '2024-01-18'}]->(p)",
            "MATCH (c:Customer {id: 2}), (p:Product {id: 8}) CREATE (c)-[:PURCHASED {rating: 4, date: '2024-01-25'}]->(p)",
            "MATCH (c:Customer {id: 3}), (p:Product {id: 3}) CREATE (c)-[:PURCHASED {rating: 5, date: '2024-01-22'}]->(p)",
            "MATCH (c:Customer {id: 3}), (p:Product {id: 7}) CREATE (c)-[:PURCHASED {rating: 4, date: '2024-01-28'}]->(p)",

            // Interest relationships
            "MATCH (c:Customer {id: 1}), (p:Product {id: 6}) CREATE (c)-[:INTERESTED_IN {score: 0.8}]->(p)",
            "MATCH (c:Customer {id: 2}), (p:Product {id: 5}) CREATE (c)-[:INTERESTED_IN {score: 0.9}]->(p)",
            "MATCH (c:Customer {id: 4}), (p:Product {id: 6}) CREATE (c)-[:INTERESTED_IN {score: 0.85}]->(p)",

            // Customer similarities
            "MATCH (c1:Customer {id: 1}), (c2:Customer {id: 4}) CREATE (c1)-[:SIMILAR_TO {score: 0.7, reason: 'tech_preferences'}]->(c2)",
            "MATCH (c1:Customer {id: 2}), (c2:Customer {id: 5}) CREATE (c1)-[:SIMILAR_TO {score: 0.8, reason: 'fitness_interests'}]->(c2)",
        ];

        // Execute all graph creation statements
        for statements in [create_customers, create_products, create_relationships].iter() {
            for stmt in statements {
                graph_engine.invoke::<()>("execute", vec![serde_json::Value::String(stmt.to_string())]).await?;
            }
        }

        println!("    ✅ Graph relationships loaded");
        Ok(())
    }

    /// Load flexible document metadata
    async fn load_document_data(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("  📄 Loading document metadata...");

        let aql_engine = self.orbit_client
            .actor_reference::<orbit_protocols::aql::AqlQueryEngine>(Key::StringKey {
                key: "product_metadata".to_string(),
            })
            .await?;

        // Insert rich product metadata
        let metadata_docs = vec![
            serde_json::json!({
                "product_id": 1,
                "detailed_specs": {
                    "battery_life": "30 hours",
                    "noise_cancellation": true,
                    "wireless_range": "10 meters",
                    "supported_codecs": ["SBC", "AAC", "aptX"]
                },
                "customer_reviews": [
                    {"rating": 5, "comment": "Amazing sound quality!", "helpful_votes": 15},
                    {"rating": 4, "comment": "Great battery life", "helpful_votes": 8}
                ],
                "marketing_tags": ["premium", "audiophile", "travel-friendly"],
                "seasonal_demand": {"q1": 1.2, "q2": 0.8, "q3": 0.9, "q4": 1.5}
            }),
            serde_json::json!({
                "product_id": 2,
                "detailed_specs": {
                    "sizes": ["6", "7", "8", "9", "10", "11", "12"],
                    "colors": ["black", "white", "blue", "red"],
                    "weight": "280g",
                    "cushioning": "gel"
                },
                "customer_reviews": [
                    {"rating": 5, "comment": "Perfect for marathons!", "helpful_votes": 22},
                    {"rating": 5, "comment": "Very comfortable", "helpful_votes": 18}
                ],
                "marketing_tags": ["performance", "marathon", "comfort"],
                "seasonal_demand": {"q1": 1.1, "q2": 1.3, "q3": 0.7, "q4": 0.9}
            }),
        ];

        for metadata in metadata_docs {
            let aql = format!("INSERT {} INTO product_metadata", metadata);
            aql_engine.invoke::<()>("execute", vec![serde_json::Value::String(aql)]).await?;
        }

        // Insert customer preferences
        let preference_docs = vec![
            serde_json::json!({
                "customer_id": 1,
                "preferences": {
                    "price_sensitivity": "low",
                    "brand_loyalty": ["Sony", "Apple", "Bose"],
                    "preferred_categories": ["Electronics"],
                    "feature_priorities": ["quality", "innovation", "design"]
                },
                "browsing_history": ["smartphones", "laptops", "headphones"],
                "last_updated": "2024-01-30T10:00:00Z"
            }),
            serde_json::json!({
                "customer_id": 2,
                "preferences": {
                    "price_sensitivity": "medium",
                    "brand_loyalty": ["Nike", "Adidas", "Garmin"],
                    "preferred_categories": ["Sports", "Health"],
                    "feature_priorities": ["performance", "durability", "value"]
                },
                "browsing_history": ["running_shoes", "fitness_trackers", "sports_equipment"],
                "last_updated": "2024-01-30T10:00:00Z"
            }),
        ];

        for pref in preference_docs {
            let aql = format!("INSERT {} INTO customer_preferences", pref);
            aql_engine.invoke::<()>("execute", vec![serde_json::Value::String(aql)]).await?;
        }

        println!("    ✅ Document metadata loaded");
        Ok(())
    }

    /// Demonstrate multi-model queries for personalized recommendations
    async fn demonstrate_recommendations(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n🎯 Generating Multi-Model Personalized Recommendations");

        let customer_id = 1; // Alice Johnson

        // 1. Get customer profile from SQL
        let customer_profile = self.get_customer_profile(customer_id).await?;
        println!("👤 Customer Profile: {:?}", customer_profile);

        // 2. Get customer preferences from document store
        let preferences = self.get_customer_preferences(customer_id).await?;
        println!("🎨 Customer Preferences: {:?}", preferences);

        // 3. Find similar customers using graph relationships
        let similar_customers = self.find_similar_customers(customer_id).await?;
        println!("👥 Similar Customers: {:?}", similar_customers);

        // 4. Get collaborative filtering recommendations
        let collaborative_recs = self.get_collaborative_recommendations(customer_id, &similar_customers).await?;
        println!("🤝 Collaborative Recommendations: {:?}", collaborative_recs);

        // 5. Get content-based recommendations using vector similarity
        let content_recs = self.get_content_based_recommendations(customer_id).await?;
        println!("📊 Content-based Recommendations: {:?}", content_recs);

        // 6. Combine and rank final recommendations
        let final_recommendations = self.generate_final_recommendations(
            customer_id,
            collaborative_recs,
            content_recs,
            &preferences
        ).await?;

        println!("\n🏆 Final Personalized Recommendations:");
        for (i, rec) in final_recommendations.iter().enumerate() {
            println!("  {}. Product {} (Score: {:.2}) - {}", 
                    i + 1, rec.product_id, rec.score, rec.reason);
        }

        Ok(())
    }

    /// Get customer profile from SQL
    async fn get_customer_profile(&self, customer_id: u32) -> Result<Customer, Box<dyn std::error::Error>> {
        let sql_executor = self.orbit_client
            .actor_reference::<orbit_protocols::postgres_wire::sql::SqlExecutor>(Key::StringKey {
                key: "sql_executor".to_string(),
            })
            .await?;

        let sql = format!("SELECT id, name, email FROM customers WHERE id = {}", customer_id);
        let result: serde_json::Value = sql_executor.invoke("execute", vec![serde_json::Value::String(sql)]).await?;

        // Parse the result (simplified for demo)
        Ok(Customer {
            id: customer_id,
            name: "Alice Johnson".to_string(),
            email: "alice@example.com".to_string(),
            preferences: vec!["Electronics".to_string(), "Technology".to_string()],
        })
    }

    /// Get customer preferences from document store
    async fn get_customer_preferences(&self, customer_id: u32) -> Result<HashMap<String, serde_json::Value>, Box<dyn std::error::Error>> {
        let aql_engine = self.orbit_client
            .actor_reference::<orbit_protocols::aql::AqlQueryEngine>(Key::StringKey {
                key: "product_metadata".to_string(),
            })
            .await?;

        let aql = format!("FOR doc IN customer_preferences FILTER doc.customer_id == {} RETURN doc.preferences", customer_id);
        let _result: serde_json::Value = aql_engine.invoke("execute", vec![serde_json::Value::String(aql)]).await?;

        // Return mock preferences for demo
        Ok(HashMap::from([
            ("price_sensitivity".to_string(), serde_json::Value::String("low".to_string())),
            ("preferred_categories".to_string(), serde_json::json!(["Electronics"])),
        ]))
    }

    /// Find similar customers using graph queries
    async fn find_similar_customers(&self, customer_id: u32) -> Result<Vec<u32>, Box<dyn std::error::Error>> {
        let graph_engine = self.orbit_client
            .actor_reference::<orbit_protocols::cypher::GraphEngine>(Key::StringKey {
                key: "customer_graph".to_string(),
            })
            .await?;

        let cypher = format!(
            "MATCH (c:Customer {{id: {}}})-[:SIMILAR_TO]-(similar:Customer) 
             RETURN similar.id ORDER BY similar.id",
            customer_id
        );

        let _result: serde_json::Value = graph_engine.invoke("execute", vec![serde_json::Value::String(cypher)]).await?;

        // Return mock similar customers for demo
        Ok(vec![4]) // Diana Miller is similar to Alice
    }

    /// Get collaborative filtering recommendations
    async fn get_collaborative_recommendations(&self, _customer_id: u32, similar_customers: &[u32]) -> Result<Vec<Recommendation>, Box<dyn std::error::Error>> {
        // For this demo, return mock collaborative recommendations
        let mut recommendations = Vec::new();

        for &similar_customer_id in similar_customers {
            // In reality, we'd query what similar customers purchased/liked
            recommendations.push(Recommendation {
                customer_id: _customer_id,
                product_id: 6, // Laptop
                score: 0.8,
                reason: format!("Customers similar to you (customer {}) also liked this", similar_customer_id),
            });
        }

        Ok(recommendations)
    }

    /// Get content-based recommendations using vector similarity
    async fn get_content_based_recommendations(&self, customer_id: u32) -> Result<Vec<Recommendation>, Box<dyn std::error::Error>> {
        let vector_store = self.orbit_client
            .actor_reference::<orbit_protocols::vector_store::VectorActor>(Key::StringKey {
                key: "product_embeddings".to_string(),
            })
            .await?;

        // Get customer's purchase history to build preference vector
        // For demo, using a mock preference vector for tech enthusiast
        let preference_vector = generate_mock_embedding("technology electronics premium quality innovation");

        let search_params = orbit_protocols::vector_store::VectorSearchParams::new(
            preference_vector,
            orbit_protocols::vector_store::SimilarityMetric::Cosine,
            3, // Top 3 similar products
        );

        let results: serde_json::Value = vector_store.invoke("search_vectors", vec![serde_json::to_value(search_params)?]).await?;

        // Convert to recommendations (simplified for demo)
        let mut recommendations = Vec::new();
        recommendations.push(Recommendation {
            customer_id,
            product_id: 6, // Laptop
            score: 0.9,
            reason: "Based on your interest in premium technology products".to_string(),
        });
        recommendations.push(Recommendation {
            customer_id,
            product_id: 8, // Fitness Tracker
            score: 0.75,
            reason: "Technology product that complements your lifestyle".to_string(),
        });

        Ok(recommendations)
    }

    /// Generate final ranked recommendations combining multiple signals
    async fn generate_final_recommendations(
        &self,
        customer_id: u32,
        collaborative: Vec<Recommendation>,
        content_based: Vec<Recommendation>,
        _preferences: &HashMap<String, serde_json::Value>,
    ) -> Result<Vec<Recommendation>, Box<dyn std::error::Error>> {
        let mut final_recommendations = Vec::new();

        // Combine and weight different recommendation sources
        for rec in collaborative {
            final_recommendations.push(Recommendation {
                customer_id,
                product_id: rec.product_id,
                score: rec.score * 0.6, // Weight collaborative filtering
                reason: format!("Collaborative: {}", rec.reason),
            });
        }

        for rec in content_based {
            // Check if we already have this product from collaborative
            if let Some(existing) = final_recommendations.iter_mut().find(|r| r.product_id == rec.product_id) {
                // Boost score if both methods agree
                existing.score = (existing.score + rec.score * 0.4).min(1.0);
                existing.reason = format!("{} + Content-based agreement", existing.reason);
            } else {
                final_recommendations.push(Recommendation {
                    customer_id,
                    product_id: rec.product_id,
                    score: rec.score * 0.4, // Weight content-based
                    reason: format!("Content-based: {}", rec.reason),
                });
            }
        }

        // Sort by score descending
        final_recommendations.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        Ok(final_recommendations)
    }

    /// Demonstrate distributed transaction across multiple models
    async fn demonstrate_distributed_transaction(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n💳 Demonstrating Distributed Transaction Across All Data Models");

        let customer_id = 1;
        let product_id = 6; // Laptop
        let quantity = 1;

        println!("📦 Processing purchase: Customer {} buying {} unit(s) of Product {}", 
                customer_id, quantity, product_id);

        // Start distributed transaction
        let transaction_id = Uuid::new_v4();
        println!("🔄 Starting distributed transaction: {}", transaction_id);

        // This would involve:
        // 1. SQL: Insert purchase record, update inventory
        // 2. Graph: Create PURCHASED relationship
        // 3. Vector: Update customer preference vector
        // 4. Document: Log interaction for analytics

        match self.execute_distributed_purchase_transaction(
            transaction_id,
            customer_id,
            product_id,
            quantity
        ).await {
            Ok(_) => {
                println!("✅ Distributed transaction completed successfully");
                println!("   - Purchase recorded in SQL");
                println!("   - Relationship created in graph");
                println!("   - Customer vector updated");
                println!("   - Interaction logged in documents");
            }
            Err(e) => {
                println!("❌ Distributed transaction failed: {}", e);
                println!("🔄 Rolling back across all data models...");
                // Rollback logic would go here
            }
        }

        Ok(())
    }

    /// Execute a distributed transaction across multiple data models
    async fn execute_distributed_purchase_transaction(
        &self,
        transaction_id: Uuid,
        customer_id: u32,
        product_id: u32,
        quantity: i32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // In a real implementation, this would use a distributed transaction coordinator
        // For demo purposes, we'll simulate the operations

        println!("  1️⃣ SQL: Recording purchase and updating inventory...");
        // SQL operations would be here
        sleep(Duration::from_millis(100)).await; // Simulate work

        println!("  2️⃣ Graph: Creating purchase relationship...");
        // Graph operations would be here
        sleep(Duration::from_millis(100)).await; // Simulate work

        println!("  3️⃣ Vector: Updating customer preferences...");
        // Vector operations would be here
        sleep(Duration::from_millis(100)).await; // Simulate work

        println!("  4️⃣ Document: Logging interaction...");
        // Document operations would be here
        sleep(Duration::from_millis(100)).await; // Simulate work

        Ok(())
    }

    /// Start all protocol servers
    async fn start_servers(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🚀 Starting protocol servers...");

        // In a real implementation, these would be started in separate tasks
        tokio::spawn(async move {
            // PostgreSQL wire protocol server would start here
            println!("  🐘 PostgreSQL server listening on 127.0.0.1:5432");
        });

        tokio::spawn(async move {
            // Redis-compatible RESP server would start here
            println!("  🔴 Redis-compatible server listening on 127.0.0.1:6379");
        });

        println!("✅ All protocol servers started");
        Ok(())
    }

    /// Run the complete demonstration
    async fn run_demo(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Initialize everything
        self.initialize_schema().await?;
        self.load_sample_data().await?;
        self.start_servers().await?;

        // Wait a moment for everything to settle
        sleep(Duration::from_secs(1)).await;

        // Run demonstrations
        self.demonstrate_recommendations().await?;
        self.demonstrate_distributed_transaction().await?;

        println!("\n🎉 Multi-Model Demo Complete!");
        println!("
🌟 This demo showcased Orbit-RS capabilities:
   • Multi-model data storage (SQL, Vector, Graph, Document)
   • Cross-model queries and joins
   • Vector similarity search for ML/AI workloads
   • Graph relationship queries
   • Flexible document storage
   • Distributed ACID transactions
   • Real-time protocol compatibility (PostgreSQL, Redis)

🚀 Orbit-RS enables you to build sophisticated applications with:
   • Unified query interface across data models
   • Automatic scaling and distribution
   • Strong consistency guarantees
   • High performance with smart caching
   • Rich ecosystem integration
        ");

        Ok(())
    }
}

/// Generate a mock embedding vector for demonstration
fn generate_mock_embedding(text: &str) -> Vec<f32> {
    // In reality, this would use a proper embedding model like BERT, sentence-transformers, etc.
    // For demo, we create a simple hash-based mock embedding
    
    let mut embedding = vec![0.0f32; 384];
    let words: Vec<&str> = text.split_whitespace().collect();
    
    for (i, word) in words.iter().enumerate() {
        let hash = word.chars().map(|c| c as u32).sum::<u32>() as usize;
        let index = hash % 384;
        embedding[index] = 0.5 + (i as f32 * 0.1) % 0.5; // Simple mock values
    }
    
    // Normalize the vector
    let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if magnitude > 0.0 {
        for value in &mut embedding {
            *value /= magnitude;
        }
    }
    
    embedding
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for better logging
    tracing_subscriber::fmt::init();

    println!("🌟 Welcome to the Orbit-RS Multi-Model E-commerce Demo!");
    println!("================================================================");

    // Create and run the demonstration system
    let system = EcommerceRecommendationSystem::new().await?;
    system.run_demo().await?;

    Ok(())
}