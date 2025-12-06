// End-to-end retail example in Cypher with embeddings and GraphRAG
// 1) Create customers, products, orders; relationships
// 2) Generate embeddings for product descriptions and compute similarity
// 3) Ingest retail policies and query via GraphRAG
CREATE (:Customer {name:'Rita', segment:'vip'})
CREATE (:Customer {name:'Sam', segment:'standard'})
CREATE (:Product {name:'4K TV', category:'electronics', description:'High-resolution television ideal for home theaters'})
CREATE (:Product {name:'Espresso Machine', category:'kitchen', description:'Compact espresso maker for coffee enthusiasts'})
CREATE (:Product {name:'Running Shoes', category:'fashion', description:'Lightweight shoes suitable for daily running'})
MATCH (c:Customer {name:'Rita'}),(p:Product {name:'Espresso Machine'}) CREATE (c)-[:BOUGHT]->(p)
MATCH (c:Customer {name:'Rita'}),(p:Product {name:'4K TV'}) CREATE (c)-[:BOUGHT]->(p)
MATCH (c:Customer {name:'Sam'}),(p:Product {name:'Running Shoes'}) CREATE (c)-[:BOUGHT]->(p)
MATCH (p:Product {name:'Espresso Machine'}) SET p.embedding = ML_EMBED_TEXT(p.description,'sentence-transformers')
MATCH (p:Product {name:'4K TV'}) SET p.embedding = ML_EMBED_TEXT(p.description,'sentence-transformers')
MATCH (a:Product {name:'Espresso Machine'}),(b:Product {name:'4K TV'}) RETURN 1 - (a.embedding <=> b.embedding) AS product_similarity
CALL orbit.graphrag.buildKnowledge('retail_kg','policy_1','High-value carts above $500 require address verification before shipping.',{source:'policy',domain:'retail'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.buildKnowledge('retail_kg','policy_2','VIP customers receive free expedited shipping unless flagged by risk systems.',{source:'policy',domain:'retail'},{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms;
CALL orbit.graphrag.ragQuery('retail_kg','What rules apply to VIP customers with high-value carts?',{max_hops:2,include_path_reasoning:true}) YIELD response, confidence, reasoning_paths RETURN response, confidence, reasoning_paths;
