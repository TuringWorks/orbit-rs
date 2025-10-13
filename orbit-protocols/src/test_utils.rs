//! Test utilities and fixtures for Orbit-RS protocols
//!
//! This module provides comprehensive testing utilities including:
//! - Mock data generators for realistic testing
//! - Test fixtures for common scenarios  
//! - Helper functions for test setup and teardown
//! - Performance testing utilities
//! - Integration test helpers

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
pub mod test_fixtures {
    use crate::error::ProtocolError;
    use crate::postgres_wire::sql::{ExecutionResult, SqlExecutor};
    use crate::vector_store::{SimilarityMetric, Vector};
    use rand::prelude::*;
    use std::collections::HashMap;
    use tokio::time::{Duration, Instant};

    /// Test fixture for SQL operations
    pub struct SqlTestFixture {
        pub executor: SqlExecutor,
    }

    impl SqlTestFixture {
        /// Create a new SQL test fixture with memory storage
        pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
            let executor = SqlExecutor::new().await?;
            Ok(Self { executor })
        }

        /// Create SQL fixture with sample e-commerce schema
        pub async fn with_ecommerce_schema() -> Result<Self, Box<dyn std::error::Error>> {
            let mut fixture = Self::new().await?;
            fixture.setup_ecommerce_schema().await?;
            Ok(fixture)
        }

        /// Setup typical e-commerce database schema
        pub async fn setup_ecommerce_schema(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            let schemas = vec![
                "CREATE TABLE customers (
                    id SERIAL PRIMARY KEY,
                    name VARCHAR(255) NOT NULL,
                    email VARCHAR(255) UNIQUE NOT NULL,
                    registration_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )",
                "CREATE TABLE products (
                    id SERIAL PRIMARY KEY,
                    name VARCHAR(255) NOT NULL,
                    description TEXT,
                    price DECIMAL(10,2) NOT NULL,
                    category VARCHAR(100),
                    stock_quantity INTEGER DEFAULT 0
                )",
                "CREATE TABLE orders (
                    id SERIAL PRIMARY KEY,
                    customer_id INTEGER REFERENCES customers(id),
                    order_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    total_amount DECIMAL(10,2),
                    status VARCHAR(50) DEFAULT 'pending'
                )",
                "CREATE TABLE order_items (
                    id SERIAL PRIMARY KEY,
                    order_id INTEGER REFERENCES orders(id),
                    product_id INTEGER REFERENCES products(id),
                    quantity INTEGER NOT NULL,
                    unit_price DECIMAL(10,2)
                )",
                "CREATE INDEX idx_orders_customer ON orders(customer_id)",
                "CREATE INDEX idx_order_items_order ON order_items(order_id)",
                "CREATE INDEX idx_products_category ON products(category)",
            ];

            for schema in schemas {
                self.executor.execute(schema).await?;
            }

            Ok(())
        }

        /// Load sample data for testing
        pub async fn load_sample_data(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            let mut sample_data = SampleDataGenerator::new();

            // Insert customers
            for customer in sample_data.generate_customers(10) {
                let sql = format!(
                    "INSERT INTO customers (name, email) VALUES ('{}', '{}')",
                    customer.name, customer.email
                );
                self.executor.execute(&sql).await?;
            }

            // Insert products
            for product in sample_data.generate_products(20) {
                let sql = format!(
                    "INSERT INTO products (name, description, price, category, stock_quantity) 
                     VALUES ('{}', '{}', {}, '{}', {})",
                    product.name,
                    product.description,
                    product.price,
                    product.category,
                    product.stock
                );
                self.executor.execute(&sql).await?;
            }

            Ok(())
        }

        /// Execute SQL and expect success
        pub async fn execute_ok(
            &self,
            sql: &str,
        ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
            Ok(self.executor.execute(sql).await?)
        }

        /// Execute SQL and expect failure
        pub async fn execute_expect_error(&self, sql: &str) -> ProtocolError {
            self.executor.execute(sql).await.unwrap_err()
        }
    }

    /// Test fixture for vector operations - simplified for demo
    pub struct VectorTestFixture {
        pub vectors: Vec<Vector>,
        pub dimension: usize,
    }

    impl VectorTestFixture {
        /// Create a new vector test fixture
        pub fn new(dimension: usize, _metric: SimilarityMetric) -> Self {
            Self {
                vectors: Vec::new(),
                dimension,
            }
        }

        /// Create fixture with sample vectors
        pub fn with_sample_vectors(
            dimension: usize,
            metric: SimilarityMetric,
            count: usize,
        ) -> Result<Self, Box<dyn std::error::Error>> {
            let mut fixture = Self::new(dimension, metric);
            let mut generator = SampleDataGenerator::new();

            for i in 0..count {
                let vector = generator.generate_random_vector(dimension, &format!("vec_{}", i));
                fixture.vectors.push(vector);
            }

            Ok(fixture)
        }

        /// Add a vector with known properties for testing
        pub fn add_test_vector(
            &mut self,
            id: &str,
            data: Vec<f32>,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let vector = Vector::new(id.to_string(), data);
            self.vectors.push(vector);
            Ok(())
        }

        /// Simple search implementation for testing
        pub fn search(
            &self,
            _query: &[f32],
            limit: usize,
        ) -> Result<Vec<MockSearchResult>, Box<dyn std::error::Error>> {
            let mut results = Vec::new();
            for (i, vector) in self.vectors.iter().enumerate().take(limit) {
                results.push(MockSearchResult {
                    vector: vector.clone(),
                    score: 0.8 - (i as f32 * 0.1), // Mock decreasing scores
                });
            }
            Ok(results)
        }
    }

    /// Mock search result for testing
    #[derive(Debug, Clone)]
    pub struct MockSearchResult {
        pub vector: Vector,
        pub score: f32,
    }

    /// Mock data generator for realistic test scenarios
    pub struct SampleDataGenerator {
        rng: StdRng,
    }

    impl Default for SampleDataGenerator {
        fn default() -> Self {
            Self::new()
        }
    }

    impl SampleDataGenerator {
        pub fn new() -> Self {
            Self {
                rng: StdRng::seed_from_u64(42), // Fixed seed for reproducible tests
            }
        }

        /// Generate realistic customer data
        pub fn generate_customers(&mut self, count: usize) -> Vec<CustomerData> {
            let first_names = [
                "Alice", "Bob", "Charlie", "Diana", "Eve", "Frank", "Grace", "Henry", "Ivy",
                "Jack", "Karen", "Liam", "Mia", "Noah", "Olivia", "Paul",
            ];
            let last_names = [
                "Smith",
                "Johnson",
                "Williams",
                "Brown",
                "Jones",
                "Garcia",
                "Miller",
                "Davis",
                "Rodriguez",
                "Martinez",
                "Hernandez",
                "Lopez",
                "Gonzalez",
            ];

            (0..count)
                .map(|i| {
                    let first = first_names[self.rng.gen_range(0..first_names.len())];
                    let last = last_names[self.rng.gen_range(0..last_names.len())];
                    CustomerData {
                        id: i as u32 + 1,
                        name: format!("{} {}", first, last),
                        email: format!("{}{}@example.com", first.to_lowercase(), i),
                    }
                })
                .collect()
        }

        /// Generate realistic product data
        pub fn generate_products(&mut self, count: usize) -> Vec<ProductData> {
            let product_templates = [
                ("Wireless Headphones", "Electronics", 50.0, 200.0),
                ("Running Shoes", "Sports", 60.0, 180.0),
                ("Coffee Maker", "Home", 30.0, 150.0),
                ("Smartphone", "Electronics", 200.0, 1000.0),
                ("Yoga Mat", "Sports", 15.0, 80.0),
                ("Laptop", "Electronics", 500.0, 2000.0),
                ("Kitchen Knife", "Home", 20.0, 100.0),
                ("Fitness Tracker", "Electronics", 50.0, 300.0),
                ("Books", "Education", 5.0, 50.0),
                ("Office Chair", "Furniture", 100.0, 500.0),
            ];

            (0..count)
                .map(|i| {
                    let template = &product_templates[i % product_templates.len()];
                    let price = self.rng.gen_range(template.2..=template.3);
                    ProductData {
                        id: i as u32 + 1,
                        name: if i >= product_templates.len() {
                            format!("{} v{}", template.0, i / product_templates.len() + 1)
                        } else {
                            template.0.to_string()
                        },
                        description: format!(
                            "High-quality {} perfect for everyday use",
                            template.0.to_lowercase()
                        ),
                        price: (price * 100.0_f64).round() / 100.0, // Round to 2 decimal places
                        category: template.1.to_string(),
                        stock: self.rng.gen_range(10..=100),
                    }
                })
                .collect()
        }

        /// Generate random vector with realistic properties
        pub fn generate_random_vector(&mut self, dimension: usize, id: &str) -> Vector {
            let data: Vec<f32> = (0..dimension)
                .map(|_| self.rng.gen_range(-1.0..1.0))
                .collect();

            // Normalize the vector
            let magnitude: f32 = data.iter().map(|x| x * x).sum::<f32>().sqrt();
            let normalized_data = if magnitude > 0.0 {
                data.into_iter().map(|x| x / magnitude).collect()
            } else {
                data
            };

            Vector::new(id.to_string(), normalized_data)
        }

        /// Generate vectors with known similarity relationships
        pub fn generate_similar_vectors(
            &mut self,
            base_vector: &[f32],
            similarity: f32,
            count: usize,
        ) -> Vec<Vector> {
            let mut vectors = Vec::new();
            let _dimension = base_vector.len();

            for i in 0..count {
                // Create a vector that's similar to base_vector
                let noise_amplitude = (1.0 - similarity).sqrt();
                let mut similar_data: Vec<f32> = base_vector
                    .iter()
                    .map(|&x| x + self.rng.gen_range(-noise_amplitude..noise_amplitude))
                    .collect();

                // Normalize
                let magnitude: f32 = similar_data.iter().map(|x| x * x).sum::<f32>().sqrt();
                if magnitude > 0.0 {
                    for val in &mut similar_data {
                        *val /= magnitude;
                    }
                }

                vectors.push(Vector::new(format!("similar_{}", i), similar_data));
            }

            vectors
        }

        /// Generate test data for GraphRAG scenarios
        pub fn generate_graph_data(&mut self) -> GraphTestData {
            GraphTestData {
                entities: vec![
                    EntityData {
                        id: "person:alice".to_string(),
                        entity_type: "Person".to_string(),
                        properties: HashMap::from([(
                            "name".to_string(),
                            "Alice Johnson".to_string(),
                        )]),
                    },
                    EntityData {
                        id: "person:bob".to_string(),
                        entity_type: "Person".to_string(),
                        properties: HashMap::from([("name".to_string(), "Bob Smith".to_string())]),
                    },
                    EntityData {
                        id: "company:acme".to_string(),
                        entity_type: "Company".to_string(),
                        properties: HashMap::from([("name".to_string(), "ACME Corp".to_string())]),
                    },
                ],
                relationships: vec![
                    RelationshipData {
                        from: "person:alice".to_string(),
                        to: "company:acme".to_string(),
                        rel_type: "WORKS_FOR".to_string(),
                        properties: HashMap::new(),
                    },
                    RelationshipData {
                        from: "person:bob".to_string(),
                        to: "company:acme".to_string(),
                        rel_type: "WORKS_FOR".to_string(),
                        properties: HashMap::new(),
                    },
                    RelationshipData {
                        from: "person:alice".to_string(),
                        to: "person:bob".to_string(),
                        rel_type: "KNOWS".to_string(),
                        properties: HashMap::new(),
                    },
                ],
            }
        }
    }

    /// Performance testing utilities
    pub struct PerformanceTestSuite {
        pub results: Vec<PerformanceResult>,
    }

    impl Default for PerformanceTestSuite {
        fn default() -> Self {
            Self::new()
        }
    }

    impl PerformanceTestSuite {
        pub fn new() -> Self {
            Self {
                results: Vec::new(),
            }
        }

        /// Time an async operation
        pub async fn time_async<F, T, E>(&mut self, name: &str, operation: F) -> Result<T, E>
        where
            F: std::future::Future<Output = Result<T, E>>,
        {
            let start = Instant::now();
            let result = operation.await;
            let duration = start.elapsed();

            self.results.push(PerformanceResult {
                test_name: name.to_string(),
                duration,
                success: result.is_ok(),
            });

            result
        }

        /// Generate performance report
        pub fn generate_report(&self) -> String {
            let mut report = String::new();
            report.push_str("=== Performance Test Results ===\n");

            for result in &self.results {
                let status = if result.success {
                    "✅ PASS"
                } else {
                    "❌ FAIL"
                };
                report.push_str(&format!(
                    "{} {} - {:?}\n",
                    status, result.test_name, result.duration
                ));
            }

            let total_tests = self.results.len();
            let passed_tests = self.results.iter().filter(|r| r.success).count();
            let avg_duration = if total_tests > 0 {
                self.results.iter().map(|r| r.duration).sum::<Duration>() / total_tests as u32
            } else {
                Duration::from_secs(0)
            };

            report.push_str(&format!(
                "\nSummary: {}/{} tests passed\n",
                passed_tests, total_tests
            ));
            report.push_str(&format!("Average duration: {:?}\n", avg_duration));

            report
        }
    }

    // Data structures for test fixtures
    #[derive(Debug, Clone)]
    pub struct CustomerData {
        pub id: u32,
        pub name: String,
        pub email: String,
    }

    #[derive(Debug, Clone)]
    pub struct ProductData {
        pub id: u32,
        pub name: String,
        pub description: String,
        pub price: f64,
        pub category: String,
        pub stock: i32,
    }

    #[derive(Debug, Clone)]
    pub struct EntityData {
        pub id: String,
        pub entity_type: String,
        pub properties: HashMap<String, String>,
    }

    #[derive(Debug, Clone)]
    pub struct RelationshipData {
        pub from: String,
        pub to: String,
        pub rel_type: String,
        pub properties: HashMap<String, String>,
    }

    #[derive(Debug, Clone)]
    pub struct GraphTestData {
        pub entities: Vec<EntityData>,
        pub relationships: Vec<RelationshipData>,
    }

    #[derive(Debug, Clone)]
    pub struct PerformanceResult {
        pub test_name: String,
        pub duration: Duration,
        pub success: bool,
    }

    /// Integration test helpers
    pub struct IntegrationTestHelper {
        pub postgres_port: u16,
        pub redis_port: u16,
    }

    impl IntegrationTestHelper {
        /// Find available ports for test servers
        pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
            use tokio::net::TcpListener;

            // Find available ports
            let postgres_listener = TcpListener::bind("127.0.0.1:0").await?;
            let postgres_port = postgres_listener.local_addr()?.port();
            drop(postgres_listener);

            let redis_listener = TcpListener::bind("127.0.0.1:0").await?;
            let redis_port = redis_listener.local_addr()?.port();
            drop(redis_listener);

            Ok(Self {
                postgres_port,
                redis_port,
            })
        }

        /// Create database connection string for testing
        pub fn postgres_connection_string(&self) -> String {
            format!(
                "host=localhost port={} user=test dbname=test",
                self.postgres_port
            )
        }

        /// Create Redis connection string for testing
        pub fn redis_connection_string(&self) -> String {
            format!("redis://127.0.0.1:{}", self.redis_port)
        }

        /// Wait for servers to be ready
        pub async fn wait_for_servers(
            &self,
            timeout: Duration,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let start = Instant::now();

            while start.elapsed() < timeout {
                // Try to connect to both servers
                if self.check_postgres_ready().await && self.check_redis_ready().await {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            Err("Servers failed to start within timeout".into())
        }

        /// Check if PostgreSQL server is ready
        async fn check_postgres_ready(&self) -> bool {
            use tokio::net::TcpStream;
            TcpStream::connect(format!("127.0.0.1:{}", self.postgres_port))
                .await
                .is_ok()
        }

        /// Check if Redis server is ready
        async fn check_redis_ready(&self) -> bool {
            use tokio::net::TcpStream;
            TcpStream::connect(format!("127.0.0.1:{}", self.redis_port))
                .await
                .is_ok()
        }
    }

    /// Assertion helpers for testing
    pub mod assertions {
        use super::*;

        /// Assert that SQL result has expected number of rows
        pub fn assert_row_count(result: &ExecutionResult, expected: usize) {
            match result {
                ExecutionResult::Select { rows, .. } => {
                    assert_eq!(
                        rows.len(),
                        expected,
                        "Expected {} rows, got {}",
                        expected,
                        rows.len()
                    );
                }
                _ => panic!("Expected Select result for row count assertion"),
            }
        }

        /// Assert that SQL operation affected expected number of rows
        pub fn assert_affected_rows(result: &ExecutionResult, expected: usize) {
            let actual = match result {
                ExecutionResult::Insert { count } => *count,
                ExecutionResult::Update { count } => *count,
                ExecutionResult::Delete { count } => *count,
                _ => panic!("Expected Insert/Update/Delete result for affected rows assertion"),
            };
            assert_eq!(
                actual, expected,
                "Expected {} affected rows, got {}",
                expected, actual
            );
        }

        /// Assert that vector search returns results within similarity threshold
        pub fn assert_similarity_threshold(results: &[MockSearchResult], min_similarity: f32) {
            for result in results {
                assert!(
                    result.score >= min_similarity,
                    "Result with score {} below threshold {}",
                    result.score,
                    min_similarity
                );
            }
        }

        /// Assert that error contains expected message
        pub fn assert_error_contains(error: &ProtocolError, expected_message: &str) {
            let error_string = error.to_string();
            assert!(
                error_string.contains(expected_message),
                "Error '{}' does not contain '{}'",
                error_string,
                expected_message
            );
        }
    }
}

/// Export test utilities for use in integration tests
pub use test_fixtures::*;
