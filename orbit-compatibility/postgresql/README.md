# PostgreSQL Compatibility Checks

This directory contains a comprehensive suite of compatibility checks for the PostgreSQL protocol, derived from the [PostgreSQL Compatibility Index (PCI)](https://pgscorecard.com/).

## Contents

- `compatibility_check.py`: A Python script that runs a series of SQL commands to verify support for various PostgreSQL features.
- `pgvector_check.py`: Verifies support for the `pgvector` extension (vector embeddings, similarity search).
- `timescale_check.py`: Verifies support for the `timescaledb` extension (hypertables, time-series functions).
- `pg18_check.py`: Verifies support for PostgreSQL 18 features (JSON_TABLE, UUIDv7).
- `requirements.txt`: Python dependencies for the scripts.

## Running the Checks

1.  Install dependencies:
    ```bash
    pip install -r requirements.txt
    ```

2.  Run the script:
    ```bash
    # Set environment variables if needed (defaults: localhost:5432, user=postgres, db=testdb)
    export PG_HOST=localhost
    export PG_PORT=5432
    export PG_USER=postgres
    export PG_PASSWORD=password
    export PG_DBNAME=testdb
    
    python3 compatibility_check.py
    
    # Run pgvector checks
    python3 pgvector_check.py
    
    # Run TimescaleDB checks
    python3 timescale_check.py
    
    # Run PostgreSQL 18 checks
    python3 pg18_check.py
    ```

## Advanced Testing

For even more rigorous testing, consider using **pgTAP**, a unit testing framework for PostgreSQL.
It allows you to write tests in SQL and run them using `pg_prove`.

Example pgTAP test (`test.sql`):
```sql
BEGIN;
SELECT plan(1);
SELECT pass('Simple test');
SELECT * FROM finish();
ROLLBACK;
```

Run with:
```bash
pg_prove -h localhost -p 5432 -U postgres -d testdb test.sql
```