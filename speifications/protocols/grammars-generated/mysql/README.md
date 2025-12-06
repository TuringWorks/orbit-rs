# MySQL 9.5 ANTLR4 Grammar with MySQL AI Support

A comprehensive ANTLR4 grammar for MySQL 9.5 including support for MySQL AI features (HeatWave GenAI, AutoML, and Vector operations).

## Overview

This grammar provides complete lexer and parser definitions for MySQL 9.5, the latest Innovation Release from Oracle. It includes full support for:

- **Standard MySQL 9.5 SQL syntax**
- **VECTOR data type** (MySQL 9.0+)
- **JavaScript stored programs** (MySQL 9.0+ Enterprise)
- **MySQL HeatWave AI/ML functions**
- **HeatWave GenAI features** (LLM integration, RAG, embeddings)
- **Enhanced replication features**

## Files

| File | Description |
|------|-------------|
| `MySQLLexer.g4` | Lexer grammar defining all tokens, keywords, operators, and literals |
| `MySQLParser.g4` | Parser grammar defining the complete MySQL SQL syntax |
| `README.md` | This documentation file |

## Requirements

- **ANTLR4** version 4.9 or higher
- **Java** 8+ (for ANTLR tool)
- Target language runtime (Java, Python, C#, JavaScript, Go, C++, etc.)

## Installation

### 1. Install ANTLR4

```bash
# Using Homebrew (macOS)
brew install antlr

# Using apt (Debian/Ubuntu)
sudo apt-get install antlr4

# Manual download
curl -O https://www.antlr.org/download/antlr-4.13.1-complete.jar
alias antlr4='java -jar /path/to/antlr-4.13.1-complete.jar'
```

### 2. Generate Parser/Lexer

```bash
# Java target
antlr4 MySQLLexer.g4 MySQLParser.g4

# Python target
antlr4 -Dlanguage=Python3 MySQLLexer.g4 MySQLParser.g4

# JavaScript target
antlr4 -Dlanguage=JavaScript MySQLLexer.g4 MySQLParser.g4

# TypeScript target
antlr4 -Dlanguage=TypeScript MySQLLexer.g4 MySQLParser.g4

# C# target
antlr4 -Dlanguage=CSharp MySQLLexer.g4 MySQLParser.g4

# Go target
antlr4 -Dlanguage=Go MySQLLexer.g4 MySQLParser.g4

# C++ target
antlr4 -Dlanguage=Cpp MySQLLexer.g4 MySQLParser.g4
```

### 3. Compile Generated Code

```bash
# Java
javac *.java

# Python - no compilation needed

# JavaScript/TypeScript
npm install antlr4
```

## Usage Examples

### Java

```java
import org.antlr.v4.runtime.*;
import org.antlr.v4.runtime.tree.*;

public class MySQLParserExample {
    public static void main(String[] args) {
        String sql = "SELECT * FROM users WHERE age > 21";
        
        CharStream input = CharStreams.fromString(sql);
        MySQLLexer lexer = new MySQLLexer(input);
        CommonTokenStream tokens = new CommonTokenStream(lexer);
        MySQLParser parser = new MySQLParser(tokens);
        
        ParseTree tree = parser.root();
        System.out.println(tree.toStringTree(parser));
    }
}
```

### Python

```python
from antlr4 import *
from MySQLLexer import MySQLLexer
from MySQLParser import MySQLParser

def parse_sql(sql: str):
    input_stream = InputStream(sql)
    lexer = MySQLLexer(input_stream)
    token_stream = CommonTokenStream(lexer)
    parser = MySQLParser(token_stream)
    
    tree = parser.root()
    return tree

# Example usage
sql = "SELECT id, name FROM customers WHERE country = 'USA'"
tree = parse_sql(sql)
print(tree.toStringTree(recog=parser))
```

### JavaScript/Node.js

```javascript
const antlr4 = require('antlr4');
const MySQLLexer = require('./MySQLLexer').MySQLLexer;
const MySQLParser = require('./MySQLParser').MySQLParser;

function parseSQL(sql) {
    const chars = new antlr4.InputStream(sql);
    const lexer = new MySQLLexer(chars);
    const tokens = new antlr4.CommonTokenStream(lexer);
    const parser = new MySQLParser(tokens);
    
    return parser.root();
}

// Example usage
const sql = "INSERT INTO orders (customer_id, amount) VALUES (1, 99.99)";
const tree = parseSQL(sql);
console.log(tree.toStringTree(parser.ruleNames));
```

## MySQL 9.5 Features Supported

### VECTOR Data Type (MySQL 9.0+)

```sql
-- Create table with VECTOR column
CREATE TABLE embeddings (
    id INT PRIMARY KEY,
    content TEXT,
    embedding VECTOR(768)
);

-- Use VECTOR with USING VARBINARY (Cloud SQL style)
CREATE TABLE vectors (
    id INT PRIMARY KEY,
    vec VECTOR(3) USING VARBINARY
);

-- Vector functions
SELECT STRING_TO_VECTOR('[1.0, 2.0, 3.0]');
SELECT TO_VECTOR('[1.5, -2.5, 3.5]');
SELECT VECTOR_TO_STRING(embedding) FROM embeddings;
SELECT FROM_VECTOR(vec) FROM vectors;
SELECT VECTOR_DIM(embedding) FROM embeddings;
```

### JavaScript Stored Programs (MySQL 9.0+ Enterprise)

```sql
-- Create JavaScript function
CREATE FUNCTION calculate_tax(amount DECIMAL(10,2))
RETURNS DECIMAL(10,2)
LANGUAGE JAVASCRIPT
DETERMINISTIC
BEGIN
    return amount * 0.08;
END;

-- Create JavaScript procedure
CREATE PROCEDURE process_order(IN order_id INT)
LANGUAGE JAVASCRIPT
BEGIN
    // JavaScript code here
    let result = processOrderLogic(order_id);
    return result;
END;
```

### MySQL HeatWave AutoML

```sql
-- Train a model
CALL sys.ML_TRAIN(
    'sales_db.training_data',
    'target_column',
    JSON_OBJECT('task', 'classification'),
    @model_handle
);

-- Load the model
CALL sys.ML_MODEL_LOAD(@model_handle, NULL);

-- Predict on a single row
SELECT sys.ML_PREDICT_ROW(
    JSON_OBJECT('feature1', 100, 'feature2', 'value'),
    @model_handle,
    NULL
);

-- Predict on a table
CALL sys.ML_PREDICT_TABLE(
    'sales_db.test_data',
    @model_handle,
    'sales_db.predictions',
    NULL
);

-- Get prediction explanations
SELECT sys.ML_EXPLAIN_ROW(
    JSON_OBJECT('feature1', 100, 'feature2', 'value'),
    @model_handle,
    NULL
);

-- Score model performance
CALL sys.ML_SCORE(
    'sales_db.test_data',
    'target_column',
    @model_handle,
    'accuracy',
    @score,
    NULL
);
```

### MySQL HeatWave GenAI

```sql
-- Generate content using LLM
SELECT sys.ML_GENERATE(
    'Explain the benefits of cloud computing in 3 sentences',
    JSON_OBJECT('model', 'llama3.2-3b-instruct-v1')
);

-- Generate embeddings
SELECT sys.ML_EMBED(
    'This is a sample text to embed',
    JSON_OBJECT('model', 'multilingual-e5-small')
);

-- Natural Language to SQL
SELECT sys.ML_NL_SQL(
    'Show me the top 10 customers by revenue',
    JSON_OBJECT('schema', 'sales_db')
);

-- Batch content generation
CALL sys.ML_GENERATE_TABLE(
    'prompts_table',
    'prompt_column',
    'results_table',
    JSON_OBJECT('model', 'llama3.2-3b-instruct-v1')
);

-- Batch embedding generation
CALL sys.ML_EMBED_TABLE(
    'documents_table',
    'content_column',
    'embeddings_table',
    JSON_OBJECT('model', 'multilingual-e5-small')
);
```

### HeatWave Secondary Engine

```sql
-- Create table with HeatWave secondary engine
CREATE TABLE analytics_data (
    id INT PRIMARY KEY,
    data JSON
) SECONDARY_ENGINE = RAPID;

-- Load data to HeatWave
ALTER TABLE analytics_data SECONDARY_LOAD;

-- Unload from HeatWave
ALTER TABLE analytics_data SECONDARY_UNLOAD;
```

### MySQL 9.5 Replication Enhancements

```sql
-- Configure replication with SSL enabled by default (MySQL 9.5)
CHANGE REPLICATION SOURCE TO
    SOURCE_HOST = 'primary.example.com',
    SOURCE_USER = 'repl_user',
    SOURCE_PASSWORD = 'secret',
    SOURCE_SSL = 1,  -- Now default in 9.5
    SOURCE_AUTO_POSITION = 1;

-- GTID mode enabled by default in MySQL 9.5
-- No need to explicitly set gtid_mode = ON
```

### EXPLAIN ANALYZE with JSON Output (MySQL 9.0+)

```sql
-- Save EXPLAIN ANALYZE output to variable
EXPLAIN ANALYZE FORMAT=JSON INTO @explain_result
SELECT * FROM orders WHERE customer_id = 100;

-- Use the result with JSON functions
SELECT JSON_EXTRACT(@explain_result, '$.query_block.cost_info');

-- With FOR SCHEMA clause
EXPLAIN ANALYZE FORMAT=JSON INTO @result FOR SCHEMA mydb
SELECT * FROM products WHERE price > 100;
```

## Grammar Structure

### Lexer (`MySQLLexer.g4`)

The lexer defines:

- **Keywords**: All MySQL reserved and non-reserved keywords (~500+)
- **Operators**: Arithmetic, comparison, logical, bitwise operators
- **Literals**: Strings, numbers, hex, binary values
- **Identifiers**: Regular, quoted, Unicode identifiers
- **Variables**: User and system variables
- **Comments**: Single-line, multi-line, hints, version comments

### Parser (`MySQLParser.g4`)

The parser defines rules for:

| Category | Description |
|----------|-------------|
| DDL | CREATE, ALTER, DROP, TRUNCATE, RENAME |
| DML | SELECT, INSERT, UPDATE, DELETE, REPLACE |
| DCL | GRANT, REVOKE |
| Transaction | BEGIN, COMMIT, ROLLBACK, SAVEPOINT |
| Replication | CHANGE REPLICATION SOURCE, START/STOP REPLICA |
| Administration | SET, SHOW, FLUSH, KILL |
| ML Statements | ML_TRAIN, ML_PREDICT, ML_GENERATE, etc. |
| Compound | Stored procedures, functions, triggers |

## Supported MySQL Versions

This grammar is designed for MySQL 9.5 but maintains backward compatibility with:

- MySQL 9.0 - 9.4 (full support)
- MySQL 8.4 LTS (most features)
- MySQL 8.0 (core features)

## Key Differences from MySQL 8.x

| Feature | MySQL 8.x | MySQL 9.5 |
|---------|-----------|-----------|
| VECTOR type | Not supported | ✅ Supported |
| JavaScript stored programs | Not supported | ✅ Enterprise |
| GTID mode default | OFF | ON |
| SSL replication default | OFF | ON |
| EXPLAIN ANALYZE INTO | Not supported | ✅ Supported |
| ML/AI functions | External only | ✅ In-database |

## Error Handling

The grammar includes error recovery strategies. To customize error handling:

```java
parser.removeErrorListeners();
parser.addErrorListener(new BaseErrorListener() {
    @Override
    public void syntaxError(Recognizer<?,?> recognizer,
                           Object offendingSymbol,
                           int line, int charPositionInLine,
                           String msg, RecognitionException e) {
        throw new RuntimeException("Syntax error at " + line + ":" + 
                                   charPositionInLine + " - " + msg);
    }
});
```

## Extending the Grammar

### Adding Custom Functions

```antlr
// In MySQLParser.g4, add to builtInFunction rule:
builtInFunction
    : functionName OPEN_PAREN expressionList? CLOSE_PAREN
    | myCustomFunction
    ;

myCustomFunction
    : MY_CUSTOM_FUNC OPEN_PAREN expression CLOSE_PAREN
    ;
```

### Adding Custom Keywords

```antlr
// In MySQLLexer.g4:
MY_CUSTOM_KEYWORD: 'MY_CUSTOM_KEYWORD';

// In MySQLParser.g4, add to nonReservedKeyword:
nonReservedKeyword
    : ... | MY_CUSTOM_KEYWORD
    ;
```

## Testing

### Test with Sample SQL

```bash
# Create a test file
cat > test.sql << 'EOF'
SELECT id, name, VECTOR_TO_STRING(embedding) as vec_str
FROM products
WHERE category = 'electronics'
ORDER BY price DESC
LIMIT 10;
EOF

# Run the parser (Java example)
java org.antlr.v4.gui.TestRig MySQL root -gui test.sql
```

### Unit Testing (Python)

```python
import unittest
from antlr4 import *
from MySQLLexer import MySQLLexer
from MySQLParser import MySQLParser

class TestMySQLParser(unittest.TestCase):
    def parse(self, sql):
        input_stream = InputStream(sql)
        lexer = MySQLLexer(input_stream)
        token_stream = CommonTokenStream(lexer)
        parser = MySQLParser(token_stream)
        return parser
    
    def test_select(self):
        parser = self.parse("SELECT * FROM users")
        tree = parser.root()
        self.assertIsNotNone(tree)
    
    def test_vector_type(self):
        parser = self.parse("CREATE TABLE t (v VECTOR(768))")
        tree = parser.root()
        self.assertIsNotNone(tree)
    
    def test_ml_predict(self):
        parser = self.parse(
            "SELECT sys.ML_PREDICT_ROW('{\"a\":1}', @model, NULL)"
        )
        tree = parser.root()
        self.assertIsNotNone(tree)

if __name__ == '__main__':
    unittest.main()
```

## Performance Considerations

- The grammar uses case-insensitive matching for keywords
- Large SQL statements may require increased stack size
- Consider using SLL prediction mode for better performance:

```java
parser.getInterpreter().setPredictionMode(PredictionMode.SLL);
try {
    tree = parser.root();
} catch (Exception e) {
    tokens.seek(0);
    parser.reset();
    parser.getInterpreter().setPredictionMode(PredictionMode.LL);
    tree = parser.root();
}
```

## Known Limitations

1. **Procedural SQL**: Complex stored procedure bodies with nested blocks may have edge cases
2. **Hints**: Oracle-style hints in comments are parsed but not semantically validated
3. **Character Sets**: Some exotic character set names may need to be added to the grammar
4. **Version Comments**: `/*! ... */` version comments are tokenized but not conditionally processed

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Add tests for new features
4. Submit a pull request

## License

This grammar is released under the MIT License.

## References

- [MySQL 9.5 Release Notes](https://dev.mysql.com/doc/relnotes/mysql/9.5/en/)
- [MySQL 9.0 Reference Manual](https://dev.mysql.com/doc/refman/9.0/en/)
- [MySQL HeatWave User Guide](https://dev.mysql.com/doc/heatwave/en/)
- [MySQL HeatWave GenAI](https://www.oracle.com/heatwave/genai/)
- [ANTLR4 Documentation](https://www.antlr.org/)
- [ANTLR4 GitHub](https://github.com/antlr/antlr4)

## Changelog

### v1.0.0 (December 2025)
- Initial release
- Full MySQL 9.5 syntax support
- VECTOR data type and functions
- JavaScript stored programs
- MySQL HeatWave AutoML functions
- HeatWave GenAI functions (ML_GENERATE, ML_EMBED, ML_NL_SQL)
- Enhanced replication syntax
- EXPLAIN ANALYZE INTO support
