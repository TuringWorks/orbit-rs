#!/usr/bin/env python3
"""
Cypher/Bolt protocol example for Orbit-RS Python client.
"""

from orbit_client import OrbitClient


def main():
    """Example Cypher usage."""
    # Connect to Orbit-RS Cypher/Bolt server
    client = OrbitClient.cypher(
        host="127.0.0.1",
        port=7687,
        database="neo4j"
    )
    
    try:
        # Create nodes
        client.execute(
            "CREATE (alice:Person {name: $name, age: $age})",
            params={"name": "Alice", "age": 30}
        )
        client.execute(
            "CREATE (bob:Person {name: $name, age: $age})",
            params={"name": "Bob", "age": 25}
        )
        print("✅ Nodes created")
        
        # Create relationships
        client.execute(
            """
            MATCH (a:Person {name: $name1}), (b:Person {name: $name2})
            CREATE (a)-[:KNOWS {since: $since}]->(b)
            """,
            params={"name1": "Alice", "name2": "Bob", "since": 2020}
        )
        print("✅ Relationship created")
        
        # Query nodes
        results = client.execute(
            "MATCH (n:Person) WHERE n.age > $age RETURN n.name AS name, n.age AS age",
            params={"age": 20}
        )
        print(f"\n📊 Found {len(results)} people:")
        for record in results:
            print(f"  - {record['name']}: {record['age']} years old")
        
        # Query relationships
        results = client.execute(
            """
            MATCH (a:Person)-[r:KNOWS]->(b:Person)
            RETURN a.name AS person1, b.name AS person2, r.since AS since
            """
        )
        print(f"\n📊 Relationships:")
        for record in results:
            print(f"  - {record['person1']} knows {record['person2']} since {record['since']}")
        
        # Complex query with WHERE clause
        results = client.execute(
            """
            MATCH (n:Person)
            WHERE n.age > $min_age AND n.age < $max_age
            RETURN n.name AS name, n.age AS age
            ORDER BY n.age
            """,
            params={"min_age": 20, "max_age": 35}
        )
        print(f"\n📊 People aged 20-35:")
        for record in results:
            print(f"  - {record['name']}: {record['age']} years old")
        
    finally:
        client.disconnect()
        print("\n✅ Disconnected")


if __name__ == "__main__":
    main()

