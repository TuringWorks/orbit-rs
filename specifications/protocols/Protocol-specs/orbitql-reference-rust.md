# OrbitQL Protocol & Syntax Reference

A comprehensive reference for LLM coding tools (like Claude Code) to create and maintain OrbitQL-related code, including query parsing, execution, and client library development.

---

## Table of Contents

1. [Overview](#overview)
2. [Language Design](#language-design)
3. [Keywords](#keywords)
4. [Data Types](#data-types)
5. [Operators](#operators)
6. [Statements](#statements)
7. [Expressions](#expressions)
8. [Functions](#functions)
9. [Graph Operations](#graph-operations)
10. [Time-Series Operations](#time-series-operations)
11. [Machine Learning Functions](#machine-learning-functions)
12. [Spatial Operations](#spatial-operations)
13. [Streaming Queries](#streaming-queries)
14. [Parser Architecture](#parser-architecture)
15. [Abstract Syntax Tree (AST)](#abstract-syntax-tree-ast)
16. [Implementation Libraries](#implementation-libraries)

---

## Overview

OrbitQL is Orbit-RS's native unified multi-model query language designed specifically for distributed actor systems. It combines SQL familiarity with advanced features for graph traversals, time-series analytics, machine learning, and spatial operations.

### Key Version Information

| Component | Version |
|-----------|---------|
| OrbitQL | 1.0 |
| Default Port (PostgreSQL Wire) | 5432 |
| REST API Port | 8080 |
| Supported Formats | Text, JSON, Binary |

### Key Features

- **Multi-Model Unified Syntax**: Query graphs, documents, time-series, and relational data in single queries
- **Advanced SQL Compatibility**: Full support for CTEs, CASE expressions, window functions, and temporal operations
- **Machine Learning Integration**: Built-in ML functions including XGBoost, LightGBM, CatBoost, and AdaBoost
- **Graph Operations**: Native TRAVERSE and RELATE statements for graph data
- **Time-Series Analytics**: Native NOW(), INTERVAL, and TIME_BUCKET functions
- **Spatial Queries**: PostGIS-compatible spatial functions
- **Real-Time Streaming**: LIVE queries with change notifications
- **Actor-Aware**: Direct integration with Orbit's virtual actor system
- **ACID Compliance**: Full transaction support across distributed data

---

## Language Design

### Basic Query Structure

```orbitql
[WITH recursive_cte AS (...)]
SELECT [DISTINCT] projection
FROM data_sources
[JOIN other_sources ON conditions]
[TRAVERSE graph_operations]
[WHERE conditions]
[GROUP BY grouping_expressions]
[HAVING group_conditions]
[ORDER BY sort_expressions [ASC|DESC]]
[LIMIT count [OFFSET start]]
[FOR UPDATE | FOR SHARE]
```

### Design Principles

1. **SQL Familiarity**: Build on SQL foundations that developers know
2. **Multi-Model**: Seamlessly combine relational, graph, and time-series data
3. **Extensibility**: Clean extensions for specialized operations
4. **Performance**: Query optimization for distributed actor systems
5. **Type Safety**: Compile-time query validation

---

## Keywords

### Keyword Classifications

| Classification | Description |
|---------------|-------------|
| **Reserved** | Cannot be used as identifiers |
| **Non-reserved** | Can be used as table/column names |
| **Context-specific** | Reserved only in certain contexts |

### Reserved Keywords

```text
ALL             AND             AS              ASC
BETWEEN         BY              CASE            COMMIT
CREATE          CROSS           DELETE          DESC
DISTINCT        DROP            ELSE            END
EXISTS          FALSE           FETCH           FOR
FROM            FULL            GROUP           HAVING
IF              IN              INDEX           INNER
INSERT          INTERVAL        INTO            IS
JOIN            LEFT            LIKE            LIMIT
LIVE            MAX_DEPTH       NOT             NOW
NULL            OFFSET          ON              OR
ORDER           OUTER           RELATE          RETURN
RIGHT           ROLLBACK        SELECT          SET
TABLE           THEN            TO              TRAVERSE
TRUE            UNION           UPDATE          VALUES
WHEN            WHERE           WITH
```

### Non-Reserved Keywords

```text
ABORT           ACTION          ADD             AFTER
AGGREGATE       ALGORITHM       ALWAYS          ARRAY
BEFORE          BEGIN           BOOLEAN         CASCADE
CHECK           COLUMN          CONSTRAINT      CONTINUE
COUNT           CURRENT         DATA            DATABASE
DATE            DAY             DEFAULT         DIFF
DISTINCT        DO              DOUBLE          EDGE
ENABLE          EVALUATE        EXTRACT         FEATURES
FILTER          FIRST           FLOAT           FOLLOWING
FOREIGN         FUNCTION        GENERATED       GRAPH
HASH            HOLD            HOUR            IDENTITY
ILIKE           IMMEDIATE       IMPORT          INCLUDE
INCREMENT       INOUT           INT             INTEGER
INTERSECT       KEY             LANGUAGE        LARGE
LAST            LATERAL         LOCAL           LOCATION
LOCK            MATCH           MATRIX          METRIC
MILLISECOND     MINUTE          MODEL           MONTH
NATURAL         NEXT            NODE            NONE
NORMALIZE       NOTHING         NOTIFY          NULLS
OBJECT          ONLY            OPTIONS         OUT
OUTBOUND        OVER            OVERRIDING      OWNED
OWNER           PARALLEL        PARAMETER       PARTIAL
PARTITION       PATH            PATHS           PENDING
PERCENT         PERIOD          PLAN            PRECEDING
PREDICT         PRESERVE        PRIMARY         PRIOR
PROCEDURE       RANGE           READ            RECURSIVE
REF             REFERENCES      REFRESH         RELEASE
RENAME          REPEAT          REPLACE         REPLICA
RESET           RESTRICT        RETURNS         REVERT
REVOKE          ROLE            ROUTINE         ROW
ROWS            SAVEPOINT       SCHEMA          SCORE
SCROLL          SEARCH          SECOND          SECURITY
SEQUENCE        SERIALIZABLE    SESSION         SHARE
SHOW            SIMILAR         SIMPLE          SKIP
SMALLINT        SNAPSHOT        SOME            SQL
STABLE          START           STATEMENT       STATISTICS
STDDEV          STEPS           STORAGE         STORED
STRICT          STRING          SUM             SYMMETRIC
SYSTEM          TARGET          TEMP            TEMPLATE
TEMPORARY       TEXT            TIME            TIMEOUT
TIMESTAMP       TRAIN           TRANSACTION     TRANSFORM
TRIGGER         TRUNCATE        TYPE            UNBOUNDED
UNCOMMITTED     UNIQUE          UNKNOWN         UNLOGGED
UNTIL           USING           UUID            VACUUM
VALID           VALIDATE        VALUE           VARCHAR
VARIANCE        VARYING         VERSION         VIEW
VIRTUAL         VOLATILE        WEEK            WINDOW
WITHOUT         WORK            WRITE           YEAR
ZONE
```

### Graph-Specific Keywords

```text
CONNECTED       EDGE            GRAPH           INBOUND
MAX_DEPTH       NODE            OUTBOUND        PATH
RELATE          STEPS           TRAVERSE
```

### ML-Specific Keywords

```text
ALGORITHM       CATBOOST        EVALUATE        FEATURES
FIT             GRADIENT        LIGHTGBM        MODEL
NORMALIZE       PCA             PREDICT         SCORE
TARGET          TRAIN           TRANSFORM       XGBOOST
```

### Time-Series Keywords

```text
BUCKET          DAY             DURATION        HOUR
INTERVAL        METRIC          MILLISECOND     MINUTE
MONTH           NOW             RANGE           SECOND
SERIES          TIME_BUCKET     TIMESTAMP       WEEK
WINDOW          YEAR
```

---

## Data Types

### Scalar Types

| Type | Description | Literal Examples |
|------|-------------|------------------|
| `BOOLEAN` | True/false value | `true`, `false`, `TRUE`, `FALSE` |
| `INTEGER` | 64-bit signed integer | `42`, `-100`, `0` |
| `FLOAT` | 64-bit IEEE 754 | `3.14159`, `-0.5`, `1.0e10` |
| `STRING` | UTF-8 text | `'Hello'`, `"World"` |
| `DATETIME` | ISO 8601 timestamp | `'2024-01-15T10:30:00Z'::timestamp` |
| `DURATION` | Time duration | `INTERVAL '1 hour'`, `3h`, `30m` |
| `UUID` | Universally unique ID | `'550e8400-e29b-41d4-a716-446655440000'::uuid` |

### Collection Types

| Type | Description | Literal Examples |
|------|-------------|------------------|
| `ARRAY(T)` | Ordered collection | `ARRAY[1, 2, 3]`, `['a', 'b', 'c']` |
| `OBJECT` | Key-value map | `OBJECT('key', 'value', 'num', 42)` |
| `JSON` | JSON document | `'{"name": "Alice"}'::json` |

### Spatial Types

| Type | Description |
|------|-------------|
| `GEOMETRY` | Generic geometry |
| `POINT` | 2D or 3D point |
| `LINESTRING` | Line geometry |
| `POLYGON` | Polygonal area |
| `MULTIPOINT` | Multiple points |
| `MULTILINESTRING` | Multiple lines |
| `MULTIPOLYGON` | Multiple polygons |
| `GEOMETRYCOLLECTION` | Mixed collection |

### Type Casting

```orbitql
-- Explicit CAST
CAST(value AS datatype)

-- PostgreSQL-style cast
value::datatype

-- Examples
CAST('42' AS INTEGER)
'2024-01-15'::timestamp
3.14::integer
```

---

## Operators

### Arithmetic Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `+` | Addition | `a + b` |
| `-` | Subtraction | `a - b` |
| `*` | Multiplication | `a * b` |
| `/` | Division | `a / b` |
| `%` | Modulo | `a % b` |

### Comparison Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `=` | Equal | `a = b` |
| `!=`, `<>` | Not equal | `a != b`, `a <> b` |
| `<` | Less than | `a < b` |
| `<=` | Less than or equal | `a <= b` |
| `>` | Greater than | `a > b` |
| `>=` | Greater than or equal | `a >= b` |

### Logical Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `AND` | Logical AND | `a AND b` |
| `OR` | Logical OR | `a OR b` |
| `NOT` | Logical NOT | `NOT a` |

### Pattern Matching Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `LIKE` | Pattern match (case-sensitive) | `name LIKE 'John%'` |
| `ILIKE` | Pattern match (case-insensitive) | `name ILIKE 'john%'` |
| `NOT LIKE` | Negated pattern match | `name NOT LIKE '%test%'` |
| `MATCH` | Regex match | `email MATCH '^[a-z]+@.*$'` |

### Set Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `IN` | Set membership | `status IN ('active', 'pending')` |
| `NOT IN` | Not in set | `id NOT IN (1, 2, 3)` |
| `BETWEEN` | Range check | `age BETWEEN 18 AND 65` |

### NULL Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `IS NULL` | Null check | `email IS NULL` |
| `IS NOT NULL` | Not null check | `email IS NOT NULL` |

### String Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `CONTAINS` | Substring check | `text CONTAINS 'keyword'` |
| `STARTS_WITH` | Prefix check | `name STARTS_WITH 'Dr.'` |
| `ENDS_WITH` | Suffix check | `email ENDS_WITH '.com'` |
| `\|\|` | Concatenation | `first \|\| ' ' \|\| last` |

### JSON Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `->` | JSON field access (JSON) | `profile->'address'` |
| `->>` | JSON field access (text) | `profile->>'name'` |
| `@>` | JSON contains | `data @> '{"key": "value"}'` |
| `<@` | JSON contained by | `'{"a":1}' <@ data` |

### Graph Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `->` | Outgoing edge | `user->follows->friend` |
| `<-` | Incoming edge | `user<-follows<-follower` |
| `<->` | Bidirectional edge | `node<->connected<->other` |
| `CONNECTED` | Connection check | `a CONNECTED b` |

### Update Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `=` | Set value | `SET count = 10` |
| `+=` | Add to value | `SET count += 1` |
| `-=` | Subtract from value | `SET count -= 1` |
| `*=` | Multiply value | `SET price *= 1.1` |
| `/=` | Divide value | `SET price /= 2` |

---

## Statements

### SELECT Statement

```orbitql
SELECT [DISTINCT] select_list
FROM table_reference [, ...]
[JOIN table_reference ON condition]
[WHERE condition]
[GROUP BY expression [, ...]]
[HAVING condition]
[ORDER BY expression [ASC | DESC] [, ...]]
[LIMIT count]
[OFFSET start]
[FOR UPDATE | FOR SHARE]
```

**Examples**:

```orbitql
-- Basic SELECT
SELECT * FROM users;

-- With conditions
SELECT name, email FROM users WHERE active = true;

-- With aggregation
SELECT department, COUNT(*) as count, AVG(salary) as avg_salary
FROM employees
GROUP BY department
HAVING COUNT(*) > 5;

-- With JOIN
SELECT u.name, o.total
FROM users u
INNER JOIN orders o ON u.id = o.user_id
WHERE o.created_at > NOW() - INTERVAL '30 days';

-- With CTE
WITH active_users AS (
    SELECT * FROM users WHERE last_login > NOW() - INTERVAL '7 days'
)
SELECT * FROM active_users WHERE country = 'US';
```

### INSERT Statement

```orbitql
INSERT INTO table_name [(column [, ...])]
VALUES (value [, ...]) [, ...]
[ON CONFLICT conflict_action]

INSERT INTO table_name [(column [, ...])]
SELECT ...

INSERT INTO table_name
OBJECT(key, value [, ...])
```

**Examples**:

```orbitql
-- Basic INSERT
INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com');

-- Multiple rows
INSERT INTO users (name, email) VALUES
    ('Bob', 'bob@example.com'),
    ('Carol', 'carol@example.com');

-- From SELECT
INSERT INTO archive_users
SELECT * FROM users WHERE created_at < '2020-01-01';

-- Object syntax
INSERT INTO users OBJECT('name', 'David', 'email', 'david@example.com');

-- With conflict handling
INSERT INTO users (id, name) VALUES (1, 'Alice')
ON CONFLICT DO UPDATE SET name = 'Alice Updated';
```

### UPDATE Statement

```orbitql
UPDATE table_name
SET column = expression [, ...]
[WHERE condition]
```

**Examples**:

```orbitql
-- Basic UPDATE
UPDATE users SET status = 'inactive' WHERE last_login < NOW() - INTERVAL '1 year';

-- Multiple columns
UPDATE products
SET price = price * 1.10, updated_at = NOW()
WHERE category = 'electronics';

-- With compound operators
UPDATE counters SET count += 1 WHERE id = 'page_views';
```

### DELETE Statement

```orbitql
DELETE FROM table_name
[WHERE condition]
```

**Examples**:

```orbitql
-- Basic DELETE
DELETE FROM sessions WHERE expires_at < NOW();

-- With complex condition
DELETE FROM logs
WHERE level = 'DEBUG' AND created_at < NOW() - INTERVAL '7 days';
```

### CREATE Statements

```orbitql
-- CREATE TABLE
CREATE TABLE table_name (
    column_name data_type [constraints] [, ...]
    [, table_constraints]
);

-- CREATE INDEX
CREATE [UNIQUE] INDEX index_name
ON table_name (column [, ...]);

-- CREATE VIEW
CREATE VIEW view_name AS
SELECT ...;

-- CREATE FUNCTION
CREATE FUNCTION function_name(parameters)
RETURNS return_type
AS body;
```

**Examples**:

```orbitql
CREATE TABLE users (
    id UUID PRIMARY KEY,
    name STRING NOT NULL,
    email STRING UNIQUE,
    age INTEGER CHECK (age >= 0),
    profile OBJECT,
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_users_email ON users (email);

CREATE VIEW active_users AS
SELECT * FROM users WHERE status = 'active';
```

### DROP Statements

```orbitql
DROP TABLE [IF EXISTS] table_name [CASCADE];
DROP INDEX [IF EXISTS] index_name;
DROP VIEW [IF EXISTS] view_name;
DROP FUNCTION [IF EXISTS] function_name;
```

### Transaction Statements

```orbitql
BEGIN [TRANSACTION];
COMMIT;
ROLLBACK;
```

**Example**:

```orbitql
BEGIN;
UPDATE accounts SET balance = balance - 100 WHERE id = 1;
UPDATE accounts SET balance = balance + 100 WHERE id = 2;
COMMIT;
```

---

## Expressions

### Literal Expressions

```orbitql
-- Numeric
42
3.14159
-100
1.5e10

-- String
'Hello, World!'
"Double quoted string"

-- Boolean
true
false

-- Null
null

-- Array
ARRAY[1, 2, 3, 4, 5]
['red', 'green', 'blue']

-- Object
OBJECT('name', 'Alice', 'age', 30)
```

### CASE Expression

```orbitql
CASE
    WHEN condition1 THEN result1
    WHEN condition2 THEN result2
    ...
    ELSE default_result
END
```

**Examples**:

```orbitql
SELECT
    name,
    CASE
        WHEN age < 18 THEN 'Minor'
        WHEN age < 65 THEN 'Adult'
        ELSE 'Senior'
    END as age_group
FROM users;

-- In aggregation
SELECT
    product_id,
    SUM(CASE WHEN status = 'completed' THEN amount ELSE 0 END) as revenue
FROM orders
GROUP BY product_id;
```

### COALESCE Expression

```orbitql
COALESCE(value1, value2, ..., default_value)
```

**Example**:

```orbitql
SELECT COALESCE(nickname, first_name, 'Anonymous') as display_name
FROM users;
```

### Subquery Expressions

```orbitql
-- Scalar subquery
SELECT name, (SELECT COUNT(*) FROM orders WHERE orders.user_id = users.id) as order_count
FROM users;

-- IN subquery
SELECT * FROM products
WHERE category_id IN (SELECT id FROM categories WHERE active = true);

-- EXISTS subquery
SELECT * FROM users u
WHERE EXISTS (SELECT 1 FROM orders WHERE user_id = u.id);
```

### Field Access

```orbitql
-- Simple field
table.column

-- Nested object access
user.profile.address.city

-- Array index
tags[0]

-- JSON field
data->>'key'
```

---

## Functions

### Aggregate Functions

| Function | Description | Example |
|----------|-------------|---------|
| `COUNT(*)` | Count all rows | `COUNT(*)` |
| `COUNT(column)` | Count non-null values | `COUNT(email)` |
| `COUNT(DISTINCT column)` | Count unique values | `COUNT(DISTINCT user_id)` |
| `SUM(column)` | Sum of values | `SUM(amount)` |
| `AVG(column)` | Average of values | `AVG(price)` |
| `MIN(column)` | Minimum value | `MIN(created_at)` |
| `MAX(column)` | Maximum value | `MAX(score)` |
| `FIRST(column)` | First value | `FIRST(name)` |
| `LAST(column)` | Last value | `LAST(status)` |
| `STDDEV(column)` | Standard deviation | `STDDEV(values)` |
| `VARIANCE(column)` | Variance | `VARIANCE(values)` |

### String Functions

| Function | Description | Example |
|----------|-------------|---------|
| `LENGTH(s)` | String length | `LENGTH(name)` |
| `UPPER(s)` | Uppercase | `UPPER(name)` |
| `LOWER(s)` | Lowercase | `LOWER(email)` |
| `TRIM(s)` | Remove whitespace | `TRIM(input)` |
| `LTRIM(s)` | Left trim | `LTRIM(text)` |
| `RTRIM(s)` | Right trim | `RTRIM(text)` |
| `SUBSTRING(s, start, len)` | Extract substring | `SUBSTRING(name, 1, 3)` |
| `CONCAT(s1, s2, ...)` | Concatenate strings | `CONCAT(first, ' ', last)` |
| `REPLACE(s, from, to)` | Replace substring | `REPLACE(text, 'old', 'new')` |
| `SPLIT(s, delimiter)` | Split to array | `SPLIT(tags, ',')` |

### Numeric Functions

| Function | Description | Example |
|----------|-------------|---------|
| `ABS(n)` | Absolute value | `ABS(-5)` |
| `CEIL(n)` | Ceiling | `CEIL(3.2)` |
| `FLOOR(n)` | Floor | `FLOOR(3.8)` |
| `ROUND(n, d)` | Round to decimals | `ROUND(3.14159, 2)` |
| `POWER(base, exp)` | Exponentiation | `POWER(2, 10)` |
| `SQRT(n)` | Square root | `SQRT(16)` |
| `LOG(n)` | Natural logarithm | `LOG(10)` |
| `LOG10(n)` | Base-10 logarithm | `LOG10(100)` |
| `MOD(a, b)` | Modulo | `MOD(17, 5)` |

### Date/Time Functions

| Function | Description | Example |
|----------|-------------|---------|
| `NOW()` | Current timestamp | `NOW()` |
| `CURRENT_DATE` | Current date | `CURRENT_DATE` |
| `CURRENT_TIME` | Current time | `CURRENT_TIME` |
| `DATE(ts)` | Extract date | `DATE(created_at)` |
| `TIME(ts)` | Extract time | `TIME(created_at)` |
| `YEAR(ts)` | Extract year | `YEAR(birth_date)` |
| `MONTH(ts)` | Extract month | `MONTH(created_at)` |
| `DAY(ts)` | Extract day | `DAY(created_at)` |
| `HOUR(ts)` | Extract hour | `HOUR(timestamp)` |
| `MINUTE(ts)` | Extract minute | `MINUTE(timestamp)` |
| `SECOND(ts)` | Extract second | `SECOND(timestamp)` |
| `DATE_DIFF(a, b)` | Difference between dates | `DATE_DIFF(end, start)` |
| `DATE_ADD(ts, interval)` | Add to date | `DATE_ADD(NOW(), INTERVAL '7 days')` |

### JSON Functions

| Function | Description | Example |
|----------|-------------|---------|
| `JSON_EXTRACT(doc, path)` | Extract JSON value | `JSON_EXTRACT(data, '$.name')` |
| `JSON_ARRAY_LENGTH(arr)` | Array length | `JSON_ARRAY_LENGTH(items)` |
| `JSON_KEYS(obj)` | Get object keys | `JSON_KEYS(metadata)` |
| `JSON_TYPEOF(val)` | Get JSON type | `JSON_TYPEOF(field)` |

### Array Functions

| Function | Description | Example |
|----------|-------------|---------|
| `ARRAY_LENGTH(arr)` | Array length | `ARRAY_LENGTH(tags)` |
| `ARRAY_CONTAINS(arr, val)` | Check contains | `ARRAY_CONTAINS(roles, 'admin')` |
| `ARRAY_APPEND(arr, val)` | Append element | `ARRAY_APPEND(items, 'new')` |
| `ARRAY_CONCAT(arr1, arr2)` | Concatenate arrays | `ARRAY_CONCAT(a, b)` |
| `ARRAY_DISTINCT(arr)` | Unique elements | `ARRAY_DISTINCT(tags)` |

### NULL Handling Functions

| Function | Description | Example |
|----------|-------------|---------|
| `COALESCE(v1, v2, ...)` | First non-null | `COALESCE(nickname, name)` |
| `NULLIF(a, b)` | NULL if equal | `NULLIF(value, 0)` |
| `IFNULL(val, default)` | Default if null | `IFNULL(count, 0)` |

---

## Graph Operations

### TRAVERSE Statement

```orbitql
TRAVERSE edge_type
FROM start_node
[MAX_DEPTH n]
[WHERE condition]
```

**Examples**:

```orbitql
-- Simple traversal
TRAVERSE follows FROM user:alice MAX_DEPTH 2;

-- With conditions
TRAVERSE follows FROM user:alice MAX_DEPTH 3
WHERE active = true;
```

### RELATE Statement

```orbitql
RELATE from_node -> edge_type -> to_node
[SET { property: value, ... }]
```

**Examples**:

```orbitql
-- Create relationship
RELATE user:alice -> follows -> user:bob;

-- With properties
RELATE user:alice -> follows -> user:bob
SET { timestamp: NOW(), strength: 0.8 };
```

### Graph Path Expressions

```orbitql
-- In SELECT
SELECT
    person.name,
    ->follows->friend.name as friend_name
FROM users person
WHERE person.location = 'NYC';

-- Multi-hop traversal
SELECT start.name, end.name
FROM users start
TRAVERSE OUTBOUND 1..5 STEPS ON follows TO end
RETURN PATHS AS path_info
WHERE start.name = 'Alice' AND end.location = 'LA';
```

### Graph Operators

| Syntax | Description |
|--------|-------------|
| `->edge->` | Outgoing edge traversal |
| `<-edge<-` | Incoming edge traversal |
| `<->edge<->` | Bidirectional traversal |
| `OUTBOUND` | Follow outgoing edges |
| `INBOUND` | Follow incoming edges |
| `BOTH` | Follow both directions |

---

## Time-Series Operations

### NOW() Function

```orbitql
SELECT NOW() AS current_time;

SELECT * FROM events
WHERE created_at > NOW() - INTERVAL '1 hour';
```

### INTERVAL Expressions

```orbitql
-- Syntax
INTERVAL 'value unit'

-- Supported units
INTERVAL '1 millisecond'
INTERVAL '30 seconds'
INTERVAL '5 minutes'
INTERVAL '2 hours'
INTERVAL '7 days'
INTERVAL '4 weeks'
INTERVAL '3 months'
INTERVAL '1 year'

-- Short syntax (parser extension)
3h    -- 3 hours
30m   -- 30 minutes
7d    -- 7 days
1y    -- 1 year
```

### TIME_BUCKET Function

```orbitql
TIME_BUCKET(bucket_size, timestamp_column)
```

**Examples**:

```orbitql
-- 15-minute buckets
SELECT
    TIME_BUCKET('15 minutes', timestamp) AS bucket,
    AVG(cpu_usage) AS avg_cpu,
    MAX(memory_usage) AS max_memory
FROM metrics
WHERE timestamp >= NOW() - INTERVAL '6 hours'
GROUP BY TIME_BUCKET('15 minutes', timestamp)
ORDER BY bucket;

-- Daily aggregation
SELECT
    TIME_BUCKET('1 day', created_at) AS day,
    COUNT(*) AS orders
FROM orders
GROUP BY TIME_BUCKET('1 day', created_at);
```

### Time-Series Aggregations

| Aggregation | Description |
|-------------|-------------|
| `AVG` | Average value in bucket |
| `SUM` | Sum of values |
| `COUNT` | Number of points |
| `MIN` | Minimum value |
| `MAX` | Maximum value |
| `FIRST` | First value in window |
| `LAST` | Last value in window |
| `STDDEV` | Standard deviation |
| `PERCENTILE(p)` | Percentile calculation |

---

## Machine Learning Functions

### Model Training

```orbitql
ML_TRAIN_MODEL(
    model_name,
    algorithm,
    features_array,
    target_column,
    [parameters_object]
)
```

**Supported Algorithms**:

| Algorithm | Function | Description |
|-----------|----------|-------------|
| XGBoost | `ML_XGBOOST` | Gradient boosting (tree-based) |
| LightGBM | `ML_LIGHTGBM` | Fast gradient boosting |
| CatBoost | `ML_CATBOOST` | Categorical feature handling |
| AdaBoost | `ML_ADABOOST` | Adaptive boosting |
| Linear Regression | `ML_LINEAR_REGRESSION` | Linear models |
| Logistic Regression | `ML_LOGISTIC_REGRESSION` | Classification |
| Random Forest | `ML_RANDOM_FOREST` | Ensemble trees |
| K-Means | `ML_KMEANS` | Clustering |
| PCA | `ML_PCA` | Dimensionality reduction |

**Examples**:

```orbitql
-- Train XGBoost model
SELECT ML_TRAIN_MODEL(
    'fraud_detector',
    'XGBOOST',
    ARRAY[amount, merchant_category, hour_of_day, user_age],
    is_fraud,
    OBJECT(
        'n_estimators', 100,
        'learning_rate', 0.1,
        'max_depth', 6
    )
) FROM transactions
WHERE created_at > NOW() - INTERVAL '90 days';

-- Train with LightGBM
SELECT ML_LIGHTGBM(
    ARRAY[feature1, feature2, feature3],
    target_column
) FROM training_data;
```

### Model Prediction

```orbitql
ML_PREDICT(model_name, features_array)
```

**Example**:

```orbitql
SELECT
    transaction_id,
    amount,
    ML_PREDICT('fraud_detector', ARRAY[amount, category, hour, age]) AS fraud_score
FROM new_transactions
WHERE ML_PREDICT('fraud_detector', ARRAY[amount, category, hour, age]) > 0.8;
```

### Model Management

```orbitql
-- List all models
SELECT ML_LIST_MODELS();

-- Get model info
SELECT ML_MODEL_INFO('model_name');

-- Evaluate model
SELECT ML_EVALUATE_MODEL(
    'model_name',
    ARRAY[test_features],
    test_target,
    ARRAY['accuracy', 'precision', 'recall', 'f1']
);

-- Drop model
SELECT ML_DROP_MODEL('model_name');
```

### Feature Engineering

```orbitql
-- Normalization
ML_NORMALIZE(values, 'MinMax' | 'ZScore' | 'Robust')

-- Categorical encoding
ML_ENCODE_CATEGORICAL(column, 'OneHot' | 'Label' | 'Target')

-- PCA
ML_PCA(features_array, n_components)

-- Feature selection
ML_FEATURE_SELECTION(features, target, 'method')
```

### Vector Operations

```orbitql
-- Text embedding
ML_EMBED_TEXT(text_column, 'model_name')

-- Similarity search
ML_SIMILARITY_SEARCH(query_vector, target_vectors, k)

-- Clustering
ML_VECTOR_CLUSTER(vectors, k)
```

---

## Spatial Operations

### Spatial Constructors

```orbitql
ST_POINT(x, y)
ST_MAKEPOINT(x, y, z)
ST_LINESTRING(point1, point2, ...)
ST_POLYGON(ring)
ST_GEOMFROMTEXT(wkt_string)
ST_GEOMFROMGEOJSON(geojson)
```

### Spatial Relationships

| Function | Description |
|----------|-------------|
| `ST_CONTAINS(a, b)` | A contains B |
| `ST_WITHIN(a, b)` | A within B |
| `ST_INTERSECTS(a, b)` | A intersects B |
| `ST_OVERLAPS(a, b)` | A overlaps B |
| `ST_TOUCHES(a, b)` | A touches B |
| `ST_CROSSES(a, b)` | A crosses B |
| `ST_DISJOINT(a, b)` | A disjoint from B |
| `ST_EQUALS(a, b)` | A equals B |
| `ST_DWITHIN(a, b, d)` | A within distance d of B |

### Spatial Measurements

| Function | Description |
|----------|-------------|
| `ST_DISTANCE(a, b)` | Distance between geometries |
| `ST_LENGTH(geom)` | Length of linestring |
| `ST_AREA(geom)` | Area of polygon |
| `ST_PERIMETER(geom)` | Perimeter of polygon |

### Spatial Processing

| Function | Description |
|----------|-------------|
| `ST_BUFFER(geom, dist)` | Buffer around geometry |
| `ST_CENTROID(geom)` | Center point |
| `ST_ENVELOPE(geom)` | Bounding box |
| `ST_UNION(a, b)` | Union of geometries |
| `ST_INTERSECTION(a, b)` | Intersection |
| `ST_DIFFERENCE(a, b)` | Difference |
| `ST_SIMPLIFY(geom, tol)` | Simplify geometry |
| `ST_TRANSFORM(geom, srid)` | Transform coordinates |

### Spatial Accessors

| Function | Description |
|----------|-------------|
| `ST_X(point)` | X coordinate |
| `ST_Y(point)` | Y coordinate |
| `ST_ASTEXT(geom)` | WKT representation |
| `ST_ASGEOJSON(geom)` | GeoJSON representation |
| `ST_SRID(geom)` | Spatial reference ID |

**Example**:

```orbitql
-- Find stores within 10km of a location
SELECT name, address
FROM stores
WHERE ST_DWITHIN(
    location,
    ST_POINT(-122.4194, 37.7749),
    10000
);

-- Calculate delivery distance
SELECT
    order_id,
    ST_DISTANCE(customer_location, store_location) AS distance
FROM orders
ORDER BY distance;
```

---

## Streaming Queries

### LIVE Statement

```orbitql
LIVE SELECT select_list
FROM table_reference
[WHERE condition]
[DIFF]
```

**Examples**:

```orbitql
-- Real-time subscription
LIVE SELECT * FROM orders WHERE status = 'pending';

-- With diff mode (receive changes only)
LIVE DIFF SELECT * FROM users WHERE active = true;
```

### Change Types

| Type | Description |
|------|-------------|
| `INSERT` | New row added |
| `UPDATE` | Row modified |
| `DELETE` | Row removed |

### Window Specifications

```orbitql
WINDOW (
    SIZE duration
    [SLIDE duration]
    [WATERMARK duration]
)
```

### Stream Triggers

| Trigger | Description |
|---------|-------------|
| `PROCESSING_TIME(duration)` | Trigger on processing time |
| `EVENT_TIME` | Trigger on event time |
| `COUNT(n)` | Trigger after n items |

---

## Parser Architecture

### Lexer

The OrbitQL lexer tokenizes input into the following token types:

**Token Categories**:

1. **Keywords**: Reserved and non-reserved keywords
2. **Identifiers**: Table names, column names, aliases
3. **Literals**: Numbers, strings, booleans, null
4. **Operators**: Arithmetic, comparison, logical
5. **Punctuation**: Parentheses, brackets, commas
6. **Comments**: Single-line (`--`) and multi-line (`/* */`)

**Token Structure**:

```rust
pub struct Token {
    pub token_type: TokenType,
    pub lexeme: String,
    pub line: usize,
    pub column: usize,
}
```

### Parser

The parser uses recursive descent parsing with the following expression precedence (lowest to highest):

1. OR
2. AND
3. NOT
4. Comparison (`=`, `!=`, `<`, `>`, `<=`, `>=`)
5. Addition/Subtraction (`+`, `-`)
6. Multiplication/Division (`*`, `/`, `%`)
7. Unary (`-`, `NOT`)
8. Primary (literals, identifiers, function calls, parentheses)

### Error Handling

```rust
pub enum ParseError {
    UnexpectedToken {
        expected: Vec<TokenType>,
        found: Token,
    },
    UnexpectedEndOfInput {
        expected: Vec<TokenType>,
    },
    InvalidExpression {
        message: String,
        token: Token,
    },
    LexError(String),
}
```

---

## Abstract Syntax Tree (AST)

### Statement Types

```rust
pub enum Statement {
    Select(SelectStatement),
    Insert(InsertStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement),
    Create(CreateStatement),
    Drop(DropStatement),
    Relate(RelateStatement),
    Traverse(TraverseStatement),
    Live(LiveStatement),
    Transaction(TransactionStatement),
    GraphRAG(GraphRAGStatement),
}
```

### Select Statement

```rust
pub struct SelectStatement {
    pub with_clauses: Vec<WithClause>,
    pub distinct: bool,
    pub fields: Vec<SelectField>,
    pub from: Vec<FromClause>,
    pub join_clauses: Vec<JoinClause>,
    pub where_clause: Option<Expression>,
    pub group_by: Vec<Expression>,
    pub having: Option<Expression>,
    pub order_by: Vec<OrderByClause>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub fetch: Vec<FetchClause>,
    pub for_update: bool,
    pub timeout: Option<Duration>,
}
```

### Expression Types

```rust
pub enum Expression {
    Literal(QueryValue),
    Identifier(String),
    FieldAccess { object: Box<Expression>, field: String },
    IndexAccess { object: Box<Expression>, index: Box<Expression> },
    Binary { left: Box<Expression>, operator: BinaryOperator, right: Box<Expression> },
    Unary { operator: UnaryOperator, operand: Box<Expression> },
    Function { name: String, args: Vec<Expression> },
    Aggregate { function: AggregateFunction, expression: Box<Expression>, distinct: bool },
    Case { when_clauses: Vec<WhenClause>, else_clause: Option<Box<Expression>> },
    Subquery(Box<SelectStatement>),
    Exists(Box<SelectStatement>),
    Array(Vec<Expression>),
    Object(HashMap<String, Expression>),
    Graph(GraphPath),
    TimeSeries { metric: String, filters: Vec<Expression>, aggregation: TimeSeriesAggregation, window: Option<TimeWindow> },
    Geometry(GeometryLiteral),
    SpatialFunction { name: String, args: Vec<Expression>, srid: Option<i32> },
    MLFunction { function: MLFunction, args: Vec<Expression> },
    Parameter(String),
}
```

### Binary Operators

```rust
pub enum BinaryOperator {
    // Arithmetic
    Add, Subtract, Multiply, Divide, Modulo,

    // Comparison
    Equal, NotEqual, LessThan, LessThanOrEqual, GreaterThan, GreaterThanOrEqual,

    // Logical
    And, Or,

    // Pattern matching
    Like, ILike, NotLike, NotILike, Match, NotMatch,

    // Set operations
    In, NotIn, Between,

    // Null checking
    Is, IsNot,

    // String operations
    Contains, StartsWith, EndsWith,

    // JSON operations
    JsonExtract, JsonContains,

    // Graph operations
    Connected, NotConnected,

    // Spatial operations
    SpatialContains, SpatialWithin, SpatialIntersects,
    SpatialOverlaps, SpatialTouches, SpatialCrosses,
    SpatialDisjoint, SpatialEquals, SpatialDWithin(f64),
}
```

### Data Types

```rust
pub enum DataType {
    Boolean,
    Integer,
    Float,
    String { max_length: Option<u32> },
    DateTime,
    Duration,
    Uuid,
    Array(Box<DataType>),
    Object,
    Json,
    Geometry,
    Point,
    LineString,
    Polygon,
    Any,
}
```

---

## Implementation Libraries

### Rust Dependencies

```toml
[dependencies]
# Async runtime
tokio = { version = "1.48", features = ["full"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Date/Time
chrono = { version = "0.4", features = ["serde"] }

# UUID
uuid = { version = "1.0", features = ["v4", "serde"] }

# Error handling
anyhow = "1.0"
thiserror = "1.0"

# Logging
tracing = "0.1"

# Async channels
futures = "0.3"
```

### Code Locations

| Component | Location |
|-----------|----------|
| Lexer | `/orbit/shared/src/orbitql/lexer.rs` |
| Parser | `/orbit/shared/src/orbitql/parser.rs` |
| AST | `/orbit/shared/src/orbitql/ast.rs` |
| Executor | `/orbit/shared/src/orbitql/executor.rs` |
| Optimizer | `/orbit/shared/src/orbitql/optimizer.rs` |
| Planner | `/orbit/shared/src/orbitql/planner.rs` |
| Cache | `/orbit/shared/src/orbitql/cache.rs` |
| Streaming | `/orbit/shared/src/orbitql/streaming.rs` |
| ML Functions | `/orbit/shared/src/orbitql/ml_functions.rs` |
| Spatial | `/orbit/shared/src/orbitql/spatial.rs` |
| LSP Support | `/orbit/shared/src/orbitql/lsp.rs` |

### Implementation Status

| Feature | Status | Completion |
|---------|--------|------------|
| Core SELECT/INSERT/UPDATE/DELETE | ✅ Complete | 100% |
| JOINs (all types) | ✅ Complete | 100% |
| GROUP BY / HAVING | ✅ Complete | 100% |
| ORDER BY / LIMIT / OFFSET | ✅ Complete | 100% |
| CTEs (WITH clause) | ✅ Complete | 100% |
| CASE expressions | ✅ Complete | 100% |
| NOW() / INTERVAL | ✅ Complete | 100% |
| COUNT(DISTINCT) | ✅ Complete | 100% |
| Transactions | ✅ Complete | 100% |
| Graph TRAVERSE / RELATE | ✅ Complete | 95% |
| Time-series queries | ✅ Complete | 95% |
| ML functions | ✅ Complete | 90% |
| Spatial operations | ✅ Complete | 90% |
| LIVE streaming | ✅ Complete | 95% |
| GraphRAG | ✅ Complete | 85% |
| LSP/IDE Support | ✅ Complete | 95% |

---

## Quick Reference

### Common Query Patterns

```orbitql
-- Basic CRUD
SELECT * FROM users WHERE active = true;
INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com');
UPDATE users SET status = 'inactive' WHERE last_login < NOW() - INTERVAL '1 year';
DELETE FROM sessions WHERE expires_at < NOW();

-- Aggregation with CTE
WITH monthly_stats AS (
    SELECT DATE_TRUNC('month', created_at) AS month, COUNT(*) AS orders
    FROM orders
    GROUP BY DATE_TRUNC('month', created_at)
)
SELECT * FROM monthly_stats ORDER BY month DESC;

-- Graph traversal
SELECT person.name, ->follows->friend.name AS friend
FROM users person
TRAVERSE OUTBOUND 1..2 STEPS ON follows TO friend
WHERE person.location = 'NYC';

-- Time-series with buckets
SELECT
    TIME_BUCKET('1 hour', timestamp) AS hour,
    AVG(cpu_usage) AS avg_cpu
FROM metrics
WHERE timestamp >= NOW() - INTERVAL '24 hours'
GROUP BY TIME_BUCKET('1 hour', timestamp);

-- ML prediction
SELECT
    customer_id,
    ML_PREDICT('churn_model', ARRAY[tenure, monthly_charges, total_charges]) AS churn_probability
FROM customers
WHERE contract_type = 'Month-to-month';

-- Spatial query
SELECT name, address
FROM stores
WHERE ST_DWITHIN(location, ST_POINT(-122.4194, 37.7749), 5000);
```

---

*OrbitQL Reference v1.0 - Last Updated: December 2025*
