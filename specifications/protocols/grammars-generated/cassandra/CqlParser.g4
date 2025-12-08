/*
 * CQL Parser Grammar for Apache Cassandra 5.0 and ScyllaDB
 * 
 * This parser grammar defines the complete syntax for Cassandra Query Language (CQL).
 * It supports:
 * - Apache Cassandra 5.0 features (vector types, SAI, dynamic data masking, new math functions)
 * - ScyllaDB extensions (BYPASS CACHE, USING TIMEOUT, synchronous materialized views, etc.)
 * 
 * Licensed under Apache License 2.0
 * 
 * Based on CQL specification from Apache Cassandra documentation
 * and ScyllaDB CQL extensions documentation.
 */

parser grammar CqlParser;

options { tokenVocab = CqlLexer; }

// =============================================================================
// Root Rule
// =============================================================================

root
    : cqlStatements EOF
    ;

cqlStatements
    : (cqlStatement SEMICOLON?)*
    ;

cqlStatement
    : ddlStatement
    | dmlStatement
    | secondaryIndexStatement
    | materializedViewStatement
    | roleOrPermissionStatement
    | udfStatement
    | udtStatement
    | triggerStatement
    | serviceLevelStatement      // ScyllaDB Service Levels
    ;

// =============================================================================
// DDL Statements (Data Definition Language)
// =============================================================================

ddlStatement
    : useStatement
    | createKeyspaceStatement
    | alterKeyspaceStatement
    | dropKeyspaceStatement
    | createTableStatement
    | alterTableStatement
    | dropTableStatement
    | truncateStatement
    | describeStatement
    ;

// USE Statement
useStatement
    : K_USE keyspaceName
    ;

// CREATE KEYSPACE
createKeyspaceStatement
    : K_CREATE K_KEYSPACE ifNotExists? keyspaceName
      K_WITH keyspaceProperties
    ;

// ALTER KEYSPACE
alterKeyspaceStatement
    : K_ALTER K_KEYSPACE keyspaceName
      K_WITH keyspaceProperties
    ;

// DROP KEYSPACE
dropKeyspaceStatement
    : K_DROP K_KEYSPACE ifExists? keyspaceName
    ;

keyspaceProperties
    : keyspaceProperty (K_AND keyspaceProperty)*
    ;

keyspaceProperty
    : K_REPLICATION EQ mapLiteral
    | K_DURABLE_WRITES EQ booleanLiteral
    | K_STORAGE EQ mapLiteral           // ScyllaDB S3 storage
    ;

// CREATE TABLE
createTableStatement
    : K_CREATE (K_TABLE | K_COLUMNFAMILY) ifNotExists? tableName
      LPAREN columnDefinitionList RPAREN
      (K_WITH tableOptions)?
    ;

columnDefinitionList
    : columnDefinition (COMMA columnDefinition)* (COMMA primaryKeyClause)?
    ;

columnDefinition
    : columnName dataType K_STATIC? K_PRIMARY? K_KEY?
      maskDefinition?                   // Cassandra 5.0 Dynamic Data Masking
    ;

// Cassandra 5.0 Dynamic Data Masking
maskDefinition
    : K_MASKED K_WITH maskFunction
    ;

maskFunction
    : K_DEFAULT
    | functionCall
    ;

primaryKeyClause
    : K_PRIMARY K_KEY LPAREN primaryKeyDefinition RPAREN
    ;

primaryKeyDefinition
    : partitionKey
    | partitionKey COMMA clusteringColumns
    ;

partitionKey
    : columnName
    | LPAREN columnName (COMMA columnName)* RPAREN
    ;

clusteringColumns
    : columnName (COMMA columnName)*
    ;

// ALTER TABLE
alterTableStatement
    : K_ALTER (K_TABLE | K_COLUMNFAMILY) tableName alterTableInstruction
    ;

alterTableInstruction
    : K_ALTER columnName K_TYPE dataType
    | K_ADD columnName dataType K_STATIC?
    | K_DROP columnName
    | K_DROP LPAREN columnName (COMMA columnName)* RPAREN
    | K_RENAME columnName K_TO columnName (K_AND columnName K_TO columnName)*
    | K_WITH tableOptions
    ;

// DROP TABLE
dropTableStatement
    : K_DROP (K_TABLE | K_COLUMNFAMILY) ifExists? tableName
    ;

// TRUNCATE TABLE
truncateStatement
    : K_TRUNCATE (K_TABLE | K_COLUMNFAMILY)? tableName
      (K_USING usingClauseOptions)?     // ScyllaDB USING TIMEOUT
    ;

// DESCRIBE
describeStatement
    : K_DESCRIBE K_CLUSTER
    | K_DESCRIBE K_SCHEMA (K_WITH K_INTERNALS (K_AND K_PASSWORDS)?)?  // ScyllaDB extension
    | K_DESCRIBE K_KEYSPACES
    | K_DESCRIBE K_KEYSPACE? keyspaceName?
    | K_DESCRIBE K_TABLES
    | K_DESCRIBE (K_TABLE | K_COLUMNFAMILY)? tableName
    | K_DESCRIBE K_INDEX indexName
    | K_DESCRIBE K_MATERIALIZED K_VIEW materializedViewName
    | K_DESCRIBE K_TYPE userTypeName
    | K_DESCRIBE K_FUNCTION functionName
    | K_DESCRIBE K_AGGREGATE aggregateName
    ;

// Table Options
tableOptions
    : tableOption (K_AND tableOption)*
    ;

tableOption
    : property
    | K_COMPACT K_STORAGE
    | K_CLUSTERING K_ORDER K_BY LPAREN clusteringOrder (COMMA clusteringOrder)* RPAREN
    | K_ID EQ STRING_LITERAL
    // ScyllaDB specific options
    | K_PAXOS_GRACE_SECONDS EQ INTEGER
    | K_PER_PARTITION_RATE_LIMIT EQ mapLiteral
    ;

clusteringOrder
    : columnName (K_ASC | K_DESC)
    ;

property
    : propertyName EQ propertyValue
    ;

propertyName
    : IDENT
    | unreservedKeyword
    ;

propertyValue
    : constant
    | mapLiteral
    ;

// =============================================================================
// DML Statements (Data Manipulation Language)
// =============================================================================

dmlStatement
    : selectStatement
    | insertStatement
    | updateStatement
    | deleteStatement
    | batchStatement
    ;

// SELECT Statement
selectStatement
    : K_SELECT (K_JSON)? K_DISTINCT? selectClause
      K_FROM tableName
      whereClause?
      groupByClause?
      orderByClause?
      perPartitionLimitClause?
      limitClause?
      (K_ALLOW K_FILTERING)?
      (K_BYPASS K_CACHE)?              // ScyllaDB extension
    ;

selectClause
    : STAR
    | selectElement (COMMA selectElement)*
    ;

selectElement
    : columnName (K_AS alias)?
    | functionCall (K_AS alias)?
    | K_WRITETIME LPAREN columnName RPAREN (K_AS alias)?
    | K_TTL LPAREN columnName RPAREN (K_AS alias)?
    | K_CAST LPAREN selector K_AS dataType RPAREN (K_AS alias)?
    | K_COUNT LPAREN STAR RPAREN (K_AS alias)?
    | columnName LBRACKET term RBRACKET (K_AS alias)?  // Collection element access
    | columnName DOT fieldName (K_AS alias)?          // UDT field access
    // Cassandra 5.0 Vector Search
    | K_SIMILARITY LPAREN columnName COMMA vectorLiteral RPAREN (K_AS alias)?
    ;

selector
    : columnName
    | functionCall
    | term
    ;

alias
    : IDENT
    | QUOTED_IDENT
    ;

whereClause
    : K_WHERE relationElements
    ;

relationElements
    : relationElement (K_AND relationElement)*
    ;

relationElement
    : columnName relationalOperator term
    | columnName K_IN LPAREN termList? RPAREN
    | columnName K_IN bindMarker
    | columnName K_CONTAINS (K_KEY)? term
    | columnName K_LIKE term
    | LPAREN columnName (COMMA columnName)* RPAREN relationalOperator tupleLiteral
    | LPAREN columnName (COMMA columnName)* RPAREN K_IN LPAREN tupleLiteral (COMMA tupleLiteral)* RPAREN
    | K_TOKEN LPAREN columnName (COMMA columnName)* RPAREN relationalOperator term
    | columnName LBRACKET term RBRACKET relationalOperator term  // Collection element
    | columnName DOT fieldName relationalOperator term           // UDT field
    // Cassandra 5.0 Vector Search - ANN query
    | K_ORDER K_BY columnName K_ANN K_OF vectorLiteral (K_LIMIT INTEGER)?
    ;

relationalOperator
    : EQ
    | LT
    | GT
    | LE
    | GE
    | NE
    ;

groupByClause
    : K_GROUP K_BY columnName (COMMA columnName)*
    ;

orderByClause
    : K_ORDER K_BY orderingElement (COMMA orderingElement)*
    ;

orderingElement
    : columnName (K_ASC | K_DESC)?
    ;

perPartitionLimitClause
    : K_PER K_PARTITION K_LIMIT INTEGER
    ;

limitClause
    : K_LIMIT INTEGER
    ;

// INSERT Statement
insertStatement
    : K_INSERT K_INTO tableName
      ( insertColumnsAndValues | insertJson )
      ifNotExists?
      (K_USING usingClauseOptions)?
    ;

insertColumnsAndValues
    : LPAREN columnName (COMMA columnName)* RPAREN
      K_VALUES LPAREN term (COMMA term)* RPAREN
    ;

insertJson
    : K_JSON STRING_LITERAL (K_DEFAULT K_UNSET | K_DEFAULT K_NULL)?
    ;

// UPDATE Statement
updateStatement
    : K_UPDATE tableName
      (K_USING usingClauseOptions)?
      K_SET assignmentElement (COMMA assignmentElement)*
      whereClause
      (ifCondition)?
    ;

assignmentElement
    : columnName EQ term
    | columnName EQ columnName (PLUS | MINUS) term
    | columnName LBRACKET term RBRACKET EQ term
    | columnName DOT fieldName EQ term
    | columnName EQ columnName PLUS collectionLiteral
    | columnName EQ collectionLiteral PLUS columnName
    | columnName EQ columnName MINUS collectionLiteral
    // ScyllaDB internal functions for sstableloader
    | columnName LBRACKET K_SCYLLA_TIMEUUID_LIST_INDEX LPAREN term RPAREN RBRACKET EQ term
    | columnName EQ K_SCYLLA_COUNTER_SHARD_LIST LPAREN term RPAREN
    ;

// DELETE Statement
deleteStatement
    : K_DELETE deleteSelection?
      K_FROM tableName
      (K_USING usingClauseOptions)?
      whereClause
      (ifCondition)?
    ;

deleteSelection
    : deleteElement (COMMA deleteElement)*
    ;

deleteElement
    : columnName
    | columnName LBRACKET term RBRACKET
    | columnName DOT fieldName
    ;

// BATCH Statement
batchStatement
    : K_BEGIN (K_UNLOGGED | K_COUNTER)? K_BATCH
      (K_USING usingClauseOptions)?
      batchStatementElement*
      K_APPLY K_BATCH
    ;

batchStatementElement
    : (insertStatement | updateStatement | deleteStatement) SEMICOLON?
    ;

// USING Clause Options
usingClauseOptions
    : usingClauseOption (K_AND usingClauseOption)*
    ;

usingClauseOption
    : K_TIMESTAMP term
    | K_TTL term
    | K_TIMEOUT term        // ScyllaDB extension
    ;

// IF Condition (LWT - Lightweight Transactions)
ifCondition
    : K_IF ifConditionElement (K_AND ifConditionElement)*
    | K_IF K_EXISTS
    | K_IF K_NOT K_EXISTS
    ;

ifConditionElement
    : columnName relationalOperator term
    | columnName K_IN LPAREN termList RPAREN
    | columnName LBRACKET term RBRACKET relationalOperator term
    ;

ifNotExists
    : K_IF K_NOT K_EXISTS
    ;

ifExists
    : K_IF K_EXISTS
    ;

// =============================================================================
// Secondary Index Statements
// =============================================================================

secondaryIndexStatement
    : createIndexStatement
    | dropIndexStatement
    ;

// CREATE INDEX
createIndexStatement
    : K_CREATE K_CUSTOM? K_INDEX ifNotExists?
      indexName? K_ON tableName LPAREN indexTarget RPAREN
      (K_USING STRING_LITERAL)?
      (K_WITH K_OPTIONS EQ mapLiteral)?
    ;

indexTarget
    : columnName
    | K_KEYS LPAREN columnName RPAREN
    | K_VALUES LPAREN columnName RPAREN
    | K_ENTRIES LPAREN columnName RPAREN
    | K_FULL LPAREN columnName RPAREN
    ;

// DROP INDEX
dropIndexStatement
    : K_DROP K_INDEX ifExists? indexName
    ;

// =============================================================================
// Materialized View Statements
// =============================================================================

materializedViewStatement
    : createMaterializedViewStatement
    | alterMaterializedViewStatement
    | dropMaterializedViewStatement
    | pruneMaterializedViewStatement    // ScyllaDB extension
    ;

// CREATE MATERIALIZED VIEW
createMaterializedViewStatement
    : K_CREATE K_MATERIALIZED K_VIEW ifNotExists? materializedViewName
      K_AS selectStatement
      K_PRIMARY K_KEY LPAREN primaryKeyDefinition RPAREN
      (K_WITH materializedViewOptions)?
    ;

// ALTER MATERIALIZED VIEW
alterMaterializedViewStatement
    : K_ALTER K_MATERIALIZED K_VIEW materializedViewName
      K_WITH materializedViewOptions
    ;

// DROP MATERIALIZED VIEW
dropMaterializedViewStatement
    : K_DROP K_MATERIALIZED K_VIEW ifExists? materializedViewName
    ;

// PRUNE MATERIALIZED VIEW (ScyllaDB extension)
pruneMaterializedViewStatement
    : K_PRUNE K_MATERIALIZED K_VIEW materializedViewName
      whereClause?
    ;

materializedViewOptions
    : materializedViewOption (K_AND materializedViewOption)*
    ;

materializedViewOption
    : tableOption
    | K_SYNCHRONOUS_UPDATES EQ booleanLiteral    // ScyllaDB extension
    ;

// =============================================================================
// Role and Permission Statements
// =============================================================================

roleOrPermissionStatement
    : createRoleStatement
    | alterRoleStatement
    | dropRoleStatement
    | grantRoleStatement
    | revokeRoleStatement
    | listRolesStatement
    | grantPermissionStatement
    | revokePermissionStatement
    | listPermissionsStatement
    | createUserStatement
    | alterUserStatement
    | dropUserStatement
    | listUsersStatement
    ;

// CREATE ROLE
createRoleStatement
    : K_CREATE K_ROLE ifNotExists? roleName
      (K_WITH roleOptions)?
    ;

// ALTER ROLE
alterRoleStatement
    : K_ALTER K_ROLE roleName
      K_WITH roleOptions
    ;

// DROP ROLE
dropRoleStatement
    : K_DROP K_ROLE ifExists? roleName
    ;

// GRANT ROLE
grantRoleStatement
    : K_GRANT roleName K_TO roleName
    ;

// REVOKE ROLE
revokeRoleStatement
    : K_REVOKE roleName K_FROM roleName
    ;

// LIST ROLES
listRolesStatement
    : K_LIST K_ROLES (K_OF roleName)? K_NORECURSIVE?
    ;

roleOptions
    : roleOption (K_AND roleOption)*
    ;

roleOption
    : K_PASSWORD EQ STRING_LITERAL
    | K_LOGIN EQ booleanLiteral
    | K_SUPERUSER EQ booleanLiteral
    | K_OPTIONS EQ mapLiteral
    ;

// GRANT PERMISSION
grantPermissionStatement
    : K_GRANT permissionOrAll K_ON resource K_TO roleName
    ;

// REVOKE PERMISSION
revokePermissionStatement
    : K_REVOKE permissionOrAll K_ON resource K_FROM roleName
    ;

// LIST PERMISSIONS
listPermissionsStatement
    : K_LIST permissionOrAll (K_ON resource)? (K_OF roleName)? K_NORECURSIVE?
    ;

permissionOrAll
    : permission
    | K_ALL K_PERMISSIONS?
    ;

permission
    : K_CREATE
    | K_ALTER
    | K_DROP
    | K_SELECT
    | K_MODIFY
    | K_AUTHORIZE
    | K_DESCRIBE
    | K_EXECUTE
    | K_UNMASK        // Cassandra 5.0
    | K_SELECT_MASKED // Cassandra 5.0
    ;

resource
    : K_ALL K_KEYSPACES
    | K_KEYSPACE keyspaceName
    | K_TABLE? tableName
    | K_ALL K_ROLES
    | K_ROLE roleName
    | K_ALL K_FUNCTIONS (K_IN K_KEYSPACE keyspaceName)?
    | K_FUNCTION functionName
    | K_ALL K_MBEANS
    | K_MBEAN STRING_LITERAL
    | K_MBEANS STRING_LITERAL
    ;

// Legacy User Statements (deprecated, use Roles instead)
createUserStatement
    : K_CREATE K_USER ifNotExists? userName
      (K_WITH K_PASSWORD STRING_LITERAL)?
      (K_SUPERUSER | K_NOSUPERUSER)?
    ;

alterUserStatement
    : K_ALTER K_USER userName
      (K_WITH K_PASSWORD STRING_LITERAL)?
      (K_SUPERUSER | K_NOSUPERUSER)?
    ;

dropUserStatement
    : K_DROP K_USER ifExists? userName
    ;

listUsersStatement
    : K_LIST K_USERS
    ;

// =============================================================================
// User-Defined Function (UDF) Statements
// =============================================================================

udfStatement
    : createFunctionStatement
    | dropFunctionStatement
    | createAggregateStatement
    | dropAggregateStatement
    ;

// CREATE FUNCTION
createFunctionStatement
    : K_CREATE (K_OR K_REPLACE)? K_FUNCTION ifNotExists? functionName
      LPAREN (functionParameter (COMMA functionParameter)*)? RPAREN
      (K_CALLED | K_RETURNS K_NULL) K_ON K_NULL K_INPUT
      K_RETURNS dataType
      K_LANGUAGE IDENT
      K_AS (STRING_LITERAL | DOLLAR_STRING)
    ;

functionParameter
    : columnName dataType
    ;

// DROP FUNCTION
dropFunctionStatement
    : K_DROP K_FUNCTION ifExists? functionName
      (LPAREN (dataType (COMMA dataType)*)? RPAREN)?
    ;

// CREATE AGGREGATE
createAggregateStatement
    : K_CREATE (K_OR K_REPLACE)? K_AGGREGATE ifNotExists? aggregateName
      LPAREN dataType RPAREN
      K_SFUNC functionName
      K_STYPE dataType
      (K_REDUCEFUNC functionName)?     // ScyllaDB extension
      (K_FINALFUNC functionName)?
      (K_INITCOND term)?
    ;

// DROP AGGREGATE
dropAggregateStatement
    : K_DROP K_AGGREGATE ifExists? aggregateName
      (LPAREN dataType? RPAREN)?
    ;

// =============================================================================
// User-Defined Type (UDT) Statements
// =============================================================================

udtStatement
    : createTypeStatement
    | alterTypeStatement
    | dropTypeStatement
    ;

// CREATE TYPE
createTypeStatement
    : K_CREATE K_TYPE ifNotExists? userTypeName
      LPAREN fieldDefinition (COMMA fieldDefinition)* RPAREN
    ;

fieldDefinition
    : fieldName dataType
    ;

// ALTER TYPE
alterTypeStatement
    : K_ALTER K_TYPE userTypeName alterTypeInstruction
    ;

alterTypeInstruction
    : K_ALTER fieldName K_TYPE dataType
    | K_ADD fieldName dataType
    | K_RENAME fieldName K_TO fieldName (K_AND fieldName K_TO fieldName)*
    ;

// DROP TYPE
dropTypeStatement
    : K_DROP K_TYPE ifExists? userTypeName
    ;

// =============================================================================
// Trigger Statements
// =============================================================================

triggerStatement
    : createTriggerStatement
    | dropTriggerStatement
    ;

// CREATE TRIGGER
createTriggerStatement
    : K_CREATE K_TRIGGER ifNotExists? triggerName
      K_ON tableName
      K_USING STRING_LITERAL
    ;

// DROP TRIGGER
dropTriggerStatement
    : K_DROP K_TRIGGER ifExists? triggerName K_ON tableName
    ;

// =============================================================================
// ScyllaDB Service Level Statements
// =============================================================================

serviceLevelStatement
    : createServiceLevelStatement
    | alterServiceLevelStatement
    | dropServiceLevelStatement
    | attachServiceLevelStatement
    | detachServiceLevelStatement
    | listServiceLevelsStatement
    | listEffectiveServiceLevelStatement
    ;

createServiceLevelStatement
    : K_CREATE K_SERVICE K_LEVEL ifNotExists? serviceLevelName
      (K_WITH serviceLevelOptions)?
    ;

alterServiceLevelStatement
    : K_ALTER K_SERVICE K_LEVEL serviceLevelName
      K_WITH serviceLevelOptions
    ;

dropServiceLevelStatement
    : K_DROP K_SERVICE K_LEVEL ifExists? serviceLevelName
    ;

attachServiceLevelStatement
    : K_ATTACH K_SERVICE K_LEVEL serviceLevelName K_TO roleName
    ;

detachServiceLevelStatement
    : K_DETACH K_SERVICE K_LEVEL K_FROM roleName
    ;

listServiceLevelsStatement
    : K_LIST K_ALL? K_SERVICE K_LEVELS
    ;

listEffectiveServiceLevelStatement
    : K_LIST K_EFFECTIVE K_SERVICE K_LEVEL K_OF roleName
    ;

serviceLevelName
    : IDENT
    | QUOTED_IDENT
    ;

serviceLevelOptions
    : serviceLevelOption (K_AND serviceLevelOption)*
    ;

serviceLevelOption
    : K_TIMEOUT EQ term
    | K_WORKLOAD_TYPE EQ STRING_LITERAL
    | property
    ;

// =============================================================================
// Data Types
// =============================================================================

dataType
    : nativeType
    | collectionType
    | tupleType
    | userDefinedType
    | frozenType
    | vectorType           // Cassandra 5.0
    ;

nativeType
    : K_ASCII
    | K_BIGINT
    | K_BLOB
    | K_BOOLEAN
    | K_COUNTER
    | K_DATE
    | K_DECIMAL
    | K_DOUBLE
    | K_DURATION
    | K_FLOAT
    | K_INET
    | K_INT
    | K_SMALLINT
    | K_TEXT
    | K_TIME
    | K_TIMESTAMP
    | K_TIMEUUID
    | K_TINYINT
    | K_UUID
    | K_VARCHAR
    | K_VARINT
    ;

collectionType
    : K_LIST LT dataType GT
    | K_SET LT dataType GT
    | K_MAP LT dataType COMMA dataType GT
    ;

tupleType
    : K_TUPLE LT dataType (COMMA dataType)* GT
    ;

userDefinedType
    : (keyspaceName DOT)? IDENT
    ;

frozenType
    : K_FROZEN LT (collectionType | tupleType | userDefinedType) GT
    ;

// Cassandra 5.0 Vector Type
vectorType
    : K_VECTOR LT nativeType COMMA INTEGER GT
    ;

// =============================================================================
// Terms and Expressions
// =============================================================================

term
    : constant
    | collectionLiteral
    | udtLiteral
    | tupleLiteral
    | vectorLiteral        // Cassandra 5.0
    | functionCall
    | arithmeticOperation
    | typeHint
    | bindMarker
    ;

constant
    : STRING_LITERAL
    | INTEGER
    | FLOAT
    | HEXNUMBER
    | booleanLiteral
    | UUID
    | DURATION
    | K_NULL
    | K_NAN
    | K_INFINITY
    | K_NEGATIVE_NAN
    | K_NEGATIVE_INFINITY
    ;

booleanLiteral
    : K_TRUE
    | K_FALSE
    ;

collectionLiteral
    : listLiteral
    | setLiteral
    | mapLiteral
    ;

listLiteral
    : LBRACKET (term (COMMA term)*)? RBRACKET
    ;

setLiteral
    : LBRACE term (COMMA term)* RBRACE
    ;

mapLiteral
    : LBRACE (mapEntry (COMMA mapEntry)*)? RBRACE
    ;

mapEntry
    : term COLON term
    ;

udtLiteral
    : LBRACE fieldAssignment (COMMA fieldAssignment)* RBRACE
    ;

fieldAssignment
    : fieldName COLON term
    ;

tupleLiteral
    : LPAREN term (COMMA term)* RPAREN
    ;

// Cassandra 5.0 Vector Literal
vectorLiteral
    : LBRACKET (FLOAT | INTEGER) (COMMA (FLOAT | INTEGER))* RBRACKET
    ;

termList
    : term (COMMA term)*
    ;

functionCall
    : functionName LPAREN (term (COMMA term)*)? RPAREN
    | K_TOKEN LPAREN term (COMMA term)* RPAREN
    | K_CAST LPAREN term K_AS dataType RPAREN
    // Cassandra 5.0 Math functions
    | K_ABS LPAREN term RPAREN
    | K_EXP LPAREN term RPAREN
    | K_LOG LPAREN term RPAREN
    | K_LOG10 LPAREN term RPAREN
    | K_ROUND LPAREN term RPAREN
    // Cassandra 5.0 Vector similarity functions
    | K_SIMILARITY LPAREN term COMMA term RPAREN
    | K_COSINE LPAREN term COMMA term RPAREN
    | K_DOT_PRODUCT LPAREN term COMMA term RPAREN
    | K_EUCLIDEAN LPAREN term COMMA term RPAREN
    ;

arithmeticOperation
    : MINUS term
    | term (PLUS | MINUS | STAR | SLASH | PERCENT) term
    ;

typeHint
    : LPAREN dataType RPAREN term
    ;

bindMarker
    : QMARK
    | NAMED_BIND_MARKER
    ;

// =============================================================================
// Names and Identifiers
// =============================================================================

keyspaceName
    : IDENT
    | QUOTED_IDENT
    | unreservedKeyword
    ;

tableName
    : (keyspaceName DOT)? tableIdentifier
    ;

tableIdentifier
    : IDENT
    | QUOTED_IDENT
    | unreservedKeyword
    ;

columnName
    : IDENT
    | QUOTED_IDENT
    | unreservedKeyword
    ;

fieldName
    : IDENT
    | QUOTED_IDENT
    ;

indexName
    : (keyspaceName DOT)? IDENT
    | (keyspaceName DOT)? QUOTED_IDENT
    ;

materializedViewName
    : (keyspaceName DOT)? IDENT
    | (keyspaceName DOT)? QUOTED_IDENT
    ;

functionName
    : (keyspaceName DOT)? IDENT
    | (keyspaceName DOT)? QUOTED_IDENT
    ;

aggregateName
    : (keyspaceName DOT)? IDENT
    | (keyspaceName DOT)? QUOTED_IDENT
    ;

userTypeName
    : (keyspaceName DOT)? IDENT
    | (keyspaceName DOT)? QUOTED_IDENT
    ;

roleName
    : IDENT
    | QUOTED_IDENT
    | STRING_LITERAL
    ;

userName
    : IDENT
    | QUOTED_IDENT
    | STRING_LITERAL
    ;

triggerName
    : IDENT
    | QUOTED_IDENT
    ;

// =============================================================================
// Unreserved Keywords (can be used as identifiers)
// =============================================================================

unreservedKeyword
    : K_AGGREGATE
    | K_ALL
    | K_AS
    | K_ASCII
    | K_BIGINT
    | K_BLOB
    | K_BOOLEAN
    | K_CALLED
    | K_CAST
    | K_CLUSTERING
    | K_COMPACT
    | K_CONTAINS
    | K_COUNT
    | K_COUNTER
    | K_CUSTOM
    | K_DATE
    | K_DECIMAL
    | K_DEFAULT
    | K_DISTINCT
    | K_DOUBLE
    | K_DURATION
    | K_EXISTS
    | K_FILTERING
    | K_FINALFUNC
    | K_FLOAT
    | K_FROZEN
    | K_FUNCTION
    | K_FUNCTIONS
    | K_GROUP
    | K_INET
    | K_INITCOND
    | K_INPUT
    | K_INT
    | K_JSON
    | K_KEY
    | K_KEYS
    | K_KEYSPACES
    | K_LANGUAGE
    | K_LIKE
    | K_LIST
    | K_LOGIN
    | K_MAP
    | K_NOLOGIN
    | K_NOSUPERUSER
    | K_OPTIONS
    | K_PARTITION
    | K_PASSWORD
    | K_PER
    | K_PERMISSION
    | K_PERMISSIONS
    | K_RETURNS
    | K_ROLE
    | K_ROLES
    | K_SFUNC
    | K_SMALLINT
    | K_STATIC
    | K_STORAGE
    | K_STYPE
    | K_SUPERUSER
    | K_TABLES
    | K_TEXT
    | K_TIME
    | K_TIMESTAMP
    | K_TIMEUUID
    | K_TINYINT
    | K_TRIGGER
    | K_TTL
    | K_TUPLE
    | K_TYPE
    | K_USER
    | K_USERS
    | K_UUID
    | K_VALUES
    | K_VARCHAR
    | K_VARINT
    | K_WRITETIME
    // Cassandra 5.0
    | K_MASK
    | K_MASKED
    | K_VECTOR
    | K_ANN
    | K_SIMILARITY
    // ScyllaDB extensions
    | K_BYPASS
    | K_CACHE
    | K_TIMEOUT
    | K_PRUNE
    | K_SYNCHRONOUS_UPDATES
    | K_REDUCEFUNC
    | K_SERVICE
    | K_LEVEL
    | K_EFFECTIVE
    ;

// Missing keywords used in some contexts
K_COUNT: C O U N T;
K_ATTACH: A T T A C H;
K_DETACH: D E T A C H;
K_LEVELS: L E V E L S;
