# PostgreSQL Protocol Implementation

Orbit-RS implements the PostgreSQL wire protocol (v3.0/3.2), allowing standard PostgreSQL clients (like `psql`, JDBC/ODBC drivers, and ORMs) to connect directly to Orbit clusters. This interface provides a SQL-compliant way to interact with distributed actors, persistent data, and vector indexes.

## Architecture

The PostgreSQL implementation consists of several layers:

1.  **Wire Protocol Server (`PostgresServer`)**: Listens on TCP (default 5432) and handles connection lifecycle.
2.  **Protocol Adapter (`PostgresWireProtocol`)**: Implements the PostgreSQL message flow (Startup, Authentication, Query, Parse/Bind/Execute).
3.  **Query Engine (`QueryEngine`)**: Parses SQL and routes execution to the appropriate backend:
    *   **Actor Backend**: Accesses actor state via the virtual `actors` table.
    *   **Persistent Storage**: Routes to the configured persistent storage engine (e.g., RocksDB, S3) for standard tables.
    *   **Vector Engine**: Handles vector search queries (`pgvector` compatible syntax).
    *   **GraphRAG Engine**: Handles specialized GraphRAG function calls.

## Supported Features

### Core SQL

Orbit-RS supports a subset of standard SQL-92 for data manipulation and definition.

*   **Statements**: `SELECT`, `INSERT`, `UPDATE`, `DELETE`, `CREATE TABLE`, `DROP TABLE`.
*   **Data Types**: `TEXT`, `INTEGER`, `FLOAT`, `BOOLEAN`, `JSONB`.
*   **Transactions**: Basic `BEGIN`, `COMMIT`, `ROLLBACK` support.

### Actors as Tables

Orbit-RS exposes actors as a virtual table named `actors`.

```sql
-- Select all actors of a specific type
SELECT * FROM actors WHERE type = 'user_profile';

-- Update actor state
UPDATE actors SET state = '{"status": "active"}' WHERE id = 'user:123';
```

### Vector Search (pgvector compatibility)

The implementation supports `pgvector`-style syntax for semantic search operations, powered by Orbit's vector store.

```sql
-- Create a table with vector column
CREATE TABLE items (id SERIAL PRIMARY KEY, embedding VECTOR(1536));

-- Semantic search (Cosine similarity)
SELECT * FROM items ORDER BY embedding <=> '[0.1, 0.2, ...]' LIMIT 5;
```

**Supported Operators:**
*   `<=>`: Cosine distance
*   `<->`: Euclidean distance
*   `<#>`: Inner product

### GraphRAG Extensions

Specialized functions are available for Graph Retrieval-Augmented Generation (GraphRAG) operations.

*   `GRAPHRAG_BUILD()`: Triggers graph construction.
*   `GRAPHRAG_QUERY(query_text)`: Performs retrieval over the graph.

## Configuration

PostgreSQL protocol settings in `orbit.toml`:

```toml
[postgres]
enabled = true
port = 5432
host = "0.0.0.0"
max_connections = 100
```

## Limitations

*   **Complex Joins**: Cross-backend joins (e.g., joining actors with persistent tables) have limited support.
*   **Stored Procedures**: PL/pgSQL is not supported.
*   **Authentication**: Currently defaults to 'trust' authentication; suitable for internal networks or development.
