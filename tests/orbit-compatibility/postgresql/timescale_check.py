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
    
    print("Running TimescaleDB compatibility checks...")
    
    checks = [
        ("Create Extension", "CREATE EXTENSION IF NOT EXISTS timescaledb;"),
        ("Create Table", "DROP TABLE IF EXISTS conditions; CREATE TABLE conditions (time TIMESTAMPTZ NOT NULL, location TEXT NOT NULL, temperature DOUBLE PRECISION, humidity DOUBLE PRECISION);"),
        ("Create Hypertable", "SELECT create_hypertable('conditions', 'time');"),
        ("Insert Data", "INSERT INTO conditions (time, location, temperature, humidity) VALUES (NOW(), 'office', 70.0, 50.0), (NOW() - INTERVAL '1 hour', 'office', 69.0, 51.0);"),
        ("Time Bucket Query", "SELECT time_bucket('15 minutes', time) AS bucket, avg(temperature) FROM conditions GROUP BY bucket ORDER BY bucket DESC;"),
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
        print("\nAll TimescaleDB checks passed.")

if __name__ == "__main__":
    run_checks()
