#!/bin/bash
# =============================================================================
# OrbitRS Telco Example: REST API for Network Management
# =============================================================================
# Demonstrates OrbitRS REST API usage for telecom network operations.
#
# Prerequisites:
#   - OrbitRS server running on localhost:8080
#   - curl installed
#
# Usage:
#   chmod +x 01_network_api.sh
#   ./01_network_api.sh
# =============================================================================

BASE_URL="http://localhost:8080/api/v1"

echo "============================================="
echo "OrbitRS Telco REST API Examples"
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
# NETWORK SCHEMA SETUP
# -----------------------------------------------------------------------------

echo "2. Create Telco Schema"
echo "----------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "CREATE SCHEMA IF NOT EXISTS telco"
  }' | jq .
echo ""

echo "3. Create Cell Towers Table"
echo "---------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "CREATE TABLE IF NOT EXISTS telco.cell_towers (tower_id VARCHAR(50) PRIMARY KEY, name VARCHAR(255), tower_type VARCHAR(50), latitude DOUBLE PRECISION, longitude DOUBLE PRECISION, status VARCHAR(20) DEFAULT '"'"'ACTIVE'"'"', max_capacity INTEGER, current_connections INTEGER DEFAULT 0, installed_date DATE, last_maintenance DATE)"
  }' | jq .
echo ""

echo "4. Create Network Metrics Table"
echo "--------------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "CREATE TABLE IF NOT EXISTS telco.network_metrics (metric_id SERIAL PRIMARY KEY, tower_id VARCHAR(50), metric_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP, signal_strength_dbm DOUBLE PRECISION, bandwidth_mbps DOUBLE PRECISION, active_connections INTEGER, latency_ms DOUBLE PRECISION, packet_loss_rate DOUBLE PRECISION)"
  }' | jq .
echo ""

echo "5. Create Subscribers Table"
echo "---------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "CREATE TABLE IF NOT EXISTS telco.subscribers (subscriber_id VARCHAR(50) PRIMARY KEY, phone_number VARCHAR(20) UNIQUE, first_name VARCHAR(100), last_name VARCHAR(100), plan_type VARCHAR(50), monthly_price DECIMAL(10,2), status VARCHAR(20) DEFAULT '"'"'ACTIVE'"'"', signup_date DATE, data_usage_gb DOUBLE PRECISION DEFAULT 0)"
  }' | jq .
echo ""

echo "6. Create Network Alerts Table"
echo "-------------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "CREATE TABLE IF NOT EXISTS telco.network_alerts (alert_id VARCHAR(50) PRIMARY KEY, tower_id VARCHAR(50), severity VARCHAR(20), alert_type VARCHAR(50), description TEXT, created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, resolved_at TIMESTAMP, status VARCHAR(20) DEFAULT '"'"'OPEN'"'"')"
  }' | jq .
echo ""

# -----------------------------------------------------------------------------
# INSERT SAMPLE DATA
# -----------------------------------------------------------------------------

echo "7. Insert Cell Towers"
echo "---------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "INSERT INTO telco.cell_towers (tower_id, name, tower_type, latitude, longitude, status, max_capacity, current_connections) VALUES ('"'"'TWR-NYC-001'"'"', '"'"'Manhattan Downtown'"'"', '"'"'5G-MACRO'"'"', 40.7128, -74.0060, '"'"'ACTIVE'"'"', 5000, 1250), ('"'"'TWR-NYC-002'"'"', '"'"'Midtown'"'"', '"'"'5G-MACRO'"'"', 40.7549, -73.9840, '"'"'ACTIVE'"'"', 6000, 2100), ('"'"'TWR-NYC-003'"'"', '"'"'Brooklyn Heights'"'"', '"'"'4G-LTE'"'"', 40.6892, -73.9942, '"'"'ACTIVE'"'"', 4000, 1800), ('"'"'TWR-NYC-004'"'"', '"'"'Queens Central'"'"', '"'"'5G-SMALL'"'"', 40.7282, -73.7949, '"'"'MAINTENANCE'"'"', 2000, 0)"
  }' | jq .
echo ""

echo "8. Insert Subscribers"
echo "---------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "INSERT INTO telco.subscribers (subscriber_id, phone_number, first_name, last_name, plan_type, monthly_price, status, data_usage_gb) VALUES ('"'"'SUB-001'"'"', '"'"'+1-555-0101'"'"', '"'"'John'"'"', '"'"'Smith'"'"', '"'"'UNLIMITED_5G'"'"', 89.99, '"'"'ACTIVE'"'"', 45.2), ('"'"'SUB-002'"'"', '"'"'+1-555-0102'"'"', '"'"'Jane'"'"', '"'"'Doe'"'"', '"'"'FAMILY_SHARE'"'"', 149.99, '"'"'ACTIVE'"'"', 120.5), ('"'"'SUB-003'"'"', '"'"'+1-555-0103'"'"', '"'"'Bob'"'"', '"'"'Johnson'"'"', '"'"'BASIC_4G'"'"', 45.00, '"'"'ACTIVE'"'"', 8.5)"
  }' | jq .
echo ""

echo "9. Insert Network Metrics"
echo "-------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "INSERT INTO telco.network_metrics (tower_id, signal_strength_dbm, bandwidth_mbps, active_connections, latency_ms, packet_loss_rate) VALUES ('"'"'TWR-NYC-001'"'"', -72.5, 850.0, 1250, 12.5, 0.001), ('"'"'TWR-NYC-001'"'"', -74.2, 920.0, 1320, 11.8, 0.0008), ('"'"'TWR-NYC-002'"'"', -68.0, 1100.0, 2100, 8.5, 0.0005), ('"'"'TWR-NYC-003'"'"', -82.0, 450.0, 1800, 22.0, 0.003)"
  }' | jq .
echo ""

echo "10. Insert Network Alerts"
echo "-------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "INSERT INTO telco.network_alerts (alert_id, tower_id, severity, alert_type, description, status) VALUES ('"'"'ALT-001'"'"', '"'"'TWR-NYC-003'"'"', '"'"'WARNING'"'"', '"'"'HIGH_LATENCY'"'"', '"'"'Latency exceeds 20ms threshold'"'"', '"'"'OPEN'"'"'), ('"'"'ALT-002'"'"', '"'"'TWR-NYC-004'"'"', '"'"'CRITICAL'"'"', '"'"'MAINTENANCE'"'"', '"'"'Scheduled maintenance in progress'"'"', '"'"'ACKNOWLEDGED'"'"')"
  }' | jq .
echo ""

# -----------------------------------------------------------------------------
# NETWORK MONITORING QUERIES
# -----------------------------------------------------------------------------

echo "11. Tower Status Dashboard"
echo "--------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT tower_id, name, tower_type, status, current_connections, max_capacity, ROUND(CAST(current_connections AS DECIMAL) / max_capacity * 100, 2) as utilization_percent FROM telco.cell_towers ORDER BY status, tower_id"
  }' | jq .
echo ""

echo "12. Network Health Metrics"
echo "--------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT t.tower_id, t.name, AVG(m.signal_strength_dbm) as avg_signal, AVG(m.bandwidth_mbps) as avg_bandwidth, AVG(m.latency_ms) as avg_latency, AVG(m.packet_loss_rate) as avg_packet_loss FROM telco.cell_towers t JOIN telco.network_metrics m ON t.tower_id = m.tower_id GROUP BY t.tower_id, t.name ORDER BY avg_latency DESC"
  }' | jq .
echo ""

echo "13. Active Alerts Summary"
echo "-------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT a.alert_id, a.severity, a.alert_type, t.name as tower_name, a.description, a.status, a.created_at FROM telco.network_alerts a JOIN telco.cell_towers t ON a.tower_id = t.tower_id WHERE a.status != '"'"'RESOLVED'"'"' ORDER BY CASE a.severity WHEN '"'"'CRITICAL'"'"' THEN 1 WHEN '"'"'WARNING'"'"' THEN 2 ELSE 3 END"
  }' | jq .
echo ""

echo "14. Subscriber Analytics"
echo "------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT plan_type, COUNT(*) as subscriber_count, SUM(monthly_price) as total_revenue, AVG(data_usage_gb) as avg_data_usage FROM telco.subscribers WHERE status = '"'"'ACTIVE'"'"' GROUP BY plan_type ORDER BY total_revenue DESC"
  }' | jq .
echo ""

echo "15. Capacity Planning Report"
echo "----------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT tower_type, COUNT(*) as tower_count, SUM(max_capacity) as total_capacity, SUM(current_connections) as total_connections, ROUND(CAST(SUM(current_connections) AS DECIMAL) / SUM(max_capacity) * 100, 2) as overall_utilization FROM telco.cell_towers WHERE status = '"'"'ACTIVE'"'"' GROUP BY tower_type"
  }' | jq .
echo ""

# -----------------------------------------------------------------------------
# OPERATIONAL UPDATES
# -----------------------------------------------------------------------------

echo "16. Resolve Alert"
echo "-----------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "UPDATE telco.network_alerts SET status = '"'"'RESOLVED'"'"', resolved_at = CURRENT_TIMESTAMP WHERE alert_id = '"'"'ALT-001'"'"'"
  }' | jq .
echo ""

echo "17. Update Tower Status"
echo "-----------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "UPDATE telco.cell_towers SET status = '"'"'ACTIVE'"'"', last_maintenance = CURRENT_DATE WHERE tower_id = '"'"'TWR-NYC-004'"'"'"
  }' | jq .
echo ""

# -----------------------------------------------------------------------------
# SYSTEM INFORMATION
# -----------------------------------------------------------------------------

echo "18. List All Tables"
echo "-------------------"
curl -s "${BASE_URL}/tables" | jq .
echo ""

echo "19. Cluster Health"
echo "------------------"
curl -s "${BASE_URL}/cluster/status" | jq .
echo ""

echo "============================================="
echo "Telco REST API Examples Complete!"
echo "============================================="
