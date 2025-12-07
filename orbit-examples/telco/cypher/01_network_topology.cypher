// =============================================================================
// OrbitRS Telco Example: Network Topology with Cypher
// =============================================================================
// Demonstrates graph-based network topology modeling for telecom
// using Cypher query language with OrbitRS Neo4j-compatible protocol.

// =============================================================================
// SCHEMA AND CONSTRAINTS
// =============================================================================

// Create constraints for unique identifiers
CREATE CONSTRAINT tower_id_unique IF NOT EXISTS
FOR (t:Tower) REQUIRE t.tower_id IS UNIQUE;

CREATE CONSTRAINT node_id_unique IF NOT EXISTS
FOR (n:NetworkNode) REQUIRE n.node_id IS UNIQUE;

CREATE CONSTRAINT subscriber_id_unique IF NOT EXISTS
FOR (s:Subscriber) REQUIRE s.subscriber_id IS UNIQUE;

CREATE CONSTRAINT device_imei_unique IF NOT EXISTS
FOR (d:Device) REQUIRE d.imei IS UNIQUE;

// Create indexes for performance
CREATE INDEX tower_location IF NOT EXISTS FOR (t:Tower) ON (t.latitude, t.longitude);
CREATE INDEX subscriber_plan IF NOT EXISTS FOR (s:Subscriber) ON (s.plan_type);
CREATE INDEX node_status IF NOT EXISTS FOR (n:NetworkNode) ON (n.status);

// =============================================================================
// NETWORK INFRASTRUCTURE NODES
// =============================================================================

// Create cell towers
CREATE (t1:Tower:Infrastructure {
    tower_id: 'TOWER-NYC-001',
    name: 'Manhattan Downtown Tower',
    tower_type: '5G-MACRO',
    latitude: 40.7128,
    longitude: -74.0060,
    height_meters: 45.0,
    coverage_radius_km: 2.5,
    status: 'ACTIVE',
    max_capacity: 5000,
    current_load: 1250,
    frequency_bands: ['n78', 'n257', 'n258'],
    installed_date: date('2022-03-15'),
    last_maintenance: date('2024-01-10')
});

CREATE (t2:Tower:Infrastructure {
    tower_id: 'TOWER-NYC-002',
    name: 'Midtown Tower',
    tower_type: '5G-MACRO',
    latitude: 40.7549,
    longitude: -73.9840,
    height_meters: 52.0,
    coverage_radius_km: 3.0,
    status: 'ACTIVE',
    max_capacity: 6000,
    current_load: 2100,
    frequency_bands: ['n78', 'n257'],
    installed_date: date('2021-11-20'),
    last_maintenance: date('2024-01-05')
});

CREATE (t3:Tower:Infrastructure {
    tower_id: 'TOWER-NYC-003',
    name: 'Brooklyn Heights Tower',
    tower_type: '4G-LTE',
    latitude: 40.6892,
    longitude: -73.9942,
    height_meters: 38.0,
    coverage_radius_km: 4.0,
    status: 'ACTIVE',
    max_capacity: 4000,
    current_load: 1800,
    frequency_bands: ['B2', 'B4', 'B66'],
    installed_date: date('2019-06-10'),
    last_maintenance: date('2023-12-20')
});

// Create core network nodes
CREATE (core1:NetworkNode:CoreNetwork {
    node_id: 'CORE-NYC-01',
    name: 'NYC Primary Core',
    node_type: 'MME',
    ip_address: '10.1.1.1',
    status: 'ACTIVE',
    datacenter: 'NYC-DC-1',
    throughput_gbps: 100,
    connections: 45000,
    uptime_percent: 99.99
});

CREATE (core2:NetworkNode:CoreNetwork {
    node_id: 'CORE-NYC-02',
    name: 'NYC Secondary Core',
    node_type: 'SGW',
    ip_address: '10.1.1.2',
    status: 'ACTIVE',
    datacenter: 'NYC-DC-1',
    throughput_gbps: 80,
    connections: 38000,
    uptime_percent: 99.98
});

CREATE (core3:NetworkNode:CoreNetwork {
    node_id: 'CORE-NYC-03',
    name: 'NYC PDN Gateway',
    node_type: 'PGW',
    ip_address: '10.1.1.3',
    status: 'ACTIVE',
    datacenter: 'NYC-DC-2',
    throughput_gbps: 120,
    connections: 52000,
    uptime_percent: 99.97
});

// Create edge nodes
CREATE (edge1:NetworkNode:EdgeNode {
    node_id: 'EDGE-NYC-01',
    name: 'Manhattan Edge',
    node_type: 'MEC',
    ip_address: '10.2.1.1',
    status: 'ACTIVE',
    latency_ms: 5.2,
    compute_units: 256,
    storage_tb: 50
});

CREATE (edge2:NetworkNode:EdgeNode {
    node_id: 'EDGE-NYC-02',
    name: 'Brooklyn Edge',
    node_type: 'MEC',
    ip_address: '10.2.1.2',
    status: 'ACTIVE',
    latency_ms: 6.8,
    compute_units: 128,
    storage_tb: 25
});

// =============================================================================
// SUBSCRIBERS AND DEVICES
// =============================================================================

// Create subscribers
CREATE (s1:Subscriber:Customer {
    subscriber_id: 'SUB-001',
    name: 'John Smith',
    phone_number: '+1-555-0101',
    plan_type: 'UNLIMITED_5G',
    account_status: 'ACTIVE',
    signup_date: date('2023-01-15'),
    monthly_spend: 89.99,
    data_usage_gb: 45.2,
    loyalty_tier: 'GOLD'
});

CREATE (s2:Subscriber:Customer {
    subscriber_id: 'SUB-002',
    name: 'Jane Doe',
    phone_number: '+1-555-0102',
    plan_type: 'FAMILY_SHARE',
    account_status: 'ACTIVE',
    signup_date: date('2022-06-20'),
    monthly_spend: 149.99,
    data_usage_gb: 120.5,
    loyalty_tier: 'PLATINUM'
});

CREATE (s3:Subscriber:Customer {
    subscriber_id: 'SUB-003',
    name: 'Bob Johnson',
    phone_number: '+1-555-0103',
    plan_type: 'BASIC_4G',
    account_status: 'ACTIVE',
    signup_date: date('2023-08-10'),
    monthly_spend: 45.00,
    data_usage_gb: 8.5,
    loyalty_tier: 'STANDARD'
});

// Create devices
CREATE (d1:Device {
    imei: '353456789012345',
    device_type: 'SMARTPHONE',
    manufacturer: 'Apple',
    model: 'iPhone 15 Pro',
    os_version: 'iOS 17.2',
    supports_5g: true,
    esim_capable: true
});

CREATE (d2:Device {
    imei: '353456789012346',
    device_type: 'SMARTPHONE',
    manufacturer: 'Samsung',
    model: 'Galaxy S24 Ultra',
    os_version: 'Android 14',
    supports_5g: true,
    esim_capable: true
});

CREATE (d3:Device {
    imei: '353456789012347',
    device_type: 'TABLET',
    manufacturer: 'Apple',
    model: 'iPad Pro',
    os_version: 'iPadOS 17.2',
    supports_5g: true,
    esim_capable: true
});

// =============================================================================
// RELATIONSHIPS
// =============================================================================

// Network topology relationships
MATCH (t1:Tower {tower_id: 'TOWER-NYC-001'}),
      (t2:Tower {tower_id: 'TOWER-NYC-002'}),
      (t3:Tower {tower_id: 'TOWER-NYC-003'})
CREATE (t1)-[:CONNECTS_TO {fiber_capacity_gbps: 10, distance_km: 3.2}]->(t2)
CREATE (t2)-[:CONNECTS_TO {fiber_capacity_gbps: 10, distance_km: 3.2}]->(t1)
CREATE (t2)-[:CONNECTS_TO {fiber_capacity_gbps: 10, distance_km: 4.5}]->(t3)
CREATE (t3)-[:CONNECTS_TO {fiber_capacity_gbps: 10, distance_km: 4.5}]->(t2);

// Tower to core network
MATCH (t:Tower), (core:NetworkNode:CoreNetwork {node_type: 'MME'})
CREATE (t)-[:ROUTES_THROUGH {latency_ms: 2.5}]->(core);

// Core network internal
MATCH (mme:NetworkNode {node_type: 'MME'}),
      (sgw:NetworkNode {node_type: 'SGW'}),
      (pgw:NetworkNode {node_type: 'PGW'})
CREATE (mme)-[:FORWARDS_TO {protocol: 'GTP'}]->(sgw)
CREATE (sgw)-[:FORWARDS_TO {protocol: 'GTP'}]->(pgw);

// Tower to edge nodes
MATCH (t1:Tower {tower_id: 'TOWER-NYC-001'}), (e1:EdgeNode {node_id: 'EDGE-NYC-01'})
CREATE (t1)-[:USES_EDGE {latency_ms: 1.2}]->(e1);

MATCH (t3:Tower {tower_id: 'TOWER-NYC-003'}), (e2:EdgeNode {node_id: 'EDGE-NYC-02'})
CREATE (t3)-[:USES_EDGE {latency_ms: 1.5}]->(e2);

// Subscriber relationships
MATCH (s1:Subscriber {subscriber_id: 'SUB-001'}),
      (d1:Device {imei: '353456789012345'}),
      (t1:Tower {tower_id: 'TOWER-NYC-001'})
CREATE (s1)-[:OWNS {since: date('2023-01-15')}]->(d1)
CREATE (d1)-[:CONNECTED_TO {signal_strength: -72, connection_type: '5G_NR'}]->(t1);

MATCH (s2:Subscriber {subscriber_id: 'SUB-002'}),
      (d2:Device {imei: '353456789012346'}),
      (t2:Tower {tower_id: 'TOWER-NYC-002'})
CREATE (s2)-[:OWNS {since: date('2022-06-20')}]->(d2)
CREATE (d2)-[:CONNECTED_TO {signal_strength: -68, connection_type: '5G_NR'}]->(t2);

MATCH (s3:Subscriber {subscriber_id: 'SUB-003'}),
      (d3:Device {imei: '353456789012347'}),
      (t3:Tower {tower_id: 'TOWER-NYC-003'})
CREATE (s3)-[:OWNS {since: date('2023-08-10')}]->(d3)
CREATE (d3)-[:CONNECTED_TO {signal_strength: -78, connection_type: 'LTE'}]->(t3);

// =============================================================================
// GRAPH ANALYTICS QUERIES
// =============================================================================

// Query 1: Find all towers within N hops of a given tower
MATCH path = (start:Tower {tower_id: 'TOWER-NYC-001'})-[:CONNECTS_TO*1..3]-(connected:Tower)
RETURN start.tower_id AS source,
       connected.tower_id AS connected_tower,
       connected.status AS status,
       length(path) AS hops;

// Query 2: Network path from device to internet (PDN Gateway)
MATCH path = (d:Device {imei: '353456789012345'})-[:CONNECTED_TO]->(t:Tower)
             -[:ROUTES_THROUGH]->(mme:NetworkNode)
             -[:FORWARDS_TO*]->(pgw:NetworkNode {node_type: 'PGW'})
RETURN d.model AS device,
       t.tower_id AS tower,
       [n IN nodes(path) | labels(n)[0] + ': ' + coalesce(n.node_id, n.tower_id, n.imei)] AS path_nodes;

// Query 3: Find overloaded towers (>70% capacity)
MATCH (t:Tower)
WHERE toFloat(t.current_load) / t.max_capacity > 0.7
RETURN t.tower_id AS tower,
       t.name AS name,
       t.current_load AS load,
       t.max_capacity AS capacity,
       round(toFloat(t.current_load) / t.max_capacity * 100, 2) AS utilization_percent
ORDER BY utilization_percent DESC;

// Query 4: Subscriber's network path analysis
MATCH (s:Subscriber)-[:OWNS]->(d:Device)-[conn:CONNECTED_TO]->(t:Tower)
RETURN s.subscriber_id AS subscriber,
       s.plan_type AS plan,
       d.model AS device,
       t.tower_id AS tower,
       conn.signal_strength AS signal_dbm,
       conn.connection_type AS connection;

// Query 5: Find redundant paths between towers
MATCH (t1:Tower {tower_id: 'TOWER-NYC-001'}),
      (t2:Tower {tower_id: 'TOWER-NYC-003'})
MATCH paths = allShortestPaths((t1)-[:CONNECTS_TO*]-(t2))
RETURN [n IN nodes(paths) | n.tower_id] AS path_towers,
       length(paths) AS hops;

// Query 6: PageRank for tower importance
CALL orbit.graph.pagerank({damping: 0.85, iterations: 20})
YIELD node_id, pagerank
MATCH (t:Tower) WHERE t.tower_id = node_id
RETURN t.tower_id, t.name, pagerank
ORDER BY pagerank DESC;

// Query 7: Community detection for network segments
CALL orbit.graph.louvain({minCommunitySize: 2})
YIELD community_id, members
RETURN community_id, members;

// Query 8: Find subscribers at risk of churn (low signal, high spend)
MATCH (s:Subscriber)-[:OWNS]->(d:Device)-[conn:CONNECTED_TO]->(t:Tower)
WHERE conn.signal_strength < -75 AND s.monthly_spend > 50
RETURN s.subscriber_id AS at_risk_subscriber,
       s.name AS name,
       s.plan_type AS plan,
       conn.signal_strength AS signal,
       s.monthly_spend AS monthly_spend;

// Query 9: Calculate network diameter
MATCH (t1:Tower), (t2:Tower)
WHERE t1 <> t2
MATCH path = shortestPath((t1)-[:CONNECTS_TO*]-(t2))
RETURN max(length(path)) AS network_diameter;

// Query 10: Betweenness centrality for critical infrastructure
CALL orbit.graph.betweennesscentrality()
YIELD node_id, betweenness
MATCH (n:NetworkNode) WHERE n.node_id = node_id
RETURN n.node_id, n.node_type, betweenness
ORDER BY betweenness DESC
LIMIT 5;
