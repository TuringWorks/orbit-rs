# Advanced MCP Server Example

## Features

### Core MCP Capabilities
- ✅ **Tool Management**: Register, list, and execute tools with JSON schema validation
- ✅ **Resource Management**: Static and dynamic resource serving with URI routing
- ✅ **Prompt Management**: Dynamic prompt generation with parameterization
- ✅ **Session Management**: Multi-session support with capability tracking
- ✅ **Logging**: Comprehensive structured logging with tracing

### Advanced Tools
- 🔢 **SQL Query Tool**: Execute SQL queries with result limiting and metadata
- 🧮 **Mathematical Calculator**: Perform mathematical calculations with precision control
- 🔍 **Vector Search Tool**: Similarity search with configurable thresholds and k-nearest neighbors
- 📊 **Data Analysis Tool**: Statistical analysis with descriptive statistics and data quality metrics

### Infrastructure
- 🌐 **HTTP REST API**: Full RESTful interface for all MCP operations
- 🗄️ **Database Integration**: Optional PostgreSQL connection support
- 📏 **Vector Operations**: Optional vector search capabilities
- 🔒 **CORS Support**: Cross-origin resource sharing for web clients
- 📝 **Request Tracing**: HTTP request/response logging and monitoring

## Quick Start

### Prerequisites

- Rust 1.70+ with Cargo
- Optional: PostgreSQL database (if using database features)

### Installation

1. Navigate to the advanced server directory:
```bash
cd examples/mcp-advanced-server
```

2. Install dependencies:
```bash
cargo build
```

3. Run the server with default configuration:
```bash
cargo run
```

4. Run with advanced options:
```bash
cargo run -- --port 9090 --host 0.0.0.0 --enable-vectors --database-url "postgresql://user:pass@localhost/db"
```

### Command Line Options

```bash
Advanced MCP server with real tool implementations

Usage: mcp-advanced-server [OPTIONS]

Options:
  -p, --port <PORT>              Port to bind the server to [default: 8080]
      --host <HOST>              Host to bind the server to [default: localhost]
      --database-url <DATABASE_URL>  Database URL (optional)
      --enable-vectors           Enable vector operations
      --log-level <LOG_LEVEL>    Log level [default: info]
  -h, --help                     Print help
```

## API Endpoints

### Server Information
- `GET /` - Server metadata and endpoint discovery

### MCP Protocol Operations
- `POST /mcp/initialize` - Initialize MCP session with capabilities
- `GET /mcp/tools` - List all available tools
- `POST /mcp/tools/:name` - Execute a specific tool
- `GET /mcp/resources` - List available resources
- `GET /mcp/resources/*uri` - Read specific resource content
- `GET /mcp/prompts` - List available prompts
- `POST /mcp/prompts/:name` - Get prompt with parameters

## Tools Documentation

### 1. SQL Query Tool (`sql_query`)

Execute SQL queries with result limiting and execution metadata.

**Parameters:**
- `query` (required): SQL query string to execute
- `limit` (optional): Maximum number of rows to return (default: 100)

**Example:**
```json
{
  "query": "SELECT * FROM users WHERE active = true",
  "limit": 50
}
```

**Response:**
```json
{
  "query": "SELECT * FROM users WHERE active = true",
  "limit": 50,
  "results": [
    {"id": 1, "name": "Alice", "email": "alice@example.com"},
    {"id": 2, "name": "Bob", "email": "bob@example.com"}
  ],
  "metadata": {
    "execution_time_ms": 45,
    "rows_affected": 2,
    "has_database": true
  }
}
```

### 2. Mathematical Calculator (`calculator`)

Perform mathematical calculations with configurable precision.

**Parameters:**
- `expression` (required): Mathematical expression to evaluate
- `precision` (optional): Number of decimal places (default: 10)

**Example:**
```json
{
  "expression": "123.456 * 789.012",
  "precision": 3
}
```

**Response:**
```json
{
  "expression": "123.456 * 789.012",
  "result": "97408.265",
  "precision": 3,
  "calculated_at": "2024-01-15T10:30:00Z"
}
```

### 3. Vector Search Tool (`vector_search`)

*Available only when `--enable-vectors` flag is used*

Perform vector similarity search operations with configurable parameters.

**Parameters:**
- `query_vector` (required): Array of numbers representing the query vector
- `k` (optional): Number of similar vectors to return (default: 10)
- `threshold` (optional): Minimum similarity threshold (default: 0.5)

**Example:**
```json
{
  "query_vector": [0.1, 0.2, 0.3, 0.4],
  "k": 5,
  "threshold": 0.7
}
```

**Response:**
```json
{
  "query_vector_dim": 4,
  "k": 5,
  "threshold": 0.7,
  "results": [
    {
      "id": "doc_1",
      "similarity": 0.95,
      "metadata": {"title": "Similar Document", "category": "research"},
      "content": "Document content..."
    }
  ],
  "search_metadata": {
    "total_vectors_searched": 10000,
    "search_time_ms": 23,
    "vectors_enabled": true
  }
}
```

### 4. Data Analysis Tool (`analyze_data`)

Perform comprehensive statistical analysis on numerical datasets.

**Parameters:**
- `data` (required): Array of numbers to analyze
- `analysis_types` (optional): Specific types of analysis to perform

**Example:**
```json
{
  "data": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
  "analysis_types": ["mean", "median", "std", "quartiles"]
}
```

**Response:**
```json
{
  "summary_statistics": {
    "count": 10,
    "mean": 5.5,
    "median": 5.5,
    "std_deviation": 3.0276503540974917,
    "variance": 9.166666666666666,
    "min": 1.0,
    "max": 10.0,
    "range": 9.0
  },
  "quartiles": {
    "q1": 3.0,
    "q2_median": 5.5,
    "q3": 8.0,
    "iqr": 5.0
  },
  "data_quality": {
    "has_outliers": false,
    "coefficient_of_variation": 0.5504,
    "skewness_indicator": "symmetric"
  },
  "analysis_metadata": {
    "analysis_performed_at": "2024-01-15T10:30:00Z",
    "data_points_analyzed": 10
  }
}
```

## Resources

### Static Resources
- `file:///tmp/data.json` - Sample JSON data for testing
- `memory://sessions` - Active MCP server sessions information

### Conditional Resources
- `database://main` - Main database connection (when database URL is provided)
- `vector://embeddings` - Vector embedding store (when vectors are enabled)

## Prompts

### 1. SQL Query Generator (`sql_query_generator`)

Generate SQL queries from natural language descriptions.

**Arguments:**
- `description` (required): Natural language description of the query
- `table_schema` (optional): Database table schema information

### 2. Data Analysis (`data_analysis`)

Perform comprehensive data analysis with configurable analysis types.

**Arguments:**
- `dataset` (required): Dataset to analyze
- `analysis_type` (optional): Type of analysis (descriptive, diagnostic, predictive)

## Usage Examples

### Using cURL

#### Initialize Session
```bash
curl -X POST http://localhost:8080/mcp/initialize \
  -H "Content-Type: application/json" \
  -d '{
    "protocol_version": "2024-11-05",
    "capabilities": {},
    "client_info": {"name": "test-client", "version": "1.0"}
  }'
```

#### Execute Calculator Tool
```bash
curl -X POST http://localhost:8080/mcp/tools/calculator \
  -H "Content-Type: application/json" \
  -d '{
    "params": {
      "expression": "25 * 4 + 10",
      "precision": 2
    }
  }'
```

#### Perform Data Analysis
```bash
curl -X POST http://localhost:8080/mcp/tools/analyze_data \
  -H "Content-Type: application/json" \
  -d '{
    "params": {
      "data": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    }
  }'
```

#### List Available Resources
```bash
curl http://localhost:8080/mcp/resources
```

#### Read Session Data
```bash
curl "http://localhost:8080/mcp/resources/memory://sessions"
```

### Using the Orbit MCP Client

You can use the basic MCP client example to interact with this server:

```bash

# In another terminal, run the basic client
cd ../mcp-basic-client
cargo run -- --server-url http://localhost:8080 list-tools
cargo run -- --server-url http://localhost:8080 execute-tool calculator '{"expression": "100 / 3", "precision": 5}'
```

## Development and Testing

### Running Tests
```bash
cargo test
```

### Development Mode with Auto-reload
```bash
cargo install cargo-watch
cargo watch -x run
```

### Enable Debug Logging
```bash
cargo run -- --log-level debug
```

### Testing with Different Configurations

#### Database Integration
```bash

# Start PostgreSQL (example with Docker)
docker run -d --name postgres \
  -e POSTGRES_DB=testdb \
  -e POSTGRES_USER=testuser \
  -e POSTGRES_PASSWORD=testpass \
  -p 5432:5432 \
  postgres:15

# Run server with database
cargo run -- --database-url "postgresql://testuser:testpass@localhost:5432/testdb"
```

#### Vector Operations
```bash

# Enable vector search capabilities
cargo run -- --enable-vectors --port 9090
```

#### Production Configuration
```bash

# Production-like setup
cargo run -- \
  --host 0.0.0.0 \
  --port 8080 \
  --enable-vectors \
  --database-url "postgresql://prod_user:prod_pass@db.example.com/prod_db" \
  --log-level warn
```

## Architecture

### Server Components
- **HTTP Router** (Axum): Handles REST API endpoints
- **MCP Server Core**: Manages protocol-level operations
- **Tool Registry**: Dynamic tool registration and execution
- **Session Manager**: Multi-session state management
- **Resource Handler**: Static and dynamic resource serving

### Tool Architecture
- **Schema Validation**: JSON Schema validation for tool parameters
- **Async Execution**: All tools support async operations
- **Error Handling**: Comprehensive error handling with structured responses
- **Extensibility**: Easy registration of new tools

### Security Considerations
- **Input Validation**: All tool parameters are validated against JSON schemas
- **Error Sanitization**: Error messages are sanitized before client transmission
- **Resource Isolation**: Resources are accessed through controlled URIs
- **Session Isolation**: Each session maintains separate state

## Extending the Server

### Adding New Tools

1. Create tool implementation function:
```rust
async fn my_custom_tool(params: Value) -> Result<CallToolResult> {
    // Tool implementation
    Ok(CallToolResult {
        content: vec![ToolResultContent::Text {
            text: "Tool result".to_string(),
        }],
        is_error: false,
    })
}
```

2. Register in `register_tools` function:
```rust
registry.register_tool(
    "my_tool",
    "Description of my tool",
    json!({
        "type": "object",
        "properties": {
            "param1": {"type": "string", "description": "Parameter description"}
        },
        "required": ["param1"]
    }),
    {
        move |params: Value| {
            Box::pin(async move {
                my_custom_tool(params).await
            })
        }
    },
);
```

### Adding New Resources

Extend the `list_resources_handler` and `read_resource_handler` functions to support new resource URIs and content types.

### Adding New Prompts

Extend the `list_prompts_handler` and `get_prompt_handler` functions to support new prompt templates and parameters.

## Performance Considerations

- **Connection Pooling**: Database connections should use connection pooling for production
- **Request Limits**: Consider implementing request rate limiting for production deployments
- **Memory Management**: Large datasets in data analysis tool should be streamed for memory efficiency
- **Async Operations**: All I/O operations are async for optimal performance

## Troubleshooting

### Common Issues

1. **Port Already in Use**
   ```bash
   Error: Address already in use (os error 48)
   ```
   Solution: Change port with `--port` flag or kill process using the port

2. **Database Connection Failed**
   ```bash
   Error: Failed to connect to database
   ```
   Solution: Verify database URL, credentials, and that database is running

3. **Tool Execution Timeout**
   ```bash
   Error: Tool execution timeout
   ```
   Solution: Check tool implementation for blocking operations and convert to async

### Debug Mode

Enable debug logging to see detailed request/response information:
```bash
RUST_LOG=debug cargo run
```

## Production Deployment

### Docker Deployment

Create a `Dockerfile`:
```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/mcp-advanced-server /usr/local/bin/
EXPOSE 8080
CMD ["mcp-advanced-server"]
```

### Environment Variables

The server can be configured via environment variables:
- `PORT`: Server port (default: 8080)
- `HOST`: Server host (default: localhost)
- `DATABASE_URL`: PostgreSQL connection string
- `ENABLE_VECTORS`: Enable vector operations (true/false)
- `RUST_LOG`: Log level (debug, info, warn, error)

## Contributing

When contributing to this example:

1. Ensure all new tools include comprehensive error handling
2. Add appropriate logging statements for debugging
3. Include JSON schema validation for tool parameters
4. Update this README with new features or changes
5. Add tests for new functionality

## License

This example is part of the orbit-rs project and is dual-licensed under MIT OR Apache-2.0.