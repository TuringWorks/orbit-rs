/*
 * OrbitQL Parser Grammar for ANTLR4
 *
 * This grammar covers the OrbitQL unified multi-model query language for Orbit-RS.
 * OrbitQL combines SQL with graph traversal, time-series analytics, machine learning,
 * and spatial operations for distributed actor systems.
 *
 * Key Features:
 * - SQL-compatible syntax (SELECT, INSERT, UPDATE, DELETE)
 * - Graph operations (TRAVERSE, RELATE)
 * - Time-series functions (NOW, INTERVAL, TIME_BUCKET)
 * - Machine learning functions (ML_TRAIN_MODEL, ML_PREDICT, ML_XGBOOST, etc.)
 * - Spatial operations (ST_* functions)
 * - Real-time streaming (LIVE queries)
 * - Common Table Expressions (WITH clause)
 * - CASE expressions and conditional aggregates
 *
 * License: BSD-3-Clause OR MIT
 * Copyright (c) 2025 TuringWorks
 */

parser grammar OrbitQLParser;

options {
    tokenVocab = OrbitQLLexer;
}

// =============================================================================
// ROOT RULE
// =============================================================================

root
    : statement* EOF
    ;

// =============================================================================
// STATEMENTS
// =============================================================================

statement
    : dmlStatement
    | ddlStatement
    | transactionStatement
    | liveStatement
    | graphStatement
    | graphRAGStatement
    | explainStatement
    | SEMICOLON
    ;

// =============================================================================
// DML STATEMENTS (Data Manipulation Language)
// =============================================================================

dmlStatement
    : selectStatement
    | insertStatement
    | updateStatement
    | deleteStatement
    ;

// -----------------------------------------------------------------------------
// SELECT STATEMENT
// -----------------------------------------------------------------------------

selectStatement
    : withClause? selectCore orderByClause? limitClause? offsetClause? forClause? timeoutClause?
    ;

withClause
    : WITH RECURSIVE? commonTableExpression (COMMA commonTableExpression)*
    ;

commonTableExpression
    : identifier columnNameList? AS OPEN_PAREN selectStatement CLOSE_PAREN
    ;

selectCore
    : SELECT setQuantifier? selectList
      fromClause?
      joinClause*
      traverseClause?
      whereClause?
      groupByClause?
      havingClause?
      (UNION ALL? selectCore)?
      (INTERSECT ALL? selectCore)?
      (EXCEPT ALL? selectCore)?
    ;

setQuantifier
    : DISTINCT
    | ALL
    ;

selectList
    : selectItem (COMMA selectItem)*
    ;

selectItem
    : ASTERISK                                              # selectAll
    | tableIdentifier DOT ASTERISK                          # selectTableAll
    | expression (AS? identifier)?                          # selectExpression
    | graphPathExpression (AS? identifier)?                 # selectGraphPath
    ;

fromClause
    : FROM tableReference (COMMA tableReference)*
    ;

tableReference
    : tablePrimary
    | tableReference joinType tableReference joinCondition?
    ;

tablePrimary
    : tableIdentifier (AS? identifier)?                     # tableSource
    | OPEN_PAREN selectStatement CLOSE_PAREN (AS? identifier)  # subquerySource
    | LATERAL OPEN_PAREN selectStatement CLOSE_PAREN (AS? identifier)  # lateralSubquery
    ;

tableIdentifier
    : (schemaName DOT)? tableName
    ;

schemaName
    : identifier
    ;

tableName
    : identifier
    ;

joinClause
    : joinType tableReference joinCondition?
    ;

joinType
    : INNER? JOIN                                           # innerJoin
    | LEFT OUTER? JOIN                                      # leftJoin
    | RIGHT OUTER? JOIN                                     # rightJoin
    | FULL OUTER? JOIN                                      # fullJoin
    | CROSS JOIN                                            # crossJoin
    | NATURAL JOIN                                          # naturalJoin
    | GRAPH JOIN                                            # graphJoin
    ;

joinCondition
    : ON expression
    | USING OPEN_PAREN columnNameList CLOSE_PAREN
    ;

// Graph traversal in SELECT
traverseClause
    : TRAVERSE traverseDirection? traverseDepth? STEPS? ON edgeTypeList TO identifier
      returnPathsClause?
    ;

traverseDirection
    : OUTBOUND
    | INBOUND
    | BOTH
    ;

traverseDepth
    : INTEGER_LITERAL DOUBLE_DOT INTEGER_LITERAL
    | INTEGER_LITERAL
    ;

edgeTypeList
    : edgeType
    | OPEN_BRACKET edgeType (COMMA edgeType)* CLOSE_BRACKET
    ;

edgeType
    : identifier
    ;

returnPathsClause
    : RETURN PATHS AS identifier
    ;

whereClause
    : WHERE expression
    ;

groupByClause
    : GROUP BY expressionList
    ;

havingClause
    : HAVING expression
    ;

orderByClause
    : ORDER BY orderByItem (COMMA orderByItem)*
    ;

orderByItem
    : expression (ASC | DESC)? (NULLS (FIRST | LAST))?
    ;

limitClause
    : LIMIT (INTEGER_LITERAL | ALL)
    ;

offsetClause
    : OFFSET INTEGER_LITERAL (ROW | ROWS)?
    ;

forClause
    : FOR (UPDATE | SHARE) (OF tableNameList)? (NOWAIT | SKIP_LOCKED)?
    ;

timeoutClause
    : TIMEOUT intervalExpression
    ;

fetchClause
    : FETCH identifierList
    ;

// -----------------------------------------------------------------------------
// INSERT STATEMENT
// -----------------------------------------------------------------------------

insertStatement
    : INSERT INTO tableIdentifier columnNameList?
      (VALUES valuesList | selectStatement | objectLiteral)
      onConflictClause?
      returningClause?
    ;

columnNameList
    : OPEN_PAREN identifier (COMMA identifier)* CLOSE_PAREN
    ;

valuesList
    : valuesRow (COMMA valuesRow)*
    ;

valuesRow
    : OPEN_PAREN expression (COMMA expression)* CLOSE_PAREN
    ;

onConflictClause
    : ON CONFLICT conflictTarget? conflictAction
    ;

conflictTarget
    : OPEN_PAREN columnNameList CLOSE_PAREN
    | ON CONSTRAINT identifier
    ;

conflictAction
    : DO NOTHING
    | DO UPDATE SET updateAssignmentList whereClause?
    ;

returningClause
    : RETURNING selectList
    ;

// -----------------------------------------------------------------------------
// UPDATE STATEMENT
// -----------------------------------------------------------------------------

updateStatement
    : UPDATE tableIdentifier (AS? identifier)?
      SET updateAssignmentList
      fromClause?
      whereClause?
      returningClause?
    ;

updateAssignmentList
    : updateAssignment (COMMA updateAssignment)*
    ;

updateAssignment
    : identifier updateOperator expression
    ;

updateOperator
    : EQUALS
    | PLUS_EQUALS
    | MINUS_EQUALS
    | ASTERISK_EQUALS
    | SLASH_EQUALS
    ;

// -----------------------------------------------------------------------------
// DELETE STATEMENT
// -----------------------------------------------------------------------------

deleteStatement
    : DELETE FROM tableIdentifier (AS? identifier)?
      whereClause?
      returningClause?
    ;

// =============================================================================
// DDL STATEMENTS (Data Definition Language)
// =============================================================================

ddlStatement
    : createStatement
    | alterStatement
    | dropStatement
    ;

// CREATE statements
createStatement
    : createTableStatement
    | createIndexStatement
    | createViewStatement
    | createFunctionStatement
    | createTriggerStatement
    | createSchemaStatement
    ;

createTableStatement
    : CREATE (TEMP | TEMPORARY)? TABLE ifNotExists? tableIdentifier
      OPEN_PAREN tableElementList CLOSE_PAREN
      tableOptions?
    ;

ifNotExists
    : IF NOT EXISTS
    ;

tableElementList
    : tableElement (COMMA tableElement)*
    ;

tableElement
    : columnDefinition
    | tableConstraint
    ;

columnDefinition
    : identifier dataType columnConstraint*
    ;

dataType
    : primitiveType arrayDimension*
    | ARRAY OPEN_BRACKET dataType CLOSE_BRACKET
    | OBJECT
    | JSON
    ;

primitiveType
    : BOOLEAN
    | INTEGER | INT | BIGINT | SMALLINT
    | FLOAT | DOUBLE PRECISION? | REAL | DECIMAL | NUMERIC | DEC
    | STRING | TEXT | VARCHAR (OPEN_PAREN INTEGER_LITERAL CLOSE_PAREN)?
    | DATETIME | TIMESTAMP (WITH TIME ZONE)? | DATE | TIME
    | DURATION
    | UUID
    | BINARY
    | GEOMETRY | POINT | LINESTRING | POLYGON
    | MULTIPOINT | MULTILINESTRING | MULTIPOLYGON | GEOMETRYCOLLECTION
    | VECTOR (OPEN_PAREN INTEGER_LITERAL CLOSE_PAREN)?
    ;

arrayDimension
    : OPEN_BRACKET INTEGER_LITERAL? CLOSE_BRACKET
    ;

columnConstraint
    : constraintName? columnConstraintDef
    ;

constraintName
    : CONSTRAINT identifier
    ;

columnConstraintDef
    : NOT NULL                                              # notNullConstraint
    | NULL                                                  # nullableConstraint
    | UNIQUE                                                # uniqueConstraint
    | PRIMARY KEY                                           # primaryKeyColumnConstraint
    | DEFAULT expression                                    # defaultConstraint
    | CHECK OPEN_PAREN expression CLOSE_PAREN               # checkConstraint
    | REFERENCES tableIdentifier columnNameList?
      (ON DELETE referentialAction)?
      (ON UPDATE referentialAction)?                        # referencesConstraint
    | GENERATED (ALWAYS | BY DEFAULT) AS IDENTITY
      identityOptions?                                      # generatedIdentityConstraint
    | GENERATED ALWAYS AS OPEN_PAREN expression CLOSE_PAREN
      (STORED | VIRTUAL)?                                   # generatedExpressionConstraint
    ;

identityOptions
    : OPEN_PAREN sequenceOption* CLOSE_PAREN
    ;

sequenceOption
    : START WITH? INTEGER_LITERAL
    | INCREMENT BY? INTEGER_LITERAL
    | MINVALUE INTEGER_LITERAL
    | MAXVALUE INTEGER_LITERAL
    | NO (MINVALUE | MAXVALUE | CYCLE)
    | CYCLE
    | CACHE INTEGER_LITERAL
    ;

tableConstraint
    : constraintName? tableConstraintDef
    ;

tableConstraintDef
    : PRIMARY KEY OPEN_PAREN columnNameList CLOSE_PAREN     # primaryKeyTableConstraint
    | UNIQUE OPEN_PAREN columnNameList CLOSE_PAREN          # uniqueTableConstraint
    | CHECK OPEN_PAREN expression CLOSE_PAREN               # checkTableConstraint
    | FOREIGN KEY OPEN_PAREN columnNameList CLOSE_PAREN
      REFERENCES tableIdentifier columnNameList?
      (ON DELETE referentialAction)?
      (ON UPDATE referentialAction)?                        # foreignKeyConstraint
    ;

referentialAction
    : CASCADE
    | RESTRICT
    | NO ACTION
    | SET NULL
    | SET DEFAULT
    ;

tableOptions
    : tableOption+
    ;

tableOption
    : PARTITION BY partitionStrategy OPEN_PAREN expressionList CLOSE_PAREN
    ;

partitionStrategy
    : RANGE
    | LIST
    | HASH
    ;

createIndexStatement
    : CREATE UNIQUE? INDEX ifNotExists? identifier
      ON tableIdentifier OPEN_PAREN indexColumnList CLOSE_PAREN
      indexOptions?
    ;

indexColumnList
    : indexColumn (COMMA indexColumn)*
    ;

indexColumn
    : identifier (ASC | DESC)?
    | OPEN_PAREN expression CLOSE_PAREN (ASC | DESC)?
    ;

indexOptions
    : USING indexMethod
    | WHERE expression
    ;

indexMethod
    : identifier
    ;

createViewStatement
    : CREATE (OR REPLACE)? (TEMP | TEMPORARY)? VIEW ifNotExists? tableIdentifier
      columnNameList?
      AS selectStatement
    ;

createFunctionStatement
    : CREATE (OR REPLACE)? FUNCTION tableIdentifier
      OPEN_PAREN functionParameterList? CLOSE_PAREN
      RETURNS dataType
      (LANGUAGE identifier)?
      AS functionBody
    ;

functionParameterList
    : functionParameter (COMMA functionParameter)*
    ;

functionParameter
    : identifier dataType (DEFAULT expression)?
    ;

functionBody
    : STRING_LITERAL
    | DOLLAR_STRING
    | BEGIN statement* END
    ;

createTriggerStatement
    : CREATE (OR REPLACE)? TRIGGER identifier
      triggerTiming triggerEvent (OR triggerEvent)* ON tableIdentifier
      (FOR EACH? (ROW | STATEMENT))?
      triggerAction
    ;

triggerTiming
    : BEFORE
    | AFTER
    | INSTEAD OF
    ;

triggerEvent
    : INSERT
    | UPDATE (OF columnNameList)?
    | DELETE
    ;

triggerAction
    : EXECUTE (FUNCTION | PROCEDURE) tableIdentifier OPEN_PAREN CLOSE_PAREN
    ;

createSchemaStatement
    : CREATE SCHEMA ifNotExists? identifier
    ;

// ALTER statements
alterStatement
    : ALTER TABLE tableIdentifier alterTableAction
    ;

alterTableAction
    : ADD COLUMN? columnDefinition
    | DROP COLUMN? ifExists? identifier (CASCADE | RESTRICT)?
    | ALTER COLUMN? identifier alterColumnAction
    | ADD tableConstraint
    | DROP CONSTRAINT ifExists? identifier (CASCADE | RESTRICT)?
    | RENAME TO identifier
    | RENAME COLUMN identifier TO identifier
    ;

alterColumnAction
    : SET DATA? TYPE dataType
    | SET DEFAULT expression
    | DROP DEFAULT
    | SET NOT NULL
    | DROP NOT NULL
    ;

ifExists
    : IF EXISTS
    ;

// DROP statements
dropStatement
    : DROP objectType ifExists? tableIdentifier (CASCADE | RESTRICT)?
    ;

objectType
    : TABLE
    | INDEX
    | VIEW
    | FUNCTION
    | TRIGGER
    | SCHEMA
    | DATABASE
    ;

// =============================================================================
// TRANSACTION STATEMENTS
// =============================================================================

transactionStatement
    : BEGIN (WORK | TRANSACTION)?
    | COMMIT (WORK | TRANSACTION)?
    | ROLLBACK (WORK | TRANSACTION)?
    ;

// =============================================================================
// LIVE STREAMING STATEMENTS
// =============================================================================

liveStatement
    : LIVE DIFF? selectStatement
    ;

// =============================================================================
// GRAPH STATEMENTS
// =============================================================================

graphStatement
    : traverseStatement
    | relateStatement
    ;

traverseStatement
    : TRAVERSE edgeType FROM expression
      (MAX_DEPTH INTEGER_LITERAL)?
      whereClause?
    ;

relateStatement
    : RELATE expression ARROW_RIGHT edgeType ARROW_RIGHT expression
      (SET objectLiteral)?
    ;

// =============================================================================
// GRAPHRAG STATEMENTS
// =============================================================================

graphRAGStatement
    : graphRAGBuild
    | graphRAGQuery
    | graphRAGExtract
    | graphRAGReason
    | graphRAGStats
    | graphRAGEntities
    | graphRAGSimilar
    ;

graphRAGBuild
    : GRAPHRAG BUILD identifier
      FROM expression
      TEXT expression
      graphRAGOptions?
    ;

graphRAGQuery
    : GRAPHRAG QUERY identifier
      TEXT expression
      graphRAGOptions?
    ;

graphRAGExtract
    : GRAPHRAG EXTRACT identifier
      FROM expression
      TEXT expression
      graphRAGOptions?
    ;

graphRAGReason
    : GRAPHRAG REASON identifier
      FROM expression TO expression
      graphRAGOptions?
    ;

graphRAGStats
    : GRAPHRAG STATS identifier
    ;

graphRAGEntities
    : GRAPHRAG ENTITIES identifier
      whereClause?
      limitClause?
    ;

graphRAGSimilar
    : GRAPHRAG SIMILAR identifier
      ENTITY expression
      limitClause?
      graphRAGOptions?
    ;

graphRAGOptions
    : WITH graphRAGOption (AND graphRAGOption)*
    ;

graphRAGOption
    : identifier EQUALS expression
    ;

// =============================================================================
// EXPLAIN STATEMENT
// =============================================================================

explainStatement
    : EXPLAIN explainOptions? statement
    ;

explainOptions
    : OPEN_PAREN explainOption (COMMA explainOption)* CLOSE_PAREN
    ;

explainOption
    : identifier (expression)?
    ;

// =============================================================================
// EXPRESSIONS
// =============================================================================

expression
    : orExpression
    ;

orExpression
    : andExpression (OR andExpression)*
    ;

andExpression
    : notExpression (AND notExpression)*
    ;

notExpression
    : NOT notExpression
    | comparisonExpression
    ;

comparisonExpression
    : additiveExpression comparisonOperator additiveExpression    # binaryComparison
    | additiveExpression IS NOT? NULL                             # isNullExpression
    | additiveExpression NOT? BETWEEN additiveExpression AND additiveExpression  # betweenExpression
    | additiveExpression NOT? IN inExpressionValue                # inExpression
    | additiveExpression NOT? (LIKE | ILIKE) additiveExpression (ESCAPE expression)?  # likeExpression
    | additiveExpression NOT? MATCH additiveExpression            # matchExpression
    | additiveExpression NOT? SIMILAR TO additiveExpression       # similarToExpression
    | additiveExpression CONTAINS additiveExpression              # containsExpression
    | additiveExpression STARTS_WITH additiveExpression           # startsWithExpression
    | additiveExpression ENDS_WITH additiveExpression             # endsWithExpression
    | additiveExpression NOT? CONNECTED additiveExpression        # connectedExpression
    | additiveExpression                                          # simpleComparison
    ;

comparisonOperator
    : EQUALS
    | NOT_EQUALS
    | LESS_THAN
    | GREATER_THAN
    | LESS_THAN_OR_EQUALS
    | GREATER_THAN_OR_EQUALS
    ;

inExpressionValue
    : OPEN_PAREN expressionList CLOSE_PAREN
    | OPEN_PAREN selectStatement CLOSE_PAREN
    ;

additiveExpression
    : multiplicativeExpression ((PLUS | MINUS | ARRAY_CONCAT) multiplicativeExpression)*
    ;

multiplicativeExpression
    : unaryExpression ((ASTERISK | SLASH | PERCENT) unaryExpression)*
    ;

unaryExpression
    : (PLUS | MINUS | NOT | TILDE) unaryExpression
    | primaryExpression
    ;

primaryExpression
    : literal                                                 # literalExpression
    | identifier                                              # identifierExpression
    | qualifiedIdentifier                                     # qualifiedIdentifierExpression
    | OPEN_PAREN expression CLOSE_PAREN                       # parenthesizedExpression
    | OPEN_PAREN selectStatement CLOSE_PAREN                  # subqueryExpression
    | caseExpression                                          # caseExpr
    | functionCall                                            # functionCallExpression
    | aggregateFunction                                       # aggregateFunctionExpression
    | windowFunction                                          # windowFunctionExpression
    | mlFunction                                              # mlFunctionExpression
    | spatialFunction                                         # spatialFunctionExpression
    | castExpression                                          # castExpr
    | arrayExpression                                         # arrayExpr
    | objectLiteral                                           # objectExpr
    | intervalExpression                                      # intervalExpr
    | parameterExpression                                     # parameterExpr
    | existsExpression                                        # existsExpr
    | primaryExpression DOT identifier                        # fieldAccessExpression
    | primaryExpression OPEN_BRACKET expression CLOSE_BRACKET # indexAccessExpression
    | primaryExpression JSON_EXTRACT expression               # jsonExtractExpression
    | primaryExpression JSON_EXTRACT_TEXT expression          # jsonExtractTextExpression
    | primaryExpression DOUBLE_COLON dataType                 # typeCastExpression
    | graphPathExpression                                     # graphPathExpr
    ;

// CASE expression
caseExpression
    : CASE expression? whenClause+ elseClause? END
    ;

whenClause
    : WHEN expression THEN expression
    ;

elseClause
    : ELSE expression
    ;

// Function calls
functionCall
    : functionName OPEN_PAREN (DISTINCT? expressionList | ASTERISK)? CLOSE_PAREN
      filterClause?
    ;

functionName
    : identifier
    | NOW
    | TIME_BUCKET
    | COALESCE
    | NULLIF
    | CONCAT
    | SUBSTRING
    | TRIM
    | UPPER
    | LOWER
    | LENGTH
    | EXTRACT
    | DATE
    | TIME
    ;

filterClause
    : FILTER OPEN_PAREN WHERE expression CLOSE_PAREN
    ;

// Aggregate functions
aggregateFunction
    : aggregateFunctionName OPEN_PAREN setQuantifier? expression CLOSE_PAREN
      filterClause?
    ;

aggregateFunctionName
    : COUNT
    | SUM
    | AVG
    | MIN
    | MAX
    | FIRST
    | LAST
    | STDDEV
    | VARIANCE
    ;

// Window functions
windowFunction
    : functionCall OVER windowSpec
    ;

windowSpec
    : OPEN_PAREN windowDefinition CLOSE_PAREN
    | identifier
    ;

windowDefinition
    : partitionByClause? orderByClause? frameClause?
    ;

partitionByClause
    : PARTITION BY expressionList
    ;

frameClause
    : frameType frameBound
    | frameType BETWEEN frameBound AND frameBound
    ;

frameType
    : ROWS
    | RANGE
    | GROUPS
    ;

frameBound
    : UNBOUNDED (PRECEDING | FOLLOWING)
    | CURRENT ROW
    | expression (PRECEDING | FOLLOWING)
    ;

// ML functions
mlFunction
    : mlTrainFunction
    | mlPredictFunction
    | mlEvaluateFunction
    | mlModelManagementFunction
    | mlBoostingFunction
    | mlFeatureFunction
    | mlVectorFunction
    ;

mlTrainFunction
    : ML_TRAIN_MODEL OPEN_PAREN
        expression COMMA  // model name
        expression COMMA  // algorithm
        expression COMMA  // features array
        expression        // target
        (COMMA expression)?  // optional parameters
      CLOSE_PAREN
    ;

mlPredictFunction
    : ML_PREDICT OPEN_PAREN expression COMMA expression CLOSE_PAREN
    ;

mlEvaluateFunction
    : ML_EVALUATE_MODEL OPEN_PAREN expressionList CLOSE_PAREN
    ;

mlModelManagementFunction
    : ML_DROP_MODEL OPEN_PAREN expression CLOSE_PAREN
    | ML_LIST_MODELS OPEN_PAREN CLOSE_PAREN
    | ML_MODEL_INFO OPEN_PAREN expression CLOSE_PAREN
    ;

mlBoostingFunction
    : (ML_XGBOOST | ML_LIGHTGBM | ML_CATBOOST | ML_ADABOOST | ML_GRADIENT_BOOSTING
      | ML_LINEAR_REGRESSION | ML_LOGISTIC_REGRESSION | ML_RANDOM_FOREST | ML_KMEANS)
      OPEN_PAREN expressionList CLOSE_PAREN
    ;

mlFeatureFunction
    : (ML_NORMALIZE | ML_PCA | ML_ENCODE_CATEGORICAL | ML_FEATURE_SELECTION)
      OPEN_PAREN expressionList CLOSE_PAREN
    ;

mlVectorFunction
    : (ML_EMBED_TEXT | ML_SIMILARITY_SEARCH | ML_VECTOR_CLUSTER
      | ML_FORECAST | ML_ANOMALY_DETECTION | ML_CORRELATION)
      OPEN_PAREN expressionList CLOSE_PAREN
    ;

// Spatial functions
spatialFunction
    : spatialConstructor
    | spatialRelationship
    | spatialMeasurement
    | spatialProcessing
    | spatialAccessor
    ;

spatialConstructor
    : (ST_POINT | ST_MAKEPOINT | ST_LINESTRING | ST_POLYGON
      | ST_GEOMFROMTEXT | ST_GEOMFROMGEOJSON)
      OPEN_PAREN expressionList CLOSE_PAREN
    ;

spatialRelationship
    : (ST_CONTAINS | ST_WITHIN | ST_INTERSECTS | ST_OVERLAPS
      | ST_TOUCHES | ST_CROSSES | ST_DISJOINT | ST_EQUALS | ST_DWITHIN)
      OPEN_PAREN expressionList CLOSE_PAREN
    ;

spatialMeasurement
    : (ST_DISTANCE | ST_LENGTH | ST_AREA | ST_PERIMETER)
      OPEN_PAREN expressionList CLOSE_PAREN
    ;

spatialProcessing
    : (ST_BUFFER | ST_CENTROID | ST_ENVELOPE | ST_UNION
      | ST_INTERSECTION | ST_DIFFERENCE | ST_SIMPLIFY | ST_TRANSFORM | ST_KMEANS)
      OPEN_PAREN expressionList CLOSE_PAREN
    ;

spatialAccessor
    : (ST_X | ST_Y | ST_Z | ST_SRID | ST_ASTEXT | ST_ASGEOJSON | ST_ASWKB)
      OPEN_PAREN expression CLOSE_PAREN
    ;

// Cast expression
castExpression
    : CAST OPEN_PAREN expression AS dataType CLOSE_PAREN
    ;

// Array expression
arrayExpression
    : ARRAY OPEN_BRACKET expressionList? CLOSE_BRACKET
    | OPEN_BRACKET expressionList? CLOSE_BRACKET
    ;

// Object literal
objectLiteral
    : OBJECT OPEN_PAREN objectElementList? CLOSE_PAREN
    | OPEN_BRACE objectMemberList? CLOSE_BRACE
    ;

objectElementList
    : expression COMMA expression (COMMA expression COMMA expression)*
    ;

objectMemberList
    : objectMember (COMMA objectMember)*
    ;

objectMember
    : (identifier | STRING_LITERAL) COLON expression
    ;

// Interval expression
intervalExpression
    : INTERVAL expression intervalUnit?
    | NOW OPEN_PAREN CLOSE_PAREN (MINUS | PLUS) INTERVAL expression intervalUnit?
    | DURATION_LITERAL
    ;

intervalUnit
    : YEAR | YEARS
    | MONTH | MONTHS
    | WEEK | WEEKS
    | DAY | DAYS
    | HOUR | HOURS
    | MINUTE | MINUTES
    | SECOND | SECONDS
    | MILLISECOND | MILLISECONDS
    ;

// Parameter expression
parameterExpression
    : NAMED_PARAMETER
    | POSITIONAL_PARAMETER
    ;

// EXISTS expression
existsExpression
    : EXISTS OPEN_PAREN selectStatement CLOSE_PAREN
    ;

// Graph path expression
graphPathExpression
    : graphPathStep+
    ;

graphPathStep
    : (ARROW_RIGHT | ARROW_LEFT | ARROW_BOTH) identifier (ARROW_RIGHT | ARROW_LEFT | ARROW_BOTH)
    ;

// Qualified identifier
qualifiedIdentifier
    : identifier (DOT identifier)+
    ;

// =============================================================================
// LITERALS
// =============================================================================

literal
    : numericLiteral
    | stringLiteral
    | booleanLiteral
    | nullLiteral
    | arrayLiteral
    | dateLiteral
    ;

numericLiteral
    : INTEGER_LITERAL
    | FLOAT_LITERAL
    | MINUS numericLiteral
    ;

stringLiteral
    : STRING_LITERAL
    | DOUBLE_QUOTED_STRING
    | DOLLAR_STRING
    ;

booleanLiteral
    : TRUE
    | FALSE
    ;

nullLiteral
    : NULL
    ;

arrayLiteral
    : ARRAY OPEN_BRACKET expressionList? CLOSE_BRACKET
    ;

dateLiteral
    : DATE STRING_LITERAL
    | TIME STRING_LITERAL
    | TIMESTAMP STRING_LITERAL
    ;

// =============================================================================
// COMMON RULES
// =============================================================================

identifier
    : IDENTIFIER
    | QUOTED_IDENTIFIER
    | BACKTICK_IDENTIFIER
    | nonReservedKeyword
    ;

nonReservedKeyword
    : ACTION | ADD | AFTER | ALGORITHM | BEFORE | BUCKET | CASCADE
    | CLUSTER | COLUMN | COLUMNS | DATA | DATABASE | DEFAULT
    | EDGE | EVALUATE | FEATURES | FILTER | FIT | FORECAST | FUNCTION
    | GENERATED | GLOBAL | GRADIENT | GRAPH | KEY | LANGUAGE | LOCAL
    | METRIC | MODEL | NODE | NORMALIZE | PARALLEL | PARTITION
    | PATH | PCA | PREDICT | RANGE | REGRESSION | REPLACE | RESTRICT
    | RETURNS | ROLE | ROW | ROWS | SCHEMA | SCORE | SCROLL | SEARCH
    | SEQUENCE | SERIES | SESSION | SIMPLE | SIZE | SLIDE | SPATIAL
    | START | STATS | STEPS | STORED | TARGET | TEMP | TEMPORARY
    | TEXT | TIMEOUT | TRAIN | TRANSFORM | TRIGGER | TYPE
    | VALUE | VECTOR | VIEW | VIRTUAL | WATERMARK | WINDOW | WORK | ZONE
    ;

expressionList
    : expression (COMMA expression)*
    ;

identifierList
    : identifier (COMMA identifier)*
    ;

tableNameList
    : tableIdentifier (COMMA tableIdentifier)*
    ;

// Skip locked helper
SKIP_LOCKED: SKIP_ LOCKED;
fragment SKIP_: 'SKIP';
fragment LOCKED: 'LOCKED';
fragment NOWAIT: 'NOWAIT';
fragment GROUPS: 'GROUPS';
fragment INSTEAD: 'INSTEAD';
fragment MAXVALUE: 'MAXVALUE';
fragment MINVALUE: 'MINVALUE';
