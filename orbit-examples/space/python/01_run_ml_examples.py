import os
import psycopg2

CONN = {
    "host": os.environ.get("ORBIT_PG_HOST", "127.0.0.1"),
    "port": int(os.environ.get("ORBIT_PG_PORT", "5432")),
    "database": os.environ.get("ORBIT_PG_DB", "orbit"),
    "user": os.environ.get("ORBIT_PG_USER", "orbit"),
    "password": os.environ.get("ORBIT_PG_PASSWORD", "orbit"),
}

def run_file(conn, path):
    with open(path, "r", encoding="utf-8") as f:
        sql = f.read()
    with conn.cursor() as cur:
        for stmt in [s.strip() for s in sql.split(";") if s.strip()]:
            try:
                cur.execute(stmt)
                try:
                    rows = cur.fetchall()
                    if rows:
                        print(rows)
                except Exception:
                    pass
            except Exception as e:
                print("error:", e)
    conn.commit()

def main():
    conn = psycopg2.connect(**CONN)
    base = os.path.dirname(os.path.dirname(__file__))
    run_file(conn, os.path.join(base, "sql/01_schema_space.sql"))
    run_file(conn, os.path.join(base, "sql/02_ml_examples.sql"))
    conn.close()

if __name__ == "__main__":
    main()
