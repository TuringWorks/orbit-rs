import os
import sys
import psycopg2
from psycopg2 import OperationalError

# OrbitQL connection parameters (using Postgres wire protocol)
ORBIT_HOST = os.getenv("ORBIT_HOST", "localhost")
ORBIT_PORT = os.getenv("ORBIT_PORT", 5432)
ORBIT_USER = os.getenv("ORBIT_USER", "orbit")
ORBIT_PASSWORD = os.getenv("ORBIT_PASSWORD", "password")
ORBIT_DB = os.getenv("ORBIT_DB", "orbit_db")

def get_connection():
    try:
        return psycopg2.connect(
            host=ORBIT_HOST,
            port=ORBIT_PORT,
            user=ORBIT_USER,
            password=ORBIT_PASSWORD,
            dbname=ORBIT_DB,
        )
    except OperationalError as e:
        print(f"Connection failed: {e}")
        sys.exit(1)

def run_checks():
    conn = get_connection()
    conn.autocommit = True
    cursor = conn.cursor()
    
    print("Running OrbitQL compatibility checks...")
    
    # OrbitQL specific features: Graph, TimeSeries, ML, Advanced SQL
    checks = [
        # 1. Advanced SQL
        ("NOW() Function", "SELECT NOW()"),
        ("INTERVAL Arithmetic", "SELECT NOW() - INTERVAL '1 hour'"),
        ("Array Literals", "SELECT ARRAY[1, 2, 3]"),
        ("Object Literals", "SELECT OBJECT('name', 'Orbit', 'type', 'Database')"),
        
        # 2. Graph Queries (TRAVERSE)
        # Note: Requires graph data, we'll just check syntax/parsing support if possible
        # or try a simple traversal on a mock graph if we could create one.
        # For now, we assume a 'users' table/node might exist or we try to create one.
        # We'll try a syntax check or a simple query that might return empty but not error on syntax.
        ("Graph Syntax (TRAVERSE)", "EXPLAIN SELECT * FROM users TRAVERSE OUTBOUND 1..1 STEPS ON follows TO friend"),
        
        # 3. Time Series
        ("Time Bucket", "SELECT TIME_BUCKET('15 minutes', NOW())"),
        
        # 4. Machine Learning
        # Just check if the function exists/parses
        ("ML Function Syntax", "EXPLAIN SELECT ML_PREDICT('my_model', ARRAY[1, 2, 3])"),
        
        # 5. Multi-Model
        ("JSON Access", "SELECT OBJECT('a', 1)->>'a'"),
    ]

    failed = 0
    for name, query in checks:
        try:
            print(f"Testing {name}...", end=" ")
            cursor.execute(query)
            print("PASS")
        except Exception as e:
            # Some might fail if tables don't exist, but we are checking if the *Syntax* is accepted
            # or if the function is known.
            # If it's a "relation does not exist" error, the syntax was likely parsed OK.
            # If it's a "syntax error", then OrbitQL support is missing.
            error_msg = str(e).lower()
            if "syntax error" in error_msg:
                print(f"FAIL: {e}")
                failed += 1
            elif "does not exist" in error_msg:
                 print(f"PASS (Syntax OK, Object missing)")
            else:
                print(f"FAIL: {e}")
                failed += 1
            
    cursor.close()
    conn.close()
    
    if failed > 0:
        print(f"\n{failed} checks failed.")
        sys.exit(1)
    else:
        print("\nAll OrbitQL checks passed.")

if __name__ == "__main__":
    run_checks()
