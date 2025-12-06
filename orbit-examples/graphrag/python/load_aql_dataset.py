import os
import requests

base_url = os.environ.get("ORBIT_AQL_URL", "http://localhost:8529")

docs = [
    ("business_kg","doc_b1","Apple Inc. acquired NeXT. Steve Jobs returned to Apple after the acquisition.",{"source":"news","category":"business"}),
    ("business_kg","doc_b2","Apple produces the iPhone and iPad.",{"source":"catalog","category":"products"}),
    ("research_kg","doc_r1","Transformers rely on attention mechanisms.",{"source":"paper","category":"ml"}),
    ("research_kg","doc_r2","Graph Neural Networks use message passing between nodes.",{"source":"wiki","category":"ml"})
]

def run_aql(query):
    requests.post(f"{base_url}/_api/cursor", json={"query": query})

def main():
    for kg, doc_id, text, meta in docs:
        q = (
            "FOR result IN GRAPHRAG_BUILD_KNOWLEDGE(@text, {"
            " \"knowledge_graph\": @kg, \"document_id\": @doc_id, \"metadata\": @meta, "
            " \"extractors\": [\"entity\", \"relationship\"], \"build_graph\": true, \"generate_embeddings\": true }) "
            " RETURN result"
        )
        payload = {"query": q, "bindVars": {"kg": kg, "doc_id": doc_id, "text": text, "meta": meta}}
        requests.post(f"{base_url}/_api/cursor", json=payload)

if __name__ == "__main__":
    main()
