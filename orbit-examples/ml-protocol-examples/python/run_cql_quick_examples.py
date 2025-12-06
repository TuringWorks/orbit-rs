#!/usr/bin/env python3
"""
Run Orbit-RS CQL (Cassandra) ML quick examples using cassandra-driver.

Requirements:
- pip install cassandra-driver
"""

import os
import sys

try:
    from cassandra.cluster import Cluster
except ImportError:
    print("cassandra-driver is not installed. Install with: pip install cassandra-driver")
    sys.exit(1)


HOST = os.environ.get("ORBIT_CQL_HOST", "127.0.0.1")
PORT = int(os.environ.get("ORBIT_CQL_PORT", "9042"))


def main():
    print("=" * 60)
    print("Orbit ML Examples - CQL Protocol")
    print("=" * 60)

    try:
        cluster = Cluster([HOST], port=PORT)
        session = cluster.connect()
        rows = session.execute("SELECT ML_PREDICT('demo_rf', '[0.2,0.8]') FROM system.local")
        for row in rows:
            print("Prediction:", row[0])
        cluster.shutdown()
    except Exception as e:
        print(f"CQL Error: {e}")
        print("Ensure Orbit server is running on port 9042 and CQL adapter is enabled.")
        sys.exit(2)

    print("=" * 60)
    print("CQL quick example completed.")


if __name__ == "__main__":
    main()

