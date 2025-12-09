# Banking & Financial Services

Comprehensive examples demonstrating OrbitRS's multi-protocol capabilities for banking applications including core banking, real-time fraud detection, transaction processing, and regulatory compliance.

## Why OrbitRS for Banking?

OrbitRS excels in banking environments where:
- **Sub-millisecond latency** is required for payment authorization decisions
- **Multi-protocol access** allows trading systems (Redis), core banking (PostgreSQL), and analytics (CQL) to share data
- **ACID compliance** ensures transaction integrity across distributed systems
- **7-year retention** with efficient storage tiering meets regulatory requirements
- **ML integration** enables real-time fraud scoring and credit risk assessment

## Scenarios

### 1. Core Banking - PostgreSQL
**File**: [`sql/01_schema_core.sql`](sql/01_schema_core.sql)

Comprehensive relational schema for banking operations:
- **Customers**: KYC/AML status, risk ratings, ML-generated fraud and churn scores
- **Accounts**: Checking, savings, credit cards, loans with real-time balances
- **Transactions**: Full transaction lifecycle with fraud detection integration
- **Cards**: Debit/credit card management with PCI-compliant encrypted storage
- **Loans**: Personal, auto, mortgage, student loans with ML default prediction
- **Fraud Alerts**: ML-powered fraud detection with risk factors
- **Views**: Customer account summaries, high-risk transaction monitoring

**Use Case**: Account management, balance inquiries, transaction posting, loan origination

```sql
-- Query high-risk transactions for review
SELECT t.transaction_number, t.amount, t.fraud_score, t.merchant_name,
       c.customer_number, c.first_name, c.last_name
FROM transactions t
JOIN accounts a ON t.account_id = a.account_id
JOIN customers c ON a.customer_id = c.customer_id
WHERE t.fraud_score > 0.7
ORDER BY t.fraud_score DESC;
```

### 2. Session Cache & Real-time Banking - Redis
**File**: [`redis/01_user_sessions.redis`](redis/01_user_sessions.redis)

High-performance real-time operations using Redis data structures:
- **Sessions**: JWT tokens, MFA state, multi-device tracking with auto-expiration
- **2FA**: Time-limited verification codes with attempt tracking
- **Fraud Velocity**: Transaction counting per minute/hour, geographic anomaly detection
- **Alerts**: Real-time pub/sub for fraud alerts and security notifications
- **Notifications**: Streams for transaction alerts, balance warnings, security events
- **Rate Limiting**: API and transfer limits per user
- **Caching**: Customer profiles, account balances, exchange rates, ATM locations
- **Analytics**: Real-time spending by category, monthly trends
- **Security**: Failed login tracking, account/card locks
- **Queues**: Pending transaction processing with priority support
- **Market Data**: Real-time stock price pub/sub for trading platforms
- **Gamification**: Savings leaderboards
- **Full-Text Search**: Merchant lookup (OrbitRS extension)
- **Vector Search**: Transaction similarity for fraud patterns (OrbitRS extension)

**Use Case**: Mobile banking, real-time fraud detection, session management, notifications

```redis
# Fraud velocity check: count transactions per minute
INCR velocity:txn:user_1001:minute
EXPIRE velocity:txn:user_1001:minute 60 NX

# Geographic anomaly detection
GEOADD geo:txn:user_1001:day -122.4194 37.7749 "txn_001"
GEODIST geo:txn:user_1001:day "txn_001" "txn_002" km

# Real-time fraud alert
PUBLISH alert:fraud:high_risk "FRAUD_DETECTED|user_1001|TXN-12345|$5000|UNUSUAL_LOCATION"
```

### 3. Transaction History & Time-Series - CQL (Cassandra)
**File**: [`cql/01_transactions.cql`](cql/01_transactions.cql)

Wide-column time-series storage for high-volume financial data:
- **Customer Transactions**: Partitioned by account + month for efficient statements (7-year retention)
- **Fraud Events**: Real-time fraud scoring with ML model tracking and resolution workflow
- **Daily Balances**: End-of-day snapshots for reconciliation and reporting
- **Card Authorizations**: Real-time auth events with AVS/CVV results
- **Payment Schedules**: Recurring bill pay and scheduled transfers
- **Loan Payments**: Amortization tracking with principal/interest breakdown
- **Spending Analytics**: Pre-aggregated category spending with MoM trends
- **Transaction Embeddings**: Vector storage for ML fraud clustering (OrbitRS extension)
- **SAR Reports**: Suspicious Activity Report tracking for BSA compliance
- **Data Access Log**: Audit trail for regulatory compliance

**Use Case**: Statement generation, fraud analytics, regulatory reporting, ML training

```cql
-- Generate monthly statement
SELECT transaction_time, transaction_type, amount, merchant_name, balance_after
FROM customer_transactions
WHERE account_id = 11111111-1111-1111-1111-111111111111
  AND year_month = '2024-12';

-- Query fraud events with high scores
SELECT event_time, amount, fraud_score, risk_factors, alert_status
FROM fraud_events
WHERE customer_id = 22222222-2222-2222-2222-222222222222
  AND event_date = '2024-12-09'
  AND fraud_score > 0.7
ALLOW FILTERING;
```

### 4. Audit Trail - MongoDB
**File**: [`mongodb/01_audit_logs.js`](mongodb/01_audit_logs.js)

Flexible document storage for compliance and audit:
- Immutable JSON logs of every system interaction
- Login attempts with device and location metadata
- Transaction lifecycle events
- Failed authentication tracking

**Use Case**: SOX compliance, security audits, incident investigation

## Multi-Protocol Integration Pattern

A typical banking deployment uses multiple protocols for different workloads:

```
                    ┌─────────────────────────────────────────────────────────┐
                    │                      OrbitRS                            │
                    │                                                         │
  ┌─────────────┐   │   ┌────────────┐   ┌────────────┐   ┌────────────┐    │
  │ Core Banking│◄──┼──►│ PostgreSQL │   │   Redis    │   │    CQL     │    │
  │    System   │   │   │   :5432    │   │   :6379    │   │   :9042    │    │
  └─────────────┘   │   └─────┬──────┘   └─────┬──────┘   └─────┬──────┘    │
                    │         │                │                │           │
  ┌─────────────┐   │         │                │                │           │
  │   Mobile    │◄──┼─────────┼────────────────┘                │           │
  │   Banking   │   │         │    (sessions, notifications)    │           │
  └─────────────┘   │         │                                 │           │
                    │         │                                 │           │
  ┌─────────────┐   │         │    ┌────────────────────────────┘           │
  │   Fraud     │◄──┼─────────┼────┘  (high-volume analytics)               │
  │  Detection  │   │         │                                             │
  └─────────────┘   │         ▼                                             │
                    │   ┌──────────────────────────────────────────────┐    │
                    │   │           Unified Storage Layer              │    │
                    │   │  (RocksDB + 7-Year Tiered Archival)         │    │
                    │   └──────────────────────────────────────────────┘    │
                    └─────────────────────────────────────────────────────────┘
```

## Compliance Considerations

These examples include patterns for:
- **PCI-DSS**: Encrypted card data, tokenization, access logging
- **SOX**: Audit trails, separation of duties, change management
- **BSA/AML**: Suspicious Activity Reports (SAR), transaction monitoring
- **GDPR**: Data retention policies, access controls
- **FFIEC**: Secure session management, fraud detection

## Performance Characteristics

| Operation | Protocol | Expected Latency |
|-----------|----------|-----------------|
| Balance inquiry | Redis (cache) | < 1ms |
| Card authorization | Redis + PostgreSQL | < 10ms |
| Transaction post | PostgreSQL | < 50ms |
| Statement generation | CQL | < 500ms |
| Fraud scoring (ML) | Redis + CQL | < 100ms |

## Getting Started

1. Start OrbitRS with banking configuration:
```bash
cargo run --bin orbit-server -- --config config/banking.toml
```

2. Load the SQL schema:
```bash
psql -h localhost -p 5432 -f sql/01_schema_core.sql
```

3. Run Redis session examples:
```bash
redis-cli -p 6379 < redis/01_user_sessions.redis
```

4. Load CQL transaction schema:
```bash
cqlsh localhost 9042 -f cql/01_transactions.cql
```

5. Load MongoDB audit collection:
```bash
mongosh --port 27017 < mongodb/01_audit_logs.js
```

## Related Documentation

- [PostgreSQL Protocol](../../docs/content/protocols/postgresql.md)
- [Redis Protocol](../../docs/content/protocols/redis.md)
- [CQL Protocol](../../docs/content/protocols/cql.md)
- [MongoDB Protocol](../../docs/content/protocols/mongodb.md)
- [Vector Search](../../docs/content/features/vector-search.md)
- [Full-Text Search](../../docs/content/features/full-text-search.md)
