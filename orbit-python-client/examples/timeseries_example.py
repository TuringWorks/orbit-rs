"""
Time Series Example - Orbit-RS Python Client

Demonstrates usage of Redis TimeSeries compatible commands.
"""

from orbit_client import OrbitClient


def main():
    # Connect to Orbit-RS Redis protocol
    client = OrbitClient.redis(host="127.0.0.1", port=6379)

    with client:
        print("=== Time Series Example ===\n")

        # Create time series with labels
        print("1. Creating time series...")
        client.ts_create(
            "sensor:temperature:room1",
            retention_ms=86400000,  # 24 hours retention
            labels={"location": "room1", "type": "temperature"},
            duplicate_policy="LAST",
        )

        client.ts_create(
            "sensor:temperature:room2",
            retention_ms=86400000,
            labels={"location": "room2", "type": "temperature"},
        )

        # Add samples
        print("2. Adding samples...")
        samples = [
            (1000, 22.5),
            (2000, 23.1),
            (3000, 22.8),
            (4000, 24.0),
            (5000, 23.5),
        ]

        for ts, value in samples:
            client.ts_add("sensor:temperature:room1", ts, value)

        # Add to room2 as well
        for ts, value in samples:
            client.ts_add("sensor:temperature:room2", ts, value + 1.0)

        # Get latest value
        print("\n3. Getting latest value...")
        latest = client.ts_get("sensor:temperature:room1")
        print(f"   Latest: timestamp={latest[0]}, value={latest[1]}")

        # Query range
        print("\n4. Querying range...")
        range_data = client.ts_range("sensor:temperature:room1", "-", "+")
        print(f"   All samples: {range_data}")

        # Query with aggregation
        print("\n5. Querying with aggregation...")
        avg_data = client.ts_range(
            "sensor:temperature:room1",
            "-",
            "+",
            aggregation="AVG",
            bucket_duration_ms=2000,  # 2 second buckets
        )
        print(f"   2s average buckets: {avg_data}")

        # Multi-range query across series
        print("\n6. Multi-range query (TS.MRANGE)...")
        multi_data = client.ts_mrange(
            "-",
            "+",
            filters=["type=temperature"],
        )
        print(f"   All temperature sensors: {multi_data}")

        # Get info
        print("\n7. Getting time series info...")
        info = client.ts_info("sensor:temperature:room1")
        print(f"   Info: {info}")

        # Create compaction rule
        print("\n8. Creating compaction rule...")
        client.ts_create(
            "sensor:temperature:room1:hourly",
            retention_ms=604800000,  # 7 days
            labels={"location": "room1", "type": "temperature", "aggregation": "hourly"},
        )
        client.ts_createrule(
            "sensor:temperature:room1",
            "sensor:temperature:room1:hourly",
            "AVG",
            3600000,  # 1 hour buckets
        )
        print("   Compaction rule created: AVG per hour")

        # Delete samples in range
        print("\n9. Deleting samples in range...")
        deleted = client.ts_del("sensor:temperature:room1", 1000, 2000)
        print(f"   Deleted {deleted} samples")

        # Cleanup - delete compaction rule
        print("\n10. Cleaning up...")
        client.ts_deleterule(
            "sensor:temperature:room1",
            "sensor:temperature:room1:hourly",
        )
        print("   Compaction rule deleted")

        print("\n=== Example Complete ===")


if __name__ == "__main__":
    main()
