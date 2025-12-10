# ArangoDB AQL Compatibility Specification

**Target**: ArangoDB AQL (ArangoDB Query Language) 3.10+
**Reference**: https://www.arangodb.com/docs/stable/aql/
**Last Updated**: 2025-12-10
**Current Estimated Coverage**: ~65%

---

## Overview

This document specifies OrbitRS's compatibility with ArangoDB's AQL (ArangoDB Query Language). AQL is a declarative query language for multi-model databases, supporting document, graph, and key-value data models. The goal is to support the full AQL feature set to enable OrbitRS to serve as a drop-in replacement for ArangoDB in most use cases.

## Table of Contents

1. [High-Level Operations](#high-level-operations)
2. [Data Types](#data-types)
3. [Operators](#operators)
4. [Functions](#functions)
5. [Graph Operations](#graph-operations)
6. [System Features](#system-features)
7. [Client Compatibility](#client-compatibility)

---

## High-Level Operations

AQL uses high-level operations (statements) to manipulate data. Note that AQL does not strictly distinguish between DDL and DML in the same way SQL does; many operations can be mixed.

### Legend
- ✅ **Implemented** - Fully functional
- 🔶 **Partial** - Basic support, missing features
- ❌ **Not Implemented** - Not yet available

### Query Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| FOR | ✅ | Iterate over collections, arrays, or ranges (`1..10`) |
| RETURN | ✅ | Project results (`RETURN doc`, `RETURN { a: doc.a }`) |
| FILTER | ✅ | Filter results (`FILTER doc.age > 10`) |
| SORT | ✅ | Sort results (`SORT doc.name ASC, doc.age DESC`) |
| LIMIT | ✅ | Slice results (`LIMIT 10`, `LIMIT 5, 10`) |
| LET | ✅ | Assign variables (`LET a = 1`) |
| COLLECT | 🔶 | Group results (`COLLECT city = doc.city WITH COUNT INTO length`). Missing `AGGREGATE` syntax. |
| WINDOW | ❌ | Window operations (aggregation over sliding windows) |
| WITH | ❌ | Collection hints for locking/loading |
| DISTINCT | ✅ | Unique results (`FOR doc IN ... RETURN DISTINCT doc`) |

### Data Modification Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| INSERT | ✅ | Insert documents (`INSERT { ... } INTO collection`) |
| UPDATE | ✅ | Update documents (`UPDATE key WITH { ... } IN collection`) |
| REPLACE | ✅ | Replace documents (`REPLACE key WITH { ... } IN collection`) |
| REMOVE | ✅ | Remove documents (`REMOVE key IN collection`) |
| UPSERT | ✅ | Insert or update (`UPSERT search INSERT insert UPDATE update IN collection`) |

### Options and Hints

| Feature | Status | Notes |
|---------|--------|-------|
| OPTIONS { ... } | ❌ | General options clause for operations |
| indexHint | ❌ | Index selection hint |
| forceIndexHint | ❌ | Force index usage |
| maxIterations | ❌ | Limit traversal iterations |

---

## Data Types

### Primitive Types

| Type | Status | Notes |
|------|--------|-------|
| null | ✅ | Null value |
| boolean | ✅ | `true` / `false` |
| number | ✅ | 64-bit IEEE 754 double precision |
| string | ✅ | UTF-8 encoded strings |

### Compound Types

| Type | Status | Notes |
|------|--------|-------|
| array | ✅ | Ordered list of values (`[1, 2, 3]`) |
| object | ✅ | Unordered key-value pairs (`{ "a": 1 }`) |

### System Attributes

ArangoDB documents have special system attributes starting with `_`.

| Attribute | Status | Notes |
|-----------|--------|-------|
| _key | ✅ | Primary key (string), unique within collection |
| _id | ✅ | Document ID (`collection/_key`), unique within database |
| _rev | ✅ | Revision ID (string) |
| _from | ✅ | Source document ID (edges only) |
| _to | ✅ | Target document ID (edges only) |

---

## Operators

### Arithmetic Operators

| Operator | Status | Description |
|----------|--------|-------------|
| + | ✅ | Addition |
| - | ✅ | Subtraction |
| * | ✅ | Multiplication |
| / | ✅ | Division |
| % | ✅ | Modulo |

### Comparison Operators

| Operator | Status | Description |
|----------|--------|-------------|
| == | ✅ | Equal |
| != | ✅ | Not equal |
| < | ✅ | Less than |
| <= | ✅ | Less than or equal |
| > | ✅ | Greater than |
| >= | ✅ | Greater than or equal |
| IN | ✅ | Test if value is in array |
| NOT IN | ✅ | Test if value is not in array |
| LIKE | ✅ | String pattern matching (`%`, `_`) |
| =~ | ✅ | Regex match |
| !~ | ✅ | Negated regex match |

### Logical Operators

| Operator | Status | Description |
|----------|--------|-------------|
| AND / && | ✅ | Logical AND |
| OR / \|\| | ✅ | Logical OR |
| NOT / ! | ✅ | Logical NOT |

### Range Operators

| Operator | Status | Description |
|----------|--------|-------------|
| .. | ✅ | Configure range (`0..10`) |

### Array Operators

| Operator | Status | Description |
|----------|--------|-------------|
| [*] | ✅ | Array expansion (all elements) |
| [**] | ❌ | Array expansion (recursive/flatten) |
| [? filter] | ✅ | Inline array filter |
| [limit] | ✅ | Inline array slicing/access |

---

## Functions

### String Functions

| Function | Status | Notes |
|----------|--------|-------|
| CHAR_LENGTH(str) | ✅ | Length in characters |
| CONCAT(str1, str2, ...) | ✅ | Concatenate strings |
| CONCAT_SEPARATOR(sep, str1, ...) | ✅ | Concatenate with separator |
| CONTAINS(text, search) | ✅ | Check if search is in text |
| FIND_FIRST(text, search) | ✅ | Index of first occurrence |
| FIND_LAST(text, search) | ✅ | Index of last occurrence |
| JSON_PARSE(str) | ❌ | Parse JSON string |
| JSON_STRINGIFY(val) | ❌ | Serialize to JSON string |
| LEFT(str, n) | ✅ | Left n characters |
| LENGTH(str) | ✅ | Byte length (alias for implementation) |
| LIKE(text, pattern) | ✅ | Pattern matching |
| LOWER(str) | ✅ | Convert to lowercase |
| LTRIM(str) | ✅ | Trim left whitespace |
| MD5(text) | ✅ | Calculate MD5 hash |
| RANDOM_TOKEN(length) | ❌ | Generate random token |
| REGEX_MATCHES(text, regex) | ✅ | Return matches |
| REGEX_REPLACE(text, regex, rep) | ✅ | Replace matches |
| REGEX_SPLIT(text, regex) | ✅ | Split by regex |
| REGEX_TEST(text, regex) | ✅ | Test regex match |
| REVERSE(str) | ✅ | Reverse string |
| RIGHT(str, n) | ✅ | Right n characters |
| RTRIM(str) | ✅ | Trim right whitespace |
| SHA1(text) | ✅ | SHA1 hash |
| SHA256(text) | ✅ | SHA256 hash |
| SHA512(text) | ✅ | SHA512 hash |
| SPLIT(text, separator) | ✅ | Split string |
| SUBSTITUTE(text, search, replace) | ✅ | Replace substring |
| SUBSTRING(text, offset, length) | ✅ | Extract substring |
| TO_BASE64(text) | ❌ | Encode base64 |
| TO_HEX(val) | ❌ | Convert to hex |
| TRIM(str) | ✅ | Trim whitespace |
| UPPER(str) | ✅ | Convert to uppercase |
| UUID() | ✅ | Generate UUID |

### Numeric Functions

| Function | Status | Notes |
|----------|--------|-------|
| ABS(num) | ✅ | Absolute value |
| ACOS(num) | ✅ | Arc cosine |
| ASIN(num) | ✅ | Arc sine |
| ATAN(num) | ✅ | Arc tangent |
| ATAN2(y, x) | ✅ | Arc tangent 2 |
| AVERAGE(arr) | ✅ | Average of array values |
| CEIL(num) | ✅ | Ceiling |
| COS(num) | ✅ | Cosine |
| DEGREES(rad) | ✅ | Radians to degrees |
| EXP(num) | ✅ | Exponential `e^num` |
| EXP2(num) | ✅ | `2^num` |
| FLOOR(num) | ✅ | Floor |
| LOG(num) | ✅ | Natural logarithm |
| LOG2(num) | ✅ | Base-2 logarithm |
| LOG10(num) | ✅ | Base-10 logarithm |
| MAX(arr) | ✅ | Maximum value |
| MEDIAN(arr) | ✅ | Median value |
| MIN(arr) | ✅ | Minimum value |
| PERCENTILE(arr, p) | ✅ | p-th percentile |
| PI() | ✅ | Pi constant |
| POW(base, exp) | ✅ | Power |
| RADIANS(deg) | ✅ | Degrees to radians |
| RAND() | ✅ | Random number [0, 1) |
| RANGE(start, end, step) | ✅ | Generate range array |
| ROUND(num) | ✅ | Round to nearest integer |
| SIN(num) | ✅ | Sine |
| SQRT(num) | ✅ | Square root |
| STDDEV_POPULATION(arr) | ✅ | Population standard deviation |
| STDDEV_SAMPLE(arr) | ✅ | Sample standard deviation |
| SUM(arr) | ✅ | Sum values |
| TAN(num) | ✅ | Tangent |
| VARIANCE_POPULATION(arr) | ✅ | Population variance |
| VARIANCE_SAMPLE(arr) | ✅ | Sample variance |

### Date Functions

| Function | Status | Notes |
|----------|--------|-------|
| DATE_ADD(date, amount, unit) | ✅ | Add to date |
| DATE_COMPARE(date1, date2) | ✅ | Compare dates (-1, 0, 1) |
| DATE_DAY(date) | ✅ | Day of month |
| DATE_DAYOFWEEK(date) | ✅ | Day of week (0-6) |
| DATE_DAYOFYEAR(date) | ✅ | Day of year |
| DATE_DAYS_IN_MONTH(date) | ✅ | Days in month |
| DATE_DIFF(date1, date2, unit) | ✅ | Difference between dates |
| DATE_FORMAT(date, format) | ✅ | Format date string |
| DATE_HOUR(date) | ✅ | Hour part |
| DATE_ISO8601(date) | ✅ | Convert to ISO8601 string |
| DATE_ISOWEEK(date) | ✅ | ISO week number |
| DATE_LEAPYEAR(date) | ✅ | Assert leap year |
| DATE_MILLISECOND(date) | ✅ | Millisecond part |
| DATE_MINUTE(date) | ✅ | Minute part |
| DATE_MONTH(date) | ✅ | Month part |
| DATE_NOW() | ✅ | Current Unix timestamp (ms) |
| DATE_QUARTER(date) | ✅ | Quarter (1-4) |
| DATE_ROUND(date, amount, unit) | ✅ | Round date |
| DATE_SECOND(date) | ✅ | Second part |
| DATE_SUBTRACT(date, amount, unit) | ✅ | Subtract from date |
| DATE_TIMESTAMP(date) | ✅ | Convert to timestamp |
| DATE_TRUNC(date, unit) | ✅ | Truncate date |
| DATE_YEAR(date) | ✅ | Year part |
| IS_DATESTRING(str) | ✅ | Check format |

### Array Functions

| Function | Status | Notes |
|----------|--------|-------|
| APPEND(arr, vals) | ✅ | Append elements |
| COUNT(arr) | ✅ | Count elements |
| FIRST(arr) | ✅ | First element |
| FLATTEN(arr, depth) | ✅ | Flatten nested arrays |
| INTERSECTION(arr1, arr2) | ✅ | Intersection of arrays |
| INTERLEAVE(arr1, arr2) | ❌ | Interleave arrays |
| LAST(arr) | ✅ | Last element |
| LENGTH(arr) | ✅ | Count elements (alias) |
| MINUS(arr1, arr2) | ✅ | Subtract arr2 from arr1 |
| NTH(arr, n) | ✅ | Get n-th element |
| OUTERSECTION(arr1, arr2) | ✅ | Values in one but not both |
| POP(arr) | ✅ | Remove last element |
| POSITION(arr, val) | ✅ | Find index of value |
| PUSH(arr, val) | ✅ | Append value |
| REMOVE_NTH(arr, n) | ✅ | Remove element at n |
| REMOVE_VALUE(arr, val) | ✅ | Remove first occurrence |
| REMOVE_VALUES(arr, vals) | ✅ | Remove all occurrences |
| REVERSE(arr) | ✅ | Reverse array |
| SHIFT(arr) | ✅ | Remove first element |
| SLICE(arr, start, len) | ✅ | Extract sub-array |
| SORTED_UNIQUE(arr) | ✅ | Dedup and sort |
| UNION(arr1, arr2) | ✅ | Join arrays |
| UNION_DISTINCT(arr1, arr2) | ✅ | Join and dedup |
| UNIQUE(arr) | ✅ | Return unique values |
| UNSHIFT(arr, val) | ✅ | Prepend value |

### Object / Document Functions

| Function | Status | Notes |
|----------|--------|-------|
| ATTRIBUTES(obj) | ✅ | Get keys |
| HAS(obj, key) | ✅ | Check key existence |
| IS_SAME_COLLECTION(col, id) | ✅ | Check ID belongs to collection |
| KEEP(obj, keys) | ✅ | Keep only specific keys |
| KEYS(obj) | ✅ | Alias for ATTRIBUTES |
| MATCHES(obj, example) | ✅ | Check if matches example |
| MERGE(obj1, obj2) | ✅ | Merge objects (shallow) |
| MERGE_RECURSIVE(obj1, obj2) | ✅ | Recursive merge |
| PARSE_IDENTIFIER(id) | ✅ | Parse ID into keys/collection |
| TRANSLATE(val, map, default) | ✅ | Map value |
| UNSET(obj, keys) | ✅ | Remove keys |
| UNSET_RECURSIVE(obj, keys) | ✅ | Remove keys recursively |
| VALUES(obj) | ✅ | Get values |
| ZIP(keys, values) | ✅ | Create object from arrays |

### Type Check & Cast Functions

| Function | Status | Notes |
|----------|--------|-------|
| IS_ARRAY(val) | ✅ | Check if array |
| IS_BOOL(val) | ✅ | Check if boolean |
| IS_DATESTRING(val) | ✅ | Check if date string |
| IS_DOCUMENT(val) | ✅ | Check if document |
| IS_KEY(val) | ✅ | Check if valid key |
| IS_LIST(val) | ✅ | Alias for IS_ARRAY |
| IS_NULL(val) | ✅ | Check if null |
| IS_NUMBER(val) | ✅ | Check if number |
| IS_OBJECT(val) | ✅ | Check if object |
| IS_STRING(val) | ✅ | Check if string |
| TO_ARRAY(val) | ✅ | Cast to array |
| TO_BOOL(val) | ✅ | Cast to boolean |
| TO_LIST(val) | ✅ | Alias for TO_ARRAY |
| TO_NUMBER(val) | ✅ | Cast to number |
| TO_STRING(val) | ✅ | Cast to string |
| TYPENAME(val) | ✅ | Get type name |

### Geo Functions

| Function | Status | Notes |
|----------|--------|-------|
| DISTANCE(lat1, lon1, lat2, lon2) | ✅ | Haversine distance in meters |
| GEO_AREA(geo) | ✅ | Area of polygon (sq meters) |
| GEO_CONTAINS(geo1, geo2) | ✅ | Check if polygon contains point |
| GEO_DISTANCE(geo1, geo2) | ✅ | Distance between GeoJSON objects |
| GEO_EQUALS(geo1, geo2) | ✅ | Check equality |
| GEO_INTERSECTS(geo1, geo2) | ✅ | Check intersection |
| GEO_LINESTRING(points) | ✅ | Create GeoJSON LineString |
| GEO_MULTIPOINT(points) | ✅ | Create GeoJSON MultiPoint |
| GEO_POINT(lon, lat) | ✅ | Create GeoJSON Point |
| GEO_POLYGON(points) | ✅ | Create GeoJSON Polygon |
| IS_IN_POLYGON(poly, lat, lon) | ✅ | Point in polygon check |

### Fulltext Functions

| Function | Status | Notes |
|----------|--------|-------|
| ANALYZER(expr, analyzer) | ✅ | Set analyzer for expression |
| BM25(doc) | ✅ | Get BM25 relevance score |
| BOOST(expr, factor) | ✅ | Boost relevance of expression |
| FULLTEXT(coll, attr, query) | 🔶 | Fulltext search (stub - returns empty) |
| PHRASE(tokens, text, analyzer) | ✅ | Build phrase for search |
| TFIDF(doc) | ✅ | Get TFIDF relevance score |
| TOKENS(input, analyzer) | ✅ | Tokenize text |

### Miscellaneous Functions

| Function | Status | Notes |
|----------|--------|-------|
| APPLY(func, args) | ✅ | Dynamically apply function |
| ASSERT(cond, msg) | ✅ | Throw error if false |
| CALL(func, args) | 🔶 | Call user-defined function (stub) |
| COLLECTIONS() | ✅ | List collections |
| CURRENT_DATABASE() | ✅ | Get database name |
| CURRENT_USER() | ✅ | Get current user |
| DECODE_REV(rev) | ✅ | Decode revision string |
| DOCUMENT(id) | ✅ | Retrieve document by ID |
| FAIL(msg) | ✅ | Throw error |
| HASH(val) | ✅ | Calculate hash |
| NOT_NULL(args...) | ✅ | First non-null arg |
| PASSTHRU(val) | ✅ | No-op |
| SLEEP(seconds) | ✅ | Sleep execution |
| V8(script) | 🔶 | Execute V8 JavaScript (stub) |
| VERSION() | ✅ | Server version |
| WARN(msg) | ✅ | Emit warning |

---

## Graph Operations

OrbitRS supports basic graph traversals but lacks advanced graph algorithms.

### Traversal

Format: `FOR v, e, p IN [min..max] OUTBOUND|INBOUND|ANY start_vertex GRAPH graph_name|edge_collections`

| Component | Status | Notes |
|-----------|--------|-------|
| OUTBOUND | 🔶 | Follow outgoing edges |
| INBOUND | 🔶 | Follow incoming edges |
| ANY | 🔶 | Follow edges in any direction |
| min..max | 🔶 | Depth constraints (1..1 supported) |
| GRAPH name | 🔶 | Named graph support |
| EDGE collections | ✅ | Direct edge collection usage |
| Options | ❌ | `uniqueVertices`, `bsf` (Breadth-First), etc. |
| PRUNE | ❌ | Prune condition |

### Path Finding

| Feature | Status | Notes |
|---------|--------|-------|
| SHORTEST_PATH | 🔶 | Find shortest path (stub - returns empty path) |
| K_SHORTEST_PATHS | 🔶 | Find top K shortest paths (stub) |
| K_PATHS | 🔶 | Find all paths (stub) |
| ALL_SHORTEST_PATHS | 🔶 | Find all shortest paths (stub) |

### Graph Functions

| Function | Status | Notes |
|----------|--------|-------|
| GRAPH_COMMON_NEIGHBORS | 🔶 | Get common neighbors (stub) |
| GRAPH_COMMON_PROPERTIES | 🔶 | Get common properties (stub) |
| GRAPH_DISTANCE_TO | 🔶 | Get distance between vertices (stub) |
| GRAPH_EDGES | 🔶 | Get all edges (stub) |
| GRAPH_NEIGHBORS | 🔶 | Get neighbors (stub) |
| GRAPH_PATHS | 🔶 | Get all paths (stub) |
| GRAPH_SHORTEST_PATH | 🔶 | Named graph shortest path (stub) |
| GRAPH_VERTICES | 🔶 | Get all vertices (stub) |
| PREGEL_RESULT | 🔶 | Get Pregel algorithm result (stub) |

---

## System Features

### Transactions

| Feature | Status | Notes |
|---------|--------|-------|
| Stream Transactions | 🔶 | Basic support via driver |
| JS Transactions | ❌ | `db._executeTransaction` (server-side JS) |

### Explanation & Profiling

| Feature | Status | Notes |
|---------|--------|-------|
| EXPLAIN | 🔶 | Basic query plan visualization |
| PROFILE | ❌ | Execution profiling |

---

## Client Compatibility

OrbitRS aims to support standard ArangoDB drivers.

| Driver | Status | Notes |
|--------|--------|-------|
| **ArangoJS** (Node.js) | 🔶 | Basic queries work; authentication handshake supported. |
| **Python-arango** | 🔶 | Basic queries work. |
| **ArangoDB-Java-Driver** | 🔶 | Basic support. |
| **ArangoDB-Go-Driver** | 🔶 | Basic support. |

**Note**: Advanced driver features like connection pooling, custom serialization, and cluster management APIs may encounter issues.
