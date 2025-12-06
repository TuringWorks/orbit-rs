#!/usr/bin/env python3
"""
Run Orbit-RS MySQL ML quick examples using mysql-connector-python.

Requirements:
- pip install mysql-connector-python
"""

import os
import sys

try:
    import mysql.connector
except ImportError:
    print("mysql-connector-python is not installed. Install with: pip install mysql-connector-python")
    sys.exit(1)


HOST = os.environ.get("ORBIT_MYSQL_HOST", "127.0.0.1")
PORT = int(os.environ.get("ORBIT_MYSQL_PORT", "3306"))
USER = os.environ.get("ORBIT_MYSQL_USER", "orbit")
PASSWORD = os.environ.get("ORBIT_MYSQL_PASSWORD", "")
DATABASE = os.environ.get("ORBIT_MYSQL_DB", None)


def main():
    print("=" * 60)
    print("Orbit ML Examples - MySQL Protocol")
    print("=" * 60)

    try:
        conn = mysql.connector.connect(
            host=HOST,
            port=PORT,
            user=USER,
            password=PASSWORD,
            database=DATABASE,
        )
        cur = conn.cursor()
        cur.execute("SELECT ML_PREDICT('demo_rf', '[0.2,0.8]')")
        row = cur.fetchone()
        print("Prediction:", row[0] if row else None)
        cur.close()
        conn.close()
    except mysql.connector.Error as e:
        print(f"MySQL Error: {e}")
        print("Ensure Orbit server is running on port 3306 and user credentials are valid.")
        sys.exit(2)

    print("=" * 60)
    print("MySQL quick example completed.")


if __name__ == "__main__":
    main()

