// End-to-end defense example in Cypher with embeddings and GraphRAG
// 1) Create assets, missions, threats; relationships
// 2) Generate embeddings for threat descriptions and compute similarity
// 3) Ingest response policies and query via GraphRAG
CREATE (:Asset {name:'Drone-Alpha', type:'UAV'})
CREATE (:Asset {name:'Radar-1', type:'Sensor'})
CREATE (:Mission {name:'Recon-Op', status:'active'})
CREATE (:Threat {code:'THR_VIB', description:'High vibration pattern indicative of mechanical stress'})
CREATE (:Threat {code:'THR_SIG', description:'Signal degradation likely due to interference'})
MATCH (a:Asset {name:'Drone-Alpha'}),(m:Mission {name:'Recon-Op'}) CREATE (a)-[:ASSIGNED_TO]->(m)
MATCH (t:Threat {code:'THR_VIB'}) SET t.embedding = ML_EMBED_TEXT(t.description,'sentence-transformers')
MATCH (t:Threat {code:'THR_SIG'}) SET t.embedding = ML_EMBED_TEXT(t.description,'sentence-transformers')
MATCH (x:Threat {code:'THR_VIB'}),(y:Threat {code:'THR_SIG'}) RETURN 1 - (x.embedding <=> y.embedding) AS threat_similarity
CALL orbit.graphrag.buildKnowledge('defense_kg','policy_1','Assets with sustained high vibration must be grounded for inspection within 2 hours.',{source:'policy',domain:'defense'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.buildKnowledge('defense_kg','policy_2','Signal interference incidents require spectrum analysis and mission reassessment.',{source:'policy',domain:'defense'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.ragQuery('defense_kg','What response policies apply to vibration anomalies and signal degradation?',{max_hops:2,include_path_reasoning:true}) YIELD response, confidence, reasoning_paths RETURN response, confidence, reasoning_paths;
