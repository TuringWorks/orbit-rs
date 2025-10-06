# OrbitQL - Unified Multi-Model Query Language

OrbitQL is a modern, unified query language for orbit-rs that supports querying across multiple data models (documents, graphs, time-series, and key-value stores) in a single, expressive SQL-like syntax.

## Features

### ✅ Implemented (Phase 20.1)
- **Core SQL Operations**: SELECT, INSERT, UPDATE, DELETE with WHERE, ORDER BY, LIMIT, GROUP BY
- **Advanced SQL**: JOINs (INNER, LEFT, RIGHT, FULL, CROSS), subqueries, aggregations
- **Transactions**: BEGIN, COMMIT, ROLLBACK support
- **Data Types**: String, Integer, Float, Boolean, NULL, DateTime, UUID, Arrays, Objects
- **Parameters**: Support for parameterized queries (`$param`, `@param`)
- **Query Planning**: Cost-based query optimization with execution plans
- **Live Queries**: LIVE SELECT for real-time subscriptions
- **Comprehensive Parser**: Full lexer, parser, AST, optimizer, and planner modules
- **Error Handling**: Detailed error messages with line/column information

### 🚧 Planned (Future Phases)
- **Graph Traversal**: `->`/`<-` operators, path expressions, RELATE statements
- **Time-Series**: `metrics[...]` notation, time windows, aggregation functions
- **Schema Management**: CREATE TABLE, INDEX, VIEW statements
- **Advanced Features**: Full-text search, geospatial queries, AI/ML functions

## Architecture

OrbitQL follows a traditional database query processing pipeline:

```
Query String → Lexer → Parser → Optimizer → Planner → Executor
```

### Components

1. **Lexer** (`lexer.rs`): Tokenizes query strings into tokens
2. **Parser** (`parser.rs`): Converts tokens into an Abstract Syntax Tree (AST)
3. **AST** (`ast.rs`): Defines the query representation structures
4. **Optimizer** (`optimizer.rs`): Applies optimization rules (constant folding, predicate pushdown, etc.)
5. **Planner** (`planner.rs`): Creates execution plans with cost estimation
6. **Executor** (`executor.rs`): Executes query plans against data stores

## Usage

### Basic Example

```rust
use orbit_shared::orbitql::{OrbitQLEngine, QueryParams, QueryContext};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = OrbitQLEngine::new();
    
    // Basic document query
    let query = "SELECT name, age FROM users WHERE age > $min_age ORDER BY name";
    let params = QueryParams::new().set("min_age", 18);
    let context = QueryContext::default();
    
    // Validate query syntax
    engine.validate(query)?;
    
    // Execute query (stub implementation)
    let result = engine.execute(query, params, context).await?;
    
    Ok(())
}
```

### Multi-Model Query Examples

```sql
-- Document query with filtering
SELECT name, email, profile.bio 
FROM users 
WHERE age > 21 AND city = 'San Francisco'
ORDER BY created_at DESC
LIMIT 10;

-- Parameterized query
SELECT * FROM posts WHERE author_id = $user_id AND created_at > $since;

-- Transaction
BEGIN;
INSERT INTO users { name: "Alice", email: "alice@example.com" };
UPDATE user_stats SET total_users = total_users + 1;
COMMIT;

-- Live query for real-time updates
LIVE SELECT post.title, COUNT(*) as like_count
FROM posts post
WHERE post.created_at > NOW() - 1h
GROUP BY post.id;
```

### Planned Advanced Syntax

```sql
-- Graph traversal (planned)
SELECT 
    user.name,
    ->follows->user.name AS friends,
    <-followed_by<-user.name AS followers
FROM users user
WHERE user.active = true;

-- Time-series queries (planned)
SELECT 
    user.name,
    metrics[cpu_usage WHERE timestamp > NOW() - 1h] AS recent_cpu,
    AVG(metrics[memory_usage WHERE timestamp > NOW() - 24h]) AS avg_memory
FROM users user;

-- Multi-model relationships (planned)
RELATE $user1 ->friends $user2 SET since = NOW();
```

## Query Value Types

OrbitQL supports rich data types:

```rust
use orbit_shared::orbitql::QueryValue;

let values = vec![
    QueryValue::String("Hello".to_string()),
    QueryValue::Integer(42),
    QueryValue::Float(3.14159),
    QueryValue::Boolean(true),
    QueryValue::Null,
    QueryValue::DateTime(chrono::Utc::now()),
    QueryValue::Uuid(uuid::Uuid::new_v4()),
    QueryValue::Array(vec![...]),
    QueryValue::Object(HashMap::new()),
];
```

## Error Handling

OrbitQL provides detailed error information:

```rust
match engine.validate("SELECT * FORM users") {
    Err(ParseError::UnexpectedToken { expected, found }) => {
        println!("Syntax error: expected {:?}, found {} at line {}, column {}", 
            expected, found.value, found.line, found.column);
    },
    _ => {}
}
```

## Testing

Run the OrbitQL test suite:

```bash
cargo test orbitql
```

See the example:

```bash
cargo run --bin orbitql-example
```

## Optimization Rules

Current optimization rules include:

- **Constant Folding**: Evaluate constant expressions at parse time
- **Predicate Pushdown**: Push WHERE conditions closer to data sources
- **Projection Pushdown**: Minimize data transfer by selecting only needed columns
- **Join Reordering**: Optimize join order based on cost estimates
- **Index Selection**: Choose optimal indexes for query execution

## Future Enhancements

1. **Advanced Graph Operations**
   - Multi-hop traversals: `->{1,5}->`
   - Recursive patterns: `->*`
   - Variable-length paths

2. **Time-Series Extensions**
   - Window functions: `WINDOW 1h SLIDE 15m`
   - Interpolation: `INTERPOLATE LINEAR`
   - Forecasting: `PREDICT NEXT 24h`

3. **AI/ML Integration**
   - Vector similarity: `SIMILARITY(embedding, $query_vector)`
   - Model inference: `PREDICT sentiment FROM text USING model`

4. **Performance Features**
   - Query result caching
   - Materialized views
   - Distributed execution across nodes

## Contributing

OrbitQL is part of the orbit-rs project. To contribute:

1. Focus on implementing missing AST node handling in the parser
2. Add new optimization rules in the optimizer
3. Extend the executor with actual data store integration
4. Improve error messages and debugging tools

See the main orbit-rs contributing guidelines for more details.

## License

OrbitQL is licensed under the same terms as orbit-rs: BSD-3-Clause OR MIT.