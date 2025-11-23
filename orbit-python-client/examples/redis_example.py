#!/usr/bin/env python3
"""
Redis/RESP protocol example for Orbit-RS Python client.
"""

from orbit_client import OrbitClient


def main():
    """Example Redis usage."""
    # Connect to Orbit-RS Redis server
    client = OrbitClient.redis(
        host="127.0.0.1",
        port=6379
    )
    
    try:
        # String operations
        client.set("user:1", "Alice")
        client.set("user:2", "Bob", ex=3600)  # With expiration
        print("✅ Keys set")
        
        # Get values
        user1 = client.get("user:1")
        user2 = client.get("user:2")
        print(f"✅ Retrieved: user:1 = {user1}, user:2 = {user2}")
        
        # Hash operations
        client.execute("HSET", args=("user:profile:1", "name", "Alice", "age", "30", "city", "NYC"))
        profile = client.execute("HGETALL", args=("user:profile:1",))
        print(f"✅ User profile: {profile}")
        
        # List operations
        client.execute("LPUSH", args=("tasks", "task1", "task2", "task3"))
        tasks = client.execute("LRANGE", args=("tasks", 0, -1))
        print(f"✅ Tasks: {tasks}")
        
        # Set operations
        client.execute("SADD", args=("tags", "python", "rust", "database"))
        tags = client.execute("SMEMBERS", args=("tags",))
        print(f"✅ Tags: {tags}")
        
        # Sorted set operations
        client.execute("ZADD", args=("leaderboard", 100, "Alice", 200, "Bob", 150, "Charlie"))
        top_users = client.execute("ZREVRANGE", args=("leaderboard", 0, 2, "WITHSCORES"))
        print(f"✅ Top users: {top_users}")
        
        # Get all keys
        all_keys = client.keys("*")
        print(f"\n📊 All keys: {all_keys}")
        
        # Cleanup
        client.delete("user:1", "user:2", "user:profile:1", "tasks", "tags", "leaderboard")
        print("\n✅ Cleaned up")
        
    finally:
        client.disconnect()
        print("✅ Disconnected")


if __name__ == "__main__":
    main()

