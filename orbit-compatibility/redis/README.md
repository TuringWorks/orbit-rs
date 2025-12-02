# Redis Compatibility Checks

This directory contains compatibility checks for the Redis (RESP) protocol.

## Contents

- `compatibility_check.py`: A Python script that verifies Redis command support using `redis-py`.
- `requirements.txt`: Python dependencies.

## Running the Checks

1.  Install dependencies:
    ```bash
    pip install -r requirements.txt
    ```

2.  Run the script:
    ```bash
    # Set environment variables if needed
    export REDIS_HOST=localhost
    export REDIS_PORT=6379
    
    python3 compatibility_check.py
    ```