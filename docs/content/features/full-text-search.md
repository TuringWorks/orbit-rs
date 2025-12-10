# Multi-Model Full-Text Search

Orbit-RS provides a unified, high-performance Full-Text Search (FTS) engine powered by **Tantivy**. This engine is integrated across all supported protocols, allowing you to use protocol-native syntax while benefiting from a consistent, high-speed search backend.

## Architecture

The FTS engine is built on top of [Tantivy](https://github.com/quickwit-oss/tantivy), a high-performance full-text search engine library inspired by Apache Lucene.

Key components:
- **Index Management**: Indexes are stored in the configured `index_dir` and managed by the `FtsEngine`.
- **Query Parsing**: A unified `QueryParser` translates protocol-specific queries (e.g., Postgres `tsquery`, MongoDB `$text`) into Tantivy's internal query representation.
- **Scoring**: Uses BM25 scoring by default, with support for field boosting and custom ranking.

## Supported Protocols

### PostgreSQL

Orbit-RS implements PostgreSQL's native FTS types and functions, powered by the Tantivy backend.

**Features:**
- Types: `tsvector`, `tsquery`
- Operators: `@@` (Match)
- Functions: `to_tsvector`, `to_tsquery`, `plainto_tsquery`
- Indexing: GIN index support

**Usage Example:**

```sql
-- Create a table with a text column
CREATE TABLE articles (
    id SERIAL PRIMARY KEY,
    title TEXT,
    body TEXT
);

-- Create a GIN index (automatically uses Tantivy)
CREATE INDEX article_fts ON articles USING GIN (to_tsvector('english', body));

-- Search using @@ operator
SELECT title 
FROM articles 
WHERE to_tsvector('english', body) @@ to_tsquery('english', 'search & engine');
```

**Query Syntax:**
- `&` : AND
- `|` : OR
- `!` : NOT
- `( )` : Grouping

### MySQL

Full-text search in MySQL mode supports the `MATCH() ... AGAINST()` syntax, specifically in **Boolean Mode**.

**Features:**
- Boolean mode operators
- Relevance scoring

**Usage Example:**

```sql
CREATE TABLE products (
    id INT PRIMARY KEY,
    name VARCHAR(255),
    description TEXT,
    FULLTEXT (name, description)
);

-- Search using Boolean Mode
SELECT * 
FROM products 
WHERE MATCH(name, description) AGAINST('+database -sql' IN BOOLEAN MODE);
```

**Query Syntax:**
- `+` : Word must be present.
- `-` : Word must not be present.
- (no operator) : Word is optional (increases relevance).

### MongoDB

MongoDB support includes text indexes and the `$text` query operator.

**Features:**
- Text indexes with field weights
- `$text` operator with `$search`
- `textScore` projection for sorting

**Usage Example:**

```javascript
// Create text index
db.collection.createIndex({ content: "text" });

// Search
db.collection.find(
    { $text: { $search: "orbit search" } },
    { score: { $meta: "textScore" } }
).sort(
    { score: { $meta: "textScore" } }
);
```

### Redis (RESP)

Orbit-RS supports the RediSearch `FT.SEARCH` command family (partially compatible).

**Features:**
- `FT.CREATE`: Create an index
- `FT.SEARCH`: Execute search queries

**Usage Example:**

```bash
# Create index
FT.CREATE myIdx ON HASH PREFIX 1 doc: SCHEMA title TEXT weight 5.0 body TEXT

# Search
FT.SEARCH myIdx "hello world" LIMIT 0 10
```

## Configuration

FTS configuration is handled in the main Orbit-RS configuration file:

```toml
[fts]
index_dir = "./data/indexes"
max_memory = 100_000_000 # 100 MB
```
