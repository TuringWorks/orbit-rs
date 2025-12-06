import os
import psycopg2

host = os.environ.get("ORBIT_PG_HOST", "localhost")
port = int(os.environ.get("ORBIT_PG_PORT", "5432"))
db = os.environ.get("ORBIT_PG_DB", "orbit")
user = os.environ.get("ORBIT_PG_USER", "orbit")
password = os.environ.get("ORBIT_PG_PASSWORD", "orbit")

docs = [
    ("business_kg","doc_b1","Apple Inc. acquired NeXT. Steve Jobs returned to Apple after the acquisition.","{\"source\":\"news\",\"category\":\"business\"}"),
    ("business_kg","doc_b2","Apple produces the iPhone and iPad.","{\"source\":\"catalog\",\"category\":\"products\"}"),
    ("research_kg","doc_r1","Transformers rely on attention mechanisms.","{\"source\":\"paper\",\"category\":\"ml\"}"),
    ("research_kg","doc_r2","Graph Neural Networks use message passing between nodes.","{\"source\":\"wiki\",\"category\":\"ml\"}"),
]

def main():
    conn = psycopg2.connect(host=host, port=port, database=db, user=user, password=password)
    with conn, conn.cursor() as cur:
        for kg, doc_id, text, meta in docs:
            cur.execute(
                "SELECT * FROM GRAPHRAG_BUILD(%s,%s,%s,%s::json)",
                (kg, doc_id, text, meta)
            )
    conn.close()

if __name__ == "__main__":
    main()
