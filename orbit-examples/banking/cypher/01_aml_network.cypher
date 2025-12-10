// Banking Use Case: Anti-Money Laundering (AML) Network Analysis
// Purpose: Detect money laundering schemes (Smurfing, Layering, Circular flows) by analyzing relationships
// between accounts, customers, devices, and transactions.

// 1. Create Data: Accounts, Customers, and Transfers
CREATE (c1:Customer {id: "CUST_001", name: "Alice", risk_score: 10})
CREATE (c2:Customer {id: "CUST_002", name: "Bob", risk_score: 85}) // High risk
CREATE (c3:Customer {id: "CUST_003", name: "Charlie", risk_score: 15})
CREATE (a1:Account {id: "ACC_1001", type: "SAVINGS", balance: 50000})
CREATE (a2:Account {id: "ACC_2002", type: "CHECKING", balance: 1200})
CREATE (a3:Account {id: "ACC_3003", type: "BUSINESS", balance: 150000})
CREATE (dev1:Device {id: "DEV_IP_192_168_1_5", fingerprint: "fp_xyz_123"})

// Relationships
CREATE (c1)-[:OWNS]->(a1)
CREATE (c2)-[:OWNS]->(a2)
CREATE (c3)-[:OWNS]->(a3)

// Bob logs in from a device known for suspicious activity
CREATE (c2)-[:USED_DEVICE {last_seen: datetime()}]->(dev1)

// Transfers
// Pattern: Placement/Structuring?
CREATE (a1)-[:TRANSFERRED {amount: 9000, date: datetime("2024-12-01T10:00:00")}]->(a2)
CREATE (a3)-[:TRANSFERRED {amount: 9500, date: datetime("2024-12-01T10:05:00")}]->(a2)
// Pattern: Integration/Layering
CREATE (a2)-[:TRANSFERRED {amount: 18000, date: datetime("2024-12-02T09:00:00")}]->(a3)

// 2. Query: Detect "Smurfing" / Structuring
// Finding accounts receiving multiple transfers just below the reporting threshold ($10,000)
// from different sources within a short time window.
MATCH (sender:Account)-[t:TRANSFERRED]->(receiver:Account)
WHERE t.amount > 8000 AND t.amount < 10000
WITH receiver, count(sender) as sender_count, sum(t.amount) as total_received
WHERE sender_count >= 2
RETURN receiver.id, sender_count, total_received
ORDER BY total_received DESC;

// 3. Query: Detect Circular Money Flow (Round Tripping)
// A -> B -> C -> A patterns often used to artificially inflate transaction volume or obscure funds.
MATCH path = (a:Account)-[:TRANSFERRED*3..5]->(a)
RETURN [n in nodes(path) | n.id] as circular_path,
       [r in relationships(path) | r.amount] as amounts
LIMIT 5;

// 4. Query: Multi-Hop Trace from High-Risk Customer
// Find where money from a high-risk customer (Bob) eventually lands up to 4 hops away.
MATCH (risky:Customer {name: "Bob"})-[:OWNS]->(risky_acc:Account)
MATCH path = (risky_acc)-[:TRANSFERRED*1..4]->(destination:Account)
RETURN destination.id as final_destination, 
       length(path) as hops, 
       reduce(s = 0, r in relationships(path) | s + r.amount) as total_flow_value
ORDER BY total_flow_value DESC;

// 5. Query: Device-Based Link Analysis
// Find customers who share the same device as a known high-risk entity.
MATCH (known_bad:Customer {risk_score: 85})-[:USED_DEVICE]->(d:Device)<-[:USED_DEVICE]-(linked_customer:Customer)
WHERE known_bad <> linked_customer
RETURN known_bad.name as high_risk_user, 
       linked_customer.name as potentially_compromised_user, 
       d.fingerprint as shared_device_id;
