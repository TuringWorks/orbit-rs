---
layout: default
title: AQL GraphRAG Function Design
category: documentation
---

# AQL GraphRAG Function Design

## Overview

This document defines the AQL (ArangoDB Query Language) GraphRAG functions that integrate with Orbit's GraphRAG capabilities. These functions follow AQL conventions and provide natural integration with ArangoDB-style queries.

## Function Categories

### 1. Document Processing Functions

#### GRAPHRAG_BUILD_KNOWLEDGE(collection, options)
Builds knowledge graphs from documents in a collection.

```aql
// Build knowledge graph from all documents in papers collection
FOR result IN GRAPHRAG_BUILD_KNOWLEDGE("research_papers", {
    "knowledge_graph": "scientific_knowledge",
    "extractors": ["entity_extraction", "relationship_extraction"],
    "build_graph": true,
    "generate_embeddings": true
})
RETURN result

// Build with metadata filtering
FOR doc IN research_papers
    FILTER doc.year >= 2020
    FOR result IN GRAPHRAG_BUILD_KNOWLEDGE(doc, {
        "knowledge_graph": "recent_research", 
        "document_id": doc._key,
        "metadata": {"year": doc.year, "journal": doc.journal}
    })
    RETURN result
```

#### GRAPHRAG_EXTRACT_ENTITIES(text, options)
Extracts entities from text without building full knowledge graph.

```aql
// Extract entities from document text
FOR doc IN news_articles
    FOR entity IN GRAPHRAG_EXTRACT_ENTITIES(doc.content, {
        "extractors": ["named_entity", "keyword"],
        "confidence_threshold": 0.7
    })
    RETURN {
        "article_id": doc._key,
        "entity": entity.text,
        "type": entity.entity_type,
        "confidence": entity.confidence
    }
```

### 2. Query and Retrieval Functions

#### GRAPHRAG_QUERY(knowledge_graph, query_text, options)
Performs RAG queries against knowledge graphs.

```aql
// Simple RAG query
FOR answer IN GRAPHRAG_QUERY("scientific_knowledge", 
    "What are the latest developments in quantum computing?", {
    "max_hops": 3,
    "context_size": 2048,
    "llm_provider": "ollama",
    "include_explanation": true
})
RETURN answer

// RAG query with filtering by metadata
FOR paper IN research_papers
    FILTER paper.field == "quantum_computing"
    FOR answer IN GRAPHRAG_QUERY("scientific_knowledge",
        CONCAT("Summarize findings from ", paper.title), {
        "max_hops": 2,
        "entity_filter": {"source_document": paper._key}
    })
    RETURN {
        "paper": paper.title,
        "summary": answer.response,
        "confidence": answer.confidence
    }
```

#### GRAPHRAG_FIND_PATHS(knowledge_graph, from_entity, to_entity, options)
Finds reasoning paths between entities using multi-hop reasoning.

```aql
// Find connections between concepts
FOR path IN GRAPHRAG_FIND_PATHS("tech_knowledge", "Apple Inc.", "iPhone", {
    "max_hops": 4,
    "include_explanation": true,
    "max_results": 5
})
RETURN {
    "path_nodes": path.nodes,
    "relationships": path.relationships,
    "score": path.score,
    "explanation": path.explanation
}

// Find paths with relationship type filtering
FOR entity1 IN entities
    FILTER entity1.type == "Company"
    FOR entity2 IN entities
        FILTER entity2.type == "Product" AND entity1 != entity2
        FOR path IN GRAPHRAG_FIND_PATHS("business_kg", entity1.name, entity2.name, {
            "relationship_types": ["PRODUCES", "DEVELOPS", "OWNS"],
            "max_hops": 3
        })
        RETURN {
            "company": entity1.name,
            "product": entity2.name,
            "connection_strength": path.score
        }
```

### 3. Similarity and Search Functions

#### GRAPHRAG_FIND_SIMILAR(knowledge_graph, entity, options)
Finds entities similar to a given entity using vector similarity.

```aql
// Find similar companies
FOR similar IN GRAPHRAG_FIND_SIMILAR("business_kg", "Apple Inc.", {
    "limit": 10,
    "threshold": 0.8,
    "entity_types": ["Company"]
})
RETURN {
    "entity": similar.entity_name,
    "similarity": similar.score,
    "shared_attributes": similar.common_properties
}

// Find similar concepts across different knowledge graphs
FOR concept IN concepts
    FOR similar IN GRAPHRAG_FIND_SIMILAR("research_kg", concept.name, {
        "limit": 5,
        "cross_kg_search": true
    })
    FILTER similar.score > 0.7
    RETURN {
        "original": concept.name,
        "similar": similar.entity_name,
        "similarity": similar.score
    }
```

### 4. Knowledge Graph Management Functions

#### GRAPHRAG_GET_STATS(knowledge_graph)
Retrieves statistics about a knowledge graph.

```aql
// Get knowledge graph statistics
FOR stats IN GRAPHRAG_GET_STATS("research_kg")
RETURN {
    "documents_processed": stats.documents_processed,
    "entities_count": stats.entities_extracted,
    "relationships_count": stats.relationships_extracted,
    "avg_processing_time": stats.avg_document_processing_time_ms,
    "success_rate": stats.rag_success_rate
}

// Compare statistics across knowledge graphs  
FOR kg_name IN ["research_kg", "business_kg", "news_kg"]
    FOR stats IN GRAPHRAG_GET_STATS(kg_name)
    RETURN {
        "knowledge_graph": kg_name,
        "entity_density": stats.entities_extracted / stats.documents_processed,
        "relationship_density": stats.relationships_extracted / stats.entities_extracted
    }
```

#### GRAPHRAG_LIST_ENTITIES(knowledge_graph, options)
Lists entities from a knowledge graph with optional filtering.

```aql
// List all person entities
FOR entity IN GRAPHRAG_LIST_ENTITIES("news_kg", {
    "entity_type": "Person",
    "limit": 100,
    "sort_by": "confidence",
    "sort_order": "DESC"
})
RETURN entity

// List entities with property filters
FOR entity IN GRAPHRAG_LIST_ENTITIES("business_kg", {
    "entity_type": "Company",
    "property_filters": {
        "industry": "technology",
        "founded_after": 2000
    }
})
RETURN entity
```

### 5. Advanced Analytics Functions

#### GRAPHRAG_ANALYZE_TRENDS(knowledge_graph, concept, options)
Analyzes trends and patterns around a specific concept.

```aql
// Analyze research trends over time
FOR trend IN GRAPHRAG_ANALYZE_TRENDS("research_kg", "artificial intelligence", {
    "time_grouping": "year",
    "metrics": ["mention_frequency", "sentiment", "innovation_score"],
    "time_range": {"start": "2020-01-01", "end": "2024-01-01"}
})
SORT trend.year
RETURN trend

// Compare trends between related concepts
FOR concept IN ["machine learning", "deep learning", "neural networks"]
    FOR trend IN GRAPHRAG_ANALYZE_TRENDS("research_kg", concept, {
        "time_grouping": "quarter",
        "normalize": true
    })
    RETURN {
        "concept": concept,
        "period": trend.period,
        "relative_growth": trend.normalized_growth
    }
```

#### GRAPHRAG_DETECT_COMMUNITIES(knowledge_graph, options)
Detects communities and clusters within the knowledge graph.

```aql
// Detect research communities
FOR community IN GRAPHRAG_DETECT_COMMUNITIES("research_kg", {
    "algorithm": "louvain",
    "min_community_size": 5,
    "edge_weight_threshold": 0.5
})
RETURN {
    "community_id": community.id,
    "size": LENGTH(community.entities),
    "main_topics": community.dominant_concepts[0..5],
    "cohesion_score": community.internal_density
}
```

## Integration with AQL Features

### 1. Collection-Based Processing

```aql
// Process documents with AQL iteration
FOR doc IN documents
    FILTER doc.language == "en"
    FOR result IN GRAPHRAG_BUILD_KNOWLEDGE(doc, {
        "knowledge_graph": CONCAT("kg_", doc.category),
        "document_id": doc._key
    })
    INSERT {
        "document_id": doc._key,
        "processing_result": result,
        "processed_at": DATE_NOW()
    } INTO processing_log
```

### 2. Graph Traversal Integration

```aql
// Combine AQL graph traversal with GraphRAG
FOR vertex, edge, path IN 1..3 OUTBOUND "papers/quantum_paper_1" paper_citations
    FOR related_concepts IN GRAPHRAG_QUERY("research_kg", 
        CONCAT("What concepts are related to ", vertex.title), {
        "max_hops": 2,
        "context_from_path": path
    })
    RETURN {
        "paper": vertex.title,
        "citation_path_length": LENGTH(path.vertices),
        "related_concepts": related_concepts.entities_involved
    }
```

### 3. Aggregation and Analytics

```aql
// Aggregate GraphRAG results
FOR doc IN research_papers
    FOR entities IN GRAPHRAG_EXTRACT_ENTITIES(doc.abstract, {})
    COLLECT entity_type = entities.entity_type INTO groups = entities
    RETURN {
        "entity_type": entity_type,
        "total_mentions": LENGTH(groups),
        "avg_confidence": AVG(groups[*].confidence),
        "unique_entities": LENGTH(UNIQUE(groups[*].text))
    }
```

### 4. Subqueries and Joins

```aql
// Use GraphRAG in subqueries
FOR paper IN research_papers
    LET similar_papers = (
        FOR similar IN GRAPHRAG_FIND_SIMILAR("research_kg", paper.title, {"limit": 5})
        FOR p IN research_papers
            FILTER p.title == similar.entity_name
            RETURN p
    )
    RETURN {
        "original_paper": paper,
        "similar_papers": similar_papers,
        "similarity_cluster_size": LENGTH(similar_papers)
    }
```

## Function Return Types

### Standard Result Structure
All GraphRAG functions return objects with consistent structure:

```json
{
    "success": true,
    "result": { /* function-specific result */ },
    "metadata": {
        "processing_time_ms": 1250,
        "knowledge_graph": "research_kg", 
        "timestamp": "2024-01-15T10:30:00Z"
    },
    "warnings": []
}
```

### Error Handling

```aql
// Error handling with AQL
FOR result IN GRAPHRAG_QUERY("nonexistent_kg", "test query", {})
    FILTER result.success == true
    RETURN result.result

// Handle warnings
FOR result IN GRAPHRAG_BUILD_KNOWLEDGE(doc, {})
    FILTER LENGTH(result.warnings) > 0
    INSERT {
        "document_id": doc._key,
        "warnings": result.warnings,
        "timestamp": DATE_NOW()
    } INTO processing_warnings
```

## Performance Optimization

### 1. Batch Processing

```aql
// Batch document processing
FOR batch IN RANGE(0, LENGTH(documents), 100)
    LET batch_docs = SLICE(documents, batch, 100)
    FOR doc IN batch_docs
        FOR result IN GRAPHRAG_BUILD_KNOWLEDGE(doc, {
            "batch_mode": true,
            "defer_indexing": true
        })
        RETURN result
```

### 2. Caching Integration

```aql
// Use AQL caching for repeated queries
FOR cached_result IN GRAPHRAG_QUERY("research_kg", @query_text, {
    "cache_ttl_seconds": 3600,
    "cache_key": CONCAT("query_", MD5(@query_text))
})
RETURN cached_result
```

This design provides native AQL integration while leveraging all of ArangoDB's powerful query capabilities alongside GraphRAG functionality.