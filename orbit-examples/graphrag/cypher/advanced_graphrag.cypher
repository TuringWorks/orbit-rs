// Build knowledge graph from multiple docs
CALL orbit.graphrag.buildKnowledge(
  'research_kg','doc_dl',
  'Transformers use attention to model relationships in sequences; attention enables context fusion.',
  {source:'paper', category:'dl'},
  {extractors:['entity','relationship'], build_graph:true, generate_embeddings:true}
) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms
RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;

CALL orbit.graphrag.buildKnowledge(
  'research_kg','doc_gnn',
  'Graph Neural Networks (GNNs) operate over graph-structured data; message passing aggregates neighbor information.',
  {source:'wiki', category:'gnn'},
  {extractors:['entity','relationship'], build_graph:true, generate_embeddings:true}
) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms
RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;

// Collect context entities via graph patterns for RAG
MATCH p = (t:Concept {name:'Transformers'})-[*1..2]-(rel:Concept)
WITH collect(DISTINCT rel.name) AS ctx
CALL orbit.graphrag.ragQuery(
  'research_kg',
  'Explain how attention supports Transformers, and contrast with GNN message passing.',
  {max_hops:3, context_entities:ctx, include_path_reasoning:true}
) YIELD response, confidence, reasoning_paths
RETURN response, confidence, reasoning_paths;

// Multi-hop reasoning with path materialization and thresholding
MATCH (start:Concept {name:'Transformers'})
MATCH (end:Concept {name:'Graph Neural Networks'})
CALL orbit.graphrag.findPaths(
  'research_kg', start.name, end.name, {max_hops:4}
) YIELD path_nodes, relationships, score
WITH start, end, path_nodes, relationships, score
WHERE score > 0.75
UNWIND range(0, size(path_nodes)-2) AS i
WITH start, end, path_nodes, relationships, score, i,
     path_nodes[i] AS current_node,
     path_nodes[i+1] AS next_node,
     relationships[i] AS rel_info
MERGE (c:ReasoningNode {name: current_node})
MERGE (n:ReasoningNode {name: next_node})
MERGE (c)-[:REASON_LINK {type: rel_info.type, score: score}]->(n)
RETURN path_nodes, relationships, score;

// Semantic search blended with graph constraints
CALL orbit.graphrag.semanticSearch(
  'research_kg',
  'attention mechanisms in sequence models',
  {max_results: 15, max_hops: 2}
) YIELD type, content, relevance_score
WHERE type = 'entity' OR type = 'path'
RETURN type, content, relevance_score;
