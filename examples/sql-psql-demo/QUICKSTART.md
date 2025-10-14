# Quick Start Guide

## 🚀 Start the Integrated Server (RECOMMENDED)

**Get PostgreSQL + Redis + gRPC in one command:**

```bash
# From project root
cargo run --package orbit-server --example integrated-server

# Connect with any client:
# PostgreSQL: psql -h localhost -p 5432 -U orbit -d actors
# Redis: redis-cli -h localhost -p 6379
# gRPC: grpcurl -plaintext localhost:50051 list
```

## 🚀 Alternative: SQL Demo Only

```bash
cd examples/sql-psql-demo
./run_demo.sh
```

Then choose option 1-4 to run the demo scripts, or option 5 for interactive psql.

## 🔗 Manual PostgreSQL Server Only

If you want just the PostgreSQL server:

```bash
# Terminal 1: Start PostgreSQL server only
cargo run --package orbit-server --example postgres-server

# Terminal 2: Connect with psql  
psql -h localhost -p 5433 -U orbit -d actors
```

## 📝 Quick SQL Commands to Try

Once connected to psql:

```sql
-- Create a table
CREATE TABLE demo (id INTEGER, name TEXT);

-- Insert data
INSERT INTO demo VALUES (1, 'Hello'), (2, 'World');

-- Query data
SELECT * FROM demo;

-- Create another table for joins
CREATE TABLE orders (id INTEGER, user_id INTEGER, amount DECIMAL);
INSERT INTO orders VALUES (1, 1, 100.50), (2, 1, 200.75);

-- Try a JOIN
SELECT d.name, SUM(o.amount) as total
FROM demo d 
LEFT JOIN orders o ON d.id = o.user_id
GROUP BY d.id, d.name;

-- Exit
\q
```

## ✅ What Works

- ✅ All basic SQL: CREATE TABLE, INSERT, SELECT, UPDATE, DELETE
- ✅ All JOIN types: INNER, LEFT, RIGHT, FULL OUTER, CROSS
- ✅ Indices: CREATE INDEX on single/multiple columns
- ✅ Transactions: BEGIN, COMMIT, ROLLBACK
- ✅ Data types: INTEGER, TEXT, DECIMAL, BOOLEAN, TIMESTAMP
- ✅ Vector types: VECTOR(n) for embeddings
- ✅ JSON: JSONB storage and basic queries

## ⚠️ Current Limitations

- Data is in-memory only (lost on server restart)
- Some advanced SQL features are parsed but not executed
- Vector similarity operators need implementation
- Advanced JSONB operators pending

Enjoy exploring the Orbit-RS PostgreSQL implementation! 🎉