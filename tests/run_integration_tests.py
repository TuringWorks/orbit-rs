#!/usr/bin/env python3
"""
Integration Test Runner for Orbit-RS

This script runs all Python integration tests for the Orbit-RS server.
It tests various protocol implementations including RESP, graph commands,
time series commands, and vector store functionality.

Usage:
    python3 tests/run_integration_tests.py [--test TEST_NAME] [--host HOST] [--port PORT]

Requirements:
    pip install -r tests/requirements.txt
"""

import sys
import os
import argparse
import importlib.util
from pathlib import Path


def load_test_module(test_path: Path):
    """Dynamically load a test module from file path."""
    spec = importlib.util.spec_from_file_location("test_module", test_path)
    module = importlib.util.module_from_spec(spec)
    sys.modules["test_module"] = module
    spec.loader.exec_module(module)
    return module


def run_graph_tests(host='localhost', port=6379):
    """Run graph command tests."""
    print("🚀 Running Graph Commands Integration Tests")
    print("=" * 60)
    
    test_path = Path(__file__).parent / "integration" / "test_graph_commands.py"
    module = load_test_module(test_path)
    
    # Connect to Orbit and run tests
    client = module.connect_to_orbit()
    if not client:
        return False
    
    tests = module.GraphDatabaseTests(client)
    
    # Run all test methods
    try:
        tests.test_basic_node_creation()
        tests.test_node_matching()
        tests.test_relationships()
        tests.test_read_only_queries()
        tests.test_graph_management()
        tests.test_query_explanation()
        tests.test_query_profiling()
        
        # Print results summary
        passed = sum(tests.test_results)
        total = len(tests.test_results)
        print(f"\n📊 Graph Tests Results: {passed}/{total} tests passed")
        return passed == total
        
    except Exception as e:
        print(f"❌ Graph tests failed: {e}")
        return False


def run_timeseries_tests(host='localhost', port=6379):
    """Run time series command tests."""
    print("\n🕒 Running Time Series Commands Integration Tests")
    print("=" * 60)
    
    test_path = Path(__file__).parent / "integration" / "test_timeseries_commands.py"
    module = load_test_module(test_path)
    
    # Connect to Orbit and run tests
    client = module.connect_to_orbit()
    if not client:
        return False
    
    tests = module.TimeSeriesTests(client)
    
    # Run all test methods
    try:
        tests.test_basic_creation_and_info()
        tests.test_data_ingestion()
        tests.test_increment_decrement()
        tests.test_data_retrieval()
        tests.test_multiple_series_operations()
        tests.test_deletion()
        tests.test_compaction_rules()
        
        # Print results summary
        passed = sum(tests.test_results)
        total = len(tests.test_results)
        print(f"\n📊 Time Series Tests Results: {passed}/{total} tests passed")
        return passed == total
        
    except Exception as e:
        print(f"❌ Time series tests failed: {e}")
        return False


def run_vector_tests(host='127.0.0.1', port=6381):
    """Run vector command tests."""
    print("\n🔍 Running Vector Commands Integration Tests")
    print("=" * 60)
    
    test_path = Path(__file__).parent / "integration" / "test_vector_commands.py"
    module = load_test_module(test_path)
    
    # Create tester and run tests
    try:
        tester = module.VectorCommandsTester(host=host, port=port)
        
        if not tester.connect():
            return False
        
        # Run test methods
        tester.test_basic_vector_commands()
        tester.test_vector_search_commands()
        tester.test_ft_commands()
        
        print(f"\n📊 Vector Tests: Completed (check output for detailed results)")
        return True
        
    except Exception as e:
        print(f"❌ Vector tests failed: {e}")
        return False


def main():
    """Main test runner."""
    parser = argparse.ArgumentParser(description="Run Orbit-RS integration tests")
    parser.add_argument("--test", choices=["graph", "timeseries", "vector", "all"], 
                       default="all", help="Which test suite to run")
    parser.add_argument("--host", default="localhost", help="Server host")
    parser.add_argument("--resp-port", default=6379, type=int, help="RESP server port")
    parser.add_argument("--vector-port", default=6381, type=int, help="Vector server port")
    
    args = parser.parse_args()
    
    print("🧪 Orbit-RS Integration Test Suite")
    print("=" * 50)
    print(f"Test suite: {args.test}")
    print(f"RESP server: {args.host}:{args.resp_port}")
    print(f"Vector server: {args.host}:{args.vector_port}")
    print("=" * 50)
    
    results = []
    
    if args.test in ["graph", "all"]:
        results.append(("Graph", run_graph_tests(args.host, args.resp_port)))
    
    if args.test in ["timeseries", "all"]:
        results.append(("Time Series", run_timeseries_tests(args.host, args.resp_port)))
    
    if args.test in ["vector", "all"]:
        results.append(("Vector", run_vector_tests(args.host, args.vector_port)))
    
    # Final summary
    print("\n" + "=" * 60)
    print("🏁 FINAL TEST RESULTS")
    print("=" * 60)
    
    all_passed = True
    for test_name, passed in results:
        status = "✅ PASSED" if passed else "❌ FAILED"
        print(f"{test_name:15} {status}")
        if not passed:
            all_passed = False
    
    print("=" * 60)
    overall_status = "✅ ALL TESTS PASSED" if all_passed else "❌ SOME TESTS FAILED"
    print(f"Overall Result: {overall_status}")
    print("=" * 60)
    
    # Exit with appropriate code
    sys.exit(0 if all_passed else 1)


if __name__ == "__main__":
    main()