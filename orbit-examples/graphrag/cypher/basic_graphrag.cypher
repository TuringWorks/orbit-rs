CALL orbit.graphrag.buildKnowledge(
  'business_kg',
  'doc_apple',
  'Apple Inc. was founded by Steve Jobs in 1976. Steve Jobs co-founded Apple with Steve Wozniak and Ronald Wayne.',
  {source:'news'},
  {extractors:['entity','relationship'], build_graph:true, generate_embeddings:true}
) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms
RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.buildKnowledge(
  'business_kg',
  'doc_iphone',
  'Apple develops the iPhone. The iPhone is a smartphone product by Apple Inc.',
  {source:'catalog'},
  {extractors:['entity','relationship'], build_graph:true, generate_embeddings:true}
) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms
RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.ragQuery(
  'business_kg',
  'What is the relationship between Apple and Steve Jobs?',
  {max_hops:2, include_path_reasoning:true}
) YIELD response, confidence, reasoning_paths
RETURN response, confidence, reasoning_paths;
CALL orbit.graphrag.findPaths(
  'business_kg',
  'Apple Inc.',
  'iPhone',
  {max_hops:3}
) YIELD path_nodes, relationships, score
RETURN path_nodes, relationships, score;
