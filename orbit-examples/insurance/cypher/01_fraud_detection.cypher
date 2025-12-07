// ============================================================================
// OrbitRS Insurance Examples - Neo4j Graph Relationships
// ============================================================================
// Fraud detection, customer networks, claims investigation using Cypher
// ============================================================================

// ============================================================================
// CREATE NODES - CUSTOMERS
// ============================================================================

CREATE (c1:Customer {
  customer_id: '550e8400-e29b-41d4-a716-446655440000',
  name: 'John Doe',
  email: 'john.doe@email.com',
  phone: '+14155550123',
  risk_score: 72,
  customer_since: date('2019-01-15')
});

CREATE (c2:Customer {
  customer_id: '550e8400-e29b-41d4-a716-446655440001',
  name: 'Jane Smith',
  email: 'jane.smith@email.com',
  phone: '+14155550124',
  risk_score: 65,
  customer_since: date('2020-03-20')
});

CREATE (c3:Customer {
  customer_id: '550e8400-e29b-41d4-a716-446655440002',
  name: 'Bob Johnson',
  email: 'bob.johnson@email.com',
  phone: '+14155550125',
  risk_score: 88,
  customer_since: date('2023-06-10')
});

// ============================================================================
// CREATE NODES - POLICIES
// ============================================================================

CREATE (p1:Policy {
  policy_id: 'POL-001',
  policy_number: 'AUTO-2024-001234',
  policy_type: 'AUTO',
  status: 'ACTIVE',
  premium: 1650.00,
  effective_date: date('2024-01-15')
});

CREATE (p2:Policy {
  policy_id: 'POL-002',
  policy_number: 'HOME-2024-005678',
  policy_type: 'HOME',
  status: 'ACTIVE',
  premium: 3300.00,
  effective_date: date('2024-02-01')
});

// ============================================================================
// CREATE NODES - CLAIMS
// ============================================================================

CREATE (cl1:Claim {
  claim_id: 'CLM-2024-001',
  claim_number: '2024-AUTO-001234',
  claim_type: 'AUTO_COLLISION',
  claim_amount: 5000.00,
  incident_date: date('2024-12-05'),
  status: 'APPROVED',
  fraud_score: 15
});

CREATE (cl2:Claim {
  claim_id: 'CLM-2024-002',
  claim_number: '2024-HOME-005678',
  claim_type: 'HOME_FIRE',
  claim_amount: 250000.00,
  incident_date: date('2024-12-06'),
  status: 'INVESTIGATING',
  fraud_score: 85
});

// ============================================================================
// CREATE NODES - AGENTS
// ============================================================================

CREATE (a1:Agent {
  agent_id: 'agent-001',
  name: 'Sarah Williams',
  license: 'AG-12345',
  territory: 'Northern California',
  commission_rate: 0.10
});

// ============================================================================
// CREATE NODES - ADDRESSES
// ============================================================================

CREATE (addr1:Address {
  address_id: 'addr-001',
  street: '123 Main St',
  city: 'San Francisco',
  state: 'CA',
  zip: '94102'
});

CREATE (addr2:Address {
  address_id: 'addr-002',
  street: '456 Oak Ave',
  city: 'San Francisco',
  state: 'CA',
  zip: '94103'
});

// ============================================================================
// CREATE RELATIONSHIPS - CUSTOMER TO POLICY
// ============================================================================

MATCH (c:Customer {customer_id: '550e8400-e29b-41d4-a716-446655440000'})
MATCH (p:Policy {policy_id: 'POL-001'})
CREATE (c)-[:HAS_POLICY {since: date('2024-01-15')}]->(p);

MATCH (c:Customer {customer_id: '550e8400-e29b-41d4-a716-446655440001'})
MATCH (p:Policy {policy_id: 'POL-002'})
CREATE (c)-[:HAS_POLICY {since: date('2024-02-01')}]->(p);

// ============================================================================
// CREATE RELATIONSHIPS - POLICY TO CLAIM
// ============================================================================

MATCH (p:Policy {policy_id: 'POL-001'})
MATCH (cl:Claim {claim_id: 'CLM-2024-001'})
CREATE (p)-[:HAS_CLAIM {filed_date: date('2024-12-06')}]->(cl);

MATCH (p:Policy {policy_id: 'POL-002'})
MATCH (cl:Claim {claim_id: 'CLM-2024-002'})
CREATE (p)-[:HAS_CLAIM {filed_date: date('2024-12-06')}]->(cl);

// ============================================================================
// CREATE RELATIONSHIPS - AGENT TO CUSTOMER
// ============================================================================

MATCH (a:Agent {agent_id: 'agent-001'})
MATCH (c:Customer {customer_id: '550e8400-e29b-41d4-a716-446655440000'})
CREATE (a)-[:MANAGES {since: date('2019-01-15')}]->(c);

MATCH (a:Agent {agent_id: 'agent-001'})
MATCH (c:Customer {customer_id: '550e8400-e29b-41d4-a716-446655440001'})
CREATE (a)-[:MANAGES {since: date('2020-03-20')}]->(c);

// ============================================================================
// CREATE RELATIONSHIPS - CUSTOMER TO ADDRESS
// ============================================================================

MATCH (c:Customer {customer_id: '550e8400-e29b-41d4-a716-446655440000'})
MATCH (addr:Address {address_id: 'addr-001'})
CREATE (c)-[:LIVES_AT {since: date('2015-06-01'), is_primary: true}]->(addr);

// ============================================================================
// CREATE RELATIONSHIPS - FAMILY/RELATED CUSTOMERS
// ============================================================================

MATCH (c1:Customer {customer_id: '550e8400-e29b-41d4-a716-446655440000'})
MATCH (c2:Customer {customer_id: '550e8400-e29b-41d4-a716-446655440001'})
CREATE (c1)-[:RELATED_TO {relationship: 'SPOUSE'}]->(c2);

// ============================================================================
// FRAUD DETECTION QUERIES
// ============================================================================

// Find claims with high fraud scores
MATCH (cl:Claim)
WHERE cl.fraud_score > 70
RETURN cl.claim_number, cl.claim_type, cl.claim_amount, cl.fraud_score
ORDER BY cl.fraud_score DESC;

// Find customers with multiple high-value claims in short period
MATCH (c:Customer)-[:HAS_POLICY]->(p:Policy)-[:HAS_CLAIM]->(cl:Claim)
WHERE cl.incident_date > date('2024-01-01')
WITH c, COUNT(cl) AS claim_count, SUM(cl.claim_amount) AS total_claimed
WHERE claim_count > 2 OR total_claimed > 100000
RETURN c.name, c.email, claim_count, total_claimed
ORDER BY total_claimed DESC;

// Find suspicious claim patterns - same address, different customers
MATCH (c1:Customer)-[:LIVES_AT]->(addr:Address)<-[:LIVES_AT]-(c2:Customer)
MATCH (c1)-[:HAS_POLICY]->(p1:Policy)-[:HAS_CLAIM]->(cl1:Claim)
MATCH (c2)-[:HAS_POLICY]->(p2:Policy)-[:HAS_CLAIM]->(cl2:Claim)
WHERE c1 <> c2
  AND cl1.incident_date = cl2.incident_date
RETURN c1.name, c2.name, addr.street, cl1.claim_number, cl2.claim_number, cl1.incident_date;

// Find claim rings - customers who filed claims on same date
MATCH (c1:Customer)-[:HAS_POLICY]->(:Policy)-[:HAS_CLAIM]->(cl1:Claim)
MATCH (c2:Customer)-[:HAS_POLICY]->(:Policy)-[:HAS_CLAIM]->(cl2:Claim)
WHERE c1 <> c2
  AND cl1.incident_date = cl2.incident_date
  AND cl1.claim_amount > 10000
  AND cl2.claim_amount > 10000
RETURN c1.name, c2.name, cl1.incident_date, cl1.claim_amount, cl2.claim_amount;

// ============================================================================
// CUSTOMER NETWORK ANALYSIS
// ============================================================================

// Find all relationships for a customer
MATCH (c:Customer {customer_id: '550e8400-e29b-41d4-a716-446655440000'})-[r]->(n)
RETURN c, type(r) AS relationship_type, n;

// Find customers with shared addresses (potential fraud or family)
MATCH (c1:Customer)-[:LIVES_AT]->(addr:Address)<-[:LIVES_AT]-(c2:Customer)
WHERE c1 <> c2
RETURN c1.name, c2.name, addr.street, addr.city, addr.zip;

// Find agent's customer network
MATCH (a:Agent {agent_id: 'agent-001'})-[:MANAGES]->(c:Customer)
OPTIONAL MATCH (c)-[:HAS_POLICY]->(p:Policy)
RETURN a.name AS agent, c.name AS customer, COUNT(p) AS policy_count
ORDER BY policy_count DESC;

// Find customers referred by same agent
MATCH (a:Agent)-[:MANAGES]->(c:Customer)
WITH a, COLLECT(c) AS customers
WHERE SIZE(customers) > 1
RETURN a.name, SIZE(customers) AS customer_count, [c IN customers | c.name] AS customer_names;

// ============================================================================
// CLAIMS INVESTIGATION
// ============================================================================

// Find similar claims (same type, similar amount, similar date)
MATCH (cl1:Claim)
WHERE cl1.claim_id = 'CLM-2024-001'
MATCH (cl2:Claim)
WHERE cl2.claim_id <> cl1.claim_id
  AND cl2.claim_type = cl1.claim_type
  AND ABS(cl2.claim_amount - cl1.claim_amount) < 1000
  AND ABS(duration.between(cl2.incident_date, cl1.incident_date).days) < 30
RETURN cl1.claim_number, cl2.claim_number, cl2.claim_amount, cl2.incident_date;

// Find claim history for a customer
MATCH (c:Customer {customer_id: '550e8400-e29b-41d4-a716-446655440000'})
      -[:HAS_POLICY]->(p:Policy)-[:HAS_CLAIM]->(cl:Claim)
RETURN p.policy_number, cl.claim_number, cl.claim_type, 
       cl.claim_amount, cl.incident_date, cl.status
ORDER BY cl.incident_date DESC;

// Find customers with claims across multiple policy types
MATCH (c:Customer)-[:HAS_POLICY]->(p:Policy)-[:HAS_CLAIM]->(cl:Claim)
WITH c, COLLECT(DISTINCT p.policy_type) AS policy_types, COUNT(cl) AS claim_count
WHERE SIZE(policy_types) > 1
RETURN c.name, policy_types, claim_count
ORDER BY claim_count DESC;

// ============================================================================
// RISK ANALYSIS
// ============================================================================

// Find high-risk customers (high risk score + recent claims)
MATCH (c:Customer)-[:HAS_POLICY]->(p:Policy)-[:HAS_CLAIM]->(cl:Claim)
WHERE c.risk_score > 70
  AND cl.incident_date > date('2024-01-01')
RETURN c.name, c.risk_score, COUNT(cl) AS recent_claims, SUM(cl.claim_amount) AS total_claimed
ORDER BY c.risk_score DESC, total_claimed DESC;

// Find policies at risk (high claim frequency)
MATCH (p:Policy)-[:HAS_CLAIM]->(cl:Claim)
WITH p, COUNT(cl) AS claim_count, SUM(cl.claim_amount) AS total_claimed
WHERE claim_count > 2
RETURN p.policy_number, p.policy_type, claim_count, total_claimed, p.premium
ORDER BY claim_count DESC;

// ============================================================================
// AGENT PERFORMANCE
// ============================================================================

// Agent performance - customers, policies, claims
MATCH (a:Agent)-[:MANAGES]->(c:Customer)-[:HAS_POLICY]->(p:Policy)
OPTIONAL MATCH (p)-[:HAS_CLAIM]->(cl:Claim)
WITH a, COUNT(DISTINCT c) AS customer_count, 
     COUNT(DISTINCT p) AS policy_count,
     COUNT(cl) AS claim_count,
     SUM(p.premium) AS total_premium
RETURN a.name, customer_count, policy_count, claim_count, total_premium
ORDER BY total_premium DESC;

// Find agents with high claim ratios
MATCH (a:Agent)-[:MANAGES]->(c:Customer)-[:HAS_POLICY]->(p:Policy)
OPTIONAL MATCH (p)-[:HAS_CLAIM]->(cl:Claim)
WITH a, COUNT(DISTINCT p) AS policy_count, COUNT(cl) AS claim_count
WHERE policy_count > 0
RETURN a.name, policy_count, claim_count, 
       ROUND(toFloat(claim_count) / policy_count, 2) AS claims_per_policy
ORDER BY claims_per_policy DESC;

// ============================================================================
// GRAPH ALGORITHMS (if Neo4j GDS is available)
// ============================================================================

// PageRank - find most influential customers in network
// CALL gds.pageRank.stream('customer_network')
// YIELD nodeId, score
// RETURN gds.util.asNode(nodeId).name AS customer, score
// ORDER BY score DESC LIMIT 10;

// Community detection - find customer clusters
// CALL gds.louvain.stream('customer_network')
// YIELD nodeId, communityId
// RETURN communityId, COLLECT(gds.util.asNode(nodeId).name) AS customers
// ORDER BY SIZE(customers) DESC;

// ============================================================================
// CLEANUP (use with caution!)
// ============================================================================

// Delete all nodes and relationships
// MATCH (n) DETACH DELETE n;

// Delete specific claim
// MATCH (cl:Claim {claim_id: 'CLM-2024-001'}) DETACH DELETE cl;

print("Neo4j insurance graph created successfully!");
