# Orbit CLI

Interactive command-line client for Orbit database with syntax highlighting and formatted output.

## Features

- **SQL Syntax Highlighting** - Color-coded SQL keywords, operators, and values
- **Pretty-Printed Tables** - UTF-8 bordered tables with aligned columns
- **Multiple Output Formats** - Table, JSON, CSV, and plain text
- **Interactive REPL** - Command history, multi-line queries, and tab completion
- **Multi-Protocol Support** - PostgreSQL, MySQL, and CQL protocols
- **Meta Commands** - PostgreSQL-style backslash commands

## Installation

```bash
cargo build --release -p orbit-cli
```

The binary will be available at `target/release/orbit`.

## Usage

### Interactive Mode

Start the interactive REPL:

```bash
orbit
```

Connect to a specific database:

```bash
orbit --protocol postgres --host localhost --port 5432 --database mydb --username myuser
```

Short form:

```bash
orbit -H localhost -p 5432 -d mydb -u myuser
```

### Single Command Mode

Execute a single query and exit:

```bash
orbit -e "SELECT * FROM users WHERE age > 18;"
```

### File Execution Mode

Execute SQL from a file:

```bash
orbit -f script.sql
```

### Output Formats

Choose output format (table is default):

```bash
orbit --format json -e "SELECT * FROM users;"
orbit --format csv -e "SELECT * FROM users;"
orbit --format plain -e "SELECT * FROM users;"
```

## Command-Line Options

```
Options:
      --protocol <PROTOCOL>    Protocol to use [default: postgres]
                               [possible values: postgres, mysql, cql]
  -H, --host <HOST>            Host to connect to [default: localhost]
  -p, --port <PORT>            Port to connect to
  -d, --database <DATABASE>    Database name [default: orbit]
  -u, --username <USERNAME>    Username [default: orbit]
  -W, --password <PASSWORD>    Password for authentication
  -o, --format <FORMAT>        Output format [default: table]
                               [possible values: table, json, csv, plain]
  -e, --execute <EXECUTE>      Execute a single command and exit
  -f, --file <FILE>            File containing SQL commands to execute
  -v, --verbose                Enable verbose logging
  -h, --help                   Print help
  -V, --version                Print version
```

## Interactive REPL Commands

### SQL Queries

- End queries with semicolon (`;`) to execute
- Multi-line queries are supported
- Press **Enter** to continue query on next line
- Press **Ctrl+C** to cancel current query

Example:

```sql
orbit> SELECT id, name, email
    -> FROM users
    -> WHERE age > 18
    -> ORDER BY name;
```

### Meta Commands

PostgreSQL-style backslash commands:

- `\?` or `\help` - Show help information
- `\q`, `\quit`, `\exit` - Exit the CLI
- `\d` or `\dt` - List tables
- `\l` - List databases
- `\c` - Connect to different database
- `\timing` - Toggle query timing display
- `\format <format>` - Set output format (table, json, csv, plain)

Example:

```
orbit> \d
orbit> \timing
orbit> \format json
orbit> \q
```

## Examples

### Connect to PostgreSQL

```bash
orbit --protocol postgres -H localhost -p 5432 -d mydb -u postgres
```

### Connect to MySQL

```bash
orbit --protocol mysql -H localhost -p 3306 -d mydb -u root
```

### Connect to CQL (Cassandra)

```bash
orbit --protocol cql -H localhost -p 9042 -d mykeyspace
```

### Execute Query with JSON Output

```bash
orbit --format json -e "SELECT * FROM users LIMIT 10;"
```

### Execute SQL Script

```bash
orbit -f /path/to/schema.sql
```

### Interactive Session with Verbose Logging

```bash
orbit --verbose
```

## Color Scheme

The CLI uses syntax highlighting with the following color scheme:

**SQL Keywords** (SELECT, FROM, WHERE, etc.):
- Displayed in purple/magenta

**Operators** (=, >, <, AND, OR, etc.):
- Displayed in white/grey

**Numbers**:
- Displayed in orange/peach

**Strings**:
- Displayed in green

**Comments**:
- Displayed in grey

## Table Formatting

Query results are displayed in pretty-printed tables with:

- **UTF-8 rounded corners** - Smooth, modern appearance
- **Color-coded values** by data type:
  - Numbers → Green, right-aligned
  - Text → White, left-aligned
  - Booleans → Yellow, centered
  - NULL → Grey, centered
  - Dates/Times → Blue
  - UUIDs → Cyan
  - JSON → Yellow
  - Binary data → Magenta
- **Row count summary** at the bottom
- **Column headers** in cyan, centered

Example output:

```
╭─────┬────────┬───────────────────────┬─────╮
│ id  │ name   │ email                 │ age │
├─────┼────────┼───────────────────────┼─────┤
│ 1   │ Alice  │ alice@example.com     │ 30  │
│ 2   │ Bob    │ bob@example.com       │ 25  │
│ 3   │ Carol  │ carol@example.com     │ 35  │
╰─────┴────────┴───────────────────────┴─────╯

3 rows
```

## History

Command history is automatically saved to `~/.orbit_history` and persists across sessions.

Navigate history:
- **Up Arrow** - Previous command
- **Down Arrow** - Next command
- **Ctrl+R** - Reverse search through history

## Keyboard Shortcuts

- **Enter** - Execute query (if ends with `;`) or add new line
- **Ctrl+C** - Cancel current query/input
- **Ctrl+D** - Exit CLI (same as `\q`)
- **Ctrl+L** - Clear screen
- **Ctrl+A** - Move to beginning of line
- **Ctrl+E** - Move to end of line
- **Ctrl+K** - Delete from cursor to end of line
- **Ctrl+U** - Delete from cursor to beginning of line
- **Up/Down** - Navigate command history
- **Ctrl+R** - Search command history

## Environment Variables

The CLI respects the following environment variables:

- `RUST_LOG` - Set logging level (e.g., `RUST_LOG=debug orbit`)
- `NO_COLOR` - Disable color output if set

## Limitations (Current Version)

The current implementation includes all UI and formatting features but requires integration with the Orbit database connection logic:

- Connection logic is pending implementation
- Query execution will be added in the next iteration
- All formatting and syntax highlighting features are fully functional

## Development

Build in debug mode:

```bash
cargo build -p orbit-cli
```

Run directly:

```bash
cargo run -p orbit-cli -- [OPTIONS]
```

Run with debug logging:

```bash
RUST_LOG=debug cargo run -p orbit-cli
```

## License

BSD-3-Clause OR MIT

## Contributing

Please see the main Orbit repository for contribution guidelines.
