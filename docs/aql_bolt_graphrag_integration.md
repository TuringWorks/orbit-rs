---
layout: default
title: AQL and Bolt GraphRAG Integration Guide
category: documentation
---

## AQL and Bolt GraphRAG Integration Guide

## Overview

This document provides a comprehensive guide to using GraphRAG functionality through AQL (ArangoDB Query Language) and Bolt (Neo4j) protocols in Orbit-RS. These integrations make GraphRAG capabilities accessible through familiar graph database interfaces.

## Table of Contents

- [AQL GraphRAG Integration](#aql-graphrag-integration)
- [Bolt/Cypher GraphRAG Integration](#boltcypher-graphrag-integration)  
- [Comparison Matrix](#comparison-matrix)
- [Best Practices](#best-practices)
- [Performance Optimization](#performance-optimization)
- [Integration Examples](#integration-examples)

## AQL GraphRAG Integration

### AQL Integration Architecture

The AQL GraphRAG integration provides GraphRAG functionality through AQL function calls that can be used within standard ArangoDB-style queries.

```rust
use orbit_protocols::aql::{AqlQueryEngine, AqlValue};
use orbit_client::OrbitClient;

// Create query engine with GraphRAG support
let orbit_client = OrbitClient::new(/* config */);
let query_engine = AqlQueryEngine::new_with_graphrag(orbit_client);
```

### Core Functions

#### 1. Knowledge Graph Construction (Cypher)

##### GRAPHRAG_BUILD_KNOWLEDGE(document, options)

```aql
// Build from document text
FOR result IN GRAPHRAG_BUILD_KNOWLEDGE("Research paper text...", {
    "knowledge_graph": "research_kg",
    "extractors": ["entity_extraction", "relationship_extraction"],
    "build_graph": true,
    "generate_embeddings": true
})
RETURN result

// Build from document collection with metadata
FOR doc IN research_papers
    FILTER doc.year >= 2020
    FOR result IN GRAPHRAG_BUILD_KNOWLEDGE(doc, {
        "knowledge_graph": "recent_research",
        "document_id": doc._key,
        "metadata": {"year": doc.year, "journal": doc.journal}
    })
    RETURN {
        "document": doc._key,
        "entities_extracted": result.entities_extracted,
        "processing_time": result.processing_time_ms
    }
```

#### 2. RAG Queries

##### GRAPHRAG_QUERY(knowledge_graph, query_text, options)

```aql
// Simple RAG query
FOR answer IN GRAPHRAG_QUERY(
    "research_kg", 
    "What are recent developments in quantum computing?", {
    "max_hops": 3,
    "context_size": 2048,
    "llm_provider": "ollama",
    "include_explanation": true
})
RETURN {
    "question": "What are recent developments in quantum computing?",
    "answer": answer.response,
    "confidence": answer.confidence,
    "entities_used": answer.entities_involved
}

// RAG with result filtering and aggregation
FOR paper IN research_papers
    FILTER paper.field == "quantum_computing"
    FOR answer IN GRAPHRAG_QUERY("research_kg",
        CONCAT("Summarize findings from ", paper.title), {
        "max_hops": 2,
        "entity_filter": {"source_document": paper._key}
    })
    FILTER answer.confidence > 0.8
    COLLECT field = paper.field INTO summaries = {
        "paper": paper.title,
        "summary": answer.response,
        "confidence": answer.confidence
    }
    RETURN {
        "field": field,
        "high_confidence_summaries": summaries
    }
```

#### 3. Multi-hop Reasoning

#### GRAPHRAG_FIND_PATHS(knowledge_graph, from_entity, to_entity, options)

```aql
// Find connection paths
FOR path IN GRAPHRAG_FIND_PATHS("tech_kg", "Apple Inc.", "iPhone", {
    "max_hops": 4,
    "include_explanation": true,
    "max_results": 5
})
SORT path.score DESC
RETURN {
    "connection_path": path.path_nodes,
    "relationships": path.relationships,
    "confidence": path.score,
    "explanation": path.explanation
}

// Cross-reference with existing data
FOR company IN companies
    FOR product IN products
        FILTER company.industry == "technology"
        FOR path IN GRAPHRAG_FIND_PATHS("business_kg", company.name, product.name, {
            "relationship_types": ["PRODUCES", "DEVELOPS", "OWNS"],
            "max_hops": 3
        })
        FILTER path.score > 0.7
        RETURN {
            "company": company.name,
            "product": product.name,
            "connection_strength": path.score,
            "path_length": path.length
        }
```

#### 4. Entity Similarity Search

##### GRAPHRAG_FIND_SIMILAR(knowledge_graph, entity, options)

Finds entities similar to a given entity using embedding similarity or text similarity.

```aql
// Find similar entities using embeddings
FOR similar IN GRAPHRAG_FIND_SIMILAR("research_kg", "Machine Learning", {
    "limit": 10,
    "similarity_threshold": 0.7
})
RETURN {
    "entity": similar.entity_text,
    "entity_type": similar.entity_type,
    "similarity_score": similar.similarity,
    "confidence": similar.confidence
}

// Find similar concepts for research recommendations
FOR paper IN papers
    FOR concept IN GRAPHRAG_EXTRACT_ENTITIES(paper.abstract, {})
    FOR similar IN GRAPHRAG_FIND_SIMILAR("research_kg", concept.text, {
        "limit": 5,
        "similarity_threshold": 0.8
    })
    COLLECT concept_text = concept.text INTO similar_concepts = similar
    RETURN {
        "concept": concept_text,
        "similar_concepts": similar_concepts[*].entity_text,
        "avg_similarity": AVG(similar_concepts[*].similarity)
    }
```

#### 5. Semantic Search

##### GRAPHRAG_SEMANTIC_SEARCH(knowledge_graph, query_text, options)

Performs semantic search across the knowledge graph using RAG queries.

```aql
// Semantic search for relevant entities and context
FOR result IN GRAPHRAG_SEMANTIC_SEARCH("research_kg", 
    "What are the latest developments in quantum computing?", {
    "max_results": 20,
    "max_hops": 2,
    "context_size": 1024
})
FILTER result.type == "entity" OR result.type == "path"
RETURN {
    "type": result.type,
    "content": result.content,
    "relevance_score": result.relevance_score
}

// Combine semantic search with document filtering
FOR doc IN research_papers
    FILTER doc.year >= 2023
    FOR search_result IN GRAPHRAG_SEMANTIC_SEARCH("research_kg",
        CONCAT("Find information about ", doc.topic), {
        "max_results": 10
    })
    WHERE search_result.type == "entity"
    RETURN {
        "paper": doc.title,
        "topic": doc.topic,
        "relevant_entities": search_result.content
    }
```

#### 6. Entity Listing

##### GRAPHRAG_LIST_ENTITIES(knowledge_graph, options)

Lists all entities in a knowledge graph with optional filtering.

```aql
// List all entities
FOR entity IN GRAPHRAG_LIST_ENTITIES("research_kg", {
    "limit": 100
})
RETURN {
    "id": entity.id,
    "text": entity.text,
    "type": entity.entity_type,
    "confidence": entity.confidence,
    "labels": entity.labels,
    "source_documents": entity.source_documents
}

// Filter by entity type
FOR entity IN GRAPHRAG_LIST_ENTITIES("business_kg", {
    "entity_type": "Organization",
    "limit": 50
})
SORT entity.confidence DESC
RETURN {
    "organization": entity.text,
    "confidence": entity.confidence,
    "mentioned_in": entity.source_documents
}
```

#### 7. Trend Analysis

##### GRAPHRAG_ANALYZE_TRENDS(knowledge_graph, concept, options)

Analyzes trends for a concept over time based on relationship creation patterns.

```aql
// Analyze trends for a concept
FOR trend IN GRAPHRAG_ANALYZE_TRENDS("research_kg", "Artificial Intelligence", {
    "time_window_days": 30
})
RETURN {
    "timestamp": trend.timestamp,
    "relationship_count": trend.relationship_count,
    "concept_entities_found": trend.concept_entities_found
}

// Compare trends across multiple concepts
FOR concept IN ["AI", "ML", "Deep Learning"]
    FOR trend IN GRAPHRAG_ANALYZE_TRENDS("research_kg", concept, {
        "time_window_days": 90
    })
    COLLECT period = trend.timestamp INTO trends = trend
    RETURN {
        "concept": concept,
        "trends": trends,
        "total_activity": SUM(trends[*].relationship_count)
    }
```

#### 8. Community Detection

##### GRAPHRAG_DETECT_COMMUNITIES(knowledge_graph, options)

Detects communities (connected components) in the knowledge graph.

```aql
// Detect communities in knowledge graph
FOR community IN GRAPHRAG_DETECT_COMMUNITIES("research_kg", {
    "min_community_size": 3
})
RETURN {
    "community_id": community.community_id,
    "size": community.size,
    "entity_ids": community.entity_ids
}

// Analyze community structure
FOR community IN GRAPHRAG_DETECT_COMMUNITIES("business_kg", {
    "min_community_size": 5
})
LET entities = community.entity_ids
FOR entity_id IN entities
    FOR entity IN GRAPHRAG_LIST_ENTITIES("business_kg", {
        "limit": 1000
    })
    WHERE entity.id == entity_id
    COLLECT community_id = community.community_id INTO community_entities = entity
    RETURN {
        "community": community_id,
        "size": LENGTH(community_entities),
        "entity_types": UNIQUE(community_entities[*].entity_type),
        "top_entities": community_entities[0..5].text
    }
```

#### 9. Analytics and Statistics

##### GRAPHRAG_GET_STATS(knowledge_graph)

```aql
// Knowledge graph statistics
FOR stats IN GRAPHRAG_GET_STATS("research_kg")
RETURN {
    "documents_processed": stats.documents_processed,
    "entity_density": stats.entities_extracted / stats.documents_processed,
    "relationship_density": stats.relationships_extracted / stats.entities_extracted,
    "processing_efficiency": stats.avg_document_processing_time_ms,
    "query_performance": stats.avg_rag_query_time_ms
}

// Compare multiple knowledge graphs
FOR kg_name IN ["research_kg", "business_kg", "news_kg"]
    FOR stats IN GRAPHRAG_GET_STATS(kg_name)
    SORT stats.entities_extracted DESC
    RETURN {
        "knowledge_graph": kg_name,
        "total_entities": stats.entities_extracted,
        "success_rate": stats.rag_success_rate,
        "performance_score": 1000 / stats.avg_rag_query_time_ms
    }
```

### AQL Integration Patterns

#### 1. Document Processing Pipeline

```aql
// Multi-stage document processing
FOR doc IN documents
    FILTER doc.status == "pending" AND doc.language == "en"
    
    // Stage 1: Extract entities
    FOR entities IN GRAPHRAG_EXTRACT_ENTITIES(doc.content, {
        "extractors": ["named_entity", "keyword"],
        "confidence_threshold": 0.7
    })
    
    // Stage 2: Build knowledge graph
    FOR kg_result IN GRAPHRAG_BUILD_KNOWLEDGE(doc, {
        "knowledge_graph": CONCAT("kg_", doc.category),
        "document_id": doc._key,
        "build_graph": true
    })
    
    // Stage 3: Update document status
    UPDATE doc WITH {
        "status": "processed",
        "entities_count": LENGTH(entities),
        "kg_nodes_created": kg_result.entities_extracted,
        "processed_at": DATE_NOW()
    } IN documents
    
    RETURN {
        "document_id": doc._key,
        "entities_extracted": LENGTH(entities),
        "kg_integration": kg_result.success
    }
```

#### 2. Cross-Collection Analysis

```aql
// Analyze relationships across collections
FOR author IN authors
    FOR paper IN papers
        FILTER paper.author_id == author._key
        
        FOR concepts IN GRAPHRAG_EXTRACT_ENTITIES(paper.abstract, {
            "entity_types": ["CONCEPT", "METHODOLOGY", "TECHNOLOGY"]
        })
        
        COLLECT author_name = author.name INTO author_concepts = concepts
        
        FOR concept_group IN author_concepts
            COLLECT concept_text = concept_group.text INTO concept_papers = concept_group
            FILTER LENGTH(concept_papers) >= 3
            
            RETURN {
                "author": author_name,
                "expertise_area": concept_text,
                "paper_count": LENGTH(concept_papers),
                "expertise_confidence": AVG(concept_papers[*].confidence)
            }
```

## Bolt/Cypher GraphRAG Integration

### Bolt/Cypher Integration Architecture

The Bolt GraphRAG integration provides GraphRAG functionality through Cypher stored procedures that follow Neo4j conventions.

```rust
use orbit_protocols::cypher::{CypherServer, BoltGraphRAGProcedures};
use orbit_client::OrbitClient;

// Create Cypher server with GraphRAG procedures
let orbit_client = OrbitClient::new(/* config */);
let procedures = BoltGraphRAGProcedures::new(orbit_client);
let server = CypherServer::new_with_graphrag("127.0.0.1:7687", procedures);
```

### Core Procedures

#### 1. Knowledge Graph Building

##### orbit.graphrag.buildKnowledge(kg_name, document_id, text, metadata, config)

```cypher
// Build knowledge graph with node creation
MATCH (doc:Document {id: "paper_001"})
CALL orbit.graphrag.buildKnowledge(
    "research_kg",
    doc.id,
    doc.content,
    {category: doc.category, authors: doc.authors},
    {extractors: ["entity", "relationship"], build_graph: true}
) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms

// Create tracking nodes
CREATE (kg:KnowledgeGraph {
    name: kg_name,
    entities_count: entities_extracted,
    relationships_count: relationships_extracted,
    last_updated: datetime()
})
CREATE (doc)-[:CONTRIBUTES_TO]->(kg)

RETURN kg_name, entities_extracted, relationships_extracted;
```

#### 2. Entity Extraction with Graph Integration

##### orbit.graphrag.extractEntities(text, config)

```cypher
// Extract entities and create graph nodes
MATCH (doc:Document) WHERE doc.processed = false
CALL orbit.graphrag.extractEntities(
    doc.content,
    {extractors: ["named_entity", "keyword"], confidence_threshold: 0.7}
) YIELD entity_text, entity_type, confidence, start_pos, end_pos

// Create entity nodes and relationships
MERGE (e:Entity {name: entity_text, type: entity_type})
SET e.confidence = confidence,
    e.first_mentioned = CASE WHEN e.first_mentioned IS NULL THEN datetime() ELSE e.first_mentioned END,
    e.mention_count = COALESCE(e.mention_count, 0) + 1

CREATE (doc)-[:MENTIONS {position: start_pos, confidence: confidence}]->(e)
SET doc.processed = true

RETURN doc.id, entity_text, entity_type, confidence;
```

#### 3. RAG Queries with Graph Context

##### orbit.graphrag.ragQuery(kg_name, query_text, config)

```cypher
// RAG query with graph pattern context
MATCH path = (start:Concept {name: "Machine Learning"})-[*1..2]-(related:Concept)
WITH collect(related.name) as context_concepts

CALL orbit.graphrag.ragQuery(
    "research_kg", 
    "How do these concepts relate to deep learning?",
    {
        max_hops: 2,
        context_entities: context_concepts,
        include_path_reasoning: true
    }
) YIELD response, confidence, reasoning_paths

// Create result nodes for further analysis
CREATE (result:QueryResult {
    query: "How do these concepts relate to deep learning?",
    response: response,
    confidence: confidence,
    timestamp: datetime()
})

// Link to reasoning paths
UNWIND reasoning_paths as path
CREATE (reasoning:ReasoningPath {
    nodes: path.nodes,
    explanation: path.explanation,
    score: path.score
})
CREATE (result)-[:BASED_ON]->(reasoning)

RETURN response, confidence, 
       [path IN reasoning_paths | path.explanation] as explanations;
```

#### 4. Multi-hop Reasoning with Path Creation

##### orbit.graphrag.findPaths(kg_name, from_entity, to_entity, config)

```cypher
// Find and materialize reasoning paths
MATCH (start:Company {name: "Apple Inc."})
MATCH (end:Product {name: "iPhone"})

CALL orbit.graphrag.findPaths(
    "business_kg",
    start.name,
    end.name,
    {max_hops: 3, relationship_types: ["PRODUCES", "DEVELOPS", "OWNS"]}
) YIELD path_nodes, relationships, score

WITH start, end, path_nodes, relationships, score
WHERE score > 0.8

// Create reasoning path in graph
UNWIND range(0, size(path_nodes)-2) as i
WITH start, end, path_nodes, relationships, score, i,
     path_nodes[i] as current_node,
     path_nodes[i+1] as next_node,
     relationships[i] as rel_info

MERGE (c:ReasoningNode {name: current_node})
MERGE (n:ReasoningNode {name: next_node})
CREATE (c)-[:REASONING_STEP {
    type: rel_info.type,
    score: score,
    step: i,
    reasoning_session: randomUUID()
}]->(n)

RETURN path_nodes, relationships, score
ORDER BY score DESC;
```

#### 5. Entity Similarity Search

##### orbit.graphrag.findSimilar(kg_name, entity, config)

Finds entities similar to a given entity using embedding or text similarity.

```cypher
// Find similar entities using embeddings
MATCH (target:Entity {name: "Machine Learning"})
CALL orbit.graphrag.findSimilar(
    "research_kg",
    target.name,
    {limit: 10, similarity_threshold: 0.7}
) YIELD entity_id, entity_text, entity_type, similarity, confidence

// Create similarity relationships
MERGE (similar:Entity {id: entity_id, name: entity_text})
CREATE (target)-[:SIMILAR_TO {
    similarity: similarity,
    confidence: confidence,
    discovered_at: datetime()
}]->(similar)

RETURN entity_text, entity_type, similarity, confidence
ORDER BY similarity DESC;
```

#### 6. Semantic Search

##### orbit.graphrag.semanticSearch(kg_name, query_text, config)

Performs semantic search across the knowledge graph using RAG queries.

```cypher
// Semantic search for relevant entities and context
CALL orbit.graphrag.semanticSearch(
    "research_kg",
    "What are the latest developments in quantum computing?",
    {max_results: 20, max_hops: 2, context_size: 1024}
) YIELD type, content, entity_id, score

// Filter and process results
WITH * WHERE type IN ["entity", "path"]
MATCH (e:Entity) WHERE e.id = entity_id OR e.name = content
RETURN type, content, score, e
ORDER BY score DESC;
```

#### 7. Entity Listing

##### orbit.graphrag.listEntities(kg_name, config)

Lists all entities in a knowledge graph with optional filtering.

```cypher
// List all entities
CALL orbit.graphrag.listEntities(
    "research_kg",
    {limit: 100}
) YIELD entity_id, entity_text, entity_type, confidence, labels, source_documents

// Create entity nodes in graph
MERGE (e:GraphRAGEntity {
    id: entity_id,
    name: entity_text,
    type: entity_type
})
SET e.confidence = confidence,
    e.labels = labels,
    e.source_documents = source_documents

RETURN e
ORDER BY confidence DESC
LIMIT 100;
```

#### 8. Trend Analysis

##### orbit.graphrag.analyzeTrends(kg_name, concept, config)

Analyzes trends for a concept over time based on relationship creation patterns.

```cypher
// Analyze trends for a concept
CALL orbit.graphrag.analyzeTrends(
    "research_kg",
    "Artificial Intelligence",
    {time_window_days: 30}
) YIELD timestamp, relationship_count, concept_entities_found

// Create trend visualization nodes
CREATE (trend:Trend {
    concept: "Artificial Intelligence",
    timestamp: timestamp,
    activity_level: relationship_count,
    entities_involved: concept_entities_found
})

RETURN trend
ORDER BY timestamp;
```

#### 9. Community Detection

##### orbit.graphrag.detectCommunities(kg_name, config)

Detects communities (connected components) in the knowledge graph.

```cypher
// Detect communities
CALL orbit.graphrag.detectCommunities(
    "research_kg",
    {min_community_size: 3}
) YIELD community_id, size, entity_ids

// Create community nodes and relationships
CREATE (comm:Community {
    id: community_id,
    size: size
})

// Link entities to communities
UNWIND entity_ids as entity_id
MATCH (e:Entity {id: entity_id})
CREATE (e)-[:BELONGS_TO]->(comm)

RETURN comm, size
ORDER BY size DESC;
```

#### 10. Graph Analytics and Statistics

##### orbit.graphrag.getStats(kg_name)

```cypher
// Knowledge graph analytics with visualization
CALL orbit.graphrag.getStats("research_kg")
YIELD documents_processed, entities_extracted, relationships_extracted,
      avg_document_processing_time_ms, rag_success_rate

// Create analytics dashboard nodes
CREATE (dashboard:Analytics {
    knowledge_graph: "research_kg",
    documents_processed: documents_processed,
    entities_extracted: entities_extracted,
    relationships_extracted: relationships_extracted,
    entity_density: entities_extracted / documents_processed,
    relationship_density: relationships_extracted / entities_extracted,
    processing_efficiency: 1000 / avg_document_processing_time_ms,
    success_rate: rag_success_rate,
    generated_at: datetime()
})

RETURN dashboard;
```

### Advanced Cypher Integration Patterns

#### 1. Graph Augmentation with GraphRAG

```cypher
// Augment existing citation graph with semantic relationships
MATCH (paper1:Paper)-[:CITES]->(paper2:Paper)
WHERE NOT (paper1)-[:SEMANTICALLY_RELATED]-(paper2)
WITH collect(paper1.title + " relates to " + paper2.title) as relationship_texts

CALL orbit.graphrag.ragQuery(
    "research_kg",
    "Analyze semantic relationships between these papers",
    {context_texts: relationship_texts, derive_relationships: true}
) YIELD entities_involved, reasoning_paths

// Create semantic relationship edges
UNWIND reasoning_paths as path
MATCH (p1:Paper {title: path.from_entity})
MATCH (p2:Paper {title: path.to_entity})
WHERE path.score > 0.7
CREATE (p1)-[:SEMANTICALLY_RELATED {
    score: path.score,
    reasoning: path.explanation,
    discovered_by: "graphrag",
    discovered_at: datetime()
}]->(p2);
```

#### 2. Real-time Knowledge Graph Updates

```cypher
// Stream processing with GraphRAG integration
CALL orbit.graphrag.streamEntities(
    "live_news_kg",
    {
        min_confidence: 0.8,
        entity_types: ["Person", "Organization", "Event"],
        stream_duration_seconds: 3600
    }
) YIELD entity_text, entity_type, confidence, timestamp, source_document

// Create or update entity nodes in real-time
MERGE (e:StreamedEntity {name: entity_text, type: entity_type})
ON CREATE SET 
    e.first_seen = timestamp,
    e.confidence = confidence,
    e.mention_count = 1
ON MATCH SET 
    e.mention_count = e.mention_count + 1,
    e.last_seen = timestamp,
    e.confidence = (e.confidence + confidence) / 2

// Link to source documents
MERGE (doc:Document {id: source_document})
CREATE (doc)-[:MENTIONS {
    confidence: confidence,
    discovered_at: timestamp
}]->(e);
```

## Comparison Matrix

| Feature | AQL GraphRAG | Bolt/Cypher GraphRAG | PostgreSQL GraphRAG |
|---------|--------------|----------------------|---------------------|
| **Query Style** | Functional within AQL | Stored procedures | SQL functions |
| **Result Format** | AQL objects/arrays | Cypher table rows | PostgreSQL result sets |
| **Graph Integration** | Document-oriented | Native graph patterns | Relational with JSON |
| **Streaming** | FOR loops over results | YIELD streaming | Standard SQL cursors |
| **Aggregation** | COLLECT/GROUP BY | Native Cypher aggregation | SQL GROUP BY |
| **Pattern Matching** | Filter expressions | MATCH patterns | WHERE clauses |
| **Transaction Support** | ArangoDB transactions | Neo4j transactions | PostgreSQL ACID |
| **Performance** | Document store optimized | Graph traversal optimized | Relational optimized |

## Best Practices

### 1. Protocol Selection Guidelines

**Use AQL GraphRAG when:**

- Working with document-heavy workloads
- Need complex aggregation and analytics
- Integrating with existing ArangoDB infrastructure
- Processing large document collections

**Use Bolt/Cypher GraphRAG when:**

- Working with highly connected data
- Need complex graph pattern matching
- Integrating with existing Neo4j infrastructure
- Emphasis on relationship analysis

**Use PostgreSQL GraphRAG when:**

- Working with traditional relational data
- Need ACID compliance and strong consistency
- Integrating with existing PostgreSQL applications
- Using BI tools and standard SQL interfaces

### 2. Performance Optimization

#### AQL Optimization

```aql
// Use indexes for filtering
FOR doc IN documents
    FILTER doc.category == "research" AND doc.year >= 2020  // Indexed fields first
    FOR result IN GRAPHRAG_BUILD_KNOWLEDGE(doc, {
        "batch_mode": true,  // Enable batch processing
        "defer_indexing": true  // Defer expensive operations
    })
    RETURN result

// Batch operations for efficiency
FOR batch IN RANGE(0, LENGTH(documents), 100)
    LET batch_docs = SLICE(documents, batch, 100)
    FOR doc IN batch_docs
        FOR result IN GRAPHRAG_BUILD_KNOWLEDGE(doc, {
            "parallel_processing": true
        })
        RETURN result
```

#### Cypher Optimization

```cypher
// Use parameters and prepare statements
:param kg_name => "research_kg"
:param min_confidence => 0.8

// Create indexes for performance
CREATE INDEX entity_name_idx FOR (e:Entity) ON (e.name)
CREATE INDEX entity_type_idx FOR (e:Entity) ON (e.type)

// Use PROFILE to analyze query performance
PROFILE
CALL orbit.graphrag.findPaths(
    $kg_name, 
    "Apple Inc.", 
    "iPhone", 
    {max_hops: 3, min_confidence: $min_confidence}
) YIELD path_nodes, score
RETURN path_nodes, score
ORDER BY score DESC
LIMIT 10;
```

### 3. Error Handling

#### AQL Error Handling

```aql
// Graceful error handling with fallbacks
FOR doc IN documents
    FOR result IN GRAPHRAG_BUILD_KNOWLEDGE(doc, {}) 
    FILTER result.success == true
    RETURN result.result

// Handle warnings and partial failures
FOR result IN GRAPHRAG_BUILD_KNOWLEDGE(doc, {})
    FILTER LENGTH(result.warnings) > 0
    INSERT {
        "document_id": doc._key,
        "warnings": result.warnings,
        "timestamp": DATE_NOW()
    } INTO processing_warnings
    RETURN result
```

#### Cypher Error Handling

```cypher
// Use APOC for conditional processing
CALL orbit.graphrag.buildKnowledge("kg", "doc1", "text", {}, {})
YIELD success, errors, warnings
FOREACH (error IN CASE WHEN NOT success THEN errors ELSE [] END |
    CREATE (err:ProcessingError {
        message: error,
        timestamp: datetime(),
        procedure: "buildKnowledge"
    })
)
RETURN success, size(errors) as error_count;
```

## Integration Examples

### Multi-Protocol Knowledge Discovery

```python

# Python application using multiple protocols
import psycopg2
import neo4j
import requests

class MultiProtocolGraphRAG:
    def __init__(self):
        # PostgreSQL for structured queries and BI integration
        self.pg_conn = psycopg2.connect("host=localhost port=5433 dbname=orbit")
        
        # Neo4j for graph pattern matching
        self.neo4j_driver = neo4j.GraphDatabase.driver("bolt://localhost:7687")
        
        # ArangoDB for document processing (via REST)
        self.arango_url = "http://localhost:8529"
    
    def build_knowledge_graph(self, documents):
        """Build knowledge graph using optimal protocol per task"""
        
        # Use ArangoDB/AQL for document processing
        for doc in documents:
            aql_query = """
            FOR result IN GRAPHRAG_BUILD_KNOWLEDGE(@doc, {
                "knowledge_graph": @kg_name,
                "extractors": ["entity", "relationship"]
            })
            RETURN result
            """
            # Execute AQL query...
        
        # Use Neo4j for relationship analysis
        with self.neo4j_driver.session() as session:
            session.run("""
                CALL orbit.graphrag.findPaths($kg_name, $entity1, $entity2, {
                    max_hops: 3,
                    include_explanation: true
                })
            """, kg_name="main_kg", entity1="concept1", entity2="concept2")
        
        # Use PostgreSQL for analytics and reporting
        with self.pg_conn.cursor() as cur:
            cur.execute("""
                SELECT kg_name, documents_processed, rag_success_rate
                FROM GRAPHRAG_STATS(%s)
                WHERE rag_success_rate > 0.8
            """, ("main_kg",))
    
    def query_knowledge_graph(self, query_text):
        """Query using best protocol for the query type"""
        
        # Use PostgreSQL for simple Q&A
        with self.pg_conn.cursor() as cur:
            cur.execute("""
                SELECT response, confidence 
                FROM GRAPHRAG_QUERY(%s, %s, 3, 2048, 'ollama', false)
            """, ("main_kg", query_text))
            return cur.fetchone()
```

This comprehensive integration demonstrates how GraphRAG functionality can be accessed through multiple query languages and protocols, each optimized for different use cases while maintaining consistency in the underlying GraphRAG operations.
