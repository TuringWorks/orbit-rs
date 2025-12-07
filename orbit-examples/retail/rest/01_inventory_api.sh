#!/bin/bash
# =============================================================================
# OrbitRS Retail Example: REST API for Inventory Management
# =============================================================================
# Demonstrates OrbitRS REST API usage for retail inventory operations.
#
# Prerequisites:
#   - OrbitRS server running on localhost:8080
#   - curl installed
#
# Usage:
#   chmod +x 01_inventory_api.sh
#   ./01_inventory_api.sh
# =============================================================================

BASE_URL="http://localhost:8080/api/v1"

echo "============================================="
echo "OrbitRS Retail REST API Examples"
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
# SQL QUERY ENDPOINTS
# -----------------------------------------------------------------------------

echo "2. Create Inventory Schema"
echo "--------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "CREATE SCHEMA IF NOT EXISTS retail"
  }' | jq .
echo ""

echo "3. Create Products Table"
echo "------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "CREATE TABLE IF NOT EXISTS retail.products (product_id VARCHAR(50) PRIMARY KEY, name VARCHAR(255) NOT NULL, category VARCHAR(100), price DECIMAL(10,2), stock_quantity INTEGER DEFAULT 0, reorder_level INTEGER DEFAULT 10, supplier_id VARCHAR(50), created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP)"
  }' | jq .
echo ""

echo "4. Create Inventory Transactions Table"
echo "--------------------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "CREATE TABLE IF NOT EXISTS retail.inventory_transactions (transaction_id VARCHAR(50) PRIMARY KEY, product_id VARCHAR(50) REFERENCES retail.products(product_id), transaction_type VARCHAR(20), quantity INTEGER, unit_cost DECIMAL(10,2), transaction_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP, notes TEXT)"
  }' | jq .
echo ""

echo "5. Insert Sample Products"
echo "-------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "INSERT INTO retail.products (product_id, name, category, price, stock_quantity, reorder_level, supplier_id) VALUES ('"'"'PROD-001'"'"', '"'"'Wireless Mouse'"'"', '"'"'Electronics'"'"', 29.99, 150, 25, '"'"'SUP-TECH-01'"'"'), ('"'"'PROD-002'"'"', '"'"'Mechanical Keyboard'"'"', '"'"'Electronics'"'"', 89.99, 75, 15, '"'"'SUP-TECH-01'"'"'), ('"'"'PROD-003'"'"', '"'"'USB-C Hub'"'"', '"'"'Electronics'"'"', 49.99, 200, 30, '"'"'SUP-TECH-02'"'"'), ('"'"'PROD-004'"'"', '"'"'Monitor Stand'"'"', '"'"'Furniture'"'"', 79.99, 50, 10, '"'"'SUP-FURN-01'"'"'), ('"'"'PROD-005'"'"', '"'"'Desk Lamp'"'"', '"'"'Furniture'"'"', 34.99, 100, 20, '"'"'SUP-FURN-01'"'"')"
  }' | jq .
echo ""

echo "6. Query All Products"
echo "---------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT product_id, name, category, price, stock_quantity FROM retail.products ORDER BY category, name"
  }' | jq .
echo ""

echo "7. Query Low Stock Products"
echo "---------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT product_id, name, stock_quantity, reorder_level FROM retail.products WHERE stock_quantity <= reorder_level * 1.5 ORDER BY stock_quantity ASC"
  }' | jq .
echo ""

echo "8. Record Inventory Receipt"
echo "---------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "INSERT INTO retail.inventory_transactions (transaction_id, product_id, transaction_type, quantity, unit_cost, notes) VALUES ('"'"'TXN-001'"'"', '"'"'PROD-001'"'"', '"'"'RECEIPT'"'"', 100, 15.00, '"'"'PO-2024-001 received'"'"')"
  }' | jq .
echo ""

echo "9. Update Stock Quantity"
echo "------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "UPDATE retail.products SET stock_quantity = stock_quantity + 100 WHERE product_id = '"'"'PROD-001'"'"'"
  }' | jq .
echo ""

echo "10. Inventory Value Report"
echo "--------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT category, COUNT(*) as product_count, SUM(stock_quantity) as total_units, SUM(price * stock_quantity) as total_value FROM retail.products GROUP BY category ORDER BY total_value DESC"
  }' | jq .
echo ""

# -----------------------------------------------------------------------------
# BATCH OPERATIONS
# -----------------------------------------------------------------------------

echo "11. Batch SQL Execution"
echo "-----------------------"
curl -s -X POST "${BASE_URL}/sql/batch" \
  -H "Content-Type: application/json" \
  -d '{
    "queries": [
      "INSERT INTO retail.inventory_transactions (transaction_id, product_id, transaction_type, quantity, unit_cost) VALUES ('"'"'TXN-002'"'"', '"'"'PROD-002'"'"', '"'"'SALE'"'"', -5, 89.99)",
      "UPDATE retail.products SET stock_quantity = stock_quantity - 5 WHERE product_id = '"'"'PROD-002'"'"'",
      "INSERT INTO retail.inventory_transactions (transaction_id, product_id, transaction_type, quantity, unit_cost) VALUES ('"'"'TXN-003'"'"', '"'"'PROD-003'"'"', '"'"'SALE'"'"', -10, 49.99)",
      "UPDATE retail.products SET stock_quantity = stock_quantity - 10 WHERE product_id = '"'"'PROD-003'"'"'"
    ]
  }' | jq .
echo ""

# -----------------------------------------------------------------------------
# SCHEMA AND TABLE INFORMATION
# -----------------------------------------------------------------------------

echo "12. List Schemas"
echo "----------------"
curl -s "${BASE_URL}/schemas" | jq .
echo ""

echo "13. List Tables"
echo "---------------"
curl -s "${BASE_URL}/tables" | jq .
echo ""

echo "14. Describe Products Table"
echo "---------------------------"
curl -s "${BASE_URL}/tables/retail/products" | jq .
echo ""

echo "15. List Indexes on Products"
echo "----------------------------"
curl -s "${BASE_URL}/tables/retail/products/indexes" | jq .
echo ""

# -----------------------------------------------------------------------------
# CLUSTER AND SERVER INFO
# -----------------------------------------------------------------------------

echo "16. Cluster Status"
echo "------------------"
curl -s "${BASE_URL}/cluster/status" | jq .
echo ""

echo "17. Cluster Nodes"
echo "-----------------"
curl -s "${BASE_URL}/cluster/nodes" | jq .
echo ""

echo "18. Server Configuration"
echo "------------------------"
curl -s "${BASE_URL}/config" | jq .
echo ""

echo "19. Database Statistics"
echo "-----------------------"
curl -s "${BASE_URL}/stats" | jq .
echo ""

# -----------------------------------------------------------------------------
# ADVANCED QUERIES
# -----------------------------------------------------------------------------

echo "20. Products with Transaction History"
echo "-------------------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT p.product_id, p.name, p.stock_quantity, COALESCE(SUM(CASE WHEN t.transaction_type = '"'"'RECEIPT'"'"' THEN t.quantity ELSE 0 END), 0) as total_received, COALESCE(SUM(CASE WHEN t.transaction_type = '"'"'SALE'"'"' THEN ABS(t.quantity) ELSE 0 END), 0) as total_sold FROM retail.products p LEFT JOIN retail.inventory_transactions t ON p.product_id = t.product_id GROUP BY p.product_id, p.name, p.stock_quantity ORDER BY p.product_id"
  }' | jq .
echo ""

echo "21. Stock Turnover Analysis"
echo "---------------------------"
curl -s -X POST "${BASE_URL}/sql" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT p.category, COUNT(DISTINCT p.product_id) as products, SUM(p.stock_quantity) as current_stock, AVG(p.price) as avg_price, MIN(p.stock_quantity) as min_stock, MAX(p.stock_quantity) as max_stock FROM retail.products p GROUP BY p.category"
  }' | jq .
echo ""

# -----------------------------------------------------------------------------
# OPENAPI DOCUMENTATION
# -----------------------------------------------------------------------------

echo "22. OpenAPI Specification"
echo "-------------------------"
echo "Available at: ${BASE_URL%/api/v1}/openapi.json"
curl -s "${BASE_URL%/api/v1}/openapi.json" | jq '.info, .paths | keys'
echo ""

echo "============================================="
echo "REST API Examples Complete!"
echo "============================================="
