# REST API Natural Language Query Endpoints

## Overview

The Orbit-RS REST API now includes natural language query endpoints that allow clients to query the database using natural language instead of SQL.

## Endpoints

### 1. Execute Natural Language Query

**Endpoint:** `POST /api/v1/query/natural-language`

**Description:** Converts natural language to SQL, executes it, and returns formatted results.

**Request Body:**
```json
{
  "query": "Show me all users from California",
  "limit": 100,
  "max_preview_rows": 10,
  "execute": true
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "sql": "SELECT * FROM users WHERE state = $1",
    "parameters": ["California"],
    "query_type": "Read",
    "complexity": "Low",
    "optimization_hints": ["UseIndex"],
    "results": {
      "summary": "Query returned 42 rows with 3 columns: id, name, state",
      "data_preview": [
        {"id": 1, "name": "Alice", "state": "California"},
        {"id": 2, "name": "Bob", "state": "California"}
      ],
      "total_rows": 42,
      "full_result_available": false,
      "continuation_token": "offset_10",
      "statistics": {
        "total_rows": 42,
        "column_stats": {...}
      },
      "visualization_hints": [
        {
          "viz_type": "BarChart",
          "columns": ["state", "count"],
          "description": "Bar chart showing count by state"
        }
      ]
    },
    "metadata": {
      "processing_time_ms": 45,
      "nlp_time_ms": 10,
      "execution_time_ms": 30,
      "result_processing_time_ms": 5,
      "confidence": 0.85
    }
  }
}
```

### 2. Generate SQL Only

**Endpoint:** `POST /api/v1/query/generate-sql`

**Description:** Converts natural language to SQL without executing it.

**Request Body:**
```json
{
  "query": "Show me all users from California",
  "execute": false
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "sql": "SELECT * FROM users WHERE state = $1",
    "parameters": ["California"],
    "query_type": "Read",
    "complexity": "Low",
    "optimization_hints": ["UseIndex"],
    "results": null,
    "metadata": {
      "processing_time_ms": 10,
      "nlp_time_ms": 10,
      "confidence": 0.85
    }
  }
}
```

## Usage Examples

### Using curl

```bash
# Execute natural language query
curl -X POST http://localhost:8080/api/v1/query/natural-language \
  -H "Content-Type: application/json" \
  -d '{
    "query": "Show me the top 10 products by revenue",
    "limit": 10,
    "execute": true
  }'

# Generate SQL only
curl -X POST http://localhost:8080/api/v1/query/generate-sql \
  -H "Content-Type: application/json" \
  -d '{
    "query": "Find all users where age is greater than 25"
  }'
```

### Using JavaScript/TypeScript

```typescript
async function queryNaturalLanguage(query: string) {
  const response = await fetch('http://localhost:8080/api/v1/query/natural-language', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      query: query,
      limit: 100,
      max_preview_rows: 10,
      execute: true,
    }),
  });

  const result = await response.json();
  return result.data;
}

// Usage
const result = await queryNaturalLanguage("Show me all users from California");
console.log(result.sql); // Generated SQL
console.log(result.results?.summary); // Query summary
console.log(result.results?.data_preview); // Data preview
```

### Using Python

```python
import requests

def query_natural_language(query: str, execute: bool = True):
    url = "http://localhost:8080/api/v1/query/natural-language"
    payload = {
        "query": query,
        "limit": 100,
        "max_preview_rows": 10,
        "execute": execute
    }
    
    response = requests.post(url, json=payload)
    response.raise_for_status()
    return response.json()["data"]

# Usage
result = query_natural_language("Show me all users from California")
print(f"SQL: {result['sql']}")
print(f"Summary: {result['results']['summary']}")
print(f"Rows: {result['results']['data_preview']}")
```

## Request Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `query` | string | Yes | - | Natural language query |
| `limit` | integer | No | 100 | Maximum number of results |
| `max_preview_rows` | integer | No | 10 | Maximum preview rows |
| `execute` | boolean | No | true | Whether to execute the query |

## Response Fields

### Query Response

- `sql`: Generated SQL query
- `parameters`: Query parameters (for parameterized queries)
- `query_type`: Type of query (Read, Write, Analysis)
- `complexity`: Estimated complexity (Low, Medium, High)
- `optimization_hints`: Suggested optimizations
- `results`: Query results (if executed)
- `metadata`: Processing metadata

### Results (if executed)

- `summary`: Human-readable summary
- `data_preview`: First N rows of data
- `total_rows`: Total number of rows
- `full_result_available`: Whether all results are in preview
- `continuation_token`: Token for pagination
- `statistics`: Statistical summary
- `visualization_hints`: Suggested visualizations

## Error Responses

### 400 Bad Request

```json
{
  "error": true,
  "code": "NLP_PROCESSING_ERROR",
  "message": "Failed to process natural language query: ..."
}
```

### 503 Service Unavailable

```json
{
  "error": true,
  "code": "MCP_SERVER_NOT_AVAILABLE",
  "message": "Natural language query processing is not available. MCP server not configured."
}
```

## Setup

### 1. Create REST API Server with MCP

```rust
use orbit_protocols::rest::{RestApiServer, RestApiConfig};
use orbit_protocols::mcp::{McpServer, McpConfig, McpCapabilities};
use orbit_protocols::mcp::integration::OrbitMcpIntegration;
use orbit_protocols::postgres_wire::query_engine::QueryEngine;
use std::sync::Arc;

// Create query engine
let query_engine = Arc::new(QueryEngine::new());

// Create MCP integration
let integration = Arc::new(OrbitMcpIntegration::new(query_engine));

// Create MCP server
let mcp_server = Arc::new(McpServer::with_orbit_integration(
    McpConfig::default(),
    McpCapabilities::default(),
    integration,
));

// Create REST API server with MCP
let rest_config = RestApiConfig::default();
let server = RestApiServer::with_mcp(
    orbit_client,
    rest_config,
    mcp_server,
);

server.run().await?;
```

### 2. Start Server

```bash
cargo run --example rest-api-nl-query
```

## OpenAPI Documentation

The OpenAPI specification is available at:

```
http://localhost:8080/openapi.json
```

This includes full documentation for the natural language query endpoints with request/response schemas.

## Supported Query Types

### Data Retrieval
- "Show me all users"
- "What are the top 10 products by revenue?"
- "Find users where age is greater than 25"
- "Get the average order value by month"

### Data Analysis
- "Analyze the distribution of customer ages"
- "What's the correlation between price and sales?"
- "Show me trends in user sign-ups"

### Data Modification
- "Add a new user with email john@example.com"
- "Update all products in category 'electronics'"
- "Delete old log entries from before last month"

## Performance

- **NLP Processing**: < 10ms (rule-based)
- **SQL Generation**: < 5ms
- **Query Execution**: Depends on Orbit-RS (typically < 100ms)
- **Result Processing**: < 20ms for 1000 rows

## Security

- All queries use parameterized SQL (SQL injection protection)
- Input validation on natural language queries
- Rate limiting (configured via REST API server)
- Authentication (if configured)

## Integration with Existing REST API

The natural language query endpoints are fully integrated with the existing REST API:

- Same authentication/authorization
- Same error handling
- Same OpenAPI documentation
- Same monitoring and logging

## Examples

See `examples/rest-api-nl-query.rs` for a complete working example.

