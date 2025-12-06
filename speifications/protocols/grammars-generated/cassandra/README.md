# CQL ANTLR4 Grammar for Apache Cassandra 5.0 and ScyllaDB

A complete ANTLR4 grammar implementation for the Cassandra Query Language (CQL), supporting both **Apache Cassandra 5.0** and **ScyllaDB** extensions.

## Overview

This project provides ANTLR4 lexer and parser grammar files for CQL, enabling:
- Parsing CQL statements into Abstract Syntax Trees (AST)
- Building custom CQL tools (formatters, validators, analyzers)
- IDE integration for syntax highlighting and auto-completion
- Migration tools and query analyzers
- Educational purposes and documentation

## Features

### Apache Cassandra Support

- **CQL 3.4.x** - Full support for standard CQL syntax
- **Cassandra 5.0 Features**:
  - Vector data type and vector literals
  - Vector search with ANN (Approximate Nearest Neighbor)
  - Storage-Attached Indexes (SAI)
  - Dynamic Data Masking
  - New math functions (abs, exp, log, log10, round)
  - Vector similarity functions (cosine, dot_product, euclidean)

### ScyllaDB Extensions

- `BYPASS CACHE` clause for SELECT statements
- `USING TIMEOUT` for per-query timeouts
- `PRUNE MATERIALIZED VIEW` statement
- `SYNCHRONOUS_UPDATES` for materialized views
- `REDUCEFUNC` for User-Defined Aggregates
- `PAXOS_GRACE_SECONDS` table option
- `PER_PARTITION_RATE_LIMIT` table option
- Service Level management statements
- `DESCRIBE SCHEMA WITH INTERNALS [AND PASSWORDS]`
- S3 storage options for keyspaces
- Internal functions (SCYLLA_TIMEUUID_LIST_INDEX, etc.)

## File Structure

```
cql-antlr4/
├── CqlLexer.g4      # Lexer grammar - token definitions
├── CqlParser.g4     # Parser grammar - syntax rules
├── README.md        # This documentation
├── examples/        # Example CQL files for testing
│   ├── cassandra_5_features.cql
│   ├── scylladb_extensions.cql
│   └── basic_cql.cql
└── generated/       # Generated parser files (after running ANTLR)
```

## Requirements

- **ANTLR 4.x** (4.9+ recommended)
- **Java 8+** (for ANTLR tool)
- Target language runtime (Java, Python, C#, JavaScript, Go, C++, etc.)

## Installation

### 1. Install ANTLR4

**macOS (Homebrew):**
```bash
brew install antlr
```

**Linux (Ubuntu/Debian):**
```bash
sudo apt-get install antlr4
```

**Manual Installation:**
```bash
cd /usr/local/lib
curl -O https://www.antlr.org/download/antlr-4.13.1-complete.jar
export CLASSPATH=".:/usr/local/lib/antlr-4.13.1-complete.jar:$CLASSPATH"
alias antlr4='java -jar /usr/local/lib/antlr-4.13.1-complete.jar'
alias grun='java org.antlr.v4.gui.TestRig'
```

### 2. Generate Parser Code

**For Java:**
```bash
antlr4 -Dlanguage=Java -visitor -listener CqlLexer.g4 CqlParser.g4
javac *.java
```

**For Python:**
```bash
antlr4 -Dlanguage=Python3 -visitor -listener CqlLexer.g4 CqlParser.g4
```

**For JavaScript/TypeScript:**
```bash
antlr4 -Dlanguage=JavaScript -visitor -listener CqlLexer.g4 CqlParser.g4
```

**For C#:**
```bash
antlr4 -Dlanguage=CSharp -visitor -listener CqlLexer.g4 CqlParser.g4
```

**For Go:**
```bash
antlr4 -Dlanguage=Go -visitor -listener -package cql CqlLexer.g4 CqlParser.g4
```

**For C++:**
```bash
antlr4 -Dlanguage=Cpp -visitor -listener CqlLexer.g4 CqlParser.g4
```

## Usage

### Testing with ANTLR TestRig (grun)

```bash
# Test a single statement
echo "SELECT * FROM users WHERE id = 123;" | grun Cql root -tree

# Test with GUI visualization
echo "CREATE TABLE test (id int PRIMARY KEY, name text);" | grun Cql root -gui

# Test from file
grun Cql root -tree examples/basic_cql.cql
```

### Java Example

```java
import org.antlr.v4.runtime.*;
import org.antlr.v4.runtime.tree.*;

public class CqlParserExample {
    public static void main(String[] args) {
        String cql = "SELECT * FROM users WHERE id = ?;";
        
        CharStream input = CharStreams.fromString(cql);
        CqlLexer lexer = new CqlLexer(input);
        CommonTokenStream tokens = new CommonTokenStream(lexer);
        CqlParser parser = new CqlParser(tokens);
        
        ParseTree tree = parser.root();
        System.out.println(tree.toStringTree(parser));
    }
}
```

### Python Example

```python
from antlr4 import *
from CqlLexer import CqlLexer
from CqlParser import CqlParser

def parse_cql(statement):
    input_stream = InputStream(statement)
    lexer = CqlLexer(input_stream)
    stream = CommonTokenStream(lexer)
    parser = CqlParser(stream)
    tree = parser.root()
    return tree

# Parse a CQL statement
tree = parse_cql("SELECT * FROM keyspace.table WHERE id = 1;")
print(tree.toStringTree(recog=parser))
```

### JavaScript Example

```javascript
const antlr4 = require('antlr4');
const CqlLexer = require('./CqlLexer').CqlLexer;
const CqlParser = require('./CqlParser').CqlParser;

function parseCql(statement) {
    const chars = new antlr4.InputStream(statement);
    const lexer = new CqlLexer(chars);
    const tokens = new antlr4.CommonTokenStream(lexer);
    const parser = new CqlParser(tokens);
    return parser.root();
}

const tree = parseCql("INSERT INTO users (id, name) VALUES (1, 'Alice');");
console.log(tree.toStringTree(null, parser));
```

## Grammar Structure

### Lexer (CqlLexer.g4)

The lexer defines all tokens including:
- **Keywords**: Reserved and unreserved CQL keywords
- **Operators**: Comparison, arithmetic, and special operators
- **Literals**: Strings, numbers, UUIDs, blobs, durations, vectors
- **Identifiers**: Regular and quoted identifiers
- **Comments**: Single-line (`--`, `//`) and multi-line (`/* */`)

### Parser (CqlParser.g4)

The parser defines the syntax rules organized into:

1. **DDL Statements**
   - USE, CREATE/ALTER/DROP KEYSPACE
   - CREATE/ALTER/DROP TABLE
   - TRUNCATE, DESCRIBE

2. **DML Statements**
   - SELECT (with all clauses including vector search)
   - INSERT (including JSON)
   - UPDATE
   - DELETE
   - BATCH

3. **Secondary Index Statements**
   - CREATE/DROP INDEX

4. **Materialized View Statements**
   - CREATE/ALTER/DROP MATERIALIZED VIEW
   - PRUNE MATERIALIZED VIEW (ScyllaDB)

5. **Role & Permission Statements**
   - CREATE/ALTER/DROP ROLE/USER
   - GRANT/REVOKE permissions

6. **UDF/UDA Statements**
   - CREATE/DROP FUNCTION
   - CREATE/DROP AGGREGATE

7. **UDT Statements**
   - CREATE/ALTER/DROP TYPE

8. **Trigger Statements**
   - CREATE/DROP TRIGGER

9. **Service Level Statements** (ScyllaDB)
   - CREATE/ALTER/DROP SERVICE LEVEL
   - ATTACH/DETACH SERVICE LEVEL

## Supported CQL Statements

### Data Definition Language (DDL)

| Statement | Cassandra | ScyllaDB | Notes |
|-----------|-----------|----------|-------|
| USE | ✅ | ✅ | |
| CREATE KEYSPACE | ✅ | ✅ | ScyllaDB adds STORAGE option |
| ALTER KEYSPACE | ✅ | ✅ | |
| DROP KEYSPACE | ✅ | ✅ | |
| CREATE TABLE | ✅ | ✅ | Cassandra 5.0 adds MASKED |
| ALTER TABLE | ✅ | ✅ | |
| DROP TABLE | ✅ | ✅ | |
| TRUNCATE | ✅ | ✅ | ScyllaDB adds USING TIMEOUT |
| DESCRIBE | ✅ | ✅ | ScyllaDB adds WITH INTERNALS |

### Data Manipulation Language (DML)

| Statement | Cassandra | ScyllaDB | Notes |
|-----------|-----------|----------|-------|
| SELECT | ✅ | ✅ | Vector search, BYPASS CACHE |
| INSERT | ✅ | ✅ | JSON support |
| UPDATE | ✅ | ✅ | LWT support |
| DELETE | ✅ | ✅ | LWT support |
| BATCH | ✅ | ✅ | |

### Index & View Management

| Statement | Cassandra | ScyllaDB | Notes |
|-----------|-----------|----------|-------|
| CREATE INDEX | ✅ | ✅ | SAI support |
| DROP INDEX | ✅ | ✅ | |
| CREATE MATERIALIZED VIEW | ✅ | ✅ | SYNCHRONOUS_UPDATES |
| ALTER MATERIALIZED VIEW | ✅ | ✅ | |
| DROP MATERIALIZED VIEW | ✅ | ✅ | |
| PRUNE MATERIALIZED VIEW | ❌ | ✅ | ScyllaDB only |

### Security & Access Control

| Statement | Cassandra | ScyllaDB | Notes |
|-----------|-----------|----------|-------|
| CREATE/ALTER/DROP ROLE | ✅ | ✅ | |
| GRANT/REVOKE | ✅ | ✅ | Cassandra 5.0 adds UNMASK |
| LIST ROLES/PERMISSIONS | ✅ | ✅ | |
| CREATE/ALTER/DROP USER | ✅ | ✅ | Legacy |

### User-Defined Functions & Types

| Statement | Cassandra | ScyllaDB | Notes |
|-----------|-----------|----------|-------|
| CREATE/DROP FUNCTION | ✅ | ✅ | |
| CREATE/DROP AGGREGATE | ✅ | ✅ | REDUCEFUNC (ScyllaDB) |
| CREATE/ALTER/DROP TYPE | ✅ | ✅ | |

### ScyllaDB Service Levels

| Statement | Cassandra | ScyllaDB |
|-----------|-----------|----------|
| CREATE SERVICE LEVEL | ❌ | ✅ |
| ALTER SERVICE LEVEL | ❌ | ✅ |
| DROP SERVICE LEVEL | ❌ | ✅ |
| ATTACH SERVICE LEVEL | ❌ | ✅ |
| DETACH SERVICE LEVEL | ❌ | ✅ |
| LIST SERVICE LEVELS | ❌ | ✅ |
| LIST EFFECTIVE SERVICE LEVEL | ❌ | ✅ |

## Data Types

### Native Types

| Type | Description |
|------|-------------|
| `ascii` | ASCII character string |
| `bigint` | 64-bit signed integer |
| `blob` | Arbitrary bytes |
| `boolean` | true or false |
| `counter` | 64-bit counter |
| `date` | Date without time |
| `decimal` | Variable-precision decimal |
| `double` | 64-bit floating point |
| `duration` | Duration (months, days, nanoseconds) |
| `float` | 32-bit floating point |
| `inet` | IP address (IPv4 or IPv6) |
| `int` | 32-bit signed integer |
| `smallint` | 16-bit signed integer |
| `text` | UTF-8 encoded string |
| `time` | Time without date |
| `timestamp` | Date and time |
| `timeuuid` | Type 1 UUID |
| `tinyint` | 8-bit signed integer |
| `uuid` | UUID |
| `varchar` | Alias for text |
| `varint` | Arbitrary-precision integer |

### Collection Types

- `list<T>` - Ordered collection
- `set<T>` - Unordered unique collection
- `map<K, V>` - Key-value pairs

### Other Types

- `tuple<T1, T2, ...>` - Fixed-length tuple
- `frozen<T>` - Immutable collection/UDT
- `vector<T, N>` - Fixed-dimension vector (Cassandra 5.0)

## Cassandra 5.0 Vector Search Examples

### Creating a Table with Vector Column

```sql
CREATE TABLE products (
    id uuid PRIMARY KEY,
    name text,
    description text,
    embedding vector<float, 1536>
);
```

### Creating a Vector Index

```sql
CREATE CUSTOM INDEX ON products(embedding) 
USING 'StorageAttachedIndex'
WITH OPTIONS = {'similarity_function': 'cosine'};
```

### Querying with Vector Search

```sql
SELECT name, description, similarity(embedding, [0.1, 0.2, ...]) as score
FROM products
ORDER BY embedding ANN OF [0.1, 0.2, ...]
LIMIT 10;
```

## ScyllaDB Extension Examples

### BYPASS CACHE

```sql
SELECT * FROM large_table 
WHERE partition_key = 'value' 
ALLOW FILTERING 
BYPASS CACHE;
```

### USING TIMEOUT

```sql
SELECT * FROM users USING TIMEOUT 500ms;
INSERT INTO data (id, value) VALUES (1, 'test') USING TIMEOUT 1s AND TTL 3600;
```

### PRUNE MATERIALIZED VIEW

```sql
PRUNE MATERIALIZED VIEW user_by_email;
PRUNE MATERIALIZED VIEW orders_by_date WHERE token(order_date) > 100;
```

### Service Levels

```sql
CREATE SERVICE LEVEL gold WITH timeout = 100ms AND workload_type = 'interactive';
ATTACH SERVICE LEVEL gold TO premium_users;
LIST EFFECTIVE SERVICE LEVEL OF john;
```

## Error Handling

The generated parser includes error handling. You can customize error recovery:

```java
parser.removeErrorListeners();
parser.addErrorListener(new BaseErrorListener() {
    @Override
    public void syntaxError(Recognizer<?, ?> recognizer,
                           Object offendingSymbol,
                           int line, int charPositionInLine,
                           String msg,
                           RecognitionException e) {
        throw new CqlSyntaxException(line, charPositionInLine, msg);
    }
});
```

## Known Limitations

1. **Semantic Validation**: The grammar only performs syntactic validation. Semantic checks (valid keyspace names, type compatibility, etc.) must be implemented separately.

2. **Version Differences**: Some features are only available in specific versions:
   - Vector search requires Cassandra 5.0+
   - Service levels are ScyllaDB-specific
   - Some ScyllaDB extensions may not work on Cassandra

3. **Custom Types**: The grammar accepts custom type names but doesn't validate Java class names.

4. **Comments in Strings**: Comments inside string literals are treated as literal content.

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Add tests for new features
4. Submit a pull request

### Testing Changes

```bash
# Run ANTLR to verify grammar compiles
antlr4 CqlLexer.g4 CqlParser.g4

# Test with sample CQL
grun Cql root -tree < examples/basic_cql.cql
```

## References

- [Apache Cassandra CQL Documentation](https://cassandra.apache.org/doc/latest/cassandra/cql/)
- [Cassandra 5.0 New Features](https://cassandra.apache.org/doc/trunk/cassandra/new/)
- [ScyllaDB CQL Reference](https://docs.scylladb.com/stable/cql/)
- [ScyllaDB CQL Extensions](https://enterprise.docs.scylladb.com/stable/cql/cql-extensions.html)
- [ANTLR4 Documentation](https://www.antlr.org/documentation.html)
- [ANTLR4 Grammars Repository](https://github.com/antlr/grammars-v4)

## License

This grammar is licensed under the Apache License 2.0, consistent with Apache Cassandra's licensing.

```
Copyright 2024 

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2024 | Initial release with Cassandra 5.0 and ScyllaDB support |

## Support

For issues and questions:
- Open an issue on GitHub
- Check the [Apache Cassandra mailing lists](https://cassandra.apache.org/community/)
- Visit the [ScyllaDB community forum](https://forum.scylladb.com/)
