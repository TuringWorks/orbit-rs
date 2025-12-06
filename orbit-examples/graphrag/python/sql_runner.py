#!/usr/bin/env python3
import os
import psycopg2
from psycopg2.extras import RealDictCursor
import pathlib

host = os.environ.get("ORBIT_PG_HOST", "localhost")
port = int(os.environ.get("ORBIT_PG_PORT", "5432"))
db = os.environ.get("ORBIT_PG_DB", "orbit")
user = os.environ.get("ORBIT_PG_USER", "orbit")
password = os.environ.get("ORBIT_PG_PASSWORD", "orbit")

def run_file(cur, path):
    sql = pathlib.Path(path).read_text()
    stmts = [s.strip() for s in sql.split(";") if s.strip()]
    for s in stmts:
        cur.execute(s)
        try:
            rows = cur.fetchall()
            print({"statement": s[:80], "rows": rows})
        except psycopg2.ProgrammingError:
            pass

def main():
    conn = psycopg2.connect(host=host, port=port, database=db, user=user, password=password)
    base = pathlib.Path(__file__).parent.parent
    with conn, conn.cursor(cursor_factory=RealDictCursor) as cur:
        run_file(cur, str(base / "sql" / "basic_graphrag.sql"))
        run_file(cur, str(base / "sql" / "advanced_graphrag.sql"))
        run_file(cur, str(base / "sql" / "entities_similarity_semantic.sql"))
        run_file(cur, str(base / "sql" / "cross_kg_hybrid.sql"))
    conn.close()

if __name__ == "__main__":
    main()
