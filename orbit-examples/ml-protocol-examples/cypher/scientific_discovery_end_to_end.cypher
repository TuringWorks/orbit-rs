// End-to-end scientific discovery example in Cypher with embeddings and GraphRAG
// 1) Create papers, experiments; relationships
// 2) Generate embeddings for abstracts and compute similarity
// 3) Ingest methodology policies and query via GraphRAG
CREATE (:Paper {title:'Quantum Coherence in Novel Materials', abstract:'We investigate coherence properties and experimental constraints.'})
CREATE (:Paper {title:'Catalysis Efficiency in New Compounds', abstract:'Efficiency of catalysts measured across varying conditions.'})
CREATE (:Experiment {name:'Exp-Q1'})
CREATE (:Experiment {name:'Exp-C1'})
MATCH (p:Paper {title:'Quantum Coherence in Novel Materials'}),(e:Experiment {name:'Exp-Q1'}) CREATE (p)-[:HAS_EXPERIMENT]->(e)
MATCH (p:Paper {title:'Catalysis Efficiency in New Compounds'}),(e:Experiment {name:'Exp-C1'}) CREATE (p)-[:HAS_EXPERIMENT]->(e)
MATCH (p:Paper {title:'Quantum Coherence in Novel Materials'}) SET p.embedding = ML_EMBED_TEXT(p.abstract,'sentence-transformers')
MATCH (p:Paper {title:'Catalysis Efficiency in New Compounds'}) SET p.embedding = ML_EMBED_TEXT(p.abstract,'sentence-transformers')
MATCH (a:Paper {title:'Quantum Coherence in Novel Materials'}),(b:Paper {title:'Catalysis Efficiency in New Compounds'}) RETURN 1 - (a.embedding <=> b.embedding) AS abstract_similarity
CALL orbit.graphrag.buildKnowledge('science_kg','policy_1','Experiments showing anomalies must include replication steps and instrument calibration logs.',{source:'policy',domain:'scientific'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.buildKnowledge('science_kg','policy_2','Catalysis efficiency reports require standardized measurement methodology and variance analysis.',{source:'policy',domain:'scientific'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.ragQuery('science_kg','What methodology policies apply to anomaly reports and catalysis efficiency?',{max_hops:2,include_path_reasoning:true}) YIELD response, confidence, reasoning_paths RETURN response, confidence, reasoning_paths;
