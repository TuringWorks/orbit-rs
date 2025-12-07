# OrbitQL ANTLR4 Grammar

This directory contains the ANTLR4 grammar for OrbitQL, the unified multi-model query language for Orbit-RS.

## Files

| File | Description |
|------|-------------|
| `OrbitQLLexer.g4` | Lexer grammar defining tokens and keywords |
| `OrbitQLParser.g4` | Parser grammar defining syntax rules |

## Overview

OrbitQL is a SQL-compatible query language with extensions for:

- **Graph Operations**: TRAVERSE, RELATE statements for graph traversal
- **Time-Series Analytics**: NOW(), INTERVAL, TIME_BUCKET functions
- **Machine Learning**: ML_TRAIN_MODEL, ML_PREDICT, XGBoost, LightGBM, CatBoost, etc.
- **Spatial Queries**: PostGIS-compatible ST_* functions
- **Real-Time Streaming**: LIVE queries with change notifications
- **Common Table Expressions**: WITH clause for recursive and non-recursive CTEs
- **Advanced SQL**: CASE expressions, window functions, conditional aggregates

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

2. **DDL Statements**
   - CREATE TABLE/INDEX/VIEW/FUNCTION/TRIGGER/SCHEMA
   - ALTER TABLE
   - DROP statements

3. **Transaction Statements**
   - BEGIN
   - COMMIT
   - ROLLBACK

4. **Graph Statements**
   - TRAVERSE (graph traversal with depth control)
   - RELATE (create graph relationships)

5. **Streaming Statements**
   - LIVE SELECT (real-time subscriptions)

6. **GraphRAG Statements**
   - GRAPHRAG BUILD/QUERY/EXTRACT/REASON/STATS/ENTITIES/SIMILAR

### Expression Types

- Literals (numeric, string, boolean, null, array, object)
- Identifiers and qualified names
- Arithmetic operations (+, -, *, /, %)
- Comparison operations (=, !=, <, >, <=, >=)
- Logical operations (AND, OR, NOT)
- Pattern matching (LIKE, ILIKE, MATCH)
- CASE expressions
- Function calls
- Aggregate functions (COUNT, SUM, AVG, MIN, MAX, etc.)
- Window functions
- ML functions (ML_TRAIN_MODEL, ML_PREDICT, etc.)
- Spatial functions (ST_POINT, ST_DISTANCE, etc.)
- Subqueries
- Graph path expressions

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

## Related Documentation

- [OrbitQL Reference](../../Protocol-specs/orbitql-reference-rust.md) - Complete language reference
- [OrbitQL Documentation](../../../../docs/content/server/ORBITQL_COMPLETE_DOCUMENTATION.md) - User documentation

## License

BSD-3-Clause OR MIT

Copyright (c) 2025 TuringWorks
