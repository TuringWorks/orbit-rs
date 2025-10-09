# Query Languages Comparison Guide

## Table of Contents

1. [Overview](#overview)
2. [Language Comparison Matrix](#language-comparison-matrix)
3. [Cypher](#cypher)
4. [AQL (ArrangoDB Query Language)](#aql-arrangodb-query-language)
5. [OrbitQL](#orbitql)
6. [Use Case Recommendations](#use-case-recommendations)
7. [Syntax Comparisons](#syntax-comparisons)
8. [Performance Characteristics](#performance-characteristics)
9. [Migration Guide](#migration-guide)
10. [Best Practices](#best-practices)

## Overview

### Why Multiple Query Languages?

Orbit-RS supports multiple query languages to provide:

- **Ecosystem compatibility**: Work with existing tools and applications
- **Developer familiarity**: Use languages you already know
- **Specialized optimization**: Each language optimized for specific patterns
- **Flexibility**: Choose the best tool for each task

### Language Philosophy

| Language | Philosophy | Primary Focus |
|----------|------------|---------------|
| **Cypher** | Graph-first declarative querying | Graph pattern matching and traversal |
| **AQL** | Multi-model document-centric | Document manipulation with graph capabilities |
| **OrbitQL** | Unified multi-model with SQL familiarity | Cross-model queries with SQL comfort |

## Language Comparison Matrix

| Feature | Cypher | AQL | OrbitQL |
|---------|--------|-----|---------|
| **Graph Traversal** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| **Pattern Matching** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ |
| **Document Queries** | ⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| **Time Series** | ⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| **SQL Familiarity** | ⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| **Advanced SQL Features** | ⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| **Aggregations** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| **Distributed Queries** | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| **Actor Integration** | ⭐⭐ | ⭐⭐ | ⭐⭐⭐⭐⭐ |
| **Neo4j Compatibility** | ⭐⭐⭐⭐⭐ | ⭐ | ⭐⭐ |
| **ArangoDB Compatibility** | ⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐ |
| **Learning Curve** | Medium | Medium | Easy (for SQL users) |

## Cypher

### Strengths

- **Graph-native**: Designed specifically for graph pattern matching
- **Neo4j ecosystem**: Full compatibility with Neo4j tools and drivers
- **Intuitive patterns**: Visual representation of graph relationships
- **Rich traversal**: Advanced path finding and pattern matching

### Best For

- Graph analytics and social networks
- Knowledge graphs and recommendation systems
- Path finding and network analysis
- Neo4j migrations and integrations

### Example

```cypher
// Find influential users in a social network
MATCH (user:Person)-[r:FOLLOWS]->(influencer:Person)
WHERE influencer.follower_count > 10000
WITH influencer, count(r) as influence_score
ORDER BY influence_score DESC
LIMIT 10
RETURN influencer.name, influencer.follower_count, influence_score
```

### Limitations

- Limited document/JSON manipulation
- No native time series support
- Less familiar to SQL developers
- Focused primarily on graph operations

## AQL (ArrangoDB Query Language)

### Strengths

- **Multi-model**: Native support for documents, graphs, and key-value
- **Powerful aggregations**: Advanced statistical and analytical functions
- **Flexible syntax**: Combines declarative and functional programming
- **ArangoDB compatibility**: Full feature parity with ArangoDB

### Best For

- Document-centric applications with graph relationships
- Complex analytical queries across multiple data types
- ArangoDB migrations and integrations
- Applications requiring flexible data modeling

### Example

```aql
// Analyze user behavior across documents and graphs
FOR user IN users
  FILTER user.active == true
  LET friends = (
    FOR friend IN 1..1 OUTBOUND user follows
      RETURN friend
  )
  FOR order IN orders
    FILTER order.customer_id == user._id
    COLLECT user_info = user INTO user_orders = order
    RETURN {
      user: user_info.name,
      friend_count: LENGTH(friends),
      total_spent: SUM(user_orders[*].amount),
      avg_order: AVERAGE(user_orders[*].amount)
    }
```

### Limitations

- Learning curve for developers new to functional syntax
- Less intuitive for pure graph operations
- Limited time series capabilities
- Not as familiar as SQL for most developers

## OrbitQL

### Strengths

- **Advanced SQL compatibility**: Full support for CTEs, CASE expressions, temporal functions
- **SQL familiarity**: Builds on well-known SQL foundations with modern extensions
- **Multi-model unified**: Single syntax for all data models with cross-model JOINs
- **Temporal functions**: Native NOW(), INTERVAL, and time-based analytics
- **Enhanced aggregates**: COUNT(DISTINCT), conditional aggregates, complex expressions
- **Null-handling**: COALESCE and advanced null manipulation
- **Distributed by design**: Optimized for cross-node operations
- **Actor integration**: Direct access to Orbit actor system

### Best For

- Applications requiring multiple data models
- Time series analytics and IoT data processing
- Teams with strong SQL background
- Complex distributed queries
- Real-time analytics dashboards

### Example

```orbitql
-- Advanced multi-model query with CTEs, CASE expressions, and temporal functions
WITH recent_activity AS (
    SELECT 
        user_id,
        COUNT(DISTINCT session_id) as unique_sessions,
        AVG(CASE WHEN metric_name = 'cpu_usage' THEN value END) as avg_cpu,
        COUNT(CASE WHEN timestamp > NOW() - INTERVAL '1 hour' THEN 1 END) as recent_events
    FROM metrics 
    WHERE timestamp >= NOW() - INTERVAL '24 hours'
    GROUP BY user_id
    HAVING COUNT(*) > 10
),
user_segments AS (
    SELECT 
        u.id,
        u.name,
        CASE 
            WHEN ra.unique_sessions > 50 THEN 'power_user'
            WHEN ra.unique_sessions > 10 THEN 'regular_user'
            ELSE 'casual_user'
        END as user_type,
        COUNT(DISTINCT f.to_user_id) as follower_count
    FROM users u
    JOIN recent_activity ra ON u.id = ra.user_id
    LEFT JOIN follows f ON u.id = f.to_user_id
    GROUP BY u.id, u.name, ra.unique_sessions
)
SELECT 
    us.name,
    us.user_type,
    us.follower_count,
    COALESCE(ra.avg_cpu, 0) as avg_cpu_usage,
    ra.recent_events,
    CASE 
        WHEN us.follower_count > 1000 AND ra.avg_cpu > 80 THEN 'high_impact'
        WHEN us.follower_count > 100 OR ra.avg_cpu > 50 THEN 'medium_impact'
        ELSE 'low_impact'
    END as impact_level
FROM user_segments us
JOIN recent_activity ra ON us.id = ra.user_id
WHERE us.user_type != 'casual_user'
ORDER BY us.follower_count DESC, ra.avg_cpu DESC
LIMIT 20;
```

### Limitations

- Newer language with smaller ecosystem
- Some advanced graph operations may be less concise than Cypher
- Still evolving feature set

## Use Case Recommendations

### Choose Cypher When:

✅ **Primary focus is graph operations**
```cypher
// Complex graph pattern matching
MATCH (a:Person)-[:KNOWS]->(b:Person)-[:WORKS_AT]->(c:Company)
WHERE c.industry = 'tech'
RETURN a, b, c
```

✅ **Migrating from Neo4j**
✅ **Building recommendation engines**
✅ **Analyzing social networks**
✅ **Need Neo4j tool compatibility**

### Choose AQL When:

✅ **Document-heavy workloads with graph relationships**
```aql
FOR user IN users
  FILTER user.profile.preferences.notifications == true
  FOR friend IN 1..1 OUTBOUND user follows
    RETURN {user: user, friend: friend}
```

✅ **Migrating from ArangoDB**
✅ **Complex analytical queries**
✅ **Need flexible document manipulation**
✅ **Working with varied data structures**

### Choose OrbitQL When:

✅ **Multi-model applications**
```orbitql
SELECT u.name, ts.avg_value, COUNT(f.id) as friends
FROM users u
JOIN TimeSeries ts ON u.sensor_id = ts.series_id
TRAVERSE OUTBOUND 1..1 STEPS ON follows TO f
WHERE ts.timestamp >= NOW() - INTERVAL '1 hour'
GROUP BY u.id, u.name, ts.avg_value;
```

✅ **Team has strong SQL background**
✅ **Time series analytics are important**
✅ **Building distributed applications**
✅ **Need actor system integration**

## Syntax Comparisons

### Basic Node Selection

**Cypher**
```cypher
MATCH (user:Person {age: 30})
RETURN user.name
```

**AQL**
```aql
FOR user IN users
  FILTER user.age == 30
  RETURN user.name
```

**OrbitQL**
```orbitql
SELECT name 
FROM users 
WHERE age = 30
```

### Graph Traversal

**Cypher**
```cypher
MATCH (user:Person)-[:FOLLOWS*1..3]->(friend:Person)
WHERE user.name = 'Alice'
RETURN friend.name
```

**AQL**
```aql
FOR friend IN 1..3 OUTBOUND 'users/alice' follows
  RETURN friend.name
```

**OrbitQL**
```orbitql
SELECT friend.name
FROM users user
TRAVERSE OUTBOUND 1..3 STEPS ON follows TO friend
WHERE user.name = 'Alice'
```

### Aggregations

**Cypher**
```cypher
MATCH (user:Person)-[:PURCHASED]->(product:Product)
RETURN product.category, count(*) as purchases, avg(product.price) as avg_price
ORDER BY purchases DESC
```

**AQL**
```aql
FOR user IN users
  FOR product IN products
    FILTER product.id IN user.purchased_products
    COLLECT category = product.category
    AGGREGATE purchases = COUNT(), avg_price = AVERAGE(product.price)
    SORT purchases DESC
    RETURN {category, purchases, avg_price}
```

**OrbitQL**
```orbitql
SELECT 
    p.category,
    COUNT(*) as purchases,
    AVG(p.price) as avg_price
FROM users u
JOIN user_purchases up ON u.id = up.user_id
JOIN products p ON up.product_id = p.id
GROUP BY p.category
ORDER BY purchases DESC
```

### Time Series Operations

**Cypher** (Limited support)
```cypher
// Not natively supported, requires custom functions
MATCH (sensor:Sensor)
CALL time_series.query(sensor.id, '1 hour') YIELD timestamp, value
RETURN sensor.name, avg(value)
```

**AQL**
```aql
FOR point IN timeseries
  FILTER point.series_id == "sensor_01"
    AND point.timestamp >= DATE_SUBTRACT(DATE_NOW(), 1, "hour")
  COLLECT sensor = point.series_id
  AGGREGATE avg_value = AVERAGE(point.value)
  RETURN {sensor, avg_value}
```

**OrbitQL**
```orbitql
SELECT 
    series_id,
    AVG(value) OVER TIME WINDOW '15 minutes' as avg_value
FROM metrics 
WHERE series_id = 'sensor_01'
  AND timestamp >= NOW() - INTERVAL '1 hour'
GROUP BY TIME BUCKET '15 minutes'
```

## Performance Characteristics

### Query Execution Performance

| Operation | Cypher | AQL | OrbitQL |
|-----------|--------|-----|---------|
| **Simple node lookup** | Excellent | Good | Excellent |
| **Graph traversal** | Excellent | Good | Very Good |
| **Document filtering** | Good | Excellent | Very Good |
| **Aggregations** | Good | Excellent | Excellent |
| **Time series** | Limited | Good | Excellent |
| **Cross-model joins** | Limited | Good | Excellent |
| **Distributed queries** | Good | Good | Excellent |

### Memory Usage

- **Cypher**: Optimized for graph operations, efficient path storage
- **AQL**: Flexible memory usage, good for varied data types
- **OrbitQL**: Optimized for streaming and large result sets

### Scalability

- **Cypher**: Scales well for graph-heavy workloads
- **AQL**: Good scalability for mixed workloads  
- **OrbitQL**: Designed for distributed scaling from ground up

## Migration Guide

### From Neo4j/Cypher to OrbitQL

```cypher
// Cypher
MATCH (u:User)-[:FOLLOWS]->(f:User)
WHERE u.location = 'NYC'
RETURN u.name, f.name
```

```orbitql
-- OrbitQL
SELECT u.name, f.name
FROM users u
TRAVERSE OUTBOUND 1..1 STEPS ON follows TO f
WHERE u.location = 'NYC'
```

### From ArangoDB/AQL to OrbitQL

```aql
// AQL
FOR user IN users
  FILTER user.age > 25
  FOR friend IN 1..1 OUTBOUND user follows
    RETURN {user: user.name, friend: friend.name}
```

```orbitql
-- OrbitQL
SELECT u.name as user, f.name as friend
FROM users u
TRAVERSE OUTBOUND 1..1 STEPS ON follows TO f  
WHERE u.age > 25
```

### From SQL to OrbitQL

```sql
-- SQL
SELECT u.name, COUNT(o.id) as order_count
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
GROUP BY u.id, u.name
```

```orbitql
-- OrbitQL (identical syntax)
SELECT u.name, COUNT(o.id) as order_count
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
GROUP BY u.id, u.name
```

## Best Practices

### Language Selection Strategy

1. **Start with your team's expertise**
   - SQL background → OrbitQL
   - Neo4j experience → Cypher
   - ArangoDB experience → AQL

2. **Consider primary data patterns**
   - Graph-heavy → Cypher
   - Document-centric → AQL
   - Multi-model/Time series → OrbitQL

3. **Evaluate ecosystem requirements**
   - Neo4j tools → Cypher
   - ArangoDB tools → AQL
   - Custom applications → OrbitQL

### Performance Optimization

#### Cypher Optimization
```cypher
// Use specific labels and properties for efficient filtering
MATCH (u:User {active: true})-[:FOLLOWS]->(f:User)
WHERE u.created_at > date('2023-01-01')
RETURN u, f
```

#### AQL Optimization
```aql
// Filter early and use indexes
FOR user IN users
  FILTER user.active == true AND user.created_at > "2023-01-01"
  FOR friend IN 1..1 OUTBOUND user follows
    RETURN {user, friend}
```

#### OrbitQL Optimization
```orbitql
-- Use appropriate indexes and limit result sets
SELECT /*+ INDEX(users, idx_active_created) */ u.name, f.name
FROM users u
TRAVERSE OUTBOUND 1..1 STEPS ON follows TO f
WHERE u.active = true AND u.created_at > '2023-01-01'
LIMIT 1000
```

### Cross-Language Integration

You can use multiple languages in the same application:

```rust
// Use the best language for each operation
let social_data = cypher_engine.execute(
    "MATCH (u:User)-[:FOLLOWS*2..3]->(influence:User) 
     WHERE u.name = $name 
     RETURN influence"
).await?;

let user_profile = aql_engine.execute(
    "FOR user IN users 
     FILTER user._id == @user_id 
     RETURN user.profile"
).await?;

let time_series_data = orbitql_engine.execute(
    "SELECT AVG(value) OVER TIME WINDOW '1 hour'
     FROM metrics 
     WHERE series_id = @series_id 
     AND timestamp >= NOW() - INTERVAL '24 hours'"
).await?;
```

### When to Use Multiple Languages

✅ **Different teams with different expertise**
✅ **Migrating between systems gradually**
✅ **Specialized operations requiring optimal performance**
✅ **Integration with existing tools and systems**

❌ **Don't mix languages unnecessarily**
❌ **Avoid if team size is small**
❌ **Skip if maintenance complexity is a concern**

## Conclusion

Orbit-RS's multi-language support provides unprecedented flexibility for building modern applications. Choose the language that best fits your use case, team expertise, and performance requirements. Remember that you can always mix languages within the same application to get the best of all worlds.

The key is to start with one language that matches your primary use case and expand to others as your needs evolve. OrbitQL is often the best starting point for new projects due to its SQL familiarity and multi-model capabilities, while Cypher and AQL provide specialized advantages for graph and document operations respectively.