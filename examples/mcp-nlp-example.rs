//! MCP Natural Language Query Example
//!
//! This example demonstrates the complete natural language to SQL pipeline:
//! 1. Natural language query processing
//! 2. SQL generation
//! 3. Result processing and formatting

use orbit_protocols::mcp::{
    nlp::NlpQueryProcessor,
    result_processor::{ProcessedResult, QueryResult, ResultProcessor},
    schema::SchemaAnalyzer,
    server::McpServer,
    sql_generator::SqlGenerator,
    McpCapabilities, McpConfig,
};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== MCP Natural Language Query Example ===\n");

    // Create MCP server with all components
    let config = McpConfig::default();
    let capabilities = McpCapabilities::default();
    let server = McpServer::new(config, capabilities);

    // Example natural language queries
    let queries = vec![
        "Show me all users from California",
        "What are the top 10 products by revenue?",
        "Find documents similar to machine learning",
        "Get the average order value by month",
        "Analyze the distribution of customer ages",
    ];

    for query in queries {
        println!("📝 Natural Language Query: \"{}\"", query);
        println!("{}", "=".repeat(60));

        // Step 1: Process natural language query
        match server.process_natural_language_query(query).await {
            Ok(generated_query) => {
                println!("\n✅ Generated SQL:");
                println!("   {}", generated_query.sql);
                println!("\n📊 Parameters: {:?}", generated_query.parameters);
                println!("\n🔍 Query Type: {:?}", generated_query.query_type);
                println!(
                    "⚡ Complexity: {:?}",
                    generated_query.estimated_complexity
                );
                println!(
                    "💡 Optimization Hints: {:?}",
                    generated_query.optimization_hints
                );

                // Step 2: Simulate query execution and result processing
                // (In real implementation, this would execute against Orbit-RS)
                let mock_results = create_mock_results(&generated_query);
                let processed = server
                    .result_processor
                    .process_results(&mock_results, 10);

                println!("\n📈 Processed Results:");
                print_processed_results(&processed);
            }
            Err(e) => {
                println!("\n❌ Error: {}", e);
            }
        }

        println!("\n{}\n", "=".repeat(60));
    }

    // Demonstrate schema operations
    println!("\n=== Schema Operations ===\n");
    let schema_analyzer = SchemaAnalyzer::new();
    println!("Available tables: {:?}", schema_analyzer.list_tables());

    Ok(())
}

/// Create mock query results for demonstration
fn create_mock_results(
    _generated_query: &orbit_protocols::mcp::sql_generator::GeneratedQuery,
) -> QueryResult {
    // Create sample data based on query type
    let mut rows = Vec::new();

    // Sample user data
    for i in 1..=5 {
        let mut row = HashMap::new();
        row.insert("id".to_string(), serde_json::Value::Number(i.into()));
        row.insert(
            "name".to_string(),
            serde_json::Value::String(format!("User {}", i)),
        );
        row.insert(
            "state".to_string(),
            serde_json::Value::String(if i % 2 == 0 {
                "California".to_string()
            } else {
                "New York".to_string()
            }),
        );
        row.insert(
            "age".to_string(),
            serde_json::Value::Number((20 + i * 5).into()),
        );
        rows.push(row);
    }

    QueryResult {
        columns: vec![
            "id".to_string(),
            "name".to_string(),
            "state".to_string(),
            "age".to_string(),
        ],
        rows,
        row_count: 5,
    }
}

/// Print processed results in a readable format
fn print_processed_results(processed: &ProcessedResult) {
    println!("   Summary: {}", processed.summary);
    println!("\n   Data Preview ({} rows):", processed.data_preview.len());
    for (idx, row) in processed.data_preview.iter().take(3).enumerate() {
        println!("     Row {}: {:?}", idx + 1, row);
    }

    println!("\n   Statistics:");
    println!("     Total Rows: {}", processed.statistics.total_rows);
    println!(
        "     Columns Analyzed: {}",
        processed.statistics.column_stats.len()
    );

    println!("\n   Visualization Hints:");
    for hint in &processed.visualization_hints {
        println!(
            "     - {:?}: {} (columns: {:?})",
            hint.viz_type, hint.description, hint.columns
        );
    }

    if let Some(token) = &processed.continuation_token {
        println!("\n   Continuation Token: {}", token);
    }
}

