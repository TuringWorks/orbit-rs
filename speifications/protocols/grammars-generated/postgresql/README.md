# PostgreSQL 18 ANTLR4 Grammar

A comprehensive ANTLR4 lexer and parser grammar for PostgreSQL 18, the latest version of the world's most advanced open-source relational database (released September 25, 2025).

## Overview

This grammar provides full parsing support for PostgreSQL 18 SQL syntax, including all new features introduced in this major release. It is designed to be used with ANTLR4 for building SQL parsers, linters, formatters, and other SQL analysis tools.

## Files

| File | Description |
|------|-------------|
| `PostgreSQL18Lexer.g4` | Lexer grammar defining all tokens (keywords, operators, literals, identifiers) |
| `PostgreSQL18Parser.g4` | Parser grammar defining SQL syntax rules |
| `README.md` | This documentation file |

## PostgreSQL 18 New Features Supported

This grammar includes support for all major PostgreSQL 18 syntax additions:

### Virtual Generated Columns (Default)
```sql
-- VIRTUAL is now the default for generated columns
CREATE TABLE orders (
    id SERIAL PRIMARY KEY,
    subtotal DECIMAL(10,2),
    tax_rate DECIMAL(5,4) DEFAULT 0.0875,
    total DECIMAL(10,2) GENERATED ALWAYS AS (subtotal * (1 + tax_rate)),  -- Virtual by default
    audit_info TEXT GENERATED ALWAYS AS (...) STORED  -- Explicitly stored
);
```

### UUIDv7 Support
```sql
-- Generate timestamp-ordered UUIDs (native function)
SELECT uuidv7();

-- Use as primary key
CREATE TABLE events (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    event_data JSONB
);
```

### OLD/NEW in RETURNING Clauses
```sql
-- Access both old and new values in DML operations
UPDATE products 
SET price = price * 1.10 
WHERE price <= 99.99
RETURNING name, old.price AS old_price, new.price AS new_price;

-- Works with INSERT ON CONFLICT
INSERT INTO products (name, price) VALUES ('Widget', 25.00)
ON CONFLICT (name) DO UPDATE SET price = EXCLUDED.price
RETURNING name, old.price AS previous_price, new.price AS current_price;

-- And with DELETE
DELETE FROM products WHERE price < 10.00
RETURNING old.name AS deleted_product, old.price AS deleted_price;

-- And with MERGE
MERGE INTO target USING source ON target.id = source.id
WHEN MATCHED THEN UPDATE SET value = source.value
RETURNING old.value, new.value;
```

### Temporal Constraints (WITHOUT OVERLAPS)
```sql
-- Prevent overlapping time ranges with PRIMARY KEY
CREATE TABLE room_bookings (
    room_id INTEGER,
    booking_period TSTZRANGE,
    PRIMARY KEY (room_id, booking_period WITHOUT OVERLAPS)
);

-- Also works with UNIQUE constraints
CREATE TABLE employee_assignments (
    employee_id INTEGER,
    project_id INTEGER,
    assignment_period DATERANGE,
    UNIQUE (employee_id, assignment_period WITHOUT OVERLAPS)
);

-- Temporal foreign keys with PERIOD
CREATE TABLE booking_details (
    booking_id INTEGER,
    room_id INTEGER,
    detail_period TSTZRANGE,
    FOREIGN KEY (room_id, PERIOD detail_period) 
        REFERENCES room_bookings (room_id, PERIOD booking_period)
);
```

### ENFORCED/NOT ENFORCED for CHECK Constraints
```sql
-- Create unenforced check constraint
ALTER TABLE orders 
ADD CONSTRAINT chk_positive_amount 
CHECK (amount > 0) NOT ENFORCED;

-- Later enforce it
ALTER TABLE orders 
ALTER CONSTRAINT chk_positive_amount ENFORCED;
```

### NOT VALID for NOT NULL Constraints
```sql
-- Add NOT NULL without immediate validation
ALTER TABLE large_table 
ADD CONSTRAINT nn_column NOT NULL (column_name) NOT VALID;

-- Validate later without exclusive lock
ALTER TABLE large_table VALIDATE CONSTRAINT nn_column;
```

### Enhanced COPY Options
```sql
-- REJECT_LIMIT: Control number of invalid rows to ignore
COPY my_table FROM '/path/to/file.csv'
WITH (FORMAT csv, ON_ERROR 'ignore', REJECT_LIMIT 100);

-- LOG_VERBOSITY: Control logging of ignored rows
COPY my_table FROM '/path/to/file.csv'
WITH (FORMAT csv, ON_ERROR 'ignore', LOG_VERBOSITY silent);
```

### CREATE FOREIGN TABLE ... LIKE
```sql
-- Create foreign table based on existing table structure
CREATE FOREIGN TABLE foreign_orders
LIKE local_orders
SERVER remote_server;
```

### ALTER CONSTRAINT ... INHERIT
```sql
-- Control constraint inheritance on partitioned tables
ALTER TABLE partitioned_table 
ALTER CONSTRAINT my_constraint INHERIT;

ALTER TABLE partitioned_table 
ALTER CONSTRAINT my_constraint NO INHERIT;
```

### EXPLAIN ANALYZE Auto-includes BUFFERS
```sql
-- BUFFERS is now automatically included with ANALYZE
EXPLAIN ANALYZE SELECT * FROM large_table WHERE id > 1000;
-- Output now includes buffer usage by default
```

## Installation & Usage

### Prerequisites
- ANTLR4 4.13+ (recommended: 4.13.1 or later)
- Java Runtime Environment (JRE) 11+

### Generate Parser Code

#### Java
```bash
antlr4 -Dlanguage=Java PostgreSQL18Lexer.g4 PostgreSQL18Parser.g4
javac *.java
```

#### Python
```bash
antlr4 -Dlanguage=Python3 PostgreSQL18Lexer.g4 PostgreSQL18Parser.g4
```

#### C#
```bash
antlr4 -Dlanguage=CSharp PostgreSQL18Lexer.g4 PostgreSQL18Parser.g4
```

#### TypeScript
```bash
antlr4 -Dlanguage=TypeScript PostgreSQL18Lexer.g4 PostgreSQL18Parser.g4
```

#### Go
```bash
antlr4 -Dlanguage=Go -package postgresql PostgreSQL18Lexer.g4 PostgreSQL18Parser.g4
```

#### C++
```bash
antlr4 -Dlanguage=Cpp PostgreSQL18Lexer.g4 PostgreSQL18Parser.g4
```

### Example Usage (Java)

```java
import org.antlr.v4.runtime.*;
import org.antlr.v4.runtime.tree.*;

public class PostgreSQLParserExample {
    public static void main(String[] args) {
        String sql = "SELECT * FROM users WHERE age > 21";
        
        // Create lexer and parser
        CharStream input = CharStreams.fromString(sql);
        PostgreSQL18Lexer lexer = new PostgreSQL18Lexer(input);
        CommonTokenStream tokens = new CommonTokenStream(lexer);
        PostgreSQL18Parser parser = new PostgreSQL18Parser(tokens);
        
        // Parse starting from root rule
        ParseTree tree = parser.root();
        
        // Print parse tree
        System.out.println(tree.toStringTree(parser));
    }
}
```

### Example Usage (Python)

```python
from antlr4 import *
from PostgreSQL18Lexer import PostgreSQL18Lexer
from PostgreSQL18Parser import PostgreSQL18Parser

sql = "SELECT * FROM users WHERE age > 21"

# Create lexer and parser
input_stream = InputStream(sql)
lexer = PostgreSQL18Lexer(input_stream)
token_stream = CommonTokenStream(lexer)
parser = PostgreSQL18Parser(token_stream)

# Parse starting from root rule
tree = parser.root()

# Print parse tree
print(tree.toStringTree(recog=parser))
```

## Grammar Structure

### Lexer Organization
The lexer (`PostgreSQL18Lexer.g4`) is organized into these sections:

1. **Keywords** - All PostgreSQL reserved and non-reserved keywords (A-Z)
2. **Operators** - Comparison, arithmetic, bitwise, string, JSON, array, range, geometric, and text search operators
3. **Punctuation** - Parentheses, brackets, braces, comma, semicolon, etc.
4. **Literals** - Integer, numeric, hex, binary, bit string, string (single-quoted, dollar-quoted, escape, unicode)
5. **Identifiers** - Regular, quoted, unicode identifiers, parameters
6. **Comments** - Single-line (`--`) and multi-line (`/* */`) with nesting support
7. **Whitespace** - Spaces, tabs, newlines (sent to hidden channel)

### Parser Organization
The parser (`PostgreSQL18Parser.g4`) is organized into these sections:

1. **DDL Statements** - CREATE, ALTER, DROP for all database objects
2. **DML Statements** - SELECT, INSERT, UPDATE, DELETE, MERGE, COPY
3. **DCL Statements** - GRANT, REVOKE, REASSIGN OWNED
4. **Transaction Statements** - BEGIN, COMMIT, ROLLBACK, SAVEPOINT, etc.
5. **Session Statements** - SET, RESET, SHOW
6. **Utility Statements** - ANALYZE, VACUUM, EXPLAIN, PREPARE, EXECUTE, etc.
7. **PL/pgSQL Support** - Basic block structure and exception handling
8. **Expressions** - Full expression grammar with operators, functions, subqueries
9. **Data Types** - All PostgreSQL data types including ranges and JSON
10. **Common Rules** - Identifiers, literals, qualified names, etc.

## Supported SQL Features

### Data Definition Language (DDL)
- CREATE/ALTER/DROP for: DATABASE, SCHEMA, TABLE, VIEW, MATERIALIZED VIEW, INDEX, SEQUENCE, TYPE, DOMAIN, FUNCTION, PROCEDURE, TRIGGER, RULE, POLICY, ROLE, EXTENSION, PUBLICATION, SUBSCRIPTION, SERVER, FOREIGN TABLE, and more
- Table constraints: PRIMARY KEY, UNIQUE, FOREIGN KEY, CHECK, EXCLUDE
- Partitioning: RANGE, LIST, HASH with full partition management
- Generated columns (STORED and VIRTUAL)
- Identity columns

### Data Manipulation Language (DML)
- SELECT with all clauses: FROM, WHERE, GROUP BY, HAVING, WINDOW, ORDER BY, LIMIT, OFFSET, FETCH, FOR UPDATE/SHARE
- Common Table Expressions (WITH, WITH RECURSIVE)
- Set operations: UNION, INTERSECT, EXCEPT
- INSERT with ON CONFLICT (upsert)
- UPDATE with FROM clause
- DELETE with USING clause
- MERGE with all WHEN clauses
- COPY with all options

### Data Control Language (DCL)
- GRANT/REVOKE for all privileges
- Role management
- Row-level security policies

### Additional Features
- Full expression support including JSON/JSONB operators
- Window functions
- Aggregate functions with FILTER and WITHIN GROUP
- Array operations
- Range types and operations
- Text search operators
- XML functions

## Testing

To test the grammar with the ANTLR4 TestRig:

```bash
# Compile the grammar
antlr4 PostgreSQL18Lexer.g4 PostgreSQL18Parser.g4
javac *.java

# Test with GUI parse tree viewer
echo "SELECT * FROM users" | grun PostgreSQL18 root -gui

# Test with tokens output
echo "SELECT * FROM users" | grun PostgreSQL18 root -tokens

# Test with parse tree output
echo "SELECT * FROM users" | grun PostgreSQL18 root -tree
```

## References

- [PostgreSQL 18 Release Notes](https://www.postgresql.org/docs/18/release-18.html)
- [PostgreSQL 18 Documentation](https://www.postgresql.org/docs/18/)
- [ANTLR4 Documentation](https://www.antlr.org/documentation.html)
- [ANTLR4 GitHub](https://github.com/antlr/antlr4)

## License

MIT License

Copyright (c) 2025

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.

## Changelog

### 1.0.0 (December 2025)
- Initial release with full PostgreSQL 18 support
- Support for all new PostgreSQL 18 features:
  - Virtual generated columns (VIRTUAL keyword)
  - UUIDv7 function support
  - OLD/NEW aliases in RETURNING clauses
  - Temporal constraints (WITHOUT OVERLAPS, PERIOD)
  - ENFORCED/NOT ENFORCED for CHECK constraints
  - NOT VALID for NOT NULL constraints
  - Enhanced COPY options (REJECT_LIMIT, LOG_VERBOSITY)
  - CREATE FOREIGN TABLE ... LIKE
  - ALTER CONSTRAINT ... INHERIT
  - EXPLAIN ANALYZE auto-includes BUFFERS
