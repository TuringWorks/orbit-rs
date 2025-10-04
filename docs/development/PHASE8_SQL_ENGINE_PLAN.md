# Phase 8: SQL Query Engine Implementation Plan

## Overview

With the comprehensive SQL expression parser now complete, Phase 8 focuses on implementing the core DML (Data Manipulation Language) operations to provide a fully functional SQL query engine. This builds directly on the foundation of:

- ✅ Complete SQL lexer with all PostgreSQL tokens
- ✅ Comprehensive AST for SQL constructs
- ✅ Expression parser with proper operator precedence
- ✅ Vector operations support (pgvector compatible)
- ✅ DDL operations (CREATE/ALTER/DROP TABLE, INDEX, etc.)

## Architecture

```
PostgreSQL Wire Protocol
├── SQL Parser
│   ├── Lexer (✅ Complete)
│   ├── AST (✅ Complete) 
│   ├── Expression Parser (✅ Complete)
│   └── Query Parser (🚧 Phase 8)
│       ├── SELECT Parser
│       ├── INSERT Parser
│       ├── UPDATE Parser
│       └── DELETE Parser
├── Query Executor
│   ├── SELECT Executor
│   ├── INSERT Executor
│   ├── UPDATE Executor
│   └── DELETE Executor
└── Actor Integration
    ├── Table Actor Mapping
    ├── Data Storage Interface
    └── Transaction Coordination
```

## Implementation Strategy

### 1. SELECT Statement Implementation

**Priority: HIGH (Foundation for all other operations)**

**Components:**
- SELECT clause parsing (column projection)
- FROM clause parsing (table references)
- WHERE clause integration (using existing expression parser)
- ORDER BY clause parsing
- LIMIT/OFFSET clause parsing
- Basic JOIN support (INNER JOIN initially)

**Files to create/modify:**
- `orbit-protocols/src/postgres_wire/sql/parser/select.rs`
- `orbit-protocols/src/postgres_wire/sql/executor/select.rs`

**AST Extensions needed:**
```rust
pub struct SelectStatement {
    pub select_list: Vec<SelectItem>,
    pub from_clause: Option<FromClause>,
    pub where_clause: Option<Expression>,
    pub order_by: Vec<OrderByExpression>,
    pub limit: Option<LimitClause>,
}

pub enum SelectItem {
    Wildcard,
    Expression { expr: Expression, alias: Option<String> },
}

pub struct FromClause {
    pub table_name: String,
    pub alias: Option<String>,
    pub joins: Vec<JoinClause>,
}
```

### 2. INSERT Statement Implementation

**Priority: HIGH (Basic data modification)**

**Components:**
- INSERT INTO parsing
- VALUES clause parsing
- Column list specification
- Type validation and conversion
- Batch insert support
- RETURNING clause support

**Files to create/modify:**
- `orbit-protocols/src/postgres_wire/sql/parser/insert.rs`
- `orbit-protocols/src/postgres_wire/sql/executor/insert.rs`

**AST Extensions:**
```rust
pub struct InsertStatement {
    pub table_name: String,
    pub columns: Option<Vec<String>>,
    pub values: InsertValues,
    pub returning: Option<Vec<SelectItem>>,
}

pub enum InsertValues {
    Values(Vec<Vec<Expression>>),
    Select(Box<SelectStatement>),
}
```

### 3. UPDATE Statement Implementation

**Priority: MEDIUM (Data modification with conditions)**

**Components:**
- UPDATE clause parsing
- SET clause parsing (column = expression pairs)
- WHERE clause integration
- JOIN support in UPDATE
- RETURNING clause support

**Files to create/modify:**
- `orbit-protocols/src/postgres_wire/sql/parser/update.rs`
- `orbit-protocols/src/postgres_wire/sql/executor/update.rs`

**AST Extensions:**
```rust
pub struct UpdateStatement {
    pub table_name: String,
    pub set_clauses: Vec<SetClause>,
    pub from_clause: Option<FromClause>,
    pub where_clause: Option<Expression>,
    pub returning: Option<Vec<SelectItem>>,
}

pub struct SetClause {
    pub column: String,
    pub value: Expression,
}
```

### 4. DELETE Statement Implementation

**Priority: MEDIUM (Data deletion with conditions)**

**Components:**
- DELETE FROM parsing
- WHERE clause integration
- JOIN support in DELETE
- RETURNING clause support

**Files to create/modify:**
- `orbit-protocols/src/postgres_wire/sql/parser/delete.rs`
- `orbit-protocols/src/postgres_wire/sql/executor/delete.rs`

**AST Extensions:**
```rust
pub struct DeleteStatement {
    pub table_name: String,
    pub where_clause: Option<Expression>,
    pub returning: Option<Vec<SelectItem>>,
}
```

## Query Executor Architecture

### Data Storage Interface

```rust
pub trait DataStorage {
    async fn create_table(&self, table: &TableDefinition) -> Result<(), StorageError>;
    async fn select(&self, query: &SelectQuery) -> Result<QueryResult, StorageError>;
    async fn insert(&self, table: &str, rows: Vec<Row>) -> Result<InsertResult, StorageError>;
    async fn update(&self, query: &UpdateQuery) -> Result<UpdateResult, StorageError>;
    async fn delete(&self, query: &DeleteQuery) -> Result<DeleteResult, StorageError>;
}

pub struct QueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Row>,
    pub row_count: usize,
}

pub struct Row {
    pub values: Vec<SqlValue>,
}
```

### Actor Integration Strategy

1. **Table Actor Mapping**: Each SQL table maps to an Orbit actor type
2. **Collection Actors**: Manage collections of data with CRUD operations
3. **Query Coordination**: Distribute queries across multiple actors when needed
4. **Transaction Integration**: Use existing transaction system for ACID compliance

## Implementation Timeline

### Week 1: SELECT Foundation
- [ ] Implement basic SELECT parser
- [ ] Create query executor interface  
- [ ] Implement simple SELECT execution (no JOINs)
- [ ] Add WHERE clause integration using expression parser
- [ ] Basic testing and validation

### Week 2: INSERT & UPDATE
- [ ] Implement INSERT parser and executor
- [ ] Add type validation and conversion
- [ ] Implement UPDATE parser and executor
- [ ] Add comprehensive error handling
- [ ] Integration testing with actor system

### Week 3: DELETE & Advanced Features
- [ ] Implement DELETE parser and executor
- [ ] Add RETURNING clause support across all operations
- [ ] Implement basic JOIN support (INNER JOIN)
- [ ] Add ORDER BY and LIMIT/OFFSET support
- [ ] Performance optimization and testing

## Testing Strategy

### Unit Tests
- Parser tests for each DML operation
- Expression integration tests
- Type validation tests
- Error handling tests

### Integration Tests
- End-to-end SQL query execution
- Actor system integration
- Transaction coordination
- PostgreSQL wire protocol compatibility

### Performance Tests
- Query execution benchmarks
- Memory usage analysis
- Concurrency testing
- Large dataset handling

## Success Criteria

1. **Basic CRUD Operations**: All four DML operations (SELECT, INSERT, UPDATE, DELETE) fully functional
2. **Expression Integration**: WHERE clauses using the complete expression parser
3. **PostgreSQL Compatibility**: Wire protocol compatibility with psql and other PostgreSQL clients
4. **Actor Integration**: Seamless integration with Orbit's actor system
5. **Performance**: Reasonable performance for medium-scale datasets (10K-100K rows)
6. **Testing**: Comprehensive test coverage (>90%) with integration tests

## Next Steps After Phase 8

1. **JOIN Operations**: Full JOIN support (LEFT, RIGHT, FULL OUTER)
2. **Aggregate Functions**: COUNT, SUM, AVG, MIN, MAX, GROUP BY
3. **Subqueries**: Correlated and non-correlated subqueries
4. **Advanced SQL Features**: CTEs, window functions, stored procedures

This plan provides a solid foundation for implementing a production-ready SQL query engine that leverages Orbit's distributed actor architecture while maintaining PostgreSQL compatibility.