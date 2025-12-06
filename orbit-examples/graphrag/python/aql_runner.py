#!/usr/bin/env python3
import os
import requests
import pathlib

base_url = os.environ.get("ORBIT_AQL_URL", "http://localhost:8529")

def run(text):
    r = requests.post(f"{base_url}/_api/cursor", json={"query": text})
    print(r.status_code)
    try:
        print(r.json())
    except Exception:
        print(r.text)

def main():
    base = pathlib.Path(__file__).parent.parent
    run(pathlib.Path(base / "aql" / "basic_graphrag.aql").read_text())
    run(pathlib.Path(base / "aql" / "advanced_graphrag.aql").read_text())
    run(pathlib.Path(base / "aql" / "similar.aql").read_text())
    run(pathlib.Path(base / "aql" / "cross_kg_hybrid.aql").read_text())

if __name__ == "__main__":
    main()
