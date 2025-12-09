// Pipeline Network Graph (Cypher)
// Managing the topology of pipelines, valves, and stations to route oil/gas.

// 1. Create Nodes
CREATE (s1:Station {name: 'Extraction Point Alpha', type: 'Source'})
CREATE (s2:Station {name: 'Refinery Beta', type: 'Sink'})
CREATE (v1:Valve {id: 'V-101', status: 'OPEN'})
CREATE (v2:Valve {id: 'V-102', status: 'CLOSED'})
CREATE (d1:DistributionHub {name: 'Central Hub'});

// 2. Create Paths (Pipelines)
// Alpha -> Valve 1 -> Hub -> Valve 2 -> Beta
MATCH (a:Station {name: 'Extraction Point Alpha'}), (v:Valve {id: 'V-101'})
CREATE (a)-[:PIPELINE {capacity_bpd: 50000, length_km: 10}]->(v);

MATCH (v:Valve {id: 'V-101'}), (h:DistributionHub {name: 'Central Hub'})
CREATE (v)-[:PIPELINE {capacity_bpd: 50000, length_km: 50}]->(h);

MATCH (h:DistributionHub {name: 'Central Hub'}), (v:Valve {id: 'V-102'})
CREATE (h)-[:PIPELINE {capacity_bpd: 25000, length_km: 20}]->(v);

MATCH (v:Valve {id: 'V-102'}), (s:Station {name: 'Refinery Beta'})
CREATE (v)-[:PIPELINE {capacity_bpd: 25000, length_km: 5}]->(s);

// 3. Query: Find Available Flow Path
// Find path from Source to Sink where all valves are OPEN
MATCH p = (start:Station {type: 'Source'})-[:PIPELINE|FLOWS_TO*]->(end:Station {type: 'Sink'})
WHERE ALL(n IN nodes(p) WHERE (n:Valve IMPLIES n.status = 'OPEN'))
RETURN p;
