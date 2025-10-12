---
layout: default
title: AQL (ArrangoDB Query Language) Reference
category: documentation
---

# AQL (ArrangoDB Query Language) Reference

AQL (ArrangoDB Query Language) is a declarative query language for multi-model databases that combines the power of SQL-like syntax with native support for graph traversals and document operations. Orbit-RS provides a comprehensive AQL implementation optimized for distributed graph and time series data.

## Table of Contents

1. [Overview](#overview)
2. [Syntax Fundamentals](#syntax-fundamentals)
3. [Data Types](#data-types)
4. [Operations](#operations)
5. [Graph Traversals](#graph-traversals)
6. [Aggregations](#aggregations)
7. [Functions](#functions)
8. [Time Series Integration](#time-series-integration)
9. [Performance Optimization](#performance-optimization)
10. [Examples](#examples)
11. [API Reference](#api-reference)

## Overview

AQL in Orbit-RS provides:

- **Multi-model support**: Query graphs, documents, and time series data
- **ACID transactions**: Full transaction support with consistency guarantees
- **Graph traversals**: Native graph pattern matching and path finding
- **High performance**: Optimized query execution with smart indexing
- **Distributed execution**: Scale queries across cluster nodes
- **Type safety**: Strong typing with compile-time query validation

### Key Features

- **Declarative syntax**: Focus on what you want, not how to get it
- **Flexible joins**: Join across different data models
- **Advanced analytics**: Built-in statistical and mathematical functions
- **Streaming results**: Process large result sets efficiently
- **Query optimization**: Automatic query plan optimization

## Syntax Fundamentals

### Basic Query Structure

```aql
FOR variable IN collection
  [FILTER condition]
  [SORT expression]
  [LIMIT count]
  RETURN expression
```

### Variables and Expressions

```aql
// Variable binding
FOR doc IN users
  RETURN doc.name

// Expression evaluation
FOR doc IN users
  RETURN {
    fullName: CONCAT(doc.firstName, " ", doc.lastName),
    age: doc.age,
    isAdult: doc.age >= 18
  }
```

### Comments

```aql
// Single line comment
FOR doc IN users // Another comment
  /* Multi-line
     comment */
  RETURN doc
```

## Data Types

### Primitive Types

```aql
// Numbers
FOR doc IN collection
  RETURN {
    integer: 42,
    float: 3.14159,
    scientific: 1.23e-4
  }

// Strings
FOR doc IN collection
  RETURN {
    simple: "Hello World",
    escaped: "Line 1\nLine 2",
    unicode: "Unicode: 🚀"
  }

// Booleans
FOR doc IN collection
  RETURN {
    isTrue: true,
    isFalse: false,
    isNull: null
  }
```

### Complex Types

```aql
// Arrays
FOR doc IN collection
  RETURN {
    numbers: [1, 2, 3, 4, 5],
    mixed: [1, "hello", true, null],
    nested: [[1, 2], [3, 4]]
  }

// Objects
FOR doc IN collection
  RETURN {
    user: {
      name: "Alice",
      age: 30,
      preferences: {
        theme: "dark",
        language: "en"
      }
    }
  }
```

## Operations

### FOR Loops

```aql
// Simple iteration
FOR user IN users
  RETURN user

// Multiple collections
FOR user IN users
  FOR post IN posts
    FILTER post.authorId == user._id
    RETURN {user: user.name, post: post.title}

// Array iteration
FOR item IN [1, 2, 3, 4, 5]
  RETURN item * 2

// Object iteration
FOR key IN ATTRIBUTES({name: "Alice", age: 30})
  RETURN key
```

### FILTER Operations

```aql
// Basic filters
FOR user IN users
  FILTER user.age > 18
  RETURN user

// Complex conditions
FOR user IN users
  FILTER user.age BETWEEN 25 AND 65
    AND user.status == "active"
    AND user.email LIKE "%@company.com"
  RETURN user

// Array filters
FOR user IN users
  FILTER "javascript" IN user.skills
  RETURN user

// Null checks
FOR user IN users
  FILTER user.lastLogin != null
  RETURN user
```

### SORT Operations

```aql
// Simple sorting
FOR user IN users
  SORT user.name
  RETURN user

// Multiple sort criteria
FOR user IN users
  SORT user.department, user.salary DESC, user.name
  RETURN user

// Expression sorting
FOR user IN users
  SORT CONCAT(user.lastName, user.firstName)
  RETURN user
```

### LIMIT Operations

```aql
// Simple limit
FOR user IN users
  LIMIT 10
  RETURN user

// Offset and limit
FOR user IN users
  LIMIT 20, 10  // Skip 20, take 10
  RETURN user

// Dynamic limits
FOR user IN users
  LIMIT @offset, @count
  RETURN user
```

## Graph Traversals

### Basic Traversals

```aql
// Outbound traversal
FOR vertex, edge, path IN 1..3 OUTBOUND 'users/alice' follows
  RETURN {vertex, edge, path}

// Inbound traversal
FOR vertex, edge IN 1..2 INBOUND 'posts/123' authored
  RETURN vertex

// Any direction
FOR vertex IN 1..5 ANY 'users/alice' GRAPH 'social'
  RETURN vertex
```

### Advanced Traversals

```aql
// Multiple edge collections
FOR vertex, edge IN 1..3 OUTBOUND 'users/alice' follows, likes
  RETURN {vertex, edge}

// Path filtering
FOR vertex, edge, path IN 1..4 OUTBOUND 'users/alice' follows
  FILTER path.edges[*].weight > 0.5
  RETURN vertex

// Shortest path
FOR path IN OUTBOUND SHORTEST_PATH 'users/alice' TO 'users/bob' follows
  RETURN path
```

### Named Graphs

```aql
// Define and use named graphs
FOR vertex, edge IN 1..3 OUTBOUND 'users/alice' GRAPH 'social_network'
  RETURN {
    person: vertex.name,
    relationship: edge.type,
    since: edge.created_at
  }

// Multiple graphs
FOR vertex IN 1..2 OUTBOUND 'users/alice' 
  GRAPH ['professional', 'social']
  RETURN vertex
```

## Aggregations

### Basic Aggregations

```aql
// COUNT
FOR user IN users
  COLLECT status = user.status WITH COUNT INTO count
  RETURN {status, count}

// SUM, AVERAGE
FOR sale IN sales
  COLLECT year = DATE_YEAR(sale.date) 
  AGGREGATE total = SUM(sale.amount), avg = AVERAGE(sale.amount)
  RETURN {year, total, avg}

// MIN, MAX
FOR product IN products
  COLLECT category = product.category
  AGGREGATE 
    minPrice = MIN(product.price),
    maxPrice = MAX(product.price)
  RETURN {category, minPrice, maxPrice}
```

### Advanced Aggregations

```aql
// Multiple grouping levels
FOR sale IN sales
  COLLECT 
    year = DATE_YEAR(sale.date),
    month = DATE_MONTH(sale.date),
    region = sale.region
  AGGREGATE total = SUM(sale.amount)
  RETURN {year, month, region, total}

// COLLECT with arrays
FOR user IN users
  COLLECT department = user.department INTO departmentUsers
  RETURN {
    department,
    users: departmentUsers[*].user.name,
    count: LENGTH(departmentUsers)
  }
```

### Statistical Functions

```aql
// Advanced statistics
FOR metric IN metrics
  COLLECT hour = DATE_HOUR(metric.timestamp)
  AGGREGATE 
    count = COUNT(),
    avg = AVERAGE(metric.value),
    stddev = STDDEV(metric.value),
    variance = VARIANCE(metric.value),
    percentile_95 = PERCENTILE(metric.value, 95)
  RETURN {hour, count, avg, stddev, variance, percentile_95}
```

## Functions

### String Functions

```aql
FOR user IN users
  RETURN {
    name: user.name,
    upper: UPPER(user.name),
    lower: LOWER(user.name),
    length: LENGTH(user.name),
    substring: SUBSTRING(user.name, 0, 3),
    concat: CONCAT(user.firstName, " ", user.lastName),
    split: SPLIT(user.email, "@"),
    replace: SUBSTITUTE(user.phone, "-", ""),
    regex: REGEX_MATCHES(user.email, "^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$")
  }
```

### Numeric Functions

```aql
FOR data IN dataset
  RETURN {
    value: data.value,
    abs: ABS(data.value),
    floor: FLOOR(data.value),
    ceil: CEIL(data.value),
    round: ROUND(data.value, 2),
    sqrt: SQRT(ABS(data.value)),
    power: POW(data.value, 2),
    log: LOG(ABS(data.value)),
    sin: SIN(data.angle),
    cos: COS(data.angle),
    random: RAND()
  }
```

### Date/Time Functions

```aql
FOR event IN events
  RETURN {
    timestamp: event.timestamp,
    year: DATE_YEAR(event.timestamp),
    month: DATE_MONTH(event.timestamp),
    day: DATE_DAY(event.timestamp),
    hour: DATE_HOUR(event.timestamp),
    formatted: DATE_FORMAT(event.timestamp, "%Y-%m-%d %H:%M:%S"),
    unix: DATE_TIMESTAMP(event.timestamp),
    from_unix: DATE_ISO8601(event.unix_timestamp),
    diff: DATE_DIFF(event.end_time, event.start_time, "minutes"),
    add: DATE_ADD(event.timestamp, 1, "day")
  }
```

### Array Functions

```aql
FOR user IN users
  RETURN {
    skills: user.skills,
    first: FIRST(user.skills),
    last: LAST(user.skills),
    length: LENGTH(user.skills),
    sorted: SORTED(user.skills),
    unique: UNIQUE(user.skills),
    reversed: REVERSE(user.skills),
    slice: SLICE(user.skills, 1, 2),
    contains: "javascript" IN user.skills,
    intersection: INTERSECTION(user.skills, ["javascript", "python", "rust"]),
    union: UNION(user.skills, ["go", "typescript"]),
    flatten: FLATTEN([user.skills, user.certifications])
  }
```

## Time Series Integration

### Time Series Queries

```aql
// Query time series data
FOR point IN timeseries
  FILTER point.series_id == "temperature_sensor_01"
    AND point.timestamp >= DATE_SUBTRACT(DATE_NOW(), 1, "hour")
  SORT point.timestamp
  RETURN {
    time: point.timestamp,
    value: point.value,
    labels: point.labels
  }

// Aggregate time series data
FOR point IN timeseries
  FILTER point.series_id LIKE "cpu_usage_%"
    AND point.timestamp >= DATE_SUBTRACT(DATE_NOW(), 24, "hour")
  COLLECT 
    hour = DATE_HOUR(point.timestamp),
    server = point.labels.server
  AGGREGATE 
    avg_cpu = AVERAGE(point.value),
    max_cpu = MAX(point.value),
    count = COUNT()
  RETURN {hour, server, avg_cpu, max_cpu, count}
```

### Combined Graph and Time Series

```aql
// Join graph and time series data
FOR sensor IN sensors
  FOR point IN timeseries
    FILTER point.series_id == sensor._id
      AND point.timestamp >= DATE_SUBTRACT(DATE_NOW(), 1, "hour")
    COLLECT 
      sensor_name = sensor.name,
      location = sensor.location
    AGGREGATE 
      avg_value = AVERAGE(point.value),
      sample_count = COUNT()
    RETURN {sensor_name, location, avg_value, sample_count}

// Graph traversal with time series
FOR vertex, edge IN 1..2 OUTBOUND 'sensors/temp_01' connected_to
  FOR point IN timeseries
    FILTER point.series_id == vertex._id
      AND point.timestamp >= @start_time
    COLLECT sensor = vertex
    AGGREGATE latest_value = LAST(point.value)
    RETURN {
      sensor: sensor.name,
      type: sensor.type,
      latest_reading: latest_value,
      location: sensor.location
    }
```

## Performance Optimization

### Index Usage

```aql
// Efficient filtering with indexes
FOR user IN users
  FILTER user.email == @email  // Uses hash index
  RETURN user

FOR post IN posts
  FILTER post.created_at >= @start_date  // Uses skiplist index
  SORT post.created_at DESC
  RETURN post

// Compound index usage
FOR order IN orders
  FILTER order.customer_id == @customer_id
    AND order.status == "completed"
    AND order.total >= @min_total
  RETURN order
```

### Query Optimization

```aql
// Early filtering
FOR user IN users
  FILTER user.active == true  // Filter early
  FOR order IN orders
    FILTER order.user_id == user._id
      AND order.status == "completed"
    RETURN {user, order}

// Efficient joins
FOR user IN users
  FILTER user.department == @department
  FOR order IN orders
    FILTER order.user_id == user._id
    COLLECT user_info = user INTO orders_group = order
    RETURN {
      user: user_info,
      order_count: LENGTH(orders_group),
      total_amount: SUM(orders_group[*].amount)
    }
```

### Batch Operations

```aql
// Efficient batch processing
FOR batch IN 1..100
  FOR user IN users
    LIMIT (batch - 1) * 1000, 1000  // Process in batches
    // Process each batch
    RETURN user
```

## Examples

### Social Network Analysis

```aql
// Find mutual friends
FOR user IN users
  FILTER user._id == @user_id
  FOR friend1 IN 1..1 OUTBOUND user follows
    FOR friend2 IN 1..1 OUTBOUND @other_user_id follows
      FILTER friend1._id == friend2._id
      RETURN DISTINCT friend1

// Friend recommendations
FOR user IN users
  FILTER user._id == @user_id
  FOR friend IN 1..1 OUTBOUND user follows
    FOR friend_of_friend IN 1..1 OUTBOUND friend follows
      FILTER friend_of_friend._id != @user_id
        AND friend_of_friend._id NOT IN (
          FOR direct_friend IN 1..1 OUTBOUND user follows
            RETURN direct_friend._id
        )
      COLLECT recommended_user = friend_of_friend WITH COUNT INTO mutual_count
      SORT mutual_count DESC
      LIMIT 10
      RETURN {user: recommended_user, mutual_friends: mutual_count}
```

### E-commerce Analytics

```aql
// Customer lifetime value
FOR customer IN customers
  FOR order IN orders
    FILTER order.customer_id == customer._id
    COLLECT customer_info = customer
    AGGREGATE 
      total_spent = SUM(order.total),
      order_count = COUNT(),
      avg_order_value = AVERAGE(order.total),
      first_order = MIN(order.created_at),
      last_order = MAX(order.created_at)
    LET days_active = DATE_DIFF(last_order, first_order, "day")
    RETURN {
      customer: customer_info.name,
      lifetime_value: total_spent,
      orders: order_count,
      avg_order: avg_order_value,
      days_active: days_active,
      value_per_day: total_spent / MAX(days_active, 1)
    }

// Product recommendation based on purchase history
FOR customer IN customers
  FILTER customer._id == @customer_id
  FOR order IN orders
    FILTER order.customer_id == customer._id
    FOR item IN order.items
      FOR similar_item IN products
        FILTER similar_item.category == item.category
          AND similar_item._id != item._id
          AND similar_item._id NOT IN (
            FOR past_order IN orders
              FILTER past_order.customer_id == @customer_id
              FOR past_item IN past_order.items
                RETURN past_item._id
          )
        COLLECT recommended_product = similar_item WITH COUNT INTO relevance_score
        SORT relevance_score DESC
        LIMIT 5
        RETURN {product: recommended_product, score: relevance_score}
```

### IoT Data Analysis

```aql
// Sensor anomaly detection
FOR sensor IN sensors
  FOR point IN timeseries
    FILTER point.series_id == sensor._id
      AND point.timestamp >= DATE_SUBTRACT(DATE_NOW(), 1, "hour")
    COLLECT sensor_info = sensor
    AGGREGATE 
      avg_value = AVERAGE(point.value),
      stddev_value = STDDEV(point.value),
      points = point[*]
    LET threshold = avg_value + (2 * stddev_value)
    LET anomalies = (
      FOR p IN points
        FILTER p.value > threshold
        RETURN p
    )
    FILTER LENGTH(anomalies) > 0
    RETURN {
      sensor: sensor_info.name,
      location: sensor_info.location,
      avg_value: avg_value,
      threshold: threshold,
      anomaly_count: LENGTH(anomalies),
      anomalies: anomalies
    }

// Equipment maintenance prediction
FOR equipment IN equipment
  FOR metric IN timeseries
    FILTER metric.series_id == equipment.sensor_id
      AND metric.timestamp >= DATE_SUBTRACT(DATE_NOW(), 30, "day")
    COLLECT 
      equipment_info = equipment,
      day = DATE_DAY(metric.timestamp)
    AGGREGATE daily_avg = AVERAGE(metric.value)
    COLLECT equipment_final = equipment_info INTO daily_values = daily_avg
    LET trend = SLOPE(daily_values)  // Custom function for trend analysis
    LET maintenance_needed = ABS(trend) > 0.1  // Degradation threshold
    FILTER maintenance_needed
    RETURN {
      equipment: equipment_final.name,
      location: equipment_final.location,
      trend: trend,
      priority: ABS(trend) > 0.2 ? "high" : "medium",
      next_maintenance: DATE_ADD(DATE_NOW(), CEIL(30 / ABS(trend)), "day")
    }
```

## API Reference

### Query Execution

```rust
use orbit_protocols::aql::{AQLEngine, QueryOptions};

// Execute AQL query
let result = aql_engine.execute(
    "FOR user IN users FILTER user.age > @min_age RETURN user",
    [("min_age", 18)].iter().cloned().collect()
).await?;

// Execute with options
let options = QueryOptions {
    max_runtime: Duration::from_secs(30),
    memory_limit: 1024 * 1024 * 100, // 100MB
    intermediate_commit_size: 10000,
    ..Default::default()
};

let result = aql_engine.execute_with_options(query, params, options).await?;
```

### Streaming Results

```rust
use futures_util::stream::StreamExt;

// Stream large result sets
let mut stream = aql_engine.execute_stream(
    "FOR doc IN large_collection RETURN doc"
).await?;

while let Some(batch) = stream.next().await {
    for document in batch? {
        // Process each document
        println!("{:?}", document);
    }
}
```

### Prepared Statements

```rust
// Prepare query for multiple executions
let prepared = aql_engine.prepare(
    "FOR user IN users FILTER user.department == @dept RETURN user"
).await?;

// Execute with different parameters
let results1 = prepared.execute([("dept", "engineering")]).await?;
let results2 = prepared.execute([("dept", "marketing")]).await?;
```

### Transaction Support

```rust
// Execute within transaction
let transaction = aql_engine.begin_transaction().await?;

let result1 = transaction.execute(
    "FOR user IN users FILTER user._id == @id UPDATE user WITH {last_login: DATE_NOW()} IN users",
    [("id", "users/123")]
).await?;

let result2 = transaction.execute(
    "INSERT {user_id: @id, action: 'login', timestamp: DATE_NOW()} IN audit_log",
    [("id", "users/123")]
).await?;

transaction.commit().await?;
```

AQL in Orbit-RS provides a powerful, flexible query language that seamlessly integrates graph traversals, document operations, and time series analytics in a single, optimized execution engine.