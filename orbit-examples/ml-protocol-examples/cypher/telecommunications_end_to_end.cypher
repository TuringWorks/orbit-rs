// End-to-end telecommunications example in Cypher with embeddings and GraphRAG
// 1) Create subscribers, towers, and relationships
// 2) Generate embeddings for issue descriptions and compute similarity
// 3) Ingest telecom policies and query via GraphRAG
CREATE (:Subscriber {name:'Eve', segment:'premium'})
CREATE (:Subscriber {name:'Frank', segment:'standard'})
CREATE (:Tower {name:'Tower-1', latitude:37.7749, longitude:-122.4194})
CREATE (:Tower {name:'Tower-2', latitude:37.7840, longitude:-122.4090})
MATCH (s:Subscriber {name:'Eve'}),(t:Tower {name:'Tower-1'}) CREATE (s)-[:CONNECTS_TO]->(t)
MATCH (s:Subscriber {name:'Frank'}),(t:Tower {name:'Tower-2'}) CREATE (s)-[:CONNECTS_TO]->(t)
CREATE (:Issue {text:'Network congestion at peak hours'})
CREATE (:Issue {text:'Hardware fault causing signal instability'})
MATCH (i:Issue {text:'Network congestion at peak hours'}) SET i.embedding = ML_EMBED_TEXT(i.text,'sentence-transformers')
MATCH (i:Issue {text:'Hardware fault causing signal instability'}) SET i.embedding = ML_EMBED_TEXT(i.text,'sentence-transformers')
MATCH (a:Issue {text:'Network congestion at peak hours'}),(b:Issue {text:'Hardware fault causing signal instability'}) RETURN 1 - (a.embedding <=> b.embedding) AS issue_similarity
CALL orbit.graphrag.buildKnowledge('telecom_kg','policy_1','Subscribers with drop rate above 3% in the last hour should receive proactive outreach.',{source:'policy',domain:'telecommunications'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.buildKnowledge('telecom_kg','policy_2','Low signal areas must be prioritized in tower optimization within 7 days.',{source:'policy',domain:'telecommunications'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.ragQuery('telecom_kg','What actions are recommended for high drop rates and low signal areas?',{max_hops:2,include_path_reasoning:true}) YIELD response, confidence, reasoning_paths RETURN response, confidence, reasoning_paths;
