// Cypher: Network Topology & Connectivity
// Models the physical cabling and logical routing of the data center network.

// 1. Create Nodes (Devices, Satellites)
CREATE (r1:Router {hostname: 'core-router-01', ip: '10.0.0.1'})
CREATE (s1:Switch {hostname: 'tor-switch-01', ip: '10.0.1.1', rack: 'rack-055'})
CREATE (srv1:Server {hostname: 'compute-node-01', ip: '10.0.1.10', rack: 'rack-055'})

CREATE (sat1:Satellite {id: 'SAT-101', orbit: 'LEO'})
CREATE (sat2:Satellite {id: 'SAT-102', orbit: 'LEO'})
CREATE (gs1:GroundStation {id: 'GS-London'})

// 2. Create Relationships (CONNECTED_TO, UPLINK)
// Terrestrial Cabling: Server -> Switch -> Router
CREATE (srv1)-[:CONNECTED_TO {port: 'eth0', type: 'fiber_10g'}]->(s1)
CREATE (s1)-[:CONNECTED_TO {port: 'uplink1', type: 'fiber_100g'}]->(r1)

// Orbital Mesh: Satellite -> Satellite (ISL) -> Ground Station
CREATE (sat1)-[:IS_LINKED {type: 'laser', latency_ms: 5}]->(sat2)
CREATE (sat1)-[:UPLINK {band: 'Ka', status: 'active'}]->(gs1)

// 3. Routing Query: Server to Internet
// Find the shortest physical path from a specific server to the core router.
MATCH path = shortestPath((s:Server {hostname: 'compute-node-01'})-[*]->(r:Router {hostname: 'core-router-01'}))
RETURN path

// 4. Orbital Routing: Ground to Ground via Space
// Route packet from London Ground Station to a target Satellite via the mesh.
MATCH path = shortestPath((gs:GroundStation {id: 'GS-London'})-[*]->(target:Satellite {id: 'SAT-102'}))
RETURN path

// 5. Impact Analysis: Switch Failure
// Find all servers affected if 'tor-switch-01' goes down.
MATCH (s:Switch {hostname: 'tor-switch-01'})<-[:CONNECTED_TO]-(affected_server:Server)
RETURN affected_server.hostname, affected_server.ip
