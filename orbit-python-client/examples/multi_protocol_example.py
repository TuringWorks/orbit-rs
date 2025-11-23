#!/usr/bin/env python3
"""
Multi-protocol example showing how to use different protocols with Orbit-RS.
"""

from orbit_client import OrbitClient


def postgres_example():
    """PostgreSQL example."""
    print("\n=== PostgreSQL Example ===")
    with OrbitClient.postgres() as client:
        # Create table
        client.execute("""
            CREATE TABLE IF NOT EXISTS products (
                id SERIAL PRIMARY KEY,
                name VARCHAR(100),
                price DECIMAL(10, 2)
            )
        """)
        
        # Insert data
        client.execute(
            "INSERT INTO products (name, price) VALUES (%s, %s)",
            params=("Laptop", 999.99)
        )
        
        # Query
        results = client.execute("SELECT * FROM products")
        print(f"Products: {results}")


def redis_example():
    """Redis example."""
    print("\n=== Redis Example ===")
    with OrbitClient.redis() as client:
        # Set values
        client.set("session:user1", "active", ex=3600)
        client.set("counter", "100")
        
        # Get values
        session = client.get("session:user1")
        counter = client.get("counter")
        print(f"Session: {session}, Counter: {counter}")
        
        # Hash operations
        client.execute("HSET", args=("user:1", "name", "Alice", "role", "admin"))


def cypher_example():
    """Cypher example."""
    print("\n=== Cypher Example ===")
    with OrbitClient.cypher() as client:
        # Create node
        client.execute(
            "CREATE (n:Product {name: $name, price: $price})",
            params={"name": "Laptop", "price": 999.99}
        )
        
        # Query
        results = client.execute("MATCH (n:Product) RETURN n")
        print(f"Products: {len(results)} found")


def aql_example():
    """AQL example."""
    print("\n=== AQL Example ===")
    with OrbitClient.aql() as client:
        # Create collection
        try:
            client._protocol_client._db.create_collection("products")
        except:
            pass  # Collection may already exist
        
        # Insert document
        client.execute(
            """
            INSERT {
                name: @name,
                price: @price
            } INTO products
            """,
            bind_vars={"name": "Laptop", "price": 999.99}
        )
        
        # Query
        results = client.execute(
            "FOR doc IN products RETURN doc",
            bind_vars={}
        )
        print(f"Products: {results}")


def main():
    """Run all protocol examples."""
    print("🚀 Orbit-RS Multi-Protocol Python Client Examples\n")
    
    try:
        postgres_example()
    except Exception as e:
        print(f"❌ PostgreSQL error: {e}")
    
    try:
        redis_example()
    except Exception as e:
        print(f"❌ Redis error: {e}")
    
    try:
        cypher_example()
    except Exception as e:
        print(f"❌ Cypher error: {e}")
    
    try:
        aql_example()
    except Exception as e:
        print(f"❌ AQL error: {e}")
    
    print("\n✅ Examples completed!")


if __name__ == "__main__":
    main()

