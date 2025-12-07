#!/bin/bash
# =============================================================================
# OrbitRS Insurance Example: REST API for Insurance Operations
# =============================================================================
# Demonstrates OrbitRS REST API usage for insurance management operations.
#
# Prerequisites:
#   - OrbitRS server running on localhost:8080
#   - curl installed
#   - jq installed (for JSON formatting)
#
# Usage:
#   chmod +x 01_insurance_api.sh
#   ./01_insurance_api.sh
# =============================================================================

BASE_URL="http://localhost:8080/api/v1"

echo "============================================="
echo "OrbitRS Insurance REST API Examples"
echo "============================================="
echo ""

# -----------------------------------------------------------------------------
# HEALTH CHECK
# -----------------------------------------------------------------------------
echo "1. Health Check"
echo "---------------"
curl -s "${BASE_URL%/api/v1}/health" | jq .
echo ""

# -----------------------------------------------------------------------------
# SCHEMA SETUP
# -----------------------------------------------------------------------------

echo "2. Create Insurance Schema"
echo "--------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "CREATE SCHEMA IF NOT EXISTS insurance_api"
  }' | jq .
echo ""

echo "3. Create Policies Table"
echo "------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "CREATE TABLE IF NOT EXISTS insurance_api.policies (policy_id VARCHAR(50) PRIMARY KEY, policy_number VARCHAR(30) UNIQUE, policy_type VARCHAR(20), customer_id VARCHAR(50), customer_name VARCHAR(200), effective_date DATE, expiration_date DATE, premium DECIMAL(10,2), coverage_limit DECIMAL(14,2), deductible DECIMAL(10,2), status VARCHAR(20) DEFAULT '"'"'ACTIVE'"'"', created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP)"
  }' | jq .
echo ""

echo "4. Create Claims Table"
echo "----------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "CREATE TABLE IF NOT EXISTS insurance_api.claims (claim_id VARCHAR(50) PRIMARY KEY, claim_number VARCHAR(30) UNIQUE, policy_id VARCHAR(50), loss_date TIMESTAMP, loss_type VARCHAR(50), loss_description TEXT, claimed_amount DECIMAL(12,2), approved_amount DECIMAL(12,2) DEFAULT 0, paid_amount DECIMAL(12,2) DEFAULT 0, status VARCHAR(20) DEFAULT '"'"'OPEN'"'"', adjuster_id VARCHAR(50), created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP)"
  }' | jq .
echo ""

echo "5. Create Customers Table"
echo "-------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "CREATE TABLE IF NOT EXISTS insurance_api.customers (customer_id VARCHAR(50) PRIMARY KEY, first_name VARCHAR(100), last_name VARCHAR(100), email VARCHAR(200) UNIQUE, phone VARCHAR(20), date_of_birth DATE, address_city VARCHAR(100), address_state VARCHAR(2), risk_score INTEGER DEFAULT 50, customer_since DATE DEFAULT CURRENT_DATE)"
  }' | jq .
echo ""

# -----------------------------------------------------------------------------
# INSERT SAMPLE DATA
# -----------------------------------------------------------------------------

echo "6. Insert Sample Customers"
echo "--------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "INSERT INTO insurance_api.customers (customer_id, first_name, last_name, email, phone, date_of_birth, address_city, address_state, risk_score) VALUES ('"'"'CUST-001'"'"', '"'"'John'"'"', '"'"'Smith'"'"', '"'"'john.smith@email.com'"'"', '"'"'+1-555-0101'"'"', '"'"'1985-03-15'"'"', '"'"'New York'"'"', '"'"'NY'"'"', 75), ('"'"'CUST-002'"'"', '"'"'Jane'"'"', '"'"'Doe'"'"', '"'"'jane.doe@email.com'"'"', '"'"'+1-555-0102'"'"', '"'"'1990-07-22'"'"', '"'"'Los Angeles'"'"', '"'"'CA'"'"', 85), ('"'"'CUST-003'"'"', '"'"'Robert'"'"', '"'"'Johnson'"'"', '"'"'robert.j@email.com'"'"', '"'"'+1-555-0103'"'"', '"'"'1978-11-08'"'"', '"'"'Chicago'"'"', '"'"'IL'"'"', 65)"
  }' | jq .
echo ""

echo "7. Insert Sample Policies"
echo "-------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "INSERT INTO insurance_api.policies (policy_id, policy_number, policy_type, customer_id, customer_name, effective_date, expiration_date, premium, coverage_limit, deductible, status) VALUES ('"'"'POL-001'"'"', '"'"'AUTO-2024-0001'"'"', '"'"'AUTO'"'"', '"'"'CUST-001'"'"', '"'"'John Smith'"'"', '"'"'2024-01-01'"'"', '"'"'2025-01-01'"'"', 1200.00, 100000.00, 500.00, '"'"'ACTIVE'"'"'), ('"'"'POL-002'"'"', '"'"'HOME-2024-0001'"'"', '"'"'HOME'"'"', '"'"'CUST-001'"'"', '"'"'John Smith'"'"', '"'"'2024-01-01'"'"', '"'"'2025-01-01'"'"', 1800.00, 500000.00, 1000.00, '"'"'ACTIVE'"'"'), ('"'"'POL-003'"'"', '"'"'LIFE-2024-0001'"'"', '"'"'LIFE'"'"', '"'"'CUST-002'"'"', '"'"'Jane Doe'"'"', '"'"'2024-02-01'"'"', '"'"'2044-02-01'"'"', 850.00, 1000000.00, 0.00, '"'"'ACTIVE'"'"'), ('"'"'POL-004'"'"', '"'"'AUTO-2024-0002'"'"', '"'"'AUTO'"'"', '"'"'CUST-003'"'"', '"'"'Robert Johnson'"'"', '"'"'2024-03-01'"'"', '"'"'2025-03-01'"'"', 1450.00, 150000.00, 750.00, '"'"'ACTIVE'"'"')"
  }' | jq .
echo ""

echo "8. Insert Sample Claims"
echo "-----------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "INSERT INTO insurance_api.claims (claim_id, claim_number, policy_id, loss_date, loss_type, loss_description, claimed_amount, approved_amount, paid_amount, status, adjuster_id) VALUES ('"'"'CLM-001'"'"', '"'"'CLM-2024-0001'"'"', '"'"'POL-001'"'"', '"'"'2024-06-15 14:30:00'"'"', '"'"'COLLISION'"'"', '"'"'Rear-end collision at traffic light'"'"', 3500.00, 3200.00, 3200.00, '"'"'CLOSED'"'"', '"'"'ADJ-001'"'"'), ('"'"'CLM-002'"'"', '"'"'CLM-2024-0002'"'"', '"'"'POL-002'"'"', '"'"'2024-07-20 08:15:00'"'"', '"'"'WATER_DAMAGE'"'"', '"'"'Pipe burst in bathroom'"'"', 8500.00, 7500.00, 0.00, '"'"'APPROVED'"'"', '"'"'ADJ-002'"'"'), ('"'"'CLM-003'"'"', '"'"'CLM-2024-0003'"'"', '"'"'POL-004'"'"', '"'"'2024-08-10 16:45:00'"'"', '"'"'THEFT'"'"', '"'"'Vehicle stolen from parking lot'"'"', 25000.00, 0.00, 0.00, '"'"'UNDER_REVIEW'"'"', '"'"'ADJ-001'"'"')"
  }' | jq .
echo ""

# -----------------------------------------------------------------------------
# QUERY OPERATIONS
# -----------------------------------------------------------------------------

echo "9. List All Active Policies"
echo "---------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT policy_number, policy_type, customer_name, effective_date, expiration_date, premium, coverage_limit, status FROM insurance_api.policies WHERE status = '"'"'ACTIVE'"'"' ORDER BY policy_type, policy_number"
  }' | jq .
echo ""

echo "10. Get Customer Policy Summary"
echo "-------------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT c.customer_id, c.first_name || '"'"' '"'"' || c.last_name AS customer_name, COUNT(p.policy_id) AS policy_count, SUM(p.premium) AS total_premium, SUM(p.coverage_limit) AS total_coverage FROM insurance_api.customers c LEFT JOIN insurance_api.policies p ON c.customer_id = p.customer_id AND p.status = '"'"'ACTIVE'"'"' GROUP BY c.customer_id, c.first_name, c.last_name ORDER BY total_premium DESC"
  }' | jq .
echo ""

echo "11. Claims Dashboard"
echo "--------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT status, COUNT(*) AS claim_count, SUM(claimed_amount) AS total_claimed, SUM(approved_amount) AS total_approved, SUM(paid_amount) AS total_paid FROM insurance_api.claims GROUP BY status ORDER BY claim_count DESC"
  }' | jq .
echo ""

echo "12. Policy Expiring Soon (Next 30 Days)"
echo "----------------------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT policy_number, policy_type, customer_name, expiration_date, expiration_date - CURRENT_DATE AS days_until_expiry FROM insurance_api.policies WHERE status = '"'"'ACTIVE'"'"' AND expiration_date BETWEEN CURRENT_DATE AND CURRENT_DATE + INTERVAL '"'"'30 days'"'"' ORDER BY expiration_date"
  }' | jq .
echo ""

echo "13. Premium by Policy Type"
echo "--------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT policy_type, COUNT(*) AS policy_count, SUM(premium) AS total_premium, AVG(premium) AS avg_premium, SUM(coverage_limit) AS total_exposure FROM insurance_api.policies WHERE status = '"'"'ACTIVE'"'"' GROUP BY policy_type ORDER BY total_premium DESC"
  }' | jq .
echo ""

echo "14. Claims by Loss Type"
echo "-----------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT loss_type, COUNT(*) AS claim_count, SUM(claimed_amount) AS total_claimed, AVG(claimed_amount) AS avg_claim_size, SUM(approved_amount) AS total_approved FROM insurance_api.claims GROUP BY loss_type ORDER BY total_claimed DESC"
  }' | jq .
echo ""

# -----------------------------------------------------------------------------
# UPDATE OPERATIONS
# -----------------------------------------------------------------------------

echo "15. Approve a Claim"
echo "-------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "UPDATE insurance_api.claims SET status = '"'"'APPROVED'"'"', approved_amount = 22000.00 WHERE claim_id = '"'"'CLM-003'"'"'"
  }' | jq .
echo ""

echo "16. Process Claim Payment"
echo "-------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "UPDATE insurance_api.claims SET status = '"'"'CLOSED'"'"', paid_amount = approved_amount WHERE claim_id = '"'"'CLM-002'"'"'"
  }' | jq .
echo ""

# -----------------------------------------------------------------------------
# ADVANCED QUERIES
# -----------------------------------------------------------------------------

echo "17. Loss Ratio Analysis"
echo "-----------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT p.policy_type, COUNT(DISTINCT p.policy_id) AS policy_count, SUM(p.premium) AS earned_premium, COALESCE(SUM(c.paid_amount), 0) AS incurred_losses, ROUND(COALESCE(SUM(c.paid_amount), 0) / NULLIF(SUM(p.premium), 0) * 100, 2) AS loss_ratio FROM insurance_api.policies p LEFT JOIN insurance_api.claims c ON p.policy_id = c.policy_id GROUP BY p.policy_type ORDER BY loss_ratio DESC"
  }' | jq .
echo ""

echo "18. Customer Risk Profile"
echo "-------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT c.customer_id, c.first_name || '"'"' '"'"' || c.last_name AS customer_name, c.risk_score, COUNT(DISTINCT p.policy_id) AS policies, COUNT(DISTINCT cl.claim_id) AS claims, COALESCE(SUM(cl.claimed_amount), 0) AS total_claims_amount FROM insurance_api.customers c LEFT JOIN insurance_api.policies p ON c.customer_id = p.customer_id LEFT JOIN insurance_api.claims cl ON p.policy_id = cl.policy_id GROUP BY c.customer_id, c.first_name, c.last_name, c.risk_score ORDER BY total_claims_amount DESC"
  }' | jq .
echo ""

echo "19. Open Claims Aging Report"
echo "----------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT claim_number, policy_id, loss_type, loss_date, claimed_amount, status, CURRENT_DATE - DATE(loss_date) AS days_open FROM insurance_api.claims WHERE status NOT IN ('"'"'CLOSED'"'"', '"'"'DENIED'"'"') ORDER BY days_open DESC"
  }' | jq .
echo ""

# -----------------------------------------------------------------------------
# BATCH OPERATIONS
# -----------------------------------------------------------------------------

echo "20. Batch Policy Renewal"
echo "------------------------"
curl -s -X POST "${BASE_URL}/sql/batch" \
  -H "Content-Type: application/json" \
  -d '{
    "queries": [
      "UPDATE insurance_api.policies SET premium = premium * 1.05 WHERE policy_type = '"'"'AUTO'"'"' AND status = '"'"'ACTIVE'"'"'",
      "UPDATE insurance_api.policies SET premium = premium * 1.03 WHERE policy_type = '"'"'HOME'"'"' AND status = '"'"'ACTIVE'"'"'"
    ]
  }' | jq .
echo ""

# -----------------------------------------------------------------------------
# SYSTEM INFORMATION
# -----------------------------------------------------------------------------

echo "21. List Tables"
echo "---------------"
curl -s "${BASE_URL}/tables" | jq .
echo ""

echo "22. Cluster Status"
echo "------------------"
curl -s "${BASE_URL}/cluster/status" | jq .
echo ""

echo "23. Server Statistics"
echo "---------------------"
curl -s "${BASE_URL}/stats" | jq .
echo ""

echo "============================================="
echo "Insurance REST API Examples Complete!"
echo "============================================="
