# GraphRAG PostgreSQL Integration

This document describes how to use GraphRAG functionality through PostgreSQL-compatible SQL function calls in Orbit-RS.

## Overview

The PostgreSQL wire protocol in Orbit-RS includes support for GraphRAG operations through SQL function calls. This allows you to build and query knowledge graphs using familiar PostgreSQL syntax and any PostgreSQL-compatible client (psql, pgAdmin, applications using pg drivers, etc.).

## Setup

To enable GraphRAG support in your PostgreSQL server:

```rust
use orbit_protocols::postgres_wire::{PostgresServer, QueryEngine};
use orbit_client::OrbitClient;

// Create query engine with GraphRAG support
let orbit_client = OrbitClient::new(/* config */);
let query_engine = QueryEngine::new_with_vector_support(orbit_client);
let server = PostgresServer::new("127.0.0.1:5433", query_engine);
```

## GraphRAG SQL Functions

### 1. GRAPHRAG_BUILD - Build Knowledge Graph

Processes a document and builds knowledge graph entities and relationships.

**Syntax:**
```sql
SELECT * FROM GRAPHRAG_BUILD(
    kg_name TEXT,           -- Knowledge graph name/identifier
    document_id TEXT,       -- Unique document identifier
    document_text TEXT,     -- Document content to process
    metadata JSON           -- Optional: document metadata as JSON
);
```

**Example:**
```sql
-- Build knowledge graph from a research paper
SELECT * FROM GRAPHRAG_BUILD(
    'research_papers',
    'paper_001',
    'Machine learning is a subset of artificial intelligence that focuses on algorithms...',
    '{"author": "John Doe", "year": 2023, "journal": "AI Review"}'::json
);
```

**Returns:**
- `kg_name`: Knowledge graph name
- `document_id`: Document identifier
- `entities_extracted`: Number of entities found
- `relationships_extracted`: Number of relationships found
- `extractors_used`: Number of extractors applied
- `processing_time_ms`: Processing duration

### 2. GRAPHRAG_QUERY - RAG Query

Performs Retrieval-Augmented Generation queries against the knowledge graph.

**Syntax:**
```sql
SELECT * FROM GRAPHRAG_QUERY(
    kg_name TEXT,                -- Knowledge graph name
    query_text TEXT,             -- Query/question to answer
    max_hops INTEGER,            -- Optional: max relationship hops (default: 3)
    context_size INTEGER,        -- Optional: context window size (default: 2048)
    llm_provider TEXT,           -- Optional: LLM provider ('ollama', 'openai', etc.)
    include_explanation BOOLEAN  -- Optional: include reasoning explanation
);
```

**Example:**
```sql
-- Query knowledge graph about machine learning
SELECT * FROM GRAPHRAG_QUERY(
    'research_papers',
    'What are the main applications of machine learning?',
    3,
    2048,
    'ollama',
    true
);
```

**Returns:**
- `kg_name`: Knowledge graph name
- `query_text`: Original query
- `response`: Generated answer
- `confidence`: Confidence score (0.0 to 1.0)
- `processing_time_ms`: Query processing time
- `entities_involved`: JSON array of entities used
- `citations`: JSON array of source citations

### 3. GRAPHRAG_EXTRACT - Entity Extraction

Extracts entities and relationships from text without building the full knowledge graph.

**Syntax:**
```sql
SELECT * FROM GRAPHRAG_EXTRACT(
    kg_name TEXT,        -- Knowledge graph context
    document_id TEXT,    -- Document identifier
    text TEXT           -- Text to extract from
);
```

**Example:**
```sql
-- Extract entities from a news article
SELECT * FROM GRAPHRAG_EXTRACT(
    'news_kg',
    'article_2023_001',
    'Apple Inc. announced new iPhone models at their Cupertino headquarters yesterday...'
);
```

**Returns:**
- `kg_name`: Knowledge graph name
- `document_id`: Document identifier
- `entities_extracted`: Number of entities found
- `relationships_extracted`: Number of relationships found
- `processing_time_ms`: Extraction duration

### 4. GRAPHRAG_REASON - Multi-hop Reasoning

Finds reasoning paths between two entities in the knowledge graph.

**Syntax:**
```sql
SELECT * FROM GRAPHRAG_REASON(
    kg_name TEXT,                -- Knowledge graph name
    from_entity TEXT,            -- Starting entity
    to_entity TEXT,              -- Target entity
    max_hops INTEGER,            -- Optional: maximum hops to search
    include_explanation BOOLEAN  -- Optional: include reasoning explanation
);
```

**Example:**
```sql
-- Find connection between Apple and iPhone
SELECT * FROM GRAPHRAG_REASON(
    'tech_companies',
    'Apple Inc.',
    'iPhone',
    4,
    true
);
```

**Returns (one row per path found):**
- `kg_name`: Knowledge graph name
- `from_entity`: Starting entity
- `to_entity`: Target entity
- `path_nodes`: JSON array of entities in the path
- `path_relationships`: JSON array of relationships in the path
- `score`: Path relevance score
- `length`: Number of hops in the path
- `explanation`: Human-readable path explanation

### 5. GRAPHRAG_STATS - Knowledge Graph Statistics

Retrieves statistics and metrics for a knowledge graph.

**Syntax:**
```sql
SELECT * FROM GRAPHRAG_STATS(kg_name TEXT);
```

**Example:**
```sql
-- Get statistics for research papers knowledge graph
SELECT * FROM GRAPHRAG_STATS('research_papers');
```

**Returns:**
- `kg_name`: Knowledge graph name
- `documents_processed`: Total documents processed
- `rag_queries_executed`: Number of RAG queries performed
- `reasoning_queries_executed`: Number of reasoning queries performed
- `entities_extracted`: Total entities extracted
- `relationships_extracted`: Total relationships extracted
- `avg_document_processing_time_ms`: Average document processing time
- `avg_rag_query_time_ms`: Average RAG query time
- `rag_success_rate`: Success rate for RAG queries (0.0 to 1.0)

### 6. GRAPHRAG_ENTITIES - List Entities

Lists entities in the knowledge graph with optional filtering.

**Syntax:**
```sql
SELECT * FROM GRAPHRAG_ENTITIES(
    kg_name TEXT,        -- Knowledge graph name
    entity_type TEXT,    -- Optional: filter by entity type
    limit INTEGER        -- Optional: maximum results to return
);
```

**Example:**
```sql
-- Get all person entities, limit to 10
SELECT * FROM GRAPHRAG_ENTITIES('news_kg', 'Person', 10);
```

**Returns:**
- `kg_name`: Knowledge graph name
- `entity_type`: Requested entity type filter
- `limit_requested`: Requested result limit
- `total_count`: Total matching entities

### 7. GRAPHRAG_SIMILAR - Find Similar Entities

Finds entities similar to a given entity using vector similarity.

**Syntax:**
```sql
SELECT * FROM GRAPHRAG_SIMILAR(
    kg_name TEXT,        -- Knowledge graph name
    entity_name TEXT,    -- Reference entity
    limit INTEGER,       -- Optional: maximum results
    threshold FLOAT      -- Optional: similarity threshold (0.0 to 1.0)
);
```

**Example:**
```sql
-- Find entities similar to "Apple Inc."
SELECT * FROM GRAPHRAG_SIMILAR('tech_companies', 'Apple Inc.', 5, 0.8);
```

**Returns:**
- `kg_name`: Knowledge graph name
- `entity_name`: Reference entity
- `limit_requested`: Requested result limit
- `threshold_used`: Similarity threshold used
- `similar_count`: Number of similar entities found

## Usage Patterns

### 1. Document Processing Pipeline

```sql
-- Step 1: Build knowledge graph from documents
SELECT * FROM GRAPHRAG_BUILD('scientific_papers', 'paper_001', $document_text, $metadata);

-- Step 2: Query the knowledge graph
SELECT response, confidence FROM GRAPHRAG_QUERY('scientific_papers', 'What are the key findings?', 3, 2048, 'ollama', true);

-- Step 3: Get statistics
SELECT * FROM GRAPHRAG_STATS('scientific_papers');
```

### 2. Entity Relationship Analysis

```sql
-- Extract entities from text
SELECT * FROM GRAPHRAG_EXTRACT('company_news', 'news_001', $article_text);

-- Find relationships between companies
SELECT * FROM GRAPHRAG_REASON('company_news', 'Apple Inc.', 'Samsung Electronics', 5, true);

-- Find similar companies
SELECT * FROM GRAPHRAG_SIMILAR('company_news', 'Apple Inc.', 10, 0.7);
```

### 3. Knowledge Base Queries

```sql
-- Build comprehensive knowledge base
INSERT INTO processed_docs 
SELECT kg_name, document_id, entities_extracted, relationships_extracted 
FROM GRAPHRAG_BUILD('kb_main', 'doc_' || generate_series(1,1000), document_content, metadata)
WHERE document_content IS NOT NULL;

-- Query with high confidence threshold
SELECT query_text, response, confidence 
FROM GRAPHRAG_QUERY('kb_main', $user_question, 4, 4096, 'ollama', true)
WHERE confidence > 0.8;
```

## Client Examples

### Using psql

```bash

# Connect to Orbit PostgreSQL server
psql -h localhost -p 5433 -U orbit -d orbit

# Build knowledge graph
orbit=# SELECT * FROM GRAPHRAG_BUILD('docs', 'readme', 'Orbit is an actor framework...', '{}'::json);

# Query the knowledge graph
orbit=# SELECT response FROM GRAPHRAG_QUERY('docs', 'What is Orbit?', 3, 1024, 'ollama', false);
```

### Using Python (psycopg2)

```python
import psycopg2
import json

# Connect to Orbit
conn = psycopg2.connect(
    host="localhost",
    port=5433,
    database="orbit",
    user="orbit"
)
cur = conn.cursor()

# Build knowledge graph
doc_text = "Your document content here..."
metadata = json.dumps({"source": "web", "date": "2023-12-01"})

cur.execute("""
    SELECT * FROM GRAPHRAG_BUILD(%s, %s, %s, %s::json)
""", ("my_kg", "doc_1", doc_text, metadata))

result = cur.fetchone()
print(f"Processed: {result[2]} entities, {result[3]} relationships")

# Query knowledge graph
cur.execute("""
    SELECT response, confidence FROM GRAPHRAG_QUERY(%s, %s, 3, 2048, 'ollama', true)
""", ("my_kg", "What are the main topics discussed?"))

response, confidence = cur.fetchone()
print(f"Answer: {response} (confidence: {confidence})")

conn.close()
```

### Using Node.js (pg)

```javascript
const { Client } = require('pg');

const client = new Client({
    host: 'localhost',
    port: 5433,
    database: 'orbit',
    user: 'orbit'
});

async function queryKnowledgeGraph() {
    await client.connect();
    
    // Build knowledge graph
    const buildResult = await client.query(
        `SELECT * FROM GRAPHRAG_BUILD($1, $2, $3, $4::json)`,
        ['articles', 'article_1', documentText, JSON.stringify(metadata)]
    );
    
    console.log('Build result:', buildResult.rows[0]);
    
    // Query knowledge graph
    const queryResult = await client.query(
        `SELECT response, confidence FROM GRAPHRAG_QUERY($1, $2, 3, 2048, 'ollama', true)`,
        ['articles', 'Summarize the main points']
    );
    
    console.log('Query result:', queryResult.rows[0]);
    
    await client.end();
}

queryKnowledgeGraph().catch(console.error);
```

## Error Handling

GraphRAG functions return PostgreSQL-compatible errors:

```sql
-- Invalid knowledge graph name
orbit=# SELECT * FROM GRAPHRAG_QUERY('nonexistent_kg', 'test query', 3, 1024, 'ollama', false);
ERROR:  Actor error: Knowledge graph 'nonexistent_kg' not found

-- Invalid function syntax
orbit=# SELECT * FROM GRAPHRAG_BUILD('kg');  -- Missing required parameters
ERROR:  Invalid GRAPHRAG_BUILD function syntax
```

## Performance Considerations

1. **Document Processing**: Large documents may take significant time to process. Consider breaking them into smaller chunks.

2. **Query Context Size**: Larger context sizes provide better answers but increase processing time and memory usage.

3. **Max Hops**: Higher hop counts in reasoning queries increase computation time exponentially.

4. **Caching**: The GraphRAG actor system includes intelligent caching for repeated queries.

5. **Parallel Processing**: Multiple GraphRAG functions can run concurrently on different knowledge graphs.

## Integration with BI Tools

Since GraphRAG functions return standard PostgreSQL result sets, they can be used with any PostgreSQL-compatible BI tool:

- **Grafana**: Create dashboards showing knowledge graph statistics
- **Tableau**: Visualize entity relationships and query results  
- **Power BI**: Build reports on document processing metrics
- **Metabase**: Create interactive knowledge base queries

Example Grafana query:
```sql
SELECT 
    kg_name,
    documents_processed,
    entities_extracted,
    avg_rag_query_time_ms
FROM GRAPHRAG_STATS('$kg_name') 
WHERE $__timeFilter(created_at)
```

This integration makes GraphRAG functionality accessible through the familiar SQL interface while maintaining the power and flexibility of the underlying actor-based system.