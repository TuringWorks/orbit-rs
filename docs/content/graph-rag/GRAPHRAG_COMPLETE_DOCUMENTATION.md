# GraphRAG (Graph-enhanced Retrieval-Augmented Generation) - Complete Documentation

**Last Updated**: January 2025  
**Status**: ✅ **Production Ready**

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Overview](#overview)
3. [Architecture](#architecture)
4. [RESP Protocol Commands](#resp-protocol-commands)
5. [PostgreSQL Integration](#postgresql-integration)
6. [Usage Examples](#usage-examples)
7. [Performance Optimizations](#performance-optimizations)
8. [Implementation Phases](#implementation-phases)
9. [Contributing](#contributing)

---

## Executive Summary

GraphRAG (Graph-enhanced Retrieval-Augmented Generation) provides comprehensive knowledge graph construction and querying capabilities in Orbit-RS, leveraging the existing distributed actor system, graph database (GRAPH.*), vector store (VECTOR.*), and search engine (FT.*) infrastructure.

### Key Features

✅ **Knowledge Graph Construction**
- Entity extraction from documents
- Relationship identification
- Multi-extractor support
- Entity deduplication

✅ **RAG Query Processing**
- Graph traversal with LLM generation
- Multi-hop reasoning
- Hybrid search (graph + vector + text)
- Citation support

✅ **Multiple Interfaces**
- RESP protocol commands (GRAPHRAG.*)
- PostgreSQL SQL functions
- AQL function calls (GRAPHRAG_*)
- Cypher/Bolt stored procedures (orbit.graphrag.*)
- REST API support

✅ **Advanced Analytics Functions** (NEW - November 2025)
- Entity similarity search (embedding and text-based)
- Semantic search with RAG integration
- Entity listing with filtering
- Trend analysis over time windows
- Community detection using connected components

---

## Overview

GraphRAG combines graph databases, vector stores, and LLM generation to provide powerful knowledge graph construction and querying capabilities. It extends Orbit-RS's existing infrastructure with specialized actors and commands.

### Core Principles

1. **Leverage Existing Infrastructure**: Build upon existing actors and protocols
2. **Maintain Redis Compatibility**: Extend RESP protocol with GraphRAG.* commands
3. **Distributed by Design**: Support scaling across multiple nodes
4. **Multi-Modal Integration**: Combine graph, vector, and text search seamlessly
5. **LLM-Ready**: Design for integration with various LLM providers

---

## Architecture

### GraphRAG Actor Ecosystem

```text
┌─────────────────────────────────────────────────────────────────┐
│                     GraphRAG Orchestrator                       │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │ Knowledge Graph │  │   RAG Pipeline  │  │ Query Processor │  │
│  │   Constructor   │  │     Manager     │  │    & Router     │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                                │
                ┌───────────────┼────---───────────┐
                │               │                  │
┌─────────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│   GraphActor        │ │  VectorActor    │ │  SearchActor    │
│   (Enhanced)        │ │   (Enhanced)    │ │   (Enhanced)    │
│                     │ │                 │ │                 │
│ • Cypher Queries    │ │ • Embeddings    │ │ • Text Search   │
│ • Entity Relations  │ │ • Similarity    │ │ • Indexing      │
│ • Multi-hop Paths   │ │ • KNN Search    │ │ • Ranking       │
│ • Graph Algorithms  │ │ • Metadata      │ │ • Filtering     │
│ • Vector Properties │ │ • Clustering    │ │ • Analysis      │
└─────────────────────┘ └─────────────────┘ └─────────────────┘
```

### New Components

#### 1. GraphRAGActor

**Purpose**: Orchestrates knowledge graph construction, entity extraction, and RAG operations.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRAGActor {
    pub kg_name: String,
    pub config: GraphRAGConfig,
    pub stats: GraphRAGStats,
    pub entity_extractors: Vec<EntityExtractor>,
    pub embedding_models: Vec<EmbeddingModel>,
    pub llm_providers: Vec<LLMProvider>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRAGConfig {
    pub max_entities_per_document: usize,
    pub max_relationships_per_document: usize,
    pub embedding_dimension: usize,
    pub similarity_threshold: f32,
    pub max_hops: u32,
    pub rag_context_size: usize,
    pub enable_entity_deduplication: bool,
    pub enable_relationship_inference: bool,
}
```

**Key Methods**:
- `process_document()` - Extract entities/relationships and build knowledge graph
- `query_rag()` - Perform retrieval-augmented generation queries
- `traverse_graph()` - Multi-hop reasoning across relationships
- `semantic_search()` - Vector similarity search across graph entities
- `hybrid_search()` - Combine graph traversal, vector search, and text search

#### 2. EntityExtractionActor

**Purpose**: NLP pipeline for identifying entities and relationships from text.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityExtractionActor {
    pub extractors: Vec<ExtractorConfig>,
    pub confidence_threshold: f32,
    pub deduplication_strategy: DeduplicationStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtractorConfig {
    NamedEntityRecognition { model_path: String },
    RelationshipExtraction { model_path: String },
    CustomPattern { regex_patterns: Vec<String> },
    LLMBased { provider: LLMProvider, prompt_template: String },
}
```

#### 3. Enhanced GraphNode with Vector Embeddings

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedGraphNode {
    pub id: String,
    pub labels: Vec<String>,
    pub properties: HashMap<String, serde_json::Value>,
    
    // GraphRAG Extensions
    pub embeddings: HashMap<String, Vec<f32>>, // Multiple embeddings per model
    pub entity_type: Option<EntityType>,
    pub confidence_score: f32,
    pub source_documents: Vec<String>,
    pub last_updated: i64,
}
```

---

## RESP Protocol Commands

GraphRAG commands are available under the `GRAPHRAG.*` namespace via the RESP protocol.

### GRAPHRAG.BUILD

Build a knowledge graph from a document by extracting entities and relationships.

**Syntax:**
```text
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
```text
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
```text
GRAPHRAG.EXTRACT <kg_name> <document_id> <text> [EXTRACTORS extractor1 extractor2 ...]
```

**Example:**
```redis
GRAPHRAG.EXTRACT company_kg doc2 "Tesla Inc. was founded by Elon Musk in 2003." EXTRACTORS regex_ner keyword_extraction
```

### GRAPHRAG.REASON

Find reasoning paths between two specific entities in the knowledge graph.

**Syntax:**
```text
GRAPHRAG.REASON <kg_name> <from_entity> <to_entity> [MAX_HOPS <hops>] [INCLUDE_EXPLANATION]
```

**Example:**
```redis
GRAPHRAG.REASON company_kg "Apple Inc." "Steve Jobs" MAX_HOPS 2 INCLUDE_EXPLANATION
```

### GRAPHRAG.STATS

Get statistics about a knowledge graph.

**Syntax:**
```text
GRAPHRAG.STATS <kg_name>
```

**Response:** Array with statistics
- `total_entities` - Total number of entities
- `total_relationships` - Total number of relationships
- `entity_types` - Breakdown by entity type
- `relationship_types` - Breakdown by relationship type
- `last_updated` - Timestamp of last update

### GRAPHRAG.ENTITIES

List entities in a knowledge graph.

**Syntax:**
```text
GRAPHRAG.ENTITIES <kg_name> [TYPE <type>] [LIMIT <count>] [OFFSET <offset>]
```

### GRAPHRAG.SIMILAR

Find similar entities using vector embeddings.

**Syntax:**
```text
GRAPHRAG.SIMILAR <kg_name> <entity_id> [LIMIT <count>] [THRESHOLD <score>]
```

---

## PostgreSQL Integration

GraphRAG functionality is available through PostgreSQL-compatible SQL function calls.

### Setup

```rust
use orbit_server::protocols::PostgresServer;
use orbit_protocols::postgres_wire::QueryEngine;

// Create query engine with GraphRAG support
let query_engine = QueryEngine::new_with_vector_support(orbit_client);
let server = PostgresServer::new("127.0.0.1:5433", query_engine);
```

### SQL Functions

#### 1. GRAPHRAG_BUILD

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
SELECT * FROM GRAPHRAG_BUILD(
    'research_papers',
    'paper_001',
    'Machine learning is a subset of artificial intelligence...',
    '{"author": "John Doe", "year": 2023}'::json
);
```

#### 2. GRAPHRAG_QUERY

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
SELECT * FROM GRAPHRAG_QUERY(
    'research_papers',
    'What are the main applications of machine learning?',
    3,
    2048,
    'ollama',
    true
);
```

#### 3. GRAPHRAG_EXTRACT

Extracts entities and relationships from text without building the full knowledge graph.

**Syntax:**
```sql
SELECT * FROM GRAPHRAG_EXTRACT(
    kg_name TEXT,        -- Knowledge graph context
    document_id TEXT,    -- Document identifier
    text TEXT           -- Text to extract from
);
```

#### 4. GRAPHRAG_REASON

Finds reasoning paths between two entities in the knowledge graph.

**Syntax:**
```sql
SELECT * FROM GRAPHRAG_REASON(
    kg_name TEXT,                -- Knowledge graph name
    from_entity TEXT,            -- Starting entity
    to_entity TEXT,              -- Target entity
    max_hops INTEGER,            -- Optional: max hops (default: 3)
    include_explanation BOOLEAN  -- Optional: include explanations
);
```

#### 5. GRAPHRAG_STATS

Returns statistics about a knowledge graph.

**Syntax:**
```sql
SELECT * FROM GRAPHRAG_STATS(kg_name TEXT);
```

#### 6. GRAPHRAG_ENTITIES

Lists entities in a knowledge graph.

**Syntax:**
```sql
SELECT * FROM GRAPHRAG_ENTITIES(
    kg_name TEXT,        -- Knowledge graph name
    entity_type TEXT,    -- Optional: filter by type
    limit INTEGER,       -- Optional: limit results
    offset INTEGER      -- Optional: offset for pagination
);
```

#### 7. GRAPHRAG_SIMILAR

Finds similar entities using vector embeddings.

**Syntax:**
```sql
SELECT * FROM GRAPHRAG_SIMILAR(
    kg_name TEXT,        -- Knowledge graph name
    entity_id TEXT,      -- Entity to find similar entities for
    limit INTEGER,       -- Optional: limit results (default: 10)
    threshold FLOAT      -- Optional: similarity threshold (default: 0.7)
);
```

---

## Usage Examples

### Building a Knowledge Graph

**Using RESP:**
```redis
GRAPHRAG.BUILD company_kg doc1 "Apple Inc. is a technology company founded by Steve Jobs."
```

**Using PostgreSQL:**
```sql
SELECT * FROM GRAPHRAG_BUILD('company_kg', 'doc1', 'Apple Inc. is a technology company...');
```

### Querying with RAG

**Using RESP:**
```redis
GRAPHRAG.QUERY company_kg "What is the relationship between Apple and technology?" MAX_HOPS 3
```

**Using PostgreSQL:**
```sql
SELECT * FROM GRAPHRAG_QUERY('company_kg', 'What is the relationship between Apple and technology?', 3);
```

### Entity Analysis

**Using RESP:**
```redis
GRAPHRAG.ENTITIES company_kg TYPE "Organization"
GRAPHRAG.SIMILAR company_kg "Apple Inc." LIMIT 5
```

**Using PostgreSQL:**
```sql
SELECT * FROM GRAPHRAG_ENTITIES('company_kg', 'Organization');
SELECT * FROM GRAPHRAG_SIMILAR('company_kg', 'Apple Inc.', 5);
```

---

## Performance Optimizations

### 1. Caching Strategy

- Entity extraction results cached
- Embedding vectors cached
- Graph traversal paths cached
- LLM responses cached (with TTL)

### 2. Batch Processing

- Batch entity extraction
- Batch embedding generation
- Batch graph updates

### 3. Distributed Processing

- Distributed entity extraction
- Distributed graph traversal
- Load balancing across nodes

---

## Implementation Phases

### Phase 1: Foundation (Current Sprint)
- GraphRAGActor implementation
- Basic entity extraction
- Simple graph construction

### Phase 2: Core GraphRAG Features
- RAG query processing
- Multi-hop reasoning
- Vector-enhanced nodes

### Phase 3: Advanced Features
- LLM provider integration
- Hybrid search
- Advanced reasoning

### Phase 4: Production Readiness
- Performance optimization
- Comprehensive testing
- Documentation completion

---

## Contributing

Contributions to GraphRAG are welcome! Areas needing help:

1. **Entity Extraction**: Improve NER and relationship extraction
2. **LLM Integration**: Add support for more LLM providers
3. **Performance**: Optimize graph traversal and vector search
4. **Testing**: Expand test coverage

---

## References

- [GraphRAG Complete Documentation](./GRAPHRAG_COMPLETE_DOCUMENTATION.md)
- [Orbit-RS Architecture](./architecture/ORBIT_ARCHITECTURE.md)
- [Vector Store Documentation](./VECTOR_STORE.md)
- [Graph Database Documentation](../graph/GRAPH_DATABASE.md)

---

**Last Updated**: January 2025  
**Maintainer**: Orbit-RS Development Team

