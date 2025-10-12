# Orbit-RS PostgreSQL psql Demo

This demo showcases the PostgreSQL wire protocol implementation in Orbit-RS, allowing you to connect with standard PostgreSQL tools like `psql` and execute real SQL commands.

## Features Demonstrated

### ✅ **Implemented and Working**
- **DDL Operations**: CREATE/DROP TABLE, CREATE/DROP INDEX
- **DML Operations**: INSERT, SELECT, UPDATE, DELETE with WHERE clauses
- **JOIN Operations**: INNER, LEFT, RIGHT, FULL OUTER, CROSS JOINs
- **Index Creation**: B-tree indices with various column combinations
- **Transaction Support**: BEGIN, COMMIT, ROLLBACK
- **Data Types**: INTEGER, TEXT, DECIMAL, BOOLEAN, TIMESTAMP
- **PostgreSQL Wire Protocol**: Full compatibility with psql client
- **Information Schema**: Complete `information_schema` support for metadata queries
  - `information_schema.tables` - Table listing and metadata
  - `information_schema.columns` - Column definitions and data types
  - `information_schema.table_constraints` - Constraint information
  - `information_schema.key_column_usage` - Key and foreign key relationships

### ⚠️ **Partial Implementation**
- **Vector Operations**: VECTOR data type supported, similarity operators pending
- **JSON Operations**: Basic JSONB storage, advanced operators pending
- **Advanced SQL**: CTEs and window functions (syntax parsing, execution pending)

### 📋 **Data Persistence**
- **Current**: In-memory storage (data lost on server restart)
- **Planned**: WAL-based persistence with backup/recovery capabilities

## Prerequisites

- **Rust**: Install from [rustup.rs](https://rustup.rs/)
- **PostgreSQL Client**: For the `psql` command
  - **macOS**: `brew install postgresql`
  - **Ubuntu/Debian**: `sudo apt install postgresql-client`
  - **CentOS/RHEL**: `sudo yum install postgresql`

## Quick Start

### 1. Interactive Demo (Recommended)

```bash
cd examples/sql-psql-demo
./run_demo.sh
```

This will:
- Build the PostgreSQL server
- Start it on `localhost:5433`
- Present an interactive menu with demo options
- Allow you to run SQL scripts or open a direct psql session

### 2. Automated Demo

```bash
./run_demo.sh --auto-demo
```

Runs all demo scripts automatically without user interaction.

### 3. Manual Connection

Start the server:
```bash
./run_demo.sh --build-only
cargo run --bin postgres_server
```

Connect with psql:
```bash
# Note: SSL must be disabled as the server doesn't support SSL
PGSSLMODE=disable psql -h localhost -p 5433 -U orbit -d orbit_demo
```

## Demo Scripts

### 1. Basic Operations (`01_basic_operations.sql`)

Demonstrates fundamental SQL operations:

```sql
-- Create tables with constraints
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT UNIQUE,
    age INTEGER
);

-- Insert sample data
INSERT INTO users (id, name, email, age) VALUES 
    (1, 'Alice Johnson', 'alice@example.com', 28),
    (2, 'Bob Smith', 'bob@example.com', 34);

-- Query data
SELECT name, email FROM users WHERE age > 30;

-- Update records
UPDATE users SET age = 35 WHERE name = 'Bob Smith';

-- Delete records
DELETE FROM orders WHERE id = 1007;
```

### 2. JOINs and Indices (`02_joins_and_indices.sql`)

Showcases advanced query capabilities:

```sql
-- Create performance indices
CREATE INDEX idx_users_email ON users (email);
CREATE INDEX idx_orders_user_id ON orders (user_id);

-- INNER JOIN with multiple tables
SELECT 
    o.id as order_id,
    u.name as customer,
    p.name as product_name,
    o.total_amount
FROM orders o
INNER JOIN users u ON o.user_id = u.id
INNER JOIN products p ON o.product_id = p.id;

-- LEFT JOIN with aggregation
SELECT 
    u.name as user_name,
    COUNT(o.id) as order_count,
    SUM(o.total_amount) as total_spent
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
GROUP BY u.id, u.name, u.email
ORDER BY total_spent DESC;

-- FULL OUTER JOIN demonstration
SELECT 
    COALESCE(u.name, 'No user') as user_name,
    COALESCE(p.name, 'No product') as product_name
FROM users u
FULL OUTER JOIN orders o ON u.id = o.user_id
FULL OUTER JOIN products p ON o.product_id = p.id;
```

### 3. Vector & Advanced Features (`03_vector_and_advanced.sql`)

Explores cutting-edge capabilities:

### 4. Information Schema Demo (`04_information_schema.sql`)

Demonstrates metadata querying capabilities:

```sql
-- Query all tables in the database
SELECT table_name, table_type, is_insertable_into
FROM information_schema.tables
WHERE table_schema = 'public';

-- Get column information for all tables
SELECT table_name, column_name, data_type, is_nullable, ordinal_position
FROM information_schema.columns
ORDER BY table_name, ordinal_position;

-- Find all constraints in the database
SELECT table_name, constraint_name, constraint_type
FROM information_schema.table_constraints;

-- Query key column usage for foreign keys
SELECT table_name, column_name, constraint_name
FROM information_schema.key_column_usage;

-- Practical queries: Find all TEXT columns
SELECT table_name, column_name, data_type
FROM information_schema.columns
WHERE data_type = 'text';
```

This enables standard PostgreSQL metadata queries and provides the foundation for tools that introspect database schemas.

```sql
-- Enable vector extension
CREATE EXTENSION IF NOT EXISTS vector;

-- Create tables with vector embeddings
CREATE TABLE documents (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    embedding VECTOR(5),  -- 5-dimensional vectors
    metadata JSONB
);

-- Insert documents with embeddings
INSERT INTO documents (id, title, embedding, metadata) VALUES 
    (1, 'ML Introduction', '[0.8, 0.2, 0.9, 0.1, 0.7]',
     '{"tags": ["ML", "AI"], "difficulty": "easy"}');

-- Transaction demonstrations
BEGIN;
INSERT INTO documents (id, title) VALUES (6, 'Test Document');
ROLLBACK;  -- Data rolled back

-- Advanced SQL features
WITH category_stats AS (
    SELECT category, COUNT(*) as doc_count
    FROM documents GROUP BY category
)
SELECT category, doc_count,
    CASE WHEN doc_count >= 2 THEN 'Popular' ELSE 'Niche' END
FROM category_stats;
```

## Interactive psql Commands

Once connected, try these commands:

```sql
-- List all tables
\dt

-- Describe a table structure
\d users

-- Show table contents
SELECT * FROM users;

-- Complex query with JOIN
SELECT u.name, COUNT(o.id) as orders
FROM users u LEFT JOIN orders o ON u.id = o.user_id
GROUP BY u.id, u.name;

-- Create and query indices
CREATE INDEX idx_complex ON orders (user_id, total_amount);
\di  -- List indices
```

## Architecture

### Server Components

```
PostgreSQL Wire Protocol Server
├── Enhanced Connection Handler
│   ├── Message parsing (Query, Parse, Bind, Execute)
│   ├── Authentication simulation
│   └── Response formatting
├── SQL Executor Integration
│   ├── Full DDL support (CREATE/DROP TABLE/INDEX)
│   ├── Complete DML support (INSERT/SELECT/UPDATE/DELETE)
│   ├── Advanced JOIN processing
│   └── Transaction management
└── Data Storage
    ├── In-memory tables and indices
    ├── Schema validation
    └── Concurrent access support
```

### SQL Processing Pipeline

```
SQL Query → Parser → AST → Executor → Storage Engine → Results
     ↓         ↓       ↓        ↓           ↓           ↓
   "SELECT"  Tokens  Tree   Execution   Memory      Rows
```

## Troubleshooting

### Connection Issues

If psql fails to connect:

1. **Check server logs**: `cat server.log`
2. **Verify port**: Ensure port 5433 is available
3. **Restart server**: Use menu option 7 in interactive mode

### Build Issues

```bash
# Update Rust toolchain
rustup update

# Clean and rebuild
cargo clean
cargo build --release
```

### Performance Notes

- **Memory Usage**: Tables stored in RAM, limited by system memory
- **Concurrency**: Multiple connections supported
- **Indices**: Created but optimization pending in some cases

## Implementation Status

| Feature Category | Implementation Status | Notes |
|------------------|----------------------|-------|
| **Basic DDL** | ✅ Complete | CREATE/DROP TABLE, INDEX |
| **Basic DML** | ✅ Complete | INSERT/SELECT/UPDATE/DELETE |
| **JOIN Operations** | ✅ Complete | All JOIN types supported |
| **WHERE Clauses** | ✅ Complete | Complex expressions supported |
| **Transactions** | ✅ Complete | BEGIN/COMMIT/ROLLBACK |
| **Indices** | ✅ Complete | B-tree indices |
| **Vector Types** | ⚠️ Partial | Data type supported, operators pending |
| **JSON Operations** | ⚠️ Partial | Basic storage, advanced ops pending |
| **CTEs/Window Functions** | ⚠️ Partial | Parsing done, execution pending |
| **Data Persistence** | ❌ Planned | Currently in-memory only |

## Next Steps

1. **pg_catalog Schema**: Add `pg_catalog` system views to support native `\dt`, `\d` commands
2. **Vector Similarity**: Implement `<->`, `<=>`, `<#>` operators
3. **JSON Operators**: Add `->`, `->>`, `@>`, `?` operators  
4. **Advanced SQL**: Complete CTE and window function execution
5. **Persistence**: Add WAL-based durability
6. **Performance**: Query optimization and caching

## Contributing

To extend this demo:

1. **Add SQL Scripts**: Create new `.sql` files in the `sql/` directory
2. **Enhance Server**: Modify `src/server.rs` for new protocol features
3. **Update Menu**: Add new options to `run_demo.sh`

## Related Examples

- **Basic SQL Executor**: `examples/advanced_postgres_features.rs`
- **Vector Operations**: See vector-specific examples in `examples/`
- **Performance Benchmarks**: `orbit-benchmarks/` directory

---

**🚀 Enjoy exploring the Orbit-RS PostgreSQL implementation!**

For questions or issues, check the main project documentation or create an issue in the repository.