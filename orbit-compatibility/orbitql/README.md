# OrbitQL Compatibility Checks

This directory contains compatibility checks for OrbitQL, Orbit-RS's native multi-model query language.

## Contents

- `compatibility_check.py`: A Python script that verifies OrbitQL syntax and features (Graph, TimeSeries, ML) using the PostgreSQL wire protocol (which OrbitQL supports).
- `requirements.txt`: Python dependencies (`psycopg2-binary`).

## Running the Checks

1.  Install dependencies:
    ```bash
    pip install -r requirements.txt
    ```

2.  Run the script:
    ```bash
    # Set environment variables if needed
    export ORBIT_HOST=localhost
    export ORBIT_PORT=5432
    export ORBIT_USER=orbit
    export ORBIT_PASSWORD=password
    
    python3 compatibility_check.py
    ```