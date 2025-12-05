import psycopg2
import sys
import os

def get_connection():
    try:
        conn = psycopg2.connect(
            host="localhost",
            port=os.environ.get("PG_PORT", "5432"),
            user="postgres",
            password="password",
            database="postgres"
        )
        conn.autocommit = True
        return conn
    except Exception as e:
        print(f"Failed to connect: {e}")
        sys.exit(1)

def test_array_types():
    conn = get_connection()
    cur = conn.cursor()
    
    print("Testing PostgreSQL Array Types...")
    
    try:
        # Test 1: Array type declaration
        print("  1. Array type declaration...", end="", flush=True)
        cur.execute("""
            CREATE TABLE test_arrays (
                id INTEGER,
                tags TEXT[],
                scores FLOAT[]
            )
        """)
        print(" PASS")
        
        # Test 2: ARRAY literal syntax
        print("  2. ARRAY literal syntax...", end="", flush=True)
        cur.execute("""
            INSERT INTO test_arrays VALUES (
                1,
                ARRAY['tag1', 'tag2', 'tag3'],
                ARRAY[0.5, 0.8, 0.9]
            )
        """)
        print(" PASS")
        
        # Test 3: Multi-dimensional arrays
        print("  3. Multi-dimensional arrays...", end="", flush=True)
        cur.execute("""
            CREATE TABLE test_multi_arrays (
                id INTEGER,
                matrix INTEGER[][]
            )
        """)
        print(" PASS")
        
        # Test 4: Fixed-size arrays
        print("  4. Fixed-size arrays...", end="", flush=True)
        cur.execute("""
            CREATE TABLE test_fixed_arrays (
                id INTEGER,
                fixed_array INTEGER[10]
            )
        """)
        print(" PASS")

    except Exception as e:
        print(f" FAIL: {e}")
        sys.exit(1)
    finally:
        cur.close()
        conn.close()

if __name__ == "__main__":
    test_array_types()
