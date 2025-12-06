// End-to-end automotive example in Cypher with embeddings and GraphRAG
// 1) Create vehicles, drivers, components; link relationships
// 2) Generate embeddings for component descriptions and compute similarity
// 3) Ingest maintenance policies and query via GraphRAG
CREATE (:Vehicle {vin:'VIN12345', model:'Sedan-X', year:2022})
CREATE (:Vehicle {vin:'VIN67890', model:'SUV-Y', year:2021})
CREATE (:Driver {name:'Dana'})
CREATE (:Driver {name:'Chris'})
// Driver-to-vehicle relationships
MATCH (v:Vehicle {vin:'VIN12345'}),(d:Driver {name:'Dana'}) CREATE (d)-[:DRIVES]->(v)
MATCH (v:Vehicle {vin:'VIN67890'}),(d:Driver {name:'Chris'}) CREATE (d)-[:DRIVES]->(v)
CREATE (:Component {name:'Engine'})
CREATE (:Component {name:'OilPressureSensor'})
// Vehicle components
MATCH (v:Vehicle {vin:'VIN12345'}),(c:Component {name:'Engine'}) CREATE (v)-[:HAS_COMPONENT]->(c)
MATCH (v:Vehicle {vin:'VIN12345'}),(c:Component {name:'OilPressureSensor'}) CREATE (v)-[:HAS_COMPONENT]->(c)
// Embeddings for component issue descriptions
MATCH (c:Component {name:'Engine'}) SET c.embedding = ML_EMBED_TEXT('Engine Overheating Condition','sentence-transformers')
MATCH (c:Component {name:'OilPressureSensor'}) SET c.embedding = ML_EMBED_TEXT('Engine Oil Pressure Sensor Range/Performance','sentence-transformers')
// Cosine similarity between components
MATCH (e:Component {name:'Engine'}),(o:Component {name:'OilPressureSensor'}) RETURN 1 - (e.embedding <=> o.embedding) AS component_similarity
// GraphRAG knowledge ingestion for maintenance policies
CALL orbit.graphrag.buildKnowledge('automotive_kg','policy_1','Vehicles with engine temperature above 105C should be scheduled for inspection within 24 hours.',{source:'policy',domain:'automotive'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.buildKnowledge('automotive_kg','policy_2','Persistent low oil pressure requires immediate service and diagnostic testing.',{source:'policy',domain:'automotive'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
// RAG query with path reasoning to explain relevant rules
CALL orbit.graphrag.ragQuery('automotive_kg','What rules affect high engine temperature and low oil pressure incidents?',{max_hops:2,include_path_reasoning:true}) YIELD response, confidence, reasoning_paths RETURN response, confidence, reasoning_paths;
