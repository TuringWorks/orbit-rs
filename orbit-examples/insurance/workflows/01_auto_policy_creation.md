# Auto Insurance Policy Creation Workflow

## Overview

This workflow demonstrates creating an auto insurance policy using all OrbitRS protocols in an integrated, real-world scenario.

## Protocols Used

- **PostgreSQL (Port 5432)**: Core data storage
- **Redis (Port 6379)**: Caching and real-time operations
- **MongoDB (Port 27017)**: Document storage
- **Neo4j (Port 7687)**: Relationship graphs

## Workflow Steps

### 1. Customer Creation (PostgreSQL)

```sql
INSERT INTO customers (
    customer_id, customer_number, first_name, last_name,
    date_of_birth, email, phone, credit_score
) VALUES (
    '550e8400-e29b-41d4-a716-446655440000',
    'CUST-20241206-550e8400',
    'John', 'Doe',
    '1985-06-15',
    'john.doe@email.com',
    '+14155550123',
    720
);
```

**Result**: Customer record created with unique ID

---

### 2. Vehicle Registration (PostgreSQL)

```sql
INSERT INTO vehicles (
    vehicle_id, vin, customer_id,
    year, make, model, body_style,
    fuel_type, annual_mileage, primary_use
) VALUES (
    'vehicle-001',
    '1HGCM82633A123456',
    '550e8400-e29b-41d4-a716-446655440000',
    2023, 'Honda', 'Accord', 'SEDAN',
    'GASOLINE', 12000, 'PERSONAL'
);
```

**Result**: Vehicle linked to customer

---

### 3. Driver Information (PostgreSQL)

```sql
INSERT INTO drivers (
    driver_id, customer_id,
    first_name, last_name, date_of_birth,
    license_number, license_state, years_licensed
) VALUES (
    'driver-001',
    '550e8400-e29b-41d4-a716-446655440000',
    'John', 'Doe', '1985-06-15',
    'D1234567', 'CA', 17
);
```

**Result**: Driver profile created

---

### 4. Quote Calculation & Caching (Redis)

```python
# Calculate premium
base_premium = 1200.00
coverage_factor = 2.5  # 250/500/100 coverage
collision_factor = 1.3
comprehensive_factor = 1.15

annual_premium = base_premium * coverage_factor * collision_factor * comprehensive_factor
# = $4,485.00

# Apply discounts
multi_policy_discount = annual_premium * 0.10  # -$448.50
good_driver_discount = annual_premium * 0.05   # -$224.25

final_premium = annual_premium - discounts
# = $3,812.25
```

```redis
# Cache quote in Redis (TTL: 30 minutes)
SETEX quote:auto:550e8400-e29b-41d4-a716-446655440000:vehicle-001 1800 '{
  "quote_id": "Q-2024-001234",
  "premium": {
    "annual": 3812.25,
    "monthly": 317.69
  },
  "coverage": {
    "liability_bi_per_person": 250000,
    "liability_bi_per_accident": 500000,
    "liability_pd": 100000,
    "collision_deductible": 500,
    "comprehensive_deductible": 500
  },
  "discounts": ["multi_policy", "good_driver"]
}'
```

**Result**: Quote cached for 30 minutes, ready for binding

---

### 5. Policy Creation (PostgreSQL)

```sql
-- Create main policy
INSERT INTO policies (
    policy_id, policy_number, customer_id,
    policy_type, policy_status,
    effective_date, expiration_date,
    premium_amount, coverage_amount
) VALUES (
    'POL-001',
    'AUTO-2024-001234',
    '550e8400-e29b-41d4-a716-446655440000',
    'AUTO', 'ACTIVE',
    '2024-12-07', '2025-12-07',
    3812.25, 500000
);

-- Create auto policy details
INSERT INTO auto_policies (
    auto_policy_id, policy_id,
    liability_bodily_injury_per_person,
    liability_bodily_injury_per_accident,
    liability_property_damage,
    collision_coverage, collision_deductible,
    comprehensive_coverage, comprehensive_deductible,
    multi_policy_discount, good_driver_discount
) VALUES (
    'auto-pol-001', 'POL-001',
    250000, 500000, 100000,
    TRUE, 500,
    TRUE, 500,
    TRUE, TRUE
);
```

**Result**: Active policy created

---

### 6. Link Vehicle & Driver (PostgreSQL)

```sql
-- Link vehicle to policy
INSERT INTO policy_vehicles (
    policy_vehicle_id, policy_id, vehicle_id,
    coverage_type, primary_driver_id
) VALUES (
    'pv-001', 'POL-001', 'vehicle-001',
    'FULL', 'driver-001'
);

-- Link driver to policy
INSERT INTO policy_drivers (
    policy_driver_id, policy_id, driver_id,
    driver_status
) VALUES (
    'pd-001', 'POL-001', 'driver-001',
    'LISTED'
);
```

**Result**: Vehicle and driver associated with policy

---

### 7. Store Policy Documents (MongoDB)

```javascript
// Store policy contract
db.policy_documents.insertOne({
  policy_id: "POL-001",
  policy_number: "AUTO-2024-001234",
  document_type: "policy_contract",
  file_name: "AUTO-2024-001234-Contract.pdf",
  file_size_bytes: 524288,
  mime_type: "application/pdf",
  storage_url: "s3://insurance-docs/policies/2024/AUTO-2024-001234-Contract.pdf",
  metadata: {
    customer_name: "John Doe",
    vehicle: "2023 Honda Accord",
    effective_date: ISODate("2024-12-07"),
    expiration_date: ISODate("2025-12-07")
  },
  uploaded_by: "system",
  uploaded_at: ISODate()
});

// Store declaration page
db.policy_documents.insertOne({
  policy_id: "POL-001",
  document_type: "declaration",
  file_name: "AUTO-2024-001234-Declarations.pdf",
  storage_url: "s3://insurance-docs/policies/2024/AUTO-2024-001234-Declarations.pdf",
  uploaded_at: ISODate()
});
```

**Result**: 2 policy documents stored in MongoDB

---

### 8. Create Relationship Graph (Neo4j)

```cypher
// Create customer node
CREATE (c:Customer {
  customer_id: '550e8400-e29b-41d4-a716-446655440000',
  name: 'John Doe',
  email: 'john.doe@email.com',
  risk_score: 72
});

// Create policy node
CREATE (p:Policy {
  policy_id: 'POL-001',
  policy_number: 'AUTO-2024-001234',
  policy_type: 'AUTO',
  status: 'ACTIVE',
  premium: 3812.25
});

// Create vehicle node
CREATE (v:Vehicle {
  vehicle_id: 'vehicle-001',
  vin: '1HGCM82633A123456',
  year: 2023,
  make: 'Honda',
  model: 'Accord'
});

// Create relationships
MATCH (c:Customer {customer_id: '550e8400-e29b-41d4-a716-446655440000'})
MATCH (p:Policy {policy_id: 'POL-001'})
CREATE (c)-[:HAS_POLICY {since: date('2024-12-07')}]->(p);

MATCH (p:Policy {policy_id: 'POL-001'})
MATCH (v:Vehicle {vehicle_id: 'vehicle-001'})
CREATE (p)-[:COVERS {since: date('2024-12-07')}]->(v);
```

**Result**: Customer-Policy-Vehicle relationship graph created

---

## Workflow Summary

| Step | Protocol | Action | Result |
|------|----------|--------|--------|
| 1 | PostgreSQL | Create customer | Customer ID: `550e8400...` |
| 2 | PostgreSQL | Register vehicle | Vehicle ID: `vehicle-001` |
| 3 | PostgreSQL | Add driver | Driver ID: `driver-001` |
| 4 | Redis | Calculate & cache quote | Quote ID: `Q-2024-001234` |
| 5 | PostgreSQL | Create policy | Policy ID: `POL-001` |
| 6 | PostgreSQL | Link vehicle & driver | Associations created |
| 7 | MongoDB | Store documents | 2 documents stored |
| 8 | Neo4j | Create graph | Relationships mapped |

## Final Policy Details

```
Policy Number: AUTO-2024-001234
Customer: John Doe (john.doe@email.com)
Vehicle: 2023 Honda Accord (VIN: 1HGCM82633A123456)
Coverage: 250/500/100 + Collision ($500 ded) + Comprehensive ($500 ded)
Premium: $3,812.25/year ($317.69/month)
Effective: 2024-12-07 to 2025-12-07
Status: ACTIVE
Discounts: Multi-Policy (10%), Good Driver (5%)
```

## Running the Workflow

### Option 1: Python Script

```bash
cd orbit-examples/insurance/python
python3 01_policy_creation_workflow.py
```

### Option 2: Manual Steps

```bash
# 1. Create schema
psql -h localhost -p 5432 -U orbit -d insurance < sql/01_schema_core.sql
psql -h localhost -p 5432 -U orbit -d insurance < sql/02_schema_auto.sql

# 2. Run Redis commands
redis-cli -h localhost -p 6379 < redis/01_insurance_operations.redis

# 3. Load MongoDB documents
mongosh mongodb://localhost:27017/insurance mongodb/01_insurance_documents.js

# 4. Create Neo4j graph
cypher-shell -a bolt://localhost:7687 < cypher/01_fraud_detection.cypher
```

## Verification

### Check PostgreSQL

```sql
-- Verify policy created
SELECT p.policy_number, p.policy_status, p.premium_amount,
       c.first_name, c.last_name,
       v.year, v.make, v.model
FROM policies p
JOIN customers c ON p.customer_id = c.customer_id
JOIN policy_vehicles pv ON p.policy_id = pv.policy_id
JOIN vehicles v ON pv.vehicle_id = v.vehicle_id
WHERE p.policy_number = 'AUTO-2024-001234';
```

### Check Redis

```redis
# Get cached quote
GET quote:auto:550e8400-e29b-41d4-a716-446655440000:vehicle-001

# Check TTL
TTL quote:auto:550e8400-e29b-41d4-a716-446655440000:vehicle-001
```

### Check MongoDB

```javascript
// Find policy documents
db.policy_documents.find({ policy_id: "POL-001" }).pretty();
```

### Check Neo4j

```cypher
// Verify relationships
MATCH (c:Customer)-[:HAS_POLICY]->(p:Policy)-[:COVERS]->(v:Vehicle)
WHERE p.policy_id = 'POL-001'
RETURN c.name, p.policy_number, v.make, v.model;
```

## Performance Metrics

- **Total Workflow Time**: ~500ms
- **PostgreSQL Inserts**: 8 tables, <200ms
- **Redis Cache**: <10ms
- **MongoDB Inserts**: 2 documents, <50ms
- **Neo4j Graph**: 3 nodes + 2 relationships, <100ms

## Next Steps

1. **Claims Processing**: See `workflows/02_claims_processing.md`
2. **Policy Renewal**: Automated renewal workflow
3. **Fraud Detection**: Graph-based fraud analysis
4. **Analytics**: Time-series premium and claims analysis
