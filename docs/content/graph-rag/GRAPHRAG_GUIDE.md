---
layout: default
title: "GraphRAG Guide"
subtitle: "Graph-augmented retrieval in Orbit-RS"
category: "graph-rag"
---

# GraphRAG - Graph-Augmented Retrieval

Orbit-RS implements GraphRAG for intelligent knowledge retrieval combining graph traversal with RAG (Retrieval-Augmented Generation).

---

## Overview

GraphRAG enhances traditional RAG by:

1. **Graph Context**: Leverages relationships between entities
2. **Multi-hop Reasoning**: Follows entity relationships for deeper understanding
3. **Semantic Search**: Vector similarity with graph structure
4. **Knowledge Extraction**: Automatic entity and relationship extraction

---

## Architecture

```
Query Input --> Embedding --> Vector Search --> Graph Expansion --> Context Assembly
                                    |                |
                              Top-K Chunks    Related Entities
                                    |                |
                                    +-------> Combined Context --> LLM Response
```

---

## Creating Knowledge Graphs

### Schema Definition

```cypher
// Create node types
CREATE CONSTRAINT ON (c:Concept) ASSERT c.name IS UNIQUE;
CREATE CONSTRAINT ON (d:Document) ASSERT d.id IS UNIQUE;
CREATE CONSTRAINT ON (e:Entity) ASSERT e.name IS UNIQUE;

// Create indexes
CREATE INDEX FOR (c:Concept) ON (c.embedding);
CREATE INDEX FOR (d:Document) ON (d.embedding);
```

### Entity Extraction

```sql
-- Automatic entity extraction from documents
SELECT GRAPHRAG_EXTRACT_ENTITIES(content)
FROM documents
WHERE processed = false;

-- Results: List of (entity_type, entity_name, confidence)
```

### Relationship Building

```cypher
// Create relationships between entities
MATCH (e1:Entity {name: 'Machine Learning'})
MATCH (e2:Entity {name: 'Neural Networks'})
CREATE (e1)-[:RELATED_TO {weight: 0.9}]->(e2);

// Automatic relationship extraction
CALL graphrag.buildRelationships('documents', 'entities');
```

---

## Querying GraphRAG

### Basic Query

```sql
SELECT * FROM GRAPHRAG_QUERY(
    query := 'What is the impact of AI on healthcare?',
    top_k := 5,
    max_hops := 2
);
```

### Advanced Query Options

```sql
SELECT * FROM GRAPHRAG_QUERY(
    query := 'Explain machine learning',
    top_k := 10,
    max_hops := 3,
    similarity_threshold := 0.7,
    include_metadata := true,
    filter := 'category = "technology"'
);
```

### Multi-hop Reasoning

```cypher
// Find all related concepts within 3 hops
MATCH path = (start:Concept)-[*1..3]-(related)
WHERE start.name = 'Artificial Intelligence'
  AND GRAPHRAG_SIMILARITY(start.embedding, $query_embedding) > 0.7
RETURN path, nodes(path), relationships(path)
ORDER BY length(path)
LIMIT 20;
```

---

## Vector Integration

### Creating Embeddings

```sql
-- Add embedding column
ALTER TABLE documents ADD COLUMN embedding VECTOR(1536);

-- Generate embeddings (uses configured embedding model)
UPDATE documents
SET embedding = GRAPHRAG_EMBED(content)
WHERE embedding IS NULL;
```

### Similarity Search with Graph Context

```sql
-- Find similar documents and expand with graph
WITH similar AS (
    SELECT id, content, embedding <=> $query_embedding AS distance
    FROM documents
    ORDER BY distance
    LIMIT 5
)
SELECT s.*, g.related_entities
FROM similar s
CROSS JOIN GRAPHRAG_EXPAND(s.id, max_hops := 2) g;
```

---

## Cypher Integration

### Graph Queries with RAG

```cypher
// Semantic search with graph traversal
MATCH (d:Document)
WHERE GRAPHRAG_SIMILARITY(d.embedding, $query_embedding) > 0.8
MATCH (d)-[:MENTIONS]->(e:Entity)
MATCH (e)-[:RELATED_TO*1..2]->(related:Entity)
RETURN d, collect(DISTINCT e), collect(DISTINCT related)
LIMIT 10;
```

### Building Knowledge Graphs from Documents

```cypher
// Process document and create graph
CALL graphrag.processDocument($document_id) YIELD entities, relationships
WITH entities, relationships
UNWIND entities AS entity
MERGE (e:Entity {name: entity.name, type: entity.type})
SET e.embedding = entity.embedding;
```

---

## Storage Configuration

### GraphRAG Storage Options

```toml
# orbit-server.toml
[graphrag]
enabled = true
vector_index = "hnsw"
embedding_model = "text-embedding-3-small"
embedding_dimension = 1536

[graphrag.storage]
graph_backend = "rocksdb"  # rocksdb, memory
vector_backend = "rocksdb"
cache_size = "2GB"

[graphrag.extraction]
auto_extract = true
batch_size = 100
confidence_threshold = 0.7
```

---

## Performance Optimization

### Index Configuration

```sql
-- HNSW index for vectors
CREATE INDEX idx_doc_embedding ON documents
USING hnsw (embedding vector_cosine_ops)
WITH (m = 32, ef_construction = 200);

-- Graph indexes for traversal
CREATE INDEX FOR (e:Entity) ON (e.type, e.name);
CREATE INDEX FOR ()-[r:RELATED_TO]-() ON (r.weight);
```

### Query Optimization

```sql
-- Use graph expansion only when needed
SELECT * FROM GRAPHRAG_QUERY(
    query := $query,
    -- Pre-filter before graph expansion
    pre_filter := 'category = "tech"',
    -- Limit graph depth based on query complexity
    max_hops := CASE
        WHEN length($query) < 50 THEN 1
        ELSE 2
    END
);
```

---

## API Reference

### Functions

| Function | Description |
|----------|-------------|
| `GRAPHRAG_QUERY(query, options)` | Full GraphRAG query |
| `GRAPHRAG_EMBED(text)` | Generate embedding |
| `GRAPHRAG_SIMILARITY(v1, v2)` | Compute similarity |
| `GRAPHRAG_EXPAND(id, hops)` | Expand graph from node |
| `GRAPHRAG_EXTRACT_ENTITIES(text)` | Extract entities |

### Cypher Procedures

| Procedure | Description |
|-----------|-------------|
| `graphrag.processDocument(id)` | Process document |
| `graphrag.buildRelationships(...)` | Build relationships |
| `graphrag.rebuildIndex(...)` | Rebuild vector index |
| `graphrag.stats()` | Get GraphRAG statistics |

---

## Resources

- **Source**: `orbit/server/src/protocols/cypher/graphrag/`
- **Examples**: `orbit-examples/graphrag-examples/`
- **RFC**: [Vector Database RFC](../rfcs/RFC_INDEX.md#rfc-009-vector-database-capabilities)
