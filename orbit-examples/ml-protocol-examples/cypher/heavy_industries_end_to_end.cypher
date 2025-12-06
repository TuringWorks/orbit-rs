// End-to-end heavy industries example in Cypher with embeddings and GraphRAG
// 1) Create equipment, incidents; relationships
// 2) Generate embeddings for incident types and compute similarity
// 3) Ingest safety policies and query via GraphRAG
CREATE (:Equipment {name:'Compressor-1', type:'compressor'})
CREATE (:Equipment {name:'Smelter-2', type:'smelter'})
CREATE (:Incident {name:'Unexpected shutdown'})
CREATE (:Incident {name:'Overheat warning'})
MATCH (e:Equipment {name:'Compressor-1'}),(i:Incident {name:'Unexpected shutdown'}) CREATE (e)-[:HAS_INCIDENT]->(i)
MATCH (i:Incident {name:'Unexpected shutdown'}) SET i.embedding = ML_EMBED_TEXT(i.name,'sentence-transformers')
MATCH (i:Incident {name:'Overheat warning'}) SET i.embedding = ML_EMBED_TEXT(i.name,'sentence-transformers')
MATCH (a:Incident {name:'Unexpected shutdown'}),(b:Incident {name:'Overheat warning'}) RETURN 1 - (a.embedding <=> b.embedding) AS incident_similarity
CALL orbit.graphrag.buildKnowledge('heavy_kg','policy_1','Equipment with frequent overheat events must reduce load and schedule maintenance within 24 hours.',{source:'policy',domain:'heavy'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.buildKnowledge('heavy_kg','policy_2','Unexpected shutdowns require root cause analysis before next shift.',{source:'policy',domain:'heavy'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.ragQuery('heavy_kg','What actions apply to overheat events and unexpected shutdowns?',{max_hops:2,include_path_reasoning:true}) YIELD response, confidence, reasoning_paths RETURN response, confidence, reasoning_paths;
