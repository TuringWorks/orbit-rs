#!/usr/bin/env python3
import os
from neo4j import GraphDatabase
import pathlib

uri = os.environ.get("ORBIT_CYPHER_URI", "bolt://localhost:7687")
user = os.environ.get("ORBIT_CYPHER_USER", "orbit")
password = os.environ.get("ORBIT_CYPHER_PASSWORD", "orbit")

def run(session, text):
    stmts = [s.strip() for s in text.split(";") if s.strip()]
    for s in stmts:
        res = session.run(s)
        rows = [r.data() for r in res]
        if rows:
            print(rows)

def main():
    driver = GraphDatabase.driver(uri, auth=(user, password))
    base = pathlib.Path(__file__).parent.parent
    with driver.session() as session:
        run(session, pathlib.Path(base / "cypher" / "basic_graphrag.cypher").read_text())
        run(session, pathlib.Path(base / "cypher" / "advanced_graphrag.cypher").read_text())
        run(session, pathlib.Path(base / "cypher" / "similar_trends.cypher").read_text())
        run(session, pathlib.Path(base / "cypher" / "cross_kg_hybrid.cypher").read_text())
    driver.close()

if __name__ == "__main__":
    main()
