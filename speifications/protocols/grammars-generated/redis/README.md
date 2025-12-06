# Redis RESP Protocol ANTLR4 Grammar

A comprehensive ANTLR4 grammar for parsing the Redis Serialization Protocol (RESP), including support for RESP2, RESP3, and all Redis Stack modules.

## Overview

This project provides ANTLR4 grammar files for:

1. **RESP Protocol** - The wire protocol used by Redis for client-server communication
2. **Redis Stack Modules** - Extended commands for RediSearch, RedisJSON, RedisTimeSeries, RedisBloom, RedisGraph, and RedisGears

## Files

| File | Description |
|------|-------------|
| `RESP.g4` | Combined lexer/parser grammar for RESP2/RESP3 protocol |
| `RESPLexer.g4` | Separate lexer grammar with modal tokenization |
| `RESPParser.g4` | Separate parser grammar for RESP protocol |
| `RedisModules.g4` | Grammar for Redis Stack module commands |

## RESP Protocol Support

### RESP2 Types (Redis 2.0+)

| Type | First Byte | Example |
|------|------------|---------|
| Simple String | `+` | `+OK\r\n` |
| Simple Error | `-` | `-ERR unknown command\r\n` |
| Integer | `:` | `:1000\r\n` |
| Bulk String | `$` | `$5\r\nhello\r\n` |
| Array | `*` | `*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n` |

### RESP3 Types (Redis 6.0+)

| Type | First Byte | Example |
|------|------------|---------|
| Null | `_` | `_\r\n` |
| Boolean | `#` | `#t\r\n` or `#f\r\n` |
| Double | `,` | `,1.23\r\n` |
| Big Number | `(` | `(3492890328409238509324850943825024385\r\n` |
| Bulk Error | `!` | `!21\r\nSYNTAX invalid syntax\r\n` |
| Verbatim String | `=` | `=15\r\ntxt:Some string\r\n` |
| Map | `%` | `%2\r\n+first\r\n:1\r\n+second\r\n:2\r\n` |
| Attribute | `\|` | `\|1\r\n+key-popularity\r\n...` |
| Set | `~` | `~3\r\n:1\r\n:2\r\n:3\r\n` |
| Push | `>` | `>3\r\n$7\r\nmessage\r\n...` |

## Redis Stack Module Support

### RediSearch (FT.*)

Full-text search, secondary indexing, and query engine.

```
FT.CREATE idx ON HASH PREFIX 1 doc: SCHEMA title TEXT WEIGHT 5.0 body TEXT
FT.SEARCH idx "hello world" LIMIT 0 10
FT.AGGREGATE idx "*" GROUPBY 1 @category REDUCE COUNT 0 AS count
```

**Supported Commands:**
- `FT.CREATE` - Create an index
- `FT.SEARCH` - Search the index
- `FT.AGGREGATE` - Run aggregation queries
- `FT.ALTER` - Alter index schema
- `FT.DROPINDEX` - Delete an index
- `FT.INFO` - Get index info
- `FT.SUGADD/FT.SUGGET` - Auto-complete suggestions
- `FT.SPELLCHECK` - Spell checking
- And more...

### RedisJSON (JSON.*)

Native JSON data type support.

```
JSON.SET doc $ '{"name":"John","age":30}'
JSON.GET doc $.name
JSON.ARRAPPEND doc $.tags '"new"'
JSON.NUMINCRBY doc $.age 1
```

**Supported Commands:**
- `JSON.SET/JSON.GET` - Set/get JSON values
- `JSON.MSET/JSON.MGET` - Multi-key operations
- `JSON.ARRAPPEND/JSON.ARRINSERT` - Array operations
- `JSON.NUMINCRBY/JSON.NUMMULTBY` - Numeric operations
- `JSON.OBJKEYS/JSON.OBJLEN` - Object inspection
- And more...

### RedisTimeSeries (TS.*)

Time series data structure.

```
TS.CREATE temperature RETENTION 86400 LABELS sensor_id 1
TS.ADD temperature * 25.5
TS.RANGE temperature - + AGGREGATION avg 3600
TS.MRANGE - + FILTER sensor_id=1
```

**Supported Commands:**
- `TS.CREATE` - Create a time series
- `TS.ADD/TS.MADD` - Add samples
- `TS.INCRBY/TS.DECRBY` - Increment/decrement
- `TS.RANGE/TS.REVRANGE` - Query ranges
- `TS.MRANGE/TS.MREVRANGE` - Multi-key range queries
- `TS.CREATERULE` - Create compaction rules
- And more...

### RedisBloom

Probabilistic data structures.

#### Bloom Filter (BF.*)
```
BF.RESERVE filter 0.01 1000
BF.ADD filter item1
BF.EXISTS filter item1
```

#### Cuckoo Filter (CF.*)
```
CF.RESERVE filter 1000
CF.ADD filter item1
CF.COUNT filter item1
```

#### Count-Min Sketch (CMS.*)
```
CMS.INITBYPROB sketch 0.001 0.01
CMS.INCRBY sketch item1 5
CMS.QUERY sketch item1
```

#### Top-K (TOPK.*)
```
TOPK.RESERVE topk 10
TOPK.ADD topk item1 item2
TOPK.LIST topk
```

#### T-Digest (TDIGEST.*)
```
TDIGEST.CREATE td
TDIGEST.ADD td 1 2 3 4 5
TDIGEST.QUANTILE td 0.5 0.95
```

### RedisGraph (GRAPH.*)

Graph database with Cypher query language.

```
GRAPH.QUERY social "CREATE (:Person {name:'John'})-[:KNOWS]->(:Person {name:'Jane'})"
GRAPH.QUERY social "MATCH (p:Person) RETURN p.name"
GRAPH.RO_QUERY social "MATCH (p:Person) WHERE p.name = 'John' RETURN p"
```

**Supported Commands:**
- `GRAPH.QUERY` - Execute Cypher queries
- `GRAPH.RO_QUERY` - Read-only queries
- `GRAPH.EXPLAIN` - Query execution plan
- `GRAPH.PROFILE` - Query profiling
- `GRAPH.DELETE` - Delete a graph
- `GRAPH.CONFIG` - Configuration
- `GRAPH.CONSTRAINT` - Manage constraints

### RedisGears

Serverless engine for data processing.

```
TFUNCTION LOAD REPLACE "#!js name=mylib\n redis.registerFunction('hello', () => 'world')"
TFCALL mylib.hello 0
```

**Supported Commands:**
- `TFUNCTION LOAD` - Load a library
- `TFUNCTION DELETE` - Delete a library
- `TFUNCTION LIST` - List libraries
- `TFCALL` - Call a function
- `TFCALLASYNC` - Async function call

## Usage

### Prerequisites

- [ANTLR4](https://www.antlr.org/) (version 4.9+)
- Java Runtime Environment (JRE) 8+

### Generating Parsers

#### Java
```bash
antlr4 RESP.g4
antlr4 RedisModules.g4
javac *.java
```

#### Python
```bash
antlr4 -Dlanguage=Python3 RESP.g4
antlr4 -Dlanguage=Python3 RedisModules.g4
```

#### JavaScript/TypeScript
```bash
antlr4 -Dlanguage=JavaScript RESP.g4
antlr4 -Dlanguage=TypeScript RESP.g4
```

#### C++
```bash
antlr4 -Dlanguage=Cpp RESP.g4
```

#### Go
```bash
antlr4 -Dlanguage=Go RESP.g4
```

#### Rust (with antlr4rust)
```bash
antlr4 -Dlanguage=Rust RESP.g4
```

### Example: Parsing RESP in Java

```java
import org.antlr.v4.runtime.*;
import org.antlr.v4.runtime.tree.*;

public class RESPParserExample {
    public static void main(String[] args) {
        String input = "*2\r\n$3\r\nGET\r\n$3\r\nkey\r\n";
        
        CharStream charStream = CharStreams.fromString(input);
        RESPLexer lexer = new RESPLexer(charStream);
        CommonTokenStream tokens = new CommonTokenStream(lexer);
        RESPParser parser = new RESPParser(tokens);
        
        ParseTree tree = parser.respMessage();
        System.out.println(tree.toStringTree(parser));
    }
}
```

### Example: Parsing RESP in Python

```python
from antlr4 import *
from RESPLexer import RESPLexer
from RESPParser import RESPParser

def parse_resp(input_str):
    input_stream = InputStream(input_str)
    lexer = RESPLexer(input_stream)
    stream = CommonTokenStream(lexer)
    parser = RESPParser(stream)
    tree = parser.respMessage()
    return tree

# Parse a simple RESP message
tree = parse_resp("*2\r\n$3\r\nGET\r\n$3\r\nkey\r\n")
print(tree.toStringTree(recog=parser))
```

### Example: Using Visitor Pattern

```java
public class RESPValueVisitor extends RESPParserBaseVisitor<Object> {
    @Override
    public Object visitSimpleString(RESPParser.SimpleStringContext ctx) {
        return ctx.simpleStringContent().getText();
    }
    
    @Override
    public Object visitInteger(RESPParser.IntegerContext ctx) {
        return Long.parseLong(ctx.integerValue().getText());
    }
    
    @Override
    public Object visitBulkString(RESPParser.BulkStringContext ctx) {
        int length = Integer.parseInt(ctx.bulkLength().getText());
        if (length < 0) return null; // Null bulk string
        return ctx.bulkData().bulkDataContent().getText();
    }
    
    @Override
    public Object visitArray(RESPParser.ArrayContext ctx) {
        int length = Integer.parseInt(ctx.arrayLength().getText());
        if (length < 0) return null; // Null array
        
        List<Object> result = new ArrayList<>();
        if (ctx.arrayElements() != null) {
            for (var elem : ctx.arrayElements().respValue()) {
                result.add(visit(elem));
            }
        }
        return result;
    }
    
    // RESP3 types
    @Override
    public Object visitNull_(RESPParser.Null_Context ctx) {
        return null;
    }
    
    @Override
    public Object visitBoolean_(RESPParser.Boolean_Context ctx) {
        return ctx.booleanValue().BOOLEAN_TRUE() != null;
    }
    
    @Override
    public Object visitDouble_(RESPParser.Double_Context ctx) {
        String text = ctx.doubleValue().getText();
        if (text.equals("inf")) return Double.POSITIVE_INFINITY;
        if (text.equals("-inf")) return Double.NEGATIVE_INFINITY;
        if (text.equals("nan")) return Double.NaN;
        return Double.parseDouble(text);
    }
}
```

## Protocol Notes

### Binary Safety

RESP bulk strings are binary-safe. The grammar handles this by:
1. Reading the length prefix
2. Reading exactly that many bytes
3. Expecting CRLF terminator

When implementing a parser, you'll need to handle the binary data separately from the grammar-based parsing.

### Null Values

- **RESP2**: Null is represented as `$-1\r\n` (null bulk string) or `*-1\r\n` (null array)
- **RESP3**: Null has its own type: `_\r\n`

### Inline Commands

Redis supports inline commands for simple telnet sessions:
```
PING
SET key value
```

These are parsed as space-separated arguments without RESP framing.

### Push Data (RESP3)

RESP3 push data can appear at any time in a connection:
```
>3\r\n
$7\r\nmessage\r\n
$7\r\nchannel\r\n
$12\r\nHello World!\r\n
```

## References

- [Redis Protocol Specification](https://redis.io/docs/latest/develop/reference/protocol-spec/)
- [RESP3 Specification](https://github.com/redis/redis-specifications/blob/master/protocol/RESP3.md)
- [RediSearch Documentation](https://redis.io/docs/stack/search/)
- [RedisJSON Documentation](https://redis.io/docs/stack/json/)
- [RedisTimeSeries Documentation](https://redis.io/docs/stack/timeseries/)
- [RedisBloom Documentation](https://redis.io/docs/stack/bloom/)
- [RedisGraph Documentation](https://redis.io/docs/stack/graph/)
- [RedisGears Documentation](https://redis.io/docs/stack/gears/)
- [ANTLR4 Documentation](https://github.com/antlr/antlr4/blob/master/doc/index.md)

## License

MIT License

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

## Version History

- **1.0.0** - Initial release with RESP2, RESP3, and Redis Stack module support
