# CQL (Cassandra) Compatibility Checks

This directory contains compatibility checks for the Cassandra Query Language (CQL) protocol.

## Contents

- `compatibility_check.py`: A Python script that verifies CQL support, including tables, UDTs, indexes, materialized views, and LWTs using `cassandra-driver`.
- `requirements.txt`: Python dependencies.

## Running the Checks

1.  Install dependencies:
    ```bash
    pip install -r requirements.txt
    ```

2.  Run the script:
    ```bash
    # Set environment variables if needed
    export CASSANDRA_HOST=localhost
    export CASSANDRA_PORT=9042
    export CASSANDRA_USER=cassandra
    export CASSANDRA_PASSWORD=cassandra
    
    python3 compatibility_check.py
    ```