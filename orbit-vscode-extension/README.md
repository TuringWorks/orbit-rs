# Orbit-RS VSCode Extension

VSCode extension for Orbit-RS multi-model database with support for all protocols.

## Features

- ✅ **Multi-Protocol Support**: Connect to Orbit-RS via PostgreSQL, MySQL, CQL, Redis, Cypher, AQL, and MCP
- ✅ **Syntax Highlighting**: Full syntax highlighting for OrbitQL, Cypher, and AQL
- ✅ **Query Execution**: Execute queries directly from the editor
- ✅ **Schema Browser**: Browse databases, tables, and columns
- ✅ **Results Viewer**: Beautiful table view for query results
- ✅ **Connection Management**: Manage multiple connections
- ✅ **Code Snippets**: Quick snippets for common queries
- ✅ **Status Bar Integration**: Connection status in status bar

## Installation

1. Open VSCode
2. Go to Extensions (Ctrl+Shift+X / Cmd+Shift+X)
3. Search for "Orbit-RS"
4. Click Install

Or install from command line:
```bash
code --install-extension orbit-rs-0.1.0.vsix
```

## Usage

### Connecting to Orbit-RS

1. Open Command Palette (Ctrl+Shift+P / Cmd+Shift+P)
2. Run "Orbit: Connect to Orbit-RS"
3. Select or add a connection
4. Enter connection details (host, port, credentials)

### Executing Queries

1. Open a `.sql`, `.orbitql`, `.cypher`, or `.aql` file
2. Write your query
3. Press `Ctrl+Enter` (or `Cmd+Enter` on Mac) to execute
4. Or right-click and select "Execute Selection"

### Browsing Schema

1. Open the Orbit-RS sidebar
2. Click on "Orbit Schema" to browse databases, tables, and columns
3. Click on items to see details

### Managing Connections

1. Open the Orbit-RS sidebar
2. Click on "Orbit Connections"
3. Click "+" to add a new connection
4. Right-click on connections to connect/disconnect

## Supported Protocols

- **PostgreSQL** (Port 5432) - SQL queries
- **MySQL** (Port 3306) - SQL queries
- **CQL/Cassandra** (Port 9042) - CQL queries
- **Redis/RESP** (Port 6379) - Redis commands
- **Cypher/Bolt** (Port 7687) - Neo4j Cypher queries
- **AQL/ArangoDB** (Port 8529) - AQL queries
- **MCP** (Port 8080) - Model Context Protocol

## Configuration

Add to your `settings.json`:

```json
{
  "orbit.connections": [
    {
      "name": "Local Orbit",
      "protocol": "postgres",
      "host": "127.0.0.1",
      "port": 5432,
      "username": "orbit",
      "database": "postgres"
    }
  ],
  "orbit.defaultProtocol": "postgres",
  "orbit.queryTimeout": 30,
  "orbit.maxResults": 1000
}
```

## Keyboard Shortcuts

- `Ctrl+Enter` / `Cmd+Enter`: Execute query
- `Ctrl+Shift+P` / `Cmd+Shift+P`: Open Command Palette

## Requirements

- VSCode 1.80.0 or higher
- Orbit-RS server running and accessible

## Development

```bash
# Install dependencies
npm install

# Compile TypeScript
npm run compile

# Watch for changes
npm run watch

# Package extension
npm run package
```

## License

BSD-3-Clause

## Links

- [Orbit-RS Documentation](https://turingworks.github.io/orbit-rs)
- [GitHub Repository](https://github.com/turingworks/orbit-rs)

