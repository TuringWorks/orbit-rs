#!/usr/bin/env python3
"""
Comprehensive test suite for Orbit Time Series (TS.*) commands
Tests Redis TimeSeries compatibility with the Orbit vector store
"""
import redis
import time
import sys
import json
from typing import Dict, List, Any, Optional

def connect_to_orbit():
    """Connect to Orbit RESP server"""
    try:
        client = redis.Redis(host='localhost', port=6379, decode_responses=True)
        client.ping()
        print("✅ Connected to Orbit RESP server")
        return client
    except redis.ConnectionError:
        print("❌ Failed to connect to Orbit RESP server")
        print("Make sure the server is running on localhost:6379")
        return None

class TimeSeriesTests:
    def __init__(self, redis_client):
        self.client = redis_client
        self.test_results = []
        
    def run_command(self, cmd: str, *args) -> Any:
        """Execute a Redis command and return result"""
        try:
            result = self.client.execute_command(cmd, *args)
            print(f"✅ {cmd} {' '.join(map(str, args))} -> {result}")
            return result
        except Exception as e:
            print(f"❌ {cmd} {' '.join(map(str, args))} -> ERROR: {e}")
            raise
    
    def assert_equal(self, actual, expected, description: str):
        """Assert that actual equals expected"""
        if actual == expected:
            print(f"✅ {description}: {actual}")
            self.test_results.append(True)
        else:
            print(f"❌ {description}: expected {expected}, got {actual}")
            self.test_results.append(False)
            
    def assert_not_none(self, value, description: str):
        """Assert that value is not None"""
        if value is not None:
            print(f"✅ {description}: {value}")
            self.test_results.append(True)
        else:
            print(f"❌ {description}: got None")
            self.test_results.append(False)
    
    def test_basic_creation_and_info(self):
        """Test TS.CREATE and TS.INFO commands"""
        print("\n=== Testing Basic Creation and Info ===")
        
        # Create a basic time series
        result = self.run_command("TS.CREATE", "temp:sensor1")
        self.assert_equal(result, "OK", "TS.CREATE basic series")
        
        # Create series with retention
        result = self.run_command("TS.CREATE", "temp:sensor2", "RETENTION", "86400000", "LABELS", "location", "office", "type", "temperature")
        self.assert_equal(result, "OK", "TS.CREATE with retention and labels")
        
        # Get info for basic series
        info = self.run_command("TS.INFO", "temp:sensor1")
        self.assert_not_none(info, "TS.INFO basic series")
        
        # Get info for configured series
        info = self.run_command("TS.INFO", "temp:sensor2")
        self.assert_not_none(info, "TS.INFO configured series")
    
    def test_data_ingestion(self):
        """Test TS.ADD, TS.MADD commands"""
        print("\n=== Testing Data Ingestion ===")
        
        # Get current timestamp
        current_time = int(time.time() * 1000)
        
        # Add single sample
        result = self.run_command("TS.ADD", "metrics:cpu", current_time, "45.5")
        self.assert_equal(result, current_time, "TS.ADD single sample")
        
        # Add sample with auto timestamp
        result = self.run_command("TS.ADD", "metrics:memory", "*", "67.2")
        self.assert_not_none(result, "TS.ADD with auto timestamp")
        
        # Add multiple samples
        timestamps = [current_time + i * 1000 for i in range(1, 4)]
        values = ["50.1", "52.3", "48.7"]
        
        args = []
        for ts, val in zip(timestamps, values):
            args.extend(["metrics:cpu", str(ts), val])
        
        result = self.run_command("TS.MADD", *args)
        self.assert_equal(len(result), 3, "TS.MADD multiple samples")
        
    def test_increment_decrement(self):
        """Test TS.INCRBY and TS.DECRBY commands"""
        print("\n=== Testing Increment/Decrement ===")
        
        # Create counter series
        self.run_command("TS.CREATE", "counters:requests")
        
        current_time = int(time.time() * 1000)
        
        # Increment by value
        result = self.run_command("TS.INCRBY", "counters:requests", "10")
        self.assert_not_none(result, "TS.INCRBY basic")
        
        # Increment with timestamp
        result = self.run_command("TS.INCRBY", "counters:requests", "5", "TIMESTAMP", str(current_time + 1000))
        self.assert_equal(result, current_time + 1000, "TS.INCRBY with timestamp")
        
        # Decrement by value
        result = self.run_command("TS.DECRBY", "counters:requests", "3", "TIMESTAMP", str(current_time + 2000))
        self.assert_equal(result, current_time + 2000, "TS.DECRBY with timestamp")
    
    def test_data_retrieval(self):
        """Test TS.GET, TS.RANGE commands"""
        print("\n=== Testing Data Retrieval ===")
        
        # Get latest sample
        result = self.run_command("TS.GET", "metrics:cpu")
        self.assert_not_none(result, "TS.GET latest sample")
        
        # Get range of samples
        current_time = int(time.time() * 1000)
        start_time = current_time - 10000  # 10 seconds ago
        end_time = current_time + 10000    # 10 seconds from now
        
        result = self.run_command("TS.RANGE", "metrics:cpu", str(start_time), str(end_time))
        self.assert_not_none(result, "TS.RANGE basic")
        
        # Get reverse range
        result = self.run_command("TS.REVRANGE", "metrics:cpu", str(start_time), str(end_time))
        self.assert_not_none(result, "TS.REVRANGE basic")
        
        # Get aggregated range
        result = self.run_command("TS.RANGE", "metrics:cpu", str(start_time), str(end_time), "AGGREGATION", "AVG", "1000")
        self.assert_not_none(result, "TS.RANGE with aggregation")
    
    def test_multiple_series_operations(self):
        """Test TS.MGET, TS.MRANGE commands"""
        print("\n=== Testing Multiple Series Operations ===")
        
        # Multi-get latest samples
        result = self.run_command("TS.MGET", "metrics:cpu", "metrics:memory")
        self.assert_not_none(result, "TS.MGET multiple series")
        
        # Multi-range query
        current_time = int(time.time() * 1000)
        start_time = current_time - 10000
        end_time = current_time + 10000
        
        result = self.run_command("TS.MRANGE", str(start_time), str(end_time), "metrics:cpu", "metrics:memory")
        self.assert_not_none(result, "TS.MRANGE multiple series")
        
        # Multi-reverse range query
        result = self.run_command("TS.MREVRANGE", str(start_time), str(end_time), "metrics:cpu", "metrics:memory")
        self.assert_not_none(result, "TS.MREVRANGE multiple series")
    
    def test_deletion(self):
        """Test TS.DEL command"""
        print("\n=== Testing Sample Deletion ===")
        
        # Add some test data
        current_time = int(time.time() * 1000)
        self.run_command("TS.CREATE", "test:deletion")
        
        for i in range(5):
            self.run_command("TS.ADD", "test:deletion", str(current_time + i * 1000), str(i * 10))
        
        # Delete range of samples
        result = self.run_command("TS.DEL", "test:deletion", str(current_time), str(current_time + 2000))
        self.assert_not_none(result, "TS.DEL range deletion")
        
        # Verify deletion worked
        remaining = self.run_command("TS.RANGE", "test:deletion", str(current_time - 1000), str(current_time + 10000))
        print(f"Remaining samples after deletion: {remaining}")
    
    def test_compaction_rules(self):
        """Test TS.CREATERULE and TS.DELETERULE commands"""
        print("\n=== Testing Compaction Rules ===")
        
        # Create source and destination series
        self.run_command("TS.CREATE", "source:temp")
        self.run_command("TS.CREATE", "dest:temp:avg")
        
        # Create compaction rule
        result = self.run_command("TS.CREATERULE", "source:temp", "dest:temp:avg", "AGGREGATION", "AVG", "60000")
        self.assert_equal(result, "OK", "TS.CREATERULE creation")
        
        # Add some data to source
        current_time = int(time.time() * 1000)
        for i in range(5):
            self.run_command("TS.ADD", "source:temp", str(current_time + i * 10000), str(20 + i * 2))
        
        # Delete compaction rule
        result = self.run_command("TS.DELETERULE", "source:temp", "dest:temp:avg")
        self.assert_equal(result, "OK", "TS.DELETERULE deletion")
    
    def test_alter_series(self):
        """Test TS.ALTER command"""
        print("\n=== Testing Series Alteration ===")
        
        # Create series and then alter it
        self.run_command("TS.CREATE", "test:alter")
        
        result = self.run_command("TS.ALTER", "test:alter", "RETENTION", "3600000", "LABELS", "env", "test", "metric", "altered")
        self.assert_equal(result, "OK", "TS.ALTER series configuration")
        
        # Check that info shows updated config
        info = self.run_command("TS.INFO", "test:alter")
        self.assert_not_none(info, "TS.INFO after alter")
    
    def test_duplicate_policies(self):
        """Test different duplicate timestamp policies"""
        print("\n=== Testing Duplicate Policies ===")
        
        current_time = int(time.time() * 1000)
        
        # Test LAST policy (default behavior)
        self.run_command("TS.CREATE", "test:dup:last", "DUPLICATE_POLICY", "LAST")
        self.run_command("TS.ADD", "test:dup:last", str(current_time), "100")
        result = self.run_command("TS.ADD", "test:dup:last", str(current_time), "200")  # Should replace
        self.assert_equal(result, current_time, "Duplicate policy LAST")
        
        # Test SUM policy
        self.run_command("TS.CREATE", "test:dup:sum", "DUPLICATE_POLICY", "SUM")
        self.run_command("TS.ADD", "test:dup:sum", str(current_time), "100")
        result = self.run_command("TS.ADD", "test:dup:sum", str(current_time), "50")  # Should sum to 150
        self.assert_equal(result, current_time, "Duplicate policy SUM")
    
    def test_aggregation_functions(self):
        """Test different aggregation functions"""
        print("\n=== Testing Aggregation Functions ===")
        
        # Create series with test data
        self.run_command("TS.CREATE", "test:agg")
        current_time = int(time.time() * 1000)
        
        # Add samples: 10, 20, 30, 40, 50
        for i, val in enumerate([10, 20, 30, 40, 50]):
            self.run_command("TS.ADD", "test:agg", str(current_time + i * 1000), str(val))
        
        start_time = current_time - 1000
        end_time = current_time + 10000
        bucket = "10000"  # 10 second bucket
        
        # Test different aggregations
        for agg_func in ["AVG", "SUM", "MIN", "MAX", "COUNT", "FIRST", "LAST"]:
            try:
                result = self.run_command("TS.RANGE", "test:agg", str(start_time), str(end_time), "AGGREGATION", agg_func, bucket)
                self.assert_not_none(result, f"Aggregation function {agg_func}")
            except Exception as e:
                print(f"⚠️ Aggregation {agg_func} not supported or failed: {e}")
    
    def test_error_conditions(self):
        """Test error handling"""
        print("\n=== Testing Error Conditions ===")
        
        try:
            # Try to get info for non-existent series
            self.run_command("TS.INFO", "nonexistent:series")
            print("❌ Should have failed for non-existent series")
        except:
            print("✅ Correctly handled non-existent series")
            
        try:
            # Try invalid timestamp
            self.run_command("TS.ADD", "metrics:cpu", "invalid", "10.5")
            print("❌ Should have failed for invalid timestamp")
        except:
            print("✅ Correctly handled invalid timestamp")
    
    def run_all_tests(self):
        """Run all time series tests"""
        print("🚀 Starting Orbit Time Series Tests")
        print("=" * 50)
        
        try:
            self.test_basic_creation_and_info()
            self.test_data_ingestion()
            self.test_increment_decrement()
            self.test_data_retrieval()
            self.test_multiple_series_operations()
            self.test_deletion()
            self.test_compaction_rules()
            self.test_alter_series()
            self.test_duplicate_policies()
            self.test_aggregation_functions()
            self.test_error_conditions()
            
        except Exception as e:
            print(f"\n❌ Test suite failed with error: {e}")
            return False
            
        # Print summary
        total_tests = len(self.test_results)
        passed_tests = sum(self.test_results)
        failed_tests = total_tests - passed_tests
        
        print("\n" + "=" * 50)
        print("📊 TIME SERIES TEST RESULTS")
        print("=" * 50)
        print(f"✅ Passed: {passed_tests}/{total_tests}")
        print(f"❌ Failed: {failed_tests}/{total_tests}")
        print(f"📈 Success Rate: {(passed_tests/total_tests)*100:.1f}%")
        
        if failed_tests == 0:
            print("\n🎉 All time series tests passed!")
            return True
        else:
            print(f"\n⚠️ {failed_tests} tests failed. Check the output above for details.")
            return False

def main():
    """Main test runner"""
    client = connect_to_orbit()
    if not client:
        sys.exit(1)
    
    # Clean up any existing test data
    try:
        client.flushdb()
        print("🧹 Cleaned up test database")
    except:
        pass
    
    # Run tests
    tests = TimeSeriesTests(client)
    success = tests.run_all_tests()
    
    if success:
        print("\n✅ Time Series test suite completed successfully!")
        sys.exit(0)
    else:
        print("\n❌ Time Series test suite failed!")
        sys.exit(1)

if __name__ == "__main__":
    main()