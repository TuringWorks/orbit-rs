# MySQL Protocol Examples for Orbit-RS

This directory contains comprehensive examples demonstrating MySQL wire protocol support in Orbit-RS.

## Overview

Orbit-RS implements the MySQL wire protocol, allowing you to use standard MySQL clients and tools to interact with the Orbit database. All MySQL operations are backed by Orbit's unified storage engine with RocksDB persistence.

## Quick Start

### Prerequisites

1. **Start Orbit Server** with MySQL protocol enabled (default port 3306):
   ```bash
   cargo run --bin orbit-server
   ```

2. **Install MySQL Client**:
   ```bash
   # macOS
   brew install mysql-client
   
   # Ubuntu/Debian
   sudo apt-get install mysql-client
   
   # Python driver
   pip install mysql-connector-python
   ```

### Basic Connection

```bash
# Connect with mysql client
mysql -h localhost -P 3306 -u orbit

# Or specify database
mysql -h localhost -P 3306 -u orbit -D mydb
```

## Examples Structure

```
mysql/
├── README.md                          # This file
├── 01_basic_sql.sql                   # Basic SQL operations
├── 02_ml_functions.sql                # ML function integration
├── 03_prepared_statements.sql         # Prepared statement examples
├── 04_transactions.sql                # Transaction examples
├── 05_vector_operations.sql           # Vector similarity search
├── python/
│   ├── mysql_connector.py             # Python mysql-connector examples
│   └── mysql_ml.py                    # ML integration with Python
└── scenarios/
    ├── finance_scenario.sql           # Banking/finance use case
    └── retail_scenario.sql            # Retail analytics use case
```

## Running Examples

### SQL Examples

```bash
# Run with mysql client
mysql -h localhost -P 3306 -u orbit < 01_basic_sql.sql

# Or run interactively
mysql -h localhost -P 3306 -u orbit
mysql> source 01_basic_sql.sql
```

### Python Examples

```bash
cd python
python mysql_connector.py
python mysql_ml.py
```

## Key Features Demonstrated

### 1. SQL Operations
- CREATE, ALTER, DROP tables
- INSERT, SELECT, UPDATE, DELETE
- JOINs (INNER, LEFT, RIGHT, FULL)
- Subqueries and CTEs
- Window functions

### 2. ML Integration
- ML_TRAIN_MODEL() for model training
- ML_PREDICT() for inference
- ML_EMBED_TEXT() for text embeddings
- Vector similarity search

### 3. Prepared Statements
- Parameter binding
- Execution optimization
- SQL injection prevention

### 4. Transactions
- BEGIN, COMMIT, ROLLBACK
- Isolation levels
- Savepoints
- Distributed transactions

### 5. Vector Operations
- Vector data types
- Cosine similarity
- L2 distance
- Vector indexes

## MySQL vs Other Protocols

**When to use MySQL protocol:**
- Migrating from MySQL databases
- Using MySQL-specific tools
- Need MySQL compatibility
- Working with existing MySQL applications

**Cross-Protocol Access:**
```sql
-- Write via MySQL
INSERT INTO products (name, price) VALUES ('Laptop', 999);

-- Read via PostgreSQL
psql> SELECT * FROM products WHERE name = 'Laptop';

-- Read via MongoDB
db.products.find({name: 'Laptop'})

-- Read via Redis
redis-cli> HGETALL product:1
```

## ML Function Examples

### Model Training
```sql
-- Train a classification model
SELECT ML_TRAIN_MODEL(
  'churn_model',
  'random_forest',
  ARRAY[tenure, monthly_charges, total_charges],
  churned
) FROM customers;
```

### Prediction
```sql
-- Predict customer churn
SELECT 
  customer_id,
  name,
  ML_PREDICT('churn_model', ARRAY[tenure, monthly_charges, total_charges]) as churn_risk
FROM customers
WHERE churn_risk > 0.7;
```

### Text Embeddings
```sql
-- Generate embeddings for text
SELECT 
  document_id,
  content,
  ML_EMBED_TEXT(content, 'sentence-transformers') as embedding
FROM documents;
```

### Vector Similarity
```sql
-- Find similar documents
SELECT 
  d1.document_id,
  d1.content,
  COSINE_SIMILARITY(d1.embedding, d2.embedding) as similarity
FROM documents d1
CROSS JOIN documents d2
WHERE d1.document_id != d2.document_id
  AND d2.document_id = 123
ORDER BY similarity DESC
LIMIT 5;
```

## Performance Tips

1. **Use Indexes**: Create indexes on frequently queried columns
   ```sql
   CREATE INDEX idx_customer_email ON customers(email);
   CREATE INDEX idx_product_category ON products(category, price);
   ```

2. **Prepared Statements**: Use prepared statements for repeated queries
   ```sql
   PREPARE stmt FROM 'SELECT * FROM products WHERE category = ?';
   SET @cat = 'Electronics';
   EXECUTE stmt USING @cat;
   ```

3. **Batch Inserts**: Use multi-row INSERT for bulk data
   ```sql
   INSERT INTO products (name, price) VALUES
     ('Product 1', 10),
     ('Product 2', 20),
     ('Product 3', 30);
   ```

4. **EXPLAIN**: Analyze query execution plans
   ```sql
   EXPLAIN SELECT * FROM orders WHERE customer_id = 123;
   ```

## Troubleshooting

### Connection Issues
```bash
# Check if MySQL port is listening
lsof -i :3306

# Test connection
mysql -h localhost -P 3306 -u orbit -e "SELECT 1"
```

### Authentication
```sql
-- Connect with password (if enabled)
mysql -h localhost -P 3306 -u orbit -p
```

### Error Handling
```sql
-- Check for errors
SHOW WARNINGS;
SHOW ERRORS;
```

## Differences from Standard MySQL

Orbit-RS implements the MySQL wire protocol but uses its own storage engine:

**Supported:**
- Most MySQL SQL syntax
- Prepared statements
- Transactions
- Indexes
- ML functions (Orbit extension)

**Not Supported (yet):**
- Some MySQL-specific functions
- Stored procedures
- Triggers
- Views (in progress)

## See Also

- [MySQL Wire Protocol Implementation](../../orbit/server/src/protocols/mysql/)
- [Cross-Protocol Examples](../cross-protocol/)
- [ML Protocol Integration](../ml-protocol-examples/)
- [MySQL Official Documentation](https://dev.mysql.com/doc/)

## Contributing

To add new MySQL examples:
1. Follow the existing file naming convention
2. Include comprehensive comments
3. Test against running Orbit server
4. Update this README
