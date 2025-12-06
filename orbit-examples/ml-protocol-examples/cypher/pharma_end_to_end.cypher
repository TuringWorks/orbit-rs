// End-to-end pharma example in Cypher with embeddings and GraphRAG
// 1) Create trials, subjects, compounds; relationships
// 2) Generate embeddings for compound descriptions and compute similarity
// 3) Ingest safety policies and query via GraphRAG
CREATE (:Trial {name:'Trial-Alpha', phase:'Phase II'})
CREATE (:Subject {age:45})
CREATE (:Subject {age:60})
CREATE (:Compound {name:'Compound-A', description:'Selective inhibitor with anti-inflammatory profile'})
CREATE (:Compound {name:'Compound-B', description:'Broad-spectrum molecule with metabolic modulation'})
MATCH (t:Trial {name:'Trial-Alpha'}),(s:Subject {age:45}) CREATE (t)-[:ENROLLS]->(s)
MATCH (t:Trial {name:'Trial-Alpha'}),(s:Subject {age:60}) CREATE (t)-[:ENROLLS]->(s)
MATCH (s:Subject {age:45}),(c:Compound {name:'Compound-A'}) CREATE (s)-[:RECEIVES]->(c)
MATCH (s:Subject {age:60}),(c:Compound {name:'Compound-B'}) CREATE (s)-[:RECEIVES]->(c)
MATCH (c:Compound {name:'Compound-A'}) SET c.embedding = ML_EMBED_TEXT(c.description,'sentence-transformers')
MATCH (c:Compound {name:'Compound-B'}) SET c.embedding = ML_EMBED_TEXT(c.description,'sentence-transformers')
MATCH (a:Compound {name:'Compound-A'}),(b:Compound {name:'Compound-B'}) RETURN 1 - (a.embedding <=> b.embedding) AS compound_similarity
CALL orbit.graphrag.buildKnowledge('pharma_kg','policy_1','Serious adverse events must be reported within 24 hours to the safety board.',{source:'policy',domain:'pharma'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.buildKnowledge('pharma_kg','policy_2','High dosing protocols require enhanced monitoring for elderly subjects.',{source:'policy',domain:'pharma'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.ragQuery('pharma_kg','What actions apply to serious adverse events and high dosing in elderly?',{max_hops:2,include_path_reasoning:true}) YIELD response, confidence, reasoning_paths RETURN response, confidence, reasoning_paths;
