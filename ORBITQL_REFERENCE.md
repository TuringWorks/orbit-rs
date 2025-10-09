# OrbitQL Reference

OrbitQL is Orbit-RS's native query language designed specifically for multi-model distributed systems. It combines the familiarity of SQL with advanced features for graph traversals, time series analytics, and distributed computing, all optimized for the Orbit actor system.

## Table of Contents

1. [Overview](#overview)
2. [Language Design](#language-design)
3. [Basic Syntax](#basic-syntax)
4. [Data Models](#data-models)
5. [Graph Queries](#graph-queries)
6. [Time Series Queries](#time-series-queries)
7. [Multi-Model Queries](#multi-model-queries)
8. [Distributed Operations](#distributed-operations)
9. [Advanced Features](#advanced-features)
10. [Performance & Optimization](#performance--optimization)
11. [Examples](#examples)
12. [API Reference](#api-reference)

## Overview

OrbitQL is designed as a unified query language that brings together:

- **SQL familiarity**: Standard SQL syntax for basic operations
- **Graph extensions**: Native graph traversal and pattern matching
- **Time series support**: Built-in temporal operations and analytics
- **Actor integration**: Query across distributed actor systems
- **Type safety**: Strong typing with compile-time validation
- **Performance**: Optimized for distributed execution

### Key Features

- **Multi-model unified syntax**: Query graphs, documents, time series, and relational data in single queries
- **Advanced SQL compatibility**: Full support for CTEs, CASE expressions, window functions, and temporal operations
- **Temporal functions**: Native NOW(), INTERVAL, and time-based analytics
- **Aggregate enhancements**: COUNT(DISTINCT), conditional aggregates, and complex expressions
- **Distributed by design**: Native support for cross-node queries and joins
- **Actor-aware**: Direct integration with Orbit's virtual actor system
- **Streaming support**: Handle large result sets with streaming execution
- **ACID compliance**: Full transaction support across distributed data

## Language Design

### Design Principles

1. **Familiarity**: Build on SQL foundations that developers know
2. **Extensibility**: Clean extensions for graph and time series operations
3. **Performance**: Query optimization for distributed systems
4. **Type Safety**: Compile-time query validation and optimization
5. **Composability**: Combine different data models seamlessly

### Core Extensions to SQL

```orbitql
-- Traditional SQL
SELECT name, age FROM users WHERE age > 21;

-- Graph traversal extension
SELECT u.name, friend.name 
FROM users u 
TRAVERSE OUTBOUND 1..3 STEPS ON follows TO friend
WHERE u.location = 'NYC';

-- Time series extension
SELECT series_id, AVG(value) OVER TIME WINDOW '1 hour'
FROM metrics 
WHERE timestamp >= NOW() - INTERVAL '24 hours'
GROUP BY TIME BUCKET '1 hour';

-- Multi-model combination
SELECT u.name, ts.avg_temp, COUNT(f.name) as friend_count
FROM users u
JOIN TimeSeries ts ON u.sensor_id = ts.series_id
TRAVERSE OUTBOUND 1..1 STEPS ON follows TO f
WHERE ts.timestamp >= NOW() - INTERVAL '1 hour'
GROUP BY u.id, TIME BUCKET '15 minutes';
```

## Basic Syntax

### Query Structure

```orbitql
[WITH recursive_cte AS (...)]
SELECT [DISTINCT] projection
FROM data_sources
[JOIN other_sources ON conditions]
[TRAVERSE graph_operations]
[WHERE conditions]
[GROUP BY grouping_expressions]
[HAVING group_conditions]
[ORDER BY sort_expressions]
[LIMIT count [OFFSET start]]
[FOR UPDATE | FOR SHARE]
```

### Data Types

```orbitql
-- Scalar types
SELECT 
    42 as integer,
    3.14159 as float,
    'Hello World' as string,
    true as boolean,
    null as null_value,
    '2023-10-06T19:00:00Z'::timestamp as time_value,
    '1 hour'::interval as duration;

-- Array types
SELECT 
    ARRAY[1, 2, 3, 4, 5] as numbers,
    ARRAY['red', 'green', 'blue'] as colors;

-- Object types
SELECT 
    OBJECT(
        'name', 'Alice',
        'age', 30,
        'skills', ARRAY['rust', 'sql', 'graph']
    ) as user_profile;
```

### Variables and Parameters

```orbitql
-- Named parameters
SELECT * FROM users WHERE age > @min_age AND location = @city;

-- Positional parameters
SELECT * FROM products WHERE price BETWEEN $1 AND $2;

-- Variable assignment
WITH 
    current_time AS (SELECT NOW()),
    recent_threshold AS (SELECT current_time.value - INTERVAL '1 hour')
SELECT * FROM events WHERE timestamp >= recent_threshold.value;
```

## Data Models

### Relational Operations

```orbitql
-- Standard SQL operations
SELECT u.name, u.email, d.department_name
FROM users u
JOIN departments d ON u.department_id = d.id
WHERE u.active = true
ORDER BY u.created_at DESC;

-- Window functions
SELECT 
    name,
    salary,
    AVG(salary) OVER (PARTITION BY department) as dept_avg,
    RANK() OVER (PARTITION BY department ORDER BY salary DESC) as salary_rank
FROM employees;

-- Common Table Expressions
WITH monthly_sales AS (
    SELECT 
        DATE_TRUNC('month', order_date) as month,
        SUM(total) as monthly_total
    FROM orders
    GROUP BY DATE_TRUNC('month', order_date)
)
SELECT 
    month,
    monthly_total,
    monthly_total - LAG(monthly_total) OVER (ORDER BY month) as growth
FROM monthly_sales;
```

## Advanced SQL Features

### Common Table Expressions (CTEs)

```orbitql
-- Basic CTE
WITH user_stats AS (
    SELECT user_id, COUNT(*) AS post_count
    FROM posts
    WHERE created_at > NOW() - INTERVAL '30 days'
    GROUP BY user_id
)
SELECT u.name, COALESCE(us.post_count, 0) AS recent_posts
FROM users u
LEFT JOIN user_stats us ON u.id = us.user_id;

-- Multiple CTEs
WITH 
    active_users AS (
        SELECT user_id FROM sessions 
        WHERE last_seen > NOW() - INTERVAL '7 days'
    ),
    popular_posts AS (
        SELECT post_id, COUNT(*) AS like_count
        FROM likes
        GROUP BY post_id
        HAVING COUNT(*) > 100
    )
SELECT u.name, p.title, pp.like_count
FROM users u
JOIN active_users au ON u.id = au.user_id
JOIN posts p ON u.id = p.author_id
JOIN popular_posts pp ON p.id = pp.post_id;

-- Recursive CTE (planned)
WITH RECURSIVE employee_hierarchy AS (
    SELECT id, name, manager_id, 0 AS level
    FROM employees
    WHERE manager_id IS NULL
    
    UNION ALL
    
    SELECT e.id, e.name, e.manager_id, eh.level + 1
    FROM employees e
    JOIN employee_hierarchy eh ON e.manager_id = eh.id
)
SELECT name, level FROM employee_hierarchy ORDER BY level, name;
```

### CASE Expressions

```orbitql
-- Simple CASE
SELECT 
    name,
    age,
    CASE 
        WHEN age < 18 THEN 'Minor'
        WHEN age < 65 THEN 'Adult'
        ELSE 'Senior'
    END AS age_category
FROM users;

-- CASE in aggregations
SELECT 
    department,
    COUNT(*) AS total_employees,
    COUNT(CASE WHEN salary > 100000 THEN 1 END) AS high_earners,
    AVG(CASE WHEN performance_rating = 'excellent' THEN salary END) AS avg_excellent_salary
FROM employees
GROUP BY department;

-- Nested CASE expressions
SELECT 
    user_id,
    CASE 
        WHEN subscription_type = 'premium' THEN
            CASE 
                WHEN usage_hours > 100 THEN 'power_user'
                ELSE 'premium_casual'
            END
        WHEN subscription_type = 'basic' THEN 'basic_user'
        ELSE 'free_user'
    END AS user_segment
FROM user_subscriptions;
```

### Temporal Functions

```orbitql
-- NOW() function
SELECT NOW() AS current_time;
SELECT * FROM events WHERE created_at > NOW() - INTERVAL '1 hour';

-- INTERVAL expressions
SELECT 
    event_name,
    created_at,
    NOW() - created_at AS time_ago
FROM events
WHERE created_at BETWEEN NOW() - INTERVAL '7 days' AND NOW();

-- Time-based filtering
SELECT user_id, COUNT(*) AS login_count
FROM login_events
WHERE timestamp >= NOW() - INTERVAL '30 days'
  AND timestamp < NOW()
GROUP BY user_id
HAVING COUNT(*) > 10;
```

### Enhanced Aggregates

```orbitql
-- COUNT(DISTINCT)
SELECT 
    campaign_id,
    COUNT(*) AS total_clicks,
    COUNT(DISTINCT user_id) AS unique_users,
    COUNT(DISTINCT session_id) AS unique_sessions
FROM ad_clicks
GROUP BY campaign_id;

-- Conditional aggregates
SELECT 
    product_category,
    COUNT(*) AS total_orders,
    SUM(CASE WHEN order_status = 'completed' THEN amount ELSE 0 END) AS completed_revenue,
    AVG(CASE WHEN rating IS NOT NULL THEN rating END) AS avg_rating,
    COUNT(DISTINCT CASE WHEN order_status = 'completed' THEN customer_id END) AS unique_customers
FROM orders
GROUP BY product_category;

-- Complex aggregate expressions
SELECT 
    region,
    COUNT(DISTINCT customer_id) AS customers,
    SUM(order_amount) AS total_revenue,
    SUM(order_amount) / COUNT(DISTINCT customer_id) AS revenue_per_customer,
    COUNT(CASE WHEN order_amount > 1000 THEN 1 END) AS high_value_orders
FROM sales_data
WHERE order_date >= NOW() - INTERVAL '90 days'
GROUP BY region
ORDER BY revenue_per_customer DESC;
```

### Null-Handling Functions

```orbitql
-- COALESCE for null handling
SELECT 
    user_id,
    COALESCE(display_name, username, email) AS name,
    COALESCE(phone, email, 'No contact') AS contact_method
FROM user_profiles;

-- Complex null handling
SELECT 
    order_id,
    customer_name,
    CASE 
        WHEN shipping_address IS NOT NULL THEN shipping_address
        WHEN billing_address IS NOT NULL THEN billing_address
        ELSE 'Address missing'
    END AS delivery_address,
    COALESCE(estimated_delivery, order_date + INTERVAL '7 days') AS expected_delivery
FROM orders;
```

### Document Operations

```orbitql
-- JSON document queries
SELECT 
    user_id,
    profile->>'name' as name,
    profile->'preferences'->>'theme' as theme,
    JSON_ARRAY_LENGTH(profile->'skills') as skill_count
FROM user_profiles
WHERE profile->>'active' = 'true';

-- Document updates
UPDATE user_profiles 
SET profile = JSON_SET(profile, 
    '$.last_login', NOW(),
    '$.login_count', CAST(profile->>'login_count' AS INTEGER) + 1
)
WHERE user_id = @user_id;
```

## Graph Queries

### Basic Traversal

```orbitql
-- Simple outbound traversal
SELECT person.name, friend.name
FROM users person
TRAVERSE OUTBOUND 1..1 STEPS ON follows TO friend
WHERE person.location = 'San Francisco';

-- Multi-hop traversal
SELECT start.name, end.name, path_length
FROM users start
TRAVERSE OUTBOUND 1..5 STEPS ON follows TO end
RETURN PATHS AS path_info
WHERE start.name = 'Alice' AND end.location = 'New York';
```

### Advanced Graph Operations

```orbitql
-- Bidirectional traversal
SELECT person1.name, person2.name, relationship_type
FROM users person1
TRAVERSE ANY 1..3 STEPS ON [follows, knows, works_with] TO person2
RETURN EDGES AS edge_info
WHERE person1.profession = 'engineer';

-- Shortest path queries
SELECT source.name, target.name, shortest_distance
FROM users source, users target
FIND SHORTEST PATH BETWEEN source AND target
VIA [follows, knows]
WHERE source.location = 'NYC' AND target.location = 'SF';

-- Pattern matching
SELECT company.name, COUNT(*) as employee_count
FROM companies company
TRAVERSE INBOUND 1..1 STEPS ON works_for TO employee
WHERE employee.active = true
GROUP BY company.id, company.name
HAVING employee_count > 100;
```

### Graph Analytics

```orbitql
-- Centrality measures
SELECT 
    user.name,
    BETWEENNESS_CENTRALITY(user) as betweenness,
    CLOSENESS_CENTRALITY(user) as closeness,
    DEGREE_CENTRALITY(user) as degree
FROM users user
WHERE user.active = true
ORDER BY betweenness DESC;

-- Community detection
SELECT 
    community_id,
    ARRAY_AGG(user.name) as members,
    COUNT(*) as size
FROM (
    SELECT 
        user,
        LOUVAIN_COMMUNITY(user) as community_id
    FROM users user
    TRAVERSE ANY 1..3 STEPS ON follows
) community_data
GROUP BY community_id
ORDER BY size DESC;
```

## Time Series Queries

### Basic Time Series Operations

```orbitql
-- Simple time series query
SELECT series_id, timestamp, value
FROM metrics
WHERE series_id = 'cpu_usage_server_01'
  AND timestamp >= NOW() - INTERVAL '1 hour'
ORDER BY timestamp;

-- Multiple series aggregation
SELECT 
    series_id,
    MIN(value) as min_val,
    MAX(value) as max_val,
    AVG(value) as avg_val,
    STDDEV(value) as stddev_val
FROM metrics
WHERE series_id LIKE 'temperature_sensor_%'
  AND timestamp >= NOW() - INTERVAL '24 hours'
GROUP BY series_id;
```

### Temporal Windows

```orbitql
-- Time-based windows
SELECT 
    TIME_BUCKET('15 minutes', timestamp) as time_window,
    series_id,
    AVG(value) as avg_value,
    MAX(value) as max_value,
    COUNT(*) as sample_count
FROM metrics
WHERE timestamp >= NOW() - INTERVAL '6 hours'
GROUP BY TIME_BUCKET('15 minutes', timestamp), series_id
ORDER BY time_window, series_id;

-- Sliding windows
SELECT 
    timestamp,
    value,
    AVG(value) OVER (
        ORDER BY timestamp 
        ROWS BETWEEN 10 PRECEDING AND CURRENT ROW
    ) as moving_avg_10,
    AVG(value) OVER (
        ORDER BY timestamp 
        RANGE BETWEEN INTERVAL '1 hour' PRECEDING AND CURRENT ROW
    ) as hourly_avg
FROM metrics
WHERE series_id = 'stock_price_AAPL'
ORDER BY timestamp;
```

### Advanced Time Series Analytics

```orbitql
-- Gap filling and interpolation
SELECT 
    timestamp,
    series_id,
    COALESCE(
        value, 
        INTERPOLATE_LINEAR(timestamp, value) OVER (
            PARTITION BY series_id 
            ORDER BY timestamp
        )
    ) as filled_value
FROM (
    SELECT * FROM metrics
    UNION ALL
    SELECT * FROM GENERATE_TIME_SERIES(
        '2023-01-01'::timestamp,
        '2023-01-31'::timestamp,
        INTERVAL '1 minute'
    )
) time_series
WHERE series_id = 'sensor_data';

-- Anomaly detection
WITH stats AS (
    SELECT 
        series_id,
        AVG(value) as mean_val,
        STDDEV(value) as stddev_val
    FROM metrics
    WHERE timestamp >= NOW() - INTERVAL '7 days'
    GROUP BY series_id
)
SELECT 
    m.timestamp,
    m.series_id,
    m.value,
    s.mean_val,
    s.stddev_val,
    ABS(m.value - s.mean_val) / s.stddev_val as z_score,
    CASE 
        WHEN ABS(m.value - s.mean_val) / s.stddev_val > 3 THEN 'anomaly'
        WHEN ABS(m.value - s.mean_val) / s.stddev_val > 2 THEN 'unusual'
        ELSE 'normal'
    END as status
FROM metrics m
JOIN stats s ON m.series_id = s.series_id
WHERE m.timestamp >= NOW() - INTERVAL '1 hour'
  AND ABS(m.value - s.mean_val) / s.stddev_val > 2
ORDER BY z_score DESC;
```

## Multi-Model Queries

### Graph + Time Series

```orbitql
-- Sensor network with time series data
SELECT 
    sensor.name,
    sensor.location,
    latest_readings.avg_temp,
    latest_readings.max_temp,
    connected_sensors.count as connected_count
FROM sensors sensor
JOIN (
    SELECT 
        series_id,
        AVG(value) as avg_temp,
        MAX(value) as max_temp
    FROM metrics
    WHERE timestamp >= NOW() - INTERVAL '1 hour'
    GROUP BY series_id
) latest_readings ON sensor.id = latest_readings.series_id
JOIN (
    SELECT 
        source_sensor.id,
        COUNT(*) as count
    FROM sensors source_sensor
    TRAVERSE OUTBOUND 1..1 STEPS ON connected_to TO connected_sensor
    GROUP BY source_sensor.id
) connected_sensors ON sensor.id = connected_sensors.id
WHERE sensor.active = true;
```

### Document + Graph + Time Series

```orbitql
-- User activity analysis across all models
SELECT 
    u.user_id,
    u.profile->>'name' as name,
    u.profile->>'location' as location,
    social_metrics.friend_count,
    social_metrics.avg_influence,
    activity_metrics.recent_activity,
    activity_metrics.activity_trend
FROM user_profiles u
JOIN (
    SELECT 
        user.id,
        COUNT(*) as friend_count,
        AVG(edge.influence_score) as avg_influence
    FROM users user
    TRAVERSE OUTBOUND 1..1 STEPS ON follows TO friend
    RETURN EDGES AS edge
    GROUP BY user.id
) social_metrics ON u.user_id = social_metrics.id
JOIN (
    SELECT 
        series_id,
        COUNT(*) as recent_activity,
        (COUNT(*) - LAG(COUNT(*)) OVER (ORDER BY series_id)) as activity_trend
    FROM activity_events
    WHERE timestamp >= NOW() - INTERVAL '24 hours'
    GROUP BY series_id
) activity_metrics ON u.user_id = activity_metrics.series_id
WHERE u.profile->>'active' = 'true';
```

## Distributed Operations

### Cross-Node Queries

```orbitql
-- Query data across cluster nodes
SELECT 
    node_id,
    COUNT(*) as record_count,
    AVG(processing_time) as avg_processing_time
FROM DISTRIBUTED.user_activities
WHERE created_at >= NOW() - INTERVAL '1 day'
GROUP BY node_id
ORDER BY avg_processing_time DESC;

-- Distributed joins
SELECT 
    u.name,
    o.total,
    p.product_name
FROM DISTRIBUTED.users u
JOIN DISTRIBUTED.orders o ON u.id = o.user_id
JOIN products p ON o.product_id = p.id  -- Local join
WHERE o.order_date >= NOW() - INTERVAL '30 days';
```

### Actor System Integration

```orbitql
-- Query actor states
SELECT 
    actor_id,
    actor_type,
    state->>'status' as status,
    state->>'last_heartbeat' as last_heartbeat,
    EXTRACT(EPOCH FROM (NOW() - (state->>'last_heartbeat')::timestamp)) as seconds_since_heartbeat
FROM ACTORS.all_actors
WHERE actor_type = 'UserSession';

-- Actor message patterns
SELECT 
    source_actor,
    target_actor,
    message_type,
    COUNT(*) as message_count,
    AVG(processing_time) as avg_processing_time
FROM ACTORS.message_log
WHERE timestamp >= NOW() - INTERVAL '1 hour'
GROUP BY source_actor, target_actor, message_type
ORDER BY message_count DESC;
```

## Advanced Features

### Recursive Queries

```orbitql
-- Organizational hierarchy
WITH RECURSIVE org_hierarchy AS (
    -- Base case: top-level managers
    SELECT id, name, manager_id, 0 as level, name as path
    FROM employees
    WHERE manager_id IS NULL
    
    UNION ALL
    
    -- Recursive case: employees with managers
    SELECT 
        e.id, 
        e.name, 
        e.manager_id,
        oh.level + 1,
        oh.path || ' -> ' || e.name
    FROM employees e
    JOIN org_hierarchy oh ON e.manager_id = oh.id
    WHERE oh.level < 10  -- Prevent infinite recursion
)
SELECT level, name, path
FROM org_hierarchy
ORDER BY level, name;
```

### User-Defined Functions

```orbitql
-- Define custom function
CREATE FUNCTION calculate_distance(lat1 FLOAT, lon1 FLOAT, lat2 FLOAT, lon2 FLOAT)
RETURNS FLOAT
AS $$
    -- Haversine formula implementation
    SELECT 
        2 * 6371 * ASIN(SQRT(
            POW(SIN(RADIANS(lat2 - lat1) / 2), 2) +
            COS(RADIANS(lat1)) * COS(RADIANS(lat2)) *
            POW(SIN(RADIANS(lon2 - lon1) / 2), 2)
        ))
$$;

-- Use custom function
SELECT 
    u1.name,
    u2.name,
    calculate_distance(
        u1.profile->>'latitude', u1.profile->>'longitude',
        u2.profile->>'latitude', u2.profile->>'longitude'
    ) as distance_km
FROM users u1, users u2
WHERE u1.id != u2.id
  AND calculate_distance(
        u1.profile->>'latitude', u1.profile->>'longitude',
        u2.profile->>'latitude', u2.profile->>'longitude'
    ) < 10  -- Within 10km
ORDER BY distance_km;
```

### Machine Learning Integration

```orbitql
-- Feature extraction for ML
SELECT 
    user_id,
    -- User profile features
    CAST(profile->>'age' AS INTEGER) as age,
    profile->>'location' as location,
    JSON_ARRAY_LENGTH(profile->'skills') as skill_count,
    
    -- Social network features
    (SELECT COUNT(*) FROM users TRAVERSE OUTBOUND 1..1 STEPS ON follows WHERE target.active = true) as friend_count,
    (SELECT AVG(CAST(target.profile->>'age' AS INTEGER)) FROM users TRAVERSE OUTBOUND 1..1 STEPS ON follows) as avg_friend_age,
    
    -- Activity features
    (SELECT COUNT(*) FROM activity_events WHERE series_id = users.user_id AND timestamp >= NOW() - INTERVAL '30 days') as monthly_activity,
    
    -- Time series features
    (SELECT AVG(value) FROM metrics WHERE series_id = users.user_id || '_engagement' AND timestamp >= NOW() - INTERVAL '7 days') as avg_engagement
    
FROM users
WHERE profile->>'active' = 'true';
```

## Performance & Optimization

### Index Hints

```orbitql
-- Use specific indexes
SELECT /*+ INDEX(users, idx_users_location_age) */ *
FROM users
WHERE location = 'NYC' AND age > 25;

-- Force specific join order
SELECT /*+ ORDERED */ u.name, o.total
FROM users u, orders o
WHERE u.id = o.user_id;
```

### Query Plans

```orbitql
-- Explain query execution plan
EXPLAIN (ANALYZE, VERBOSE, COSTS) 
SELECT u.name, COUNT(o.id) as order_count
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
WHERE u.created_at >= '2023-01-01'
GROUP BY u.id, u.name;
```

### Parallel Execution

```orbitql
-- Enable parallel processing
SET enable_parallel = true;
SET max_parallel_workers_per_gather = 4;

SELECT /*+ PARALLEL(4) */
    series_id,
    COUNT(*) as point_count,
    AVG(value) as avg_value
FROM metrics
WHERE timestamp >= NOW() - INTERVAL '7 days'
GROUP BY series_id;
```

## Examples

### Real-time Dashboard Query

```orbitql
-- Complete dashboard data in single query
WITH 
    -- System health metrics
    system_health AS (
        SELECT 
            'system_health' as metric_type,
            AVG(CASE WHEN series_id LIKE 'cpu_%' THEN value END) as avg_cpu,
            AVG(CASE WHEN series_id LIKE 'memory_%' THEN value END) as avg_memory,
            AVG(CASE WHEN series_id LIKE 'disk_%' THEN value END) as avg_disk
        FROM metrics
        WHERE timestamp >= NOW() - INTERVAL '5 minutes'
    ),
    
    -- Active user count
    active_users AS (
        SELECT 
            'active_users' as metric_type,
            COUNT(DISTINCT user_id) as count
        FROM activity_events
        WHERE timestamp >= NOW() - INTERVAL '15 minutes'
    ),
    
    -- Top connected sensors
    top_sensors AS (
        SELECT 
            'top_sensors' as metric_type,
            JSON_AGG(
                JSON_BUILD_OBJECT(
                    'name', sensor.name,
                    'value', latest_value.value,
                    'connections', connections.count
                )
                ORDER BY connections.count DESC
            ) as data
        FROM sensors sensor
        JOIN (
            SELECT series_id, LAST(value) as value
            FROM metrics
            WHERE timestamp >= NOW() - INTERVAL '1 minute'
            GROUP BY series_id
        ) latest_value ON sensor.id = latest_value.series_id
        JOIN (
            SELECT source.id, COUNT(*) as count
            FROM sensors source
            TRAVERSE OUTBOUND 1..1 STEPS ON connected_to
            GROUP BY source.id
        ) connections ON sensor.id = connections.id
        LIMIT 10
    )

SELECT * FROM system_health
UNION ALL
SELECT metric_type, count::TEXT, null, null FROM active_users
UNION ALL
SELECT metric_type, data::TEXT, null, null FROM top_sensors;
```

### Fraud Detection System

```orbitql
-- Multi-model fraud detection
WITH 
    -- Unusual transaction patterns
    suspicious_transactions AS (
        SELECT 
            t.user_id,
            t.amount,
            t.merchant_id,
            -- Compare with user's historical behavior
            (t.amount - user_stats.avg_amount) / user_stats.stddev_amount as amount_zscore,
            -- Time-based anomaly
            EXTRACT(HOUR FROM t.timestamp) as transaction_hour,
            user_stats.common_hours
        FROM transactions t
        JOIN (
            SELECT 
                user_id,
                AVG(amount) as avg_amount,
                STDDEV(amount) as stddev_amount,
                MODE() WITHIN GROUP (ORDER BY EXTRACT(HOUR FROM timestamp)) as common_hours
            FROM transactions
            WHERE timestamp >= NOW() - INTERVAL '90 days'
            GROUP BY user_id
        ) user_stats ON t.user_id = user_stats.user_id
        WHERE t.timestamp >= NOW() - INTERVAL '1 hour'
          AND (
              ABS(t.amount - user_stats.avg_amount) / user_stats.stddev_amount > 2
              OR ABS(EXTRACT(HOUR FROM t.timestamp) - user_stats.common_hours) > 6
          )
    ),
    
    -- Social network risk assessment
    social_risk AS (
        SELECT 
            u.id as user_id,
            COUNT(CASE WHEN friend.profile->>'risk_score'::FLOAT > 0.7 THEN 1 END) as high_risk_friends,
            AVG(friend.profile->>'risk_score'::FLOAT) as avg_friend_risk
        FROM users u
        TRAVERSE OUTBOUND 1..2 STEPS ON [friends, knows] TO friend
        WHERE friend.profile->>'risk_score' IS NOT NULL
        GROUP BY u.id
    )

SELECT 
    st.user_id,
    u.profile->>'name' as user_name,
    u.profile->>'email' as email,
    st.amount,
    st.amount_zscore,
    sr.high_risk_friends,
    sr.avg_friend_risk,
    -- Combined risk score
    (
        GREATEST(ABS(st.amount_zscore), 0) * 0.4 +
        COALESCE(sr.high_risk_friends * 0.1, 0) +
        COALESCE(sr.avg_friend_risk * 0.5, 0)
    ) as combined_risk_score
FROM suspicious_transactions st
JOIN user_profiles u ON st.user_id = u.user_id
LEFT JOIN social_risk sr ON st.user_id = sr.user_id
WHERE (
    ABS(st.amount_zscore) > 2.5
    OR sr.high_risk_friends > 2
    OR sr.avg_friend_risk > 0.6
)
ORDER BY combined_risk_score DESC;
```

## API Reference

### Query Execution

```rust
use orbit_shared::orbitql::{OrbitQLEngine, QueryBuilder, ExecutionOptions};

// Basic query execution
let result = orbitql_engine.execute(
    "SELECT name, age FROM users WHERE age > @min_age",
    params! { "min_age" => 21 }
).await?;

// Query builder pattern
let query = QueryBuilder::new()
    .select(&["u.name", "COUNT(f.id) as friend_count"])
    .from("users u")
    .traverse("OUTBOUND 1..1 STEPS ON follows TO f")
    .group_by(&["u.id", "u.name"])
    .having("COUNT(f.id) > 5")
    .order_by("friend_count DESC")
    .limit(10);

let result = orbitql_engine.execute_query(query).await?;
```

### Streaming Execution

```rust
use futures_util::stream::StreamExt;

// Stream large result sets
let mut stream = orbitql_engine.execute_stream(
    "SELECT * FROM large_table WHERE created_at >= @start_date",
    params! { "start_date" => start_date }
).await?;

while let Some(batch) = stream.next().await {
    for row in batch? {
        // Process each row
        println!("{:?}", row);
    }
}
```

### Prepared Statements

```rust
// Prepare statement
let prepared = orbitql_engine.prepare(
    "SELECT * FROM users WHERE department = $1 AND age > $2"
).await?;

// Execute with different parameters
let engineers = prepared.execute(&["Engineering", &25]).await?;
let managers = prepared.execute(&["Management", &30]).await?;
```

### Transaction Support

```rust
// Execute in transaction
let tx = orbitql_engine.begin_transaction().await?;

let user_result = tx.execute(
    "INSERT INTO users (name, email) VALUES (@name, @email) RETURNING id",
    params! { "name" => "Alice", "email" => "alice@example.com" }
).await?;

let user_id = user_result.get::<i64>(0, "id")?;

tx.execute(
    "INSERT INTO user_profiles (user_id, preferences) VALUES (@user_id, @prefs)",
    params! { 
        "user_id" => user_id,
        "prefs" => json!({"theme": "dark", "notifications": true})
    }
).await?;

tx.commit().await?;
```

### Schema Integration

```rust
// Define schema
let schema = Schema::builder()
    .table("users")
        .column("id", DataType::BigInt, true) // primary key
        .column("name", DataType::Text, false)
        .column("email", DataType::Text, false)
        .index("idx_users_email", &["email"], true) // unique
    .table("user_profiles") 
        .column("user_id", DataType::BigInt, false)
        .column("preferences", DataType::Json, true)
        .foreign_key("fk_user_profiles_user_id", &["user_id"], "users", &["id"])
    .build();

// Register schema
orbitql_engine.register_schema(schema).await?;

// Type-safe queries with schema validation
let result = orbitql_engine.execute_typed::<User>(
    "SELECT id, name, email FROM users WHERE age > @min_age",
    params! { "min_age" => 21 }
).await?;
```

OrbitQL represents the next generation of query languages, designed specifically for modern distributed, multi-model systems while maintaining the familiarity and power that developers expect from SQL.