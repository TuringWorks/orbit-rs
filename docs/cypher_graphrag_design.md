# Cypher GraphRAG Procedures Design

## Overview

This document defines Cypher stored procedures for GraphRAG operations that integrate with Orbit's GraphRAG capabilities through the Bolt protocol. These procedures follow Neo4j conventions and provide natural integration with Cypher queries.

## Procedure Categories

### 1. Knowledge Graph Building Procedures

#### `orbit.graphrag.buildKnowledge(kg_name, document_id, text, metadata, config)`
Builds knowledge graphs from document text with extracted entities and relationships.

```cypher
// Build knowledge graph from document
CALL orbit.graphrag.buildKnowledge(
    "scientific_papers",
    "paper_001", 
    "Machine learning is a subset of artificial intelligence...",
    {author: "John Doe", year: 2023, journal: "AI Review"},
    {extractors: ["entity", "relationship"], build_graph: true}
) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms
RETURN kg_name, entities_extracted, relationships_extracted;

// Build with existing graph nodes as context
MATCH (doc:Document {id: "paper_001"})
CALL orbit.graphrag.buildKnowledge(
    "research_kg",
    doc.id,
    doc.content,
    {category: doc.category, authors: doc.authors},
    {context_nodes: [doc.id], link_to_existing: true}
) YIELD kg_name, entities_extracted, relationships_extracted, graph_nodes_created
CREATE (doc)-[:HAS_KNOWLEDGE_GRAPH]->(kg:KnowledgeGraph {name: kg_name})
SET kg.entities_count = entities_extracted,
    kg.relationships_count = relationships_extracted,
    kg.nodes_created = graph_nodes_created;
```

#### `orbit.graphrag.extractEntities(text, config)`
Extracts entities from text without building the full knowledge graph.

```cypher
// Extract entities from multiple documents
MATCH (doc:Document) WHERE doc.processed = false
CALL orbit.graphrag.extractEntities(
    doc.content,
    {extractors: ["named_entity", "keyword"], confidence_threshold: 0.7}
) YIELD entity_text, entity_type, confidence, start_pos, end_pos
CREATE (e:Entity {
    text: entity_text, 
    type: entity_type, 
    confidence: confidence,
    source_document: doc.id
})
CREATE (doc)-[:CONTAINS_ENTITY]->(e)
SET doc.processed = true;

// Extract entities with relationship to existing graph
MATCH (person:Person {name: "Alice"})
CALL orbit.graphrag.extractEntities(
    "Alice works at OpenAI on language models with GPT architecture",
    {focus_entity: person.name, extract_relationships: true}
) YIELD entity_text, entity_type, relationships
FOREACH (rel IN relationships | 
    MERGE (target:Entity {text: rel.target_entity})
    CREATE (person)-[:rel.relationship_type]->(target)
);
```

### 2. Query and Retrieval Procedures

#### `orbit.graphrag.ragQuery(kg_name, query_text, config)`
Performs Retrieval-Augmented Generation queries against knowledge graphs.

```cypher
// Simple RAG query
CALL orbit.graphrag.ragQuery(
    "scientific_papers",
    "What are the main applications of machine learning?",
    {
        max_hops: 3,
        context_size: 2048,
        llm_provider: "ollama",
        include_explanation: true
    }
) YIELD response, confidence, processing_time_ms, entities_involved, citations
RETURN response, confidence, entities_involved;

// RAG query with graph context
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
RETURN response, confidence, 
       [path IN reasoning_paths | path.explanation] as explanations;
```

#### `orbit.graphrag.findPaths(kg_name, from_entity, to_entity, config)`
Finds reasoning paths between entities using multi-hop reasoning.

```cypher
// Find connection paths between entities
CALL orbit.graphrag.findPaths(
    "tech_companies",
    "Apple Inc.",
    "iPhone",
    {max_hops: 4, include_explanation: true, max_results: 5}
) YIELD path_nodes, relationships, score, length, explanation
RETURN path_nodes, relationships, score, explanation
ORDER BY score DESC;

// Find paths and create graph representation
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
FOREACH (i IN range(0, length(path_nodes)-2) |
    FOREACH (current_node IN [path_nodes[i]] |
        FOREACH (next_node IN [path_nodes[i+1]] |
            FOREACH (rel_type IN [relationships[i].type] |
                MERGE (c:PathNode {name: current_node})
                MERGE (n:PathNode {name: next_node})
                CREATE (c)-[:rel_type {score: score, from_reasoning: true}]->(n)
            )
        )
    )
);
```

### 3. Similarity and Search Procedures

#### `orbit.graphrag.findSimilar(kg_name, entity, config)`
Finds entities similar to a given entity using vector similarity.

```cypher
// Find similar companies
CALL orbit.graphrag.findSimilar(
    "business_kg",
    "Apple Inc.",
    {limit: 10, threshold: 0.8, entity_types: ["Company"]}
) YIELD entity_name, similarity_score, shared_properties
RETURN entity_name, similarity_score, shared_properties
ORDER BY similarity_score DESC;

// Find similar entities and connect them in graph
MATCH (company:Company {name: "Apple Inc."})
CALL orbit.graphrag.findSimilar(
    "business_kg",
    company.name,
    {limit: 5, threshold: 0.7, return_properties: true}
) YIELD entity_name, similarity_score, shared_properties
MERGE (similar:Company {name: entity_name})
CREATE (company)-[:SIMILAR_TO {score: similarity_score, shared: shared_properties}]->(similar);
```

#### `orbit.graphrag.semanticSearch(kg_name, query_text, config)`
Performs semantic search across the knowledge graph.

```cypher
// Semantic search for concepts
CALL orbit.graphrag.semanticSearch(
    "research_kg",
    "quantum computing applications",
    {limit: 20, min_relevance: 0.6, include_context: true}
) YIELD entity_name, entity_type, relevance_score, context_snippet
RETURN entity_name, entity_type, relevance_score, context_snippet
ORDER BY relevance_score DESC;

// Create semantic clusters from search results
CALL orbit.graphrag.semanticSearch(
    "research_kg",
    "artificial intelligence techniques",
    {limit: 50, cluster_results: true}
) YIELD entity_name, entity_type, relevance_score, cluster_id
WITH cluster_id, collect({name: entity_name, type: entity_type, score: relevance_score}) as cluster_entities
FOREACH (entity IN cluster_entities |
    MERGE (e:Concept {name: entity.name, type: entity.type})
    MERGE (cluster:SemanticCluster {id: cluster_id})
    CREATE (e)-[:BELONGS_TO_CLUSTER {relevance: entity.score}]->(cluster)
);
```

### 4. Knowledge Graph Analytics Procedures

#### `orbit.graphrag.getStats(kg_name)`
Retrieves comprehensive statistics about a knowledge graph.

```cypher
// Get knowledge graph statistics
CALL orbit.graphrag.getStats("research_kg")
YIELD documents_processed, entities_extracted, relationships_extracted,
      avg_document_processing_time_ms, avg_rag_query_time_ms, rag_success_rate
RETURN documents_processed, entities_extracted, relationships_extracted,
       avg_document_processing_time_ms, rag_success_rate;

// Compare stats across multiple knowledge graphs
UNWIND ["research_kg", "business_kg", "news_kg"] as kg_name
CALL orbit.graphrag.getStats(kg_name)
YIELD documents_processed, entities_extracted, relationships_extracted
RETURN kg_name, 
       entities_extracted / documents_processed as entity_density,
       relationships_extracted / entities_extracted as relationship_density
ORDER BY entity_density DESC;
```

#### `orbit.graphrag.listEntities(kg_name, config)`
Lists entities from a knowledge graph with filtering and pagination.

```cypher
// List entities with filtering
CALL orbit.graphrag.listEntities(
    "news_kg",
    {
        entity_type: "Person",
        limit: 100,
        offset: 0,
        sort_by: "confidence",
        sort_order: "DESC",
        property_filters: {nationality: "American"}
    }
) YIELD entity_text, entity_type, confidence, properties, aliases
RETURN entity_text, entity_type, confidence, properties;

// Create graph nodes from listed entities
CALL orbit.graphrag.listEntities(
    "business_kg",
    {entity_type: "Company", property_filters: {industry: "technology"}}
) YIELD entity_text, entity_type, properties
MERGE (c:Company {name: entity_text})
SET c += properties,
    c.from_graphrag = true,
    c.confidence = properties.confidence;
```

### 5. Advanced Analytics Procedures

#### `orbit.graphrag.analyzeTrends(kg_name, concept, config)`
Analyzes trends and patterns around specific concepts.

```cypher
// Analyze research trends over time
CALL orbit.graphrag.analyzeTrends(
    "research_kg",
    "artificial intelligence",
    {
        time_grouping: "year",
        metrics: ["mention_frequency", "sentiment", "innovation_score"],
        time_range: {start: "2020-01-01", end: "2024-01-01"}
    }
) YIELD period, mention_frequency, sentiment, innovation_score
WITH period, mention_frequency, sentiment, innovation_score
ORDER BY period
CREATE (trend:ResearchTrend {
    concept: "artificial intelligence",
    year: period,
    mentions: mention_frequency,
    sentiment: sentiment,
    innovation: innovation_score
});

// Compare trends between related concepts
UNWIND ["machine learning", "deep learning", "neural networks"] as concept
CALL orbit.graphrag.analyzeTrends(
    "research_kg",
    concept,
    {time_grouping: "quarter", normalize: true}
) YIELD period, normalized_growth
RETURN concept, period, normalized_growth
ORDER BY period, normalized_growth DESC;
```

#### `orbit.graphrag.detectCommunities(kg_name, config)`
Detects communities and clusters within the knowledge graph.

```cypher
// Detect communities in research topics
CALL orbit.graphrag.detectCommunities(
    "research_kg",
    {
        algorithm: "louvain",
        min_community_size: 5,
        edge_weight_threshold: 0.5,
        resolution: 1.0
    }
) YIELD community_id, entities, dominant_concepts, internal_density
WITH community_id, entities, dominant_concepts, internal_density
FOREACH (entity_name IN entities |
    MERGE (e:ResearchConcept {name: entity_name})
    MERGE (community:ResearchCommunity {id: community_id})
    SET community.dominant_concepts = dominant_concepts,
        community.internal_density = internal_density
    CREATE (e)-[:MEMBER_OF]->(community)
);
```

### 6. Graph Integration Procedures

#### `orbit.graphrag.augmentGraph(kg_name, config)`
Augments existing Cypher graph with GraphRAG-derived insights.

```cypher
// Augment paper citation graph with semantic relationships
MATCH (paper1:Paper)-[:CITES]->(paper2:Paper)
WITH collect(paper1.title + " cites " + paper2.title) as citation_texts
CALL orbit.graphrag.augmentGraph(
    "research_kg",
    {
        source_texts: citation_texts,
        derive_relationships: true,
        confidence_threshold: 0.8
    }
) YIELD source_entity, target_entity, relationship_type, confidence
MATCH (p1:Paper {title: source_entity})
MATCH (p2:Paper {title: target_entity})
CREATE (p1)-[:rel_name {confidence: confidence, derived: true}]->(p2)
WHERE relationship_type = rel_name;

// Enrich existing nodes with GraphRAG entity information
MATCH (company:Company)
CALL orbit.graphrag.augmentGraph(
    "business_kg",
    {
        enrich_nodes: [company.name],
        extract_properties: true,
        find_aliases: true
    }
) YIELD entity_name, enriched_properties, aliases
WHERE entity_name = company.name
SET company += enriched_properties,
    company.aliases = aliases,
    company.graphrag_enriched = true;
```

### 7. Streaming and Batch Procedures

#### `orbit.graphrag.processDocuments(documents, kg_name, config)`
Batch processes multiple documents for knowledge graph building.

```cypher
// Batch process documents from the graph
MATCH (doc:Document) WHERE doc.status = "pending"
WITH collect({id: doc.id, text: doc.content, metadata: doc.metadata}) as docs
CALL orbit.graphrag.processDocuments(
    docs,
    "document_kg",
    {batch_size: 50, parallel_processing: true}
) YIELD document_id, success, entities_extracted, processing_time_ms, errors
MATCH (doc:Document {id: document_id})
SET doc.status = CASE WHEN success THEN "processed" ELSE "error" END,
    doc.entities_count = entities_extracted,
    doc.processing_time = processing_time_ms,
    doc.errors = errors;
```

### 8. Real-time Streaming Procedures

#### `orbit.graphrag.streamEntities(kg_name, config)`
Streams entities from the knowledge graph in real-time.

```cypher
// Stream new entities as they're discovered
CALL orbit.graphrag.streamEntities(
    "live_news_kg",
    {
        min_confidence: 0.8,
        entity_types: ["Person", "Organization", "Event"],
        stream_duration_seconds: 3600
    }
) YIELD entity_text, entity_type, confidence, timestamp, source_document
CREATE (e:StreamedEntity {
    text: entity_text,
    type: entity_type,
    confidence: confidence,
    discovered_at: timestamp,
    source: source_document
});
```

## Integration with Cypher Patterns

### 1. Pattern Matching with GraphRAG

```cypher
// Combine pattern matching with GraphRAG reasoning
MATCH path = (person:Person)-[:WORKS_FOR*1..2]-(company:Company)
WHERE person.name = "Alice"
WITH person, company, length(path) as connection_distance
CALL orbit.graphrag.findPaths("business_kg", person.name, company.name, {max_hops: 3})
YIELD path_nodes, relationships, score, explanation
RETURN person.name, company.name, connection_distance, score, explanation;

// Use GraphRAG to find missing relationships
MATCH (p1:Paper), (p2:Paper)
WHERE NOT (p1)-[:CITES]-(p2) AND p1.field = p2.field
CALL orbit.graphrag.findPaths("research_kg", p1.title, p2.title, {max_hops: 2})
YIELD score
WHERE score > 0.8
CREATE (p1)-[:SEMANTICALLY_RELATED {score: score, inferred: true}]->(p2);
```

### 2. Aggregation with GraphRAG Results

```cypher
// Aggregate GraphRAG results by entity type
CALL orbit.graphrag.listEntities("news_kg", {limit: 1000})
YIELD entity_text, entity_type, confidence
WITH entity_type, collect({text: entity_text, conf: confidence}) as entities
RETURN entity_type,
       size(entities) as total_count,
       avg([e IN entities | e.conf]) as avg_confidence,
       [e IN entities | e.text][0..5] as sample_entities;
```

### 3. Conditional Processing

```cypher
// Conditional GraphRAG processing based on graph state
MATCH (kg:KnowledgeGraph {name: "research_kg"})
WHERE kg.last_updated < datetime() - duration({days: 1})
CALL orbit.graphrag.getStats(kg.name)
YIELD documents_processed, entities_extracted
CALL apoc.when(
    entities_extracted < 1000,
    "CALL orbit.graphrag.buildKnowledge($kg_name, $doc_id, $text, {}, {}) 
     YIELD entities_extracted RETURN entities_extracted",
    "",
    {kg_name: kg.name, doc_id: "update_doc", text: "Recent research updates..."}
) YIELD value
RETURN value;
```

## Result Types and Error Handling

### Standard Procedure Results
All GraphRAG procedures return consistent metadata:

```cypher
CALL orbit.graphrag.ragQuery("research_kg", "test query", {})
YIELD response, confidence, processing_time_ms, success, errors, warnings
WHERE success = true AND size(errors) = 0
RETURN response, confidence;
```

### Error Handling Patterns

```cypher
// Handle GraphRAG procedure errors gracefully
CALL orbit.graphrag.buildKnowledge("invalid_kg", "doc1", "text", {}, {})
YIELD success, errors, warnings
FOREACH (error IN CASE WHEN NOT success THEN errors ELSE [] END |
    CREATE (err:ProcessingError {
        message: error,
        timestamp: datetime(),
        procedure: "buildKnowledge"
    })
);
```

### Performance Monitoring

```cypher
// Monitor GraphRAG procedure performance
CALL orbit.graphrag.ragQuery("research_kg", "complex query", {})
YIELD response, processing_time_ms, entities_involved
WITH response, processing_time_ms, size(entities_involved) as entities_count
WHERE processing_time_ms > 5000  // Flag slow queries
CREATE (perf:PerformanceLog {
    query_type: "rag_query",
    processing_time: processing_time_ms,
    entities_involved: entities_count,
    timestamp: datetime(),
    slow_query_flag: true
});
```

This design provides comprehensive GraphRAG integration with Cypher through stored procedures while maintaining Neo4j conventions and graph-native patterns.