import os
import sys
from arango import ArangoClient, ArangoError

# ArangoDB connection parameters
ARANGO_HOST = os.getenv("ARANGO_HOST", "http://localhost:8529")
ARANGO_USER = os.getenv("ARANGO_USER", "root")
ARANGO_PASSWORD = os.getenv("ARANGO_PASSWORD", "password")
ARANGO_DB = os.getenv("ARANGO_DB", "_system")

def run_checks():
    print("Running AQL compatibility checks...")
    
    try:
        client = ArangoClient(hosts=ARANGO_HOST)
        sys_db = client.db(ARANGO_DB, username=ARANGO_USER, password=ARANGO_PASSWORD)
        
        # 1. Connection Check
        print("1. Connection to ArangoDB...", end=" ")
        version = sys_db.version()
        print(f"PASS (Version: {version})")
        
        # Setup test collection
        if not sys_db.has_collection("pci_test_coll"):
            sys_db.create_collection("pci_test_coll")
        
        checks = [
            ("Basic CRUD", "INSERT { value: 1 } INTO pci_test_coll RETURN NEW"),
            ("FOR Loop", "FOR doc IN pci_test_coll RETURN doc"),
            ("FILTER", "FOR doc IN pci_test_coll FILTER doc.value == 1 RETURN doc"),
            ("COLLECT (Aggregation)", "FOR doc IN pci_test_coll COLLECT val = doc.value WITH COUNT INTO length RETURN { val, length }"),
            ("Graph Traversal (Syntax)", "FOR v, e, p IN 1..1 OUTBOUND 'circles/A' GRAPH 'traversalGraph' RETURN p"), # Expect syntax pass, might fail runtime if graph missing
            ("Subquery", "RETURN (FOR i IN 1..3 RETURN i)"),
            ("Date Function", "RETURN DATE_NOW()"),
        ]
        
        failed = 0
        for name, query in checks:
            print(f"Testing {name}...", end=" ")
            try:
                # Validate syntax first
                sys_db.aql.validate(query)
                # Explain to check optimizer support
                sys_db.aql.explain(query)
                print("PASS")
            except ArangoError as e:
                # Some queries might fail explain if collections/graphs don't exist, which is expected for syntax checks
                # But we want to check if the *feature* is supported.
                # For now, we print the error.
                print(f"FAIL: {e}")
                failed += 1

        # Clean up
        if sys_db.has_collection("pci_test_coll"):
            sys_db.delete_collection("pci_test_coll")

        if failed > 0:
            print(f"\n{failed} checks failed.")
            sys.exit(1)
        else:
            print("\nAll AQL checks passed.")

    except Exception as e:
        print(f"\nCritical Error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    run_checks()
