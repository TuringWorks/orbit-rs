import os
import sys
from neo4j import GraphDatabase, exceptions

# Neo4j connection parameters
NEO4J_URI = os.getenv("NEO4J_URI", "bolt://localhost:7687")
NEO4J_USER = os.getenv("NEO4J_USER", "neo4j")
NEO4J_PASSWORD = os.getenv("NEO4J_PASSWORD", "password")

def run_checks():
    print("Running Neo4j compatibility checks...")
    
    driver = None
    try:
        driver = GraphDatabase.driver(NEO4J_URI, auth=(NEO4J_USER, NEO4J_PASSWORD))
        
        # 1. Connection Check
        print("1. Connection to Neo4j...", end=" ")
        with driver.session() as session:
            # Check version if possible, or just basic connectivity
            result = session.run("RETURN 1 AS num")
            record = result.single()
            if record and record["num"] == 1:
                print("PASS")
            else:
                print("FAIL (Unexpected result)")
                sys.exit(1)
        
        checks = [
            ("Create Node", "CREATE (n:Person {name: 'Alice', age: 30}) RETURN n"),
            ("Match Node", "MATCH (n:Person {name: 'Alice'}) RETURN n"),
            ("Create Relationship", "MATCH (a:Person {name: 'Alice'}) CREATE (a)-[:KNOWS]->(b:Person {name: 'Bob'}) RETURN a, b"),
            ("Match Pattern", "MATCH (a:Person)-[:KNOWS]->(b:Person) RETURN a.name, b.name"),
            ("Delete Nodes", "MATCH (n:Person) DETACH DELETE n"),
            ("Unwind", "UNWIND [1, 2, 3] AS x RETURN x"),
            ("With Clause", "WITH 1 AS a RETURN a + 1"),
            ("Order By", "UNWIND [3, 1, 2] AS x RETURN x ORDER BY x"),
            ("Aggregation", "UNWIND [1, 1, 2] AS x RETURN count(x)"),
        ]
        
        failed = 0
        with driver.session() as session:
            for name, query in checks:
                print(f"Testing {name}...", end=" ")
                try:
                    # We can use EXPLAIN to check plan support, or just run it.
                    # Running it is better for full compatibility check.
                    session.run(query).consume()
                    print("PASS")
                except exceptions.Neo4jError as e:
                    print(f"FAIL: {e}")
                    failed += 1

        if failed > 0:
            print(f"\n{failed} checks failed.")
            sys.exit(1)
        else:
            print("\nAll Neo4j checks passed.")

    except Exception as e:
        print(f"\nCritical Error: {e}")
        sys.exit(1)
    finally:
        if driver:
            driver.close()

if __name__ == "__main__":
    run_checks()
