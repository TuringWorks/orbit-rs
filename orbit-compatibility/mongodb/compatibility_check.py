import os
import sys
from pymongo import MongoClient, errors

# MongoDB connection parameters
MONGO_URI = os.getenv("MONGO_URI", "mongodb://localhost:27017/")
MONGO_DB = os.getenv("MONGO_DB", "pci_test_db")

def run_checks():
    print("Running MongoDB compatibility checks...")
    
    client = None
    try:
        client = MongoClient(MONGO_URI, serverSelectionTimeoutMS=5000)
        
        # 1. Connection Check
        print("1. Connection to MongoDB...", end=" ")
        info = client.server_info()
        print(f"PASS (Version: {info.get('version')})")
        
        db = client[MONGO_DB]
        collection = db["test_collection"]
        
        # Clean up before start
        collection.drop()
        
        checks = [
            ("Insert One", lambda: collection.insert_one({"name": "Alice", "age": 30})),
            ("Insert Many", lambda: collection.insert_many([{"name": "Bob", "age": 25}, {"name": "Charlie", "age": 35}])),
            ("Find One", lambda: collection.find_one({"name": "Alice"})),
            ("Find Many", lambda: list(collection.find({"age": {"$gt": 20}}))),
            ("Update One", lambda: collection.update_one({"name": "Alice"}, {"$set": {"age": 31}})),
            ("Delete One", lambda: collection.delete_one({"name": "Bob"})),
            ("Count Documents", lambda: collection.count_documents({})),
            ("Aggregate", lambda: list(collection.aggregate([{"$group": {"_id": "$age", "count": {"$sum": 1}}}]))),
            ("Create Index", lambda: collection.create_index("name")),
        ]
        
        failed = 0
        for name, func in checks:
            print(f"Testing {name}...", end=" ")
            try:
                func()
                print("PASS")
            except Exception as e:
                print(f"FAIL: {e}")
                failed += 1

        # Clean up
        client.drop_database(MONGO_DB)

        if failed > 0:
            print(f"\n{failed} checks failed.")
            sys.exit(1)
        else:
            print("\nAll MongoDB checks passed.")

    except errors.ServerSelectionTimeoutError:
        print("\nFAIL: Could not connect to MongoDB server.")
        sys.exit(1)
    except Exception as e:
        print(f"\nCritical Error: {e}")
        sys.exit(1)
    finally:
        if client:
            client.close()

if __name__ == "__main__":
    run_checks()
