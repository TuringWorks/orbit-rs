import os
import sys
import psycopg2
from psycopg2 import OperationalError

# PostgreSQL connection parameters
PG_HOST = os.getenv("PG_HOST", "localhost")
PG_PORT = os.getenv("PG_PORT", 5432)
PG_USER = os.getenv("PG_USER", "postgres")
PG_PASSWORD = os.getenv("PG_PASSWORD", "password")
PG_DBNAME = os.getenv("PG_DBNAME", "testdb")

def get_connection():
    try:
        return psycopg2.connect(
            host=PG_HOST,
            port=PG_PORT,
            user=PG_USER,
            password=PG_PASSWORD,
            dbname=PG_DBNAME,
        )
    except OperationalError as e:
        print(f"Connection failed: {e}")
        sys.exit(1)

def run_checks():
    conn = get_connection()
    conn.autocommit = True
    cursor = conn.cursor()
    
    print("Running pgvector compatibility checks...")
    
    checks = [
        ("Create Extension", "CREATE EXTENSION IF NOT EXISTS vector;"),
        ("Create Table", "DROP TABLE IF EXISTS items; CREATE TABLE items (id bigserial PRIMARY KEY, embedding vector(3));"),
        ("Insert Data", "INSERT INTO items (embedding) VALUES ('[1,2,3]'), ('[4,5,6]');"),
        ("L2 Distance Query", "SELECT * FROM items ORDER BY embedding <-> '[3,1,2]' LIMIT 1;"),
        ("Cosine Distance Query", "SELECT * FROM items ORDER BY embedding <=> '[3,1,2]' LIMIT 1;"),
        ("Inner Product Query", "SELECT * FROM items ORDER BY embedding <#> '[3,1,2]' LIMIT 1;"),
        ("Create IVFFlat Index", "CREATE INDEX ON items USING ivfflat (embedding vector_l2_ops) WITH (lists = 100);"),
        ("Create HNSW Index", "CREATE INDEX ON items USING hnsw (embedding vector_l2_ops);"),
    ]

    failed = 0
    for name, query in checks:
        try:
            print(f"Testing {name}...", end=" ")
            cursor.execute(query)
            print("PASS")
        except Exception as e:
            print(f"FAIL: {e}")
            failed += 1
            # Try to continue if possible, but some failures might block others
            
    cursor.close()
    conn.close()
    
    if failed > 0:
        print(f"\n{failed} checks failed.")
        sys.exit(1)
    else:
        print("\nAll pgvector checks passed.")

if __name__ == "__main__":
    run_checks()
