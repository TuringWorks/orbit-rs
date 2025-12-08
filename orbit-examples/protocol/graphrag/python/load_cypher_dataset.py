import os
from neo4j import GraphDatabase

uri = os.environ.get("ORBIT_CYPHER_URI", "bolt://localhost:7687")
user = os.environ.get("ORBIT_CYPHER_USER", "orbit")
password = os.environ.get("ORBIT_CYPHER_PASSWORD", "orbit")

docs = [
    ("business_kg","doc_b1","Apple Inc. acquired NeXT. Steve Jobs returned to Apple after the acquisition.",{ "source":"news","category":"business" }),
    ("business_kg","doc_b2","Apple produces the iPhone and iPad.",{ "source":"catalog","category":"products" }),
    ("research_kg","doc_r1","Transformers rely on attention mechanisms.",{ "source":"paper","category":"ml" }),
    ("research_kg","doc_r2","Graph Neural Networks use message passing between nodes.",{ "source":"wiki","category":"ml" })
]

def main():
    driver = GraphDatabase.driver(uri, auth=(user, password))
    with driver.session() as session:
        for kg, doc_id, text, meta in docs:
            q = (
                "CALL orbit.graphrag.buildKnowledge($kg,$doc_id,$text,$meta,{extractors:['entity','relationship'],build_graph:true,generate_embeddings:true}) "
                "YIELD kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms "
                "RETURN kg_name, document_id, entities_extracted, relationships_extracted, processing_time_ms"
            )
            res = session.run(q, {"kg": kg, "doc_id": doc_id, "text": text, "meta": meta})
            list(res)
    driver.close()

if __name__ == "__main__":
    main()
