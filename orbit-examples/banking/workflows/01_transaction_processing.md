# Banking Workflow: Transaction Processing with Fraud Detection

## Overview

End-to-end workflow for processing a customer transaction with real-time fraud detection using multiple OrbitRS protocols.

## Workflow Steps

### 1. Customer Initiates Transaction
**Protocol**: REST API  
**Action**: Customer submits transaction via mobile app or web

```http
POST /api/transactions
{
  "account_id": "acc-12345",
  "amount": 5000.00,
  "merchant": "Electronics Store",
  "location": "New York, NY"
}
```

### 2. Real-Time Fraud Check
**Protocol**: Redis  
**Action**: Check ML fraud score from cache

```redis
# Get customer fraud profile
GET fraud:profile:cust-12345

# Check recent transaction velocity
ZCOUNT transactions:cust-12345 [timestamp-1h] [now]

# Get ML fraud score
GET ml:fraud:score:cust-12345
```

**ML Model**: Random Forest (95% accuracy)  
**Output**: Fraud score 0.0-1.0

### 3. Transaction Validation
**Protocol**: PostgreSQL  
**Action**: Validate account balance and limits

```sql
-- Check account balance
SELECT balance, available_balance, status
FROM accounts
WHERE account_id = 'acc-12345';

-- Check daily transaction limit
SELECT SUM(amount) as daily_total
FROM transactions
WHERE account_id = 'acc-12345'
  AND transaction_date >= CURRENT_DATE;
```

### 4. Fraud Network Analysis
**Protocol**: Neo4j (if fraud score > 0.7)  
**Action**: Check for fraud rings and suspicious patterns

```cypher
// Find related suspicious accounts
MATCH (a:Account {id: 'acc-12345'})-[:TRANSACTED_WITH*1..3]-(suspicious:Account)
WHERE suspicious.fraud_flag = true
RETURN suspicious, COUNT(*) as connections
ORDER BY connections DESC;
```

### 5. Create Transaction Record
**Protocol**: PostgreSQL  
**Action**: Insert transaction with fraud score

```sql
INSERT INTO transactions (
  transaction_id, account_id, amount, merchant_name,
  location_city, location_state, fraud_score, status
) VALUES (
  uuid_generate_v4(), 'acc-12345', 5000.00, 'Electronics Store',
  'New York', 'NY', 0.23, 'PENDING'
);
```

### 6. Update Real-Time Cache
**Protocol**: Redis  
**Action**: Update account balance and transaction history

```redis
# Update available balance
DECRBY balance:acc-12345 5000

# Add to transaction history
ZADD transactions:cust-12345 [timestamp] "txn-67890"

# Update fraud profile
SETEX fraud:profile:cust-12345 3600 "{...updated_profile...}"
```

### 7. Create Fraud Alert (if needed)
**Protocol**: PostgreSQL  
**Action**: Create alert for manual review if fraud score > 0.7

```sql
INSERT INTO fraud_alerts (
  alert_id, transaction_id, customer_id,
  alert_type, fraud_score, status
) VALUES (
  uuid_generate_v4(), 'txn-67890', 'cust-12345',
  'ML_DETECTION', 0.85, 'OPEN'
);
```

### 8. Audit Trail
**Protocol**: PostgreSQL  
**Action**: Log all access for HIPAA/compliance

```sql
INSERT INTO audit_log (
  audit_id, user_id, action, table_name,
  record_id, timestamp, ip_address
) VALUES (
  uuid_generate_v4(), 'cust-12345', 'CREATE', 'transactions',
  'txn-67890', CURRENT_TIMESTAMP, '192.168.1.1'
);
```

### 9. Notification
**Protocol**: Redis Pub/Sub  
**Action**: Send real-time notification to customer

```redis
PUBLISH notifications:cust-12345 '{
  "type": "TRANSACTION_PROCESSED",
  "amount": 5000.00,
  "status": "PENDING",
  "fraud_check": "PASSED"
}'
```

## Performance Metrics

| Step | Protocol | Latency |
|------|----------|---------|
| Fraud Check | Redis | <20ms |
| Validation | PostgreSQL | <30ms |
| Network Analysis | Neo4j | <50ms |
| Transaction Insert | PostgreSQL | <40ms |
| Cache Update | Redis | <10ms |
| **Total** | **Multi-protocol** | **<150ms** |

## Decision Flow

```
Transaction Request
    ↓
Fraud Check (Redis ML)
    ↓
├─ Score < 0.3 → Auto-Approve
├─ Score 0.3-0.7 → Additional Checks (Neo4j)
└─ Score > 0.7 → Manual Review + Alert
    ↓
Balance Check (PostgreSQL)
    ↓
Create Transaction (PostgreSQL)
    ↓
Update Cache (Redis)
    ↓
Audit Log (PostgreSQL)
    ↓
Notify Customer (Redis Pub/Sub)
```

## Error Handling

- **Insufficient Funds**: Return 402, no transaction created
- **Fraud Detected**: Block transaction, create alert, notify customer
- **System Error**: Rollback transaction, log error, retry logic

## Compliance

- **PCI DSS**: Card data encrypted
- **KYC/AML**: Customer verification required
- **Audit Trail**: All actions logged
- **Data Retention**: 7 years for regulatory compliance
