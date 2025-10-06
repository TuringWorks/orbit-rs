# OrbitQL Language Server Protocol (LSP) Implementation

This module provides Language Server Protocol support for OrbitQL, enabling rich IDE features like syntax highlighting, autocomplete, error detection, hover information, and intelligent suggestions.

## Overview

The OrbitQL LSP implementation provides:

- **Real-time syntax validation** using the OrbitQL lexer and parser
- **Context-aware autocompletion** for keywords, functions, table names, and column names
- **Hover documentation** for tables, functions, and schema information
- **Diagnostic reporting** for syntax errors and semantic issues
- **Code formatting** for improved readability
- **Schema integration** for intelligent suggestions based on database structure

## Architecture

### Core Components

#### `OrbitQLLanguageServer`
The main language server implementation that handles LSP requests and maintains document state.

#### `Document`
Represents an open OrbitQL file with parsed queries and diagnostics.

#### `SchemaInfo`
Contains metadata about available tables, functions, and keywords for intelligent completions.

#### `ParsedQuery`
Represents a parsed OrbitQL query with AST, errors, and dependency information.

### Key Features

#### 1. Query Parsing and Validation
- Extracts individual queries from documents
- Parses each query using the OrbitQL parser
- Reports syntax errors and warnings
- Tracks table dependencies

#### 2. Context-Aware Completions
- **Table Names**: Suggests available tables after FROM, JOIN, UPDATE, INTO clauses
- **Column Names**: Suggests columns specific to referenced tables
- **Functions**: Provides function signatures with parameter hints
- **Keywords**: Context-sensitive keyword suggestions
- **OrbitQL Extensions**: Graph operations, time functions, and specialized syntax

#### 3. Hover Information
- **Tables**: Shows table type, columns, indexes, and descriptions
- **Functions**: Displays parameters, return types, and usage examples
- **Columns**: Provides data types and constraints

#### 4. Error Detection
- **Syntax Errors**: Invalid query structure
- **Semantic Errors**: Unknown tables or columns
- **Type Mismatches**: Incompatible data types
- **Performance Warnings**: Potentially inefficient queries

## Usage

### Starting the LSP Server

```bash
# Build the language server
cargo build --bin orbitql-lsp

# Start the server (typically called by IDE)
./target/debug/orbitql-lsp
```

### IDE Integration

The language server communicates via stdin/stdout using the LSP protocol. See the VS Code extension in `tools/vscode-orbitql/` for an example integration.

#### Supported LSP Features

| Feature | Status | Description |
|---------|--------|-------------|
| `textDocument/didOpen` | ✅ | Document opened |
| `textDocument/didChange` | ✅ | Document modified |
| `textDocument/completion` | ✅ | Autocompletion |
| `textDocument/hover` | ✅ | Hover information |
| `textDocument/formatting` | ✅ | Code formatting |
| `textDocument/publishDiagnostics` | ✅ | Error reporting |
| `textDocument/signatureHelp` | 🚧 | Function signatures |
| `textDocument/definition` | ❌ | Go to definition |
| `textDocument/references` | ❌ | Find references |

## Configuration

### Schema Management

The LSP server can be configured with schema information for better completions:

```rust
use orbit_shared::orbitql::lsp::{SchemaInfo, TableInfo, ColumnInfo, TableType};

let mut schema = SchemaInfo::default();

// Add table information
schema.tables.insert("users".to_string(), TableInfo {
    name: "users".to_string(),
    table_type: TableType::Document,
    columns: vec![
        ColumnInfo {
            name: "id".to_string(),
            data_type: "uuid".to_string(),
            nullable: false,
            description: Some("User identifier".to_string()),
            examples: vec![],
        },
        // ... more columns
    ],
    indexes: vec![],
    description: Some("User accounts table".to_string()),
});

// Update server schema
server.update_schema(schema).await;
```

### LSP Server Configuration

```rust
use orbit_shared::orbitql::lsp::LSPConfig;

let config = LSPConfig {
    enable_completion: true,
    enable_diagnostics: true,
    enable_hover: true,
    enable_signature_help: true,
    enable_formatting: true,
    max_completion_items: 50,
    diagnostic_delay_ms: 300,
};
```

## Examples

### Basic Query Completion

When typing:
```orbitql
SELECT name FROM u|
```

The LSP provides completions:
- `users` (table)
- `user_sessions` (table)
- `user_preferences` (table)

### Function Completion with Snippets

When typing:
```orbitql
SELECT COUNT(|
```

The LSP provides:
```
COUNT(${1:expression}) -> integer
Returns the number of rows or non-null values
Examples: COUNT(*), COUNT(user_id)
```

### Error Detection

For invalid syntax:
```orbitql
SELECT * FORM users WHERE active = true;
```

The LSP reports:
```
Parse error: Unexpected token FORM at line 1, column 10. Expected one of: [FROM]
```

### Hover Information

Hovering over `users` table shows:
```
**users** (Document table)

User accounts and profile information

**Columns:**
- `id`: uuid
- `name`: string
- `email`: string  
- `profile`: object
```

## Extension Points

### Custom Functions

Add domain-specific functions to the schema:

```rust
schema.functions.insert("geo::distance".to_string(), FunctionInfo {
    name: "geo::distance".to_string(),
    description: "Calculate distance between two geographic points".to_string(),
    parameters: vec![
        ParameterInfo {
            name: "point1".to_string(),
            data_type: "geopoint".to_string(),
            optional: false,
            description: "First geographic point".to_string(),
        },
        ParameterInfo {
            name: "point2".to_string(),
            data_type: "geopoint".to_string(),
            optional: false,
            description: "Second geographic point".to_string(),
        },
    ],
    return_type: "float".to_string(),
    examples: vec![
        "geo::distance([37.7749, -122.4194], [40.7128, -74.0060])".to_string()
    ],
});
```

### Custom Keywords

Extend OrbitQL with domain-specific keywords:

```rust
schema.keywords.extend([
    "STREAM".to_string(),
    "WINDOW".to_string(),
    "PARTITION".to_string(),
]);
```

## Testing

Run the LSP tests:

```bash
cargo test orbitql::lsp
```

Test with a real LSP client:
```bash
# Start server
cargo run --bin orbitql-lsp

# Test basic functionality
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}' | ./target/debug/orbitql-lsp
```

## Performance Considerations

### Query Parsing
- Queries are parsed incrementally as documents change
- Parse results are cached per query
- Large documents are processed in chunks

### Completion Performance
- Schema lookups are optimized with hash maps
- Completion results are limited to prevent UI lag
- Context analysis is kept lightweight

### Memory Usage
- Document content is stored only for open files
- AST trees are cached but can be evicted under pressure
- Schema information is shared across all documents

## Future Enhancements

1. **Go to Definition**: Navigate to table/function definitions
2. **Find References**: Locate all uses of tables/columns
3. **Refactoring**: Rename tables/columns across queries  
4. **Query Optimization Hints**: Suggest performance improvements
5. **Schema Inference**: Auto-detect schema from query patterns
6. **Multi-file Support**: Cross-file query analysis
7. **Workspace Symbols**: Global symbol search
8. **Code Actions**: Quick fixes and refactoring suggestions

## Contributing

When extending the LSP implementation:

1. Add new completion contexts to `CompletionContext`
2. Extend `generate_completions()` for new suggestion types
3. Update `generate_diagnostics()` for new error types
4. Add hover providers in `generate_hover()`
5. Include comprehensive tests for new functionality

## References

- [Language Server Protocol Specification](https://microsoft.github.io/language-server-protocol/)
- [Tower LSP Documentation](https://docs.rs/tower-lsp/latest/tower_lsp/)
- [OrbitQL Grammar Reference](../README.md)