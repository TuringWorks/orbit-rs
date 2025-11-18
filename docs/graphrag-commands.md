---
layout: default
title: GraphRAG Commands Reference
category: documentation
---

# GraphRAG Commands Reference

This document describes the GraphRAG (Graph-enhanced Retrieval-Augmented Generation) commands available in Orbit-RS via the RESP protocol.

## Overview

GraphRAG commands are available under the `GRAPHRAG.*` namespace and provide comprehensive knowledge graph construction and querying capabilities:

- **GRAPHRAG.BUILD** - Process documents to extract entities and build knowledge graph
- **GRAPHRAG.QUERY** - Execute RAG queries with graph traversal and LLM generation
- **GRAPHRAG.EXTRACT** - Extract entities and relationships without building graph
- **GRAPHRAG.REASON** - Find reasoning paths between entities
- **GRAPHRAG.STATS** - Get statistics about knowledge graph
- **GRAPHRAG.ENTITIES** - List entities in knowledge graph
- **GRAPHRAG.SIMILAR** - Find similar entities using vector embeddings

## Command Details

### GRAPHRAG.BUILD

Build a knowledge graph from a document by extracting entities and relationships.

**Syntax:**

```
GRAPHRAG.BUILD <kg_name> <document_id> <text> [metadata key value ...]
```

**Parameters:**

- `kg_name` - Knowledge graph identifier
- `document_id` - Unique document identifier  
- `text` - Document text content
- `metadata` - Optional key-value pairs for document metadata

**Response:** Array with processing statistics

- `entities_extracted` - Number of entities extracted
- `relationships_extracted` - Number of relationships extracted
- `extractors_used` - Number of extractors applied
- `processing_time_ms` - Processing time in milliseconds

**Example:**

```redis
GRAPHRAG.BUILD company_kg doc1 "Apple Inc. is a technology company founded by Steve Jobs. Microsoft Corporation is another major technology company." company "Apple Inc." industry "Technology"
```

### GRAPHRAG.QUERY

Execute a RAG query that combines graph traversal with LLM generation.

**Syntax:**

```
GRAPHRAG.QUERY <kg_name> <query_text> [MAX_HOPS <hops>] [CONTEXT_SIZE <size>] [LLM_PROVIDER <provider>] [INCLUDE_EXPLANATION]
```

**Parameters:**

- `kg_name` - Knowledge graph identifier
- `query_text` - Natural language query
- `MAX_HOPS` - Maximum hops for graph traversal (optional)
- `CONTEXT_SIZE` - Maximum context size for LLM (optional)
- `LLM_PROVIDER` - LLM provider to use (optional)
- `INCLUDE_EXPLANATION` - Include reasoning paths in response (optional flag)

**Response:** Array with query results

- `response` - Generated LLM response
- `confidence` - Confidence score (0.0-1.0)
- `processing_time_ms` - Total processing time
- `entities_involved` - Array of entities involved in reasoning
- `reasoning_paths` - Array of reasoning paths (if INCLUDE_EXPLANATION specified)
- `citations` - Array of source citations

**Example:**

```redis
GRAPHRAG.QUERY company_kg "What is the relationship between Apple and technology?" MAX_HOPS 3 INCLUDE_EXPLANATION
```

### GRAPHRAG.EXTRACT

Extract entities and relationships from text without building the knowledge graph.

**Syntax:**

```
GRAPHRAG.EXTRACT <kg_name> <document_id> <text> [EXTRACTORS extractor1 extractor2 ...]
```

**Parameters:**

- `kg_name` - Knowledge graph identifier
- `document_id` - Unique document identifier
- `text` - Document text content  
- `EXTRACTORS` - Optional list of specific extractors to use

**Response:** Array with extraction statistics

- `entities_extracted` - Number of entities extracted
- `relationships_extracted` - Number of relationships extracted
- `processing_time_ms` - Processing time in milliseconds
- `warnings` - Array of warning messages (if any)

**Example:**

```redis
GRAPHRAG.EXTRACT company_kg doc2 "Tesla Inc. was founded by Elon Musk in 2003." EXTRACTORS regex_ner keyword_extraction
```

### GRAPHRAG.REASON

Find reasoning paths between two specific entities in the knowledge graph.

**Syntax:**

```
GRAPHRAG.REASON <kg_name> <from_entity> <to_entity> [MAX_HOPS <hops>] [INCLUDE_EXPLANATION]
```

**Parameters:**

- `kg_name` - Knowledge graph identifier
- `from_entity` - Starting entity name
- `to_entity` - Target entity name
- `MAX_HOPS` - Maximum hops to traverse (optional)
- `INCLUDE_EXPLANATION` - Include explanations for paths (optional flag)

**Response:** Array with reasoning results

- `from_entity` - Starting entity name
- `to_entity` - Target entity name
- `paths_found` - Number of reasoning paths discovered
- `paths` - Array of reasoning paths with nodes, relationships, scores, and explanations

**Example:**

```redis
GRAPHRAG.REASON company_kg "Apple Inc." "Steve Jobs" MAX_HOPS 2 INCLUDE_EXPLANATION
```

### GRAPHRAG.STATS

Get comprehensive statistics about a knowledge graph.

**Syntax:**

```
GRAPHRAG.STATS <kg_name>
```

**Parameters:**

- `kg_name` - Knowledge graph identifier

**Response:** Array with statistics

- `kg_name` - Knowledge graph name
- `documents_processed` - Total documents processed
- `rag_queries_executed` - Total RAG queries executed
- `reasoning_queries_executed` - Total reasoning queries executed
- `entities_extracted` - Total entities extracted
- `relationships_extracted` - Total relationships extracted
- `avg_document_processing_time_ms` - Average document processing time
- `avg_rag_query_time_ms` - Average RAG query time
- `avg_reasoning_query_time_ms` - Average reasoning query time
- `rag_success_rate` - RAG query success rate

**Example:**

```redis
GRAPHRAG.STATS company_kg
```

### GRAPHRAG.ENTITIES

List entities in the knowledge graph with optional filtering.

**Syntax:**

```
GRAPHRAG.ENTITIES <kg_name> [LIMIT <limit>] [ENTITY_TYPE <type>]
```

**Parameters:**

- `kg_name` - Knowledge graph identifier
- `LIMIT` - Maximum number of entities to return (optional)
- `ENTITY_TYPE` - Filter by entity type (optional)

**Response:** Array with entity information

- `kg_name` - Knowledge graph name
- `entities` - Array of entity details
- `total_count` - Total number of matching entities

**Example:**

```redis
GRAPHRAG.ENTITIES company_kg LIMIT 10 ENTITY_TYPE person
```

### GRAPHRAG.SIMILAR

Find entities similar to a given entity using vector embeddings.

**Syntax:**

```
GRAPHRAG.SIMILAR <kg_name> <entity_name> [LIMIT <limit>] [THRESHOLD <threshold>]
```

**Parameters:**

- `kg_name` - Knowledge graph identifier
- `entity_name` - Entity name to find similar entities for
- `LIMIT` - Maximum number of similar entities to return (optional)
- `THRESHOLD` - Similarity threshold (0.0-1.0, optional)

**Response:** Array with similarity results

- `entity_name` - Query entity name
- `kg_name` - Knowledge graph name
- `similar_entities` - Array of similar entities with similarity scores
- `threshold_used` - Similarity threshold applied

**Example:**

```redis
GRAPHRAG.SIMILAR company_kg "Apple Inc." LIMIT 5 THRESHOLD 0.8
```

## Usage Patterns

### Building a Knowledge Graph

1. First, build the knowledge graph from documents:

```redis
GRAPHRAG.BUILD my_kg doc1 "Your document content here..."
GRAPHRAG.BUILD my_kg doc2 "More document content..."
```

2. Check statistics:

```redis
GRAPHRAG.STATS my_kg
```

### Querying with RAG

1. Execute natural language queries:

```redis
GRAPHRAG.QUERY my_kg "What are the main relationships in the data?" INCLUDE_EXPLANATION
```

2. Find specific connections:

```redis
GRAPHRAG.REASON my_kg "Entity A" "Entity B" MAX_HOPS 3
```

### Entity Analysis

1. List entities by type:

```redis
GRAPHRAG.ENTITIES my_kg ENTITY_TYPE person LIMIT 20
```

2. Find similar entities:

```redis
GRAPHRAG.SIMILAR my_kg "Important Entity" THRESHOLD 0.7
```

## Performance Notes

- **Document Processing**: Entity extraction and graph construction are computationally intensive
- **RAG Queries**: Combine graph traversal, vector search, and LLM generation - expect higher latency
- **Caching**: Results are cached where possible to improve performance
- **Batch Processing**: Consider processing multiple documents before querying for better efficiency

## Error Handling

All GraphRAG commands return detailed error messages for common issues:

- Invalid knowledge graph names
- Malformed query text
- Missing required parameters
- Processing timeouts
- Resource limitations

Check the response for error messages and adjust parameters accordingly.

## Integration Examples

### Python (using redis-py)

```python
import redis

# Connect to Orbit-RS RESP server
client = redis.Redis(host='localhost', port=6379)

# Build knowledge graph
result = client.execute_command(
    'GRAPHRAG.BUILD', 'my_kg', 'doc1', 
    'Apple Inc. is a technology company founded by Steve Jobs.'
)
print(f"Entities extracted: {result[1]}")

# Query the graph
response = client.execute_command(
    'GRAPHRAG.QUERY', 'my_kg', 
    'Tell me about Apple Inc.',
    'INCLUDE_EXPLANATION'
)
print(f"Response: {response[1]}")
```

### Node.js (using ioredis)

```javascript
const Redis = require('ioredis');
const client = new Redis();

// Build knowledge graph
const buildResult = await client.call(
    'GRAPHRAG.BUILD', 'my_kg', 'doc1',
    'Microsoft Corporation is a technology company.'
);
console.log(`Relationships extracted: ${buildResult[3]}`);

// Query with reasoning
const queryResult = await client.call(
    'GRAPHRAG.QUERY', 'my_kg',
    'What companies are mentioned?',
    'MAX_HOPS', 2
);
console.log(`Query response: ${queryResult[1]}`);
```

This comprehensive GraphRAG implementation provides powerful knowledge graph construction and querying capabilities through familiar Redis-compatible commands.
