# MCP Basic Client Example

- Connect to an MCP server
- List and execute available tools
- Access server resources
- Work with prompts and templates

## Features Demonstrated

### Tool Execution
- Mathematical calculations
- Database queries
- Vector similarity search
- Custom tool parameter validation

### Resource Access
- File system resources (CSV, data files)
- Database connections
- Vector embeddings

### Prompt Management
- Data analysis prompts
- Report generation templates
- Parameterized prompt arguments

## Usage

### Basic Usage
```bash
cargo run --bin basic_client
```

### With Specific Tool Execution
```bash
cargo run --bin basic_client -- --tool calculate --params '{"expression": "2 + 2"}'
```

### With Database Query
```bash
cargo run --bin basic_client -- --tool search_database --params '{"query": "SELECT * FROM users", "limit": 10}'
```

### With Vector Search
```bash
cargo run --bin basic_client -- --tool vector_search --params '{"vector": [0.1, 0.2, 0.3], "k": 5}'
```

### Custom Server URL
```bash
cargo run --bin basic_client -- --server-url http://my-mcp-server:8080
```

## Example Output

```
Available tools:
  - calculate: Perform mathematical calculations
    Schema: {
      "type": "object",
      "properties": {
        "expression": {
          "type": "string",
          "description": "Mathematical expression to evaluate"
        }
      },
      "required": ["expression"]
    }
  - search_database: Search the database for records
  - vector_search: Perform vector similarity search

Available resources:
  - file:///data/users.csv: User Data
    Description: CSV file containing user information
    MIME Type: text/csv
  - postgres://localhost/orbit_db: Main Database
    Description: Primary PostgreSQL database
    MIME Type: application/x-postgresql

Available prompts:
  - analyze_data: Analyze dataset and provide insights
    Arguments: ["dataset", "analysis_type"]
  - generate_report: Generate a comprehensive report
    Arguments: ["data_source", "format"]
```

## Architecture

This example provides a foundation for:

1. **MCP Client Pattern**: Shows how to structure MCP client code
2. **Protocol Handling**: Demonstrates proper MCP message handling
3. **Error Management**: Shows how to handle MCP-specific errors
4. **Tool Integration**: Examples of different tool types and their schemas
5. **Resource Discovery**: How to enumerate and access server resources

## Next Steps

To extend this example:

1. Add real network connectivity (HTTP/WebSocket)
2. Implement actual tool execution logic
3. Add authentication and security
4. Support streaming responses
5. Add resource caching and management
6. Implement prompt execution and templating

## Integration with Orbit

This example works with the Orbit ecosystem by:

- Using the `orbit-protocols` crate for MCP types
- Demonstrating integration with vector operations
- Showing database query patterns
- Following Orbit's async/actor patterns