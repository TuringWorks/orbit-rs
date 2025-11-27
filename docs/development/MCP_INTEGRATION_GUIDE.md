# MCP Server Integration Guide

## Overview

This guide explains how to integrate and use the MCP (Model Context Protocol) Server with Orbit-RS for natural language query processing.

## Architecture

```
Natural Language Query
        ↓
MCP Server (NLP Processing)
        ↓
SQL Generation
        ↓
Orbit-RS Integration Layer
        ↓
PostgreSQL Wire Protocol / Query Engine
        ↓
Query Results
        ↓
Result Processing & Formatting
        ↓
LLM Response
```

## Basic Usage

### 1. Create MCP Server with Orbit-RS Integration

```rust
use orbit_protocols::mcp::{
    integration::OrbitMcpIntegration,
    server::McpServer,
    McpConfig, McpCapabilities,
};
use orbit_protocols::postgres_wire::query_engine::QueryEngine;
use std::sync::Arc;

// Create Orbit-RS query engine
let query_engine = Arc::new(QueryEngine::new(/* ... */));

// Create integration layer
let integration = Arc::new(OrbitMcpIntegration::new(query_engine));

// Create MCP server with integration
let config = McpConfig::default();
let capabilities = McpCapabilities::default();
let server = McpServer::with_orbit_integration(config, capabilities, integration);
```

### 2. Execute Natural Language Queries

```rust
// Simple: Generate SQL only
let generated_query = server
    .process_natural_language_query("Show me all users from California")
    .await?;

println!("SQL: {}", generated_query.sql);
println!("Parameters: {:?}", generated_query.parameters);

// Full pipeline: NLP → SQL → Execute → Process Results
let processed_result = server
    .execute_natural_language_query("Show me all users from California")
    .await?;

println!("Summary: {}", processed_result.summary);
println!("Rows: {}", processed_result.data_preview.len());
```

### 3. Schema Operations

```rust
// Get table schema
if let Some(ref integration) = server.orbit_integration {
    let schema = integration.get_table_schema("users").await?;
    if let Some(schema) = schema {
        println!("Table: {}", schema.name);
        for col in schema.columns {
            println!("  - {}: {}", col.name, col.data_type);
        }
    }
}

// List all tables
let tables = integration.list_tables().await?;
println!("Available tables: {:?}", tables);
```

## Integration with Schema Discovery

### Connect Schema Analyzer to Orbit-RS

```rust
use orbit_protocols::mcp::schema::SchemaAnalyzer;
use orbit_protocols::postgres_wire::persistent_storage::PersistentTableStorage;

// Create integration with storage
let storage: Arc<dyn PersistentTableStorage> = /* ... */;
let integration = Arc::new(
    OrbitMcpIntegration::with_storage(query_engine, storage)
);

// Update schema analyzer with discovered schemas
let schema_analyzer = SchemaAnalyzer::new();
for table_name in integration.list_tables().await? {
    if let Some(schema) = integration.get_table_schema(&table_name).await? {
        schema_analyzer.update_schema(schema);
    }
}
```

## MCP Tools Usage

The MCP server exposes several tools that can be used by LLM clients:

### 1. `query_data` - Natural Language Query

```json
{
  "query": "Show me all users from California",
  "limit": 100
}
```

### 2. `describe_schema` - Get Table Schema

```json
{
  "table_name": "users"
}
```

### 3. `analyze_data` - Statistical Analysis

```json
{
  "table_name": "users",
  "column_name": "age",
  "analysis_type": "distribution"
}
```

### 4. `list_tables` - List Available Tables

```json
{
  "pattern": "user*"
}
```

## Error Handling

The MCP server provides comprehensive error handling:

```rust
match server.execute_natural_language_query(query).await {
    Ok(result) => {
        // Process successful result
    }
    Err(McpError::SqlError(msg)) => {
        // SQL execution error
    }
    Err(McpError::InternalError(msg)) => {
        // Internal processing error
    }
    Err(e) => {
        // Other errors
    }
}
```

## Performance Considerations

1. **Schema Caching**: Schema information is cached for 5 minutes to reduce database queries
2. **Result Preview**: Large result sets are automatically previewed (first 100 rows by default)
3. **Query Optimization**: The SQL generator provides optimization hints for complex queries

## Next Steps

1. **Enhanced Schema Discovery**: Implement real-time schema updates
2. **Query Caching**: Cache frequently used natural language queries
3. **ML-Based NLP**: Integrate ML models for better intent classification
4. **Streaming Results**: Support streaming large result sets

## Examples

See `examples/mcp-nlp-example.rs` for a complete working example.

