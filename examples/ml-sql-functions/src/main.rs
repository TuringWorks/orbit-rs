//! ML SQL Functions Example
//!
//! Demonstrates how to use machine learning functions directly in SQL queries
//! within Orbit-RS for in-database analytics and modeling.

use orbit_protocols::ml::{
    MLFunctionRegistry, ModelStorage, ModelMetadata, MLResult,
    functions::{LinearRegressionFunction, CorrelationFunction, ZScoreFunction},
};
use orbit_protocols::ml::sql_integration::{SqlMlBridge, SqlMLFunctionRegistry};
use std::collections::HashMap;

/// Example implementation of model storage
pub struct ExampleModelStorage;

#[async_trait::async_trait]
impl ModelStorage for ExampleModelStorage {
    async fn save_model(&self, _metadata: &ModelMetadata, _model_data: &[u8]) -> MLResult<()> {
        println!("Model saved (example implementation)");
        Ok(())
    }
    
    async fn load_model(&self, model_name: &str) -> MLResult<(ModelMetadata, Vec<u8>)> {
        println!("Loading model: {}", model_name);
        // Return dummy metadata for example
        let metadata = ModelMetadata {
            name: model_name.to_string(),
            algorithm: "linear_regression".to_string(),
            version: 1,
            created_at: 0,
            updated_at: 0,
            parameters: HashMap::new(),
            input_schema: Vec::new(),
            output_schema: orbit_protocols::ml::OutputSpec {
                output_type: orbit_protocols::ml::OutputType::Numeric,
                schema: HashMap::new(),
            },
            metrics: HashMap::new(),
            storage_path: format!("/models/{}", model_name),
        };
        Ok((metadata, vec![1, 2, 3, 4])) // Dummy model data
    }
    
    async fn list_models(&self) -> MLResult<Vec<ModelMetadata>> {
        Ok(Vec::new())
    }
    
    async fn delete_model(&self, model_name: &str) -> MLResult<()> {
        println!("Deleting model: {}", model_name);
        Ok(())
    }
    
    async fn get_metadata(&self, model_name: &str) -> MLResult<ModelMetadata> {
        let (metadata, _) = self.load_model(model_name).await?;
        Ok(metadata)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🤖 Orbit-RS ML SQL Functions Example");
    println!("======================================\n");

    // Initialize ML function registry
    let storage = Box::new(ExampleModelStorage);
    let mut ml_registry = MLFunctionRegistry::new(storage);

    // Register ML functions
    ml_registry.register_function(
        "ML_LINEAR_REGRESSION".to_string(),
        Box::new(LinearRegressionFunction),
    );
    
    ml_registry.register_function(
        "ML_CORRELATION".to_string(),
        Box::new(CorrelationFunction),
    );
    
    ml_registry.register_function(
        "ML_ZSCORE".to_string(),
        Box::new(ZScoreFunction),
    );

    // Create SQL-ML bridge
    let sql_bridge = SqlMlBridge::new(ml_registry);

    // Example 1: Correlation Analysis
    println!("📊 Example 1: Correlation Analysis");
    println!("SQL: SELECT ML_CORRELATION(ARRAY[1, 2, 3, 4, 5], ARRAY[2, 4, 6, 8, 10])");
    
    let x = orbit_protocols::ml::MLValue::Vector(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    let y = orbit_protocols::ml::MLValue::Vector(vec![2.0, 4.0, 6.0, 8.0, 10.0]);
    
    let correlation_func = CorrelationFunction;
    let result = correlation_func.execute(vec![x, y]).await?;
    
    match result {
        orbit_protocols::ml::MLValue::Float(corr) => {
            println!("✅ Correlation coefficient: {:.4}", corr);
            println!("   This shows a perfect positive correlation (1.0)\n");
        }
        _ => println!("❌ Unexpected result type"),
    }

    // Example 2: Z-Score Normalization
    println!("📈 Example 2: Z-Score Normalization");
    println!("SQL: SELECT ML_ZSCORE(ARRAY[10, 20, 30, 40, 50])");
    
    let values = orbit_protocols::ml::MLValue::Vector(vec![10.0, 20.0, 30.0, 40.0, 50.0]);
    let zscore_func = ZScoreFunction;
    let result = zscore_func.execute(vec![values]).await?;
    
    match result {
        orbit_protocols::ml::MLValue::Vector(z_scores) => {
            println!("✅ Z-scores: {:?}", z_scores.iter().map(|x| format!("{:.2}", x)).collect::<Vec<_>>());
            println!("   Mean should be ~0, std should be ~1\n");
        }
        _ => println!("❌ Unexpected result type"),
    }

    // Example 3: Linear Regression
    println!("🔮 Example 3: Linear Regression");
    println!("SQL: SELECT ML_LINEAR_REGRESSION(features, target) FROM sales_data");
    println!("     WHERE category = 'electronics'");
    
    // Sample data: y = 2x + 1
    let features = orbit_protocols::ml::MLValue::Array(vec![
        orbit_protocols::ml::MLValue::Vector(vec![1.0]),
        orbit_protocols::ml::MLValue::Vector(vec![2.0]),
        orbit_protocols::ml::MLValue::Vector(vec![3.0]),
        orbit_protocols::ml::MLValue::Vector(vec![4.0]),
        orbit_protocols::ml::MLValue::Vector(vec![5.0]),
    ]);
    let targets = orbit_protocols::ml::MLValue::Vector(vec![3.0, 5.0, 7.0, 9.0, 11.0]);
    
    let regression_func = LinearRegressionFunction;
    let result = regression_func.execute(vec![features, targets]).await?;
    
    match result {
        orbit_protocols::ml::MLValue::Vector(coeffs) => {
            println!("✅ Linear regression coefficients: {:?}", 
                coeffs.iter().map(|x| format!("{:.2}", x)).collect::<Vec<_>>());
            println!("   [intercept, slope] ≈ [1.00, 2.00] for y = 2x + 1\n");
        }
        _ => println!("❌ Unexpected result type"),
    }

    // Show SQL ML Function Registry
    println!("📚 Available SQL ML Functions:");
    let sql_registry = SqlMLFunctionRegistry::with_builtin_functions();
    
    for func in sql_registry.list_functions() {
        println!("   • {} - {}", func.signature.name, func.signature.description);
        if !func.examples.is_empty() {
            println!("     Example: {}", func.examples[0]);
        }
    }

    println!("\n🎯 Real-World SQL Usage Examples:");
    println!("════════════════════════════════════");
    
    // Real-world examples
    let examples = vec![
        (
            "Customer Analytics",
            "SELECT customer_id, ML_KMEANS(ARRAY[recency, frequency, monetary], 3) AS segment FROM rfm_data"
        ),
        (
            "Fraud Detection", 
            "SELECT *, ML_PREDICT('fraud_model', ARRAY[amount, merchant, hour]) AS risk_score FROM transactions"
        ),
        (
            "A/B Test Analysis",
            "SELECT ML_CORRELATION(treatment, conversion_rate) FROM experiment_results"
        ),
        (
            "Feature Engineering",
            "SELECT *, ML_ZSCORE(price) AS price_normalized FROM products"
        ),
        (
            "Predictive Analytics",
            "SELECT date, sales, ML_FORECAST(sales OVER (ORDER BY date), 30) AS predicted FROM daily_sales"
        ),
    ];

    for (title, sql) in examples {
        println!("\n📝 {}: \n   {}", title, sql);
    }

    println!("\n🚀 Benefits of In-Database ML:");
    println!("• Zero data movement - ML runs where data lives");
    println!("• Familiar SQL interface - no Python/R required");
    println!("• Distributed processing - scales with your data");
    println!("• Real-time inference - ML in live queries");
    println!("• Integrated workflow - no ETL pipelines needed");

    Ok(())
}