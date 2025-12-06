// End-to-end aerospace example in Cypher with embeddings and GraphRAG
// 1) Create aircraft, flights, faults; relationships
// 2) Generate embeddings for fault descriptions and compute similarity
// 3) Ingest procedures and query via GraphRAG
CREATE (:Aircraft {tail:'N123AB', model:'A320'})
CREATE (:Aircraft {tail:'N987XY', model:'B737'})
CREATE (:Flight {route:'SFO-LAX', status:'active'})
MATCH (a:Aircraft {tail:'N123AB'}),(f:Flight {route:'SFO-LAX'}) CREATE (a)-[:OPERATES]->(f)
CREATE (:Fault {code:'ENG_TEMP', description:'Engine over-temperature condition'})
CREATE (:Fault {code:'SPD_FLUCT', description:'Speed fluctuation beyond tolerance'})
MATCH (x:Fault {code:'ENG_TEMP'}) SET x.embedding = ML_EMBED_TEXT(x.description,'sentence-transformers')
MATCH (y:Fault {code:'SPD_FLUCT'}) SET y.embedding = ML_EMBED_TEXT(y.description,'sentence-transformers')
MATCH (x:Fault {code:'ENG_TEMP'}),(y:Fault {code:'SPD_FLUCT'}) RETURN 1 - (x.embedding <=> y.embedding) AS fault_similarity
CALL orbit.graphrag.buildKnowledge('aerospace_kg','proc_1','Engine over-temperature requires thrust reduction and cooling checks upon landing.',{source:'procedure',domain:'aerospace'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.buildKnowledge('aerospace_kg','proc_2','Persistent speed fluctuations require pitot-static system inspection within 24 hours.',{source:'procedure',domain:'aerospace'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.ragQuery('aerospace_kg','What procedures apply to engine over-temperature and speed fluctuations?',{max_hops:2,include_path_reasoning:true}) YIELD response, confidence, reasoning_paths RETURN response, confidence, reasoning_paths;
