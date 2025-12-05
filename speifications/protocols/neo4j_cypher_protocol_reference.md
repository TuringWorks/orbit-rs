# Neo4j Cypher Protocol Complete Reference

## Table of Contents

1. [Overview](#overview)
2. [Cypher Query Language](#cypher-query-language)
3. [Cypher Syntax](#cypher-syntax)
4. [Data Types](#data-types)
5. [Clauses Reference](#clauses-reference)
6. [Operators](#operators)
7. [Functions Reference](#functions-reference)
8. [Pattern Matching](#pattern-matching)
9. [Abstract Syntax Tree (AST)](#abstract-syntax-tree-ast)
10. [Parser and Grammar](#parser-and-grammar)
11. [Bolt Wire Protocol](#bolt-wire-protocol)
12. [PackStream Serialization](#packstream-serialization)
13. [Bolt Messages](#bolt-messages)
14. [Structure Semantics](#structure-semantics)
15. [Rust Implementation Guide](#rust-implementation-guide)

---

## Overview

### What is Cypher?

Cypher is Neo4j's declarative graph query language, designed specifically for querying and manipulating property graph databases. It uses an ASCII-art style syntax that visually represents patterns in graphs, making it intuitive to read and write.

### Key Characteristics

- **Declarative**: Describes what to retrieve, not how to retrieve it
- **Pattern-based**: Uses visual ASCII-art patterns to match graph structures
- **GQL Conformant**: Aligns with the Graph Query Language standard
- **SQL-like**: Shares keywords with SQL but optimized for graph operations

### Architecture Components

```
┌─────────────────────────────────────────────────────────────────┐
│                     Client Application                          │
├─────────────────────────────────────────────────────────────────┤
│                        Cypher Query                             │
│                            ↓                                    │
│                    ┌───────────────┐                            │
│                    │    Parser     │ ← Grammar (EBNF)           │
│                    └───────────────┘                            │
│                            ↓                                    │
│                    ┌───────────────┐                            │
│                    │      AST      │                            │
│                    └───────────────┘                            │
│                            ↓                                    │
│                    ┌───────────────┐                            │
│                    │  PackStream   │ ← Serialization            │
│                    └───────────────┘                            │
│                            ↓                                    │
│                    ┌───────────────┐                            │
│                    │ Bolt Protocol │ ← Wire Protocol            │
│                    └───────────────┘                            │
│                            ↓                                    │
│                     Neo4j Server                                │
└─────────────────────────────────────────────────────────────────┘
```

---

## Cypher Query Language

### Basic Query Structure

A Cypher query typically follows this flow:

```cypher
[MATCH pattern]
[WHERE predicate]
[WITH intermediate_results]
[CREATE/MERGE/DELETE/SET operations]
[RETURN expression]
```

### Visual Pattern Syntax

Cypher uses ASCII-art to represent graph patterns:

```
(node)                           -- Node
(node:Label)                     -- Node with label
(node:Label {prop: value})       -- Node with label and properties
(node)-[rel]->(other)            -- Directed relationship
(node)<-[rel]-(other)            -- Incoming relationship
(node)-[rel:TYPE]->(other)       -- Typed relationship
(node)-[rel*1..3]->(other)       -- Variable-length path
```

---

## Cypher Syntax

### Reserved Keywords

#### Reading Clauses
```
MATCH           -- Find patterns in the graph
OPTIONAL MATCH  -- Find patterns, return null if not found
WHERE           -- Filter results
```

#### Writing Clauses
```
CREATE          -- Create nodes/relationships
MERGE           -- Match or create
DELETE          -- Delete nodes/relationships
DETACH DELETE   -- Delete node and all relationships
SET             -- Set properties/labels
REMOVE          -- Remove properties/labels
```

#### Projecting Clauses
```
RETURN          -- Define what to return
WITH            -- Chain query parts
UNWIND          -- Expand a list
```

#### Subquery Clauses
```
CALL            -- Call procedures or subqueries
UNION           -- Combine results
UNION ALL       -- Combine results (with duplicates)
```

#### Administrative Keywords
```
CREATE INDEX
DROP INDEX
CREATE CONSTRAINT
DROP CONSTRAINT
SHOW INDEXES
SHOW CONSTRAINTS
```

### Complete Keyword List

```
ADD             ALL             ALLSHORTESTPATHS    AND
AS              ASC             ASCENDING           BY
CALL            CASE            COMMIT              CONSTRAINT
CONTAINS        COUNT           CREATE              CSV
CYPHER          DELETE          DESC                DESCENDING
DETACH          DISTINCT        DO                  DROP
ELSE            END             ENDS                EXISTS
EXPLAIN         FALSE           FIELDTERMINATOR    FOREACH
FROM            GRANT           GRAPH               HEADERS
IF              IN              INDEX               IS
JOIN            KEY             LIMIT               LOAD
MANDATORY       MATCH           MERGE               NODE
NODES           NONE            NOT                 NULL
OF              ON              OPTIONAL            OR
ORDER           PERIODIC        PROFILE             REDUCE
REL             RELATIONSHIP    RELATIONSHIPS       REMOVE
REQUIRE         RETURN          REVOKE              ROLE
SCAN            SET             SHORTEST            SHORTESTPATH
SHOW            SINGLE          SKIP                START
STARTS          THEN            TO                  TRUE
TYPE            UNION           UNIQUE              UNWIND
USING           WHEN            WHERE               WITH
XOR             YIELD
```

### Identifiers and Naming

```
# Valid identifiers
node_name
_privateVar
camelCase
PascalCase
node123

# Escaped identifiers (for special characters)
`my-node`
`node with spaces`
`123numeric`
```

### Comments

```cypher
// Single-line comment

/* 
   Multi-line
   comment 
*/
```

---

## Data Types

### Property Types (Storable)

| Type | Description | Example |
|------|-------------|---------|
| `BOOLEAN` | True/false value | `true`, `false` |
| `INTEGER` | 64-bit signed integer | `42`, `-17` |
| `FLOAT` | 64-bit floating point | `3.14`, `-0.5` |
| `STRING` | Unicode text (UTF-8) | `"hello"`, `'world'` |
| `DATE` | Calendar date | `date('2024-01-15')` |
| `TIME` | Time with timezone | `time('12:30:00+01:00')` |
| `LOCAL TIME` | Time without timezone | `localtime('12:30:00')` |
| `DATETIME` | Date and time with timezone | `datetime('2024-01-15T12:30:00Z')` |
| `LOCAL DATETIME` | Date and time without timezone | `localdatetime('2024-01-15T12:30:00')` |
| `DURATION` | Temporal amount | `duration('P1Y2M3D')` |
| `POINT` | Spatial 2D/3D point | `point({x: 1, y: 2})` |
| `VECTOR` | Float vector (Enterprise) | Vector for similarity search |

### Structural Types (Non-storable)

| Type | Description |
|------|-------------|
| `NODE` | Graph node with id, labels, properties |
| `RELATIONSHIP` | Graph relationship with id, type, properties |
| `PATH` | Alternating sequence of nodes and relationships |

### Constructed Types

| Type | Description | Example |
|------|-------------|---------|
| `LIST<T>` | Ordered collection | `[1, 2, 3]`, `['a', 'b']` |
| `MAP` | Key-value pairs | `{name: 'Alice', age: 30}` |

### Null Handling

```cypher
-- null represents missing/unknown values
RETURN null IS NULL        -- true
RETURN null = null         -- null (not true!)
RETURN null <> null        -- null
RETURN coalesce(null, 42)  -- 42

-- NOT NULL constraint
RETURN value IS NOT NULL
```

### Type Predicates

```cypher
-- Check value types
WHERE n.prop IS :: INTEGER
WHERE n.prop IS :: STRING
WHERE n.prop IS :: BOOLEAN NOT NULL
WHERE n.prop IS :: LIST<INTEGER>
```

---

## Clauses Reference

### MATCH

Find patterns in the graph.

```cypher
-- Match all nodes
MATCH (n)
RETURN n

-- Match nodes with label
MATCH (p:Person)
RETURN p

-- Match with property filter
MATCH (p:Person {name: 'Alice'})
RETURN p

-- Match relationships
MATCH (a:Person)-[r:KNOWS]->(b:Person)
RETURN a, r, b

-- Match with relationship properties
MATCH (a)-[r:RATED {score: 5}]->(m:Movie)
RETURN a, m

-- Match undirected relationship
MATCH (a)-[r:FRIENDS_WITH]-(b)
RETURN a, b

-- Variable-length paths
MATCH (a)-[*1..3]->(b)  -- 1 to 3 hops
MATCH (a)-[*..5]->(b)   -- up to 5 hops
MATCH (a)-[*3..]->(b)   -- 3 or more hops
MATCH (a)-[*]->(b)      -- any length

-- Named path
MATCH path = (a:Person)-[:KNOWS*]->(b:Person)
RETURN path, length(path)
```

### OPTIONAL MATCH

Like MATCH but returns null for missing patterns.

```cypher
MATCH (p:Person {name: 'Alice'})
OPTIONAL MATCH (p)-[:LIVES_IN]->(c:City)
RETURN p.name, c.name  -- c.name may be null
```

### WHERE

Filter results with predicates.

```cypher
-- Basic comparison
WHERE n.age > 30
WHERE n.name = 'Alice'
WHERE n.active = true

-- String predicates
WHERE n.name STARTS WITH 'A'
WHERE n.name ENDS WITH 'son'
WHERE n.name CONTAINS 'ali'
WHERE n.name =~ '.*[Aa]lice.*'  -- Regex

-- List predicates
WHERE n.age IN [25, 30, 35]
WHERE 'Action' IN labels(n)

-- NULL checks
WHERE n.email IS NOT NULL
WHERE n.deleted IS NULL

-- Boolean logic
WHERE n.age > 18 AND n.active = true
WHERE n.role = 'admin' OR n.role = 'moderator'
WHERE NOT n.banned

-- Path predicates
WHERE (n)-[:KNOWS]->(:Person {name: 'Bob'})
WHERE NOT exists((n)-[:BLOCKED]->())

-- Property existence
WHERE n.email IS NOT NULL
WHERE exists(n.nickname)
```

### CREATE

Create nodes and relationships.

```cypher
-- Create node
CREATE (n:Person {name: 'Alice', age: 30})
RETURN n

-- Create multiple nodes
CREATE (a:Person {name: 'Alice'}),
       (b:Person {name: 'Bob'})

-- Create relationship
MATCH (a:Person {name: 'Alice'}),
      (b:Person {name: 'Bob'})
CREATE (a)-[r:KNOWS {since: 2020}]->(b)
RETURN r

-- Create pattern
CREATE (a:Person {name: 'Charlie'})-[:WORKS_AT]->(c:Company {name: 'Acme'})
```

### MERGE

Match existing or create new.

```cypher
-- Merge node
MERGE (p:Person {name: 'Alice'})
RETURN p

-- Merge with ON CREATE / ON MATCH
MERGE (p:Person {name: 'Alice'})
ON CREATE SET p.created = datetime()
ON MATCH SET p.lastSeen = datetime()
RETURN p

-- Merge relationship
MATCH (a:Person {name: 'Alice'}),
      (b:Person {name: 'Bob'})
MERGE (a)-[r:KNOWS]->(b)
RETURN r
```

### SET

Update properties and labels.

```cypher
-- Set property
MATCH (p:Person {name: 'Alice'})
SET p.age = 31

-- Set multiple properties
SET p.age = 31, p.city = 'NYC'

-- Replace all properties
SET p = {name: 'Alice', age: 31}

-- Merge properties (keep existing)
SET p += {age: 31, city: 'NYC'}

-- Add label
SET p:Employee

-- Remove property
SET p.nickname = null
```

### DELETE / DETACH DELETE

Remove nodes and relationships.

```cypher
-- Delete relationship
MATCH (a)-[r:KNOWS]->(b)
DELETE r

-- Delete node (must have no relationships)
MATCH (n:Person {name: 'Alice'})
DELETE n

-- Delete node and all relationships
MATCH (n:Person {name: 'Alice'})
DETACH DELETE n

-- Delete all
MATCH (n)
DETACH DELETE n
```

### REMOVE

Remove properties and labels.

```cypher
-- Remove property
MATCH (p:Person {name: 'Alice'})
REMOVE p.age

-- Remove label
MATCH (p:Person:Employee {name: 'Alice'})
REMOVE p:Employee
```

### WITH

Chain query parts, project intermediate results.

```cypher
-- Filter and continue
MATCH (p:Person)
WITH p
WHERE p.age > 18
RETURN p

-- Aggregate and continue
MATCH (p:Person)-[:BOUGHT]->(prod:Product)
WITH p, count(prod) as purchaseCount
WHERE purchaseCount > 5
RETURN p.name, purchaseCount

-- Order before limiting
MATCH (p:Person)
WITH p
ORDER BY p.age DESC
LIMIT 10
RETURN p

-- Introduce new variables
WITH 'hello' AS greeting, 2024 AS year
RETURN greeting, year
```

### RETURN

Define query output.

```cypher
-- Return nodes
RETURN n

-- Return properties
RETURN n.name, n.age

-- Return with alias
RETURN n.name AS personName

-- Return all
RETURN *

-- Return distinct
RETURN DISTINCT n.city

-- Order results
RETURN n.name
ORDER BY n.age DESC

-- Limit results
RETURN n
LIMIT 10

-- Skip results
RETURN n
SKIP 5
LIMIT 10
```

### UNWIND

Expand lists into rows.

```cypher
UNWIND [1, 2, 3] AS num
RETURN num

-- With data
UNWIND [{name: 'Alice'}, {name: 'Bob'}] AS person
CREATE (p:Person)
SET p = person
```

### FOREACH

Iterate for side effects.

```cypher
MATCH path = (start)-[*]->(end)
FOREACH (n IN nodes(path) | SET n.visited = true)
```

### CALL (Procedures)

```cypher
-- Call procedure
CALL db.labels()

-- Call with YIELD
CALL db.labels() YIELD label
RETURN label

-- Call subquery
CALL {
  MATCH (p:Person)
  RETURN p
  LIMIT 10
}
RETURN p.name
```

### UNION

Combine results from multiple queries.

```cypher
-- Union (removes duplicates)
MATCH (p:Person)
RETURN p.name AS name
UNION
MATCH (c:Company)
RETURN c.name AS name

-- Union All (keeps duplicates)
MATCH (p:Person)
RETURN p.name AS name
UNION ALL
MATCH (c:Company)
RETURN c.name AS name
```

---

## Operators

### Arithmetic Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `+` | Addition | `5 + 3` → `8` |
| `-` | Subtraction | `5 - 3` → `2` |
| `*` | Multiplication | `5 * 3` → `15` |
| `/` | Division | `5 / 2` → `2.5` |
| `%` | Modulo | `5 % 2` → `1` |
| `^` | Exponentiation | `2 ^ 3` → `8.0` |

### Comparison Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `=` | Equal | `n.age = 30` |
| `<>` | Not equal | `n.age <> 30` |
| `<` | Less than | `n.age < 30` |
| `>` | Greater than | `n.age > 30` |
| `<=` | Less than or equal | `n.age <= 30` |
| `>=` | Greater than or equal | `n.age >= 30` |
| `IS NULL` | Is null | `n.email IS NULL` |
| `IS NOT NULL` | Is not null | `n.email IS NOT NULL` |

### Boolean Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `AND` | Logical and | `a AND b` |
| `OR` | Logical or | `a OR b` |
| `NOT` | Logical not | `NOT a` |
| `XOR` | Exclusive or | `a XOR b` |

### String Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `+` | Concatenation | `'Hello' + ' World'` |
| `STARTS WITH` | Prefix match | `n.name STARTS WITH 'A'` |
| `ENDS WITH` | Suffix match | `n.name ENDS WITH 'son'` |
| `CONTAINS` | Substring match | `n.name CONTAINS 'ali'` |
| `=~` | Regex match | `n.name =~ '.*Alice.*'` |

### List Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `+` | Concatenation | `[1, 2] + [3, 4]` |
| `IN` | List membership | `3 IN [1, 2, 3]` |
| `[n]` | Index access | `list[0]` |
| `[m..n]` | Slice | `list[1..3]` |

### Property Access

```cypher
-- Static property access
n.name
n.`property-with-dashes`

-- Dynamic property access
n[$propertyName]

-- Map access
map.key
map['key']
```

---

## Functions Reference

### Aggregating Functions

| Function | Description |
|----------|-------------|
| `count(expr)` | Count non-null values |
| `count(*)` | Count all rows |
| `sum(expr)` | Sum of numeric values |
| `avg(expr)` | Average of numeric values |
| `min(expr)` | Minimum value |
| `max(expr)` | Maximum value |
| `collect(expr)` | Collect values into list |
| `stDev(expr)` | Standard deviation (sample) |
| `stDevP(expr)` | Standard deviation (population) |
| `percentileCont(expr, p)` | Continuous percentile |
| `percentileDisc(expr, p)` | Discrete percentile |

```cypher
MATCH (p:Person)
RETURN count(p),
       avg(p.age),
       min(p.age),
       max(p.age),
       collect(p.name)
```

### Scalar Functions

| Function | Description | Example |
|----------|-------------|---------|
| `id(node)` | Internal node/rel ID | `id(n)` |
| `elementId(node)` | Element ID string | `elementId(n)` |
| `type(rel)` | Relationship type | `type(r)` |
| `labels(node)` | Node labels as list | `labels(n)` |
| `keys(expr)` | Property keys | `keys(n)` |
| `properties(expr)` | Properties as map | `properties(n)` |
| `coalesce(e1, e2, ...)` | First non-null | `coalesce(n.nick, n.name)` |
| `head(list)` | First element | `head([1,2,3])` → `1` |
| `last(list)` | Last element | `last([1,2,3])` → `3` |
| `size(list)` | List length | `size([1,2,3])` → `3` |
| `length(path)` | Path length | `length(p)` |
| `timestamp()` | Current timestamp (ms) | `timestamp()` |
| `randomUUID()` | Generate UUID | `randomUUID()` |

### String Functions

| Function | Description | Example |
|----------|-------------|---------|
| `toString(expr)` | Convert to string | `toString(123)` |
| `toUpper(str)` | Uppercase | `toUpper('hello')` |
| `toLower(str)` | Lowercase | `toLower('HELLO')` |
| `trim(str)` | Remove whitespace | `trim(' hello ')` |
| `ltrim(str)` | Left trim | `ltrim(' hello')` |
| `rtrim(str)` | Right trim | `rtrim('hello ')` |
| `left(str, n)` | Left n chars | `left('hello', 2)` |
| `right(str, n)` | Right n chars | `right('hello', 2)` |
| `substring(str, start, len)` | Substring | `substring('hello', 1, 3)` |
| `replace(str, from, to)` | Replace substring | `replace('hello', 'l', 'L')` |
| `split(str, delim)` | Split to list | `split('a,b,c', ',')` |
| `reverse(str)` | Reverse string | `reverse('hello')` |

### Mathematical Functions

| Function | Description |
|----------|-------------|
| `abs(x)` | Absolute value |
| `ceil(x)` | Round up |
| `floor(x)` | Round down |
| `round(x)` | Round to nearest |
| `round(x, precision)` | Round with precision |
| `sign(x)` | Sign (-1, 0, 1) |
| `sqrt(x)` | Square root |
| `log(x)` | Natural logarithm |
| `log10(x)` | Base-10 logarithm |
| `exp(x)` | Exponential |
| `sin(x)`, `cos(x)`, `tan(x)` | Trigonometric |
| `asin(x)`, `acos(x)`, `atan(x)` | Inverse trig |
| `rand()` | Random 0.0 to 1.0 |
| `e()` | Euler's number |
| `pi()` | Pi |

### List Functions

| Function | Description | Example |
|----------|-------------|---------|
| `range(start, end)` | Generate range | `range(1, 5)` → `[1,2,3,4,5]` |
| `range(start, end, step)` | Range with step | `range(0, 10, 2)` |
| `reverse(list)` | Reverse list | `reverse([1,2,3])` |
| `tail(list)` | All but first | `tail([1,2,3])` → `[2,3]` |
| `reduce(acc, x IN list \| expr)` | Reduce list | See below |
| `[x IN list WHERE pred]` | List filter | `[x IN [1,2,3] WHERE x > 1]` |
| `[x IN list \| expr]` | List map | `[x IN [1,2,3] \| x * 2]` |

```cypher
-- Reduce example
RETURN reduce(total = 0, x IN [1,2,3] | total + x) AS sum
-- Returns 6
```

### Path Functions

| Function | Description |
|----------|-------------|
| `nodes(path)` | List of nodes in path |
| `relationships(path)` | List of relationships |
| `length(path)` | Number of relationships |
| `shortestPath(pattern)` | Find shortest path |
| `allShortestPaths(pattern)` | All shortest paths |

```cypher
MATCH path = shortestPath((a:Person)-[*]-(b:Person))
WHERE a.name = 'Alice' AND b.name = 'Bob'
RETURN path, length(path)
```

### Temporal Functions

```cypher
-- Current time
RETURN datetime()      -- Current datetime with timezone
RETURN localdatetime() -- Current local datetime
RETURN date()          -- Current date
RETURN time()          -- Current time with timezone
RETURN localtime()     -- Current local time

-- Create temporal values
RETURN date('2024-01-15')
RETURN datetime('2024-01-15T12:30:00Z')
RETURN duration('P1Y2M3DT4H5M6S')

-- Temporal arithmetic
RETURN date() + duration('P1M')  -- Add 1 month
RETURN datetime() - duration('PT1H')  -- Subtract 1 hour

-- Extract components
RETURN datetime().year
RETURN datetime().month
RETURN datetime().day
RETURN datetime().hour
```

### Spatial Functions

```cypher
-- Create points
RETURN point({x: 3.0, y: 4.0})  -- 2D Cartesian
RETURN point({x: 3.0, y: 4.0, z: 5.0})  -- 3D Cartesian
RETURN point({latitude: 40.7128, longitude: -74.0060})  -- WGS84

-- Distance calculation
RETURN point.distance(
  point({latitude: 40.7128, longitude: -74.0060}),
  point({latitude: 34.0522, longitude: -118.2437})
)
```

### Type Conversion Functions

| Function | Description |
|----------|-------------|
| `toBoolean(expr)` | Convert to boolean |
| `toBooleanOrNull(expr)` | Convert or return null |
| `toFloat(expr)` | Convert to float |
| `toFloatOrNull(expr)` | Convert or return null |
| `toInteger(expr)` | Convert to integer |
| `toIntegerOrNull(expr)` | Convert or return null |
| `toString(expr)` | Convert to string |
| `toStringOrNull(expr)` | Convert or return null |

---

## Pattern Matching

### Node Patterns

```cypher
()                          -- Anonymous node
(n)                         -- Variable binding
(n:Person)                  -- With label
(n:Person:Employee)         -- Multiple labels
(n:Person {name: 'Alice'})  -- With properties
(n:Person {name: $name})    -- With parameter
(n:Person&Employee)         -- Label AND
(n:Person|Company)          -- Label OR
(n:!Person)                 -- Label NOT
```

### Relationship Patterns

```cypher
-[r]->                      -- Outgoing
<-[r]-                      -- Incoming
-[r]-                       -- Undirected
-[:KNOWS]->                 -- With type
-[:KNOWS|LIKES]->           -- Type OR
-[r:KNOWS {since: 2020}]->  -- With properties
-[*]->                      -- Variable length (any)
-[*1..3]->                  -- 1 to 3 hops
-[*..5]->                   -- Up to 5 hops
-[*2..]->                   -- At least 2 hops
-[r:KNOWS*1..3]->           -- Typed variable length
```

### Path Patterns

```cypher
-- Bind path to variable
MATCH path = (a)-[*]->(b)

-- Shortest path
MATCH path = shortestPath((a)-[*]-(b))

-- All shortest paths
MATCH path = allShortestPaths((a)-[*]-(b))

-- Quantified path patterns (GPM)
MATCH (a)-->+(b)            -- One or more hops
MATCH (a)-->*(b)            -- Zero or more hops
MATCH (a)-->{2,5}(b)        -- 2 to 5 hops
```

### Complex Patterns

```cypher
-- Multiple patterns
MATCH (a:Person)-[:KNOWS]->(b:Person),
      (b)-[:WORKS_AT]->(c:Company)
RETURN a, b, c

-- Pattern with WHERE
MATCH (a)-[r]->(b)
WHERE type(r) IN ['KNOWS', 'LIKES']
RETURN a, r, b

-- Path existence in WHERE
MATCH (a:Person)
WHERE (a)-[:KNOWS]->(:Person {name: 'Bob'})
RETURN a

-- Negative pattern
MATCH (a:Person)
WHERE NOT (a)-[:BLOCKED]->()
RETURN a
```

---

## Abstract Syntax Tree (AST)

### Overview

The Cypher parser produces an Abstract Syntax Tree (AST) that represents the semantic structure of a query. Understanding the AST is essential for implementing a Cypher parser or building tools that analyze Cypher queries.

### AST Node Types

#### Query Nodes

```rust
enum Query {
    SingleQuery(SingleQuery),
    Union(UnionQuery),
}

struct SingleQuery {
    clauses: Vec<Clause>,
}

struct UnionQuery {
    all: bool,  // UNION vs UNION ALL
    left: Box<Query>,
    right: Box<Query>,
}
```

#### Clause Nodes

```rust
enum Clause {
    Match(MatchClause),
    OptionalMatch(MatchClause),
    Create(CreateClause),
    Merge(MergeClause),
    Delete(DeleteClause),
    Set(SetClause),
    Remove(RemoveClause),
    With(WithClause),
    Return(ReturnClause),
    Unwind(UnwindClause),
    Call(CallClause),
    Foreach(ForeachClause),
}

struct MatchClause {
    pattern: Pattern,
    where_clause: Option<Expression>,
    hints: Vec<Hint>,
}

struct ReturnClause {
    distinct: bool,
    items: Vec<ReturnItem>,
    order_by: Option<Vec<SortItem>>,
    skip: Option<Expression>,
    limit: Option<Expression>,
}
```

#### Pattern Nodes

```rust
struct Pattern {
    parts: Vec<PatternPart>,
}

struct PatternPart {
    variable: Option<Variable>,
    element: PatternElement,
}

enum PatternElement {
    NodePattern(NodePattern),
    RelationshipChain(Vec<PatternElement>),
}

struct NodePattern {
    variable: Option<Variable>,
    labels: Vec<Label>,
    properties: Option<MapExpression>,
    where_clause: Option<Expression>,
}

struct RelationshipPattern {
    variable: Option<Variable>,
    types: Vec<RelationshipType>,
    range: Option<Range>,
    properties: Option<MapExpression>,
    direction: Direction,
}

enum Direction {
    Outgoing,    // -->
    Incoming,    // <--
    Both,        // --
}
```

#### Expression Nodes

```rust
enum Expression {
    // Literals
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<Expression>),
    Map(Vec<(String, Expression)>),
    
    // References
    Variable(Variable),
    Parameter(String),
    Property(Box<Expression>, String),
    
    // Operations
    BinaryOp(BinaryOp, Box<Expression>, Box<Expression>),
    UnaryOp(UnaryOp, Box<Expression>),
    
    // Function calls
    FunctionCall(FunctionCall),
    
    // Case expression
    Case(CaseExpression),
    
    // List comprehension
    ListComprehension(ListComprehension),
    
    // Pattern expression
    PatternExpression(Pattern),
    
    // Exists subquery
    ExistsSubquery(Query),
}

enum BinaryOp {
    // Arithmetic
    Add, Subtract, Multiply, Divide, Modulo, Power,
    // Comparison
    Eq, Ne, Lt, Gt, Lte, Gte,
    // Boolean
    And, Or, Xor,
    // String
    StartsWith, EndsWith, Contains, RegexMatch,
    // List
    In,
}

enum UnaryOp {
    Not,
    Negate,
    IsNull,
    IsNotNull,
}
```

### AST Example

For the query:
```cypher
MATCH (p:Person {name: 'Alice'})-[:KNOWS]->(f:Person)
WHERE f.age > 18
RETURN p.name, f.name
```

The AST structure would be:

```
SingleQuery
└── clauses
    ├── MatchClause
    │   ├── pattern
    │   │   └── PatternPart
    │   │       └── RelationshipChain
    │   │           ├── NodePattern {variable: p, labels: [Person], properties: {name: 'Alice'}}
    │   │           ├── RelationshipPattern {types: [KNOWS], direction: Outgoing}
    │   │           └── NodePattern {variable: f, labels: [Person]}
    │   └── where_clause
    │       └── BinaryOp(Gt)
    │           ├── Property(Variable(f), "age")
    │           └── Integer(18)
    └── ReturnClause
        └── items
            ├── ReturnItem {expression: Property(Variable(p), "name")}
            └── ReturnItem {expression: Property(Variable(f), "name")}
```

---

## Parser and Grammar

### openCypher Grammar (EBNF)

The openCypher project provides the formal grammar specification. Here are the key production rules:

#### Top-Level Rules

```ebnf
Cypher = Statement ;

Statement = Query ;

Query = RegularQuery
      | StandaloneCall
      ;

RegularQuery = SingleQuery { Union } ;

Union = ( UNION ALL? SingleQuery ) ;

SingleQuery = Clause+ ;
```

#### Clause Rules

```ebnf
Clause = Match
       | Unwind
       | Merge
       | Create
       | Set
       | Delete
       | Remove
       | With
       | Return
       ;

Match = OPTIONAL? MATCH Pattern Where? ;

Return = RETURN DISTINCT? ReturnBody ;

ReturnBody = ReturnItems Order? Skip? Limit? ;
```

#### Pattern Rules

```ebnf
Pattern = PatternPart { ',' PatternPart } ;

PatternPart = Variable '=' AnonymousPatternPart
            | AnonymousPatternPart
            ;

AnonymousPatternPart = PatternElement ;

PatternElement = NodePattern { PatternElementChain }
               | '(' PatternElement ')'
               ;

NodePattern = '(' Variable? NodeLabels? Properties? ')' ;

PatternElementChain = RelationshipPattern NodePattern ;

RelationshipPattern = LeftArrowHead? Dash RelationshipDetail? Dash RightArrowHead? ;

RelationshipDetail = '[' Variable? RelationshipTypes? RangeLiteral? Properties? ']' ;
```

#### Expression Rules

```ebnf
Expression = OrExpression ;

OrExpression = XorExpression { OR XorExpression } ;

XorExpression = AndExpression { XOR AndExpression } ;

AndExpression = NotExpression { AND NotExpression } ;

NotExpression = NOT* ComparisonExpression ;

ComparisonExpression = AddOrSubtractExpression { PartialComparisonExpression } ;

AddOrSubtractExpression = MultiplyDivideModuloExpression 
                          { ( '+' | '-' ) MultiplyDivideModuloExpression } ;

MultiplyDivideModuloExpression = PowerOfExpression 
                                  { ( '*' | '/' | '%' ) PowerOfExpression } ;

PowerOfExpression = UnaryAddOrSubtractExpression 
                    { '^' UnaryAddOrSubtractExpression } ;

UnaryAddOrSubtractExpression = ( '+' | '-' )* PropertyOrLabelsExpression ;

PropertyOrLabelsExpression = Atom { PropertyLookup } { NodeLabels } ;

Atom = Literal
     | Parameter
     | CaseExpression
     | CountAll
     | ListComprehension
     | PatternComprehension
     | Quantifier
     | PatternPredicate
     | ParenthesizedExpression
     | FunctionInvocation
     | ExistentialSubquery
     | Variable
     ;
```

### Token Types

```rust
enum TokenType {
    // Keywords
    Match, Optional, Where, Return, With, Unwind,
    Create, Merge, Delete, Detach, Set, Remove,
    Order, By, Asc, Desc, Skip, Limit,
    And, Or, Not, Xor, In, Is, Null,
    True, False, As, Distinct, All,
    Union, Case, When, Then, Else, End,
    
    // Punctuation
    LeftParen,      // (
    RightParen,     // )
    LeftBracket,    // [
    RightBracket,   // ]
    LeftBrace,      // {
    RightBrace,     // }
    Colon,          // :
    Semicolon,      // ;
    Comma,          // ,
    Dot,            // .
    DoubleDot,      // ..
    Pipe,           // |
    
    // Arrows
    LeftArrow,      // <-
    RightArrow,     // ->
    Dash,           // -
    
    // Operators
    Plus,           // +
    Minus,          // -
    Asterisk,       // *
    Slash,          // /
    Percent,        // %
    Caret,          // ^
    Equals,         // =
    NotEquals,      // <>
    LessThan,       // <
    GreaterThan,    // >
    LessOrEqual,    // <=
    GreaterOrEqual, // >=
    RegexMatch,     // =~
    PlusEquals,     // +=
    
    // Literals
    Integer(i64),
    Float(f64),
    String(String),
    
    // Identifiers
    Identifier(String),
    Parameter(String),  // $param
    
    // Misc
    Whitespace,
    Comment,
    Eof,
}
```

### Parser Implementation Strategy

For Rust implementation, consider using:

1. **Hand-written Recursive Descent Parser**
   - Full control over error messages
   - Best performance
   - More code to maintain

2. **Parser Combinator Libraries**
   - `nom` - Zero-copy, streaming parser
   - `pest` - PEG parser with grammar files
   - `lalrpop` - LALR parser generator

Example with `pest`:

```pest
// cypher.pest
query = { single_query ~ (union ~ single_query)* }
single_query = { clause+ }
clause = { match_clause | return_clause | create_clause | ... }

match_clause = { ^"MATCH" ~ pattern ~ where_clause? }
pattern = { pattern_part ~ ("," ~ pattern_part)* }
pattern_part = { variable? ~ "=" ~ pattern_element | pattern_element }

node_pattern = { "(" ~ variable? ~ node_labels? ~ properties? ~ ")" }
node_labels = { node_label+ }
node_label = { ":" ~ label_name }
```

---

## Bolt Wire Protocol

### Overview

Bolt is a binary protocol for communication with Neo4j databases over TCP or WebSocket. It operates on port **7687** by default.

### Protocol Versions

| Version | Neo4j Version | Key Features |
|---------|---------------|--------------|
| 1.0 | 3.0 | Initial release |
| 2.0 | 3.4 | Points, temporal types |
| 3.0 | 3.5 | Bookmarks, metadata |
| 4.0 | 4.0 | Multi-database, reactive |
| 4.1 | 4.1 | Server-side routing |
| 4.2 | 4.2 | Notification filtering |
| 4.3 | 4.3 | Minor version ranges |
| 4.4 | 4.4 | UTC datetime fix |
| 5.0 | 5.0 | Element IDs |
| 5.1+ | 5.x | Incremental improvements |

### Handshake

1. **Client sends magic bytes**: `60 60 B0 17` (Bolt identification)
2. **Client sends 4 version proposals**: Each 4 bytes, big-endian
3. **Server responds**: Selected version (4 bytes) or `00 00 00 00` if none supported

```
Client: 60 60 B0 17                          # Magic bytes
Client: 00 00 05 04 00 00 04 04 00 00 00 03 00 00 00 02  # Versions 5.4, 4.4, 3, 2
Server: 00 00 05 04                          # Selected 5.4
```

### Version Range (4.3+)

```
Byte layout: [reserved] [range] [minor] [major]
Example: 00 03 04 04 means versions 4.4, 4.3, 4.2, 4.1 (range of 3 below 4.4)
```

### Message Chunking

Messages are sent in chunks:

```
┌──────────────┬──────────────────────────┐
│ Size (2 B)   │ Data (variable)          │
├──────────────┼──────────────────────────┤
│ Size (2 B)   │ Data (variable)          │
├──────────────┼──────────────────────────┤
│ 00 00        │ End of message marker    │
└──────────────┴──────────────────────────┘
```

- Chunk size: 16-bit unsigned big-endian integer
- Maximum chunk size: 65,535 bytes
- Message end: zero-length chunk (00 00)

### Connection States

```
┌──────────────┐
│  CONNECTED   │  (initial state after handshake)
└──────┬───────┘
       │ HELLO
       ▼
┌──────────────┐
│   READY      │  (authenticated, ready for queries)
└──────┬───────┘
       │ RUN
       ▼
┌──────────────┐
│  STREAMING   │  (results available)
└──────┬───────┘
       │ PULL/DISCARD
       ▼
┌──────────────┐
│   READY      │
└──────────────┘

On FAILURE:
┌──────────────┐
│   FAILED     │  (requires RESET)
└──────┬───────┘
       │ RESET
       ▼
┌──────────────┐
│   READY      │
└──────────────┘
```

---

## PackStream Serialization

### Overview

PackStream is the binary serialization format used by Bolt. It's similar to MessagePack but with Neo4j-specific extensions.

### Marker Bytes

Every value starts with a marker byte that indicates its type and potentially its size.

| Marker Range | Type |
|--------------|------|
| `00` - `7F` | Positive tiny int (0 to 127) |
| `80` - `8F` | Tiny string (0-15 bytes) |
| `90` - `9F` | Tiny list (0-15 items) |
| `A0` - `AF` | Tiny map (0-15 entries) |
| `B0` - `BF` | Tiny structure (0-15 fields) |
| `C0` | Null |
| `C1` | Float64 |
| `C2` | Boolean false |
| `C3` | Boolean true |
| `C8` | Int8 |
| `C9` | Int16 |
| `CA` | Int32 |
| `CB` | Int64 |
| `CC` | Bytes8 (size as uint8) |
| `CD` | Bytes16 (size as uint16) |
| `CE` | Bytes32 (size as uint32) |
| `D0` | String8 (size as uint8) |
| `D1` | String16 (size as uint16) |
| `D2` | String32 (size as uint32) |
| `D4` | List8 (size as uint8) |
| `D5` | List16 (size as uint16) |
| `D6` | List32 (size as uint32) |
| `D8` | Map8 (size as uint8) |
| `D9` | Map16 (size as uint16) |
| `DA` | Map32 (size as uint32) |
| `F0` - `FF` | Negative tiny int (-16 to -1) |

### Encoding Examples

#### Null
```
C0
```

#### Boolean
```
C2    # false
C3    # true
```

#### Integers
```
2A                      # 42 (tiny int)
C8 2A                   # 42 (int8)
C9 00 2A                # 42 (int16)
CA 00 00 00 2A          # 42 (int32)
CB 00 00 00 00 00 00 00 2A  # 42 (int64)
```

#### Float
```
C1 3F F3 AE 14 7A E1 47 AE  # 1.23 (IEEE 754 double)
```

#### String
```
85 48 65 6C 6C 6F       # "Hello" (tiny string, 5 bytes)
D0 1A 41 42 43...       # 26-byte string
```

#### List
```
93 01 02 03             # [1, 2, 3] (tiny list, 3 items)
D4 0A 01 02...          # 10-item list
```

#### Map
```
A1 83 6B 65 79 01       # {key: 1} (tiny map, 1 entry)
D8 05 ...               # 5-entry map
```

#### Structure
```
B3 4E ...               # 3-field structure with tag 0x4E (Node)
```

### Rust PackStream Implementation

```rust
use std::io::{Read, Write};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    Bytes(Vec<u8>),
    String(String),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
    Structure { tag: u8, fields: Vec<Value> },
}

impl Value {
    pub fn pack<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        match self {
            Value::Null => writer.write_u8(0xC0),
            
            Value::Boolean(false) => writer.write_u8(0xC2),
            Value::Boolean(true) => writer.write_u8(0xC3),
            
            Value::Integer(n) => {
                if *n >= -16 && *n < 128 {
                    writer.write_i8(*n as i8)
                } else if *n >= i8::MIN as i64 && *n <= i8::MAX as i64 {
                    writer.write_u8(0xC8)?;
                    writer.write_i8(*n as i8)
                } else if *n >= i16::MIN as i64 && *n <= i16::MAX as i64 {
                    writer.write_u8(0xC9)?;
                    writer.write_i16::<BigEndian>(*n as i16)
                } else if *n >= i32::MIN as i64 && *n <= i32::MAX as i64 {
                    writer.write_u8(0xCA)?;
                    writer.write_i32::<BigEndian>(*n as i32)
                } else {
                    writer.write_u8(0xCB)?;
                    writer.write_i64::<BigEndian>(*n)
                }
            }
            
            Value::Float(f) => {
                writer.write_u8(0xC1)?;
                writer.write_f64::<BigEndian>(*f)
            }
            
            Value::String(s) => {
                let bytes = s.as_bytes();
                let len = bytes.len();
                if len < 16 {
                    writer.write_u8(0x80 | len as u8)?;
                } else if len <= 255 {
                    writer.write_u8(0xD0)?;
                    writer.write_u8(len as u8)?;
                } else if len <= 65535 {
                    writer.write_u8(0xD1)?;
                    writer.write_u16::<BigEndian>(len as u16)?;
                } else {
                    writer.write_u8(0xD2)?;
                    writer.write_u32::<BigEndian>(len as u32)?;
                }
                writer.write_all(bytes)
            }
            
            Value::List(items) => {
                let len = items.len();
                if len < 16 {
                    writer.write_u8(0x90 | len as u8)?;
                } else if len <= 255 {
                    writer.write_u8(0xD4)?;
                    writer.write_u8(len as u8)?;
                } else if len <= 65535 {
                    writer.write_u8(0xD5)?;
                    writer.write_u16::<BigEndian>(len as u16)?;
                } else {
                    writer.write_u8(0xD6)?;
                    writer.write_u32::<BigEndian>(len as u32)?;
                }
                for item in items {
                    item.pack(writer)?;
                }
                Ok(())
            }
            
            Value::Map(entries) => {
                let len = entries.len();
                if len < 16 {
                    writer.write_u8(0xA0 | len as u8)?;
                } else if len <= 255 {
                    writer.write_u8(0xD8)?;
                    writer.write_u8(len as u8)?;
                } else if len <= 65535 {
                    writer.write_u8(0xD9)?;
                    writer.write_u16::<BigEndian>(len as u16)?;
                } else {
                    writer.write_u8(0xDA)?;
                    writer.write_u32::<BigEndian>(len as u32)?;
                }
                for (key, value) in entries {
                    Value::String(key.clone()).pack(writer)?;
                    value.pack(writer)?;
                }
                Ok(())
            }
            
            Value::Structure { tag, fields } => {
                let len = fields.len();
                if len < 16 {
                    writer.write_u8(0xB0 | len as u8)?;
                } else {
                    panic!("Structure too large");
                }
                writer.write_u8(*tag)?;
                for field in fields {
                    field.pack(writer)?;
                }
                Ok(())
            }
            
            Value::Bytes(bytes) => {
                let len = bytes.len();
                if len <= 255 {
                    writer.write_u8(0xCC)?;
                    writer.write_u8(len as u8)?;
                } else if len <= 65535 {
                    writer.write_u8(0xCD)?;
                    writer.write_u16::<BigEndian>(len as u16)?;
                } else {
                    writer.write_u8(0xCE)?;
                    writer.write_u32::<BigEndian>(len as u32)?;
                }
                writer.write_all(bytes)
            }
        }
    }
    
    pub fn unpack<R: Read>(reader: &mut R) -> std::io::Result<Value> {
        let marker = reader.read_u8()?;
        
        match marker {
            0xC0 => Ok(Value::Null),
            0xC2 => Ok(Value::Boolean(false)),
            0xC3 => Ok(Value::Boolean(true)),
            
            // Tiny positive int
            0x00..=0x7F => Ok(Value::Integer(marker as i64)),
            
            // Tiny negative int
            0xF0..=0xFF => Ok(Value::Integer(marker as i8 as i64)),
            
            // Int8/16/32/64
            0xC8 => Ok(Value::Integer(reader.read_i8()? as i64)),
            0xC9 => Ok(Value::Integer(reader.read_i16::<BigEndian>()? as i64)),
            0xCA => Ok(Value::Integer(reader.read_i32::<BigEndian>()? as i64)),
            0xCB => Ok(Value::Integer(reader.read_i64::<BigEndian>()?)),
            
            // Float
            0xC1 => Ok(Value::Float(reader.read_f64::<BigEndian>()?)),
            
            // Tiny string
            0x80..=0x8F => {
                let len = (marker & 0x0F) as usize;
                let mut buf = vec![0u8; len];
                reader.read_exact(&mut buf)?;
                Ok(Value::String(String::from_utf8_lossy(&buf).into_owned()))
            }
            
            // String8/16/32
            0xD0 => {
                let len = reader.read_u8()? as usize;
                let mut buf = vec![0u8; len];
                reader.read_exact(&mut buf)?;
                Ok(Value::String(String::from_utf8_lossy(&buf).into_owned()))
            }
            0xD1 => {
                let len = reader.read_u16::<BigEndian>()? as usize;
                let mut buf = vec![0u8; len];
                reader.read_exact(&mut buf)?;
                Ok(Value::String(String::from_utf8_lossy(&buf).into_owned()))
            }
            0xD2 => {
                let len = reader.read_u32::<BigEndian>()? as usize;
                let mut buf = vec![0u8; len];
                reader.read_exact(&mut buf)?;
                Ok(Value::String(String::from_utf8_lossy(&buf).into_owned()))
            }
            
            // Tiny list
            0x90..=0x9F => {
                let len = (marker & 0x0F) as usize;
                let mut items = Vec::with_capacity(len);
                for _ in 0..len {
                    items.push(Value::unpack(reader)?);
                }
                Ok(Value::List(items))
            }
            
            // List8/16/32
            0xD4 => {
                let len = reader.read_u8()? as usize;
                let mut items = Vec::with_capacity(len);
                for _ in 0..len {
                    items.push(Value::unpack(reader)?);
                }
                Ok(Value::List(items))
            }
            
            // Tiny map
            0xA0..=0xAF => {
                let len = (marker & 0x0F) as usize;
                let mut entries = Vec::with_capacity(len);
                for _ in 0..len {
                    let key = match Value::unpack(reader)? {
                        Value::String(s) => s,
                        _ => return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Map key must be string"
                        )),
                    };
                    let value = Value::unpack(reader)?;
                    entries.push((key, value));
                }
                Ok(Value::Map(entries))
            }
            
            // Tiny structure
            0xB0..=0xBF => {
                let len = (marker & 0x0F) as usize;
                let tag = reader.read_u8()?;
                let mut fields = Vec::with_capacity(len);
                for _ in 0..len {
                    fields.push(Value::unpack(reader)?);
                }
                Ok(Value::Structure { tag, fields })
            }
            
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unknown marker: 0x{:02X}", marker)
            )),
        }
    }
}
```

---

## Bolt Messages

### Message Structure Tags

| Tag | Message | Direction |
|-----|---------|-----------|
| `0x01` | INIT (v1-2) / HELLO (v3+) | Client → Server |
| `0x02` | RUN | Client → Server |
| `0x0F` | RESET | Client → Server |
| `0x10` | BEGIN | Client → Server |
| `0x11` | COMMIT | Client → Server |
| `0x12` | ROLLBACK | Client → Server |
| `0x2F` | DISCARD / DISCARD_ALL | Client → Server |
| `0x3F` | PULL / PULL_ALL | Client → Server |
| `0x66` | ROUTE | Client → Server |
| `0x6A` | LOGON | Client → Server |
| `0x6B` | LOGOFF | Client → Server |
| `0x02` | GOODBYE | Client → Server |
| `0x70` | SUCCESS | Server → Client |
| `0x71` | RECORD | Server → Client |
| `0x7E` | IGNORED | Server → Client |
| `0x7F` | FAILURE | Server → Client |

### Client Messages

#### HELLO (0x01)

Initialize connection and authenticate.

```
HELLO {
    "user_agent": "MyApp/1.0.0",
    "scheme": "basic",
    "principal": "neo4j",
    "credentials": "password",
    "routing": {
        "address": "localhost:7687"
    }
}
```

Fields:
- `user_agent` (required): Client identifier
- `scheme`: Authentication scheme ("basic", "bearer", "none")
- `principal`: Username
- `credentials`: Password or token
- `routing`: Routing context for cluster connections

#### RUN (0x10)

Execute a Cypher query.

```
RUN "MATCH (n:Person) WHERE n.age > $age RETURN n" 
    {"age": 18}
    {"db": "neo4j", "mode": "r"}
```

Fields:
1. Query string
2. Parameters map
3. Extra metadata:
   - `db`: Database name
   - `mode`: Access mode ("r" for read, "w" for write)
   - `tx_timeout`: Transaction timeout in ms
   - `tx_metadata`: Custom metadata

#### PULL (0x3F)

Pull results from a query.

```
PULL {"n": 100}        # Pull 100 records
PULL {"n": -1}         # Pull all records
PULL {"n": 50, "qid": 0}  # Pull from specific query
```

Fields:
- `n`: Number of records to pull (-1 for all)
- `qid`: Query ID (for pipelining)

#### DISCARD (0x2F)

Discard results without fetching.

```
DISCARD {"n": -1}      # Discard all
DISCARD {"n": 50}      # Discard 50 records
```

#### BEGIN (0x11)

Begin an explicit transaction.

```
BEGIN {"db": "neo4j", "mode": "w", "tx_metadata": {"app": "test"}}
```

#### COMMIT (0x12)

Commit the current transaction.

```
COMMIT
```

#### ROLLBACK (0x13)

Rollback the current transaction.

```
ROLLBACK
```

#### RESET (0x0F)

Reset connection to clean state.

```
RESET
```

#### GOODBYE (0x02)

Gracefully close connection.

```
GOODBYE
```

### Server Messages

#### SUCCESS (0x70)

Operation succeeded.

```
SUCCESS {
    "fields": ["name", "age"],
    "t_first": 5,
    "bookmark": "neo4j:bookmark:v1:tx12345"
}
```

Common metadata:
- `fields`: Column names for query results
- `bookmark`: Transaction bookmark
- `t_first`: Time to first record (ms)
- `t_last`: Total execution time (ms)
- `type`: Query type ("r", "w", "rw", "s")
- `db`: Database name
- `has_more`: More results available

#### RECORD (0x71)

A result record.

```
RECORD [value1, value2, value3]
```

#### FAILURE (0x7F)

Operation failed.

```
FAILURE {
    "code": "Neo.ClientError.Statement.SyntaxError",
    "message": "Invalid input 'x': expected..."
}
```

#### IGNORED (0x7E)

Message was ignored (connection in failed state).

```
IGNORED
```

### Message Exchange Examples

#### Simple Query

```
Client: HELLO {"user_agent": "App/1.0", "scheme": "basic", ...}
Server: SUCCESS {"server": "Neo4j/5.0.0", "connection_id": "..."}

Client: RUN "RETURN 1 AS n" {} {}
Server: SUCCESS {"fields": ["n"]}

Client: PULL {"n": -1}
Server: RECORD [1]
Server: SUCCESS {"type": "r"}

Client: GOODBYE
```

#### Explicit Transaction

```
Client: BEGIN {"db": "neo4j"}
Server: SUCCESS {}

Client: RUN "CREATE (n:Person {name: $name})" {"name": "Alice"} {}
Server: SUCCESS {"fields": []}

Client: PULL {"n": -1}
Server: SUCCESS {"type": "w"}

Client: COMMIT
Server: SUCCESS {"bookmark": "..."}
```

---

## Structure Semantics

### Node Structure (0x4E)

```
B3 4E                           # Structure marker, tag 0x4E
  [element_id]                  # String
  [labels]                      # List<String>
  [properties]                  # Map
```

Example:
```rust
Structure {
    tag: 0x4E,  // 'N'
    fields: [
        String("4:abc123:0"),    // element_id
        List([String("Person")]), // labels
        Map([("name", String("Alice")), ("age", Integer(30))]) // properties
    ]
}
```

### Relationship Structure (0x52)

```
B5 52                           # Structure marker, tag 0x52
  [element_id]                  # String
  [start_node_element_id]       # String
  [end_node_element_id]         # String  
  [type]                        # String
  [properties]                  # Map
```

### Path Structure (0x50)

```
B3 50                           # Structure marker, tag 0x50
  [nodes]                       # List<Node>
  [rels]                        # List<UnboundRelationship>
  [indices]                     # List<Integer>
```

### UnboundRelationship Structure (0x72)

```
B4 72                           # Structure marker, tag 0x72
  [element_id]                  # String
  [type]                        # String
  [properties]                  # Map
```

### Temporal Structures

#### Date (0x44)
```
B1 44
  [days]                        # Integer (days since Unix epoch)
```

#### Time (0x54)
```
B2 54
  [nanoseconds]                 # Integer (nanoseconds since midnight)
  [tz_offset_seconds]           # Integer
```

#### LocalTime (0x74)
```
B1 74
  [nanoseconds]                 # Integer
```

#### DateTime (0x49) - Bolt 5.0+
```
B3 49
  [seconds]                     # Integer (seconds since Unix epoch)
  [nanoseconds]                 # Integer
  [tz_offset_seconds]           # Integer
```

#### DateTimeZoneId (0x69) - Bolt 5.0+
```
B3 69
  [seconds]                     # Integer
  [nanoseconds]                 # Integer
  [tz_id]                       # String (e.g., "Europe/Paris")
```

#### LocalDateTime (0x64)
```
B2 64
  [seconds]                     # Integer
  [nanoseconds]                 # Integer
```

#### Duration (0x45)
```
B4 45
  [months]                      # Integer
  [days]                        # Integer
  [seconds]                     # Integer
  [nanoseconds]                 # Integer
```

### Spatial Structures

#### Point2D (0x58)
```
B3 58
  [srid]                        # Integer
  [x]                           # Float
  [y]                           # Float
```

#### Point3D (0x59)
```
B4 59
  [srid]                        # Integer
  [x]                           # Float
  [y]                           # Float
  [z]                           # Float
```

SRID Values:
- `4326`: WGS-84 (latitude/longitude)
- `4979`: WGS-84-3D
- `7203`: Cartesian 2D
- `9157`: Cartesian 3D

---

## Rust Implementation Guide

### Project Structure

```
neo4j-rust-driver/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── packstream/
│   │   ├── mod.rs
│   │   ├── value.rs
│   │   ├── encoder.rs
│   │   └── decoder.rs
│   ├── bolt/
│   │   ├── mod.rs
│   │   ├── connection.rs
│   │   ├── message.rs
│   │   ├── handshake.rs
│   │   └── chunking.rs
│   ├── cypher/
│   │   ├── mod.rs
│   │   ├── lexer.rs
│   │   ├── parser.rs
│   │   ├── ast.rs
│   │   └── visitor.rs
│   ├── types/
│   │   ├── mod.rs
│   │   ├── node.rs
│   │   ├── relationship.rs
│   │   ├── path.rs
│   │   ├── temporal.rs
│   │   └── spatial.rs
│   └── driver/
│       ├── mod.rs
│       ├── session.rs
│       ├── transaction.rs
│       └── result.rs
└── tests/
```

### Cargo.toml Dependencies

```toml
[package]
name = "neo4j-driver"
version = "0.1.0"
edition = "2021"

[dependencies]
# Core
tokio = { version = "1.0", features = ["full"] }
bytes = "1.0"
byteorder = "1.5"

# Networking
tokio-rustls = "0.26"
webpki-roots = "0.26"

# Parsing (choose one)
nom = "7.0"           # Parser combinators
pest = "2.0"          # PEG parser
pest_derive = "2.0"

# Serialization
serde = { version = "1.0", features = ["derive"] }

# Utilities
thiserror = "1.0"
tracing = "0.1"
uuid = { version = "1.0", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }

[dev-dependencies]
tokio-test = "0.4"
```

### Core Types

```rust
// src/types/mod.rs
use std::collections::HashMap;
use chrono::{NaiveDate, NaiveTime, NaiveDateTime, DateTime, FixedOffset};

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub element_id: String,
    pub labels: Vec<String>,
    pub properties: HashMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Relationship {
    pub element_id: String,
    pub start_node_element_id: String,
    pub end_node_element_id: String,
    pub rel_type: String,
    pub properties: HashMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub nodes: Vec<Node>,
    pub relationships: Vec<Relationship>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Point2D {
    pub srid: i32,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Point3D {
    pub srid: i32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Duration {
    pub months: i64,
    pub days: i64,
    pub seconds: i64,
    pub nanoseconds: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Map(HashMap<String, Value>),
    Node(Node),
    Relationship(Relationship),
    Path(Path),
    Point2D(Point2D),
    Point3D(Point3D),
    Date(NaiveDate),
    Time(NaiveTime, i32), // time + offset
    LocalTime(NaiveTime),
    DateTime(DateTime<FixedOffset>),
    LocalDateTime(NaiveDateTime),
    Duration(Duration),
}
```

### Bolt Connection

```rust
// src/bolt/connection.rs
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use crate::packstream::Value;
use crate::bolt::message::Message;

pub struct BoltConnection {
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: BufWriter<tokio::net::tcp::OwnedWriteHalf>,
    version: (u8, u8),
}

impl BoltConnection {
    pub async fn connect(addr: &str) -> Result<Self, BoltError> {
        let stream = TcpStream::connect(addr).await?;
        let (read_half, write_half) = stream.into_split();
        
        let mut conn = BoltConnection {
            reader: BufReader::new(read_half),
            writer: BufWriter::new(write_half),
            version: (0, 0),
        };
        
        conn.handshake().await?;
        Ok(conn)
    }
    
    async fn handshake(&mut self) -> Result<(), BoltError> {
        // Send magic bytes
        self.writer.write_all(&[0x60, 0x60, 0xB0, 0x17]).await?;
        
        // Send version proposals (5.4, 5.3, 5.2, 5.1)
        let versions: [u8; 16] = [
            0x00, 0x03, 0x05, 0x04,  // 5.4 with range 3
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];
        self.writer.write_all(&versions).await?;
        self.writer.flush().await?;
        
        // Read selected version
        let mut version_buf = [0u8; 4];
        self.reader.read_exact(&mut version_buf).await?;
        
        self.version = (version_buf[3], version_buf[2]);
        
        if self.version == (0, 0) {
            return Err(BoltError::HandshakeFailed);
        }
        
        Ok(())
    }
    
    pub async fn send(&mut self, message: Message) -> Result<(), BoltError> {
        let data = message.serialize()?;
        
        // Chunk the message
        for chunk in data.chunks(65535) {
            let len = chunk.len() as u16;
            self.writer.write_u16(len).await?;
            self.writer.write_all(chunk).await?;
        }
        
        // End marker
        self.writer.write_u16(0).await?;
        self.writer.flush().await?;
        
        Ok(())
    }
    
    pub async fn receive(&mut self) -> Result<Message, BoltError> {
        let mut data = Vec::new();
        
        loop {
            let mut len_buf = [0u8; 2];
            self.reader.read_exact(&mut len_buf).await?;
            let len = u16::from_be_bytes(len_buf) as usize;
            
            if len == 0 {
                break;
            }
            
            let mut chunk = vec![0u8; len];
            self.reader.read_exact(&mut chunk).await?;
            data.extend(chunk);
        }
        
        Message::deserialize(&data)
    }
}
```

### Cypher Parser (with nom)

```rust
// src/cypher/parser.rs
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while1},
    character::complete::{alphanumeric1, char, multispace0, multispace1},
    combinator::{map, opt, recognize},
    multi::{many0, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, terminated, tuple},
};

use crate::cypher::ast::*;

pub fn parse_query(input: &str) -> IResult<&str, Query> {
    let (input, _) = multispace0(input)?;
    let (input, clauses) = many1(parse_clause)(input)?;
    Ok((input, Query { clauses }))
}

fn parse_clause(input: &str) -> IResult<&str, Clause> {
    let (input, _) = multispace0(input)?;
    alt((
        map(parse_match, Clause::Match),
        map(parse_return, Clause::Return),
        map(parse_create, Clause::Create),
        map(parse_with, Clause::With),
        // ... other clauses
    ))(input)
}

fn parse_match(input: &str) -> IResult<&str, MatchClause> {
    let (input, optional) = opt(preceded(
        tag_no_case("OPTIONAL"),
        multispace1
    ))(input)?;
    let (input, _) = tag_no_case("MATCH")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, pattern) = parse_pattern(input)?;
    let (input, where_clause) = opt(preceded(
        tuple((multispace1, tag_no_case("WHERE"), multispace1)),
        parse_expression
    ))(input)?;
    
    Ok((input, MatchClause {
        optional: optional.is_some(),
        pattern,
        where_clause,
    }))
}

fn parse_pattern(input: &str) -> IResult<&str, Pattern> {
    let (input, parts) = separated_list1(
        tuple((multispace0, char(','), multispace0)),
        parse_pattern_part
    )(input)?;
    
    Ok((input, Pattern { parts }))
}

fn parse_pattern_part(input: &str) -> IResult<&str, PatternPart> {
    let (input, variable) = opt(terminated(
        parse_variable,
        tuple((multispace0, char('='), multispace0))
    ))(input)?;
    let (input, element) = parse_pattern_element(input)?;
    
    Ok((input, PatternPart { variable, element }))
}

fn parse_node_pattern(input: &str) -> IResult<&str, NodePattern> {
    let (input, _) = char('(')(input)?;
    let (input, _) = multispace0(input)?;
    let (input, variable) = opt(parse_variable)(input)?;
    let (input, labels) = many0(preceded(char(':'), parse_identifier))(input)?;
    let (input, properties) = opt(preceded(
        multispace0,
        parse_map_literal
    ))(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char(')')(input)?;
    
    Ok((input, NodePattern {
        variable,
        labels,
        properties,
    }))
}

fn parse_relationship_pattern(input: &str) -> IResult<&str, RelationshipPattern> {
    let (input, left_arrow) = opt(tag("<-"))(input)?;
    let (input, _) = opt(char('-'))(input)?;
    let (input, details) = opt(delimited(
        char('['),
        parse_relationship_detail,
        char(']')
    ))(input)?;
    let (input, _) = opt(char('-'))(input)?;
    let (input, right_arrow) = opt(tag("->"))(input)?;
    
    let direction = match (left_arrow.is_some(), right_arrow.is_some()) {
        (true, false) => Direction::Incoming,
        (false, true) => Direction::Outgoing,
        _ => Direction::Both,
    };
    
    Ok((input, RelationshipPattern {
        direction,
        variable: details.as_ref().and_then(|d| d.variable.clone()),
        types: details.as_ref().map(|d| d.types.clone()).unwrap_or_default(),
        range: details.as_ref().and_then(|d| d.range.clone()),
        properties: details.and_then(|d| d.properties),
    }))
}

fn parse_expression(input: &str) -> IResult<&str, Expression> {
    parse_or_expression(input)
}

fn parse_or_expression(input: &str) -> IResult<&str, Expression> {
    let (input, first) = parse_and_expression(input)?;
    let (input, rest) = many0(preceded(
        tuple((multispace1, tag_no_case("OR"), multispace1)),
        parse_and_expression
    ))(input)?;
    
    Ok((input, rest.into_iter().fold(first, |acc, expr| {
        Expression::BinaryOp(BinaryOp::Or, Box::new(acc), Box::new(expr))
    })))
}

// Continue with other expression parsers...

fn parse_variable(input: &str) -> IResult<&str, Variable> {
    let (input, name) = parse_identifier(input)?;
    Ok((input, Variable { name: name.to_string() }))
}

fn parse_identifier(input: &str) -> IResult<&str, &str> {
    alt((
        // Escaped identifier
        delimited(char('`'), take_while1(|c| c != '`'), char('`')),
        // Regular identifier
        recognize(pair(
            alt((alphanumeric1, tag("_"))),
            many0(alt((alphanumeric1, tag("_"))))
        ))
    ))(input)
}
```

### Driver API

```rust
// src/driver/mod.rs
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Driver {
    uri: String,
    auth: Auth,
    config: Config,
}

impl Driver {
    pub fn new(uri: &str, auth: Auth, config: Config) -> Self {
        Driver {
            uri: uri.to_string(),
            auth,
            config,
        }
    }
    
    pub async fn session(&self, config: SessionConfig) -> Result<Session, DriverError> {
        let connection = BoltConnection::connect(&self.uri).await?;
        
        // Send HELLO
        connection.send(Message::Hello {
            user_agent: "neo4j-rust/0.1.0".to_string(),
            auth: self.auth.clone(),
            routing: config.routing_context.clone(),
        }).await?;
        
        let response = connection.receive().await?;
        match response {
            Message::Success { metadata } => {
                Ok(Session {
                    connection: Arc::new(Mutex::new(connection)),
                    database: config.database,
                    bookmarks: config.bookmarks,
                })
            }
            Message::Failure { code, message } => {
                Err(DriverError::AuthError { code, message })
            }
            _ => Err(DriverError::UnexpectedResponse),
        }
    }
}

pub struct Session {
    connection: Arc<Mutex<BoltConnection>>,
    database: Option<String>,
    bookmarks: Vec<String>,
}

impl Session {
    pub async fn run(
        &self,
        query: &str,
        params: HashMap<String, Value>,
    ) -> Result<QueryResult, DriverError> {
        let mut conn = self.connection.lock().await;
        
        // Send RUN
        conn.send(Message::Run {
            query: query.to_string(),
            parameters: params,
            extra: RunExtra {
                database: self.database.clone(),
                mode: None,
                bookmarks: Some(self.bookmarks.clone()),
                tx_timeout: None,
                tx_metadata: None,
            },
        }).await?;
        
        let run_response = conn.receive().await?;
        let fields = match run_response {
            Message::Success { metadata } => {
                metadata.get("fields")
                    .and_then(|v| v.as_list())
                    .map(|l| l.iter()
                        .filter_map(|v| v.as_string().cloned())
                        .collect())
                    .unwrap_or_default()
            }
            Message::Failure { code, message } => {
                return Err(DriverError::QueryError { code, message });
            }
            _ => return Err(DriverError::UnexpectedResponse),
        };
        
        // Send PULL
        conn.send(Message::Pull { n: -1, qid: None }).await?;
        
        let mut records = Vec::new();
        loop {
            match conn.receive().await? {
                Message::Record { values } => {
                    records.push(Record {
                        fields: fields.clone(),
                        values,
                    });
                }
                Message::Success { metadata } => {
                    let bookmark = metadata.get("bookmark")
                        .and_then(|v| v.as_string().cloned());
                    return Ok(QueryResult {
                        records,
                        summary: ResultSummary::from_metadata(metadata),
                    });
                }
                Message::Failure { code, message } => {
                    return Err(DriverError::QueryError { code, message });
                }
                _ => return Err(DriverError::UnexpectedResponse),
            }
        }
    }
    
    pub async fn begin_transaction(&self) -> Result<Transaction, DriverError> {
        let mut conn = self.connection.lock().await;
        
        conn.send(Message::Begin {
            database: self.database.clone(),
            mode: None,
            bookmarks: Some(self.bookmarks.clone()),
            tx_timeout: None,
            tx_metadata: None,
        }).await?;
        
        match conn.receive().await? {
            Message::Success { .. } => {
                Ok(Transaction {
                    connection: self.connection.clone(),
                    database: self.database.clone(),
                })
            }
            Message::Failure { code, message } => {
                Err(DriverError::TransactionError { code, message })
            }
            _ => Err(DriverError::UnexpectedResponse),
        }
    }
}

pub struct Transaction {
    connection: Arc<Mutex<BoltConnection>>,
    database: Option<String>,
}

impl Transaction {
    pub async fn run(
        &self,
        query: &str,
        params: HashMap<String, Value>,
    ) -> Result<QueryResult, DriverError> {
        // Similar to Session::run but within transaction context
        unimplemented!()
    }
    
    pub async fn commit(self) -> Result<(), DriverError> {
        let mut conn = self.connection.lock().await;
        conn.send(Message::Commit).await?;
        
        match conn.receive().await? {
            Message::Success { .. } => Ok(()),
            Message::Failure { code, message } => {
                Err(DriverError::TransactionError { code, message })
            }
            _ => Err(DriverError::UnexpectedResponse),
        }
    }
    
    pub async fn rollback(self) -> Result<(), DriverError> {
        let mut conn = self.connection.lock().await;
        conn.send(Message::Rollback).await?;
        
        match conn.receive().await? {
            Message::Success { .. } => Ok(()),
            Message::Failure { code, message } => {
                Err(DriverError::TransactionError { code, message })
            }
            _ => Err(DriverError::UnexpectedResponse),
        }
    }
}
```

### Usage Example

```rust
use neo4j_driver::{Driver, Auth, Config, SessionConfig};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create driver
    let driver = Driver::new(
        "bolt://localhost:7687",
        Auth::basic("neo4j", "password"),
        Config::default(),
    );
    
    // Create session
    let session = driver.session(SessionConfig {
        database: Some("neo4j".to_string()),
        ..Default::default()
    }).await?;
    
    // Run query
    let result = session.run(
        "MATCH (p:Person) WHERE p.age > $age RETURN p.name, p.age",
        HashMap::from([
            ("age".to_string(), Value::Integer(18)),
        ]),
    ).await?;
    
    // Process results
    for record in result.records {
        println!("Name: {:?}, Age: {:?}",
            record.get("p.name"),
            record.get("p.age")
        );
    }
    
    // Transaction example
    let tx = session.begin_transaction().await?;
    
    tx.run(
        "CREATE (p:Person {name: $name})",
        HashMap::from([
            ("name".to_string(), Value::String("Alice".to_string())),
        ]),
    ).await?;
    
    tx.commit().await?;
    
    Ok(())
}
```

---

## References

### Official Documentation
- [Neo4j Cypher Manual](https://neo4j.com/docs/cypher-manual/current/)
- [Bolt Protocol Documentation](https://neo4j.com/docs/bolt/current/)
- [openCypher Resources](https://opencypher.org/resources/)

### Specifications
- [openCypher Grammar (EBNF)](https://github.com/opencypher/openCypher)
- [PackStream Specification](https://neo4j.com/docs/bolt/current/packstream/)
- [Bolt Message Specification](https://neo4j.com/docs/bolt/current/bolt/message/)

### Community Resources
- [Neo4j Community Forum](https://community.neo4j.com/)
- [Neo4j Discord](https://discord.gg/neo4j)
- [GraphAcademy Courses](https://graphacademy.neo4j.com/)

---

*Document generated for use with LLM coding tools. Last updated: 2024*
