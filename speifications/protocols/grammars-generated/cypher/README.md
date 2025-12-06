# Neo4j Cypher and Bolt Protocol ANTLR4 Grammars

Comprehensive ANTLR4 grammar files for parsing Neo4j Cypher query language and Bolt protocol messages.

## 📁 Files

| File | Description |
|------|-------------|
| `CypherLexer.g4` | Lexer grammar for Neo4j Cypher 5.x / Cypher 25 |
| `CypherParser.g4` | Parser grammar for Neo4j Cypher 5.x / Cypher 25 |
| `BoltPackStream.g4` | Combined grammar for Bolt Protocol message representation |

## 🚀 Quick Start

### Prerequisites

- Java 8+ (for ANTLR tool)
- ANTLR 4.13+ (`antlr4` command-line tool)

### Installation

```bash
# Download ANTLR (if not already installed)
curl -O https://www.antlr.org/download/antlr-4.13.2-complete.jar

# Set up aliases (add to ~/.bashrc or ~/.zshrc)
alias antlr4='java -jar /path/to/antlr-4.13.2-complete.jar'
alias grun='java org.antlr.v4.gui.TestRig'
```

### Generate Parsers

#### For Java:

```bash
# Generate Cypher lexer and parser
antlr4 -Dlanguage=Java CypherLexer.g4
antlr4 -Dlanguage=Java CypherParser.g4

# Generate Bolt/PackStream parser
antlr4 -Dlanguage=Java BoltPackStream.g4

# Compile
javac -cp ".:antlr-4.13.2-complete.jar" *.java
```

#### For Python:

```bash
antlr4 -Dlanguage=Python3 CypherLexer.g4
antlr4 -Dlanguage=Python3 CypherParser.g4
antlr4 -Dlanguage=Python3 BoltPackStream.g4
```

#### For Rust (via antlr4rust):

```bash
antlr4 -Dlanguage=Rust CypherLexer.g4
antlr4 -Dlanguage=Rust CypherParser.g4
antlr4 -Dlanguage=Rust BoltPackStream.g4
```

#### For Go:

```bash
antlr4 -Dlanguage=Go -package cypher CypherLexer.g4
antlr4 -Dlanguage=Go -package cypher CypherParser.g4
antlr4 -Dlanguage=Go -package bolt BoltPackStream.g4
```

### Test the Parser

```bash
# Test Cypher parsing with GUI
grun Cypher cypher -gui input.cypher

# Test with tokens output
grun Cypher cypher -tokens input.cypher

# Test Bolt message parsing
grun BoltPackStream boltStream -gui bolt_trace.txt
```

## 📖 Cypher Grammar

### Supported Features (Neo4j 5.x / Cypher 25)

#### Query Clauses
- `MATCH` / `OPTIONAL MATCH`
- `WHERE`
- `RETURN` / `WITH`
- `ORDER BY` / `SKIP` / `LIMIT`
- `CREATE` / `MERGE`
- `SET` / `REMOVE` / `DELETE`
- `UNWIND` / `FOREACH`
- `UNION` / `UNION ALL`
- `CALL` (procedures and subqueries)
- `LOAD CSV`

#### Pattern Matching
- Node patterns: `(n:Label {prop: value})`
- Relationship patterns: `-[r:TYPE*1..5]->`
- Path patterns and shortestPath
- Variable-length relationships
- Quantified path patterns (GQL)

#### Expressions
- Arithmetic, comparison, boolean operators
- List comprehensions: `[x IN list WHERE x > 0 | x * 2]`
- Pattern comprehensions
- CASE expressions
- EXISTS/COUNT subqueries
- Quantifiers: `ALL`, `ANY`, `NONE`, `SINGLE`

#### Schema & Administration
- Index management (CREATE/DROP/SHOW INDEX)
- Constraint management
- Database administration
- User/Role management
- Privilege management
- Server management

### Example Cypher Queries

```cypher
// Basic pattern matching
MATCH (p:Person)-[:KNOWS]->(friend:Person)
WHERE p.name = 'Alice'
RETURN friend.name, friend.age
ORDER BY friend.age DESC
LIMIT 10;

// Creating nodes and relationships
CREATE (n:Movie {title: 'The Matrix', year: 1999})
RETURN n;

// Complex pattern with variable-length path
MATCH path = (start:Person {name: 'Alice'})-[:FRIENDS*1..5]-(end:Person)
WHERE end.name = 'Bob'
RETURN path, length(path) AS hops;

// Aggregation and grouping
MATCH (p:Person)-[:ACTED_IN]->(m:Movie)
RETURN p.name, count(m) AS movies, collect(m.title) AS titles
ORDER BY movies DESC;

// Subquery
CALL {
  MATCH (p:Person)
  RETURN p ORDER BY p.age DESC LIMIT 1
}
RETURN p.name AS oldest;
```

## 📡 Bolt Protocol Grammar

### Overview

The Bolt Protocol is Neo4j's binary protocol for client-server communication. The `BoltPackStream.g4` grammar parses **textual representations** of Bolt messages (as seen in logs, documentation, and debugging).

> **Note:** The actual Bolt protocol uses binary PackStream encoding. This grammar is for parsing human-readable message representations.

### Supported Message Types

#### Request Messages (Client → Server)

| Message | Bolt Version | Description |
|---------|--------------|-------------|
| `HELLO` | 3.0+ | Initialize connection |
| `INIT` | 1.0-2.0 | Legacy initialization (replaced by HELLO) |
| `LOGON` | 5.1+ | Authentication |
| `LOGOFF` | 5.1+ | De-authentication |
| `GOODBYE` | 3.0+ | Graceful disconnect |
| `RESET` | 1.0+ | Reset connection state |
| `RUN` | 1.0+ | Execute Cypher query |
| `PULL` | 4.0+ | Fetch results (with options) |
| `PULL_ALL` | 1.0-3.0 | Fetch all results |
| `DISCARD` | 4.0+ | Discard results (with options) |
| `DISCARD_ALL` | 1.0-3.0 | Discard all results |
| `BEGIN` | 3.0+ | Start explicit transaction |
| `COMMIT` | 3.0+ | Commit transaction |
| `ROLLBACK` | 3.0+ | Rollback transaction |
| `ROUTE` | 4.3+ | Get routing table |
| `TELEMETRY` | 5.4+ | Send usage telemetry |

#### Response Messages (Server → Client)

| Message | Description |
|---------|-------------|
| `SUCCESS` | Request succeeded (with metadata) |
| `FAILURE` | Request failed (with error details) |
| `IGNORED` | Request was ignored |
| `RECORD` | Result record data |

### PackStream Data Types

| Type | Description | Example |
|------|-------------|---------|
| Null | Missing value | `null` |
| Boolean | True/False | `true`, `false` |
| Integer | Signed 64-bit | `42`, `-17`, `0x2A` |
| Float | 64-bit IEEE 754 | `3.14`, `1.0e-5`, `Infinity`, `NaN` |
| String | UTF-8 text | `"hello"`, `'world'` |
| Bytes | Byte array | `Bytes[00, FF, AB]` |
| List | Ordered collection | `[1, 2, "three"]` |
| Dictionary | Key-value map | `{"key": "value"}` |
| Structure | Typed composite | `Node(...)`, `Point2D(...)` |

### Structure Types

#### Graph Types

```
Node(id, labels, properties)
  - id: Integer
  - labels: List<String>
  - properties: Dictionary

Relationship(id, startNodeId, endNodeId, type, properties)
  - id: Integer
  - startNodeId: Integer
  - endNodeId: Integer
  - type: String
  - properties: Dictionary

Path(nodes, relationships, indices)
  - nodes: List<Node>
  - relationships: List<UnboundRelationship>
  - indices: List<Integer>
```

#### Temporal Types

```
Date(days)                           - Days since Unix epoch
Time(nanos, offsetSeconds)           - Time with timezone offset
LocalTime(nanos)                     - Time without timezone
DateTime(seconds, nanos, offset)     - DateTime with offset (Bolt 5.0+)
DateTimeZoneId(seconds, nanos, tz)   - DateTime with zone ID (Bolt 5.0+)
LocalDateTime(seconds, nanos)        - DateTime without timezone
Duration(months, days, seconds, nanos)
```

#### Spatial Types

```
Point2D(srid, x, y)                  - 2D point with SRID
Point3D(srid, x, y, z)               - 3D point with SRID
```

### Example Bolt Messages

```
// Client connects and authenticates
C: HELLO {"user_agent": "MyApp/1.0", "scheme": "basic", "principal": "neo4j", "credentials": "password"}
S: SUCCESS {"server": "Neo4j/5.15.0", "connection_id": "bolt-123"}

// Run a query
C: RUN "MATCH (n:Person) RETURN n.name LIMIT $limit" {"limit": 10} {"db": "neo4j"}
S: SUCCESS {"fields": ["n.name"], "t_first": 5}

// Pull results
C: PULL {"n": -1}
S: RECORD ["Alice"]
S: RECORD ["Bob"]
S: SUCCESS {"bookmark": "neo4j:bookmark:v1:tx42", "t_last": 2, "type": "r"}

// Transaction example
C: BEGIN {"db": "neo4j", "mode": "w"}
S: SUCCESS {}
C: RUN "CREATE (n:Test)" {} {}
S: SUCCESS {"fields": [], "qid": 0}
C: PULL {"n": -1, "qid": 0}
S: SUCCESS {"stats": {"nodes-created": 1}}
C: COMMIT
S: SUCCESS {"bookmark": "neo4j:bookmark:v1:tx43"}
```

## 🔧 Binary Protocol Details

### PackStream Markers

| Type | Marker Range | Description |
|------|--------------|-------------|
| TINY_INT | `0x00`-`0x7F`, `0xF0`-`0xFF` | -16 to +127 |
| INT_8 | `0xC8` | 8-bit signed integer |
| INT_16 | `0xC9` | 16-bit signed integer |
| INT_32 | `0xCA` | 32-bit signed integer |
| INT_64 | `0xCB` | 64-bit signed integer |
| FLOAT_64 | `0xC1` | 64-bit IEEE 754 float |
| NULL | `0xC0` | Null value |
| FALSE | `0xC2` | Boolean false |
| TRUE | `0xC3` | Boolean true |
| STRING_* | `0x80`-`0x8F`, `0xD0`-`0xD2` | UTF-8 strings |
| LIST_* | `0x90`-`0x9F`, `0xD4`-`0xD6` | Lists |
| MAP_* | `0xA0`-`0xAF`, `0xD8`-`0xDA` | Dictionaries |
| STRUCT_* | `0xB0`-`0xBF` | Structures (1-15 fields) |
| BYTES_* | `0xCC`-`0xCE` | Byte arrays |

### Message Structure Tags

| Tag | Message |
|-----|---------|
| `0x01` | HELLO / INIT |
| `0x02` | GOODBYE |
| `0x0E` | ACK_FAILURE |
| `0x0F` | RESET |
| `0x10` | RUN |
| `0x11` | BEGIN |
| `0x12` | COMMIT |
| `0x13` | ROLLBACK |
| `0x2F` | DISCARD |
| `0x3F` | PULL |
| `0x54` | TELEMETRY |
| `0x66` | ROUTE |
| `0x6A` | LOGON |
| `0x6B` | LOGOFF |
| `0x70` | SUCCESS |
| `0x71` | RECORD |
| `0x7E` | IGNORED |
| `0x7F` | FAILURE |

## 📚 References

### Cypher
- [Neo4j Cypher Manual](https://neo4j.com/docs/cypher-manual/current/)
- [openCypher Specification](https://opencypher.org/)
- [ANTLR grammars-v4 Cypher](https://github.com/antlr/grammars-v4/tree/master/cypher)

### Bolt Protocol
- [Bolt Protocol Documentation](https://neo4j.com/docs/bolt/current/)
- [PackStream Specification](https://neo4j.com/docs/bolt/current/packstream/)
- [Bolt Message Specification](https://neo4j.com/docs/bolt/current/bolt/message/)

## 📄 License

These grammar files are provided under the Apache License 2.0.

The Cypher query language specification is maintained by the openCypher project.
The Bolt Protocol is developed by Neo4j, Inc.

## 🤝 Contributing

Contributions are welcome! Please ensure any changes:

1. Follow ANTLR4 best practices
2. Include test cases for new features
3. Update documentation as needed
4. Maintain backward compatibility where possible

## ⚠️ Disclaimer

These grammars are community-maintained and may not cover all edge cases or the latest Neo4j features. For production use, please verify against the official Neo4j documentation and test thoroughly with your specific use cases.
