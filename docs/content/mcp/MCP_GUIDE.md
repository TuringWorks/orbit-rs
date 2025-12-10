---
layout: default
title: "MCP Integration Guide"
subtitle: "Model Context Protocol support in Orbit-RS"
category: "mcp"
---

# Model Context Protocol (MCP) Integration

Orbit-RS supports the Model Context Protocol for AI/LLM integration.

---

## Overview

MCP enables:
- **Tool Exposure**: Database operations as LLM tools
- **Context Retrieval**: Semantic search for relevant data
- **Schema Access**: Automatic schema introspection
- **Query Generation**: Natural language to SQL/Cypher

---

## Quick Start

### Enable MCP Server

```toml
# orbit-server.toml
[mcp]
enabled = true
port = 3000
auth_required = true
```

### Connect from Claude Desktop

```json
// claude_desktop_config.json
{
  "mcpServers": {
    "orbit": {
      "command": "orbit-mcp",
      "args": ["--host", "localhost", "--port", "3000"]
    }
  }
}
```

---

## Available Tools

### Query Tool

Execute SQL, Cypher, or OrbitQL queries:

```json
{
  "name": "query",
  "description": "Execute a database query",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": {"type": "string"},
      "language": {"enum": ["sql", "cypher", "orbitql"]}
    }
  }
}
```

### Schema Tool

Get database schema information:

```json
{
  "name": "get_schema",
  "description": "Get schema for a table or all tables",
  "inputSchema": {
    "type": "object",
    "properties": {
      "table_name": {"type": "string"},
      "include_indexes": {"type": "boolean"}
    }
  }
}
```

### Search Tool

Semantic search across data:

```json
{
  "name": "semantic_search",
  "description": "Search data using natural language",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": {"type": "string"},
      "tables": {"type": "array"},
      "limit": {"type": "integer"}
    }
  }
}
```

---

## Resources

### Table Resources

```
orbit://tables/{table_name}
orbit://tables/{table_name}/schema
orbit://tables/{table_name}/sample
```

### Graph Resources

```
orbit://graph/nodes/{label}
orbit://graph/relationships/{type}
orbit://graph/schema
```

### Metrics Resources

```
orbit://metrics/queries
orbit://metrics/storage
orbit://metrics/cluster
```

---

## Configuration

### Security Settings

```toml
[mcp.security]
auth_method = "token"  # token, oauth2, none
allowed_operations = ["read", "write"]
rate_limit = 100  # requests per minute
max_result_size = "10MB"
```

### Tool Configuration

```toml
[mcp.tools]
query_enabled = true
schema_enabled = true
search_enabled = true
write_enabled = false  # Disable mutations

[mcp.tools.query]
max_execution_time = "30s"
max_rows = 10000
```

---

## REST API Integration

### Natural Language Query

```bash
curl -X POST http://localhost:8080/api/v1/nl/query \
  -H "Content-Type: application/json" \
  -d '{
    "question": "Find all users who signed up last month",
    "context": "users table has columns: id, email, created_at"
  }'
```

Response:
```json
{
  "query": "SELECT * FROM users WHERE created_at >= '2025-01-01'",
  "confidence": 0.95,
  "explanation": "Filtering users by created_at for last month"
}
```

---

## Implementation

### Source Files

```
orbit/server/src/mcp/
├── server.rs       # MCP server implementation
├── tools.rs        # Tool definitions
├── resources.rs    # Resource handlers
└── handlers/
    ├── query.rs    # Query tool handler
    ├── schema.rs   # Schema tool handler
    └── search.rs   # Search tool handler
```

---

## Resources

- **MCP Spec**: https://modelcontextprotocol.io
- **Source**: `orbit/server/src/mcp/`
