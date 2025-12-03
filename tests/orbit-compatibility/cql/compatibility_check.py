import os
import sys
from cassandra.cluster import Cluster
from cassandra.auth import PlainTextAuthProvider
from cassandra.query import SimpleStatement

# Cassandra connection parameters
CASSANDRA_HOST = os.getenv("CASSANDRA_HOST", "localhost")
CASSANDRA_PORT = int(os.getenv("CASSANDRA_PORT", 9042))
CASSANDRA_USER = os.getenv("CASSANDRA_USER", "cassandra")
CASSANDRA_PASSWORD = os.getenv("CASSANDRA_PASSWORD", "cassandra")

def run_checks():
    print("Running CQL compatibility checks...")

    try:
        # Use protocol_version=4 for compatibility with Orbit CQL adapter
        # Disable auth for initial testing
        cluster = Cluster(
            [CASSANDRA_HOST],
            port=CASSANDRA_PORT,
            protocol_version=4,  # Force CQL v4 protocol
            connect_timeout=10,
            control_connection_timeout=10,
        )
        session = cluster.connect()
        
        # 1. Connection Check
        print("1. Connection to Cassandra...", end=" ")
        row = session.execute("SELECT release_version FROM system.local").one()
        print(f"PASS (Version: {row.release_version})")
        
        # Setup test keyspace
        session.execute("CREATE KEYSPACE IF NOT EXISTS pci_test_ks WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1}")
        session.set_keyspace("pci_test_ks")
        
        checks = [
            ("Create Table", "CREATE TABLE IF NOT EXISTS users (user_id UUID PRIMARY KEY, first_name text, last_name text, emails set<text>)"),
            ("Insert Data", "INSERT INTO users (user_id, first_name, last_name, emails) VALUES (uuid(), 'Jane', 'Doe', {'jane@example.com'})"),
            ("Select Data", "SELECT * FROM users"),
            ("Secondary Index", "CREATE INDEX IF NOT EXISTS user_last_name ON users (last_name)"),
            ("Materialized View", "CREATE MATERIALIZED VIEW IF NOT EXISTS users_by_email AS SELECT * FROM users WHERE emails IS NOT NULL AND user_id IS NOT NULL PRIMARY KEY (emails, user_id)"),
            ("UDT Support", "CREATE TYPE IF NOT EXISTS address (street text, city text, zip int)"),
            ("Table with UDT", "CREATE TABLE IF NOT EXISTS locations (id int PRIMARY KEY, addr frozen<address>)"),
            ("Batch Insert", "BEGIN BATCH INSERT INTO users (user_id, first_name) VALUES (uuid(), 'Batch'); APPLY BATCH;"),
            ("Lightweight Transaction (LWT)", "INSERT INTO users (user_id, first_name) VALUES (uuid(), 'LWT') IF NOT EXISTS"),
        ]
        
        failed = 0
        for name, query in checks:
            print(f"Testing {name}...", end=" ")
            try:
                session.execute(query)
                print("PASS")
            except Exception as e:
                # Materialized views might be disabled or experimental in some setups
                print(f"FAIL: {e}")
                failed += 1

        # Clean up
        session.execute("DROP KEYSPACE IF EXISTS pci_test_ks")
        cluster.shutdown()

        if failed > 0:
            print(f"\n{failed} checks failed.")
            sys.exit(1)
        else:
            print("\nAll CQL checks passed.")

    except Exception as e:
        print(f"\nCritical Error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    run_checks()
