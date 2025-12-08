# ArangoDB AQL Protocol and Syntax Reference

> A comprehensive guide for implementing AQL support in Rust applications, including syntax, keywords, AST structure, parser details, and wire protocol specifications.

---

## Table of Contents

1. [Overview](#overview)
2. [Data Types](#data-types)
3. [Keywords and Reserved Words](#keywords-and-reserved-words)
4. [Operators](#operators)
5. [High-Level Operations](#high-level-operations)
6. [Graph Traversal Syntax](#graph-traversal-syntax)
7. [Functions](#functions)
8. [Abstract Syntax Tree (AST)](#abstract-syntax-tree-ast)
9. [Parser Details](#parser-details)
10. [Wire Protocol](#wire-protocol)
11. [HTTP API for Queries](#http-api-for-queries)
12. [Rust Implementation Guide](#rust-implementation-guide)
13. [Query Examples](#query-examples)

---

## Overview

ArangoDB Query Language (AQL) is a declarative query language for the ArangoDB multi-model database. It supports:

- **Document queries** (CRUD operations)
- **Graph traversals** (shortest path, k-shortest paths, all shortest paths)
- **Aggregations** (COLLECT, AGGREGATE)
- **Joins** across collections
- **Subqueries** and complex data transformations

### Key Characteristics

```text
Language Type:     Declarative (DML only, not DDL/DCL)
Case Sensitivity:  Keywords are case-insensitive; names are case-sensitive
Query Termination: No semicolons (single query per string)
Result Format:     Always returns an array of elements
```

### Query Structure

```aql
[WITH collection1, collection2, ...]
FOR variable IN expression
  [FILTER condition]
  [SORT expression [ASC|DESC]]
  [LIMIT offset, count]
  [LET variable = expression]
  [COLLECT ...]
  RETURN expression
```

---

## Data Types

### Primitive Types

| Type | Description | Literals | Rust Equivalent |
|------|-------------|----------|-----------------|
| `null` | Absent/empty value | `null` | `Option::None` |
| `bool` | Boolean | `true`, `false` | `bool` |
| `number` | 64-bit IEEE 754 double | `42`, `3.14`, `-1.5e10`, `0x1A`, `0b1010` | `f64` |
| `string` | UTF-8 text | `"hello"`, `'world'` | `String` |

### Compound Types

| Type | Description | Rust Equivalent |
|------|-------------|-----------------|
| `array` | Ordered list | `Vec<Value>` |
| `object` | Key-value pairs (document) | `HashMap<String, Value>` or custom struct |

### Type Ordering (for comparisons)

```text
null < bool < number < string < array < object
```

### Number Literals

```aql
// Decimal
42
-3.14
1.5e10

// Hexadecimal (unsigned, max 0xffffffff)
0xABCDEF

// Binary (unsigned, max 32-bit)
0b10101110
```

### String Escape Sequences

| Sequence | Meaning |
|----------|---------|
| `\\` | Backslash |
| `\'` | Single quote |
| `\"` | Double quote |
| `\n` | Newline |
| `\r` | Carriage return |
| `\t` | Tab |
| `\uXXXX` | Unicode codepoint |

---

## Keywords and Reserved Words

### High-Level Operation Keywords

```text
AGGREGATE    ALL          AND          ANY          ASC
COLLECT      DESC         DISTINCT     FALSE        FILTER
FOR          GRAPH        IN           INBOUND      INSERT
INTO         K_PATHS      K_SHORTEST_PATHS          LET
LIKE         LIMIT        NONE         NOT          NULL
OR           OUTBOUND     REMOVE       REPLACE      RETURN
SHORTEST_PATH SORT        TRUE         UPDATE       UPSERT
WINDOW       WITH
```

### Full Reserved Keywords List

```rust
pub const AQL_KEYWORDS: &[&str] = &[
    // High-level operations
    "FOR", "RETURN", "FILTER", "SEARCH", "SORT", "LIMIT", "LET",
    "COLLECT", "INSERT", "UPDATE", "REPLACE", "REMOVE", "UPSERT",
    "WITH", "WINDOW",
    
    // Graph operations
    "GRAPH", "OUTBOUND", "INBOUND", "ANY",
    "SHORTEST_PATH", "K_SHORTEST_PATHS", "K_PATHS", "ALL_SHORTEST_PATHS",
    "PRUNE",
    
    // Modifiers
    "ASC", "DESC", "DISTINCT",
    "INTO", "IN", "TO",
    
    // Logical operators
    "AND", "OR", "NOT",
    
    // Literals
    "TRUE", "FALSE", "NULL",
    
    // Quantifiers
    "ALL", "ANY", "NONE", "AT LEAST",
    
    // Aggregate keywords
    "AGGREGATE", "COUNT", "KEEP",
    
    // Pattern matching
    "LIKE",
    
    // Options
    "OPTIONS",
];
```

### Contextual Keywords (Not Reserved)

These are identified by parser context and can be used as names:

```text
OPTIONS     - FOR / SEARCH / COLLECT / INSERT / UPDATE / REPLACE / UPSERT / REMOVE
KEEP        - COLLECT operation
COUNT       - COLLECT operation  
AGGREGATE   - COLLECT operation
PRUNE       - Graph traversal
TO          - Shortest path queries
```

### Special Variables (Case-Sensitive)

| Variable | Context | Description |
|----------|---------|-------------|
| `CURRENT` | Array inline expressions, `?` operator | Current array element |
| `NEW` | After INSERT/UPDATE/REPLACE/UPSERT | The newly created/modified document |
| `OLD` | After UPDATE/REPLACE/UPSERT/REMOVE | The original document before modification |

---

## Operators

### Comparison Operators

| Operator | Description | Notes |
|----------|-------------|-------|
| `==` | Equality | Strict type comparison |
| `!=` | Inequality | Strict type comparison |
| `<` | Less than | |
| `<=` | Less than or equal | |
| `>` | Greater than | |
| `>=` | Greater than or equal | |
| `IN` | Membership test | Right operand must be array |
| `NOT IN` | Negative membership | Right operand must be array |
| `LIKE` | Pattern matching | `%` = any chars, `_` = single char |
| `NOT LIKE` | Negative pattern | |
| `=~` | Regex match | Right operand is regex string |
| `!~` | Negative regex match | |

### Logical Operators

| Operator | Description |
|----------|-------------|
| `AND`, `&&` | Logical AND |
| `OR`, `\|\|` | Logical OR |
| `NOT`, `!` | Logical NOT |

### Arithmetic Operators

| Operator | Description |
|----------|-------------|
| `+` | Addition |
| `-` | Subtraction |
| `*` | Multiplication |
| `/` | Division |
| `%` | Modulo |

### Array Operators

| Operator | Description |
|----------|-------------|
| `[*]` | Array expansion |
| `[**]` | Array contraction (flatten 1 level) |
| `[***]` | Array contraction (flatten 2 levels) |
| `[? ...]` | Array filter with quantifier |
| `[n]` | Index access (0-based) |
| `[-n]` | Negative index (from end) |

### Array Quantifiers (with comparison operators)

```aql
// ALL, ANY, NONE prefix
ALL elements == value
ANY elements > value
NONE elements IN array

// AT LEAST quantifier
AT LEAST (2) elements > 10
```

### Object/Attribute Access

| Operator | Description |
|----------|-------------|
| `.` | Attribute access |
| `[expr]` | Dynamic attribute access |
| `?.` | Optional chaining (returns null if missing) |

### Ternary Operator

```aql
// Full form
condition ? value_if_true : value_if_false

// Short form (returns condition if truthy)
value ?: default_value
```

### Range Operator

```aql
1..10        // [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
start..end   // Inclusive range
```

### String Concatenation

```aql
CONCAT(str1, str2, ...)
// or use CONCAT_SEPARATOR
CONCAT_SEPARATOR(", ", str1, str2, ...)
```

---

## High-Level Operations

### FOR - Iteration

```aql
// Iterate over collection
FOR doc IN collection
  RETURN doc

// Iterate over array
FOR item IN [1, 2, 3]
  RETURN item * 2

// Nested loops (cross product)
FOR a IN collection1
  FOR b IN collection2
    RETURN { a, b }
```

### FILTER - Conditions

```aql
FOR doc IN collection
  FILTER doc.status == "active"
  FILTER doc.age >= 18       // Multiple FILTERs = AND
  RETURN doc

// Equivalent
FOR doc IN collection
  FILTER doc.status == "active" AND doc.age >= 18
  RETURN doc
```

### SORT - Ordering

```aql
FOR doc IN collection
  SORT doc.name ASC, doc.age DESC
  RETURN doc

// NULL handling
SORT doc.value ASC NULLS FIRST
SORT doc.value DESC NULLS LAST
```

### LIMIT - Pagination

```aql
// Limit count
LIMIT 10

// Offset and count
LIMIT 20, 10  // Skip 20, return 10
```

### LET - Variable Assignment

```aql
FOR doc IN collection
  LET fullName = CONCAT(doc.firstName, " ", doc.lastName)
  LET age = DATE_DIFF(doc.birthDate, DATE_NOW(), "year")
  RETURN { fullName, age }
```

### COLLECT - Grouping and Aggregation

```aql
// Basic grouping
FOR doc IN collection
  COLLECT city = doc.city
  RETURN city

// Grouping with aggregation
FOR doc IN collection
  COLLECT city = doc.city
  AGGREGATE total = SUM(1), avgAge = AVG(doc.age)
  RETURN { city, total, avgAge }

// Count into variable
FOR doc IN collection
  COLLECT WITH COUNT INTO count
  RETURN count

// Keep grouped values
FOR doc IN collection
  COLLECT city = doc.city INTO groups KEEP doc
  RETURN { city, documents: groups[*].doc }

// Multiple grouping keys
FOR doc IN collection
  COLLECT country = doc.country, city = doc.city
  RETURN { country, city }
```

### WINDOW - Sliding Window Aggregation

```aql
FOR doc IN collection
  SORT doc.date
  WINDOW { preceding: 2, following: 0 }
  AGGREGATE movingAvg = AVG(doc.value)
  RETURN { date: doc.date, value: doc.value, movingAvg }
```

### INSERT - Create Documents

```aql
// Single insert
INSERT { name: "John", age: 30 } INTO users

// With return
INSERT { name: "John" } INTO users
RETURN NEW

// Bulk insert
FOR item IN @items
  INSERT item INTO collection
```

### UPDATE - Modify Documents

```aql
// Update by key
UPDATE "key123" WITH { status: "active" } IN users

// Update with merge
FOR doc IN users
  FILTER doc.status == "pending"
  UPDATE doc WITH { status: "active", updated: DATE_NOW() } IN users
  RETURN { old: OLD, new: NEW }
```

### REPLACE - Full Document Replacement

```aql
REPLACE "key123" WITH { name: "New Name", status: "active" } IN users
RETURN NEW
```

### REMOVE - Delete Documents

```aql
// Remove by key
REMOVE "key123" IN users

// Bulk remove
FOR doc IN users
  FILTER doc.status == "deleted"
  REMOVE doc IN users
  RETURN OLD
```

### UPSERT - Insert or Update

```aql
UPSERT { email: "john@example.com" }
INSERT { email: "john@example.com", name: "John", created: DATE_NOW() }
UPDATE { name: "John", updated: DATE_NOW() }
IN users
RETURN { isInsert: IS_NULL(OLD), doc: NEW }
```

### WITH - Collection Declaration

```aql
// Required for cluster deployments
WITH users, profiles
FOR u IN users
  FOR p IN profiles
    FILTER u._key == p.userId
    RETURN { user: u, profile: p }
```

### RETURN - Output

```aql
// Return document
RETURN doc

// Return projection
RETURN { name: doc.name, email: doc.email }

// Return distinct values
RETURN DISTINCT doc.category
```

---

## Graph Traversal Syntax

### Basic Traversal

```aql
// Named graph
FOR vertex, edge, path IN [min[..max]] OUTBOUND|INBOUND|ANY startVertex
  GRAPH graphName
  [PRUNE pruneCondition]
  [OPTIONS options]
  RETURN vertex

// Anonymous graph (edge collections)
FOR vertex, edge, path IN 1..5 OUTBOUND startVertex
  edgeCollection1, edgeCollection2
  RETURN vertex
```

### Traversal Parameters

| Parameter | Description |
|-----------|-------------|
| `vertex` | Current vertex document |
| `edge` | Edge leading to current vertex |
| `path` | Object with `vertices` and `edges` arrays |
| `min..max` | Depth range (default: 1..1) |

### Direction Keywords

| Keyword | Description |
|---------|-------------|
| `OUTBOUND` | Follow edges in `_from` → `_to` direction |
| `INBOUND` | Follow edges in `_to` → `_from` direction |
| `ANY` | Follow edges in both directions |

### Traversal Options

```aql
FOR v, e, p IN 1..5 OUTBOUND @startVertex GRAPH 'myGraph'
OPTIONS {
  bfs: true,                    // Breadth-first search
  uniqueVertices: "global",     // "none", "path", "global"
  uniqueEdges: "path",          // "none", "path"
  maxProjections: 5,
  parallelism: 4
}
RETURN v
```

### PRUNE - Early Termination

```aql
FOR v, e, p IN 1..10 OUTBOUND @start GRAPH 'myGraph'
  PRUNE v.type == "leaf"        // Stop traversal at leaf nodes
  RETURN v
```

### Shortest Path

```aql
// Single shortest path
FOR vertex, edge IN OUTBOUND SHORTEST_PATH
  startVertex TO targetVertex
  GRAPH graphName
  OPTIONS { weightAttribute: "distance", defaultWeight: 1 }
  RETURN { vertex, edge }
```

### K Shortest Paths

```aql
// Multiple shortest paths
FOR path IN OUTBOUND K_SHORTEST_PATHS
  startVertex TO targetVertex
  GRAPH graphName
  OPTIONS { weightAttribute: "cost" }
  LIMIT 5
  RETURN {
    vertices: path.vertices[*].name,
    weight: path.weight
  }
```

### All Shortest Paths

```aql
FOR path IN OUTBOUND ALL_SHORTEST_PATHS
  startVertex TO targetVertex
  GRAPH graphName
  RETURN path.vertices[*]._key
```

---

## Functions

### Type Functions

```rust
// Type checking
IS_NULL(value)      -> bool
IS_BOOL(value)      -> bool
IS_NUMBER(value)    -> bool
IS_STRING(value)    -> bool
IS_ARRAY(value)     -> bool
IS_OBJECT(value)    -> bool
IS_DOCUMENT(value)  -> bool    // Same as IS_OBJECT
IS_DATESTRING(str)  -> bool
IS_KEY(value)       -> bool    // Valid document key

// Type conversion
TO_BOOL(value)      -> bool
TO_NUMBER(value)    -> number
TO_STRING(value)    -> string
TO_ARRAY(value)     -> array
TO_LIST(value)      -> array   // Alias for TO_ARRAY

// Type info
TYPENAME(value)     -> string  // "null", "bool", "number", "string", "array", "object"
```

### String Functions

```rust
CONCAT(str1, str2, ...)              -> string
CONCAT_SEPARATOR(sep, str1, ...)     -> string
LENGTH(str)                          -> number
LOWER(str)                           -> string
UPPER(str)                           -> string
TRIM(str [, chars])                  -> string
LTRIM(str [, chars])                 -> string
RTRIM(str [, chars])                 -> string
SUBSTRING(str, offset [, length])    -> string
LEFT(str, n)                         -> string
RIGHT(str, n)                        -> string
CONTAINS(str, search [, returnIdx])  -> bool|number
FIND_FIRST(str, search [, start])    -> number
FIND_LAST(str, search [, start])     -> number
SPLIT(str, separator [, limit])      -> array
SUBSTITUTE(str, search, replace)     -> string
REVERSE(str)                         -> string
MD5(str)                             -> string
SHA1(str)                            -> string
SHA256(str)                          -> string
SHA512(str)                          -> string
LIKE(str, pattern [, caseInsensitive]) -> bool
REGEX_TEST(str, regex [, caseInsensitive]) -> bool
REGEX_REPLACE(str, regex, replacement) -> string
REGEX_MATCHES(str, regex)            -> array
ENCODE_URI_COMPONENT(str)            -> string
JSON_STRINGIFY(value)                -> string
JSON_PARSE(str)                      -> value
```

### Numeric Functions

```rust
ABS(value)                  -> number
ACOS(value)                 -> number
ASIN(value)                 -> number
ATAN(value)                 -> number
ATAN2(y, x)                 -> number
CEIL(value)                 -> number
COS(value)                  -> number
DEGREES(radians)            -> number
EXP(value)                  -> number
EXP2(value)                 -> number
FLOOR(value)                -> number
LOG(value)                  -> number
LOG2(value)                 -> number
LOG10(value)                -> number
MAX(value1, value2, ...)    -> number
MIN(value1, value2, ...)    -> number
PI()                        -> number
POW(base, exponent)         -> number
RADIANS(degrees)            -> number
RAND()                      -> number  // 0 <= x < 1
RANGE(start, end [, step])  -> array
ROUND(value)                -> number
SIN(value)                  -> number
SQRT(value)                 -> number
TAN(value)                  -> number
```

### Array Functions

```rust
APPEND(array, values [, unique])     -> array
COUNT(array)                         -> number
FIRST(array)                         -> value
FLATTEN(array [, depth])             -> array
INTERSECTION(arr1, arr2, ...)        -> array
LAST(array)                          -> value
LENGTH(array)                        -> number
MINUS(arr1, arr2, ...)               -> array
NTH(array, index)                    -> value
POP(array)                           -> array
POSITION(array, search [, returnIdx]) -> bool|number
PUSH(array, value [, unique])        -> array
REMOVE_NTH(array, index)             -> array
REMOVE_VALUE(array, value [, limit]) -> array
REMOVE_VALUES(array, values)         -> array
REVERSE(array)                       -> array
SHIFT(array)                         -> array
SLICE(array, start [, length])       -> array
SORTED(array)                        -> array
SORTED_UNIQUE(array)                 -> array
UNION(arr1, arr2, ...)               -> array
UNION_DISTINCT(arr1, arr2, ...)      -> array
UNIQUE(array)                        -> array
UNSHIFT(array, value [, unique])     -> array
```

### Object/Document Functions

```rust
ATTRIBUTES(doc [, removeInternal])   -> array
COUNT(object)                        -> number
HAS(doc, attributeName)              -> bool
IS_SAME_COLLECTION(doc, collection)  -> bool
KEEP(doc, attr1, attr2, ...)         -> object
MATCHES(doc, examples [, returnIdx]) -> bool|number
MERGE(obj1, obj2, ...)               -> object
MERGE_RECURSIVE(obj1, obj2, ...)     -> object
PARSE_IDENTIFIER(docHandle)          -> object
TRANSLATE(value, lookup [, default]) -> value
UNSET(doc, attr1, attr2, ...)        -> object
UNSET_RECURSIVE(doc, attr1, ...)     -> object
VALUES(doc [, removeInternal])       -> array
ZIP(keys, values)                    -> object
```

### Date Functions

```rust
DATE_NOW()                           -> number (timestamp)
DATE_ISO8601(date)                   -> string
DATE_TIMESTAMP(date)                 -> number
DATE_YEAR(date)                      -> number
DATE_MONTH(date)                     -> number
DATE_DAY(date)                       -> number
DATE_HOUR(date)                      -> number
DATE_MINUTE(date)                    -> number
DATE_SECOND(date)                    -> number
DATE_MILLISECOND(date)               -> number
DATE_DAYOFWEEK(date)                 -> number (0=Sunday)
DATE_DAYOFYEAR(date)                 -> number
DATE_ISOWEEK(date)                   -> number
DATE_LEAPYEAR(date)                  -> bool
DATE_ADD(date, amount, unit)         -> string
DATE_SUBTRACT(date, amount, unit)    -> string
DATE_DIFF(date1, date2, unit)        -> number
DATE_COMPARE(date1, date2, unitMax [, unitMin]) -> number
DATE_FORMAT(date, format)            -> string
DATE_ROUND(date, amount, unit)       -> string
DATE_TRUNC(date, unit)               -> string
```

### Aggregate Functions (use in COLLECT)

```rust
COUNT(expr)           -> number
SUM(expr)             -> number
MIN(expr)             -> value
MAX(expr)             -> value
AVG(expr)             -> number
VARIANCE(expr)        -> number
VARIANCE_SAMPLE(expr) -> number
STDDEV(expr)          -> number
STDDEV_SAMPLE(expr)   -> number
UNIQUE(expr)          -> array
SORTED_UNIQUE(expr)   -> array
COUNT_DISTINCT(expr)  -> number
BIT_AND(expr)         -> number
BIT_OR(expr)          -> number
BIT_XOR(expr)         -> number
```

### Miscellaneous Functions

```rust
DOCUMENT(collection, key)            -> document
DOCUMENT(documentHandle)             -> document
COLLECTION_COUNT(collection)         -> number
COLLECTIONS()                        -> array
CURRENT_DATABASE()                   -> string
CURRENT_USER()                       -> string|null
ASSERT(condition, message)           -> void
WARN(condition, message)             -> void
SLEEP(seconds)                       -> null
V8(expression)                       -> value
CALL(funcName, arg1, ...)            -> value
APPLY(funcName, argsArray)           -> value
VERSION()                            -> string
```

---

## Abstract Syntax Tree (AST)

### AST Node Types

```rust
pub enum AstNodeType {
    // Root
    Root,
    
    // Literals
    ValueNull,
    ValueBool,
    ValueInt,
    ValueDouble,
    ValueString,
    
    // Compound types
    Array,
    Object,
    ObjectElement,
    
    // References
    Reference,
    Variable,
    Parameter,           // Bind parameter (@name, @@collection)
    
    // Expressions
    UnaryPlus,
    UnaryMinus,
    UnaryNot,
    
    // Binary operators
    BinaryAnd,
    BinaryOr,
    BinaryPlus,
    BinaryMinus,
    BinaryTimes,
    BinaryDiv,
    BinaryMod,
    
    // Comparisons
    CompareEq,
    CompareNe,
    CompareLt,
    CompareLe,
    CompareGt,
    CompareGe,
    CompareIn,
    CompareNin,
    CompareLike,
    CompareRegex,
    
    // Access
    AttributeAccess,
    IndexedAccess,
    Expansion,           // [*]
    
    // Ternary
    Ternary,
    
    // Range
    Range,
    
    // Operations
    For,
    Let,
    Filter,
    Return,
    Sort,
    SortElement,
    Limit,
    Collect,
    CollectGroup,
    Aggregation,
    
    // Data modification
    Insert,
    Update,
    Replace,
    Remove,
    Upsert,
    
    // Graph
    Traversal,
    ShortestPath,
    KShortestPaths,
    AllShortestPaths,
    
    // Functions
    FunctionCall,
    
    // Subquery
    Subquery,
    
    // Special
    With,
    Window,
    Prune,
    Options,
    
    // Quantifiers
    QuantifierAll,
    QuantifierAny,
    QuantifierNone,
}
```

### AST Node Structure

```rust
#[derive(Debug, Clone)]
pub struct AstNode {
    pub node_type: AstNodeType,
    pub value: Option<AstValue>,
    pub children: Vec<AstNode>,
    pub location: SourceLocation,
}

#[derive(Debug, Clone)]
pub enum AstValue {
    Null,
    Bool(bool),
    Int(i64),
    Double(f64),
    String(String),
}

#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub line: u32,
    pub column: u32,
    pub offset: usize,
}
```

### Example AST

For query: `FOR doc IN users FILTER doc.age > 18 RETURN doc.name`

```json
{
  "type": "root",
  "subNodes": [
    {
      "type": "for",
      "subNodes": [
        {
          "type": "variable",
          "name": "doc"
        },
        {
          "type": "collection",
          "name": "users"
        },
        {
          "type": "filter",
          "subNodes": [
            {
              "type": "compare >",
              "subNodes": [
                {
                  "type": "attribute access",
                  "subNodes": [
                    { "type": "reference", "name": "doc" },
                    { "type": "value", "value": "age" }
                  ]
                },
                { "type": "value", "value": 18 }
              ]
            }
          ]
        },
        {
          "type": "return",
          "subNodes": [
            {
              "type": "attribute access",
              "subNodes": [
                { "type": "reference", "name": "doc" },
                { "type": "value", "value": "name" }
              ]
            }
          ]
        }
      ]
    }
  ]
}
```

### Retrieving AST via API

```javascript
// Using ArangoShell
var stmt = db._createStatement("FOR doc IN users RETURN doc");
var parsed = stmt.parse();
// parsed.ast contains the AST
// parsed.collections contains collection names
// parsed.bindVars contains bind parameter names
```

---

## Parser Details

### Lexer Tokens

```rust
pub enum Token {
    // Literals
    Null,
    True,
    False,
    IntegerLiteral(i64),
    DoubleLiteral(f64),
    StringLiteral(String),
    
    // Identifiers
    Identifier(String),
    BindParameter(String),      // @name
    CollectionParameter(String), // @@name
    
    // Keywords (use enum or string matching)
    For,
    Return,
    Filter,
    // ... all keywords
    
    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
    In,
    Like,
    RegexMatch,
    RegexNoMatch,
    Question,
    Colon,
    Range,           // ..
    
    // Delimiters
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Dot,
    
    // Special
    Arrow,           // =>
    OptionalChain,   // ?.
    
    // End
    Eof,
}
```

### Grammar Rules (EBNF-style)

```ebnf
query          = [with_clause] statement ;

with_clause    = "WITH" collection_list ;
collection_list = collection ("," collection)* ;

statement      = for_statement
               | let_statement
               | filter_statement
               | return_statement
               | collect_statement
               | sort_statement
               | limit_statement
               | insert_statement
               | update_statement
               | replace_statement
               | remove_statement
               | upsert_statement
               | window_statement ;

for_statement  = "FOR" variable "IN" expression [options] statement* return_statement ;

filter_statement = "FILTER" expression ;

return_statement = "RETURN" ["DISTINCT"] expression ;

sort_statement = "SORT" sort_element ("," sort_element)* ;
sort_element   = expression ["ASC" | "DESC"] ;

limit_statement = "LIMIT" expression ["," expression] ;

let_statement  = "LET" variable "=" expression ;

collect_statement = "COLLECT" collect_element ("," collect_element)*
                   ["INTO" variable ["=" expression]]
                   ["KEEP" variable ("," variable)*]
                   ["WITH" "COUNT" "INTO" variable]
                   ["AGGREGATE" aggregate_element ("," aggregate_element)*]
                   [options] ;

collect_element = variable "=" expression ;
aggregate_element = variable "=" aggregate_function ;

expression     = ternary_expression ;
ternary_expression = or_expression ["?" expression ":" expression] ;
or_expression  = and_expression (("OR" | "||") and_expression)* ;
and_expression = not_expression (("AND" | "&&") not_expression)* ;
not_expression = ["NOT" | "!"] comparison_expression ;

comparison_expression = additive_expression 
                       [(comparison_op) additive_expression] ;
comparison_op = "==" | "!=" | "<" | "<=" | ">" | ">=" 
              | "IN" | "NOT IN" | "LIKE" | "NOT LIKE" | "=~" | "!~" ;

additive_expression = multiplicative_expression 
                     (("+" | "-") multiplicative_expression)* ;
multiplicative_expression = unary_expression 
                           (("*" | "/" | "%") unary_expression)* ;
unary_expression = ["+" | "-"] postfix_expression ;

postfix_expression = primary_expression (postfix_op)* ;
postfix_op     = "." identifier
               | "[" expression "]"
               | "[*]"
               | "(" [argument_list] ")"
               | "?." identifier ;

primary_expression = literal
                   | variable
                   | "(" expression ")"
                   | "[" [expression_list] "]"
                   | "{" [object_elements] "}"
                   | subquery
                   | function_call ;

literal        = "NULL" | "TRUE" | "FALSE" | number | string ;
subquery       = "(" statement* return_statement ")" ;
function_call  = identifier "(" [argument_list] ")" ;
```

### Bind Parameters

```aql
// Value parameters (prefixed with @)
FOR doc IN users
  FILTER doc.name == @name
  RETURN doc

// Collection parameters (prefixed with @@)
FOR doc IN @@collection
  RETURN doc
```

```rust
#[derive(Debug, Clone)]
pub struct BindParameter {
    pub name: String,
    pub is_collection: bool,  // @@ vs @
}
```

### Comments

```aql
// Single line comment

/* Multi-line
   comment */
```

---

## Wire Protocol

### HTTP API (Primary Protocol)

ArangoDB primarily uses HTTP/HTTPS for client communication. As of v3.12, the VelocyStream binary protocol is deprecated.

#### Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/_api/cursor` | POST | Execute AQL query |
| `/_api/cursor/{id}` | POST | Fetch next batch (replaces PUT) |
| `/_api/cursor/{id}` | DELETE | Delete cursor |
| `/_api/explain` | POST | Explain query plan |
| `/_api/query` | POST | Parse and validate query |
| `/_api/query/current` | GET | List running queries |
| `/_api/query/slow` | GET | List slow queries |
| `/_api/query/{id}` | DELETE | Kill running query |

#### Query Request Format

```http
POST /_api/cursor HTTP/1.1
Host: localhost:8529
Authorization: Basic cm9vdDo=
Content-Type: application/json

{
  "query": "FOR doc IN @@collection FILTER doc.age > @minAge RETURN doc",
  "bindVars": {
    "@collection": "users",
    "minAge": 18
  },
  "batchSize": 1000,
  "count": true,
  "ttl": 30,
  "options": {
    "fullCount": true,
    "maxPlans": 10,
    "optimizer": {
      "rules": ["+use-indexes", "-all"]
    }
  }
}
```

#### Query Options

| Option | Type | Description |
|--------|------|-------------|
| `batchSize` | int | Max docs per batch (default: 1000) |
| `count` | bool | Include total count |
| `ttl` | int | Cursor time-to-live in seconds |
| `cache` | bool | Use query cache |
| `memoryLimit` | int | Max memory in bytes |
| `fullCount` | bool | Count before LIMIT |
| `maxPlans` | int | Max execution plans |
| `maxWarningCount` | int | Max warnings to return |
| `profile` | int | Profiling level (0-2) |
| `stream` | bool | Stream results |
| `optimizer.rules` | array | Optimizer rule modifications |

#### Cursor Response Format

```json
{
  "id": "12345",
  "result": [
    {"_key": "1", "name": "Alice"},
    {"_key": "2", "name": "Bob"}
  ],
  "hasMore": true,
  "count": 1000,
  "cached": false,
  "extra": {
    "warnings": [],
    "stats": {
      "writesExecuted": 0,
      "writesIgnored": 0,
      "scannedFull": 1000,
      "scannedIndex": 0,
      "cursorsCreated": 1,
      "cursorsRearmed": 0,
      "cacheHits": 0,
      "cacheMisses": 0,
      "filtered": 500,
      "httpRequests": 0,
      "executionTime": 0.025,
      "peakMemoryUsage": 1048576
    }
  },
  "error": false,
  "code": 201
}
```

### VelocyPack (VPack) Binary Format

VelocyPack is ArangoDB's binary JSON-like format for internal storage and optionally for wire communication.

#### VPack Type Bytes

| Range | Type |
|-------|------|
| `0x00` | None/Illegal |
| `0x01` | Empty array |
| `0x02-0x05` | Array (1-4 byte length) |
| `0x06-0x09` | Array with index table |
| `0x0a` | Empty object |
| `0x0b-0x0e` | Object (sorted, 1-4 byte length) |
| `0x0f-0x12` | Object (unsorted) |
| `0x13` | Compact array |
| `0x14` | Compact object |
| `0x17` | Illegal |
| `0x18` | Null |
| `0x19` | False |
| `0x1a` | True |
| `0x1b` | Double |
| `0x1c` | UTC Date |
| `0x1d` | External |
| `0x1e` | MinKey |
| `0x1f` | MaxKey |
| `0x20-0x27` | Signed int (1-8 bytes) |
| `0x28-0x2f` | Unsigned int (1-8 bytes) |
| `0x30-0x39` | Small integers (0-9) |
| `0x3a-0x3f` | Small negative (-6 to -1) |
| `0x40-0xbe` | Short strings (length = byte - 0x40) |
| `0xbf` | Long string (8-byte length) |
| `0xc0-0xc7` | Binary data (1-8 byte length) |
| `0xc8-0xcf` | Positive BCD number |
| `0xd0-0xd7` | Negative BCD number |

---

## HTTP API for Queries

### Execute Query

```http
POST /_db/{database}/_api/cursor
```

### Request Body

```rust
#[derive(Serialize)]
pub struct QueryRequest {
    pub query: String,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bind_vars: Option<HashMap<String, Value>>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<u32>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<bool>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u32>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<QueryOptions>,
}

#[derive(Serialize)]
pub struct QueryOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_count: Option<bool>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_plans: Option<u32>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_limit: Option<u64>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<u8>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optimizer: Option<OptimizerOptions>,
}

#[derive(Serialize)]
pub struct OptimizerOptions {
    pub rules: Vec<String>,
}
```

### Response Body

```rust
#[derive(Deserialize)]
pub struct CursorResponse<T> {
    pub id: Option<String>,
    pub result: Vec<T>,
    
    #[serde(rename = "hasMore")]
    pub has_more: bool,
    
    pub count: Option<u64>,
    pub cached: bool,
    pub extra: Option<CursorExtra>,
    pub error: bool,
    pub code: u16,
}

#[derive(Deserialize)]
pub struct CursorExtra {
    pub warnings: Vec<QueryWarning>,
    pub stats: QueryStats,
}

#[derive(Deserialize)]
pub struct QueryStats {
    #[serde(rename = "writesExecuted")]
    pub writes_executed: u64,
    
    #[serde(rename = "writesIgnored")]
    pub writes_ignored: u64,
    
    #[serde(rename = "scannedFull")]
    pub scanned_full: u64,
    
    #[serde(rename = "scannedIndex")]
    pub scanned_index: u64,
    
    pub filtered: u64,
    
    #[serde(rename = "executionTime")]
    pub execution_time: f64,
    
    #[serde(rename = "peakMemoryUsage")]
    pub peak_memory_usage: u64,
}
```

### Fetch Next Batch

```http
POST /_db/{database}/_api/cursor/{cursor-id}
```

### Delete Cursor

```http
DELETE /_db/{database}/_api/cursor/{cursor-id}
```

### Explain Query

```http
POST /_db/{database}/_api/explain
```

```json
{
  "query": "FOR doc IN users FILTER doc.age > 18 RETURN doc",
  "options": {
    "allPlans": true,
    "maxNumberOfPlans": 10
  }
}
```

### Parse Query (Validate Syntax)

```http
POST /_db/{database}/_api/query
```

```json
{
  "query": "FOR doc IN users RETURN doc"
}
```

Response includes AST and collection/bind variable information.

---

## Rust Implementation Guide

### Recommended Crates

```toml
[dependencies]
# HTTP client
reqwest = { version = "0.11", features = ["json"] }

# Async runtime
tokio = { version = "1", features = ["full"] }

# JSON serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Existing ArangoDB drivers (optional)
arangors = "0.5"           # Full-featured async driver
arangors_lite = "0.2"      # Lighter alternative
```

### Basic Client Implementation

```rust
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub struct ArangoClient {
    client: Client,
    base_url: String,
    database: String,
}

impl ArangoClient {
    pub fn new(url: &str, database: &str, username: &str, password: &str) -> Self {
        let auth = base64::encode(format!("{}:{}", username, password));
        
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&format!("Basic {}", auth)).unwrap(),
        );
        
        let client = Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();
        
        Self {
            client,
            base_url: url.to_string(),
            database: database.to_string(),
        }
    }
    
    pub async fn query<T: for<'de> Deserialize<'de>>(
        &self,
        query: &str,
        bind_vars: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<T>, ArangoError> {
        let url = format!("{}/_db/{}/_api/cursor", self.base_url, self.database);
        
        let body = QueryRequest {
            query: query.to_string(),
            bind_vars,
            batch_size: Some(1000),
            count: Some(true),
            ttl: None,
            options: None,
        };
        
        let response = self.client
            .post(&url)
            .json(&body)
            .send()
            .await?;
        
        let cursor: CursorResponse<T> = response.json().await?;
        
        if cursor.error {
            return Err(ArangoError::QueryError(cursor.code));
        }
        
        let mut results = cursor.result;
        
        // Fetch remaining batches
        if cursor.has_more {
            if let Some(cursor_id) = cursor.id {
                results.extend(self.fetch_all_batches(&cursor_id).await?);
            }
        }
        
        Ok(results)
    }
    
    async fn fetch_all_batches<T: for<'de> Deserialize<'de>>(
        &self,
        cursor_id: &str,
    ) -> Result<Vec<T>, ArangoError> {
        let mut results = Vec::new();
        let mut current_id = cursor_id.to_string();
        
        loop {
            let url = format!(
                "{}/_db/{}/_api/cursor/{}",
                self.base_url, self.database, current_id
            );
            
            let response = self.client.post(&url).send().await?;
            let cursor: CursorResponse<T> = response.json().await?;
            
            results.extend(cursor.result);
            
            if !cursor.has_more {
                // Delete cursor
                let delete_url = format!(
                    "{}/_db/{}/_api/cursor/{}",
                    self.base_url, self.database, current_id
                );
                let _ = self.client.delete(&delete_url).send().await;
                break;
            }
            
            if let Some(id) = cursor.id {
                current_id = id;
            } else {
                break;
            }
        }
        
        Ok(results)
    }
}
```

### AQL Query Builder

```rust
pub struct AqlBuilder {
    parts: Vec<String>,
    bind_vars: HashMap<String, serde_json::Value>,
    var_counter: usize,
}

impl AqlBuilder {
    pub fn new() -> Self {
        Self {
            parts: Vec::new(),
            bind_vars: HashMap::new(),
            var_counter: 0,
        }
    }
    
    pub fn for_in(mut self, var: &str, collection: &str) -> Self {
        self.parts.push(format!("FOR {} IN {}", var, collection));
        self
    }
    
    pub fn filter(mut self, condition: &str) -> Self {
        self.parts.push(format!("FILTER {}", condition));
        self
    }
    
    pub fn filter_eq(mut self, field: &str, value: impl Into<serde_json::Value>) -> Self {
        let param = format!("var{}", self.var_counter);
        self.var_counter += 1;
        self.bind_vars.insert(param.clone(), value.into());
        self.parts.push(format!("FILTER {} == @{}", field, param));
        self
    }
    
    pub fn sort(mut self, field: &str, direction: SortDirection) -> Self {
        let dir = match direction {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        };
        self.parts.push(format!("SORT {} {}", field, dir));
        self
    }
    
    pub fn limit(mut self, count: usize) -> Self {
        self.parts.push(format!("LIMIT {}", count));
        self
    }
    
    pub fn limit_offset(mut self, offset: usize, count: usize) -> Self {
        self.parts.push(format!("LIMIT {}, {}", offset, count));
        self
    }
    
    pub fn return_expr(mut self, expr: &str) -> Self {
        self.parts.push(format!("RETURN {}", expr));
        self
    }
    
    pub fn build(self) -> (String, HashMap<String, serde_json::Value>) {
        (self.parts.join("\n"), self.bind_vars)
    }
}

pub enum SortDirection {
    Asc,
    Desc,
}

// Usage
let (query, bind_vars) = AqlBuilder::new()
    .for_in("doc", "users")
    .filter_eq("doc.status", "active")
    .sort("doc.created", SortDirection::Desc)
    .limit(10)
    .return_expr("doc")
    .build();
```

### Lexer Implementation Skeleton

```rust
pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
    line: u32,
    column: u32,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            line: 1,
            column: 1,
        }
    }
    
    pub fn next_token(&mut self) -> Result<Token, LexerError> {
        self.skip_whitespace_and_comments();
        
        if self.pos >= self.input.len() {
            return Ok(Token::Eof);
        }
        
        let c = self.current_char();
        
        match c {
            // Single character tokens
            '(' => self.single_char_token(Token::LParen),
            ')' => self.single_char_token(Token::RParen),
            '[' => self.single_char_token(Token::LBracket),
            ']' => self.single_char_token(Token::RBracket),
            '{' => self.single_char_token(Token::LBrace),
            '}' => self.single_char_token(Token::RBrace),
            ',' => self.single_char_token(Token::Comma),
            ':' => self.single_char_token(Token::Colon),
            '+' => self.single_char_token(Token::Plus),
            '-' => self.single_char_token(Token::Minus),
            '*' => self.single_char_token(Token::Star),
            '/' => self.single_char_token(Token::Slash),
            '%' => self.single_char_token(Token::Percent),
            '?' => self.scan_question(),
            
            // Multi-character tokens
            '.' => self.scan_dot(),
            '=' => self.scan_equals(),
            '!' => self.scan_bang(),
            '<' => self.scan_less_than(),
            '>' => self.scan_greater_than(),
            '&' => self.scan_ampersand(),
            '|' => self.scan_pipe(),
            '@' => self.scan_bind_parameter(),
            
            // Strings
            '"' | '\'' => self.scan_string(),
            
            // Numbers
            '0'..='9' => self.scan_number(),
            
            // Identifiers and keywords
            'a'..='z' | 'A'..='Z' | '_' | '$' => self.scan_identifier(),
            
            // Backtick-quoted identifiers
            '`' | '´' => self.scan_quoted_identifier(),
            
            _ => Err(LexerError::UnexpectedChar(c, self.line, self.column)),
        }
    }
    
    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip whitespace
            while self.pos < self.input.len() && self.current_char().is_whitespace() {
                if self.current_char() == '\n' {
                    self.line += 1;
                    self.column = 1;
                } else {
                    self.column += 1;
                }
                self.pos += 1;
            }
            
            // Skip single-line comments
            if self.matches("//") {
                while self.pos < self.input.len() && self.current_char() != '\n' {
                    self.pos += 1;
                }
                continue;
            }
            
            // Skip multi-line comments
            if self.matches("/*") {
                self.pos += 2;
                while self.pos < self.input.len() - 1 {
                    if self.matches("*/") {
                        self.pos += 2;
                        break;
                    }
                    if self.current_char() == '\n' {
                        self.line += 1;
                        self.column = 1;
                    }
                    self.pos += 1;
                }
                continue;
            }
            
            break;
        }
    }
    
    fn scan_identifier(&mut self) -> Result<Token, LexerError> {
        let start = self.pos;
        
        while self.pos < self.input.len() {
            let c = self.current_char();
            if c.is_alphanumeric() || c == '_' || c == '$' {
                self.advance();
            } else {
                break;
            }
        }
        
        let text = &self.input[start..self.pos];
        
        // Check for keywords (case-insensitive)
        match text.to_uppercase().as_str() {
            "FOR" => Ok(Token::For),
            "IN" => Ok(Token::In),
            "RETURN" => Ok(Token::Return),
            "FILTER" => Ok(Token::Filter),
            "SORT" => Ok(Token::Sort),
            "LIMIT" => Ok(Token::Limit),
            "LET" => Ok(Token::Let),
            "COLLECT" => Ok(Token::Collect),
            "INSERT" => Ok(Token::Insert),
            "UPDATE" => Ok(Token::Update),
            "REPLACE" => Ok(Token::Replace),
            "REMOVE" => Ok(Token::Remove),
            "UPSERT" => Ok(Token::Upsert),
            "WITH" => Ok(Token::With),
            "INTO" => Ok(Token::Into),
            "AND" => Ok(Token::And),
            "OR" => Ok(Token::Or),
            "NOT" => Ok(Token::Not),
            "TRUE" => Ok(Token::True),
            "FALSE" => Ok(Token::False),
            "NULL" => Ok(Token::Null),
            "ASC" => Ok(Token::Asc),
            "DESC" => Ok(Token::Desc),
            "DISTINCT" => Ok(Token::Distinct),
            "GRAPH" => Ok(Token::Graph),
            "OUTBOUND" => Ok(Token::Outbound),
            "INBOUND" => Ok(Token::Inbound),
            "ANY" => Ok(Token::Any),
            "ALL" => Ok(Token::All),
            "NONE" => Ok(Token::None_),
            "LIKE" => Ok(Token::Like),
            "OPTIONS" => Ok(Token::Options),
            "PRUNE" => Ok(Token::Prune),
            "SEARCH" => Ok(Token::Search),
            "SHORTEST_PATH" => Ok(Token::ShortestPath),
            "K_SHORTEST_PATHS" => Ok(Token::KShortestPaths),
            "ALL_SHORTEST_PATHS" => Ok(Token::AllShortestPaths),
            "TO" => Ok(Token::To),
            "WINDOW" => Ok(Token::Window),
            "AGGREGATE" => Ok(Token::Aggregate),
            _ => Ok(Token::Identifier(text.to_string())),
        }
    }
    
    // ... other scanning methods
}
```

### Using arangors Crate

```rust
use arangors::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct User {
    #[serde(rename = "_key")]
    key: String,
    name: String,
    age: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect
    let conn = Connection::establish_jwt(
        "http://localhost:8529",
        "root",
        "password"
    ).await?;
    
    let db = conn.db("test").await?;
    
    // Simple query
    let users: Vec<User> = db
        .aql_str("FOR u IN users RETURN u")
        .await?;
    
    // Query with bind variables
    use std::collections::HashMap;
    let mut vars = HashMap::new();
    vars.insert("minAge", serde_json::json!(18));
    
    let adults: Vec<User> = db
        .aql_bind_vars(
            "FOR u IN users FILTER u.age >= @minAge RETURN u",
            vars
        )
        .await?;
    
    // Full query options
    use arangors::AqlQuery;
    
    let query = AqlQuery::builder()
        .query("FOR u IN users FILTER u.age >= @minAge SORT u.name LIMIT @limit RETURN u")
        .bind_var("minAge", 18)
        .bind_var("limit", 10)
        .batch_size(100)
        .count(true)
        .build();
    
    let result: Vec<User> = db.aql_query(query).await?;
    
    Ok(())
}
```

---

## Query Examples

### Basic CRUD

```aql
// Create
INSERT { name: "John", age: 30, email: "john@example.com" }
INTO users
RETURN NEW

// Read
FOR doc IN users
  FILTER doc.age >= 18
  RETURN doc

// Update
FOR doc IN users
  FILTER doc.email == "john@example.com"
  UPDATE doc WITH { verified: true }
  IN users
  RETURN NEW

// Delete
FOR doc IN users
  FILTER doc.status == "deleted"
  REMOVE doc IN users
  RETURN OLD
```

### Aggregations

```aql
// Count by group
FOR doc IN orders
  COLLECT status = doc.status WITH COUNT INTO count
  RETURN { status, count }

// Multiple aggregates
FOR doc IN orders
  COLLECT customerId = doc.customerId
  AGGREGATE 
    totalOrders = COUNT(1),
    totalAmount = SUM(doc.amount),
    avgAmount = AVG(doc.amount)
  RETURN { customerId, totalOrders, totalAmount, avgAmount }
```

### Joins

```aql
// Inner join
FOR user IN users
  FOR order IN orders
    FILTER order.userId == user._key
    RETURN { user: user.name, order: order }

// Left join with subquery
FOR user IN users
  LET userOrders = (
    FOR order IN orders
      FILTER order.userId == user._key
      RETURN order
  )
  RETURN { user, orders: userOrders }
```

### Graph Traversal

```aql
// Find friends of friends
FOR friend IN 2..2 OUTBOUND @userId GRAPH 'social'
  RETURN DISTINCT friend

// Shortest path with edge data
FOR v, e IN OUTBOUND SHORTEST_PATH @start TO @end GRAPH 'routes'
  RETURN { vertex: v.name, edge: e }

// Pattern matching
FOR v, e, p IN 1..5 ANY @start GRAPH 'network'
  FILTER p.edges[*].type ALL == "connection"
  FILTER v.type == "server"
  RETURN v
```

### Subqueries

```aql
// Correlated subquery
FOR user IN users
  LET recentOrders = (
    FOR order IN orders
      FILTER order.userId == user._key
      SORT order.date DESC
      LIMIT 5
      RETURN order
  )
  FILTER LENGTH(recentOrders) > 0
  RETURN { user, recentOrders }

// Non-correlated subquery
LET activeProducts = (
  FOR p IN products
    FILTER p.active == true
    RETURN p._key
)
FOR order IN orders
  FILTER order.productId IN activeProducts
  RETURN order
```

### Window Functions

```aql
// Running total
FOR doc IN sales
  SORT doc.date
  WINDOW { preceding: "unbounded", following: 0 }
  AGGREGATE runningTotal = SUM(doc.amount)
  RETURN { date: doc.date, amount: doc.amount, runningTotal }

// Moving average
FOR doc IN measurements
  SORT doc.timestamp
  WINDOW { preceding: 5, following: 0 }
  AGGREGATE movingAvg = AVG(doc.value)
  RETURN { timestamp: doc.timestamp, value: doc.value, movingAvg }
```

---

## Error Handling

### Common Error Codes

| Code | Name | Description |
|------|------|-------------|
| 400 | Bad Request | Malformed request |
| 404 | Not Found | Document/collection not found |
| 409 | Conflict | Unique constraint violation |
| 1200 | ERROR_ARANGO_CONFLICT | Write-write conflict |
| 1202 | ERROR_ARANGO_DOCUMENT_NOT_FOUND | Document not found |
| 1203 | ERROR_ARANGO_DATA_SOURCE_NOT_FOUND | Collection not found |
| 1501 | ERROR_QUERY_KILLED | Query was killed |
| 1502 | ERROR_QUERY_PARSE | Parse error |
| 1503 | ERROR_QUERY_EMPTY | Empty query |
| 1510 | ERROR_QUERY_NUMBER_OUT_OF_RANGE | Number out of range |
| 1521 | ERROR_QUERY_FUNCTION_NAME_UNKNOWN | Unknown function |
| 1541 | ERROR_QUERY_BIND_PARAMETER_MISSING | Missing bind parameter |

### Rust Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum AqlError {
    #[error("Parse error at line {line}, column {column}: {message}")]
    ParseError {
        line: u32,
        column: u32,
        message: String,
    },
    
    #[error("Unknown function: {0}")]
    UnknownFunction(String),
    
    #[error("Missing bind parameter: {0}")]
    MissingBindParameter(String),
    
    #[error("Type error: expected {expected}, got {got}")]
    TypeError {
        expected: String,
        got: String,
    },
    
    #[error("Collection not found: {0}")]
    CollectionNotFound(String),
    
    #[error("Document not found: {0}")]
    DocumentNotFound(String),
    
    #[error("Query timeout")]
    Timeout,
    
    #[error("Memory limit exceeded")]
    MemoryLimitExceeded,
    
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
}
```

---

## Performance Tips

1. **Use bind parameters** to enable query caching and prevent injection
2. **Add indexes** for frequently filtered/sorted attributes
3. **Use LIMIT early** in the query when possible
4. **Prefer `FILTER` over post-processing** in application code
5. **Use projection** in RETURN to reduce data transfer
6. **Batch operations** with FOR loops instead of individual requests
7. **Use `stream: true`** for large result sets to reduce memory
8. **Profile queries** with `profile: 2` option to find bottlenecks

---

## References

- [ArangoDB Official Documentation](https://docs.arangodb.com/)
- [AQL Documentation](https://docs.arangodb.com/stable/aql/)
- [arangors Rust Driver](https://github.com/fMeow/arangors)
- [VelocyPack Specification](https://github.com/arangodb/velocypack)
- [HTTP API Reference](https://docs.arangodb.com/stable/develop/http-api/)
