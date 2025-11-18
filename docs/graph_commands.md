---
layout: default
title: Orbit Graph Database Commands
category: documentation
---

# Orbit Graph Database Commands

## Table of Contents

- [Overview](#overview)
- [Primary Graph Commands](#primary-graph-commands)
- [Diagnostics and Profiling](#diagnostics-and-profiling)
- [Configuration Management](#configuration-management)
- [Cypher Query Language](#cypher-query-language)
- [Graph Data Model](#graph-data-model)
- [Examples and Use Cases](#examples-and-use-cases)
- [Performance Considerations](#performance-considerations)
- [Migration from RedisGraph](#migration-from-redisgraph)

## Overview

Orbit's graph database implementation provides:

- **RedisGraph Compatibility**: Drop-in replacement for RedisGraph commands
- **Cypher Query Language**: Full support for Cypher syntax and semantics
- **High Performance**: Built on Orbit's distributed actor system
- **Horizontal Scalability**: Distributed graph processing across cluster nodes
- **Advanced Analytics**: Query profiling, execution plans, and performance metrics
- **Flexible Schema**: Schema-free graph model with dynamic properties
- **ACID Transactions**: Transactional consistency for graph operations

## Primary Graph Commands

### GRAPH.QUERY

Executes a read/write Cypher query against a named graph.

```
GRAPH.QUERY <graph_name> <query>
```

**Parameters:**

- `graph_name`: Name of the graph to query
- `query`: Cypher query string

**Returns:** Array containing:

1. Column headers
2. Result rows
3. Query statistics

**Examples:**

```redis

# Create nodes
GRAPH.QUERY social "CREATE (alice:Person {name: 'Alice', age: 30})"

# Create relationships
GRAPH.QUERY social "MATCH (a:Person {name: 'Alice'}) CREATE (a)-[:KNOWS]->(b:Person {name: 'Bob'})"

# Query with results
GRAPH.QUERY social "MATCH (p:Person) RETURN p.name, p.age"
```

### GRAPH.RO_QUERY

Executes a read-only Cypher query against a named graph.

```
GRAPH.RO_QUERY <graph_name> <query>
```

**Parameters:**

- `graph_name`: Name of the graph to query
- `query`: Read-only Cypher query string

**Returns:** Same format as GRAPH.QUERY

**Examples:**

```redis

# Read-only node matching
GRAPH.RO_QUERY social "MATCH (p:Person) RETURN p.name ORDER BY p.age"

# Read-only aggregation
GRAPH.RO_QUERY social "MATCH (p:Person) RETURN avg(p.age) AS average_age"

# This will fail - write operations not allowed
GRAPH.RO_QUERY social "CREATE (n:Person {name: 'Charlie'})"
```

### GRAPH.DELETE

Deletes the specified graph and all its entities (nodes and edges).

```
GRAPH.DELETE <graph_name>
```

**Parameters:**

- `graph_name`: Name of the graph to delete

**Returns:** "OK" on success

**Example:**

```redis
GRAPH.DELETE social
```

### GRAPH.LIST

Returns a list of all the graph keys managed by the graph database.

```
GRAPH.LIST
```

**Returns:** Array of graph names

**Example:**

```redis
GRAPH.LIST

# Returns: ["social", "recommendation", "fraud_detection"]
```

## Diagnostics and Profiling

### GRAPH.EXPLAIN

Displays the query execution plan for a Cypher query without actually running it.

```
GRAPH.EXPLAIN <graph_name> <query>
```

**Parameters:**

- `graph_name`: Name of the graph
- `query`: Cypher query to explain

**Returns:** Array of execution plan steps with cost estimates

**Example:**

```redis
GRAPH.EXPLAIN social "MATCH (p:Person)-[:KNOWS]->(f:Person) WHERE p.age > 25 RETURN f.name"

# Returns execution plan:
# 1: NodeScan - Scan nodes matching pattern
# 2: Filter - Apply WHERE clause filter  
# 3: Projection - Return f.name
# Total estimated cost: 15.00
```

### GRAPH.PROFILE

Runs a query and returns the execution plan, augmented with detailed performance metrics for each operation.

```
GRAPH.PROFILE <graph_name> <query>
```

**Parameters:**

- `graph_name`: Name of the graph
- `query`: Cypher query to profile

**Returns:** Array containing:

1. Query results (header)
2. Query results (rows)  
3. Execution plan with actual performance metrics

**Example:**

```redis
GRAPH.PROFILE social "MATCH (p:Person) WHERE p.age > 30 RETURN p.name"

# Returns:
# [header], [rows], [
#   "1: NodeScan - Records produced: 150, Execution time: 2 ms",
#   "2: Filter - Records produced: 45, Execution time: 1 ms", 
#   "3: Projection - Records produced: 45, Execution time: 0 ms",
#   "Total execution time: 3 ms"
# ]
```

### GRAPH.SLOWLOG

Returns the 10 slowest queries that have been executed against a specific graph.

```
GRAPH.SLOWLOG <graph_name>
```

**Parameters:**

- `graph_name`: Name of the graph

**Returns:** Array of slow query entries, each containing:

- Entry number
- Timestamp
- Execution time (milliseconds)
- Query string

**Example:**

```redis
GRAPH.SLOWLOG social

# Returns:
# [1, 1609459200000, 1500, "MATCH (p:Person)-[:KNOWS*1..5]->(f) RETURN count(f)"]
# [2, 1609459180000, 850, "MATCH (p:Person) WHERE p.name =~ '.*Smith.*' RETURN p"]
```

## Configuration Management

### GRAPH.CONFIG GET

Retrieves the value of a specific graph database configuration parameter.

```
GRAPH.CONFIG GET <parameter>
```

**Parameters:**

- `parameter`: Configuration parameter name

**Supported Parameters:**

- `QUERY_TIMEOUT`: Query execution timeout in milliseconds
- `MAX_NODES`: Maximum number of nodes per graph
- `MAX_RELATIONSHIPS`: Maximum number of relationships per graph  
- `PROFILING_ENABLED`: Whether query profiling is enabled
- `SLOW_QUERY_THRESHOLD`: Slow query threshold in milliseconds
- `MEMORY_LIMIT`: Memory limit for graph operations in bytes

**Example:**

```redis
GRAPH.CONFIG GET QUERY_TIMEOUT

# Returns: "30000"

GRAPH.CONFIG GET PROFILING_ENABLED  

# Returns: "false"
```

### GRAPH.CONFIG SET

Changes the value of a graph database configuration parameter.

```
GRAPH.CONFIG SET <parameter> <value>
```

**Parameters:**

- `parameter`: Configuration parameter name
- `value`: New parameter value

**Example:**

```redis
GRAPH.CONFIG SET QUERY_TIMEOUT 60000

# Returns: "OK"

GRAPH.CONFIG SET PROFILING_ENABLED true

# Returns: "OK"
```

## Cypher Query Language

Orbit's graph database supports a comprehensive subset of the Cypher query language:

### Node Patterns

```cypher
-- Simple node
(n)

-- Node with label
(n:Person)

-- Node with properties
(n:Person {name: 'Alice', age: 30})

-- Node with variable, label, and properties
(alice:Person {name: 'Alice'})
```

### Relationship Patterns

```cypher
-- Undirected relationship
(a)-[r]-(b)

-- Directed relationship
(a)-[r]->(b)

-- Relationship with type
(a)-[r:KNOWS]->(b)

-- Relationship with properties
(a)-[r:KNOWS {since: 2020}]->(b)
```

### Supported Clauses

- **CREATE**: Create nodes and relationships
- **MATCH**: Pattern matching
- **WHERE**: Filtering conditions
- **RETURN**: Result projection
- **SET**: Update properties
- **DELETE**: Remove nodes/relationships
- **MERGE**: Create or match

### Supported Functions

- **count()**: Count aggregation
- **avg()**: Average aggregation
- **sum()**: Sum aggregation
- **min()**: Minimum value
- **max()**: Maximum value

## Graph Data Model

### Nodes

Nodes represent entities in the graph:

```cypher
-- Create a person node
CREATE (p:Person {
  name: 'Alice Smith',
  age: 30,
  email: 'alice@example.com',
  active: true
})
```

**Properties:**

- Nodes can have multiple labels
- Properties are key-value pairs
- Supported types: string, integer, float, boolean
- Properties are dynamically typed

### Relationships

Relationships connect nodes:

```cypher
-- Create a relationship
MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
CREATE (a)-[:FRIENDS {since: 2018, strength: 0.8}]->(b)
```

**Properties:**

- Relationships have a type (e.g., FRIENDS, WORKS_FOR)
- Relationships are directed
- Can have properties like nodes
- Connect exactly two nodes

### Labels and Types

```cypher
-- Multiple labels on nodes
CREATE (p:Person:Employee:Manager {name: 'Alice'})

-- Relationship types are always singular
CREATE (a)-[:WORKS_FOR]->(company)
CREATE (a)-[:MANAGES]->(team)
```

## Examples and Use Cases

### Social Network

```cypher
-- Create social network
CREATE (alice:Person {name: 'Alice', age: 28, location: 'NYC'})
CREATE (bob:Person {name: 'Bob', age: 32, location: 'LA'})
CREATE (carol:Person {name: 'Carol', age: 25, location: 'NYC'})

-- Create friendships
CREATE (alice)-[:FRIENDS {since: 2020}]->(bob)
CREATE (alice)-[:FRIENDS {since: 2019}]->(carol)
CREATE (bob)-[:FRIENDS {since: 2021}]->(carol)

-- Find mutual friends
MATCH (a:Person {name: 'Alice'})-[:FRIENDS]->(mutual)<-[:FRIENDS]-(other:Person)
WHERE other.name <> 'Alice'
RETURN other.name AS mutual_friend, mutual.name AS through

-- Find people in the same city
MATCH (p1:Person)-[:FRIENDS]-(p2:Person)
WHERE p1.location = p2.location
RETURN p1.name, p2.name, p1.location
```

### Recommendation Engine

```cypher
-- Create product catalog
CREATE (laptop:Product {name: 'Laptop Pro', category: 'Electronics', price: 1299})
CREATE (mouse:Product {name: 'Wireless Mouse', category: 'Electronics', price: 49})
CREATE (book:Product {name: 'Graph Databases', category: 'Books', price: 39})

-- Create user purchases
CREATE (user:User {name: 'Alice'})
CREATE (user)-[:PURCHASED {date: '2023-01-15', rating: 5}]->(laptop)
CREATE (user)-[:PURCHASED {date: '2023-01-20', rating: 4}]->(book)

-- Find product recommendations
MATCH (u:User {name: 'Alice'})-[:PURCHASED]->(p:Product)
MATCH (p)<-[:PURCHASED]-(other:User)-[:PURCHASED]->(rec:Product)
WHERE NOT (u)-[:PURCHASED]->(rec)
RETURN rec.name, rec.category, count(*) AS recommendation_score
ORDER BY recommendation_score DESC
```

### Fraud Detection

```cypher
-- Create transaction network
CREATE (acc1:Account {id: 'ACC001', type: 'checking'})
CREATE (acc2:Account {id: 'ACC002', type: 'savings'}) 
CREATE (acc3:Account {id: 'ACC003', type: 'checking'})

-- Create transactions
CREATE (acc1)-[:TRANSFER {amount: 5000, timestamp: 1609459200}]->(acc2)
CREATE (acc2)-[:TRANSFER {amount: 4900, timestamp: 1609459260}]->(acc3)
CREATE (acc3)-[:TRANSFER {amount: 4800, timestamp: 1609459320}]->(acc1)

-- Detect circular money flow (potential fraud)
MATCH path = (a1:Account)-[:TRANSFER*3..5]->(a1)
WHERE all(r IN relationships(path) WHERE r.amount > 1000)
RETURN path, length(path) AS hops
```

### Knowledge Graph

```cypher
-- Create knowledge entities
CREATE (python:Language {name: 'Python', type: 'programming'})
CREATE (django:Framework {name: 'Django', type: 'web'})
CREATE (alice:Developer {name: 'Alice', experience: 5})
CREATE (webapp:Project {name: 'WebApp', status: 'active'})

-- Create relationships
CREATE (alice)-[:KNOWS {proficiency: 'expert'}]->(python)
CREATE (alice)-[:USES {proficiency: 'intermediate'}]->(django)
CREATE (django)-[:BUILT_WITH]->(python)
CREATE (webapp)-[:IMPLEMENTED_IN]->(django)
CREATE (alice)-[:WORKS_ON]->(webapp)

-- Find technology stack for a project
MATCH (p:Project {name: 'WebApp'})-[:IMPLEMENTED_IN*]->(tech)
RETURN p.name, collect(tech.name) AS tech_stack

-- Find developers with specific skills
MATCH (d:Developer)-[k:KNOWS]->(lang:Language {name: 'Python'})
WHERE k.proficiency IN ['expert', 'advanced']
RETURN d.name, d.experience, k.proficiency
```

## Performance Considerations

### Query Optimization

```cypher
-- Use specific labels and properties for efficient filtering
MATCH (p:Person {email: 'alice@example.com'}) RETURN p
-- Better than: MATCH (p) WHERE p.email = 'alice@example.com'

-- Use LIMIT for large result sets
MATCH (p:Person) RETURN p.name LIMIT 100

-- Index frequently queried properties (conceptual - implementation dependent)
-- CREATE INDEX ON :Person(email)
```

### Memory Management

- Use appropriate data types for properties
- Avoid storing large text/binary data as node properties
- Consider relationship direction for traversal efficiency
- Monitor memory usage with GRAPH.CONFIG GET MEMORY_LIMIT

### Scaling

- Distribute graphs across multiple Orbit nodes
- Use consistent graph naming for cross-node operations
- Consider data locality for related subgraphs
- Monitor slow queries with GRAPH.SLOWLOG

## Migration from RedisGraph

Orbit's graph database is designed to be a drop-in replacement for RedisGraph:

### Command Compatibility

- ✅ All core GRAPH.* commands supported
- ✅ Same Cypher syntax and semantics
- ✅ Compatible result formats
- ✅ Same configuration parameters

### Key Differences

- **Distributed Architecture**: Built on Orbit's actor system for horizontal scaling
- **Enhanced Performance**: Consistent performance under high load
- **Advanced Profiling**: More detailed execution metrics
- **Actor Integration**: Native integration with other Orbit data types

### Migration Steps

1. **Backup RedisGraph Data**: Export existing graphs to Cypher scripts
2. **Update Connection**: Point applications to Orbit RESP endpoint
3. **Test Queries**: Verify query compatibility and performance
4. **Import Data**: Execute Cypher scripts to recreate graphs
5. **Monitor Performance**: Use GRAPH.PROFILE and GRAPH.SLOWLOG

### Example Migration Script

```python
import redis

# Connect to Orbit
orbit_client = redis.Redis(host='orbit-server', port=6379)

# Migrate a graph
migration_queries = [
    "CREATE (alice:Person {name: 'Alice', age: 30})",
    "CREATE (bob:Person {name: 'Bob', age: 25})",
    "MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'}) CREATE (a)-[:KNOWS]->(b)"
]

for query in migration_queries:
    result = orbit_client.execute_command('GRAPH.QUERY', 'migrated_graph', query)
    print(f"Executed: {query[:50]}... -> {result}")
```

## Integration Examples

### Python with redis-py

```python
import redis

# Connect to Orbit
r = redis.Redis(host='localhost', port=6379)

# Create graph data
r.execute_command('GRAPH.QUERY', 'social', 
    "CREATE (alice:Person {name: 'Alice', age: 30}) RETURN alice")

# Query graph
result = r.execute_command('GRAPH.QUERY', 'social',
    "MATCH (p:Person) RETURN p.name, p.age")

print("Query result:", result)
```

### Node.js with ioredis

```javascript
const Redis = require('ioredis');
const redis = new Redis('redis://localhost:6379');

async function graphExample() {
  // Create nodes and relationships
  await redis.call('GRAPH.QUERY', 'social',
    "CREATE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'}) RETURN a, b");
  
  // Query the graph
  const result = await redis.call('GRAPH.QUERY', 'social',
    "MATCH (p:Person) RETURN p.name ORDER BY p.name");
  
  console.log('Graph query result:', result);
}
```

### Java with Jedis

```java
import redis.clients.jedis.Jedis;

public class GraphExample {
    public static void main(String[] args) {
        Jedis jedis = new Jedis("localhost", 6379);
        
        // Create graph data
        jedis.sendCommand(() -> "GRAPH.QUERY".getBytes(), 
            "social".getBytes(),
            "CREATE (p:Person {name: 'Alice', profession: 'Engineer'}) RETURN p".getBytes());
        
        // Query graph
        Object result = jedis.sendCommand(() -> "GRAPH.QUERY".getBytes(),
            "social".getBytes(), 
            "MATCH (p:Person) RETURN p.name, p.profession".getBytes());
        
        System.out.println("Result: " + result);
    }
}
```

## Advanced Features

### Graph Analytics

```cypher
-- Centrality analysis (conceptual)
MATCH (p:Person)-[:KNOWS]-(connected)
RETURN p.name, count(connected) AS degree_centrality
ORDER BY degree_centrality DESC

-- Path finding
MATCH path = shortestPath((a:Person {name: 'Alice'})-[:KNOWS*]-(b:Person {name: 'Charlie'}))
RETURN path, length(path) AS distance

-- Community detection (simplified)
MATCH (p:Person)-[:KNOWS]-(friends)
WITH p, collect(friends) AS friend_list
RETURN p.name, size(friend_list) AS friend_count
ORDER BY friend_count DESC
```

### Performance Monitoring

```cypher
-- Monitor query performance
GRAPH.PROFILE social "MATCH (p:Person)-[:WORKS_FOR]->(c:Company) RETURN p.name, c.name"

-- Check slow queries
GRAPH.SLOWLOG social

-- Get execution plan
GRAPH.EXPLAIN social "MATCH (p:Person) WHERE p.age > 30 RETURN count(p)"
```

---

For more information about Orbit's graph database capabilities, see the [Orbit documentation](./README.md) or explore the [examples](./examples/) directory.
