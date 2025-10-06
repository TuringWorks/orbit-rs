# OrbitQL Language Support for Visual Studio Code

This extension provides comprehensive language support for OrbitQL, the unified multi-model query language used in the Orbit distributed database system.

## Features

- **Syntax Highlighting**: Rich syntax highlighting for OrbitQL queries with support for SQL, graph, and NoSQL operations
- **Intelligent Autocomplete**: Context-aware completion suggestions for keywords, functions, table names, and column names
- **Real-time Error Detection**: Instant feedback on syntax errors and semantic issues
- **Hover Documentation**: Detailed information about tables, columns, and functions on hover
- **Code Formatting**: Automatic code formatting for better readability
- **Go to Definition**: Navigate to table and function definitions
- **Query Validation**: Real-time validation of OrbitQL syntax

## Installation

### Prerequisites

1. **OrbitQL Language Server**: Ensure you have the OrbitQL language server binary installed:
   ```bash
   cargo install --path orbit-shared --bin orbitql-lsp
   ```

2. **VS Code**: Version 1.74.0 or higher

### Extension Installation

1. Package the extension:
   ```bash
   npm install
   npm run compile
   vsce package
   ```

2. Install the generated `.vsix` file in VS Code:
   ```bash
   code --install-extension orbitql-0.1.0.vsix
   ```

## Usage

### File Extensions

OrbitQL files are recognized by the following extensions:
- `.oql`
- `.orbitql`

### Example OrbitQL Query

```orbitql
-- Multi-model query combining document, graph, and time-series operations
SELECT 
    user.name,
    user.profile.* AS profile,
    ->follows->user.name AS friends,
    metrics[cpu_usage WHERE timestamp > time::now() - 1h] AS recent_cpu
FROM users AS user
WHERE user.active = true AND user.age > 18
RELATE user->viewed->product SET timestamp = time::now()
FETCH friends, recent_cpu;
```

## Configuration

Configure the extension through VS Code settings:

```json
{
  "orbitql.server.path": "orbitql-lsp",
  "orbitql.server.args": [],
  "orbitql.trace.server": "off",
  "orbitql.completion.enabled": true,
  "orbitql.diagnostics.enabled": true,
  "orbitql.hover.enabled": true,
  "orbitql.formatting.enabled": true
}
```

### Configuration Options

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `orbitql.server.path` | string | `"orbitql-lsp"` | Path to the OrbitQL language server executable |
| `orbitql.server.args` | array | `[]` | Arguments passed to the language server |
| `orbitql.trace.server` | enum | `"off"` | Trace level for LSP communication (`off`, `messages`, `verbose`) |
| `orbitql.completion.enabled` | boolean | `true` | Enable completion suggestions |
| `orbitql.diagnostics.enabled` | boolean | `true` | Enable error and warning diagnostics |
| `orbitql.hover.enabled` | boolean | `true` | Enable hover information |
| `orbitql.formatting.enabled` | boolean | `true` | Enable code formatting |

## Commands

The extension provides the following commands:

- **OrbitQL: Restart Language Server** (`orbitql.restart`): Restart the language server
- **OrbitQL: Show Server Status** (`orbitql.status`): Display the current status of the language server
- **OrbitQL: Format Document** (`orbitql.format`): Format the current OrbitQL document
- **OrbitQL: Validate Document** (`orbitql.validate`): Validate the current OrbitQL document

## Features in Detail

### Intelligent Autocomplete

The extension provides context-aware autocomplete suggestions:

- **Keywords**: SQL and OrbitQL-specific keywords
- **Functions**: Built-in functions with parameter hints
- **Tables**: Available tables and collections
- **Columns**: Table-specific column suggestions
- **OrbitQL Extensions**: Graph operations, time-series functions, and more

### Error Detection

Real-time error detection includes:

- **Syntax Errors**: Invalid query syntax
- **Semantic Errors**: Unknown tables, columns, or functions
- **Type Errors**: Incompatible data types
- **Performance Warnings**: Potentially slow queries

### Hover Documentation

Rich hover information shows:

- **Tables**: Schema information, column details, indexes
- **Functions**: Parameter types, return values, examples
- **Columns**: Data types, constraints, sample values

## Troubleshooting

### Language Server Not Starting

1. Verify the OrbitQL language server is installed:
   ```bash
   which orbitql-lsp
   ```

2. Check the server path in settings:
   ```json
   {
     "orbitql.server.path": "/full/path/to/orbitql-lsp"
   }
   ```

3. Enable verbose logging:
   ```json
   {
     "orbitql.trace.server": "verbose"
   }
   ```

### Autocomplete Not Working

1. Ensure the file has the correct extension (`.oql` or `.orbitql`)
2. Check that completion is enabled in settings
3. Restart the language server with the `orbitql.restart` command

### Syntax Highlighting Issues

1. Verify the file is recognized as OrbitQL (check the language mode in the status bar)
2. Try reopening the file
3. Check for conflicts with other SQL extensions

## Development

### Building from Source

```bash
git clone https://github.com/your-org/orbit-rs.git
cd orbit-rs/tools/vscode-orbitql
npm install
npm run compile
```

### Running Tests

```bash
npm test
```

### Contributing

Contributions are welcome! Please see the main Orbit repository for contribution guidelines.

## License

This extension is licensed under the same license as the Orbit project.

## Support

For support and bug reports, please visit the [Orbit GitHub repository](https://github.com/your-org/orbit-rs/issues).