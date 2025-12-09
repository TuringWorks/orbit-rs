# ArangoDB AQL Compatibility Specification

**Target**: ArangoDB AQL (ArangoDB Query Language) 3.x
**Reference**: https://www.arangodb.com/docs/stable/aql/
**Last Updated**: 2025-12-08
**Current Estimated Coverage**: ~50%

---

## Overview

This document specifies OrbitRS's compatibility with ArangoDB's AQL (ArangoDB Query Language). AQL is a declarative query language for multi-model databases, supporting document, graph, and key-value data models.

## Table of Contents

1. [AQL Operations](#aql-operations)
2. [Data Types](#data-types)
3. [Functions](#functions)
4. [Graph Operations](#graph-operations)
5. [Implementation Status](#implementation-status)

---

## AQL Operations

### Legend
- ✅ **Implemented** - Fully functional
- 🔶 **Partial** - Basic support, missing features
- ❌ **Not Implemented** - Not yet available

### Query Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| FOR | ✅ | Iterate over collections/arrays |
| RETURN | ✅ | Project results |
| FILTER | ✅ | Filter results |
| SORT | ✅ | Sort results |
| LIMIT | ✅ | Slice results |
| LET | ✅ | Assign variables |
| COLLECT | 🔶 | Group results |
| WINDOW | ❌ | Window operations |
| WITH | ❌ | Collection hints |

### Data Modification Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| INSERT | ✅ | Insert documents |
| UPDATE | ✅ | Update documents |
| REPLACE | ✅ | Replace documents |
| REMOVE | ✅ | Remove documents |
| UPSERT | ✅ | Insert or update |

### Subqueries

| Feature | Status | Notes |
|---------|--------|-------|
| Subquery in FOR | ✅ | Nested iteration |
| Subquery in LET | ✅ | Variable assignment |
| Subquery in FILTER | ✅ | Filtering |
| Subquery in RETURN | ✅ | Projection |

### Array Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| Array expansion ([*]) | ✅ | Expand arrays |
| Array filtering ([? expr]) | ✅ | Filter arrays |
| Array projection ([* expr]) | ✅ | Transform arrays |
| Array slicing | ✅ | Extract subarray |
| Array operators (ANY, ALL, NONE) | ✅ | Quantifiers |

### Graph Traversal

| Operation | Status | Notes |
|-----------|--------|-------|
| FOR v IN 1..3 OUTBOUND | 🔶 | Outbound traversal |
| FOR v IN 1..3 INBOUND | 🔶 | Inbound traversal |
| FOR v IN 1..3 ANY | 🔶 | Any direction |
| SHORTEST_PATH | ❌ | Shortest path |
| K_SHORTEST_PATHS | ❌ | K shortest paths |
| ALL_SHORTEST_PATHS | ❌ | All shortest paths |
| K_PATHS | ❌ | K paths |
| PRUNE | ❌ | Prune traversal |

### Index Hints

| Hint | Status | Notes |
|------|--------|-------|
| OPTIONS {indexHint: "..."} | ❌ | Index selection |
| OPTIONS {forceIndexHint: true} | ❌ | Force index |
| OPTIONS {disableIndex: true} | ❌ | Disable index |

---

## Data Types

### Primitive Types

| Type | Status | Notes |
|------|--------|-------|
| null | ✅ | Null value |
| boolean | ✅ | true/false |
| number | ✅ | 64-bit IEEE 754 |
| string | ✅ | UTF-8 strings |

### Compound Types

| Type | Status | Notes |
|------|--------|-------|
| array | ✅ | Ordered list |
| object | ✅ | Key-value pairs |

### Special Values

| Value | Status | Notes |
|-------|--------|-------|
| _key | ✅ | Document key |
| _id | ✅ | Document ID |
| _rev | ✅ | Revision ID |
| _from | ✅ | Edge source |
| _to | ✅ | Edge target |

---

## Functions

### String Functions

| Function | Status | Notes |
|----------|--------|-------|
| CONCAT() | ✅ | Concatenate strings |
| CONCAT_SEPARATOR() | ✅ | Concat with separator |
| CHAR_LENGTH() | ✅ | Character length |
| CONTAINS() | ✅ | Contains substring |
| FIND_FIRST() | ✅ | Find first occurrence |
| FIND_LAST() | ✅ | Find last occurrence |
| LEFT() | ✅ | Left substring |
| RIGHT() | ✅ | Right substring |
| LOWER() | ✅ | Lowercase |
| UPPER() | ✅ | Uppercase |
| LTRIM() | ✅ | Left trim |
| RTRIM() | ✅ | Right trim |
| TRIM() | ✅ | Trim whitespace |
| REVERSE() | ✅ | Reverse string |
| SPLIT() | ✅ | Split string |
| SUBSTITUTE() | ✅ | Replace substring |
| SUBSTRING() | ✅ | Extract substring |
| LIKE() | ✅ | Pattern matching |
| REGEX_TEST() | ✅ | Regex test |
| REGEX_REPLACE() | ✅ | Regex replace |
| REGEX_SPLIT() | ✅ | Regex split |
| REGEX_MATCHES() | ✅ | Regex matches |

### Numeric Functions

| Function | Status | Notes |
|----------|--------|-------|
| ABS() | ✅ | Absolute value |
| ACOS() | ✅ | Arc cosine |
| ASIN() | ✅ | Arc sine |
| ATAN() | ✅ | Arc tangent |
| ATAN2() | ✅ | Arc tangent 2 |
| AVERAGE() | ✅ | Average |
| CEIL() | ✅ | Ceiling |
| COS() | ✅ | Cosine |
| DEGREES() | ✅ | Radians to degrees |
| EXP() | ✅ | Exponential |
| EXP2() | ✅ | Base-2 exponential |
| FLOOR() | ✅ | Floor |
| LOG() | ✅ | Natural logarithm |
| LOG2() | ✅ | Base-2 logarithm |
| LOG10() | ✅ | Base-10 logarithm |
| MAX() | ✅ | Maximum |
| MEDIAN() | ✅ | Median |
| MIN() | ✅ | Minimum |
| PERCENTILE() | ✅ | Percentile |
| PI() | ✅ | Pi constant |
| POW() | ✅ | Power |
| RADIANS() | ✅ | Degrees to radians |
| RAND() | ✅ | Random number |
| RANGE() | ✅ | Number range |
| ROUND() | ✅ | Round |
| SIN() | ✅ | Sine |
| SQRT() | ✅ | Square root |
| STDDEV_POPULATION() | ✅ | Population std dev |
| STDDEV_SAMPLE() | ✅ | Sample std dev |
| SUM() | ✅ | Sum |
| TAN() | ✅ | Tangent |
| VARIANCE_POPULATION() | ✅ | Population variance |
| VARIANCE_SAMPLE() | ✅ | Sample variance |

### Date Functions

| Function | Status | Notes |
|----------|--------|-------|
| DATE_NOW() | ✅ | Current timestamp |
| DATE_ISO8601() | ✅ | Format as ISO 8601 |
| DATE_TIMESTAMP() | ✅ | Unix timestamp |
| IS_DATESTRING() | ✅ | Check date string |
| DATE_DAYOFWEEK() | ✅ | Day of week |
| DATE_YEAR() | ✅ | Extract year |
| DATE_MONTH() | ✅ | Extract month |
| DATE_DAY() | ✅ | Extract day |
| DATE_HOUR() | ✅ | Extract hour |
| DATE_MINUTE() | ✅ | Extract minute |
| DATE_SECOND() | ✅ | Extract second |
| DATE_MILLISECOND() | ✅ | Extract millisecond |
| DATE_DAYOFYEAR() | ✅ | Day of year |
| DATE_ISOWEEK() | ✅ | ISO week |
| DATE_LEAPYEAR() | ✅ | Is leap year |
| DATE_QUARTER() | ✅ | Quarter |
| DATE_DAYS_IN_MONTH() | ✅ | Days in month |
| DATE_ADD() | ✅ | Add duration |
| DATE_SUBTRACT() | ✅ | Subtract duration |
| DATE_DIFF() | ✅ | Date difference |
| DATE_COMPARE() | ✅ | Compare dates |
| DATE_FORMAT() | ✅ | Format date |
| DATE_TRUNC() | ✅ | Truncate date |
| DATE_ROUND() | ✅ | Round date |

### Array Functions

| Function | Status | Notes |
|----------|--------|-------|
| APPEND() | ✅ | Append element |
| COUNT() | ✅ | Count elements |
| FIRST() | ✅ | First element |
| FLATTEN() | ✅ | Flatten array |
| INTERSECTION() | ✅ | Array intersection |
| LAST() | ✅ | Last element |
| LENGTH() | ✅ | Array length |
| MINUS() | ✅ | Array difference |
| NTH() | ✅ | Nth element |
| OUTERSECTION() | ✅ | Symmetric difference |
| POP() | ✅ | Remove last |
| POSITION() | ✅ | Find position |
| PUSH() | ✅ | Add element |
| REMOVE_NTH() | ✅ | Remove nth |
| REMOVE_VALUE() | ✅ | Remove value |
| REMOVE_VALUES() | ✅ | Remove values |
| REVERSE() | ✅ | Reverse array |
| SHIFT() | ✅ | Remove first |
| SLICE() | ✅ | Extract slice |
| SORTED() | ✅ | Sort array |
| SORTED_UNIQUE() | ✅ | Sort and deduplicate |
| UNION() | ✅ | Array union |
| UNION_DISTINCT() | ✅ | Unique union |
| UNIQUE() | ✅ | Remove duplicates |
| UNSHIFT() | ✅ | Add to front |

### Document/Object Functions

| Function | Status | Notes |
|----------|--------|-------|
| ATTRIBUTES() | ✅ | Object keys |
| COUNT() | ✅ | Count attributes |
| HAS() | ✅ | Has attribute |
| IS_SAME_COLLECTION() | ✅ | Same collection check |
| KEEP() | ✅ | Keep attributes |
| KEYS() | ✅ | Object keys |
| MATCHES() | ✅ | Pattern matching |
| MERGE() | ✅ | Merge objects |
| MERGE_RECURSIVE() | ✅ | Recursive merge |
| PARSE_IDENTIFIER() | ✅ | Parse document ID |
| TRANSLATE() | ✅ | Translate values |
| UNSET() | ✅ | Remove attributes |
| UNSET_RECURSIVE() | ✅ | Recursive unset |
| VALUES() | ✅ | Object values |
| ZIP() | ✅ | Zip arrays to object |

### Type Check Functions

| Function | Status | Notes |
|----------|--------|-------|
| IS_NULL() | ✅ | Is null |
| IS_BOOL() | ✅ | Is boolean |
| IS_NUMBER() | ✅ | Is number |
| IS_STRING() | ✅ | Is string |
| IS_ARRAY() | ✅ | Is array |
| IS_OBJECT() | ✅ | Is object |
| IS_DOCUMENT() | ✅ | Is document |
| IS_DATESTRING() | ✅ | Is date string |
| IS_KEY() | ✅ | Is valid key |

### Type Cast Functions

| Function | Status | Notes |
|----------|--------|-------|
| TO_BOOL() | ✅ | Cast to boolean |
| TO_NUMBER() | ✅ | Cast to number |
| TO_STRING() | ✅ | Cast to string |
| TO_ARRAY() | ✅ | Cast to array |
| TO_LIST() | ✅ | Cast to list |

### Geo Functions

| Function | Status | Notes |
|----------|--------|-------|
| DISTANCE() | ❌ | Calculate distance |
| GEO_CONTAINS() | ❌ | Contains check |
| GEO_DISTANCE() | ❌ | Geo distance |
| GEO_EQUALS() | ❌ | Geo equals |
| GEO_INTERSECTS() | ❌ | Geo intersects |
| GEO_AREA() | ❌ | Calculate area |
| GEO_POINT() | ❌ | Create point |
| GEO_MULTIPOINT() | ❌ | Create multipoint |
| GEO_POLYGON() | ❌ | Create polygon |
| GEO_MULTIPOLYGON() | ❌ | Create multipolygon |
| GEO_LINESTRING() | ❌ | Create linestring |
| GEO_MULTILINESTRING() | ❌ | Create multilinestring |
| IS_IN_POLYGON() | ❌ | Point in polygon |

### Fulltext Functions

| Function | Status | Notes |
|----------|--------|-------|
| FULLTEXT() | ❌ | Fulltext search |

### Miscellaneous Functions

| Function | Status | Notes |
|----------|--------|-------|
| APPLY() | ✅ | Apply function |
| ASSERT() | ✅ | Assert condition |
| CALL() | ❌ | Call user function |
| COLLECTIONS() | ✅ | List collections |
| CURRENT_DATABASE() | ✅ | Current database |
| CURRENT_USER() | ✅ | Current user |
| DECODE_REV() | ✅ | Decode revision |
| DOCUMENT() | ✅ | Get document |
| FAIL() | ✅ | Fail query |
| HASH() | ✅ | Hash value |
| MD5() | ✅ | MD5 hash |
| NOT_NULL() | ✅ | First non-null |
| PASSTHRU() | ✅ | Pass through |
| SCHEMA_GET() | ❌ | Get schema |
| SCHEMA_VALIDATE() | ❌ | Validate schema |
| SHA1() | ✅ | SHA1 hash |
| SHA256() | ✅ | SHA256 hash |
| SHA512() | ✅ | SHA512 hash |
| SLEEP() | ✅ | Sleep |
| UUID() | ✅ | Generate UUID |
| V8() | ❌ | Execute JavaScript |
| VERSION() | ✅ | ArangoDB version |
| WARN() | ✅ | Warning message |

---

## Graph Operations

### Graph Traversal

| Feature | Status | Notes |
|---------|--------|-------|
| OUTBOUND traversal | 🔶 | Follow outgoing edges |
| INBOUND traversal | 🔶 | Follow incoming edges |
| ANY traversal | 🔶 | Follow any direction |
| Min/max depth | 🔶 | Depth constraints |
| Named graphs | 🔶 | Graph collections |
| Edge collections | ✅ | Direct edge access |

### Path Finding

| Algorithm | Status | Notes |
|-----------|--------|-------|
| SHORTEST_PATH | ❌ | Shortest path |
| K_SHORTEST_PATHS | ❌ | K shortest paths |
| ALL_SHORTEST_PATHS | ❌ | All shortest paths |
| K_PATHS | ❌ | K paths |

### Graph Functions

| Function | Status | Notes |
|----------|--------|-------|
| GRAPH_EDGES() | ❌ | Get graph edges |
| GRAPH_VERTICES() | ❌ | Get graph vertices |
| GRAPH_NEIGHBORS() | ❌ | Get neighbors |
| GRAPH_COMMON_NEIGHBORS() | ❌ | Common neighbors |
| GRAPH_COMMON_PROPERTIES() | ❌ | Common properties |
| GRAPH_PATHS() | ❌ | Find paths |
| GRAPH_SHORTEST_PATH() | ❌ | Shortest path |
| GRAPH_DISTANCE_TO() | ❌ | Distance to vertex |
| GRAPH_ABSOLUTE_ECCENTRICITY() | ❌ | Eccentricity |
| GRAPH_ECCENTRICITY() | ❌ | Eccentricity |
| GRAPH_ABSOLUTE_CLOSENESS() | ❌ | Closeness |
| GRAPH_CLOSENESS() | ❌ | Closeness |
| GRAPH_ABSOLUTE_BETWEENNESS() | ❌ | Betweenness |
| GRAPH_BETWEENNESS() | ❌ | Betweenness |
| GRAPH_RADIUS() | ❌ | Graph radius |
| GRAPH_DIAMETER() | ❌ | Graph diameter |

---

## Implementation Status

### Overall Coverage

| Category | Coverage | Notes |
|----------|----------|-------|
| Query Operations | ~90% | Core operations |
| Data Modification | ~90% | Full CRUD |
| Array Operations | ~90% | Comprehensive |
| String Functions | ~95% | Nearly complete |
| Numeric Functions | ~95% | Nearly complete |
| Date Functions | ~95% | Nearly complete |
| Array Functions | ~95% | Nearly complete |
| Object Functions | ~90% | Good coverage |
| Type Functions | ~95% | Nearly complete |
| Graph Traversal | ~40% | Basic support |
| Graph Algorithms | ~5% | Minimal |
| Geo Functions | ~0% | Not implemented |
| Fulltext | ~0% | Not implemented |

### ArangoDB Compatibility

| Feature Category | Compatibility | Notes |
|------------------|---------------|-------|
| Document Model | ~90% | Strong support |
| Key-Value | ~90% | Full support |
| Graph Model | ~40% | Basic traversal |
| Multi-Model | ~75% | Good integration |
| Functions | ~80% | Most functions |
| Indexes | ~60% | Basic indexes |

### Priority Roadmap

**High Priority**:
1. ✅ Core AQL operations (FOR, FILTER, RETURN)
2. ✅ Data modification (INSERT, UPDATE, DELETE)
3. ✅ Array operations
4. 🔶 Graph traversal
5. ❌ Path finding algorithms

**Medium Priority**:
1. ❌ SHORTEST_PATH
2. ❌ Graph algorithms
3. ❌ Geo functions
4. ❌ Fulltext search
5. ❌ User-defined functions

**Low Priority**:
1. ❌ Advanced graph analytics
2. ❌ Schema validation
3. ❌ JavaScript execution (V8)

---

## Known Limitations

1. **Graph Algorithms**: Limited path finding support
2. **Geospatial**: No geo functions implemented
3. **Full-Text Search**: Not implemented
4. **User Functions**: JavaScript execution not supported
5. **Schema Validation**: Not implemented
6. **Advanced Traversal**: PRUNE not supported
7. **Index Hints**: Not implemented
8. **Window Operations**: Not implemented
9. **Graph Analytics**: No centrality measures
10. **V8 Execution**: JavaScript functions not supported

---

## Client Compatibility

### Tested Drivers

| Driver | Status | Notes |
|--------|--------|-------|
| Python-arango | 🔶 | Basic queries work |
| ArangoJS | 🔶 | Basic queries work |
| arangodb-java-driver | 🔶 | Basic queries work |
| arangodbgo | 🔶 | Basic queries work |

---

## Version Compatibility

| ArangoDB Version | Compatibility | Notes |
|------------------|---------------|-------|
| ArangoDB 3.11 | 🔶 | Core features |
| ArangoDB 3.10 | 🔶 | Core features |
| ArangoDB 3.9 | ✅ | Good support |
| ArangoDB 3.8 | ✅ | Good support |

---

## References

- [ArangoDB AQL Documentation](https://www.arangodb.com/docs/stable/aql/)
- [AQL Functions Reference](https://www.arangodb.com/docs/stable/aql/functions.html)
- [AQL Graph Traversals](https://www.arangodb.com/docs/stable/aql/graphs.html)
- [ArangoDB Drivers](https://www.arangodb.com/docs/stable/drivers/)
