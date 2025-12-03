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
    
    print("Running PostgreSQL 18 compatibility checks...")
    
    checks = [
        ("JSON_TABLE Support", "SELECT * FROM json_table('[{\"a\":10},{\"a\":20}]', '$[*]' COLUMNS (a int PATH '$.a')) AS jt;"),
        ("UUIDv7 Generation", "SELECT uuidv7();"),
        ("RETURNING OLD/NEW (Merge)", "CREATE TABLE IF NOT EXISTS test_merge (id int, val text); MERGE INTO test_merge t USING (VALUES (1, 'new')) AS s(id, val) ON t.id = s.id WHEN NOT MATCHED THEN INSERT VALUES (s.id, s.val) RETURNING NEW.val;"),
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
            
    cursor.close()
    conn.close()
    
    if failed > 0:
        print(f"\n{failed} checks failed.")
        sys.exit(1)
    else:
        print("\nAll PostgreSQL 18 checks passed.")

if __name__ == "__main__":
    run_checks()
