// AdTech Use Case: Identity Graph & Cross-Device Tracking
// Purpose: Deterministically or probabilistically link disparate identifiers (Cookies, Device IDs, Emails)
// to a single "User" entity to enable cross-device retargeting and frequency capping.

// 1. Create Core Nodes
CREATE (u:User {id: "U_555666", type: "Household"})
CREATE (cookie1:Cookie {id: "c_abc123", domain: "web"})
CREATE (cookie2:Cookie {id: "c_xyz789", domain: "web"})
CREATE (mob:Device {id: "idfa_0000", os: "iOS"})
CREATE (email:EmailHash {hash: "hash_bob@email.com"})
CREATE (ip:IPAddress {ip: "192.168.1.5"})

// 2. Establish Links (Observation Events)
// "Cookie c_abc123 logged in with email hash X" -> Deterministic Link
CREATE (cookie1)-[:LOGGED_IN_WITH]->(email)
CREATE (u)-[:HAS_EMAIL]->(email)

// "Mobile Device idfa_0000 logged in with email hash X" -> Deterministic Link
CREATE (mob)-[:LOGGED_IN_WITH]->(email)

// "Cookie c_xyz789 seen on IP 192.168.1.5" -> Probabilistic Signal
CREATE (cookie2)-[:SEEN_ON {count: 5, confidence: 0.6}]->(ip)
// "Mobile Device seen on same IP"
CREATE (mob)-[:SEEN_ON {count: 20, confidence: 0.9}]->(ip)

// 3. Query: Identity Resolution (Find all IDs for a User)
// When we want to target "User U_555666", what IDs do we bid on?
MATCH (u:User {id: "U_555666"})-[:HAS_EMAIL]->(e:EmailHash)
OPTIONAL MATCH (e)<-[:LOGGED_IN_WITH]-(device_or_cookie)
RETURN u.id, collect(device_or_cookie.id) as targetable_ids;

// 4. Query: Probabilistic Matching (Household Resolution)
// Find devices that likely belong to the same household based on shared IP frequency.
MATCH (d1:Device)-[r1:SEEN_ON]->(ip:IPAddress)<-[r2:SEEN_ON]-(d2:Device)
WHERE d1 <> d2 AND r1.count > 10 AND r2.count > 10
RETURN d1.id, d2.id, ip.ip, (r1.confidence * r2.confidence) as match_score
ORDER BY match_score DESC;

// 5. Query: Attribution (Conversion Path)
// Did a user see an ad on Cookie1 and convert on Device1?
// (Simplified path logic)
MATCH path = (impression_cookie:Cookie)-[:BELONGS_TO_GRAPH]-(u:User)-[:BELONGS_TO_GRAPH]-(conversion_device:Device)
RETURN path;
