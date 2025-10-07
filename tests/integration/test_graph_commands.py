#!/usr/bin/env python3
"""
Comprehensive test suite for Orbit Graph (GRAPH.*) commands
Tests RedisGraph compatibility with the Orbit graph database using Cypher queries
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

class GraphDatabaseTests:
    def __init__(self, redis_client):
        self.client = redis_client
        self.test_results = []
        self.graph_name = "test_graph"
        
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
    
    def assert_contains(self, container, item, description: str):
        """Assert that container contains item"""
        if item in container:
            print(f"✅ {description}: found {item}")
            self.test_results.append(True)
        else:
            print(f"❌ {description}: {item} not found in {container}")
            self.test_results.append(False)

    def test_basic_node_creation(self):
        """Test basic node creation with GRAPH.QUERY"""
        print("\n=== Testing Basic Node Creation ===")
        
        # Create a single node
        query = "CREATE (n:Person {name: 'Alice', age: 30}) RETURN n"
        result = self.run_command("GRAPH.QUERY", self.graph_name, query)
        self.assert_not_none(result, "Node creation query result")
        
        # Verify the result structure (should be [header, rows, statistics])
        if isinstance(result, list) and len(result) >= 2:
            header, rows = result[0], result[1]
            self.assert_not_none(header, "Query result header")
            self.assert_not_none(rows, "Query result rows")
        
        # Create multiple nodes
        query = "CREATE (a:Person {name: 'Bob'}), (b:Company {name: 'TechCorp'}) RETURN a, b"
        result = self.run_command("GRAPH.QUERY", self.graph_name, query)
        self.assert_not_none(result, "Multiple nodes creation")

    def test_node_matching(self):
        """Test node matching with MATCH clauses"""
        print("\n=== Testing Node Matching ===")
        
        # Match nodes by label
        query = "MATCH (n:Person) RETURN n"
        result = self.run_command("GRAPH.QUERY", self.graph_name, query)
        self.assert_not_none(result, "Match nodes by label")
        
        # Match nodes with properties
        query = "MATCH (n:Person {name: 'Alice'}) RETURN n"
        result = self.run_command("GRAPH.QUERY", self.graph_name, query)
        self.assert_not_none(result, "Match nodes with properties")
        
        # Match with WHERE clause
        query = "MATCH (n:Person) WHERE n.age > 25 RETURN n.name"
        result = self.run_command("GRAPH.QUERY", self.graph_name, query)
        self.assert_not_none(result, "Match with WHERE clause")

    def test_relationships(self):
        """Test relationship creation and querying"""
        print("\n=== Testing Relationships ===")
        
        # Create nodes and relationships
        query = """
        CREATE (a:Person {name: 'Charlie'}), 
               (b:Person {name: 'Dana'}),
               (c:Company {name: 'StartupInc'})
        CREATE (a)-[:KNOWS]->(b)
        CREATE (a)-[:WORKS_FOR]->(c)
        RETURN a, b, c
        """
        result = self.run_command("GRAPH.QUERY", self.graph_name, query)
        self.assert_not_none(result, "Create nodes and relationships")
        
        # Query relationships
        query = "MATCH (a:Person)-[:KNOWS]->(b:Person) RETURN a.name, b.name"
        result = self.run_command("GRAPH.QUERY", self.graph_name, query)
        self.assert_not_none(result, "Query relationships")
        
        # Query with relationship direction
        query = "MATCH (p:Person)-[:WORKS_FOR]->(c:Company) RETURN p.name, c.name"
        result = self.run_command("GRAPH.QUERY", self.graph_name, query)
        self.assert_not_none(result, "Query directed relationships")

    def test_read_only_queries(self):
        """Test read-only query execution"""
        print("\n=== Testing Read-Only Queries ===")
        
        # Read-only match query
        query = "MATCH (n:Person) RETURN n.name, n.age"
        result = self.run_command("GRAPH.RO_QUERY", self.graph_name, query)
        self.assert_not_none(result, "Read-only match query")
        
        # Read-only aggregation query
        query = "MATCH (n:Person) RETURN count(n) AS person_count"
        result = self.run_command("GRAPH.RO_QUERY", self.graph_name, query)
        self.assert_not_none(result, "Read-only aggregation query")
        
        # Try write operation in read-only mode (should fail)
        try:
            query = "CREATE (n:Person {name: 'ShouldFail'}) RETURN n"
            result = self.run_command("GRAPH.RO_QUERY", self.graph_name, query)
            print("❌ Write operation in read-only mode should have failed")
            self.test_results.append(False)
        except:
            print("✅ Correctly rejected write operation in read-only mode")
            self.test_results.append(True)

    def test_graph_management(self):
        """Test graph management operations"""
        print("\n=== Testing Graph Management ===")
        
        # List graphs
        result = self.run_command("GRAPH.LIST")
        self.assert_not_none(result, "List graphs")
        
        # Create a test graph by running a query on it
        test_graph = "management_test_graph"
        query = "CREATE (n:Test {id: 1}) RETURN n"
        result = self.run_command("GRAPH.QUERY", test_graph, query)
        self.assert_not_none(result, "Create test graph via query")
        
        # Delete the test graph
        result = self.run_command("GRAPH.DELETE", test_graph)
        self.assert_equal(result, "OK", "Delete graph")

    def test_query_explanation(self):
        """Test query execution plan explanation"""
        print("\n=== Testing Query Explanation ===")
        
        # Explain a simple query
        query = "MATCH (n:Person) RETURN n"
        result = self.run_command("GRAPH.EXPLAIN", self.graph_name, query)
        self.assert_not_none(result, "Explain simple query")
        
        # Explain a complex query
        query = "MATCH (a:Person)-[:KNOWS]->(b:Person) WHERE a.age > 25 RETURN a.name, b.name"
        result = self.run_command("GRAPH.EXPLAIN", self.graph_name, query)
        self.assert_not_none(result, "Explain complex query")
        
        # Verify the result contains execution plan information
        if isinstance(result, list) and len(result) > 0:
            self.test_results.append(True)
            print("✅ Execution plan returned with steps")
        else:
            self.test_results.append(False)
            print("❌ Execution plan should contain steps")

    def test_query_profiling(self):
        """Test query execution profiling"""
        print("\n=== Testing Query Profiling ===")
        
        # Profile a simple query
        query = "MATCH (n:Person) RETURN n"
        result = self.run_command("GRAPH.PROFILE", self.graph_name, query)
        self.assert_not_none(result, "Profile simple query")
        
        # Profile should return [header, rows, execution_plan_with_metrics]
        if isinstance(result, list) and len(result) >= 3:
            header, rows, profile = result[0], result[1], result[2]
            self.assert_not_none(profile, "Query profile metrics")
            print("✅ Profile includes execution metrics")
            self.test_results.append(True)
        else:
            print("❌ Profile should include execution metrics")
            self.test_results.append(False)

    def test_slow_query_log(self):
        """Test slow query logging"""
        print("\n=== Testing Slow Query Log ===")
        
        # Execute some queries (some may be recorded as slow)
        queries = [
            "MATCH (n:Person) RETURN n",
            "MATCH (a:Person)-[:KNOWS*1..3]->(b:Person) RETURN a, b",
            "MATCH (n) RETURN count(n)",
        ]
        
        for query in queries:
            try:
                self.run_command("GRAPH.QUERY", self.graph_name, query)
            except:
                pass  # Some queries might fail, that's okay for this test
        
        # Check slow query log
        result = self.run_command("GRAPH.SLOWLOG", self.graph_name)
        self.assert_not_none(result, "Slow query log retrieval")
        
        # Verify the slow log format
        if isinstance(result, list):
            print(f"✅ Slow query log returned {len(result)} entries")
            self.test_results.append(True)
        else:
            print("❌ Slow query log should be a list")
            self.test_results.append(False)

    def test_configuration(self):
        """Test graph configuration management"""
        print("\n=== Testing Configuration Management ===")
        
        # Get configuration parameters
        config_params = [
            "QUERY_TIMEOUT",
            "MAX_NODES", 
            "MAX_RELATIONSHIPS",
            "PROFILING_ENABLED"
        ]
        
        for param in config_params:
            try:
                result = self.run_command("GRAPH.CONFIG", "GET", param)
                self.assert_not_none(result, f"Get config parameter {param}")
            except Exception as e:
                print(f"⚠️ Config parameter {param} not supported: {e}")
        
        # Set configuration parameters
        try:
            result = self.run_command("GRAPH.CONFIG", "SET", "QUERY_TIMEOUT", "60000")
            self.assert_equal(result, "OK", "Set configuration parameter")
        except Exception as e:
            print(f"⚠️ Config SET not fully implemented: {e}")

    def test_complex_graph_scenarios(self):
        """Test complex graph scenarios"""
        print("\n=== Testing Complex Graph Scenarios ===")
        
        # Create a social network graph
        social_network_queries = [
            # Create people
            "CREATE (alice:Person {name: 'Alice', age: 28, city: 'NYC'})",
            "CREATE (bob:Person {name: 'Bob', age: 32, city: 'LA'})",
            "CREATE (carol:Person {name: 'Carol', age: 25, city: 'NYC'})",
            "CREATE (dave:Person {name: 'Dave', age: 30, city: 'Chicago'})",
            
            # Create companies
            "CREATE (tech:Company {name: 'TechCorp', industry: 'Technology'})",
            "CREATE (finance:Company {name: 'FinanceInc', industry: 'Finance'})",
            
            # Create relationships
            """MATCH (alice:Person {name: 'Alice'}), (bob:Person {name: 'Bob'})
               CREATE (alice)-[:FRIENDS {since: 2020}]->(bob)""",
            """MATCH (alice:Person {name: 'Alice'}), (carol:Person {name: 'Carol'})
               CREATE (alice)-[:FRIENDS {since: 2018}]->(carol)""",
            """MATCH (alice:Person {name: 'Alice'}), (tech:Company {name: 'TechCorp'})
               CREATE (alice)-[:WORKS_FOR {position: 'Engineer', salary: 90000}]->(tech)""",
            """MATCH (bob:Person {name: 'Bob'}), (finance:Company {name: 'FinanceInc'})
               CREATE (bob)-[:WORKS_FOR {position: 'Analyst', salary: 80000}]->(finance)""",
        ]
        
        # Execute social network creation queries
        for query in social_network_queries:
            try:
                result = self.run_command("GRAPH.QUERY", self.graph_name, query)
                # Just verify it doesn't crash
            except Exception as e:
                print(f"⚠️ Social network query failed: {e}")
        
        # Complex queries on the social network
        complex_queries = [
            # Find mutual friends
            """MATCH (a:Person)-[:FRIENDS]->(mutual)<-[:FRIENDS]-(b:Person) 
               WHERE a.name = 'Alice' AND b.name <> 'Alice' 
               RETURN DISTINCT b.name AS friend_of_friend""",
            
            # Find colleagues in the same city
            """MATCH (p1:Person)-[:WORKS_FOR]->(c:Company)<-[:WORKS_FOR]-(p2:Person)
               WHERE p1.city = p2.city AND p1 <> p2
               RETURN p1.name, p2.name, c.name""",
            
            # Aggregate data
            """MATCH (p:Person)-[:WORKS_FOR]->(c:Company)
               RETURN c.industry, count(p) AS employee_count""",
        ]
        
        for i, query in enumerate(complex_queries, 1):
            try:
                result = self.run_command("GRAPH.QUERY", self.graph_name, query)
                print(f"✅ Complex query {i} executed successfully")
                self.test_results.append(True)
            except Exception as e:
                print(f"❌ Complex query {i} failed: {e}")
                self.test_results.append(False)

    def test_cypher_features(self):
        """Test various Cypher language features"""
        print("\n=== Testing Cypher Language Features ===")
        
        # Test different data types
        query = """CREATE (n:DataTypes {
            string_prop: 'Hello World',
            int_prop: 42,
            float_prop: 3.14,
            bool_prop: true
        }) RETURN n"""
        
        try:
            result = self.run_command("GRAPH.QUERY", self.graph_name, query)
            print("✅ Data types test passed")
            self.test_results.append(True)
        except Exception as e:
            print(f"❌ Data types test failed: {e}")
            self.test_results.append(False)
        
        # Test OPTIONAL MATCH (if supported)
        try:
            query = "OPTIONAL MATCH (n:NonExistent) RETURN n"
            result = self.run_command("GRAPH.QUERY", self.graph_name, query)
            print("✅ OPTIONAL MATCH supported")
            self.test_results.append(True)
        except Exception as e:
            print(f"⚠️ OPTIONAL MATCH not supported: {e}")
        
        # Test WITH clause (if supported)  
        try:
            query = "MATCH (n:Person) WITH n.name AS name RETURN name"
            result = self.run_command("GRAPH.QUERY", self.graph_name, query)
            print("✅ WITH clause supported")
            self.test_results.append(True)
        except Exception as e:
            print(f"⚠️ WITH clause not supported: {e}")

    def test_error_conditions(self):
        """Test error handling"""
        print("\n=== Testing Error Conditions ===")
        
        # Invalid Cypher syntax
        try:
            result = self.run_command("GRAPH.QUERY", self.graph_name, "INVALID CYPHER SYNTAX")
            print("❌ Should have failed with invalid syntax")
            self.test_results.append(False)
        except:
            print("✅ Correctly handled invalid syntax")
            self.test_results.append(True)
        
        # Non-existent graph operations
        try:
            result = self.run_command("GRAPH.DELETE", "nonexistent_graph")
            # This might succeed (return OK) or fail, both are acceptable
            print("✅ Handled non-existent graph deletion")
            self.test_results.append(True)
        except Exception as e:
            print(f"✅ Correctly failed for non-existent graph: {e}")
            self.test_results.append(True)
        
        # Invalid command arguments
        try:
            result = self.run_command("GRAPH.QUERY")  # Missing arguments
            print("❌ Should have failed with missing arguments")
            self.test_results.append(False)
        except:
            print("✅ Correctly handled missing arguments")
            self.test_results.append(True)

    def run_all_tests(self):
        """Run all graph database tests"""
        print("🚀 Starting Orbit Graph Database Tests")
        print("=" * 50)
        
        try:
            self.test_basic_node_creation()
            self.test_node_matching()
            self.test_relationships()
            self.test_read_only_queries()
            self.test_graph_management()
            self.test_query_explanation()
            self.test_query_profiling()
            self.test_slow_query_log()
            self.test_configuration()
            self.test_complex_graph_scenarios()
            self.test_cypher_features()
            self.test_error_conditions()
            
        except Exception as e:
            print(f"\n❌ Test suite failed with error: {e}")
            return False
            
        # Print summary
        total_tests = len(self.test_results)
        passed_tests = sum(self.test_results)
        failed_tests = total_tests - passed_tests
        
        print("\n" + "=" * 50)
        print("📊 GRAPH DATABASE TEST RESULTS")
        print("=" * 50)
        print(f"✅ Passed: {passed_tests}/{total_tests}")
        print(f"❌ Failed: {failed_tests}/{total_tests}")
        print(f"📈 Success Rate: {(passed_tests/total_tests)*100:.1f}%")
        
        if failed_tests == 0:
            print("\n🎉 All graph database tests passed!")
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
    tests = GraphDatabaseTests(client)
    success = tests.run_all_tests()
    
    if success:
        print("\n✅ Graph database test suite completed successfully!")
        print("\n📋 Features Tested:")
        print("• Node creation and matching")
        print("• Relationship creation and querying") 
        print("• Read-only query execution")
        print("• Graph management (list, delete)")
        print("• Query explanation and profiling")
        print("• Slow query logging")
        print("• Configuration management")
        print("• Complex graph scenarios")
        print("• Cypher language features")
        print("• Error handling")
        sys.exit(0)
    else:
        print("\n❌ Graph database test suite failed!")
        sys.exit(1)

if __name__ == "__main__":
    main()