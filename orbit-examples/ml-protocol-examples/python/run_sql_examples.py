#!/usr/bin/env python3
"""
Run Orbit-RS SQL example files via PostgreSQL protocol.

Requirements:
- pip install psycopg2-binary

Usage:
- python run_sql_examples.py sql/ml_functions_examples.sql
- python run_sql_examples.py sql/ml_vectors.sql
"""

import sys
import os
import psycopg2


CONN_PARAMS = {
    "host": os.environ.get("ORBIT_PG_HOST", "localhost"),
    "port": int(os.environ.get("ORBIT_PG_PORT", "5432")),
    "database": os.environ.get("ORBIT_PG_DB", "orbit"),
    "user": os.environ.get("ORBIT_PG_USER", "orbit"),
    "password": os.environ.get("ORBIT_PG_PASSWORD", "orbit"),
}


def split_sql_statements(sql_text: str):
    """Split SQL text into statements, respecting quotes and dollar-quoting.

    Handles:
    - Single quotes '...'
    - Dollar-quoted strings $$...$$ and $tag$...$tag$
    - Semicolons ending statements only when not inside a quoted region
    """
    statements = []
    buf = []
    i = 0
    n = len(sql_text)
    in_single = False
    in_dollar = False
    dollar_tag = None

    while i < n:
        ch = sql_text[i]
        nxt = sql_text[i + 1] if i + 1 < n else ""

        # Detect start/end of dollar-quoted strings: $tag$ or $$
        if not in_single:
            if not in_dollar and ch == "$":
                # read tag until next '$'
                j = i + 1
                while j < n and sql_text[j] != "$" and sql_text[j].isalnum() or sql_text[j] == "_":
                    j += 1
                if j < n and sql_text[j] == "$":
                    # Found $tag$
                    in_dollar = True
                    dollar_tag = sql_text[i:j + 1]  # includes trailing $
                    buf.append(sql_text[i:j + 1])
                    i = j + 1
                    continue
            elif in_dollar and ch == "$":
                # possible end tag
                tag_len = len(dollar_tag)
                if sql_text[i:i + tag_len] == dollar_tag:
                    buf.append(sql_text[i:i + tag_len])
                    i += tag_len
                    in_dollar = False
                    dollar_tag = None
                    continue

        # Handle single quotes (escape by doubling '')
        if not in_dollar:
            if ch == "'":
                if in_single:
                    # closing quote unless escaped ''
                    if nxt == "'":
                        buf.append("''")
                        i += 2
                        continue
                    in_single = False
                else:
                    in_single = True

        if ch == ";" and not in_single and not in_dollar:
            # End of statement
            stmt = "".join(buf).strip()
            if stmt:
                statements.append(stmt)
            buf = []
            i += 1
            continue

        buf.append(ch)
        i += 1

    # tail
    tail = "".join(buf).strip()
    if tail:
        statements.append(tail)
    return statements


def run_sql_file(conn, path: str):
    with open(path, "r", encoding="utf-8") as f:
        sql = f.read()
    stmts = split_sql_statements(sql)
    print(f"Executing {len(stmts)} statements from {path}")
    with conn.cursor() as cur:
        for idx, stmt in enumerate(stmts, 1):
            try:
                cur.execute(stmt)
                # Try to fetch rows for SELECT
                try:
                    rows = cur.fetchall()
                    print(f"[{idx}] Rows: {len(rows)}")
                except psycopg2.ProgrammingError:
                    pass
            except Exception as e:
                print(f"Error executing statement {idx}: {e}")
                # Continue to next statement
        conn.commit()


def main():
    if len(sys.argv) < 2:
        print("Usage: python run_sql_examples.py <path/to/sql_file.sql> [more.sql ...]")
        print("Example: python run_sql_examples.py sql/ml_functions_examples.sql sql/ml_vectors.sql")
        sys.exit(1)

    files = sys.argv[1:]
    # Normalize to repo root relative paths
    base_dir = os.path.dirname(os.path.dirname(__file__))
    base_dir = os.path.join(base_dir)  # orbit-examples/ml-protocol-examples

    # Resolve file paths
    resolved = []
    for p in files:
        rp = p
        if not os.path.isabs(rp):
            rp = os.path.join(base_dir, p)
        if not os.path.exists(rp):
            print(f"File not found: {rp}")
            sys.exit(2)
        resolved.append(rp)

    try:
        conn = psycopg2.connect(**CONN_PARAMS)
        print(f"Connected to Orbit PostgreSQL at {CONN_PARAMS['host']}:{CONN_PARAMS['port']}")
        for path in resolved:
            print("=" * 60)
            run_sql_file(conn, path)
        print("=" * 60)
        print("All SQL files executed.")
    except psycopg2.OperationalError as e:
        print(f"Connection Error: {e}")
        print("Ensure Orbit server is running on port 5432 and credentials are correct.")
        sys.exit(3)
    finally:
        try:
            conn.close()
        except Exception:
            pass


if __name__ == "__main__":
    main()

