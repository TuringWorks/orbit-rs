# pgvector-Compatible PostgreSQL Vector Store

This example demonstrates how to use Orbit-RS as a **PostgreSQL-compatible vector database** with full **pgvector extension** support. The implementation provides seamless compatibility with existing PostgreSQL applications and tools while leveraging Orbit's distributed architecture and high-performance vector operations.

## 🐘 Features

- **Full PostgreSQL Wire Protocol**: Connect with any PostgreSQL client (psql, pgAdmin, libraries)
- **pgvector Extension Compatibility**: Use familiar pgvector syntax and functions
- **Vector Data Types**: Support for `VECTOR(n)`, `HALFVEC(n)`, and `SPARSEVEC(n)`
- **Similarity Operators**: Complete support for `<->`, `<#>`, `<=>` operators
- **Vector Indexes**: ivfflat and HNSW index creation with operator classes
- **Vector Functions**: `vector_dims()`, `vector_norm()`, and other pgvector functions
- **High Performance**: Distributed vector operations powered by Orbit's actor system

## 🚀 Quick Start

### Prerequisites

1. Start the Orbit server:
   ```bash
   cargo run --bin orbit-server
   ```

2. Run the pgvector-compatible PostgreSQL server:
   ```bash
   cargo run --example pgvector-store
   ```

3. Connect with any PostgreSQL client:
   ```bash
   # Using psql
   psql -h 127.0.0.1 -p 5433 -d postgres -U postgres
   
   # Using pgAdmin
   # Host: 127.0.0.1, Port: 5433, Database: postgres, User: postgres
   ```

## 📚 pgvector Command Reference

### 1. Enable the Vector Extension

```sql
CREATE EXTENSION vector;
```

### 2. Create Tables with Vector Columns

```sql
-- Documents table with 384-dimensional embeddings
CREATE TABLE documents (
  id SERIAL PRIMARY KEY,
  title TEXT,
  content TEXT,
  embedding VECTOR(384)
);

-- Products table with 128-dimensional feature vectors
CREATE TABLE products (
  id SERIAL PRIMARY KEY,
  name TEXT,
  category TEXT,
  features VECTOR(128)
);
```

### 3. Insert Vector Data

```sql
-- Insert a document with embedding
INSERT INTO documents (title, content, embedding) VALUES (
  'Machine Learning Basics',
  'Introduction to supervised and unsupervised learning...',
  '[0.1, 0.2, 0.3, 0.4, ...]'  -- 384 dimensions
);

-- Insert product with features
INSERT INTO products (name, category, features) VALUES (
  'Wireless Headphones',
  'Electronics',
  '[0.7, 0.8, 0.9, ...]'  -- 128 dimensions
);
```

### 4. Create Vector Indexes

```sql
-- Cosine similarity index for document embeddings
CREATE INDEX ON documents USING ivfflat (embedding vector_cosine_ops);

-- Euclidean distance index for product features
CREATE INDEX ON products USING hnsw (features vector_l2_ops);

-- Inner product index
CREATE INDEX ON documents USING ivfflat (embedding vector_ip_ops);
```

### 5. Similarity Search Queries

#### Cosine Similarity Search
```sql
-- Find documents similar to a query embedding
SELECT title, content, embedding <=> '[0.1, 0.2, 0.3, ...]' AS cosine_distance
FROM documents 
ORDER BY cosine_distance 
LIMIT 5;
```

#### Euclidean Distance Search
```sql
-- Find products with similar features
SELECT name, features <-> '[0.7, 0.8, 0.9, ...]' AS euclidean_distance
FROM products 
ORDER BY euclidean_distance 
LIMIT 3;
```

#### Inner Product Similarity
```sql
-- Find documents using inner product
SELECT title, embedding <#> '[0.1, 0.2, 0.3, ...]' AS inner_product
FROM documents 
ORDER BY inner_product DESC 
LIMIT 5;
```

### 6. Vector Functions

```sql
-- Get vector dimensions
SELECT vector_dims(embedding) FROM documents LIMIT 1;
-- Returns: 384

-- Check vector norm
SELECT vector_norm(features) FROM products WHERE id = 1;

-- Vector operations in WHERE clauses
SELECT * FROM documents 
WHERE embedding <=> '[0.1, 0.2, ...]' < 0.5;
```

## 🔧 Operator Classes and Index Types

### Similarity Operators

| Operator | Description | Use Case |
|----------|-------------|----------|
| `<->` | Euclidean distance (L2) | Spatial data, continuous features |
| `<#>` | Inner product | Normalized vectors, recommendations |
| `<=>` | Cosine distance | Text embeddings, semantic similarity |

### Index Types

| Index Type | Description | Best For |
|------------|-------------|----------|
| `ivfflat` | Inverted File with Flat Compression | Fast approximate search, large datasets |
| `hnsw` | Hierarchical Navigable Small World | High accuracy, moderate datasets |

### Operator Classes

| Operator Class | Distance Metric | Index Types |
|----------------|-----------------|-------------|
| `vector_l2_ops` | Euclidean distance | ivfflat, hnsw |
| `vector_ip_ops` | Inner product | ivfflat, hnsw |
| `vector_cosine_ops` | Cosine distance | ivfflat, hnsw |

## 💡 Example Use Cases

### Semantic Document Search

```sql
-- Create documents table
CREATE TABLE documents (
  id SERIAL PRIMARY KEY,
  title TEXT,
  content TEXT,
  embedding VECTOR(384)
);

-- Create index for fast similarity search
CREATE INDEX ON documents USING ivfflat (embedding vector_cosine_ops);

-- Search for similar documents
SELECT title, content, embedding <=> $1 AS similarity
FROM documents 
ORDER BY similarity 
LIMIT 10;
```

### Product Recommendations

```sql
-- Create products table
CREATE TABLE products (
  id SERIAL PRIMARY KEY,
  name TEXT,
  category TEXT,
  price DECIMAL,
  features VECTOR(128)
);

-- Find similar products
SELECT p.name, p.price, p.features <-> $1 AS distance
FROM products p
WHERE p.category = 'Electronics'
ORDER BY distance 
LIMIT 5;
```

### Anomaly Detection

```sql
-- Find outliers based on distance threshold
SELECT id, name, features <-> $1 AS distance
FROM products 
WHERE features <-> $1 > 2.0  -- Threshold for outliers
ORDER BY distance DESC;
```

## 🔗 Client Library Integration

### Python with psycopg2

```python
import psycopg2
import numpy as np

# Connect to Orbit pgvector server
conn = psycopg2.connect(
    host="127.0.0.1",
    port=5433,
    database="postgres",
    user="postgres"
)

# Create table with vector column
cur = conn.cursor()
cur.execute("""
    CREATE TABLE IF NOT EXISTS embeddings (
        id SERIAL PRIMARY KEY,
        text TEXT,
        vector VECTOR(384)
    )
""")

# Insert vector data
embedding = np.random.rand(384).tolist()
cur.execute("""
    INSERT INTO embeddings (text, vector) VALUES (%s, %s)
""", ("Sample text", str(embedding)))

# Similarity search
query_vector = np.random.rand(384).tolist()
cur.execute("""
    SELECT text, vector <=> %s AS distance 
    FROM embeddings 
    ORDER BY distance 
    LIMIT 5
""", (str(query_vector),))

results = cur.fetchall()
conn.commit()
```

### Node.js with pg

```javascript
const { Client } = require('pg');

// Connect to Orbit pgvector server
const client = new Client({
  host: '127.0.0.1',
  port: 5433,
  database: 'postgres',
  user: 'postgres'
});

await client.connect();

// Create table
await client.query(`
  CREATE TABLE IF NOT EXISTS documents (
    id SERIAL PRIMARY KEY,
    content TEXT,
    embedding VECTOR(384)
  )
`);

// Insert with embedding
const embedding = Array.from({length: 384}, () => Math.random());
await client.query(
  'INSERT INTO documents (content, embedding) VALUES ($1, $2)',
  ['Sample document', JSON.stringify(embedding)]
);

// Similarity search
const queryVector = Array.from({length: 384}, () => Math.random());
const result = await client.query(`
  SELECT content, embedding <=> $1 AS distance 
  FROM documents 
  ORDER BY distance 
  LIMIT 5
`, [JSON.stringify(queryVector)]);

console.log(result.rows);
```

## 🎯 Performance Tips

1. **Choose the Right Index**: Use `ivfflat` for speed, `hnsw` for accuracy
2. **Optimize Vector Dimensions**: Higher dimensions = more memory/compute
3. **Use Appropriate Operators**: Cosine for embeddings, L2 for features
4. **Index Tuning**: Adjust index parameters based on your dataset size
5. **Batch Operations**: Insert vectors in batches for better performance

## 🔄 Migration from pgvector

If you're migrating from a standard PostgreSQL + pgvector setup:

1. **Schema**: No changes needed - same SQL syntax
2. **Queries**: Identical query patterns work out of the box
3. **Indexes**: Same index creation commands supported
4. **Functions**: All pgvector functions available
5. **Data**: Export/import using standard PostgreSQL tools

## 🧪 Testing

```bash
# Run tests
cargo test --example pgvector-store

# Test with different PostgreSQL clients
psql -h 127.0.0.1 -p 5433 -d postgres -U postgres
pgbench -h 127.0.0.1 -p 5433 -d postgres -U postgres
```

## 📈 Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│ PostgreSQL      │    │ Orbit pgvector   │    │ Orbit Cluster   │
│ Client (psql,   │◄──►│ Wire Protocol    │◄──►│ VectorActors    │
│ pgAdmin, libs)  │    │ Server           │    │ (Distributed)   │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

- **PostgreSQL Wire Protocol**: Full compatibility with PG clients
- **Vector Query Engine**: Translates pgvector SQL to Orbit operations  
- **VectorActors**: Distributed vector storage and similarity search
- **Orbit Cluster**: Scalable, fault-tolerant vector operations

## 🚀 Advanced Features

- **Distributed Vector Storage**: Vectors stored across multiple nodes
- **Fault Tolerance**: Actor supervision and recovery
- **Horizontal Scaling**: Add nodes to increase capacity
- **Real-time Updates**: Insert/update vectors without downtime
- **ACID Compliance**: Transactional vector operations

This pgvector-compatible interface makes Orbit-RS a drop-in replacement for PostgreSQL + pgvector while providing the benefits of a distributed, scalable architecture.