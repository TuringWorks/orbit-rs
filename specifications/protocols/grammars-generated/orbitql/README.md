# OrbitQL ANTLR4 Grammar

This directory contains the ANTLR4 grammar for OrbitQL, the unified multi-model query language for Orbit-RS.

## Files

| File | Description |
|------|-------------|
| `OrbitQLLexer.g4` | Lexer grammar defining tokens and keywords |
| `OrbitQLParser.g4` | Parser grammar defining syntax rules |

## Overview

OrbitQL is a SQL-compatible query language inspired by SurrealDB's SurrealQL, with extensions for:

- **SurrealDB-Style Schema**: DEFINE/REMOVE statements for tables, fields, indexes, functions, events
- **Graph Operations**: TRAVERSE, RELATE, MATCH statements for graph traversal
- **Vector Operations**: KNN search, hybrid search, embedding generation, distance functions
- **Time-Series Analytics**: NOW(), INTERVAL, TIME_BUCKET functions
- **Machine Learning**: ML_TRAIN_MODEL, ML_PREDICT, XGBoost, LightGBM, CatBoost, etc.
- **Spatial Queries**: PostGIS-compatible ST_* functions
- **Real-Time Streaming**: LIVE queries with change notifications, KILL statement
- **Common Table Expressions**: WITH clause for recursive and non-recursive CTEs
- **Advanced SQL**: CASE expressions, window functions, conditional aggregates
- **Control Flow**: IF/ELSE, FOR loops, LET variables, THROW exceptions
- **Transaction Control**: BEGIN, COMMIT, ROLLBACK, SAVEPOINT support

## Usage with ANTLR4

### Generate Rust Parser

```bash
# Install ANTLR4
pip install antlr4-tools

# Generate Rust code
antlr4 -Dlanguage=Rust -visitor OrbitQLLexer.g4 OrbitQLParser.g4
```

### Generate Java Parser

```bash
antlr4 -visitor OrbitQLLexer.g4 OrbitQLParser.g4
javac *.java
```

### Generate Python Parser

```bash
antlr4 -Dlanguage=Python3 -visitor OrbitQLLexer.g4 OrbitQLParser.g4
```

### Generate TypeScript Parser

```bash
antlr4 -Dlanguage=TypeScript -visitor OrbitQLLexer.g4 OrbitQLParser.g4
```

## Grammar Features

### Statement Types

1. **DML Statements**
   - SELECT (with joins, CTEs, aggregations, window functions)
   - INSERT (with conflict handling)
   - UPDATE (with compound operators: +=, -=, etc.)
   - DELETE
   - UPSERT (insert or update)
   - MERGE (SQL MERGE statement)

2. **Schema Definition Statements (SurrealDB-style)**
   - DEFINE TABLE (SCHEMAFULL/SCHEMALESS, permissions)
   - DEFINE FIELD (types, defaults, assertions, readonly)
   - DEFINE INDEX (B-tree, vector HNSW/MTREE, full-text search)
   - DEFINE FUNCTION (user-defined functions with control flow)
   - DEFINE EVENT (triggers on CREATE/UPDATE/DELETE)
   - DEFINE ANALYZER (full-text search analyzers)
   - DEFINE USER/SCOPE/TOKEN (authentication)
   - DEFINE NAMESPACE/DATABASE
   - REMOVE statements for all DEFINE types

3. **Traditional DDL Statements**
   - CREATE TABLE/INDEX/VIEW/FUNCTION/TRIGGER/SCHEMA
   - ALTER TABLE
   - DROP statements
   - TRUNCATE TABLE

4. **Transaction Statements**
   - BEGIN / BEGIN TRANSACTION
   - COMMIT
   - ROLLBACK
   - SAVEPOINT / RELEASE SAVEPOINT / ROLLBACK TO SAVEPOINT
   - CANCEL (SurrealDB-style rollback)

5. **Graph Statements**
   - TRAVERSE (graph traversal with depth control)
   - RELATE (create graph relationships)
   - MATCH (Cypher-style pattern matching)

6. **Control Flow Statements**
   - LET (variable assignment)
   - IF/ELSE IF/ELSE (conditional execution)
   - FOR loops (iteration over queries, arrays, ranges)
   - BREAK / CONTINUE
   - RETURN
   - THROW (error handling)

7. **Streaming Statements**
   - LIVE SELECT (real-time subscriptions)
   - KILL (cancel live queries)

8. **Utility Statements**
   - USE (namespace/database selection)
   - INFO (system information)
   - SHOW (display objects)
   - SLEEP (pause execution)

9. **GraphRAG Statements**
   - GRAPHRAG BUILD/QUERY/EXTRACT/REASON/STATS/ENTITIES/SIMILAR

### Expression Types

- Literals (numeric, string, boolean, null, array, object)
- Identifiers and qualified names
- Variable references ($variable)
- Arithmetic operations (+, -, *, /, %)
- Comparison operations (=, !=, <, >, <=, >=)
- Logical operations (AND, OR, NOT)
- Pattern matching (LIKE, ILIKE, MATCH, regex ~)
- CASE expressions
- Function calls (including SurrealDB-style namespaced functions like string::concat, math::abs)
- Aggregate functions (COUNT, SUM, AVG, MIN, MAX, FIRST, LAST, STDDEV, VARIANCE)
- Window functions (ROW_NUMBER, RANK, DENSE_RANK, LEAD, LAG, etc.)
- Vector functions (vector::distance::cosine, vector::similarity::cosine, etc.)
- ML functions (ML_TRAIN_MODEL, ML_PREDICT, ml::embed_text, etc.)
- Spatial functions (ST_POINT, ST_DISTANCE, geo::distance, etc.)
- Time functions (time::now, duration::days, etc.)
- Crypto functions (crypto::sha256, crypto::argon2::generate, etc.)
- JSON operators (->, ->>, @>, <@)
- Subqueries
- Graph path expressions (->edge->, <-edge<-)

### Data Types

**Scalar Types**:
- BOOLEAN
- INTEGER, BIGINT, SMALLINT
- FLOAT, DOUBLE, DECIMAL, NUMERIC
- STRING, TEXT, VARCHAR
- DATETIME, TIMESTAMP, DATE, TIME
- DURATION
- UUID

**Collection Types**:
- ARRAY
- OBJECT
- JSON

**Spatial Types**:
- GEOMETRY, POINT, LINESTRING, POLYGON
- MULTIPOINT, MULTILINESTRING, MULTIPOLYGON
- GEOMETRYCOLLECTION

## Example Queries

### Basic SELECT

```sql
SELECT * FROM users WHERE active = true;
```

### Aggregation with CTE

```sql
WITH monthly_stats AS (
    SELECT DATE_TRUNC('month', created_at) AS month, COUNT(*) AS orders
    FROM orders
    GROUP BY DATE_TRUNC('month', created_at)
)
SELECT * FROM monthly_stats ORDER BY month DESC;
```

### Graph Traversal

```sql
SELECT person.name, ->follows->friend.name AS friend
FROM users person
TRAVERSE OUTBOUND 1..2 STEPS ON follows TO friend
WHERE person.location = 'NYC';
```

### Time-Series Query

```sql
SELECT
    TIME_BUCKET('1 hour', timestamp) AS hour,
    AVG(cpu_usage) AS avg_cpu
FROM metrics
WHERE timestamp >= NOW() - INTERVAL '24 hours'
GROUP BY TIME_BUCKET('1 hour', timestamp);
```

### ML Prediction

```sql
SELECT
    customer_id,
    ML_PREDICT('churn_model', ARRAY[tenure, monthly_charges]) AS churn_probability
FROM customers
WHERE contract_type = 'Month-to-month';
```

### Spatial Query

```sql
SELECT name, address
FROM stores
WHERE ST_DWITHIN(location, ST_POINT(-122.4194, 37.7749), 5000);
```

### Live Streaming

```sql
LIVE SELECT * FROM orders WHERE status = 'pending';
```

### DEFINE Statement (SurrealDB-style)

```sql
-- Define a table with permissions
DEFINE TABLE users SCHEMAFULL
    PERMISSIONS {
        create: WHERE $auth.role = 'admin',
        select: WHERE id = $auth.id OR $auth.role = 'admin'
    };

-- Define a field with validation
DEFINE FIELD email ON TABLE users TYPE string
    ASSERT string::is::email($value);

-- Define a vector index
DEFINE INDEX idx_embeddings ON TABLE documents FIELDS embedding
    HNSW DIMENSION 384 DIST COSINE EFC 200 M 16;

-- Define a function
DEFINE FUNCTION fn::get_discount(user_id: string) {
    LET $user = (SELECT * FROM users WHERE id = $user_id);
    IF $user.subscription = 'premium' THEN
        RETURN 0.20;
    ELSE
        RETURN 0.05;
    END;
};

-- Define an event
DEFINE EVENT on_user_signup ON TABLE users
    WHEN $event = "CREATE"
    THEN {
        CREATE notifications SET user_id = $after.id, message = 'Welcome!';
    };
```

### Vector KNN Search

```sql
SELECT id, content, vector::distance::cosine(embedding, $query_vector) AS distance
FROM documents
ORDER BY vector::distance::cosine(embedding, $query_vector)
LIMIT 10;
```

### Control Flow

```sql
FOR $user IN (SELECT * FROM users WHERE status = 'pending') {
    UPDATE users SET status = 'active' WHERE id = $user.id;
    IF $user.email IS NOT NULL THEN
        CREATE notifications SET user_id = $user.id, message = 'Account activated!';
    END;
};
```

### Transaction with Savepoint

```sql
BEGIN TRANSACTION;
    INSERT INTO orders (id, user_id, total) VALUES ('ord1', 'user1', 100);
    SAVEPOINT before_items;
    INSERT INTO order_items (order_id, product_id, qty) VALUES ('ord1', 'prod1', 2);
    ROLLBACK TO SAVEPOINT before_items;
    INSERT INTO order_items (order_id, product_id, qty) VALUES ('ord1', 'prod2', 3);
COMMIT;
```

## Related Documentation

- [OrbitQL Reference](../../Protocol-specs/orbitql-reference-rust.md) - Complete language reference
- [OrbitQL Documentation](../../../../docs/content/server/ORBITQL_COMPLETE_DOCUMENTATION.md) - User documentation

## License

BSD-3-Clause OR MIT

Copyright (c) 2025 TuringWorks
