# MySQL Compatibility Checks

This directory contains compatibility checks for the MySQL protocol.

## Contents

- `compatibility_check.py`: A Python script that verifies MySQL support using `mysql-connector-python`.
- `requirements.txt`: Python dependencies.

## Running the Checks

1.  Install dependencies:
    ```bash
    pip install -r requirements.txt
    ```

2.  Run the script:
    ```bash
    # Set environment variables if needed
    export MYSQL_HOST=localhost
    export MYSQL_PORT=3306
    export MYSQL_USER=root
    export MYSQL_PASSWORD=password
    
    python3 compatibility_check.py
    ```