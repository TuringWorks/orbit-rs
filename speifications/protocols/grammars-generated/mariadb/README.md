# MariaDB 12.2 ANTLR4 Grammar

A comprehensive ANTLR4 lexer and parser grammar for the MariaDB 12.2 SQL dialect. Designed for use in SQL parsing, syntax highlighting, code analysis, and tooling development.

## Files

| File | Lines | Description |
|------|-------|-------------|
| `MariaDBLexer.g4` | 1,012 | Lexer grammar with keywords, operators, literals, identifiers |
| `MariaDBParser.g4` | 3,007 | Parser grammar with complete SQL statement coverage |

## Quick Start

### Prerequisites

- [ANTLR4](https://www.antlr.org/) (4.9+)
- Java Runtime Environment (for ANTLR tool)

### Generate Parser Code

```bash
# Generate Java (default)
antlr4 MariaDBLexer.g4 MariaDBParser.g4

# Generate Python
antlr4 -Dlanguage=Python3 MariaDBLexer.g4 MariaDBParser.g4

# Generate TypeScript
antlr4 -Dlanguage=TypeScript MariaDBLexer.g4 MariaDBParser.g4

# Generate C++
antlr4 -Dlanguage=Cpp MariaDBLexer.g4 MariaDBParser.g4

# Generate Rust (with antlr4rust)
antlr4 -Dlanguage=Rust MariaDBLexer.g4 MariaDBParser.g4
```

### Basic Usage (Java)

```java
import org.antlr.v4.runtime.*;
import org.antlr.v4.runtime.tree.*;

public class MariaDBParserExample {
    public static void main(String[] args) {
        String sql = "SELECT id, name FROM users WHERE active = 1";
        
        CharStream input = CharStreams.fromString(sql);
        MariaDBLexer lexer = new MariaDBLexer(input);
        CommonTokenStream tokens = new CommonTokenStream(lexer);
        MariaDBParser parser = new MariaDBParser(tokens);
        
        ParseTree tree = parser.root();
        System.out.println(tree.toStringTree(parser));
    }
}
```

### Basic Usage (Python)

```python
from antlr4 import CommonTokenStream, InputStream
from MariaDBLexer import MariaDBLexer
from MariaDBParser import MariaDBParser

sql = "SELECT id, name FROM users WHERE active = 1"

input_stream = InputStream(sql)
lexer = MariaDBLexer(input_stream)
token_stream = CommonTokenStream(lexer)
parser = MariaDBParser(token_stream)

tree = parser.root()
print(tree.toStringTree(recog=parser))
```

### Basic Usage (Rust)

```rust
use antlr_rust::common_token_stream::CommonTokenStream;
use antlr_rust::input_stream::InputStream;
use antlr_rust::parser::Parser;

fn main() {
    let sql = "SELECT id, name FROM users WHERE active = 1";
    
    let input = InputStream::new(sql);
    let lexer = MariaDBLexer::new(input);
    let token_stream = CommonTokenStream::new(lexer);
    let mut parser = MariaDBParser::new(token_stream);
    
    let tree = parser.root().expect("Failed to parse");
}
```

## Grammar Coverage

### DDL (Data Definition Language)

| Statement | Supported Features |
|-----------|-------------------|
| `CREATE TABLE` | Columns, constraints, indexes, partitioning, table options, `AS SELECT`, `LIKE` |
| `ALTER TABLE` | Add/drop/modify columns, indexes, constraints, partitions, rename, convert charset |
| `CREATE INDEX` | BTREE, HASH, RTREE; fulltext; spatial; unique; algorithm/lock options |
| `CREATE VIEW` | With check option, definer, algorithm, security context |
| `CREATE PROCEDURE/FUNCTION` | Parameters, characteristics, compound statements |
| `CREATE TRIGGER` | Before/after, insert/update/delete, row-level, ordering |
| `CREATE SEQUENCE` | Start, increment, min/max value, cycle, cache (MariaDB-specific) |
| `CREATE PACKAGE` | Oracle-mode package and package body (MariaDB-specific) |
| `CREATE EVENT` | Scheduling, on completion, enable/disable |
| `CREATE DATABASE` | Character set, collation, encryption |
| `CREATE ROLE` | Role creation with admin option |
| `DROP/TRUNCATE` | All object types with IF EXISTS |

### DML (Data Manipulation Language)

| Statement | Supported Features |
|-----------|-------------------|
| `SELECT` | All clauses, subqueries, joins, CTEs, window functions, QUALIFY |
| `INSERT` | VALUES, SELECT, TABLE, ON DUPLICATE KEY UPDATE, RETURNING |
| `UPDATE` | Single/multi-table, ORDER BY, LIMIT, RETURNING |
| `DELETE` | Single/multi-table, ORDER BY, LIMIT, RETURNING |
| `REPLACE` | Full syntax support |
| `LOAD DATA` | LOCAL, INFILE, character set, field/line terminators |
| `HANDLER` | Open, read, close operations |

### Query Features

```sql
-- Common Table Expressions (CTEs)
WITH RECURSIVE cte AS (
    SELECT 1 AS n
    UNION ALL
    SELECT n + 1 FROM cte WHERE n < 10
)
SELECT * FROM cte;

-- Window Functions
SELECT 
    name,
    department,
    salary,
    ROW_NUMBER() OVER (PARTITION BY department ORDER BY salary DESC) as rank,
    AVG(salary) OVER (PARTITION BY department) as dept_avg
FROM employees;

-- QUALIFY Clause (MariaDB-specific)
SELECT name, salary
FROM employees
QUALIFY ROW_NUMBER() OVER (ORDER BY salary DESC) <= 10;

-- Set Operations
SELECT * FROM table1
UNION ALL
SELECT * FROM table2
EXCEPT
SELECT * FROM table3
INTERSECT
SELECT * FROM table4;

-- JSON Operations
SELECT 
    data->'$.name' as name,
    data->>'$.email' as email
FROM users;

-- System Versioning (MariaDB-specific)
SELECT * FROM orders
FOR SYSTEM_TIME AS OF '2024-01-01 00:00:00';
```

### Transaction Control

| Statement | Features |
|-----------|----------|
| `START TRANSACTION` | Read only/write, consistent snapshot |
| `COMMIT/ROLLBACK` | Chain, release options |
| `SAVEPOINT` | Create, release, rollback to |
| `LOCK TABLES` | Read/write locks, low priority |
| `XA` | Start, end, prepare, commit, rollback, recover |

### Administration

| Category | Statements |
|----------|------------|
| User Management | `CREATE/ALTER/DROP USER`, `GRANT`, `REVOKE`, `SET PASSWORD`, `SET ROLE` |
| Table Maintenance | `ANALYZE`, `CHECK`, `CHECKSUM`, `OPTIMIZE`, `REPAIR` |
| Server Admin | `FLUSH`, `RESET`, `KILL`, `SHUTDOWN` |
| Information | 40+ `SHOW` variants |
| Plugins | `INSTALL/UNINSTALL PLUGIN` |
| Replication | `CHANGE MASTER`, `START/STOP SLAVE`, `SHOW SLAVE STATUS` |

### Procedural SQL

```sql
-- Compound Statements
CREATE PROCEDURE example()
BEGIN
    DECLARE v_count INT DEFAULT 0;
    DECLARE v_done BOOLEAN DEFAULT FALSE;
    DECLARE cur CURSOR FOR SELECT id FROM items;
    DECLARE CONTINUE HANDLER FOR NOT FOUND SET v_done = TRUE;
    
    OPEN cur;
    read_loop: LOOP
        FETCH cur INTO v_count;
        IF v_done THEN
            LEAVE read_loop;
        END IF;
        -- Process row
    END LOOP;
    CLOSE cur;
END;

-- Control Flow
IF condition THEN
    -- statements
ELSEIF other_condition THEN
    -- statements
ELSE
    -- statements
END IF;

CASE expression
    WHEN value1 THEN result1
    WHEN value2 THEN result2
    ELSE default_result
END CASE;

-- Error Handling
SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'Custom error';
RESIGNAL SET MESSAGE_TEXT = 'Modified error';
GET DIAGNOSTICS CONDITION 1 @msg = MESSAGE_TEXT;
```

### Data Types

| Category | Types |
|----------|-------|
| Numeric | `TINYINT`, `SMALLINT`, `MEDIUMINT`, `INT`, `BIGINT`, `DECIMAL`, `FLOAT`, `DOUBLE`, `BIT` |
| String | `CHAR`, `VARCHAR`, `TEXT` variants, `BINARY`, `VARBINARY`, `BLOB` variants, `ENUM`, `SET` |
| Date/Time | `DATE`, `TIME`, `DATETIME`, `TIMESTAMP`, `YEAR` |
| Spatial | `GEOMETRY`, `POINT`, `LINESTRING`, `POLYGON`, `MULTIPOINT`, `MULTILINESTRING`, `MULTIPOLYGON`, `GEOMETRYCOLLECTION` |
| JSON | `JSON` |
| Network | `INET4`, `INET6`, `UUID` (MariaDB-specific) |
| Vector | `VECTOR(n)` (MariaDB 11.6+) |

### Functions

The grammar supports 150+ built-in functions:

| Category | Examples |
|----------|----------|
| String | `CONCAT`, `SUBSTRING`, `LENGTH`, `REPLACE`, `TRIM`, `UPPER`, `LOWER`, `LPAD`, `RPAD` |
| Numeric | `ABS`, `ROUND`, `CEIL`, `FLOOR`, `SQRT`, `POW`, `MOD`, `RAND` |
| Date/Time | `NOW`, `CURDATE`, `DATE_FORMAT`, `DATEDIFF`, `DATE_ADD`, `EXTRACT` |
| Aggregate | `COUNT`, `SUM`, `AVG`, `MIN`, `MAX`, `GROUP_CONCAT`, `JSON_ARRAYAGG` |
| Window | `ROW_NUMBER`, `RANK`, `DENSE_RANK`, `LEAD`, `LAG`, `FIRST_VALUE`, `LAST_VALUE`, `NTH_VALUE`, `NTILE`, `CUME_DIST`, `PERCENT_RANK` |
| JSON | `JSON_VALUE`, `JSON_EXTRACT`, `JSON_SET`, `JSON_ARRAY`, `JSON_OBJECT`, `JSON_TABLE` |
| Encryption | `MD5`, `SHA1`, `SHA2`, `AES_ENCRYPT`, `AES_DECRYPT` |
| System | `DATABASE`, `USER`, `VERSION`, `CONNECTION_ID`, `LAST_INSERT_ID` |

### Operators

| Type | Operators |
|------|-----------|
| Comparison | `=`, `<>`, `!=`, `<`, `>`, `<=`, `>=`, `<=>` (NULL-safe) |
| Logical | `AND`, `OR`, `NOT`, `XOR`, `&&`, `\|\|` |
| Arithmetic | `+`, `-`, `*`, `/`, `%`, `DIV`, `MOD` |
| Bitwise | `&`, `\|`, `^`, `~`, `<<`, `>>` |
| JSON | `->`, `->>` |
| Pattern | `LIKE`, `REGEXP`, `RLIKE` |
| Other | `IN`, `BETWEEN`, `IS NULL`, `EXISTS`, `MEMBER OF` |

## MariaDB-Specific Features

This grammar includes syntax specific to MariaDB that differs from MySQL:

| Feature | Description |
|---------|-------------|
| Sequences | `CREATE/ALTER/DROP SEQUENCE`, `NEXT VALUE FOR`, `SETVAL` |
| System Versioning | `WITH SYSTEM VERSIONING`, `FOR SYSTEM_TIME` queries |
| Oracle Mode | `CREATE PACKAGE`, `CREATE PACKAGE BODY`, `ROWNUM`, `MINUS` |
| RETURNING | `INSERT/UPDATE/DELETE ... RETURNING` clause |
| OFFSET | `OFFSET n ROWS FETCH NEXT m ROWS ONLY` syntax |
| QUALIFY | Filter window function results |
| Vector Type | `VECTOR(dimension)` for AI/ML workloads |
| Network Types | `INET4`, `INET6`, `UUID` native types |
| INTERSECT/EXCEPT | Full set operation support |

## Lexer Channels

The lexer uses channels to separate different token types:

| Channel | Purpose |
|---------|---------|
| `DEFAULT_TOKEN_CHANNEL` | All regular tokens |
| `HIDDEN` | Whitespace |
| `MYSQLCOMMENT` | MySQL-style executable comments (`/*! ... */`) |
| `ERRORCHANNEL` | Unrecognized characters |

## Grammar Structure

### Lexer Organization

```
MariaDBLexer.g4
├── Fragments (A-Z, digits, exponents)
├── Whitespace & Comments
├── Reserved Keywords (200+)
├── Non-Reserved Keywords (300+)
├── Oracle Mode Keywords
├── Spatial Keywords
├── Data Type Keywords
├── Window Function Keywords
├── Operators
├── Symbols
├── Literals
└── Identifiers
```

### Parser Organization

```
MariaDBParser.g4
├── Root Rules
├── DDL Statements
│   ├── CREATE statements
│   ├── ALTER statements
│   └── DROP statements
├── DML Statements
│   ├── SELECT (with CTEs, window functions)
│   ├── INSERT/UPDATE/DELETE
│   └── LOAD DATA
├── Transaction Statements
├── Replication Statements
├── Prepared Statements
├── Compound Statements (procedural)
├── Administration Statements
├── Utility Statements
├── Expressions
│   ├── Logical expressions
│   ├── Comparison predicates
│   ├── Arithmetic expressions
│   └── Function calls
├── Data Types
└── Common Rules (identifiers, literals)
```

## Error Handling

The grammar includes error recovery and provides meaningful parse trees even for partially invalid input. For production use, implement an error listener:

```java
parser.removeErrorListeners();
parser.addErrorListener(new BaseErrorListener() {
    @Override
    public void syntaxError(Recognizer<?, ?> recognizer,
                           Object offendingSymbol,
                           int line, int charPositionInLine,
                           String msg, RecognitionException e) {
        throw new ParseException(
            String.format("Line %d:%d - %s", line, charPositionInLine, msg)
        );
    }
});
```

## Use Cases

- **SQL Parsing**: Build ASTs for SQL analysis and transformation
- **Syntax Highlighting**: IDE and editor support for MariaDB SQL
- **Query Validation**: Validate SQL syntax before execution
- **Code Generation**: Generate SQL from programmatic representations
- **Migration Tools**: Parse and convert SQL between dialects
- **Security Analysis**: Static analysis for SQL injection detection
- **Query Optimization**: Analyze and rewrite queries

## Testing

```bash
# Test with ANTLR's TestRig (grun)
cd output_directory
javac *.java
echo "SELECT * FROM users WHERE id = 1" | grun MariaDB root -tree

# Or use the GUI tree viewer
echo "SELECT * FROM users WHERE id = 1" | grun MariaDB root -gui
```

## Compatibility

| MariaDB Version | Support Level |
|-----------------|---------------|
| 12.2 | Full |
| 11.x | Full |
| 10.x | Full (some 12.2 features unavailable) |

The grammar maintains backward compatibility with older MariaDB versions while supporting all features introduced in 12.2.

## Known Limitations

1. **Preprocessor directives**: SQL preprocessor commands are not supported
2. **Dynamic SQL strings**: SQL embedded in string literals is not parsed
3. **Vendor extensions**: Some non-standard vendor-specific syntax may not be included
4. **Character sets**: Full Unicode identifier support depends on ANTLR configuration

## References

- [MariaDB 12.2 Documentation](https://mariadb.com/docs/)
- [MariaDB Reserved Words](https://mariadb.com/kb/en/reserved-words/)
- [MariaDB SQL Statements](https://mariadb.com/kb/en/sql-statements/)
- [MariaDB Data Types](https://mariadb.com/kb/en/data-types/)
- [ANTLR4 Documentation](https://github.com/antlr/antlr4/blob/master/doc/index.md)

## License

MIT License

## Contributing

Contributions are welcome. Please ensure any changes:

1. Maintain backward compatibility
2. Include test cases for new syntax
3. Follow existing code style
4. Update documentation as needed
