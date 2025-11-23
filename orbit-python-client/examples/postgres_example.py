#!/usr/bin/env python3
"""
PostgreSQL protocol example for Orbit-RS Python client.
"""

from orbit_client import OrbitClient


def main():
    """Example PostgreSQL usage."""
    # Connect to Orbit-RS PostgreSQL server
    client = OrbitClient.postgres(
        host="127.0.0.1",
        port=5432,
        username="orbit",
        password="",
        database="postgres"
    )
    
    try:
        # Create a table
        client.execute("""
            CREATE TABLE IF NOT EXISTS users (
                id SERIAL PRIMARY KEY,
                name VARCHAR(100) NOT NULL,
                email VARCHAR(100) UNIQUE NOT NULL,
                age INTEGER
            )
        """)
        print("✅ Table created")
        
        # Insert data
        client.execute(
            "INSERT INTO users (name, email, age) VALUES (%s, %s, %s)",
            params=("Alice", "alice@example.com", 30)
        )
        client.execute(
            "INSERT INTO users (name, email, age) VALUES (%s, %s, %s)",
            params=("Bob", "bob@example.com", 25)
        )
        print("✅ Data inserted")
        
        # Query data
        results = client.execute("SELECT * FROM users WHERE age > %s", params=(20,))
        print(f"\n📊 Found {len(results)} users:")
        for row in results:
            print(f"  - {row['name']} ({row['email']}), age {row['age']}")
        
        # Update data
        client.execute(
            "UPDATE users SET age = %s WHERE name = %s",
            params=(31, "Alice")
        )
        print("\n✅ Data updated")
        
        # Query again
        results = client.execute("SELECT * FROM users ORDER BY age")
        print(f"\n📊 All users (sorted by age):")
        for row in results:
            print(f"  - {row['name']}: {row['age']} years old")
        
    finally:
        client.disconnect()
        print("\n✅ Disconnected")


if __name__ == "__main__":
    main()

