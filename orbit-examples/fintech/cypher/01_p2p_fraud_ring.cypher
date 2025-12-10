// FinTech Use Case: Peer-to-Peer (P2P) Payment Fraud
// Purpose: Detect coordinated fraud in social payment apps (like Venmo/CashApp).
// Scenarios: Fake activity to boost credit limits, stolen card cash-out rings.

// 1. Data Model
CREATE (u1:User {id: "u_alice", created_at: datetime()})
CREATE (u2:User {id: "u_bob"})
CREATE (u3:User {id: "u_charlie"})
CREATE (u4:User {id: "u_fraudster"})

// Social Links (Friends logic)
CREATE (u1)-[:FRIEND]->(u2)
CREATE (u2)-[:FRIEND]->(u3)

// Payments
CREATE (u1)-[:PAID {amount: 50.00, note: "Dinner"}]->(u2)
CREATE (u2)-[:PAID {amount: 50.00, note: "Reimbursement"}]->(u3)
CREATE (u3)-[:PAID {amount: 50.00, note: "Gift"}]->(u1) -- Circular!

// Fraud Cluster
CREATE (u4)-[:PAID {amount: 1000.00}]->(u2)

// 2. Query: Detect Circular Tipping (Synthetic Volume)
// Users A -> B -> C -> A sending money in a circle to manufacture transaction history
// (often to qualify for loans or bypass limits).
MATCH path = (a:User)-[:PAID*3..5]->(a)
RETURN [n in nodes(path) | n.id] as ring_members,
       [r in relationships(path) | r.amount] as amounts
LIMIT 10;

// 3. Query: Payment Outside Social Graph
// High value payments to non-friends are riskier.
MATCH (sender:User)-[p:PAID]->(receiver:User)
WHERE NOT (sender)-[:FRIEND]-(receiver)
AND p.amount > 500
RETURN sender.id, receiver.id, p.amount as risky_transfer;

// 4. Query: Mule Detection (Fan-In / Fan-Out)
// Accounts receiving many small payments then sending one large payment.
// (Placeholder logic for pattern matching)
MATCH (mule:User)
WITH mule
MATCH (sender:User)-[p_in:PAID]->(mule)
WITH mule, count(sender) as incoming_count, sum(p_in.amount) as total_in
MATCH (mule)-[p_out:PAID]->(master:User)
WITH mule, incoming_count, total_in, p_out.amount as out_amount
WHERE incoming_count > 5 AND out_amount > (total_in * 0.9)
RETURN mule.id as suspected_mule, total_in, out_amount;
