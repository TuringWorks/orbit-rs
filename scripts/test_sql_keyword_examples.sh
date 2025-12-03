#!/bin/bash

# Test Script for SQL Keyword Examples
# This script tests all the SQL examples that will be added to the documentation

set -e  # Exit on any error

DB_HOST="localhost"
DB_PORT="5433"
DB_USER="orbit"
DB_NAME="actors"

# Color codes for output
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo -e "${CYAN}🧪 Testing SQL Keyword Examples for Documentation${NC}"
echo -e "${CYAN}=================================================${NC}"
echo

# Function to run a query and show result
test_example() {
    local description="$1"
    local query="$2"
    echo -e "${GREEN}📝 Testing: $description${NC}"
    echo -e "Query: ${CYAN}$query${NC}"
    echo
    psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "$query"
    echo
    echo "---"
    echo
}

echo -e "${GREEN}Starting PostgreSQL Wire Protocol Server Examples Test${NC}"
echo

# Clean up any existing data
psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "DELETE FROM actors;" > /dev/null 2>&1 || true

# =============================================================================
# SQL STATEMENT KEYWORDS EXAMPLES
# =============================================================================
echo -e "${CYAN}SQL STATEMENT KEYWORDS EXAMPLES:${NC}"
echo

test_example "INSERT - Basic actor creation" \
    "INSERT INTO actors (actor_id, actor_type, state) VALUES ('user:alice', 'UserActor', '{\"name\": \"Alice\", \"email\": \"alice@example.com\"}');"

test_example "INSERT - Complex JSON state" \
    "INSERT INTO actors (actor_id, actor_type, state) VALUES ('order:12345', 'OrderActor', '{\"order_id\": \"12345\", \"customer\": {\"name\": \"Bob\", \"address\": \"123 Main St\"}, \"items\": [{\"product\": \"laptop\", \"quantity\": 1, \"price\": 999.99}], \"status\": \"pending\", \"total\": 999.99}');"

test_example "SELECT - Retrieve all actors" \
    "SELECT * FROM actors;"

test_example "SELECT - Specific columns with WHERE" \
    "SELECT actor_id, actor_type FROM actors WHERE actor_type = 'UserActor';"

test_example "UPDATE - Modify actor state" \
    "UPDATE actors SET state = '{\"name\": \"Alice Johnson\", \"email\": \"alice.johnson@example.com\", \"verified\": true}' WHERE actor_id = 'user:alice';"

test_example "UPDATE - Conditional update with complex JSON" \
    "UPDATE actors SET state = '{\"order_id\": \"12345\", \"customer\": {\"name\": \"Bob\", \"address\": \"123 Main St\"}, \"items\": [{\"product\": \"laptop\", \"quantity\": 1, \"price\": 999.99}], \"status\": \"shipped\", \"total\": 999.99, \"tracking\": \"TRK123456\"}' WHERE actor_id = 'order:12345';"

test_example "DELETE - Remove specific actor" \
    "DELETE FROM actors WHERE actor_id = 'user:alice';"

# =============================================================================
# SQL CLAUSE KEYWORDS EXAMPLES
# =============================================================================
echo -e "${CYAN}SQL CLAUSE KEYWORDS EXAMPLES:${NC}"
echo

# Setup test data
test_example "Setup: FROM clause example data" \
    "INSERT INTO actors (actor_id, actor_type, state) VALUES ('product:laptop', 'ProductActor', '{\"name\": \"Gaming Laptop\", \"price\": 1299.99, \"category\": \"electronics\"}');"

test_example "Setup: FROM clause example data" \
    "INSERT INTO actors (actor_id, actor_type, state) VALUES ('product:mouse', 'ProductActor', '{\"name\": \"Wireless Mouse\", \"price\": 29.99, \"category\": \"electronics\"}');"

test_example "Setup: FROM clause example data" \
    "INSERT INTO actors (actor_id, actor_type, state) VALUES ('inventory:laptop', 'InventoryActor', '{\"product_id\": \"laptop\", \"quantity\": 50, \"location\": \"warehouse-a\"}');"

test_example "FROM - Select from actors table" \
    "SELECT actor_id, actor_type FROM actors;"

test_example "WHERE - Filter by actor type" \
    "SELECT * FROM actors WHERE actor_type = 'ProductActor';"

test_example "WHERE - Filter by JSON content pattern" \
    "SELECT actor_id, state FROM actors WHERE actor_id LIKE 'product:%';"

test_example "SET - Update with complex nested JSON" \
    "UPDATE actors SET state = '{\"name\": \"Premium Gaming Laptop\", \"price\": 1599.99, \"category\": \"electronics\", \"specs\": {\"cpu\": \"Intel i7\", \"ram\": \"32GB\", \"storage\": \"1TB SSD\"}, \"tags\": [\"gaming\", \"high-performance\", \"portable\"]}' WHERE actor_id = 'product:laptop';"

test_example "INTO - Insert into actors table" \
    "INSERT INTO actors (actor_id, actor_type, state) VALUES ('service:payment', 'PaymentServiceActor', '{\"provider\": \"stripe\", \"enabled\": true, \"supported_currencies\": [\"USD\", \"EUR\", \"GBP\"]}');"

test_example "VALUES - Multiple value insertion" \
    "INSERT INTO actors (actor_id, actor_type, state) VALUES ('cache:sessions', 'CacheActor', '{\"type\": \"redis\", \"ttl\": 3600}'), ('cache:products', 'CacheActor', '{\"type\": \"memory\", \"max_size\": 1000}');"

# =============================================================================
# WHERE OPERATORS EXAMPLES
# =============================================================================
echo -e "${CYAN}WHERE OPERATORS EXAMPLES:${NC}"
echo

test_example "= (Equality) - Exact match" \
    "SELECT actor_id, actor_type FROM actors WHERE actor_type = 'ProductActor';"

test_example "!= (Not Equal) - Exclude specific type" \
    "SELECT actor_id, actor_type FROM actors WHERE actor_type != 'CacheActor';"

test_example "<> (Not Equal Alternative) - Alternative syntax" \
    "SELECT actor_id, actor_type FROM actors WHERE actor_type <> 'InventoryActor';"

# =============================================================================
# TABLE SUPPORT EXAMPLES
# =============================================================================
echo -e "${CYAN}TABLE SUPPORT EXAMPLES:${NC}"
echo

test_example "actors table - The only supported table" \
    "SELECT COUNT(*) as total_actors FROM actors;"

# =============================================================================
# COLUMN SUPPORT EXAMPLES  
# =============================================================================
echo -e "${CYAN}COLUMN SUPPORT EXAMPLES:${NC}"
echo

test_example "actor_id column - Primary identifier" \
    "SELECT actor_id FROM actors WHERE actor_id LIKE 'product:%';"

test_example "actor_type column - Actor classification" \
    "SELECT DISTINCT actor_type FROM actors;"

test_example "state column - JSON state data" \
    "SELECT actor_id, state FROM actors WHERE actor_type = 'ProductActor';"

test_example "* (all columns) - Complete record" \
    "SELECT * FROM actors WHERE actor_id = 'product:laptop';"

test_example "Multiple columns - Specific fields" \
    "SELECT actor_id, actor_type, state FROM actors WHERE actor_type = 'CacheActor';"

# =============================================================================
# JSON STATE EXAMPLES
# =============================================================================
echo -e "${CYAN}JSON STATE EXAMPLES:${NC}"
echo

test_example "Simple JSON - Basic key-value pairs" \
    "INSERT INTO actors (actor_id, actor_type, state) VALUES ('config:app', 'ConfigActor', '{\"debug\": true, \"port\": 8080, \"environment\": \"development\"}');"

test_example "Nested JSON - Complex object structures" \
    "INSERT INTO actors (actor_id, actor_type, state) VALUES ('user:profile', 'UserProfileActor', '{\"user_id\": \"12345\", \"profile\": {\"personal\": {\"name\": \"John Doe\", \"age\": 30}, \"preferences\": {\"theme\": \"dark\", \"notifications\": true}}, \"metadata\": {\"created_at\": \"2024-01-15T10:30:00Z\", \"last_login\": \"2024-01-20T14:45:00Z\"}}');"

test_example "JSON with Arrays - Lists and collections" \
    "INSERT INTO actors (actor_id, actor_type, state) VALUES ('playlist:favorites', 'PlaylistActor', '{\"name\": \"My Favorites\", \"songs\": [{\"title\": \"Song 1\", \"artist\": \"Artist A\", \"duration\": 180}, {\"title\": \"Song 2\", \"artist\": \"Artist B\", \"duration\": 240}], \"tags\": [\"pop\", \"rock\", \"favorites\"], \"created_by\": \"user:12345\"}');"

test_example "JSON with Special Characters - Quotes and escapes" \
    "INSERT INTO actors (actor_id, actor_type, state) VALUES ('message:welcome', 'MessageActor', '{\"content\": \"Welcome to \\\"Orbit-RS\\\"! It'\"'\"'s great to have you here.\", \"author\": \"System\", \"type\": \"welcome\"}');"

test_example "Empty JSON - Minimal state" \
    "INSERT INTO actors (actor_id, actor_type, state) VALUES ('temp:placeholder', 'PlaceholderActor', '{}');"

test_example "Unicode JSON - International characters" \
    "INSERT INTO actors (actor_id, actor_type, state) VALUES ('greeting:international', 'GreetingActor', '{\"messages\": {\"english\": \"Hello! 👋\", \"spanish\": \"¡Hola!\", \"chinese\": \"你好\", \"japanese\": \"こんにちは\", \"emoji\": \"🌍🚀✨\"}}');"

# =============================================================================
# CASE SENSITIVITY EXAMPLES
# =============================================================================
echo -e "${CYAN}CASE SENSITIVITY EXAMPLES:${NC}"
echo

test_example "Lowercase keywords - All lowercase SQL" \
    "insert into actors (actor_id, actor_type, state) values ('test:lowercase', 'TestActor', '{\"case\": \"lowercase\"}');"

test_example "Uppercase keywords - All uppercase SQL" \
    "SELECT * FROM ACTORS WHERE ACTOR_ID = 'test:lowercase';"

test_example "Mixed case keywords - CamelCase SQL" \
    "UpDaTe actors SeT state = '{\"case\": \"mixed\", \"updated\": true}' WhErE actor_id = 'test:lowercase';"

test_example "Case insensitive column names - Various cases" \
    "SELECT ACTOR_ID, actor_type, State FROM actors WHERE actor_id = 'test:lowercase';"

# =============================================================================
# EDGE CASES AND SPECIAL CHARACTERS
# =============================================================================
echo -e "${CYAN}EDGE CASES AND SPECIAL CHARACTERS EXAMPLES:${NC}"
echo

test_example "Extra whitespace - Handles multiple spaces and formatting" \
    "   SELECT   actor_id   ,   actor_type   FROM   actors   WHERE   actor_type   =   'TestActor'   ;"

test_example "Special characters in IDs - Email-like and complex identifiers" \
    "INSERT INTO actors (actor_id, actor_type, state) VALUES ('user:john.doe@company.com', 'UserActor', '{\"email\": \"john.doe@company.com\", \"domain\": \"company.com\"}');"

test_example "Semicolon handling - Query termination" \
    "SELECT actor_id FROM actors WHERE actor_id = 'user:john.doe@company.com';"

test_example "Complex identifiers - Special characters and symbols" \
    "INSERT INTO actors (actor_id, actor_type, state) VALUES ('api:v1/users/:id/profile', 'ApiEndpointActor', '{\"method\": \"GET\", \"path\": \"/api/v1/users/:id/profile\", \"protected\": true}');"

# =============================================================================
# COMPREHENSIVE COMBINATIONS
# =============================================================================
echo -e "${CYAN}COMPREHENSIVE OPERATION COMBINATIONS:${NC}"
echo

test_example "Complex SELECT with multiple conditions" \
    "SELECT actor_id, actor_type, state FROM actors WHERE (actor_type = 'ProductActor' OR actor_type = 'UserActor') AND actor_id LIKE '%user%';"

test_example "Multi-step operation sequence" \
    "INSERT INTO actors (actor_id, actor_type, state) VALUES ('workflow:step1', 'WorkflowActor', '{\"step\": 1, \"status\": \"pending\", \"data\": {\"input\": \"task data\"}}');"

test_example "Update workflow status" \
    "UPDATE actors SET state = '{\"step\": 1, \"status\": \"completed\", \"data\": {\"input\": \"task data\", \"output\": \"processed result\"}, \"completed_at\": \"2024-01-20T15:30:00Z\"}' WHERE actor_id = 'workflow:step1';"

test_example "Final verification query" \
    "SELECT actor_id, actor_type FROM actors WHERE actor_id LIKE 'workflow:%';"

# =============================================================================
# CLEANUP
# =============================================================================
echo -e "${CYAN}CLEANUP:${NC}"
echo

test_example "Clean up all test data" \
    "DELETE FROM actors;"

test_example "Verify cleanup" \
    "SELECT COUNT(*) as remaining_actors FROM actors;"

echo -e "${GREEN}✅ All SQL keyword examples tested successfully!${NC}"
echo -e "${GREEN}These examples are ready to be added to the documentation.${NC}"