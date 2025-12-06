import os
import requests

URL = os.environ.get("ORBITQL_EXEC_URL", "http://localhost:8080/query/orbitql")
def main():
    p = os.path.join(os.path.dirname(__file__), "..", "orbitql", "graph_ml.orbitql")
    with open(p, "r") as f:
        q = f.read()
    r = requests.post(URL, json={"query": q})
    print(r.status_code)
    try:
        print(r.json())
    except Exception:
        print(r.text)

if __name__ == "__main__":
    main()
