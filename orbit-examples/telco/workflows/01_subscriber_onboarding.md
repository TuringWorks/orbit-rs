# Telecommunications Workflow: Subscriber Onboarding

## Overview

End-to-end subscriber onboarding with network provisioning, billing setup, and real-time activation.

## Workflow Steps

### 1. Customer Registration (PostgreSQL)
```sql
-- Create customer account
INSERT INTO customers (customer_id, first_name, last_name, email, phone, address)
VALUES (uuid_generate_v4(), 'John', 'Doe', 'john@email.com', '555-0123', '123 Main St');
```

### 2. Credit Check & KYC (PostgreSQL + ML)
```sql
-- Verify identity
SELECT verification_status
FROM identity_verification
WHERE ssn_encrypted = encrypt_ssn('123-45-6789');
```

```redis
# ML credit scoring
GET ml:credit:score:customer-123
# Returns: Credit score, approval recommendation
```

### 3. Plan Selection (PostgreSQL)
```sql
-- Available plans
SELECT plan_id, plan_name, data_gb, price_monthly
FROM service_plans
WHERE status = 'ACTIVE' AND plan_type = 'POSTPAID';

-- Create subscription
INSERT INTO subscriptions (subscription_id, customer_id, plan_id, status)
VALUES (uuid_generate_v4(), 'customer-123', 'plan-unlimited', 'PENDING_ACTIVATION');
```

### 4. Phone Number Assignment (PostgreSQL + Redis)
```sql
-- Get available numbers
SELECT phone_number
FROM phone_inventory
WHERE status = 'AVAILABLE' AND area_code = '555'
LIMIT 10;

-- Assign number
UPDATE phone_inventory
SET status = 'ASSIGNED', customer_id = 'customer-123'
WHERE phone_number = '555-0123';
```

```redis
# Cache number assignment
HSET number:555-0123 customer_id "customer-123"
HSET number:555-0123 status "ASSIGNED"
```

### 5. Network Provisioning (PostgreSQL + Redis)
```sql
-- Provision SIM card
INSERT INTO sim_cards (sim_id, iccid, customer_id, phone_number, status)
VALUES (uuid_generate_v4(), '89014103211118510720', 'customer-123', '555-0123', 'ACTIVE');

-- Network profile
INSERT INTO network_profiles (profile_id, customer_id, apn, data_limit_gb)
VALUES (uuid_generate_v4(), 'customer-123', 'internet.carrier.com', 50);
```

```redis
# Real-time network activation
HSET network:555-0123 status "ACTIVE"
HSET network:555-0123 apn "internet.carrier.com"
HSET network:555-0123 data_limit_gb "50"
HSET network:555-0123 data_used_gb "0"

# Publish activation event
PUBLISH network:activation '{
  "phone_number": "555-0123",
  "customer_id": "customer-123",
  "timestamp": "2024-12-07T17:50:00Z"
}'
```

### 6. Billing Setup (PostgreSQL)
```sql
-- Create billing account
INSERT INTO billing_accounts (account_id, customer_id, billing_cycle_day, payment_method)
VALUES (uuid_generate_v4(), 'customer-123', 15, 'AUTO_PAY');

-- First invoice
INSERT INTO invoices (invoice_id, account_id, amount, due_date, status)
VALUES (uuid_generate_v4(), 'account-uuid', 75.00, CURRENT_DATE + INTERVAL '30 days', 'PENDING');
```

### 7. Real-Time Usage Tracking (Redis + Cassandra)
```redis
# Initialize usage counters
SET usage:555-0123:voice_minutes 0
SET usage:555-0123:sms_count 0
SET usage:555-0123:data_mb 0
EXPIRE usage:555-0123:voice_minutes 2592000  # 30 days
```

```cql
-- Store usage events
CREATE TABLE IF NOT EXISTS usage_events (
  phone_number TEXT,
  timestamp TIMESTAMP,
  event_type TEXT,
  duration INT,
  data_mb DECIMAL,
  PRIMARY KEY (phone_number, timestamp)
) WITH CLUSTERING ORDER BY (timestamp DESC);
```

### 8. Network Monitoring (Redis + Cassandra)
```redis
# Real-time network status
HSET network:tower:tower-001 status "OPERATIONAL"
HSET network:tower:tower-001 capacity "85"
HSET network:tower:tower-001 connected_devices "1250"

# Customer connection
GEOADD network:connections -122.4194 37.7749 "555-0123"
```

```cql
-- Network performance metrics
INSERT INTO network_metrics (tower_id, timestamp, signal_strength, throughput_mbps)
VALUES ('tower-001', now(), -65, 125.5);
```

### 9. Welcome Notification (Redis Pub/Sub)
```redis
PUBLISH notifications:customer-123 '{
  "type": "WELCOME",
  "phone_number": "555-0123",
  "plan": "Unlimited Data",
  "activation_date": "2024-12-07"
}'

# Send SMS via gateway
LPUSH sms:outbound '{
  "to": "555-0123",
  "message": "Welcome to Carrier! Your service is now active."
}'
```

### 10. Customer Portal Access (PostgreSQL + Redis)
```sql
-- Create portal account
INSERT INTO portal_accounts (account_id, customer_id, username, password_hash)
VALUES (uuid_generate_v4(), 'customer-123', 'johndoe', hash_password('password'));
```

```redis
# Session management
SETEX session:customer-123 3600 '{
  "customer_id": "customer-123",
  "phone_number": "555-0123",
  "plan": "Unlimited Data"
}'
```

### 11. Usage Alerts Setup (Redis)
```redis
# Configure usage alerts
HSET alerts:555-0123 data_threshold_gb "45"  # Alert at 90%
HSET alerts:555-0123 voice_threshold_minutes "900"
HSET alerts:555-0123 alert_email "john@email.com"
```

### 12. Audit Trail (PostgreSQL)
```sql
-- Log all onboarding steps
INSERT INTO audit_log (log_id, customer_id, action, details, timestamp)
VALUES
  (uuid_generate_v4(), 'customer-123', 'CUSTOMER_CREATED', '{"email": "john@email.com"}', CURRENT_TIMESTAMP),
  (uuid_generate_v4(), 'customer-123', 'NUMBER_ASSIGNED', '{"phone": "555-0123"}', CURRENT_TIMESTAMP),
  (uuid_generate_v4(), 'customer-123', 'NETWORK_ACTIVATED', '{"sim": "89014103211118510720"}', CURRENT_TIMESTAMP),
  (uuid_generate_v4(), 'customer-123', 'BILLING_SETUP', '{"plan": "Unlimited Data"}', CURRENT_TIMESTAMP);
```

## Performance Metrics

| Step | Protocol | Latency |
|------|----------|---------|
| Customer Registration | PostgreSQL | <20ms |
| Credit Check | PostgreSQL + ML | <50ms |
| Number Assignment | PostgreSQL + Redis | <30ms |
| Network Provisioning | Redis | <10ms |
| Billing Setup | PostgreSQL | <40ms |
| **Total Onboarding** | **Multi-protocol** | **<200ms** |

## Real-Time Capabilities

### Usage Tracking
```redis
# Increment usage in real-time
INCRBY usage:555-0123:voice_minutes 5
INCRBY usage:555-0123:sms_count 1
INCRBYFLOAT usage:555-0123:data_mb 125.5

# Check if threshold exceeded
GET usage:555-0123:data_mb
# If > threshold, send alert
```

### Network Performance
```cql
-- Query recent performance
SELECT AVG(signal_strength), AVG(throughput_mbps)
FROM network_metrics
WHERE tower_id = 'tower-001'
  AND timestamp > now() - 1h;
```

## Compliance & Security

- **KYC/AML**: Identity verification required
- **PCI DSS**: Payment data encrypted
- **GDPR**: Customer data privacy
- **Audit Trail**: Complete onboarding history
- **Data Encryption**: All sensitive data encrypted at rest

## Integration Points

- **CRM**: Customer data sync
- **Billing System**: Invoice generation
- **Network Management**: Real-time provisioning
- **SMS Gateway**: Welcome messages
- **Email Service**: Notifications
- **Payment Gateway**: Auto-pay setup

**Performance**: <200ms total onboarding time, real-time network activation
