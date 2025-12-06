// End-to-end hospital systems example in Cypher with embeddings and GraphRAG
// 1) Create patients, notes; relationships
// 2) Generate embeddings for notes and compute similarity
// 3) Ingest care policies and query via GraphRAG
CREATE (:Patient {name:'Alice P', age:65})
CREATE (:Patient {name:'Bob Q', age:54})
CREATE (:Note {content:'Patient reports chills and fatigue.'})
CREATE (:Note {content:'Nurse observed elevated temperature.'})
CREATE (:Note {content:'Patient stable, no acute complaints.'})
MATCH (p:Patient {name:'Alice P'}),(n:Note {content:'Patient reports chills and fatigue.'}) CREATE (p)-[:HAS_NOTE]->(n)
MATCH (p:Patient {name:'Alice P'}),(n:Note {content:'Nurse observed elevated temperature.'}) CREATE (p)-[:HAS_NOTE]->(n)
MATCH (p:Patient {name:'Bob Q'}),(n:Note {content:'Patient stable, no acute complaints.'}) CREATE (p)-[:HAS_NOTE]->(n)
MATCH (n:Note {content:'Nurse observed elevated temperature.'}) SET n.embedding = ML_EMBED_TEXT(n.content,'sentence-transformers')
MATCH (n:Note {content:'Patient reports chills and fatigue.'}) SET n.embedding = ML_EMBED_TEXT(n.content,'sentence-transformers')
MATCH (a:Note {content:'Nurse observed elevated temperature.'}),(b:Note {content:'Patient reports chills and fatigue.'}) RETURN 1 - (a.embedding <=> b.embedding) AS note_similarity
CALL orbit.graphrag.buildKnowledge('hospital_kg','policy_1','Patients with signs of sepsis require immediate fluid resuscitation and antibiotics.',{source:'policy',domain:'hospital'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.buildKnowledge('hospital_kg','policy_2','Elevated temperature with tachycardia should trigger clinician review within 30 minutes.',{source:'policy',domain:'hospital'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.ragQuery('hospital_kg','What care steps apply to sepsis risk alerts?',{max_hops:2,include_path_reasoning:true}) YIELD response, confidence, reasoning_paths RETURN response, confidence, reasoning_paths;
