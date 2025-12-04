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

def test_create_function():
    conn = get_connection()
    cur = conn.cursor()
    
    print("Testing CREATE FUNCTION syntax...")
    
    try:
        # Test 1: Simple SQL function
        print("  1. Simple SQL function...", end="", flush=True)
        cur.execute("""
            CREATE OR REPLACE FUNCTION add(a integer, b integer) 
            RETURNS integer 
            LANGUAGE sql 
            AS $$ SELECT a + b; $$;
        """)
        print(" PASS")
        
        # Test 2: PL/pgSQL function (parsing only, execution not supported yet)
        print("  2. PL/pgSQL function...", end="", flush=True)
        cur.execute("""
            CREATE FUNCTION increment(i integer) 
            RETURNS integer AS $$
            BEGIN
                RETURN i + 1;
            END;
            $$ LANGUAGE plpgsql;
        """)
        print(" PASS")
        
        # Test 3: Function with OUT parameters
        print("  3. Function with OUT parameters...", end="", flush=True)
        cur.execute("""
            CREATE FUNCTION dup(in int, out f1 int, out f2 text)
            AS $$ SELECT $1, CAST($1 AS text) || ' is text' $$
            LANGUAGE SQL;
        """)
        print(" PASS")
        
        # Test 4: Function with volatility
        print("  4. Function with volatility...", end="", flush=True)
        cur.execute("""
            CREATE FUNCTION random_val() RETURNS float8
            LANGUAGE sql
            VOLATILE
            AS 'SELECT random()';
        """)
        print(" PASS")

    except Exception as e:
        print(f" FAIL: {e}")
        sys.exit(1)
    finally:
        cur.close()
        conn.close()

if __name__ == "__main__":
    test_create_function()
