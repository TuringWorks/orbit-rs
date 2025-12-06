import os
import requests

BASE = os.environ.get("ORBIT_AQL_URL", "http://localhost:8529")

def create_collection(name, edge=False):
    t = 3 if edge else 2
    requests.post(f"{BASE}/_api/collection", json={"name": name, "type": t})

def run_aql(query):
    r = requests.post(f"{BASE}/_api/cursor", json={"query": query})
    return r.json()

def main():
    create_collection("entities")
    create_collection("relations")
    create_collection("edges", edge=True)
    run_aql("INSERT { name: 'Alice', type: 'Person', title: 'Engineer at Acme' } INTO entities")
    run_aql("INSERT { name: 'Bob', type: 'Person', title: 'Specialist at CityNet' } INTO entities")
    run_aql("INSERT { name: 'Acme Corp', type: 'Organization', summary: 'Technology company' } INTO entities")
    run_aql("INSERT { name: 'CityNet ISP', type: 'Organization', summary: 'Internet service provider' } INTO entities")
    run_aql("INSERT { name: 'Metropolis', type: 'Location', summary: 'Urban tech hub' } INTO entities")
    run_aql("FOR e IN entities FILTER e.embedding == null UPDATE e WITH { embedding: ML_EMBED_TEXT(coalesce(e.summary, e.title, e.name), 'sentence-transformers') } IN entities")
    alice = run_aql("RETURN FIRST(FOR e IN entities FILTER e.name == 'Alice' RETURN e)")["result"][0]
    bob = run_aql("RETURN FIRST(FOR e IN entities FILTER e.name == 'Bob' RETURN e)")["result"][0]
    acme = run_aql("RETURN FIRST(FOR e IN entities FILTER e.name == 'Acme Corp' RETURN e)")["result"][0]
    citynet = run_aql("RETURN FIRST(FOR e IN entities FILTER e.name == 'CityNet ISP' RETURN e)")["result"][0]
    metro = run_aql("RETURN FIRST(FOR e IN entities FILTER e.name == 'Metropolis' RETURN e)")["result"][0]
    run_aql(f"INSERT {{ _from: '{alice['_id']}', _to: '{acme['_id']}', name: 'works_at' }} INTO edges")
    run_aql(f"INSERT {{ _from: '{bob['_id']}', _to: '{citynet['_id']}', name: 'works_at' }} INTO edges")
    run_aql(f"INSERT {{ _from: '{acme['_id']}', _to: '{metro['_id']}', name: 'located_in' }} INTO edges")
    run_aql(f"INSERT {{ _from: '{citynet['_id']}', _to: '{metro['_id']}', name: 'located_in' }} INTO edges")
    run_aql(f"INSERT {{ _from: '{citynet['_id']}', _to: '{acme['_id']}', name: 'partner_of' }} INTO edges")
    q = """
FOR h IN entities FILTER h.name == 'Alice'
  FOR t IN entities FILTER t.type == 'Organization'
    FILTER LENGTH(FOR v,e IN 1..1 OUTBOUND h edges FILTER e.name == 'works_at' RETURN v) == 0
    LET deg_h = LENGTH(FOR v,e IN 1..1 ANY h edges RETURN e)
    LET deg_t = LENGTH(FOR v,e IN 1..1 ANY t edges RETURN e)
    LET sim = 1 - (t.embedding <=> ML_EMBED_TEXT(h.name + ' works_at', 'sentence-transformers'))
    RETURN { candidate: t.name, link_probability: ML_PREDICT('kg_link_predictor', [sim, deg_h, deg_t]) }
"""
    r = run_aql(q)
    for item in r.get("result", []):
        print(item)
    q2 = """
FOR e IN entities
  LET prob = ML_PREDICT('kg_node_classifier', [LENGTH(FOR v,e2 IN 1..1 ANY e edges RETURN e2), 1 - (e.embedding <=> ML_EMBED_TEXT('organization', 'sentence-transformers'))])
  RETURN { name: e.name, organization_probability: prob }
"""
    r2 = run_aql(q2)
    for item in r2.get("result", []):
        print(item)
    ts_now = run_aql("RETURN DATE_NOW()")['result'][0]
    run_aql(f"INSERT {{ _from: '{alice['_id']}', _to: '{citynet['_id']}', name: 'interacts_with', ts: {ts_now - 3600000} }} INTO edges")
    run_aql(f"INSERT {{ _from: '{alice['_id']}', _to: '{acme['_id']}', name: 'interacts_with', ts: {ts_now - 7200000} }} INTO edges")
    run_aql(f"INSERT {{ _from: '{bob['_id']}', _to: '{acme['_id']}', name: 'interacts_with', ts: {ts_now - 180000} }} INTO edges")
    train = """
FOR h IN entities FILTER h.name == 'Alice'
  FOR t IN entities FILTER t.type == 'Organization'
    LET recent_h = LENGTH(FOR v,e IN 1..1 OUTBOUND h edges FILTER e.name == 'interacts_with' AND e.ts > DATE_NOW() - 86400000 RETURN e)
    LET recent_t = LENGTH(FOR v,e IN 1..1 ANY t edges FILTER e.name == 'interacts_with' AND e.ts > DATE_NOW() - 86400000 RETURN e)
    LET sim_ts = 1 - (t.embedding <=> ML_EMBED_TEXT(h.name + ' interacts_with', 'sentence-transformers'))
    LET has_works = LENGTH(FOR v,e IN 1..1 OUTBOUND h edges FILTER e.name == 'works_at' AND v._id == t._id RETURN v) > 0
    LET label = TO_NUMBER(has_works)
    RETURN ML_TRAIN_MODEL('kg_temporal_link_predictor', 'gradient_boosting', [sim_ts, recent_h, recent_t], label)
"""
    run_aql(train)
    q3 = """
FOR h IN entities FILTER h.name == 'Alice'
  FOR t IN entities FILTER t.type == 'Organization'
    FILTER LENGTH(FOR v,e IN 1..1 OUTBOUND h edges FILTER e.name == 'works_at' RETURN v) == 0
    LET recent_h = LENGTH(FOR v,e IN 1..1 OUTBOUND h edges FILTER e.name == 'interacts_with' AND e.ts > DATE_NOW() - 86400000 RETURN e)
    LET recent_t = LENGTH(FOR v,e IN 1..1 ANY t edges FILTER e.name == 'interacts_with' AND e.ts > DATE_NOW() - 86400000 RETURN e)
    LET sim_ts = 1 - (t.embedding <=> ML_EMBED_TEXT(h.name + ' interacts_with', 'sentence-transformers'))
    RETURN { candidate: t.name, temporal_link_probability: ML_PREDICT('kg_temporal_link_predictor', [sim_ts, recent_h, recent_t]) }
"""
    r3 = run_aql(q3)
    for item in r3.get("result", []):
        print(item)

if __name__ == "__main__":
    main()
