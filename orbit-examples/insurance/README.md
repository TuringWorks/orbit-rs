# OrbitRS Insurance Industry Examples

## Overview

This directory contains comprehensive, real-world insurance industry examples demonstrating OrbitRS's multi-protocol capabilities across various insurance domains:

- **Auto Insurance** - Vehicle coverage, driver risk, claims processing
- **Home Insurance** - Property coverage, natural disasters, valuations
- **Life Insurance** - Policies, beneficiaries, actuarial calculations
- **Property Insurance** - Commercial real estate, liability coverage
- **Disaster Insurance** - Catastrophic events, emergency response
- **Earthquake Insurance** - Seismic risk assessment, structural coverage
- **Industrial Insurance** - Manufacturing facilities, equipment, workers comp

## Architecture

```text
┌────────────────────────────────────────────────────────────────┐
│                  Insurance Platform Architecture               │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │  PostgreSQL  │  │    Redis     │  │   MongoDB    │          │
│  │              │  │              │  │              │          │
│  │ • Policies   │  │ • Quotes     │  │ • Documents  │          │
│  │ • Claims     │  │ • Sessions   │  │ • Reports    │          │
│  │ • Customers  │  │ • Risk Cache │  │ • Images     │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│                                                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │    Neo4j     │  │  Cassandra   │  │   REST API   │          │
│  │              │  │              │  │              │          │
│  │ • Fraud Net  │  │ • Time Series│  │ • Customer   │          │
│  │ • Relations  │  │ • Premiums   │  │ • Agent      │          │
│  │ • GraphRAG   │  │ • Analytics  │  │ • Claims     │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

## Protocol Usage by Domain

| Domain | PostgreSQL | Redis | MongoDB | Neo4j | Cassandra | REST |
|--------|-----------|-------|---------|-------|-----------|------|
| **Auto** | ✅ Policies | ✅ Quotes | ✅ Photos | ✅ Fraud | ✅ Telematics | ✅ API |
| **Home** | ✅ Coverage | ✅ Risk | ✅ Inspections | ✅ Claims | ✅ Weather | ✅ API |
| **Life** | ✅ Policies | ✅ Calc | ✅ Medical | ✅ Beneficiaries | ✅ Premiums | ✅ API |
| **Property** | ✅ Assets | ✅ Valuations | ✅ Docs | ✅ Ownership | ✅ Market | ✅ API |
| **Disaster** | ✅ Events | ✅ Alerts | ✅ Reports | ✅ Impact | ✅ History | ✅ API |
| **Earthquake** | ✅ Zones | ✅ Risk | ✅ Assessments | ✅ Structures | ✅ Seismic | ✅ API |
| **Industrial** | ✅ Facilities | ✅ Safety | ✅ Incidents | ✅ Supply Chain | ✅ Metrics | ✅ API |

## Directory Structure

```text
insurance/
├── README.md                         # This file
├── ARCHITECTURE.md                   # Detailed architecture
│
├── sql/                              # PostgreSQL schemas & queries
│   ├── 01_schema_core.sql            # Core insurance tables
│   ├── 02_schema_auto.sql            # Auto insurance
│   ├── 03_schema_home.sql            # Home insurance
│   ├── 04_schema_life.sql            # Life insurance
│   ├── 05_schema_property.sql        # Property insurance
│   ├── 06_schema_disaster.sql        # Disaster insurance
│   ├── 07_schema_earthquake.sql      # Earthquake insurance
│   ├── 08_schema_industrial.sql      # Industrial insurance
│   └── 09_sample_data.sql            # Sample data inserts
│
├── redis/                            # Redis commands & caching
│   ├── 01_quote_caching.redis        # Quote generation cache
│   ├── 02_risk_scoring.redis         # Real-time risk scores
│   ├── 03_session_management.redis   # Agent sessions
│   ├── 04_claims_queue.redis         # Claims processing queue
│   └── 05_premium_calc.redis         # Premium calculations
│
├── mongodb/                          # Document storage
│   ├── 01_policy_documents.js        # Policy PDFs, images
│   ├── 02_claims_documentation.js    # Claims photos, reports
│   ├── 03_inspection_reports.js      # Property inspections
│   └── 04_customer_communications.js # Emails, notes
│
├── cypher/                           # Neo4j graph queries
│   ├── 01_fraud_detection.cypher     # Fraud pattern detection
│   ├── 02_customer_networks.cypher   # Customer relationships
│   ├── 03_claims_investigation.cypher # Claims investigation
│   └── 04_agent_performance.cypher   # Agent networks
│
├── cql/                               # Cassandra time-series
│   ├── 01_premium_history.cql        # Premium payment tracking
│   ├── 02_claims_trends.cql          # Claims frequency analysis
│   ├── 03_risk_evolution.cql         # Risk score changes
│   └── 04_market_rates.cql           # Market rate tracking
│
├── rest/                              # REST API examples
│   ├── openapi.yaml                  # API specification
│   └── examples.http                 # HTTP request examples
│
├── python/                           # Python integration examples
│   ├── 01_policy_creation.py         # End-to-end policy workflow
│   ├── 02_claims_processing.py       # Claims pipeline
│   ├── 03_risk_assessment.py         # Risk scoring system
│   ├── 04_fraud_detection.py         # Fraud detection ML
│   └── 05_premium_optimization.py    # Premium calculation
│
├── javascript/                       # JavaScript examples
│   ├── 01_customer_portal.js         # Customer self-service
│   ├── 02_agent_dashboard.js         # Agent interface
│   └── 03_claims_submission.js       # Claims submission
│
└── workflows/                        # Cross-protocol workflows
    ├── 01_auto_policy_creation.md    # Auto insurance workflow
    ├── 02_home_claims_process.md     # Home claims workflow
    ├── 03_life_underwriting.md       # Life underwriting
    └── 04_disaster_response.md       # Disaster response
```

## Quick Start

### 1. Initialize Database Schemas

```bash
# Connect to OrbitRS PostgreSQL
psql -h localhost -p 5432 -U orbit -d insurance

# Run schema creation
\i sql/01_schema_core.sql
\i sql/02_schema_auto.sql
\i sql/03_schema_home.sql
# ... continue for all schemas

# Load sample data
\i sql/09_sample_data.sql
```

### 2. Set Up Redis Caching

```bash
# Connect to OrbitRS Redis
redis-cli -h localhost -p 6379

# Load quote caching examples
< redis/01_quote_caching.redis
```

### 3. Initialize MongoDB Collections

```bash
# Connect to OrbitRS MongoDB
mongosh mongodb://localhost:27017/insurance

# Load document examples
load('mongodb/01_policy_documents.js')
```

### 4. Create Graph Relationships

```bash
# Connect to OrbitRS Neo4j
cypher-shell -a bolt://localhost:7687

# Load fraud detection patterns
:source cypher/01_fraud_detection.cypher
```

## Use Case Examples

### Auto Insurance: New Policy Creation

This workflow demonstrates creating an auto insurance policy using multiple protocols:

1. **PostgreSQL**: Store policy, vehicle, and driver data
2. **Redis**: Cache risk score and premium quote
3. **MongoDB**: Store vehicle photos and documents
4. **Neo4j**: Create customer-vehicle-policy relationships
5. **Cassandra**: Initialize premium payment schedule

See: `workflows/01_auto_policy_creation.md`

### Home Insurance: Claims Processing

Complete claims workflow from submission to settlement:

1. **REST API**: Customer submits claim via mobile app
2. **Redis**: Queue claim for processing
3. **MongoDB**: Store damage photos and adjuster reports
4. **PostgreSQL**: Update claim status and payments
5. **Neo4j**: Fraud detection analysis
6. **Cassandra**: Track claim timeline

See: `workflows/02_home_claims_process.md`

### Disaster Insurance: Catastrophic Event Response

Real-time disaster response workflow:

1. **Redis**: Real-time event alerts and affected policies
2. **PostgreSQL**: Identify impacted customers and policies
3. **Neo4j**: Analyze geographic impact networks
4. **MongoDB**: Store damage assessments and satellite imagery
5. **Cassandra**: Track historical disaster patterns
6. **REST API**: Emergency claim submission

See: `workflows/04_disaster_response.md`

## Key Features Demonstrated

### 1. Multi-Protocol Data Consistency
- Single source of truth across all protocols
- Automatic data synchronization
- Cross-protocol transactions

### 2. Real-Time Operations
- Quote generation (<100ms)
- Risk scoring with Redis
- Claims queue management
- Session state management

### 3. Document Management
- Policy PDFs and contracts
- Claims photos and videos
- Inspection reports
- Customer communications

### 4. Graph Analytics
- Fraud detection networks
- Customer relationship mapping
- Claims investigation
- Agent performance analysis

### 5. Time-Series Analytics
- Premium payment history
- Claims frequency trends
- Risk score evolution
- Market rate analysis

### 6. AI/ML Integration
- Risk prediction models
- Fraud detection algorithms
- Premium optimization
- Claims cost estimation

## Data Models

### Core Entities

```sql
-- Customers (PostgreSQL)
customers (
  customer_id UUID PRIMARY KEY,
  first_name VARCHAR(100),
  last_name VARCHAR(100),
  email VARCHAR(255),
  phone VARCHAR(20),
  date_of_birth DATE,
  credit_score INTEGER,
  created_at TIMESTAMP
)

-- Policies (PostgreSQL)
policies (
  policy_id UUID PRIMARY KEY,
  customer_id UUID REFERENCES customers,
  policy_type VARCHAR(50), -- AUTO, HOME, LIFE, etc.
  policy_number VARCHAR(50) UNIQUE,
  status VARCHAR(20),
  effective_date DATE,
  expiration_date DATE,
  premium_amount DECIMAL(10,2),
  coverage_amount DECIMAL(12,2)
)

-- Claims (PostgreSQL)
claims (
  claim_id UUID PRIMARY KEY,
  policy_id UUID REFERENCES policies,
  claim_number VARCHAR(50) UNIQUE,
  claim_type VARCHAR(50),
  incident_date TIMESTAMP,
  reported_date TIMESTAMP,
  status VARCHAR(20),
  claim_amount DECIMAL(12,2),
  settled_amount DECIMAL(12,2)
)
```

### Redis Cache Patterns

```redis
# Quote Cache (TTL: 30 minutes)
quote:{customer_id}:{vehicle_vin} -> JSON

# Risk Score Cache (TTL: 1 hour)
risk:score:{customer_id} -> INTEGER

# Premium Calculation Cache (TTL: 15 minutes)
premium:calc:{policy_type}:{risk_factors_hash} -> DECIMAL

# Session Management (TTL: 8 hours)
session:agent:{agent_id} -> JSON
```

### MongoDB Document Structure

```javascript
// Policy Documents Collection
{
  _id: ObjectId,
  policy_id: UUID,
  document_type: "policy_contract" | "declaration" | "endorsement",
  file_name: String,
  file_size: Number,
  mime_type: String,
  storage_url: String,
  uploaded_at: ISODate,
  uploaded_by: UUID
}

// Claims Documentation Collection
{
  _id: ObjectId,
  claim_id: UUID,
  document_type: "photo" | "video" | "report" | "estimate",
  description: String,
  file_data: BinData, // or storage_url
  metadata: {
    location: GeoJSON,
    timestamp: ISODate,
    device: String
  }
}
```

### Neo4j Graph Model

```cypher
// Customer Node
(:Customer {
  customer_id: UUID,
  name: String,
  risk_score: Integer
})

// Policy Node
(:Policy {
  policy_id: UUID,
  policy_number: String,
  type: String
})

// Claim Node
(:Claim {
  claim_id: UUID,
  claim_number: String,
  amount: Decimal
})

// Relationships
(:Customer)-[:HAS_POLICY]->(:Policy)
(:Policy)-[:HAS_CLAIM]->(:Claim)
(:Customer)-[:RELATED_TO {relationship: String}]->(:Customer)
(:Claim)-[:SIMILAR_TO {similarity_score: Float}]->(:Claim)
```

### Cassandra Time-Series Schema

```cql
-- Premium Payment History
CREATE TABLE premium_payments (
  policy_id UUID,
  payment_date TIMESTAMP,
  payment_id UUID,
  amount DECIMAL,
  payment_method VARCHAR,
  status VARCHAR,
  PRIMARY KEY (policy_id, payment_date)
) WITH CLUSTERING ORDER BY (payment_date DESC);

-- Claims Frequency Trends
CREATE TABLE claims_frequency (
  region VARCHAR,
  claim_type VARCHAR,
  time_bucket TIMESTAMP,
  claim_count COUNTER,
  total_amount DECIMAL,
  PRIMARY KEY ((region, claim_type), time_bucket)
) WITH CLUSTERING ORDER BY (time_bucket DESC);
```

## Performance Benchmarks

| Operation | Protocol | Latency | Throughput |
|-----------|----------|---------|------------|
| Quote Generation | Redis | <50ms | 10,000/sec |
| Policy Creation | PostgreSQL | <200ms | 1,000/sec |
| Claims Submission | REST → Multi | <500ms | 500/sec |
| Fraud Detection | Neo4j | <1s | 100/sec |
| Document Upload | MongoDB | <2s | 200/sec |
| Premium History Query | Cassandra | <100ms | 5,000/sec |

## Testing

### Run Integration Tests

```bash
# Python tests
cd python
pytest test_insurance_workflows.py -v

# JavaScript tests
cd javascript
npm test
```

### Manual Testing Workflows

See individual workflow files in `workflows/` directory for step-by-step testing instructions.

## Contributing

When adding new insurance examples:

1. Follow the existing directory structure
2. Use consistent naming conventions
3. Include sample data
4. Document cross-protocol workflows
5. Add integration tests
6. Update this README

## License

Part of the OrbitRS project - see main repository LICENSE

## Support

For questions or issues:
- OrbitRS Documentation: `../../docs/`
- GitHub Issues: https://github.com/TuringWorks/orbit-rs/issues
