# MongoDB Query Language (MQL) ANTLR4 Grammar

Comprehensive ANTLR4 lexer and parser grammars for parsing MongoDB Query Language (MQL), including queries, aggregation pipelines, and update operations. **Updated for MongoDB 8.2** (December 2025).

## Files

- **MongoLexer.g4** - Lexer grammar defining all tokens
- **MongoParser.g4** - Parser grammar defining the language structure

## MongoDB Version Compatibility

| MongoDB Version | Support Level |
|-----------------|---------------|
| 8.2 | ✅ Full (including `$scoreFusion`, `$vectorSearch` in Community) |
| 8.0 | ✅ Full (including `$rankFusion`, `$toUUID`) |
| 7.0 | ✅ Full (including `$median`, `$percentile`) |
| 6.0+ | ✅ Full |

## Features Supported

### Query Operations
- **Comparison Operators**: `$eq`, `$gt`, `$gte`, `$lt`, `$lte`, `$ne`, `$in`, `$nin`
- **Logical Operators**: `$and`, `$or`, `$not`, `$nor`
- **Element Operators**: `$exists`, `$type`
- **Evaluation Operators**: `$expr`, `$jsonSchema`, `$mod`, `$regex`, `$text`, `$where`
- **Array Operators**: `$all`, `$elemMatch`, `$size`
- **Bitwise Operators**: `$bitsAllClear`, `$bitsAllSet`, `$bitsAnyClear`, `$bitsAnySet`
- **Geospatial Operators**: `$geoWithin`, `$geoIntersects`, `$near`, `$nearSphere`

### Update Operations
- **Field Updates**: `$set`, `$setOnInsert`, `$unset`, `$inc`, `$mul`, `$rename`, `$min`, `$max`, `$currentDate`
- **Array Updates**: `$push`, `$pull`, `$pullAll`, `$pop`, `$addToSet`
- **Array Modifiers**: `$each`, `$slice`, `$sort`, `$position`
- **Bitwise Updates**: `$bit`

### Aggregation Pipeline Stages
`$addFields`, `$bucket`, `$bucketAuto`, `$collStats`, `$count`, `$densify`, `$documents`, `$facet`, `$fill`, `$geoNear`, `$graphLookup`, `$group`, `$indexStats`, `$limit`, `$lookup`, `$match`, `$merge`, `$out`, `$project`, `$rankFusion` (8.0+), `$redact`, `$replaceRoot`, `$replaceWith`, `$sample`, `$scoreFusion` (8.2+), `$search`, `$searchMeta`, `$set`, `$setWindowFields`, `$skip`, `$sort`, `$sortByCount`, `$unionWith`, `$unset`, `$unwind`, `$vectorSearch` (Atlas/8.2+ Community)

### Aggregation Expressions
- **Arithmetic**: `$abs`, `$add`, `$ceil`, `$divide`, `$exp`, `$floor`, `$ln`, `$log`, `$log10`, `$mod`, `$multiply`, `$pow`, `$round`, `$sqrt`, `$subtract`, `$trunc`
- **Array**: `$arrayElemAt`, `$arrayToObject`, `$concatArrays`, `$filter`, `$first`, `$indexOfArray`, `$isArray`, `$last`, `$map`, `$objectToArray`, `$range`, `$reduce`, `$reverseArray`, `$size`, `$slice`, `$sortArray`, `$zip`
- **Bitwise** (6.3+): `$bitAnd`, `$bitOr`, `$bitXor`, `$bitNot`
- **Boolean**: `$and`, `$or`, `$not`
- **Comparison**: `$cmp`, `$eq`, `$gt`, `$gte`, `$lt`, `$lte`, `$ne`
- **Conditional**: `$cond`, `$ifNull`, `$switch`
- **Date**: `$dateAdd`, `$dateDiff`, `$dateFromParts`, `$dateFromString`, `$dateSubtract`, `$dateToParts`, `$dateToString`, `$dateTrunc`, `$dayOfMonth`, `$dayOfWeek`, `$dayOfYear`, `$hour`, `$minute`, `$month`, `$second`, `$week`, `$year`
- **String**: `$concat`, `$indexOfBytes`, `$indexOfCP`, `$ltrim`, `$regexFind`, `$regexFindAll`, `$regexMatch`, `$replaceOne`, `$replaceAll`, `$rtrim`, `$split`, `$strcasecmp`, `$strLenBytes`, `$strLenCP`, `$substr`, `$substrBytes`, `$substrCP`, `$toLower`, `$toString`, `$trim`, `$toUpper`
- **Type**: `$convert`, `$toBinData` (8.0+), `$toBool`, `$toDecimal`, `$toDouble`, `$toInt`, `$toLong`, `$toObjectId`, `$toUUID` (8.0+), `$type`

### Accumulators
`$avg`, `$bottom`, `$bottomN`, `$count`, `$first`, `$firstN`, `$last`, `$lastN`, `$max`, `$maxN`, `$median` (7.0+), `$min`, `$minN`, `$percentile` (7.0+), `$push`, `$stdDevPop`, `$stdDevSamp`, `$sum`, `$top`, `$topN`

### Window Functions
`$denseRank`, `$derivative`, `$documentNumber`, `$expMovingAvg`, `$integral`, `$linearFill`, `$locf`, `$rank`, `$shift`

### BSON Types
`ObjectId`, `ISODate`, `Date`, `NumberInt`, `NumberLong`, `NumberDecimal`, `Timestamp`, `BinData`, `UUID`, `MinKey`, `MaxKey`, `DBRef`, `RegExp`, `Code`, `CodeWithScope`

## Installation & Usage

### Prerequisites
- Java 8+ (for ANTLR tool)
- ANTLR4 (version 4.13+)

### Generate Parser (Java)
```bash
# Download ANTLR if needed
curl -O https://www.antlr.org/download/antlr-4.13.1-complete.jar

# Generate lexer and parser
java -jar antlr-4.13.1-complete.jar -Dlanguage=Java MongoLexer.g4 MongoParser.g4

# Compile generated code
javac -cp .:antlr-4.13.1-complete.jar *.java
```

### Generate Parser (Python)
```bash
java -jar antlr-4.13.1-complete.jar -Dlanguage=Python3 MongoLexer.g4 MongoParser.g4
pip install antlr4-python3-runtime
```

### Generate Parser (Rust)
```bash
java -jar antlr-4.13.1-complete.jar -Dlanguage=Rust MongoLexer.g4 MongoParser.g4
# Add antlr-rust to Cargo.toml
```

### Generate Parser (TypeScript/JavaScript)
```bash
java -jar antlr-4.13.1-complete.jar -Dlanguage=TypeScript MongoLexer.g4 MongoParser.g4
npm install antlr4ts
```

### Generate Parser (C++)
```bash
java -jar antlr-4.13.1-complete.jar -Dlanguage=Cpp MongoLexer.g4 MongoParser.g4
```

## Example Usage (Python)

```python
from antlr4 import CommonTokenStream, InputStream
from MongoLexer import MongoLexer
from MongoParser import MongoParser

def parse_mongo_query(query_string: str):
    input_stream = InputStream(query_string)
    lexer = MongoLexer(input_stream)
    token_stream = CommonTokenStream(lexer)
    parser = MongoParser(token_stream)
    
    # Parse as a general document/query
    tree = parser.mongoStatement()
    return tree

# Example: Parse a find query
query = '''
{
    "name": { "$regex": "^John", "$options": "i" },
    "age": { "$gte": 21, "$lte": 65 },
    "$or": [
        { "status": "active" },
        { "role": { "$in": ["admin", "moderator"] } }
    ]
}
'''
tree = parse_mongo_query(query)

# Example: Parse an aggregation pipeline
pipeline = '''
[
    { "$match": { "status": "active" } },
    { "$group": {
        "_id": "$department",
        "total": { "$sum": 1 },
        "avgSalary": { "$avg": "$salary" }
    }},
    { "$sort": { "total": -1 } },
    { "$limit": 10 }
]
'''
tree = parse_mongo_query(pipeline)

# Example: Parse an update document
update = '''
{
    "$set": { "status": "processed", "updatedAt": ISODate() },
    "$inc": { "processCount": 1 },
    "$push": { "history": { "$each": ["event1", "event2"], "$slice": -10 } }
}
'''
tree = parse_mongo_query(update)
```

## Example Usage (Java)

```java
import org.antlr.v4.runtime.*;
import org.antlr.v4.runtime.tree.*;

public class MongoQueryParser {
    public static ParseTree parse(String input) {
        CharStream charStream = CharStreams.fromString(input);
        MongoLexer lexer = new MongoLexer(charStream);
        CommonTokenStream tokens = new CommonTokenStream(lexer);
        MongoParser parser = new MongoParser(tokens);
        return parser.mongoStatement();
    }
    
    public static void main(String[] args) {
        String query = "{ \"name\": \"John\", \"age\": { \"$gte\": 21 } }";
        ParseTree tree = parse(query);
        System.out.println(tree.toStringTree());
    }
}
```

## Example Usage (Rust)

```rust
use antlr_rust::common_token_stream::CommonTokenStream;
use antlr_rust::InputStream;

mod mongolexer;
mod mongoparser;

use mongolexer::MongoLexer;
use mongoparser::MongoParser;

fn parse_query(input: &str) {
    let input_stream = InputStream::new(input);
    let lexer = MongoLexer::new(input_stream);
    let token_stream = CommonTokenStream::new(lexer);
    let mut parser = MongoParser::new(token_stream);
    
    let tree = parser.mongoStatement();
    // Process the parse tree
}
```

## Grammar Structure

```
mongoStatement          # Entry point
├── query               # Find query document
├── aggregationPipeline # Array of pipeline stages  
├── updateDocument      # Update operation document
└── document            # General BSON document

query
├── fieldQuery          # Field-level conditions
├── logicalQuery        # $and, $or, $nor
└── evaluationQuery     # $expr, $text, $where

aggregationPipeline
└── pipelineStage[]     # Array of stage documents
    └── stageKey: stageValue

document
└── pair[]              # key: value pairs
    ├── key
    └── value
        ├── literal
        ├── document
        ├── array
        ├── fieldReference
        ├── bsonType
        └── aggregationExpression
```

## Extending the Grammar

To add new operators or expressions:

1. **Lexer (MongoLexer.g4)**: Add new token definitions
   ```antlr
   OP_NEWOPERATOR : '$newOperator' ;
   ```

2. **Parser (MongoParser.g4)**: Add grammar rules
   ```antlr
   newOperator
       : OP_NEWOPERATOR COLON value
       ;
   ```

3. Regenerate the parser with ANTLR4

## Testing

Use ANTLR's TestRig (grun) for quick testing:

```bash
# Compile
javac -cp .:antlr-4.13.1-complete.jar *.java

# Test interactively
java -cp .:antlr-4.13.1-complete.jar org.antlr.v4.gui.TestRig Mongo mongoStatement -gui

# Test with input file
java -cp .:antlr-4.13.1-complete.jar org.antlr.v4.gui.TestRig Mongo mongoStatement -tree < test_query.json
```

## Known Limitations

1. JavaScript `function()` bodies in `$where` are parsed but not fully validated
2. Some edge cases in regex literals may require escaping
3. Extended JSON format is partially supported

## License

MIT License - Feel free to use and modify for your projects.

## References

- [MongoDB Query Language Documentation](https://www.mongodb.com/docs/manual/reference/operator/)
- [MongoDB Aggregation Pipeline](https://www.mongodb.com/docs/manual/reference/operator/aggregation-pipeline/)
- [ANTLR4 Documentation](https://github.com/antlr/antlr4/blob/master/doc/index.md)
