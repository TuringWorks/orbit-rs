#!/usr/bin/env python3
"""
Vector Commands Test Script for Orbit-RS Vector Store

Tests all VECTOR.* and FT.* commands implemented in the RESP protocol.
This script connects to the Orbit vector store and performs comprehensive testing.

Usage:
    python3 test_vector_commands.py

Requirements:
    pip install redis
"""

import redis
import json
import sys
import time
import numpy as np
from typing import List, Dict, Any

class VectorCommandsTester:
    def __init__(self, host='127.0.0.1', port=6381):
        """Initialize connection to Orbit vector store."""
        self.redis_client = redis.Redis(host=host, port=port, decode_responses=True)
        self.test_index = "test-vectors"
        self.ft_index = "ft-test-vectors"
        
    def connect(self):
        """Test connection to the vector store."""
        try:
            self.redis_client.ping()
            print("✅ Connected to Orbit Vector Store")
            return True
        except Exception as e:
            print(f"❌ Failed to connect: {e}")
            return False
    
    def generate_test_vector(self, dimension: int = 128, seed: int = 42) -> List[float]:
        """Generate a test vector with given dimension."""
        np.random.seed(seed)
        vector = np.random.rand(dimension).astype(float).tolist()
        return vector
    
    def format_vector(self, vector: List[float]) -> str:
        """Format vector for Redis commands."""
        return ','.join(f'{v:.6f}' for v in vector)
    
    def test_basic_vector_commands(self):
        """Test basic VECTOR.* commands."""
        print("\n📋 Testing Basic VECTOR.* Commands")
        
        # Generate test vectors
        vec1 = self.generate_test_vector(128, 1)
        vec2 = self.generate_test_vector(128, 2)
        vec3 = self.generate_test_vector(128, 3)
        
        test_cases = [
            # VECTOR.ADD
            {
                'command': 'VECTOR.ADD',
                'args': [self.test_index, 'doc1', self.format_vector(vec1), 'title', 'Document 1', 'category', 'AI'],
                'expected': 'OK'
            },
            {
                'command': 'VECTOR.ADD', 
                'args': [self.test_index, 'doc2', self.format_vector(vec2), 'title', 'Document 2', 'category', 'ML'],
                'expected': 'OK'
            },
            {
                'command': 'VECTOR.ADD',
                'args': [self.test_index, 'doc3', self.format_vector(vec3), 'title', 'Document 3', 'category', 'AI'],
                'expected': 'OK'
            },
            
            # VECTOR.COUNT
            {
                'command': 'VECTOR.COUNT',
                'args': [self.test_index],
                'expected': 3
            },
            
            # VECTOR.LIST
            {
                'command': 'VECTOR.LIST',
                'args': [self.test_index],
                'expected': ['doc1', 'doc2', 'doc3']  # Order may vary
            },
            
            # VECTOR.GET
            {
                'command': 'VECTOR.GET',
                'args': [self.test_index, 'doc1'],
                'expected': None  # Will check structure
            },
            
            # VECTOR.STATS
            {
                'command': 'VECTOR.STATS',
                'args': [self.test_index],
                'expected': None  # Will check structure
            }
        ]
        
        for test_case in test_cases:
            try:
                result = self.redis_client.execute_command(test_case['command'], *test_case['args'])
                
                if test_case['command'] == 'VECTOR.GET':
                    if result is not None and len(result) >= 2:
                        print(f"✅ {test_case['command']}: Found vector with {len(result)} fields")
                    else:
                        print(f"❌ {test_case['command']}: Invalid response format")
                elif test_case['command'] == 'VECTOR.STATS':
                    if isinstance(result, list) and len(result) >= 8:
                        print(f"✅ {test_case['command']}: Got stats with {len(result)} fields")
                    else:
                        print(f"❌ {test_case['command']}: Invalid stats format")
                elif test_case['command'] == 'VECTOR.LIST':
                    if isinstance(result, list) and len(result) == 3:
                        print(f"✅ {test_case['command']}: Got {len(result)} vector IDs")
                    else:
                        print(f"❌ {test_case['command']}: Expected 3 IDs, got {result}")
                else:
                    if result == test_case['expected']:
                        print(f"✅ {test_case['command']}: {result}")
                    else:
                        print(f"❌ {test_case['command']}: Expected {test_case['expected']}, got {result}")
                        
            except Exception as e:
                print(f"❌ {test_case['command']}: Error - {e}")
    
    def test_vector_search_commands(self):
        """Test vector search commands."""
        print("\n🔍 Testing Vector Search Commands")
        
        # Generate query vector similar to doc1
        query_vector = self.generate_test_vector(128, 1.1)  # Similar to doc1 but slightly different
        
        search_tests = [
            # VECTOR.SEARCH
            {
                'command': 'VECTOR.SEARCH',
                'args': [self.test_index, self.format_vector(query_vector), '5'],
                'expected_min_results': 1
            },
            {
                'command': 'VECTOR.SEARCH',
                'args': [self.test_index, self.format_vector(query_vector), '5', 'METRIC', 'COSINE'],
                'expected_min_results': 1
            },
            {
                'command': 'VECTOR.SEARCH',
                'args': [self.test_index, self.format_vector(query_vector), '2', 'METRIC', 'EUCLIDEAN'],
                'expected_min_results': 1
            },
            
            # VECTOR.KNN
            {
                'command': 'VECTOR.KNN',
                'args': [self.test_index, self.format_vector(query_vector), '3'],
                'expected_min_results': 1
            },
            {
                'command': 'VECTOR.KNN',
                'args': [self.test_index, self.format_vector(query_vector), '2', 'COSINE'],
                'expected_min_results': 1
            }
        ]
        
        for test_case in search_tests:
            try:
                result = self.redis_client.execute_command(test_case['command'], *test_case['args'])
                
                if isinstance(result, list) and len(result) >= test_case['expected_min_results']:
                    print(f"✅ {test_case['command']}: Found {len(result)} results")
                    
                    # Check result structure for first result
                    if len(result) > 0 and isinstance(result[0], list) and len(result[0]) >= 2:
                        print(f"   └─ First result: ID={result[0][0]}, Score={result[0][1]}")
                    else:
                        print(f"   └─ Warning: Unexpected result format")
                else:
                    print(f"❌ {test_case['command']}: Expected at least {test_case['expected_min_results']} results, got {len(result) if isinstance(result, list) else 'non-list'}")
                    
            except Exception as e:
                print(f"❌ {test_case['command']}: Error - {e}")
    
    def test_ft_commands(self):
        """Test FT.* (RedisSearch-compatible) commands."""
        print("\n🚀 Testing FT.* Commands")
        
        ft_tests = [
            # FT.CREATE
            {
                'command': 'FT.CREATE',
                'args': [self.ft_index, 'DIM', '128', 'DISTANCE_METRIC', 'COSINE'],
                'expected': 'OK'
            },
        ]
        
        # First run FT.CREATE
        for test_case in ft_tests:
            try:
                result = self.redis_client.execute_command(test_case['command'], *test_case['args'])
                if result == test_case['expected']:
                    print(f"✅ {test_case['command']}: {result}")
                else:
                    print(f"❌ {test_case['command']}: Expected {test_case['expected']}, got {result}")
            except Exception as e:
                print(f"❌ {test_case['command']}: Error - {e}")
                return
        
        # Generate test vectors for FT commands
        vec1 = self.generate_test_vector(128, 10)
        vec2 = self.generate_test_vector(128, 20)
        
        # Additional FT tests
        additional_ft_tests = [
            # FT.ADD
            {
                'command': 'FT.ADD',
                'args': [self.ft_index, 'ft-doc1', self.format_vector(vec1), 'title', 'FT Document 1'],
                'expected': 'OK'
            },
            {
                'command': 'FT.ADD',
                'args': [self.ft_index, 'ft-doc2', self.format_vector(vec2), 'title', 'FT Document 2'], 
                'expected': 'OK'
            },
            
            # FT.INFO
            {
                'command': 'FT.INFO',
                'args': [self.ft_index],
                'expected': None  # Will check structure
            },
            
            # FT.SEARCH
            {
                'command': 'FT.SEARCH',
                'args': [self.ft_index, self.format_vector(vec1), '5'],
                'expected_min_results': 1
            },
            
            # FT.DEL
            {
                'command': 'FT.DEL',
                'args': [self.ft_index, 'ft-doc2'],
                'expected': 1
            }
        ]
        
        for test_case in additional_ft_tests:
            try:
                result = self.redis_client.execute_command(test_case['command'], *test_case['args'])
                
                if test_case['command'] == 'FT.INFO':
                    if isinstance(result, list) and len(result) >= 4:
                        print(f"✅ {test_case['command']}: Got index info with {len(result)} fields")
                    else:
                        print(f"❌ {test_case['command']}: Invalid info format")
                elif test_case['command'] == 'FT.SEARCH':
                    if isinstance(result, list) and len(result) >= test_case.get('expected_min_results', 0):
                        print(f"✅ {test_case['command']}: Found {len(result)} results")
                    else:
                        print(f"❌ {test_case['command']}: Expected results, got {result}")
                else:
                    if result == test_case['expected']:
                        print(f"✅ {test_case['command']}: {result}")
                    else:
                        print(f"❌ {test_case['command']}: Expected {test_case['expected']}, got {result}")
                        
            except Exception as e:
                print(f"❌ {test_case['command']}: Error - {e}")
    
    def test_vector_deletion(self):
        """Test vector deletion."""
        print("\n🗑️  Testing Vector Deletion")
        
        # Test VECTOR.DEL
        try:
            result = self.redis_client.execute_command('VECTOR.DEL', self.test_index, 'doc1')
            if result == 1:
                print(f"✅ VECTOR.DEL: Successfully deleted vector")
            else:
                print(f"❌ VECTOR.DEL: Expected 1, got {result}")
                
            # Verify it's deleted
            result = self.redis_client.execute_command('VECTOR.GET', self.test_index, 'doc1')
            if result is None:
                print(f"✅ VECTOR.GET after delete: Vector properly deleted")
            else:
                print(f"❌ VECTOR.GET after delete: Vector still exists")
                
        except Exception as e:
            print(f"❌ VECTOR.DEL: Error - {e}")
    
    def test_edge_cases(self):
        """Test edge cases and error conditions."""
        print("\n⚠️  Testing Edge Cases")
        
        edge_cases = [
            # Invalid commands
            {
                'command': 'VECTOR.ADD',
                'args': [self.test_index],  # Too few arguments
                'should_error': True
            },
            {
                'command': 'VECTOR.GET',
                'args': [self.test_index, 'nonexistent'],
                'expected': None
            },
            {
                'command': 'VECTOR.DEL',
                'args': [self.test_index, 'nonexistent'],
                'expected': 0
            }
        ]
        
        for test_case in edge_cases:
            try:
                result = self.redis_client.execute_command(test_case['command'], *test_case['args'])
                
                if test_case.get('should_error', False):
                    print(f"❌ {test_case['command']}: Should have failed but got {result}")
                elif result == test_case.get('expected'):
                    print(f"✅ {test_case['command']}: Handled edge case correctly")
                else:
                    print(f"⚠️  {test_case['command']}: Got {result}, expected {test_case.get('expected')}")
                    
            except Exception as e:
                if test_case.get('should_error', False):
                    print(f"✅ {test_case['command']}: Correctly failed with error")
                else:
                    print(f"❌ {test_case['command']}: Unexpected error - {e}")
    
    def cleanup(self):
        """Clean up test data."""
        print("\n🧹 Cleaning up test data")
        try:
            # Note: In a real cleanup, we'd want to delete all test vectors
            # For now, we'll just acknowledge the cleanup step
            print("✅ Cleanup acknowledged (test vectors remain for inspection)")
        except Exception as e:
            print(f"⚠️  Cleanup warning: {e}")
    
    def run_all_tests(self):
        """Run all vector command tests."""
        print("🎯 Orbit-RS Vector Commands Test Suite")
        print("=" * 50)
        
        if not self.connect():
            return False
        
        try:
            self.test_basic_vector_commands()
            self.test_vector_search_commands()
            self.test_ft_commands()
            self.test_vector_deletion()
            self.test_edge_cases()
            self.cleanup()
            
            print("\n" + "=" * 50)
            print("🎉 Vector Commands Test Suite Complete!")
            print("\n💡 Usage Examples:")
            print("   Basic: VECTOR.ADD my-index doc1 \"1.0,2.0,3.0\" title \"My Document\"")
            print("   Search: VECTOR.SEARCH my-index \"1.0,2.0,3.0\" 5 METRIC COSINE")
            print("   KNN: VECTOR.KNN my-index \"1.0,2.0,3.0\" 3")
            print("   FT: FT.CREATE my-ft-index DIM 128 DISTANCE_METRIC COSINE")
            
            return True
            
        except KeyboardInterrupt:
            print("\n⏹️  Test interrupted by user")
            return False
        except Exception as e:
            print(f"\n💥 Test suite failed: {e}")
            return False

def main():
    """Main test function."""
    if len(sys.argv) > 1 and sys.argv[1] == '--help':
        print(__doc__)
        return
    
    tester = VectorCommandsTester()
    success = tester.run_all_tests()
    sys.exit(0 if success else 1)

if __name__ == '__main__':
    main()