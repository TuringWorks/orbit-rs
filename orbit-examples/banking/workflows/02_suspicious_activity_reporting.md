# Workflow: Suspicious Activity Reporting (SAR) Pipeline

## Overview
This workflow orchestrates the end-to-end process of detecting, investigating, and reporting suspicious financial activity. It demonstrates the **polyglot** power of Orbit-RS by leveraging the best engine for each phase of the AML lifecycle.

## Workflow Steps

### 1. Transaction Ingestion (SQL)
**System**: Orbit SQL Engine  
**Action**: High-performance ACID storage of the raw transaction.

```sql
INSERT INTO transactions (id, time, sender_id, receiver_id, amount, currency, status)
VALUES ('TXN_999', NOW(), 'ACC_1001', 'ACC_2002', 9500.00, 'USD', 'PENDING');
```

---

### 2. Real-Time Fraud Scoring (Redis)
**System**: Redis (Orbit Protocol)  
**Action**: Synchronous check against in-memory features (Blacklists, Velocity).

```redis
# 1. Check Blacklist
SISMEMBER blacklist:devices "DEV_FP_123"

# 2. Update Velocity Counter
PFADD velocity:tx_count:ACC_1001:1h "TXN_999"
val = PFCOUNT velocity:tx_count:ACC_1001:1h

# DECISION: If val > 10 OR Blacklisted -> FLAG as "SUSPICIOUS"
```

---

### 3. Network Investigation (Cypher)
**System**: Cypher (Orbit Graph Engine)  
**Trigger**: Transaction flagged as "Suspicious" in Step 2.  
**Action**: Explore the graph to find hidden connections (e.g., is the recipient connected to known criminals?).

```cypher
// Trace path from Sender to High-Risk entities
MATCH path = (sender:Account {id: "ACC_1001"})-[*1..3]->(risky:Customer {risk_level: "HIGH"})
RETURN path;
```

---

### 4. EDD Profile Retrieval (MongoDB)
**System**: MongoDB (Orbit Document Engine)  
**Action**: Fetch full KYC dossier for the investigator to review.

```javascript
db.kyc_profiles.findOne(
    { "customer_id": "CUST_001" },
    { "identity_proofs": 1, "edd_history": 1, "sanctions_screening": 1 }
);
```

---

### 5. Generate & File SAR (SQL + Document)
**Action**: Compilation of evidence into a formal report.

1.  **Create Case in SQL**:
    ```sql
    INSERT INTO aml_cases (case_id, tx_id, status, assigned_to) 
    VALUES ('CASE_555', 'TXN_999', 'OPEN', 'OFFICER_JONES');
    ```

2.  **Archive Evidence in MongoDB** (JSON Dump of graph trace + logs):
    ```javascript
    db.sar_evidence.insertOne({
        "case_id": "CASE_555",
        "generated_at": new Date(),
        "trigger_reason": "High Velocity + Link to Risk Node",
        "graph_snapshot": { ... }, // Data from Cypher
        "transaction_details": { ... } // Data from SQL
    });
    ```
