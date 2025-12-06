import os
from neo4j import GraphDatabase

URI = os.environ.get("ORBIT_CYPHER_URI", "bolt://localhost:7687")
USER = os.environ.get("ORBIT_CYPHER_USER", "orbit")
PASSWORD = os.environ.get("ORBIT_CYPHER_PASSWORD", "orbit")

def run(session, q, params=None):
    return session.run(q, params or {})

def main():
    driver = GraphDatabase.driver(URI, auth=(USER, PASSWORD))
    with driver.session() as session:
        run(session, "CREATE (:Person {name:'Alice', title:'Engineer at Acme'})")
        run(session, "CREATE (:Person {name:'Bob', title:'Specialist at CityNet'})")
        run(session, "CREATE (:Organization {name:'Acme Corp', summary:'Technology company'})")
        run(session, "CREATE (:Organization {name:'CityNet ISP', summary:'Internet service provider'})")
        run(session, "CREATE (:Location {name:'Metropolis', summary:'Urban tech hub'})")
        run(session, "MATCH (a:Person {name:'Alice'}),(ac:Organization {name:'Acme Corp'}) CREATE (a)-[:WORKS_AT]->(ac)")
        run(session, "MATCH (b:Person {name:'Bob'}),(ci:Organization {name:'CityNet ISP'}) CREATE (b)-[:WORKS_AT]->(ci)")
        run(session, "MATCH (ac:Organization {name:'Acme Corp'}),(m:Location {name:'Metropolis'}) CREATE (ac)-[:LOCATED_IN]->(m)")
        run(session, "MATCH (ci:Organization {name:'CityNet ISP'}),(m:Location {name:'Metropolis'}) CREATE (ci)-[:LOCATED_IN]->(m)")
        run(session, "MATCH (ci:Organization {name:'CityNet ISP'}),(ac:Organization {name:'Acme Corp'}) CREATE (ci)-[:PARTNER_OF]->(ac)")
        run(session, "MATCH (n) SET n.embedding = ML_EMBED_TEXT(coalesce(n.summary,n.title,n.name),'sentence-transformers')")
        res = run(session, (
            "MATCH (h:Person {name:'Alice'}), (t:Organization) "
            "WHERE NOT (h)-[:WORKS_AT]->(t) "
            "WITH h, t, size((h)--()) AS deg_h, size((t)--()) AS deg_t, "
            "1 - (t.embedding <=> ML_EMBED_TEXT(h.name + ' works_at','sentence-transformers')) AS sim "
            "RETURN t.name AS candidate, ML_PREDICT('kg_link_predictor', [sim, deg_h, deg_t]) AS link_probability "
            "ORDER BY link_probability DESC LIMIT 5"
        ))
        for r in res:
            print({"candidate": r["candidate"], "link_probability": r["link_probability"]})
        res2 = run(session, (
            "MATCH (e) "
            "RETURN e.name AS name, "
            "ML_PREDICT('kg_node_classifier', [size((e)--()), 1 - (e.embedding <=> ML_EMBED_TEXT('organization','sentence-transformers'))]) AS organization_probability "
            "ORDER BY organization_probability DESC"
        ))
        for r in res2:
            print({"name": r["name"], "organization_probability": r["organization_probability"]})
        run(session, "MATCH (a:Person {name:'Alice'}),(ci:Organization {name:'CityNet ISP'}) CREATE (a)-[:INTERACTS_WITH {ts: timestamp()-3600000}]->(ci)")
        run(session, "MATCH (a:Person {name:'Alice'}),(ac:Organization {name:'Acme Corp'}) CREATE (a)-[:INTERACTS_WITH {ts: timestamp()-7200000}]->(ac)")
        run(session, "MATCH (b:Person {name:'Bob'}),(ac:Organization {name:'Acme Corp'}) CREATE (b)-[:INTERACTS_WITH {ts: timestamp()-180000}]->(ac)")
        train = (
            "MATCH (h:Person {name:'Alice'}), (t:Organization) "
            "WITH h, t "
            "MATCH (h)-[eh:INTERACTS_WITH]->() "
            "WHERE eh.ts > timestamp() - 86400000 "
            "WITH h, t, count(eh) AS recent_h "
            "MATCH (t)-[et:INTERACTS_WITH]-() "
            "WHERE et.ts > timestamp() - 86400000 "
            "WITH h, t, recent_h, count(et) AS recent_t, "
            "1 - (t.embedding <=> ML_EMBED_TEXT(h.name + ' interacts_with','sentence-transformers')) AS sim_ts, "
            "CASE WHEN (h)-[:WORKS_AT]->(t) THEN 1 ELSE 0 END AS label "
            "RETURN ML_TRAIN_MODEL('kg_temporal_link_predictor','gradient_boosting',[sim_ts,recent_h,recent_t],label)"
        )
        run(session, train)
        res3 = run(session, (
            "MATCH (h:Person {name:'Alice'}), (t:Organization) "
            "WHERE NOT (h)-[:WORKS_AT]->(t) "
            "WITH h, t "
            "MATCH (h)-[eh:INTERACTS_WITH]->() "
            "WHERE eh.ts > timestamp() - 86400000 "
            "WITH h, t, count(eh) AS recent_h "
            "MATCH (t)-[et:INTERACTS_WITH]-() "
            "WHERE et.ts > timestamp() - 86400000 "
            "WITH h, t, recent_h, count(et) AS recent_t, "
            "1 - (t.embedding <=> ML_EMBED_TEXT(h.name + ' interacts_with','sentence-transformers')) AS sim_ts "
            "RETURN t.name AS candidate, ML_PREDICT('kg_temporal_link_predictor', [sim_ts, recent_h, recent_t]) AS link_probability "
            "ORDER BY link_probability DESC LIMIT 5"
        ))
        for r in res3:
            print({"candidate": r["candidate"], "temporal_link_probability": r["link_probability"]})
    driver.close()

if __name__ == "__main__":
    main()
