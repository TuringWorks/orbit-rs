# Orbit-RS VSCode Extension

**Status**: ✅ Complete  
**Version**: 0.1.0

## Overview

The Orbit-RS VSCode extension provides a comprehensive development experience for working with Orbit-RS multi-model database. It supports all 7 protocols and provides syntax highlighting, query execution, schema browsing, and connection management.

## Features

- ✅ **Multi-Protocol Support**: All 7 protocols (PostgreSQL, MySQL, CQL, Redis, Cypher, AQL, MCP)
- ✅ **Syntax Highlighting**: Full syntax highlighting for OrbitQL, Cypher, and AQL
- ✅ **Query Execution**: Execute queries directly from the editor with keyboard shortcuts
- ✅ **Schema Browser**: Browse databases, tables, and columns in a tree view
- ✅ **Results Viewer**: Beautiful HTML table view for query results with execution time
- ✅ **Connection Management**: Manage multiple connections with quick connect/disconnect
- ✅ **Code Snippets**: Quick snippets for common query patterns
- ✅ **Status Bar Integration**: Connection status indicator in status bar
- ✅ **Context Menus**: Right-click to execute queries
- ✅ **Command Palette Integration**: All features accessible via Command Palette

## Installation

### From VSCode Marketplace

1. Open VSCode
2. Go to Extensions (Ctrl+Shift+X / Cmd+Shift+X)
3. Search for "Orbit-RS"
4. Click Install

### From VSIX File

```bash
code --install-extension orbit-rs-0.1.0.vsix
```

### Development Build

```bash
cd orbit-vscode-extension
npm install
npm run compile
npm run package
code --install-extension orbit-rs-0.1.0.vsix
```

## Usage

### Connecting to Orbit-RS

1. Open Command Palette (Ctrl+Shift+P / Cmd+Shift+P)
2. Run "Orbit: Connect to Orbit-RS"
3. Select an existing connection or add a new one
4. Enter connection details:
   - Name
   - Protocol (PostgreSQL, MySQL, CQL, Redis, Cypher, AQL, MCP)
   - Host (default: 127.0.0.1)
   - Port (defaults per protocol)
   - Username/Password (optional)
   - Database (optional)

### Executing Queries

**Method 1: Keyboard Shortcut**
1. Open a `.sql`, `.orbitql`, `.cypher`, or `.aql` file
2. Write your query
3. Press `Ctrl+Enter` (Windows/Linux) or `Cmd+Enter` (Mac)

**Method 2: Command Palette**
1. Open Command Palette (Ctrl+Shift+P / Cmd+Shift+P)
2. Run "Orbit: Execute Query"

**Method 3: Context Menu**
1. Select query text
2. Right-click
3. Select "Execute Selection"

### Browsing Schema

1. Open the Orbit-RS sidebar (click Orbit icon in Activity Bar)
2. Click on "Orbit Schema" to browse:
   - Databases/Schemas
   - Tables/Collections
   - Columns/Fields

### Managing Connections

1. Open the Orbit-RS sidebar
2. Click on "Orbit Connections"
3. Click "+" button to add a new connection
4. Right-click on connections to connect/disconnect
5. View connection details by expanding connection items

## Supported Protocols

### PostgreSQL (Port 5432)
- Full SQL support (DDL, DML, DCL, TCL)
- Parameterized queries
- Schema browsing (databases, tables, columns)

### MySQL (Port 3306)
- Full SQL support
- Parameterized queries
- Schema browsing

### CQL/Cassandra (Port 9042)
- CQL 3.x support
- Keyspace and table browsing
- Prepared statements

### Redis/RESP (Port 6379)
- All Redis commands
- Key-value operations
- Hash, List, Set, Sorted Set operations

### Cypher/Bolt (Port 7687)
- Full Cypher query language
- Graph operations
- Node and relationship queries

### AQL/ArangoDB (Port 8529)
- AQL query language
- Document operations
- Graph traversal

### MCP (Port 8080)
- Model Context Protocol
- Natural language queries
- Tool execution

## Configuration

Add to your VSCode `settings.json`:

```json
{
  "orbit.connections": [
    {
      "name": "Local Orbit PostgreSQL",
      "protocol": "postgres",
      "host": "127.0.0.1",
      "port": 5432,
      "username": "orbit",
      "password": "",
      "database": "postgres"
    },
    {
      "name": "Local Orbit Redis",
      "protocol": "redis",
      "host": "127.0.0.1",
      "port": 6379
    }
  ],
  "orbit.defaultProtocol": "postgres",
  "orbit.queryTimeout": 30,
  "orbit.maxResults": 1000,
  "orbit.enableSyntaxHighlighting": true
}
```

## Keyboard Shortcuts

- `Ctrl+Enter` / `Cmd+Enter`: Execute query or selection
- `Ctrl+Shift+P` / `Cmd+Shift+P`: Open Command Palette

## Extension Structure

```
orbit-vscode-extension/
├── src/
│   ├── extension.ts              # Main extension entry point
│   ├── connectionManager.ts      # Connection management
│   ├── queryExecutor.ts           # Query execution
│   ├── resultsView.ts             # Results display
│   ├── schemaBrowser.ts           # Schema browsing
│   ├── connectionsView.ts         # Connections tree view
│   └── connections/               # Protocol-specific connections
│       ├── postgres.ts
│       ├── mysql.ts
│       ├── cql.ts
│       ├── redis.ts
│       ├── cypher.ts
│       ├── aql.ts
│       └── mcp.ts
├── syntaxes/                      # TextMate grammars
│   ├── orbitql.tmLanguage.json
│   ├── cypher.tmLanguage.json
│   └── aql.tmLanguage.json
├── snippets/                      # Code snippets
│   ├── orbitql.json
│   ├── sql.json
│   ├── cypher.json
│   └── aql.json
├── package.json                   # Extension manifest
├── tsconfig.json                  # TypeScript configuration
└── README.md                      # User documentation
```

## Development

### Prerequisites

- Node.js 18+
- npm or yarn
- VSCode 1.80.0+

### Setup

```bash
cd orbit-vscode-extension
npm install
```

### Build

```bash
# Compile TypeScript
npm run compile

# Watch for changes
npm run watch
```

### Test

```bash
# Run tests
npm test

# Run extension in development
# Press F5 in VSCode to launch Extension Development Host
```

### Package

```bash
# Create VSIX package
npm run package
```

## Features in Detail

### Query Execution

- Supports all 7 protocols
- Automatic protocol detection based on file language
- Progress indicator during execution
- Execution time display
- Error handling with user-friendly messages

### Results Viewer

- HTML-based table view
- Scrollable results
- Configurable max results (default: 1000)
- Execution time and row count display
- Query display for reference
- Protocol and language information

### Schema Browser

- Tree view of databases/schemas
- Expandable tables/collections
- Column/field details
- Protocol-specific schema queries
- Auto-refresh on connection changes

### Connection Management

- Multiple connection support
- Connection persistence in settings
- Quick connect/disconnect
- Connection status indicators
- Connection details view

## Related Documentation

- [Python Client Library](PYTHON_CLIENT_LIBRARY.md)
- [Protocol 100% Completion](PROTOCOL_100_PERCENT_COMPLETE.md)
- [Orbit-RS Architecture](architecture/ORBIT_ARCHITECTURE.md)

---

**Last Updated**: November 2025

