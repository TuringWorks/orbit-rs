# Telecommunications Examples - OrbitRS

## Overview

This directory contains comprehensive telecommunications industry examples demonstrating OrbitRS's multi-protocol capabilities for real-world telco operations including subscriber management, billing, network operations, cell tower management, bandwidth bidding, and marketing.

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────────┐
│                    Telco Platform on OrbitRS                            │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌────────────┐   │
│  │  PostgreSQL  │  │    Redis     │  │   MongoDB    │  │   Neo4j    │   │
│  │   :5432      │  │    :6379     │  │   :27017     │  │   :7687    │   │
│  ├──────────────┤  ├──────────────┤  ├──────────────┤  ├────────────┤   │
│  │ Subscribers  │  │ Active Calls │  │ CDR Records  │  │ Network    │   │
│  │ Billing      │  │ Data Sessions│  │ Contracts    │  │ Topology   │   │
│  │ Plans        │  │ Usage Meters │  │ Logs         │  │ Fraud Det. │   │
│  │ Cell Towers  │  │ Rate Limits  │  │ Configs      │  │ Referrals  │   │
│  │ Devices      │  │ Bandwidth    │  │ Marketing    │  │ Coverage   │   │
│  └──────────────┘  └──────────────┘  └──────────────┘  └────────────┘   │
│                                                                         │
│  ┌──────────────┐  ┌──────────────────────────────────────────────┐     │
│  │  Cassandra   │  │          Workflows & Integration             │     │
│  │   :9042      │  ├──────────────────────────────────────────────┤     │
│  ├──────────────┤  │ • Subscriber Onboarding                      │     │
│  │ Network KPIs │  │ • Real-Time Call Routing                     │     │
│  │ Usage Trends │  │ • Billing Cycle Processing                   │     │
│  │ QoS Metrics  │  │ • Network Optimization                       │     │
│  │ Tower Load   │  │ • Bandwidth Auction/Bidding                  │     │
│  │ Bandwidth    │  │ • Marketing Campaign Execution               │     │
│  └──────────────┘  └──────────────────────────────────────────────┘     │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Protocol Usage by Domain

| Domain | PostgreSQL | Redis | MongoDB | Neo4j | Cassandra |
|--------|-----------|-------|---------|-------|-----------|
| **Subscriber Management** | ✓ Master data | ✓ Active sessions | ✓ Documents | ✓ Relationships | ✓ History |
| **Billing** | ✓ Invoices | ✓ Real-time charges | ✓ Receipts | - | ✓ Payment trends |
| **Plans** | ✓ Catalog | ✓ Active plans | ✓ Terms | - | ✓ Usage patterns |
| **Network** | ✓ Infrastructure | ✓ Status/alerts | ✓ Logs | ✓ Topology | ✓ Performance |
| **Cell Towers** | ✓ Locations | ✓ Load | ✓ Maintenance | ✓ Coverage | ✓ Metrics |
| **Bandwidth** | ✓ Allocations | ✓ Bidding | - | ✓ Routing | ✓ Utilization |
| **Marketing** | ✓ Campaigns | ✓ Tracking | ✓ Content | ✓ Segments | ✓ Analytics |
| **Devices** | ✓ Inventory | ✓ Provisioning | ✓ Configs | ✓ SIM links | ✓ Usage |

## Directory Structure

```text
telco/
├── README.md                          # This file
├── sql/                               # PostgreSQL schemas
│   ├── 01_schema_core.sql            # Core entities (subscribers, accounts)
│   ├── 02_schema_billing.sql         # Billing and payments
│   ├── 03_schema_plans.sql           # Plans and features
│   ├── 04_schema_network.sql         # Network infrastructure
│   ├── 05_schema_devices.sql         # Devices and SIM cards
│   ├── 06_schema_marketing.sql       # Marketing and campaigns
│   └── 07_sample_data.sql            # Sample data
├── redis/                             # Redis operations
│   ├── 01_session_tracking.redis     # Active call/data sessions
│   ├── 02_usage_metering.redis       # Real-time usage tracking
│   ├── 03_rate_limiting.redis        # Throttling and limits
│   ├── 04_network_status.redis       # Network alerts and status
│   └── 05_bandwidth_bidding.redis    # Bandwidth auction cache
├── mongodb/                           # MongoDB documents
│   ├── 01_cdr_records.js             # Call Detail Records
│   ├── 02_contracts.js               # Customer contracts
│   ├── 03_network_logs.js            # Network diagnostics
│   └── 04_marketing_content.js       # Marketing assets
├── cypher/                            # Neo4j graphs
│   ├── 01_network_topology.cypher    # Network routing
│   ├── 02_fraud_detection.cypher     # Fraud patterns
│   ├── 03_subscriber_networks.cypher # Social graphs
│   └── 04_coverage_optimization.cypher # Tower coverage
├── cql/                               # Cassandra time-series
│   ├── 01_network_metrics.cql        # Network KPIs
│   ├── 02_usage_analytics.cql        # Usage patterns
│   ├── 03_tower_performance.cql      # Tower metrics
│   └── 04_bandwidth_utilization.cql  # Bandwidth trends
├── python/                            # Python workflows
│   ├── 01_subscriber_onboarding.py   # New subscriber workflow
│   ├── 02_call_routing.py            # Real-time routing
│   ├── 03_billing_engine.py          # Billing calculation
│   ├── 04_network_monitor.py         # Network monitoring
│   └── 05_bandwidth_auction.py       # Bandwidth bidding
├── javascript/                        # JavaScript integration
│   ├── 01_subscriber_portal.js       # Self-service portal
│   ├── 02_usage_dashboard.js         # Usage tracking
│   └── 03_network_dashboard.js       # Network monitoring
└── workflows/                         # Workflow documentation
    ├── 01_subscriber_onboarding.md   # Onboarding process
    ├── 02_call_routing.md            # Call routing workflow
    ├── 03_billing_cycle.md           # Billing process
    ├── 04_network_optimization.md    # Network optimization
    ├── 05_bandwidth_auction.md       # Bandwidth bidding
    └── 03_add_line.md                # Add a Line (Postpaid)

```

## Quick Start

### 1. Set Up Schemas

```bash
# Create PostgreSQL schemas
psql -h localhost -p 5432 -U orbit -d telco < sql/01_schema_core.sql
psql -h localhost -p 5432 -U orbit -d telco < sql/02_schema_billing.sql
psql -h localhost -p 5432 -U orbit -d telco < sql/03_schema_plans.sql
psql -h localhost -p 5432 -U orbit -d telco < sql/04_schema_network.sql
psql -h localhost -p 5432 -U orbit -d telco < sql/05_schema_devices.sql
psql -h localhost -p 5432 -U orbit -d telco < sql/06_schema_marketing.sql

# Load sample data
psql -h localhost -p 5432 -U orbit -d telco < sql/07_sample_data.sql
```

### 2. Initialize Redis

```bash
redis-cli -h localhost -p 6379 < redis/01_session_tracking.redis
redis-cli -h localhost -p 6379 < redis/02_usage_metering.redis
```

### 3. Set Up MongoDB

```bash
mongosh mongodb://localhost:27017/telco mongodb/01_cdr_records.js
mongosh mongodb://localhost:27017/telco mongodb/02_contracts.js
```

### 4. Create Neo4j Graph

```bash
cypher-shell -a bolt://localhost:7687 < cypher/01_network_topology.cypher
```

## Key Use Cases

### 1. Subscriber Onboarding

**Workflow**: New customer signs up for mobile service

**Protocols Used**:
- **PostgreSQL**: Create subscriber account, assign plan
- **Redis**: Cache subscriber profile, activate session
- **MongoDB**: Store contract documents
- **Neo4j**: Create subscriber node, link to referrer
- **Cassandra**: Initialize usage tracking

**Example**: See `workflows/01_subscriber_onboarding.md`

### 2. Real-Time Call Routing

**Workflow**: Route incoming call to subscriber

**Protocols Used**:
- **PostgreSQL**: Lookup subscriber, verify plan
- **Redis**: Check active sessions, rate limits
- **Neo4j**: Find optimal network path
- **Cassandra**: Log call metrics

**Performance**: <50ms routing decision

### 3. Billing Cycle Processing

**Workflow**: Monthly billing calculation and invoice generation

**Protocols Used**:
- **PostgreSQL**: Generate invoices, record charges
- **Redis**: Cache billing calculations
- **MongoDB**: Store invoice PDFs
- **Cassandra**: Aggregate usage data

**Scale**: Process 10M+ subscribers/hour

### 4. Bandwidth Auction

**Workflow**: Real-time bidding for network bandwidth

**Protocols Used**:
- **PostgreSQL**: Record allocations
- **Redis**: Real-time bidding, price discovery
- **Neo4j**: Network topology for routing
- **Cassandra**: Bandwidth utilization history

**Latency**: <10ms bid processing

### 5. Add a Line (Postpaid)

**Workflow**: Existing customer adds a new line to their account

**Protocols Used**:
- **SQL**: Eligibility check and billing update
- **Redis**: Number allocation and reservation
- **MongoDB**: SIM provisioning
- **Redis/CQL**: HLR/HSS activation

**Example**: See `workflows/03_add_line.md`

## Data Models

### Subscriber (PostgreSQL)

```sql
CREATE TABLE subscribers (
    subscriber_id UUID PRIMARY KEY,
    msisdn VARCHAR(15) UNIQUE,  -- Phone number
    imsi VARCHAR(15),            -- International Mobile Subscriber Identity
    status VARCHAR(20),          -- ACTIVE, SUSPENDED, TERMINATED
    plan_id UUID,
    activation_date DATE,
    credit_class VARCHAR(10)     -- POSTPAID, PREPAID
);
```

### Active Session (Redis)

```redis
HSET session:voice:+14155551234 {
    "session_id": "sess-abc123",
    "subscriber_id": "sub-001",
    "start_time": "2024-12-06T18:48:00Z",
    "cell_tower_id": "tower-sf-001",
    "codec": "AMR-WB"
}
EXPIRE session:voice:+14155551234 7200  # 2 hour TTL
```

### Call Detail Record (MongoDB)

```javascript
{
    _id: ObjectId(),
    call_id: "call-20241206-001",
    msisdn: "+14155551234",
    called_number: "+14155559999",
    call_type: "VOICE",
    start_time: ISODate("2024-12-06T18:48:00Z"),
    duration_seconds: 300,
    cell_tower_id: "tower-sf-001",
    data_usage_mb: 0,
    charges: 0.15
}
```

### Network Topology (Neo4j)

```cypher
CREATE (tower:CellTower {
    tower_id: 'tower-sf-001',
    location: point({latitude: 37.7749, longitude: -122.4194}),
    technology: '5G',
    capacity_gbps: 10
})

CREATE (bts:BaseStation {station_id: 'bts-001'})
CREATE (tower)-[:CONNECTS_TO]->(bts)
```

### Network Metrics (Cassandra)

```cql
CREATE TABLE network_metrics (
    tower_id TEXT,
    metric_time TIMESTAMP,
    bandwidth_used_mbps DECIMAL,
    active_connections INT,
    latency_ms DECIMAL,
    packet_loss_pct DECIMAL,
    PRIMARY KEY (tower_id, metric_time)
) WITH CLUSTERING ORDER BY (metric_time DESC);
```

## Performance Benchmarks

| Operation | Latency | Throughput |
|-----------|---------|------------|
| Subscriber Lookup | <5ms | 100K/sec |
| Call Routing Decision | <50ms | 50K/sec |
| Usage Metering Update | <10ms | 500K/sec |
| CDR Write | <20ms | 100K/sec |
| Billing Calculation | <100ms | 10K/sec |
| Network Metric Insert | <5ms | 1M/sec |
| Bandwidth Bid | <10ms | 50K/sec |

## Running Examples

### Python Workflow - Subscriber Onboarding

```bash
cd python
python3 01_subscriber_onboarding.py \
    --msisdn +14155551234 \
    --plan premium-unlimited \
    --payment-method credit-card
```

### JavaScript Dashboard - Network Monitoring

```bash
cd javascript
node 03_network_dashboard.js --port 3000
```

## Testing

### Integration Tests

```bash
# Run all integration tests
pytest tests/integration/

# Test specific workflow
pytest tests/integration/test_subscriber_onboarding.py
```

### Performance Tests

```bash
# Load test call routing
python tests/performance/test_call_routing.py --calls 100000
```

## Contributing

See main [CONTRIBUTING.md](../../CONTRIBUTING.md) for guidelines.

## License

See main [LICENSE](../../LICENSE) file.
