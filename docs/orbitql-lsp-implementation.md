# OrbitQL Language Server Protocol (LSP) Implementation

This document describes the comprehensive Language Server Protocol implementation for OrbitQL, the unified multi-model query language in the Orbit distributed database system.

## Overview

The OrbitQL LSP implementation provides rich IDE support for OrbitQL developers, including:

- **Syntax Highlighting**: Rich syntax highlighting for multi-model queries
- **Intelligent Autocomplete**: Context-aware completion suggestions
- **Real-time Error Detection**: Immediate feedback on syntax and semantic errors
- **Hover Documentation**: Detailed information about tables, functions, and schemas
- **Code Formatting**: Automatic query formatting and beautification
- **Schema Integration**: Dynamic schema awareness for intelligent suggestions

## Architecture

### Core Components

#### 1. OrbitQLLanguageServer (`orbit-shared/src/orbitql/lsp.rs`)

The main language server implementation that handles LSP protocol messages and maintains document state. Key features:

- **Document Management**: Tracks open OrbitQL files and their content
- **Query Parsing**: Parses queries using the existing OrbitQL lexer and parser
- **Schema Integration**: Maintains schema information for intelligent completions
- **Diagnostic Generation**: Provides real-time error and warning feedback

#### 2. Schema Information System

```rust
pub struct SchemaInfo {
    pub tables: HashMap<String, TableInfo>,
    pub functions: HashMap<String, FunctionInfo>,
    pub keywords: Vec<String>,
}

pub struct TableInfo {
    pub name: String,
    pub table_type: TableType, // Document, Graph, TimeSeries, etc.
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub description: Option<String>,
}
```

#### 3. Context-Aware Completion Engine

The completion system analyzes query context to provide relevant suggestions:

- **Table Context**: After `FROM`, `JOIN`, `UPDATE`, `INTO` clauses
- **Column Context**: Field suggestions based on referenced tables
- **Function Context**: Built-in and OrbitQL-specific functions
- **Keyword Context**: SQL and OrbitQL keywords based on current position

### Key Features

#### 1. Multi-Model Query Support

The LSP understands OrbitQL's unique multi-model syntax:

```orbitql
-- Document queries with graph traversal
SELECT 
    user.name,
    user.profile.* AS profile,
    ->follows->user.name AS friends,
    metrics[cpu_usage WHERE timestamp > time::now() - 1h] AS recent_cpu
FROM users AS user
WHERE user.active = true
RELATE user->viewed->product SET timestamp = time::now()
FETCH friends, recent_cpu;
```

Provides completion for:
- Document field access (`user.profile.*`)
- Graph traversal operators (`->follows->`)
- Time-series access (`metrics[...]`)
- OrbitQL-specific functions (`time::now()`)

#### 2. Intelligent Error Detection

Real-time diagnostics for:

**Syntax Errors**:
```orbitql
SELECT * FORM users; -- "FORM" should be "FROM"
```

**Semantic Errors**:
```orbitql
SELECT * FROM non_existent_table; -- Unknown table warning
SELECT invalid_column FROM users; -- Unknown column warning
```

**Type Mismatches**:
```orbitql
SELECT COUNT(*) + 'string'; -- Type incompatibility
```

#### 3. Rich Hover Information

Contextual information on hover:

**Tables**:
```
**users** (Document table)

User accounts and profile information

**Columns:**
- `id`: uuid (Primary Key)
- `name`: string (Not Null)
- `email`: string (Unique)
- `profile`: object (JSON document)
- `created_at`: datetime

**Indexes:**
- `idx_users_email` (Unique)
- `idx_users_created_at` (B-tree)
```

**Functions**:
```
**time::now**() -> datetime

Returns the current timestamp in UTC

**Examples:**
- time::now()
- WHERE created_at > time::now() - 24h
```

#### 4. Advanced Code Formatting

Intelligent formatting that understands OrbitQL's multi-model syntax:

```orbitql
-- Before formatting
select user.name,->follows->user.name as friends from users user where user.active=true

-- After formatting
SELECT 
    user.name,
    ->follows->user.name AS friends
FROM users AS user
WHERE user.active = true;
```

## IDE Integration

### VS Code Extension

The VS Code extension (`tools/vscode-orbitql/`) provides seamless integration:

#### Features
- File association for `.oql` and `.orbitql` files
- Syntax highlighting with OrbitQL-specific grammar
- LSP client integration for all language features
- Configurable server settings
- Command palette integration

#### Installation
```bash
# Build the language server
cargo build --bin orbitql-lsp

# Package the VS Code extension
cd tools/vscode-orbitql
npm install
npm run compile
vsce package

# Install the extension
code --install-extension orbitql-0.1.0.vsix
```

#### Configuration
```json
{
  "orbitql.server.path": "orbitql-lsp",
  "orbitql.completion.enabled": true,
  "orbitql.diagnostics.enabled": true,
  "orbitql.hover.enabled": true,
  "orbitql.formatting.enabled": true
}
```

### Other IDE Support

The LSP implementation is IDE-agnostic and can be integrated with:

#### Neovim/Vim
```lua
require'lspconfig'.orbitql.setup{
  cmd = {"orbitql-lsp"},
  filetypes = {"orbitql"},
  root_dir = function() return vim.loop.cwd() end,
}
```

#### Emacs
```elisp
(add-to-list 'lsp-language-id-configuration '(orbitql-mode . "orbitql"))
(lsp-register-client
 (make-lsp-client :new-connection (lsp-stdio-connection "orbitql-lsp")
                  :major-modes '(orbitql-mode)
                  :server-id 'orbitql-lsp))
```

#### Sublime Text
```json
{
  "clients": {
    "orbitql": {
      "enabled": true,
      "command": ["orbitql-lsp"],
      "selector": "source.orbitql",
      "schemes": ["file"]
    }
  }
}
```

## Technical Implementation

### LSP Protocol Compliance

The implementation supports the following LSP features:

| Feature | Status | Description |
|---------|--------|-------------|
| `textDocument/didOpen` | ✅ | Document opened notification |
| `textDocument/didChange` | ✅ | Document change notification |
| `textDocument/completion` | ✅ | Autocompletion requests |
| `textDocument/hover` | ✅ | Hover information |
| `textDocument/formatting` | ✅ | Document formatting |
| `textDocument/publishDiagnostics` | ✅ | Error/warning reporting |
| `textDocument/signatureHelp` | 🚧 | Function signature help |
| `textDocument/definition` | ❌ | Go to definition |
| `textDocument/references` | ❌ | Find references |

### Performance Optimizations

#### Query Parsing
- Incremental parsing for changed documents
- Cached AST trees with invalidation
- Parallel processing for large files

#### Completion Performance
- Schema information cached in memory
- Completion results limited to prevent UI lag
- Context analysis optimized for responsiveness

#### Memory Management
- Document content stored only for open files
- Configurable cache limits
- Garbage collection of unused schema data

### Error Handling

Robust error handling throughout the system:

```rust
pub enum LSPError {
    ParseError(ParseError),
    SchemaError(String),
    IOError(std::io::Error),
    ProtocolError(String),
}
```

All errors are gracefully handled and reported through the LSP protocol without crashing the server.

## Configuration and Customization

### Server Configuration

```rust
pub struct LSPConfig {
    pub enable_completion: bool,
    pub enable_diagnostics: bool,
    pub enable_hover: bool,
    pub enable_signature_help: bool,
    pub enable_formatting: bool,
    pub max_completion_items: usize,
    pub diagnostic_delay_ms: u64,
}
```

### Schema Configuration

Dynamic schema loading from multiple sources:

```rust
// Load from database metadata
let schema = SchemaInfo::from_database_metadata(&db_connection).await?;

// Load from configuration files
let schema = SchemaInfo::from_config_file("schema.json")?;

// Programmatic schema definition
let mut schema = SchemaInfo::default();
schema.add_table(TableInfo {
    name: "users".to_string(),
    table_type: TableType::Document,
    columns: vec![/* ... */],
    // ...
});
```

### Custom Functions and Extensions

Support for domain-specific OrbitQL extensions:

```rust
schema.functions.insert("geo::distance".to_string(), FunctionInfo {
    name: "geo::distance".to_string(),
    description: "Calculate distance between geographic points".to_string(),
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

## Usage Examples

### Basic Setup

```bash
# Start the LSP server (typically called by IDE)
orbitql-lsp

# Test with manual LSP messages
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}' | orbitql-lsp
```

### Programmatic Usage

```rust
use orbit_shared::orbitql::lsp::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (service, socket) = LspService::build(|client| {
        let mut server = OrbitQLLanguageServer::new(client);
        // Configure with custom schema
        tokio::spawn(async move {
            let schema = load_custom_schema().await;
            server.update_schema(schema).await;
        });
        server
    }).finish();
    
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;
    
    Ok(())
}
```

## Future Enhancements

### Planned Features

1. **Go to Definition**: Navigate to table and function definitions
2. **Find References**: Locate all uses of tables/columns across files
3. **Refactoring Support**: Rename tables/columns with cross-file updates
4. **Query Optimization Hints**: Suggest performance improvements
5. **Schema Inference**: Auto-detect schema from query patterns
6. **Multi-file Analysis**: Cross-file dependency tracking
7. **Workspace Symbols**: Global symbol search across projects
8. **Code Actions**: Quick fixes and automated refactoring
9. **Semantic Highlighting**: Advanced syntax highlighting based on semantics
10. **Query Execution Integration**: Execute queries directly from IDE

### Advanced Features

#### Live Schema Updates
```rust
// Watch for schema changes and update LSP in real-time
let schema_watcher = SchemaWatcher::new(db_connection);
schema_watcher.on_change(|new_schema| {
    lsp_server.update_schema(new_schema).await;
});
```

#### Query Performance Analysis
```rust
// Analyze query performance and suggest optimizations
let performance_analyzer = QueryPerformanceAnalyzer::new();
let suggestions = performance_analyzer.analyze_query(query).await;
lsp_server.publish_hints(suggestions).await;
```

#### Multi-Database Support
```rust
// Support multiple database connections with different schemas
let multi_schema = MultiSchemaManager::new();
multi_schema.add_database("primary", primary_schema);
multi_schema.add_database("analytics", analytics_schema);
```

## Testing and Quality Assurance

### Unit Tests
- Query parsing and validation
- Completion generation
- Hover information accuracy
- Diagnostic reporting

### Integration Tests
- LSP protocol compliance
- IDE integration testing
- Performance benchmarks
- Memory usage analysis

### Performance Benchmarks

| Operation | Target Time | Memory Usage |
|-----------|-------------|--------------|
| Document parse | < 50ms | < 10MB |
| Completion generation | < 100ms | < 5MB |
| Hover information | < 20ms | < 1MB |
| Diagnostic generation | < 200ms | < 15MB |

## Contributing

### Development Setup

```bash
# Clone repository
git clone https://github.com/your-org/orbit-rs.git
cd orbit-rs

# Build with LSP features
cargo build --features lsp

# Run tests
cargo test orbitql::lsp

# Start development server
cargo run --bin orbitql-lsp
```

### Adding New Features

1. Implement LSP message handlers in `OrbitQLLanguageServer`
2. Add corresponding protocol types in `lsp_types.rs`
3. Update client capabilities in IDE extensions
4. Add comprehensive tests
5. Update documentation

### Code Style

Follow Rust best practices and maintain consistency with the existing codebase:

```rust
// Good: Clear, documented function
/// Generate completion items for a specific context
async fn generate_completions(
    &self,
    document: &Document,
    position: Position,
) -> Vec<CompletionItem> {
    // Implementation
}
```

## License and Support

This LSP implementation is part of the Orbit project and follows the same licensing terms. For support and contributions, please refer to the main project repository.

For bugs and feature requests specifically related to the LSP implementation, please file issues with the `lsp` label in the project's issue tracker.