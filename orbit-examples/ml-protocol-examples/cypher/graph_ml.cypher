CREATE (:Person {name:'Alice', title:'Engineer at Acme'})
CREATE (:Person {name:'Bob', title:'Specialist at CityNet'})
CREATE (:Organization {name:'Acme Corp', summary:'Technology company'})
CREATE (:Organization {name:'CityNet ISP', summary:'Internet service provider'})
CREATE (:Location {name:'Metropolis', summary:'Urban tech hub'})
MATCH (a:Person {name:'Alice'}),(ac:Organization {name:'Acme Corp'}) CREATE (a)-[:WORKS_AT]->(ac)
MATCH (b:Person {name:'Bob'}),(ci:Organization {name:'CityNet ISP'}) CREATE (b)-[:WORKS_AT]->(ci)
MATCH (ac:Organization {name:'Acme Corp'}),(m:Location {name:'Metropolis'}) CREATE (ac)-[:LOCATED_IN]->(m)
MATCH (ci:Organization {name:'CityNet ISP'}),(m:Location {name:'Metropolis'}) CREATE (ci)-[:LOCATED_IN]->(m)
MATCH (ci:Organization {name:'CityNet ISP'}),(ac:Organization {name:'Acme Corp'}) CREATE (ci)-[:PARTNER_OF]->(ac)
MATCH (n) SET n.embedding = ML_EMBED_TEXT(coalesce(n.summary,n.title,n.name),'sentence-transformers')
MATCH (h:Person {name:'Alice'}), (t:Organization)
WHERE NOT (h)-[:WORKS_AT]->(t)
WITH h, t, size((h)--()) AS deg_h, size((t)--()) AS deg_t,
     1 - (t.embedding <=> ML_EMBED_TEXT(h.name + ' works_at','sentence-transformers')) AS sim
RETURN t.name AS candidate, ML_PREDICT('kg_link_predictor', [sim, deg_h, deg_t]) AS link_probability
ORDER BY link_probability DESC LIMIT 5
MATCH (e:Entity)
RETURN e.name AS name,
       ML_PREDICT('kg_node_classifier', [size((e)--()), 1 - (e.embedding <=> ML_EMBED_TEXT('organization','sentence-transformers'))]) AS organization_probability
ORDER BY organization_probability DESC
MATCH (eh:Person {name:'Alice'}),(et:Organization {name:'Acme Corp'}),(r)
WHERE EXISTS { MATCH ()-[rel]-() WHERE type(rel)=r.name }
RETURN r.name AS relation_candidate,
       1 - (r.embedding <=> ML_EMBED_TEXT(eh.name + ' ' + et.name,'sentence-transformers')) AS score
ORDER BY score DESC LIMIT 3
MATCH (a:Person {name:'Alice'}),(ci:Organization {name:'CityNet ISP'}) CREATE (a)-[:INTERACTS_WITH {ts: timestamp()-3600000}]->(ci)
MATCH (a:Person {name:'Alice'}),(ac:Organization {name:'Acme Corp'}) CREATE (a)-[:INTERACTS_WITH {ts: timestamp()-7200000}]->(ac)
MATCH (b:Person {name:'Bob'}),(ac:Organization {name:'Acme Corp'}) CREATE (b)-[:INTERACTS_WITH {ts: timestamp()-180000}]->(ac)
MATCH (h:Person {name:'Alice'}), (t:Organization)
WHERE NOT (h)-[:WORKS_AT]->(t)
WITH h, t
MATCH (h)-[eh:INTERACTS_WITH]->()
WHERE eh.ts > timestamp() - 86400000
WITH h, t, count(eh) AS recent_h
MATCH (t)-[et:INTERACTS_WITH]-()
WHERE et.ts > timestamp() - 86400000
WITH h, t, recent_h, count(et) AS recent_t,
     1 - (t.embedding <=> ML_EMBED_TEXT(h.name + ' interacts_with','sentence-transformers')) AS sim_ts
RETURN t.name AS candidate,
       ML_PREDICT('kg_temporal_link_predictor', [sim_ts, recent_h, recent_t]) AS link_probability
ORDER BY link_probability DESC LIMIT 5
MATCH (h:Person {name:'Alice'}), (t:Organization)
WITH h, t
MATCH (h)-[eh:INTERACTS_WITH]->()
WHERE eh.ts > timestamp() - 86400000
WITH h, t, count(eh) AS recent_h
MATCH (t)-[et:INTERACTS_WITH]-()
WHERE et.ts > timestamp() - 86400000
WITH h, t, recent_h, count(et) AS recent_t,
     1 - (t.embedding <=> ML_EMBED_TEXT(h.name + ' interacts_with','sentence-transformers')) AS sim_ts,
     CASE WHEN (h)-[:WORKS_AT]->(t) THEN 1 ELSE 0 END AS label
RETURN ML_TRAIN_MODEL('kg_temporal_link_predictor','gradient_boosting',[sim_ts,recent_h,recent_t],label)
