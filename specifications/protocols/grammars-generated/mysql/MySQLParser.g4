/*
 * MySQL 9.5 Parser Grammar for ANTLR4
 * Includes MySQL AI/HeatWave GenAI extensions
 * 
 * Copyright 2025 - Open Source Grammar
 * Licensed under MIT License
 */

parser grammar MySQLParser;

options {
    tokenVocab = MySQLLexer;
}

// =============================================================================
// ROOT RULE
// =============================================================================

root
    : sqlStatements? EOF
    ;

sqlStatements
    : (sqlStatement SEMICOLON?)+ 
    ;

sqlStatement
    : ddlStatement
    | dmlStatement
    | dclStatement
    | transactionStatement
    | replicationStatement
    | preparedStatement
    | compoundStatement
    | administrationStatement
    | utilityStatement
    | mlStatement           // MySQL AI/HeatWave ML statements
    ;

// =============================================================================
// DATA DEFINITION LANGUAGE (DDL)
// =============================================================================

ddlStatement
    : createStatement
    | alterStatement
    | dropStatement
    | truncateStatement
    | renameStatement
    ;

// -----------------------------------------------------------------------------
// CREATE Statements
// -----------------------------------------------------------------------------

createStatement
    : createDatabase
    | createTable
    | createIndex
    | createView
    | createProcedure
    | createFunction
    | createTrigger
    | createEvent
    | createServer
    | createTablespace
    | createLogfileGroup
    | createUser
    | createRole
    | createSpatialReferenceSystem
    ;

createDatabase
    : CREATE (DATABASE | SCHEMA) ifNotExists? databaseName createDatabaseOption*
    ;

createDatabaseOption
    : DEFAULT? (CHARACTER SET | CHARSET) EQUAL? charsetName
    | DEFAULT? COLLATE EQUAL? collationName
    | DEFAULT? ENCRYPTION EQUAL? stringLiteral
    | READ ONLY EQUAL? (DEFAULT | DECIMAL_LITERAL)
    ;

createTable
    : CREATE TEMPORARY? TABLE ifNotExists? tableName
      ( createTableDefinition tableOptions? partitionOptions?
      | createTableDefinition? tableOptions? partitionOptions? AS? selectStatement
      | LIKE tableName
      | OPEN_PAREN LIKE tableName CLOSE_PAREN
      )
    ;

createTableDefinition
    : OPEN_PAREN tableElement (COMMA tableElement)* CLOSE_PAREN
    ;

tableElement
    : columnDefinition
    | tableConstraint
    | indexDefinition
    | checkConstraint
    ;

columnDefinition
    : columnName dataType columnConstraint*
    ;

dataType
    : typeName=(
        TINYINT | SMALLINT | MEDIUMINT | INT | INTEGER | BIGINT
      | REAL | DOUBLE PRECISION? | FLOAT | DECIMAL | DEC | NUMERIC | FIXED
      | BIT | BOOL | BOOLEAN
      | DATE | DATETIME | TIMESTAMP | TIME | YEAR
      | CHAR | VARCHAR | BINARY | VARBINARY
      | TINYBLOB | BLOB | MEDIUMBLOB | LONGBLOB
      | TINYTEXT | TEXT | MEDIUMTEXT | LONGTEXT
      | ENUM | SET
      | GEOMETRY | GEOMETRYCOLLECTION | GEOMCOLLECTION
      | POINT | MULTIPOINT | LINESTRING | MULTILINESTRING
      | POLYGON | MULTIPOLYGON
      | JSON
      | SERIAL
      | VECTOR                          // MySQL 9.0+ VECTOR type
      ) typeLength? typeOptions?
    ;

typeLength
    : OPEN_PAREN DECIMAL_LITERAL (COMMA DECIMAL_LITERAL)? CLOSE_PAREN
    | OPEN_PAREN stringLiteral (COMMA stringLiteral)* CLOSE_PAREN  // For ENUM/SET
    ;

typeOptions
    : SIGNED
    | UNSIGNED
    | ZEROFILL
    | (CHARACTER SET | CHARSET) charsetName
    | COLLATE collationName
    | BINARY
    | USING VARBINARY                   // For VECTOR type
    ;

columnConstraint
    : nullability
    | DEFAULT defaultValue
    | VISIBLE
    | INVISIBLE
    | AUTO_INCREMENT
    | UNIQUE KEY?
    | PRIMARY? KEY
    | COMMENT stringLiteral
    | COLLATE collationName
    | COLUMN_FORMAT columnFormat
    | ENGINE_ATTRIBUTE EQUAL? stringLiteral
    | SECONDARY_ENGINE_ATTRIBUTE EQUAL? stringLiteral
    | STORAGE storageType
    | referenceDefinition
    | checkConstraint
    | generatedColumn
    | SRID DECIMAL_LITERAL
    ;

nullability
    : NOT? NULL
    ;

defaultValue
    : literal
    | OPEN_PAREN expression CLOSE_PAREN
    | currentTimestamp
    | NULL
    ;

currentTimestamp
    : (CURRENT_TIMESTAMP | LOCALTIME | LOCALTIMESTAMP | NOW)
      (OPEN_PAREN DECIMAL_LITERAL? CLOSE_PAREN)?
      (ON UPDATE (CURRENT_TIMESTAMP | LOCALTIME | LOCALTIMESTAMP | NOW)
       (OPEN_PAREN DECIMAL_LITERAL? CLOSE_PAREN)?)?
    ;

generatedColumn
    : (GENERATED ALWAYS)? AS OPEN_PAREN expression CLOSE_PAREN (VIRTUAL | STORED)?
    ;

tableConstraint
    : (CONSTRAINT constraintName?)?
      ( PRIMARY KEY indexType? keyList indexOption*
      | UNIQUE KEY? indexName? indexType? keyList indexOption*
      | FOREIGN KEY indexName? keyList referenceDefinition
      | FULLTEXT KEY? indexName? keyList indexOption*
      | SPATIAL KEY? indexName? keyList indexOption*
      )
    ;

checkConstraint
    : (CONSTRAINT constraintName?)? CHECK OPEN_PAREN expression CLOSE_PAREN (NOT? ENFORCED)?
    ;

indexDefinition
    : (INDEX | KEY) indexName? indexType? keyList indexOption*
    | (FULLTEXT | SPATIAL) (INDEX | KEY)? indexName? keyList indexOption*
    ;

keyList
    : OPEN_PAREN keyPart (COMMA keyPart)* CLOSE_PAREN
    ;

keyPart
    : columnName (OPEN_PAREN DECIMAL_LITERAL CLOSE_PAREN)? orderDirection?
    | OPEN_PAREN expression CLOSE_PAREN orderDirection?
    ;

indexType
    : USING (BTREE | HASH | RTREE)
    ;

indexOption
    : KEY_BLOCK_SIZE EQUAL? DECIMAL_LITERAL
    | indexType
    | WITH PARSER parserName
    | COMMENT stringLiteral
    | (VISIBLE | INVISIBLE)
    | ENGINE_ATTRIBUTE EQUAL? stringLiteral
    | SECONDARY_ENGINE_ATTRIBUTE EQUAL? stringLiteral
    ;

referenceDefinition
    : REFERENCES tableName keyList
      (MATCH (FULL | PARTIAL | SIMPLE))?
      referenceAction?
    ;

referenceAction
    : ON DELETE referenceOption (ON UPDATE referenceOption)?
    | ON UPDATE referenceOption (ON DELETE referenceOption)?
    ;

referenceOption
    : RESTRICT | CASCADE | SET NULL | NO ACTION | SET DEFAULT
    ;

tableOptions
    : tableOption (COMMA? tableOption)*
    ;

tableOption
    : ENGINE EQUAL? engineName
    | SECONDARY_ENGINE EQUAL? (NULL | engineName)           // For HeatWave RAPID
    | AUTO_INCREMENT EQUAL? DECIMAL_LITERAL
    | AVG_ROW_LENGTH EQUAL? DECIMAL_LITERAL
    | DEFAULT? (CHARACTER SET | CHARSET) EQUAL? charsetName
    | CHECKSUM EQUAL? (DECIMAL_LITERAL)
    | DEFAULT? COLLATE EQUAL? collationName
    | COMMENT EQUAL? stringLiteral
    | COMPRESSION EQUAL? stringLiteral
    | CONNECTION EQUAL? stringLiteral
    | (DATA | INDEX) DIRECTORY EQUAL? stringLiteral
    | DELAY_KEY_WRITE EQUAL? (DECIMAL_LITERAL)
    | ENCRYPTION EQUAL? stringLiteral
    | ENGINE_ATTRIBUTE EQUAL? stringLiteral
    | INSERT_METHOD EQUAL? (NO | FIRST | LAST)
    | KEY_BLOCK_SIZE EQUAL? DECIMAL_LITERAL
    | MAX_ROWS EQUAL? DECIMAL_LITERAL
    | MIN_ROWS EQUAL? DECIMAL_LITERAL
    | PACK_KEYS EQUAL? (DECIMAL_LITERAL | DEFAULT)
    | PASSWORD EQUAL? stringLiteral
    | ROW_FORMAT EQUAL? rowFormat
    | SECONDARY_ENGINE_ATTRIBUTE EQUAL? stringLiteral
    | START TRANSACTION
    | STATS_AUTO_RECALC EQUAL? (DECIMAL_LITERAL | DEFAULT)
    | STATS_PERSISTENT EQUAL? (DECIMAL_LITERAL | DEFAULT)
    | STATS_SAMPLE_PAGES EQUAL? DECIMAL_LITERAL
    | STORAGE (DISK | MEMORY)
    | TABLESPACE tablespaceName (STORAGE (DISK | MEMORY))?
    | UNION EQUAL? OPEN_PAREN tableName (COMMA tableName)* CLOSE_PAREN
    ;

rowFormat
    : DEFAULT | DYNAMIC | FIXED | COMPRESSED | REDUNDANT | COMPACT
    ;

partitionOptions
    : PARTITION BY partitionType
      (PARTITIONS DECIMAL_LITERAL)?
      (SUBPARTITION BY subpartitionType
       (SUBPARTITIONS DECIMAL_LITERAL)?)?
      (OPEN_PAREN partitionDefinition (COMMA partitionDefinition)* CLOSE_PAREN)?
    ;

partitionType
    : LINEAR? (HASH | KEY ALGORITHM? EQUAL? DECIMAL_LITERAL?) OPEN_PAREN expression CLOSE_PAREN
    | (RANGE | LIST) (OPEN_PAREN expression CLOSE_PAREN | COLUMNS OPEN_PAREN columnList CLOSE_PAREN)
    ;

subpartitionType
    : LINEAR? (HASH | KEY ALGORITHM? EQUAL? DECIMAL_LITERAL?) OPEN_PAREN expression CLOSE_PAREN
    ;

partitionDefinition
    : PARTITION partitionName
      (VALUES (LESS THAN partitionValues | IN partitionValues))?
      partitionOption*
      (OPEN_PAREN subpartitionDefinition (COMMA subpartitionDefinition)* CLOSE_PAREN)?
    ;

partitionValues
    : MAXVALUE
    | OPEN_PAREN (expression | MAXVALUE) (COMMA (expression | MAXVALUE))* CLOSE_PAREN
    ;

subpartitionDefinition
    : SUBPARTITION partitionName partitionOption*
    ;

partitionOption
    : TABLESPACE EQUAL? tablespaceName
    | STORAGE? ENGINE EQUAL? engineName
    | NODEGROUP EQUAL? DECIMAL_LITERAL
    | MAX_ROWS EQUAL? DECIMAL_LITERAL
    | MIN_ROWS EQUAL? DECIMAL_LITERAL
    | DATA DIRECTORY EQUAL? stringLiteral
    | INDEX DIRECTORY EQUAL? stringLiteral
    | COMMENT EQUAL? stringLiteral
    ;

// CREATE INDEX
createIndex
    : CREATE (UNIQUE | FULLTEXT | SPATIAL)? INDEX indexName
      indexType? ON tableName keyList indexOption*
      (ALGORITHM EQUAL? (DEFAULT | INPLACE | COPY))?
      (LOCK EQUAL? (DEFAULT | NONE | SHARED | EXCLUSIVE))?
    ;

// CREATE VIEW
createView
    : CREATE (OR REPLACE)?
      (ALGORITHM EQUAL? (UNDEFINED | MERGE | TEMPTABLE))?
      definerClause?
      (SQL SECURITY (DEFINER | INVOKER))?
      VIEW viewName (OPEN_PAREN columnList CLOSE_PAREN)? AS selectStatement
      (WITH (CASCADED | LOCAL)? CHECK OPTION)?
    ;

// CREATE PROCEDURE
createProcedure
    : CREATE definerClause? PROCEDURE procedureName
      OPEN_PAREN procedureParameter? (COMMA procedureParameter)* CLOSE_PAREN
      routineCharacteristic*
      routineBody
    ;

procedureParameter
    : (IN | OUT | INOUT)? parameterName dataType
    ;

// CREATE FUNCTION
createFunction
    : CREATE definerClause? FUNCTION functionName
      OPEN_PAREN functionParameter? (COMMA functionParameter)* CLOSE_PAREN
      RETURNS dataType
      routineCharacteristic*
      routineBody
    ;

functionParameter
    : parameterName dataType
    ;

routineCharacteristic
    : COMMENT stringLiteral
    | LANGUAGE (SQL | JAVASCRIPT)        // MySQL 9.0+ JavaScript support
    | (NOT)? DETERMINISTIC
    | (CONTAINS SQL | NO SQL | READS SQL DATA | MODIFIES SQL DATA)
    | SQL SECURITY (DEFINER | INVOKER)
    ;

routineBody
    : compoundStatement
    | sqlStatement
    ;

// CREATE TRIGGER
createTrigger
    : CREATE definerClause? TRIGGER triggerName
      triggerTime triggerEvent ON tableName FOR EACH ROW
      triggerOrder?
      triggerBody
    ;

triggerTime
    : BEFORE | AFTER
    ;

triggerEvent
    : INSERT | UPDATE | DELETE
    ;

triggerOrder
    : (FOLLOWS | PRECEDES) triggerName
    ;

triggerBody
    : compoundStatement
    | sqlStatement
    ;

// CREATE EVENT
createEvent
    : CREATE definerClause? EVENT ifNotExists? eventName
      ON SCHEDULE scheduleDefinition
      (ON COMPLETION NOT? PRESERVE)?
      (ENABLE | DISABLE | DISABLE ON SLAVE)?
      (COMMENT stringLiteral)?
      DO eventBody
    ;

scheduleDefinition
    : AT timestamp (PLUS INTERVAL intervalValue)*
    | EVERY intervalValue
      (STARTS timestamp (PLUS INTERVAL intervalValue)*)?
      (ENDS timestamp (PLUS INTERVAL intervalValue)*)?
    ;

eventBody
    : compoundStatement
    | sqlStatement
    ;

// CREATE SERVER
createServer
    : CREATE SERVER serverName FOREIGN DATA WRAPPER wrapperName
      OPTIONS OPEN_PAREN serverOption (COMMA serverOption)* CLOSE_PAREN
    ;

serverOption
    : HOST stringLiteral
    | DATABASE stringLiteral
    | USER stringLiteral
    | PASSWORD stringLiteral
    | SOCKET stringLiteral
    | OWNER stringLiteral
    | PORT DECIMAL_LITERAL
    ;

// CREATE TABLESPACE
createTablespace
    : CREATE (UNDO)? TABLESPACE tablespaceName
      (ADD DATAFILE stringLiteral)?
      (FILE_BLOCK_SIZE EQUAL? DECIMAL_LITERAL)?
      tablespaceoption*
      (ENGINE EQUAL? engineName)?
    ;

tablespaceoption
    : INITIAL_SIZE EQUAL? DECIMAL_LITERAL
    | AUTOEXTEND_SIZE EQUAL? DECIMAL_LITERAL
    | MAX_SIZE EQUAL? DECIMAL_LITERAL
    | EXTENT_SIZE EQUAL? DECIMAL_LITERAL
    | NODEGROUP EQUAL? DECIMAL_LITERAL
    | ENGINE EQUAL? engineName
    | WAIT
    | COMMENT stringLiteral
    | ENCRYPTION EQUAL? stringLiteral
    ;

// CREATE LOGFILE GROUP
createLogfileGroup
    : CREATE LOGFILE GROUP logfileGroupName
      ADD UNDOFILE stringLiteral
      (INITIAL_SIZE EQUAL? DECIMAL_LITERAL)?
      (UNDO_BUFFER_SIZE EQUAL? DECIMAL_LITERAL)?
      (REDO_BUFFER_SIZE EQUAL? DECIMAL_LITERAL)?
      (NODEGROUP EQUAL? DECIMAL_LITERAL)?
      (WAIT)?
      (COMMENT stringLiteral)?
      (ENGINE EQUAL? engineName)?
    ;

// CREATE USER
createUser
    : CREATE USER ifNotExists? createUserEntry (COMMA createUserEntry)*
      createUserOption*
    ;

createUserEntry
    : userName (IDENTIFIED BY stringLiteral
               | IDENTIFIED BY RANDOM PASSWORD
               | IDENTIFIED WITH authPlugin (BY stringLiteral | AS stringLiteral)?)?
    ;

createUserOption
    : DEFAULT ROLE roleName (COMMA roleName)*
    | REQUIRE (NONE | tlsOption (AND? tlsOption)*)
    | WITH resourceOption+
    | passwordOption
    | lockOption
    | COMMENT stringLiteral
    | ATTRIBUTE stringLiteral
    ;

tlsOption
    : SSL | X509 | CIPHER stringLiteral | ISSUER stringLiteral | SUBJECT stringLiteral
    ;

resourceOption
    : MAX_QUERIES_PER_HOUR DECIMAL_LITERAL
    | MAX_UPDATES_PER_HOUR DECIMAL_LITERAL
    | MAX_CONNECTIONS_PER_HOUR DECIMAL_LITERAL
    | MAX_USER_CONNECTIONS DECIMAL_LITERAL
    ;

passwordOption
    : PASSWORD EXPIRE (DEFAULT | NEVER | INTERVAL DECIMAL_LITERAL DAY)?
    | PASSWORD HISTORY (DEFAULT | DECIMAL_LITERAL)
    | PASSWORD REUSE INTERVAL (DEFAULT | DECIMAL_LITERAL DAY)
    | PASSWORD REQUIRE CURRENT (DEFAULT | OPTIONAL)?
    | FAILED_LOGIN_ATTEMPTS DECIMAL_LITERAL
    | PASSWORD_LOCK_TIME (DECIMAL_LITERAL | UNBOUNDED)
    ;

lockOption
    : ACCOUNT (LOCK | UNLOCK)
    ;

// CREATE ROLE
createRole
    : CREATE ROLE ifNotExists? roleName (COMMA roleName)*
    ;

// CREATE SPATIAL REFERENCE SYSTEM
createSpatialReferenceSystem
    : CREATE (OR REPLACE)? SPATIAL REFERENCE SYSTEM ifNotExists? DECIMAL_LITERAL
      srsAttribute+
    ;

srsAttribute
    : NAME stringLiteral
    | DEFINITION stringLiteral
    | ORGANIZATION stringLiteral IDENTIFIED BY DECIMAL_LITERAL
    | DESCRIPTION stringLiteral
    ;

// -----------------------------------------------------------------------------
// ALTER Statements
// -----------------------------------------------------------------------------

alterStatement
    : alterTable
    | alterDatabase
    | alterView
    | alterProcedure
    | alterFunction
    | alterEvent
    | alterServer
    | alterTablespace
    | alterLogfileGroup
    | alterUser
    | alterInstance
    ;

alterTable
    : ALTER TABLE tableName alterTableActions?
      (partitionOptions | REMOVE PARTITIONING)?
    ;

alterTableActions
    : alterTableAction (COMMA alterTableAction)*
    ;

alterTableAction
    : tableOption
    | ADD COLUMN? ifNotExists? columnDefinition (FIRST | AFTER columnName)?
    | ADD COLUMN? ifNotExists? OPEN_PAREN columnDefinition (COMMA columnDefinition)* CLOSE_PAREN
    | ADD (INDEX | KEY) ifNotExists? indexName? indexType? keyList indexOption*
    | ADD (FULLTEXT | SPATIAL) (INDEX | KEY)? indexName? keyList indexOption*
    | ADD (CONSTRAINT constraintName?)? PRIMARY KEY indexType? keyList indexOption*
    | ADD (CONSTRAINT constraintName?)? UNIQUE (INDEX | KEY)? indexName? indexType? keyList indexOption*
    | ADD (CONSTRAINT constraintName?)? FOREIGN KEY indexName? keyList referenceDefinition
    | ADD checkConstraint
    | ALTER COLUMN? columnName (SET DEFAULT defaultValue | SET (VISIBLE | INVISIBLE) | DROP DEFAULT)
    | ALTER (INDEX | KEY) indexName (VISIBLE | INVISIBLE)
    | ALTER CHECK constraintName (NOT? ENFORCED)
    | ALTER (CONSTRAINT constraintName)? (NOT? ENFORCED)
    | CHANGE COLUMN? columnName columnDefinition (FIRST | AFTER columnName)?
    | DEFAULT? (CHARACTER SET | CHARSET) EQUAL? charsetName (COLLATE EQUAL? collationName)?
    | CONVERT TO (CHARACTER SET | CHARSET) charsetName (COLLATE collationName)?
    | (DISABLE | ENABLE) KEYS
    | (DISCARD | IMPORT) TABLESPACE
    | DROP COLUMN? ifExists? columnName
    | DROP (INDEX | KEY) ifExists? indexName
    | DROP PRIMARY KEY
    | DROP FOREIGN KEY fkSymbol
    | DROP CHECK constraintName
    | DROP CONSTRAINT constraintName
    | FORCE
    | LOCK EQUAL? (DEFAULT | NONE | SHARED | EXCLUSIVE)
    | MODIFY COLUMN? columnDefinition (FIRST | AFTER columnName)?
    | ORDER BY columnName (COMMA columnName)*
    | RENAME COLUMN columnName TO columnName
    | RENAME (INDEX | KEY) indexName TO indexName
    | RENAME (TO | AS)? tableName
    | (WITHOUT | WITH) VALIDATION
    | ADD PARTITION partitionDefinition
    | DROP PARTITION partitionName (COMMA partitionName)*
    | DISCARD PARTITION (partitionName (COMMA partitionName)* | ALL) TABLESPACE
    | IMPORT PARTITION (partitionName (COMMA partitionName)* | ALL) TABLESPACE
    | TRUNCATE PARTITION (partitionName (COMMA partitionName)* | ALL)
    | COALESCE PARTITION DECIMAL_LITERAL
    | REORGANIZE PARTITION partitionName (COMMA partitionName)* INTO 
      OPEN_PAREN partitionDefinition (COMMA partitionDefinition)* CLOSE_PAREN
    | EXCHANGE PARTITION partitionName WITH TABLE tableName ((WITH | WITHOUT) VALIDATION)?
    | ANALYZE PARTITION (partitionName (COMMA partitionName)* | ALL)
    | CHECK PARTITION (partitionName (COMMA partitionName)* | ALL)
    | OPTIMIZE PARTITION (partitionName (COMMA partitionName)* | ALL)
    | REBUILD PARTITION (partitionName (COMMA partitionName)* | ALL)
    | REPAIR PARTITION (partitionName (COMMA partitionName)* | ALL)
    | UPGRADE PARTITIONING
    | SECONDARY_LOAD                    // HeatWave load
    | SECONDARY_UNLOAD                  // HeatWave unload
    | ALGORITHM EQUAL? (DEFAULT | INPLACE | COPY | INSTANT)
    ;

alterDatabase
    : ALTER (DATABASE | SCHEMA) databaseName? alterDatabaseOption+
    ;

alterDatabaseOption
    : DEFAULT? (CHARACTER SET | CHARSET) EQUAL? charsetName
    | DEFAULT? COLLATE EQUAL? collationName
    | DEFAULT? ENCRYPTION EQUAL? stringLiteral
    | READ ONLY EQUAL? (DEFAULT | DECIMAL_LITERAL)
    ;

alterView
    : ALTER (ALGORITHM EQUAL? (UNDEFINED | MERGE | TEMPTABLE))?
      definerClause?
      (SQL SECURITY (DEFINER | INVOKER))?
      VIEW viewName (OPEN_PAREN columnList CLOSE_PAREN)? AS selectStatement
      (WITH (CASCADED | LOCAL)? CHECK OPTION)?
    ;

alterProcedure
    : ALTER PROCEDURE procedureName routineCharacteristic*
    ;

alterFunction
    : ALTER FUNCTION functionName routineCharacteristic*
    ;

alterEvent
    : ALTER definerClause? EVENT eventName
      (ON SCHEDULE scheduleDefinition)?
      (ON COMPLETION NOT? PRESERVE)?
      (RENAME TO eventName)?
      (ENABLE | DISABLE | DISABLE ON SLAVE)?
      (COMMENT stringLiteral)?
      (DO eventBody)?
    ;

alterServer
    : ALTER SERVER serverName OPTIONS OPEN_PAREN serverOption (COMMA serverOption)* CLOSE_PAREN
    ;

alterTablespace
    : ALTER (UNDO)? TABLESPACE tablespaceName
      (ADD | DROP) DATAFILE stringLiteral
      (INITIAL_SIZE EQUAL? DECIMAL_LITERAL)?
      (WAIT)?
      (ENGINE EQUAL? engineName)?
    ;

alterLogfileGroup
    : ALTER LOGFILE GROUP logfileGroupName
      ADD UNDOFILE stringLiteral
      (INITIAL_SIZE EQUAL? DECIMAL_LITERAL)?
      (WAIT)?
      (ENGINE EQUAL? engineName)?
    ;

alterUser
    : ALTER USER ifExists? alterUserEntry (COMMA alterUserEntry)* createUserOption*
    | ALTER USER ifExists? USER OPEN_PAREN CLOSE_PAREN IDENTIFIED BY stringLiteral
    ;

alterUserEntry
    : userName
      ( IDENTIFIED BY stringLiteral (REPLACE stringLiteral)? (RETAIN CURRENT PASSWORD)?
      | IDENTIFIED BY RANDOM PASSWORD (REPLACE stringLiteral)? (RETAIN CURRENT PASSWORD)?
      | IDENTIFIED WITH authPlugin (BY stringLiteral | AS stringLiteral)?
      | DISCARD OLD PASSWORD
      )?
    ;

alterInstance
    : ALTER INSTANCE instanceAction
    ;

instanceAction
    : ROTATE INNODB MASTER KEY
    | ROTATE BINLOG MASTER KEY
    | RELOAD TLS (FOR CHANNEL channel)? (NO ROLLBACK ON ERROR)?
    | ENABLE INNODB REDO_LOG
    | DISABLE INNODB REDO_LOG
    | RELOAD KEYRING
    ;

channel
    : MYSQL_MAIN | MYSQL_ADMIN
    ;

// -----------------------------------------------------------------------------
// DROP Statements
// -----------------------------------------------------------------------------

dropStatement
    : dropDatabase
    | dropTable
    | dropIndex
    | dropView
    | dropProcedure
    | dropFunction
    | dropTrigger
    | dropEvent
    | dropServer
    | dropTablespace
    | dropLogfileGroup
    | dropUser
    | dropRole
    | dropSpatialReferenceSystem
    ;

dropDatabase
    : DROP (DATABASE | SCHEMA) ifExists? databaseName
    ;

dropTable
    : DROP TEMPORARY? TABLE ifExists? tableName (COMMA tableName)*
      (RESTRICT | CASCADE)?
    ;

dropIndex
    : DROP INDEX indexName ON tableName
      (ALGORITHM EQUAL? (DEFAULT | INPLACE | COPY))?
      (LOCK EQUAL? (DEFAULT | NONE | SHARED | EXCLUSIVE))?
    ;

dropView
    : DROP VIEW ifExists? viewName (COMMA viewName)* (RESTRICT | CASCADE)?
    ;

dropProcedure
    : DROP PROCEDURE ifExists? procedureName
    ;

dropFunction
    : DROP FUNCTION ifExists? functionName
    ;

dropTrigger
    : DROP TRIGGER ifExists? triggerName
    ;

dropEvent
    : DROP EVENT ifExists? eventName
    ;

dropServer
    : DROP SERVER ifExists? serverName
    ;

dropTablespace
    : DROP (UNDO)? TABLESPACE tablespaceName (ENGINE EQUAL? engineName)?
    ;

dropLogfileGroup
    : DROP LOGFILE GROUP logfileGroupName (ENGINE EQUAL? engineName)?
    ;

dropUser
    : DROP USER ifExists? userName (COMMA userName)*
    ;

dropRole
    : DROP ROLE ifExists? roleName (COMMA roleName)*
    ;

dropSpatialReferenceSystem
    : DROP SPATIAL REFERENCE SYSTEM ifExists? DECIMAL_LITERAL
    ;

// -----------------------------------------------------------------------------
// TRUNCATE Statement
// -----------------------------------------------------------------------------

truncateStatement
    : TRUNCATE TABLE? tableName
    ;

// -----------------------------------------------------------------------------
// RENAME Statement
// -----------------------------------------------------------------------------

renameStatement
    : RENAME TABLE renameTablePair (COMMA renameTablePair)*
    | RENAME USER renameUserPair (COMMA renameUserPair)*
    ;

renameTablePair
    : tableName TO tableName
    ;

renameUserPair
    : userName TO userName
    ;

// =============================================================================
// DATA MANIPULATION LANGUAGE (DML)
// =============================================================================

dmlStatement
    : selectStatement
    | insertStatement
    | updateStatement
    | deleteStatement
    | replaceStatement
    | callStatement
    | loadStatement
    | doStatement
    | handlerStatement
    | importStatement
    | tableStatement
    | valuesStatement
    ;

// -----------------------------------------------------------------------------
// SELECT Statement
// -----------------------------------------------------------------------------

selectStatement
    : queryExpression lockingClause*
    | selectStatementWithInto
    ;

selectStatementWithInto
    : OPEN_PAREN selectStatementWithInto CLOSE_PAREN
    | queryExpression intoClause lockingClause*
    | queryExpression lockingClause+ intoClause
    ;

queryExpression
    : withClause? queryExpressionBody orderByClause? limitClause?
    ;

queryExpressionBody
    : queryPrimary
    | queryExpressionBody (UNION | EXCEPT | INTERSECT) (ALL | DISTINCT)? queryPrimary
    ;

queryPrimary
    : querySpecification
    | tableValueConstructor
    | OPEN_PAREN queryExpression CLOSE_PAREN
    ;

querySpecification
    : SELECT selectOption* selectElements
      intoClause?
      fromClause?
      whereClause?
      groupByClause?
      havingClause?
      windowClause?
      qualifyClause?
    ;

selectOption
    : ALL | DISTINCT | DISTINCTROW
    | HIGH_PRIORITY
    | STRAIGHT_JOIN
    | SQL_SMALL_RESULT
    | SQL_BIG_RESULT
    | SQL_BUFFER_RESULT
    | SQL_NO_CACHE | SQL_CACHE
    | SQL_CALC_FOUND_ROWS
    ;

selectElements
    : ASTERISK
    | selectElement (COMMA selectElement)*
    ;

selectElement
    : tableName DOT ASTERISK
    | expression (AS? alias)?
    ;

fromClause
    : FROM tableSources
    ;

tableSources
    : tableSource (COMMA tableSource)*
    ;

tableSource
    : tableSourceItem joinPart*
    | OPEN_PAREN tableSource CLOSE_PAREN
    ;

tableSourceItem
    : tableName partitionClause? (AS? alias)? indexHint*
    | (LATERAL? subquery | OPEN_PAREN tableSources CLOSE_PAREN) (AS? alias)?
    | jsonTableExpression (AS? alias)?
    ;

partitionClause
    : PARTITION OPEN_PAREN partitionName (COMMA partitionName)* CLOSE_PAREN
    ;

indexHint
    : (USE | IGNORE | FORCE) (INDEX | KEY) (FOR (JOIN | ORDER BY | GROUP BY))?
      OPEN_PAREN indexName (COMMA indexName)* CLOSE_PAREN
    ;

joinPart
    : (INNER | CROSS)? JOIN tableSourceItem joinCondition?
    | STRAIGHT_JOIN tableSourceItem joinCondition?
    | (LEFT | RIGHT) OUTER? JOIN tableSourceItem joinCondition
    | NATURAL ((LEFT | RIGHT) OUTER?)? JOIN tableSourceItem
    ;

joinCondition
    : ON expression
    | USING OPEN_PAREN columnList CLOSE_PAREN
    ;

whereClause
    : WHERE expression
    ;

groupByClause
    : GROUP BY groupByItem (COMMA groupByItem)* (WITH ROLLUP)?
    ;

groupByItem
    : expression orderDirection?
    ;

havingClause
    : HAVING expression
    ;

windowClause
    : WINDOW windowDefinition (COMMA windowDefinition)*
    ;

windowDefinition
    : windowName AS windowSpec
    ;

qualifyClause
    : QUALIFY expression
    ;

orderByClause
    : ORDER BY orderByItem (COMMA orderByItem)*
    ;

orderByItem
    : expression orderDirection? (NULLS (FIRST | LAST))?
    ;

orderDirection
    : ASC | DESC
    ;

limitClause
    : LIMIT ((offset COMMA)? rowCount | rowCount OFFSET offset)
    ;

offset
    : DECIMAL_LITERAL | QUESTION | USER_VAR
    ;

rowCount
    : DECIMAL_LITERAL | QUESTION | USER_VAR
    ;

lockingClause
    : FOR (UPDATE | SHARE) lockingClauseOption*
    | LOCK IN SHARE MODE
    ;

lockingClauseOption
    : OF tableName (COMMA tableName)*
    | NOWAIT
    | SKIP LOCKED
    ;

intoClause
    : INTO (OUTFILE stringLiteral (characterSet charsetName)? exportOptions?
           | DUMPFILE stringLiteral
           | (USER_VAR | variableName) (COMMA (USER_VAR | variableName))*)
    ;

exportOptions
    : FIELDS (TERMINATED BY stringLiteral)? (OPTIONALLY? ENCLOSED BY stringLiteral)?
             (ESCAPED BY stringLiteral)?
      (LINES (STARTING BY stringLiteral)? (TERMINATED BY stringLiteral)?)?
    ;

// -----------------------------------------------------------------------------
// INSERT Statement
// -----------------------------------------------------------------------------

insertStatement
    : INSERT insertPriority? IGNORE? INTO? tableName partitionClause?
      (columnList | SET assignmentList | selectStatement)
      (VALUES | VALUE) insertValues (COMMA insertValues)*
      asRowAlias?
      onDuplicateKeyUpdate?
    | INSERT insertPriority? IGNORE? INTO? tableName partitionClause?
      (OPEN_PAREN columnList CLOSE_PAREN)?
      selectStatement
      asRowAlias?
      onDuplicateKeyUpdate?
    | INSERT insertPriority? IGNORE? INTO? tableName partitionClause?
      SET assignmentList
      asRowAlias?
      onDuplicateKeyUpdate?
    ;

insertPriority
    : LOW_PRIORITY | DELAYED | HIGH_PRIORITY
    ;

insertValues
    : OPEN_PAREN (expression | DEFAULT) (COMMA (expression | DEFAULT))* CLOSE_PAREN
    | OPEN_PAREN CLOSE_PAREN
    ;

asRowAlias
    : AS alias (OPEN_PAREN columnList CLOSE_PAREN)?
    ;

onDuplicateKeyUpdate
    : ON DUPLICATE KEY UPDATE assignmentList
    ;

// -----------------------------------------------------------------------------
// UPDATE Statement
// -----------------------------------------------------------------------------

updateStatement
    : UPDATE updatePriority? IGNORE? tableSources
      SET assignmentList
      whereClause?
      orderByClause?
      limitClause?
    ;

updatePriority
    : LOW_PRIORITY
    ;

assignmentList
    : assignment (COMMA assignment)*
    ;

assignment
    : columnName EQUAL expression
    ;

// -----------------------------------------------------------------------------
// DELETE Statement
// -----------------------------------------------------------------------------

deleteStatement
    : DELETE deletePriority? QUICK? IGNORE?
      ( FROM tableName partitionClause?
        whereClause?
        orderByClause?
        limitClause?
      | tableName (DOT ASTERISK)? (COMMA tableName (DOT ASTERISK)?)*
        FROM tableSources
        whereClause?
      | FROM tableName (DOT ASTERISK)? (COMMA tableName (DOT ASTERISK)?)*
        USING tableSources
        whereClause?
      )
    ;

deletePriority
    : LOW_PRIORITY
    ;

// -----------------------------------------------------------------------------
// REPLACE Statement
// -----------------------------------------------------------------------------

replaceStatement
    : REPLACE insertPriority? INTO? tableName partitionClause?
      ( (OPEN_PAREN columnList CLOSE_PAREN)?
        ((VALUES | VALUE) insertValues (COMMA insertValues)* | selectStatement)
      | SET assignmentList
      )
    ;

// -----------------------------------------------------------------------------
// CALL Statement
// -----------------------------------------------------------------------------

callStatement
    : CALL procedureName (OPEN_PAREN expressionList? CLOSE_PAREN)?
    ;

// -----------------------------------------------------------------------------
// LOAD Statement
// -----------------------------------------------------------------------------

loadStatement
    : loadDataStatement
    | loadXmlStatement
    ;

loadDataStatement
    : LOAD DATA loadPriority? (LOCAL)? INFILE stringLiteral
      (REPLACE | IGNORE)? INTO TABLE tableName
      partitionClause?
      (characterSet charsetName)?
      loadDataOptions?
      (IGNORE DECIMAL_LITERAL (LINES | ROWS))?
      (OPEN_PAREN columnOrVariable (COMMA columnOrVariable)* CLOSE_PAREN)?
      (SET assignmentList)?
    ;

loadXmlStatement
    : LOAD XML loadPriority? (LOCAL)? INFILE stringLiteral
      (REPLACE | IGNORE)? INTO TABLE tableName
      partitionClause?
      (characterSet charsetName)?
      (ROWS IDENTIFIED BY stringLiteral)?
      (IGNORE DECIMAL_LITERAL (LINES | ROWS))?
      (OPEN_PAREN columnOrVariable (COMMA columnOrVariable)* CLOSE_PAREN)?
      (SET assignmentList)?
    ;

loadPriority
    : LOW_PRIORITY | CONCURRENT
    ;

loadDataOptions
    : ((FIELDS | COLUMNS)
       (TERMINATED BY stringLiteral)?
       (OPTIONALLY? ENCLOSED BY stringLiteral)?
       (ESCAPED BY stringLiteral)?)?
      (LINES
       (STARTING BY stringLiteral)?
       (TERMINATED BY stringLiteral)?)?
    ;

columnOrVariable
    : columnName | USER_VAR
    ;

// -----------------------------------------------------------------------------
// DO Statement
// -----------------------------------------------------------------------------

doStatement
    : DO expressionList
    ;

// -----------------------------------------------------------------------------
// HANDLER Statement
// -----------------------------------------------------------------------------

handlerStatement
    : HANDLER tableName OPEN (AS? alias)?
    | HANDLER tableName READ
      ( (FIRST | NEXT)
      | indexName ((FIRST | NEXT | PREV | LAST) | (EQUAL | LESS_THAN_OR_EQUAL | GREATER_THAN_OR_EQUAL | LESS_THAN | GREATER_THAN) 
        OPEN_PAREN expression (COMMA expression)* CLOSE_PAREN)
      )
      whereClause? limitClause?
    | HANDLER tableName CLOSE
    ;

// -----------------------------------------------------------------------------
// IMPORT Statement
// -----------------------------------------------------------------------------

importStatement
    : IMPORT TABLE FROM stringLiteral (COMMA stringLiteral)*
    ;

// -----------------------------------------------------------------------------
// TABLE Statement
// -----------------------------------------------------------------------------

tableStatement
    : TABLE tableName orderByClause? limitClause?
    ;

// -----------------------------------------------------------------------------
// VALUES Statement
// -----------------------------------------------------------------------------

valuesStatement
    : tableValueConstructor orderByClause? limitClause?
    ;

tableValueConstructor
    : VALUES rowConstructorList
    ;

rowConstructorList
    : ROW OPEN_PAREN expressionList CLOSE_PAREN (COMMA ROW OPEN_PAREN expressionList CLOSE_PAREN)*
    ;

// =============================================================================
// DATA CONTROL LANGUAGE (DCL)
// =============================================================================

dclStatement
    : grantStatement
    | revokeStatement
    | setPasswordStatement
    | setRoleStatement
    ;

grantStatement
    : GRANT privilegeList ON grantObject TO userOrRoleList grantOption*
    | GRANT roleName (COMMA roleName)* TO userOrRoleList (WITH ADMIN OPTION)?
    | GRANT PROXY ON userName TO userName (COMMA userName)* (WITH GRANT OPTION)?
    ;

revokeStatement
    : REVOKE privilegeList ON grantObject FROM userOrRoleList
    | REVOKE ALL PRIVILEGES? COMMA? GRANT OPTION FROM userOrRoleList
    | REVOKE roleName (COMMA roleName)* FROM userOrRoleList
    | REVOKE PROXY ON userName FROM userName (COMMA userName)*
    ;

privilegeList
    : privilege (COMMA privilege)*
    ;

privilege
    : privilegeType (OPEN_PAREN columnList CLOSE_PAREN)?
    ;

privilegeType
    : ALL PRIVILEGES?
    | ALTER ROUTINE?
    | CREATE (ROUTINE | TABLESPACE | TEMPORARY TABLES | USER | VIEW | ROLE)?
    | DELETE
    | DROP (ROLE)?
    | EVENT
    | EXECUTE
    | FILE
    | GRANT OPTION
    | INDEX
    | INSERT
    | LOCK TABLES
    | PROCESS
    | PROXY
    | REFERENCES
    | RELOAD
    | REPLICATION (CLIENT | SLAVE)
    | SELECT
    | SHOW (DATABASES | VIEW)
    | SHUTDOWN
    | SUPER
    | TRIGGER
    | UPDATE
    | USAGE
    | CREATE_SPATIAL_REFERENCE_SYSTEM     // MySQL 9.2+
    | APPLICATION_PASSWORD_ADMIN
    | AUDIT_ABORT_EXEMPT
    | AUDIT_ADMIN
    | AUTHENTICATION_POLICY_ADMIN
    | BACKUP_ADMIN
    | BINLOG_ADMIN
    | BINLOG_ENCRYPTION_ADMIN
    | CLONE_ADMIN
    | CONNECTION_ADMIN
    | ENCRYPTION_KEY_ADMIN
    | FIREWALL_ADMIN
    | FIREWALL_EXEMPT
    | FIREWALL_USER
    | FLUSH_OPTIMIZER_COSTS
    | FLUSH_STATUS
    | FLUSH_TABLES
    | FLUSH_USER_RESOURCES
    | GROUP_REPLICATION_ADMIN
    | GROUP_REPLICATION_STREAM
    | INNODB_REDO_LOG_ARCHIVE
    | INNODB_REDO_LOG_ENABLE
    | NDB_STORED_USER
    | PASSWORDLESS_USER_ADMIN
    | PERSIST_RO_VARIABLES_ADMIN
    | REPLICATION_APPLIER
    | REPLICATION_SLAVE_ADMIN
    | RESOURCE_GROUP_ADMIN
    | RESOURCE_GROUP_USER
    | ROLE_ADMIN
    | SENSITIVE_VARIABLES_OBSERVER
    | SERVICE_CONNECTION_ADMIN
    | SESSION_VARIABLES_ADMIN
    | SET_ANY_DEFINER
    | SHOW_ROUTINE
    | SKIP_QUERY_REWRITE
    | SYSTEM_USER
    | SYSTEM_VARIABLES_ADMIN
    | TABLE_ENCRYPTION_ADMIN
    | TELEMETRY_LOG_ADMIN
    | TP_CONNECTION_ADMIN
    | VERSION_TOKEN_ADMIN
    | XA_RECOVER_ADMIN
    ;

grantObject
    : ASTERISK DOT ASTERISK
    | databaseName DOT (ASTERISK | tableName | PROCEDURE procedureName | FUNCTION functionName)
    | tableName
    | TABLE tableName
    | FUNCTION functionName
    | PROCEDURE procedureName
    ;

userOrRoleList
    : userOrRole (COMMA userOrRole)*
    ;

userOrRole
    : userName | roleName
    ;

grantOption
    : WITH GRANT OPTION
    | AS userName (WITH ROLE (DEFAULT | NONE | ALL (EXCEPT roleName (COMMA roleName)*)? | roleName (COMMA roleName)*))?
    ;

setPasswordStatement
    : SET PASSWORD (FOR userName)? EQUAL (stringLiteral | PASSWORD OPEN_PAREN stringLiteral CLOSE_PAREN)
    ;

setRoleStatement
    : SET DEFAULT ROLE (NONE | ALL | roleName (COMMA roleName)*) TO userName (COMMA userName)*
    | SET ROLE (DEFAULT | NONE | ALL (EXCEPT roleName (COMMA roleName)*)? | roleName (COMMA roleName)*)
    ;

// =============================================================================
// TRANSACTION STATEMENTS
// =============================================================================

transactionStatement
    : startTransaction
    | commitStatement
    | rollbackStatement
    | savepointStatement
    | releaseSavepoint
    | lockStatement
    | unlockStatement
    | xaStatement
    | setTransactionStatement
    ;

startTransaction
    : START TRANSACTION transactionCharacteristic*
    | BEGIN WORK?
    ;

transactionCharacteristic
    : WITH CONSISTENT SNAPSHOT
    | READ (WRITE | ONLY)
    ;

commitStatement
    : COMMIT WORK? (AND NO? CHAIN)? (NO? RELEASE)?
    ;

rollbackStatement
    : ROLLBACK WORK? (AND NO? CHAIN)? (NO? RELEASE)?
    | ROLLBACK WORK? TO SAVEPOINT? savepointName
    ;

savepointStatement
    : SAVEPOINT savepointName
    ;

releaseSavepoint
    : RELEASE SAVEPOINT savepointName
    ;

lockStatement
    : LOCK TABLES lockTableElement (COMMA lockTableElement)*
    | LOCK INSTANCE FOR BACKUP
    ;

lockTableElement
    : tableName (AS? alias)? lockType
    ;

lockType
    : READ LOCAL?
    | LOW_PRIORITY? WRITE
    ;

unlockStatement
    : UNLOCK (TABLES | INSTANCE)
    ;

xaStatement
    : XA (START | BEGIN) xid (JOIN | RESUME)?
    | XA END xid (SUSPEND (FOR MIGRATE)?)?
    | XA PREPARE xid
    | XA COMMIT xid (ONE PHASE)?
    | XA ROLLBACK xid
    | XA RECOVER (CONVERT XID)?
    ;

xid
    : stringLiteral (COMMA stringLiteral (COMMA DECIMAL_LITERAL)?)?
    ;

setTransactionStatement
    : SET (GLOBAL | SESSION)? TRANSACTION transactionProperty (COMMA transactionProperty)*
    ;

transactionProperty
    : ISOLATION LEVEL transactionLevel
    | READ (WRITE | ONLY)
    ;

transactionLevel
    : REPEATABLE READ
    | READ COMMITTED
    | READ UNCOMMITTED
    | SERIALIZABLE
    ;

// =============================================================================
// REPLICATION STATEMENTS
// =============================================================================

replicationStatement
    : changeReplicationSource
    | changeReplicationFilter
    | startReplica
    | stopReplica
    | resetReplica
    | showReplicaStatus
    | showBinaryLogs
    | showBinlogEvents
    | purgeBinaryLogs
    ;

changeReplicationSource
    : CHANGE (REPLICATION SOURCE | MASTER) TO
      sourceOption (COMMA sourceOption)*
      (FOR CHANNEL channelName)?
    ;

sourceOption
    : SOURCE_HOST EQUAL stringLiteral
    | SOURCE_USER EQUAL stringLiteral
    | SOURCE_PASSWORD EQUAL stringLiteral
    | SOURCE_PORT EQUAL DECIMAL_LITERAL
    | SOURCE_LOG_FILE EQUAL stringLiteral
    | SOURCE_LOG_POS EQUAL DECIMAL_LITERAL
    | SOURCE_AUTO_POSITION EQUAL DECIMAL_LITERAL
    | SOURCE_BIND EQUAL stringLiteral
    | SOURCE_CONNECT_RETRY EQUAL DECIMAL_LITERAL
    | SOURCE_RETRY_COUNT EQUAL DECIMAL_LITERAL
    | SOURCE_DELAY EQUAL DECIMAL_LITERAL
    | SOURCE_HEARTBEAT_PERIOD EQUAL DECIMAL_LITERAL
    | SOURCE_SSL EQUAL DECIMAL_LITERAL                     // MySQL 9.5 default changed to 1
    | SOURCE_SSL_CA EQUAL stringLiteral
    | SOURCE_SSL_CAPATH EQUAL stringLiteral
    | SOURCE_SSL_CERT EQUAL stringLiteral
    | SOURCE_SSL_CIPHER EQUAL stringLiteral
    | SOURCE_SSL_CRL EQUAL stringLiteral
    | SOURCE_SSL_CRLPATH EQUAL stringLiteral
    | SOURCE_SSL_KEY EQUAL stringLiteral
    | SOURCE_SSL_VERIFY_SERVER_CERT EQUAL DECIMAL_LITERAL
    | SOURCE_TLS_VERSION EQUAL stringLiteral
    | SOURCE_TLS_CIPHERSUITES EQUAL stringLiteral
    | SOURCE_PUBLIC_KEY_PATH EQUAL stringLiteral
    | GET_SOURCE_PUBLIC_KEY EQUAL DECIMAL_LITERAL
    | SOURCE_COMPRESSION_ALGORITHMS EQUAL stringLiteral
    | SOURCE_ZSTD_COMPRESSION_LEVEL EQUAL DECIMAL_LITERAL
    | PRIVILEGE_CHECKS_USER EQUAL (userName | NULL)
    | REQUIRE_ROW_FORMAT EQUAL DECIMAL_LITERAL
    | REQUIRE_TABLE_PRIMARY_KEY_CHECK EQUAL (STREAM | ON | OFF | GENERATE)
    | ASSIGN_GTIDS_TO_ANONYMOUS_TRANSACTIONS EQUAL (OFF | LOCAL | stringLiteral)
    | GTID_ONLY EQUAL DECIMAL_LITERAL
    // Legacy MASTER_* options for compatibility
    | MASTER_HOST EQUAL stringLiteral
    | MASTER_USER EQUAL stringLiteral
    | MASTER_PASSWORD EQUAL stringLiteral
    | MASTER_PORT EQUAL DECIMAL_LITERAL
    | MASTER_LOG_FILE EQUAL stringLiteral
    | MASTER_LOG_POS EQUAL DECIMAL_LITERAL
    | MASTER_AUTO_POSITION EQUAL DECIMAL_LITERAL
    | MASTER_SSL EQUAL DECIMAL_LITERAL
    ;

changeReplicationFilter
    : CHANGE REPLICATION FILTER filterRule (COMMA filterRule)* (FOR CHANNEL channelName)?
    ;

filterRule
    : REPLICATE_DO_DB EQUAL OPEN_PAREN databaseName (COMMA databaseName)* CLOSE_PAREN
    | REPLICATE_IGNORE_DB EQUAL OPEN_PAREN databaseName (COMMA databaseName)* CLOSE_PAREN
    | REPLICATE_DO_TABLE EQUAL OPEN_PAREN tableName (COMMA tableName)* CLOSE_PAREN
    | REPLICATE_IGNORE_TABLE EQUAL OPEN_PAREN tableName (COMMA tableName)* CLOSE_PAREN
    | REPLICATE_WILD_DO_TABLE EQUAL OPEN_PAREN stringLiteral (COMMA stringLiteral)* CLOSE_PAREN
    | REPLICATE_WILD_IGNORE_TABLE EQUAL OPEN_PAREN stringLiteral (COMMA stringLiteral)* CLOSE_PAREN
    | REPLICATE_REWRITE_DB EQUAL OPEN_PAREN dbPair (COMMA dbPair)* CLOSE_PAREN
    ;

dbPair
    : OPEN_PAREN databaseName COMMA databaseName CLOSE_PAREN
    ;

startReplica
    : START (REPLICA | SLAVE) threadType* (UNTIL untilOption)? userOption* (FOR CHANNEL channelName)?
    ;

stopReplica
    : STOP (REPLICA | SLAVE) threadType* (FOR CHANNEL channelName)?
    ;

resetReplica
    : RESET (REPLICA | SLAVE) ALL? (FOR CHANNEL channelName)?
    ;

threadType
    : IO_THREAD | SQL_THREAD
    ;

untilOption
    : (SOURCE_LOG_FILE | MASTER_LOG_FILE) EQUAL stringLiteral COMMA 
      (SOURCE_LOG_POS | MASTER_LOG_POS) EQUAL DECIMAL_LITERAL
    | RELAY_LOG_FILE EQUAL stringLiteral COMMA RELAY_LOG_POS EQUAL DECIMAL_LITERAL
    | SQL_BEFORE_GTIDS EQUAL stringLiteral
    | SQL_AFTER_GTIDS EQUAL stringLiteral
    | SQL_AFTER_MTS_GAPS
    ;

userOption
    : USER EQUAL stringLiteral
    | PASSWORD EQUAL stringLiteral
    | DEFAULT_AUTH EQUAL stringLiteral
    | PLUGIN_DIR EQUAL stringLiteral
    ;

showReplicaStatus
    : SHOW (REPLICA | SLAVE) STATUS (FOR CHANNEL channelName)?
    ;

showBinaryLogs
    : SHOW (BINARY | MASTER) LOGS
    ;

showBinlogEvents
    : SHOW BINLOG EVENTS (IN stringLiteral)? (FROM DECIMAL_LITERAL)? limitClause?
    ;

purgeBinaryLogs
    : PURGE (BINARY | MASTER) LOGS (TO stringLiteral | BEFORE timestamp)
    ;

// =============================================================================
// PREPARED STATEMENTS
// =============================================================================

preparedStatement
    : prepareStatement
    | executeStatement
    | deallocateStatement
    ;

prepareStatement
    : PREPARE statementName FROM (stringLiteral | USER_VAR)
    ;

executeStatement
    : EXECUTE statementName (USING USER_VAR (COMMA USER_VAR)*)?
    ;

deallocateStatement
    : (DEALLOCATE | DROP) PREPARE statementName
    ;

// =============================================================================
// COMPOUND STATEMENTS (Stored Programs)
// =============================================================================

compoundStatement
    : blockStatement
    | caseStatement
    | ifStatement
    | loopStatement
    | repeatStatement
    | whileStatement
    | leaveStatement
    | iterateStatement
    | returnStatement
    | cursorStatement
    ;

blockStatement
    : (label COLON)? BEGIN
      declareStatement*
      (sqlStatement SEMICOLON)*
      END label?
    ;

declareStatement
    : DECLARE variableName (COMMA variableName)* dataType (DEFAULT defaultValue)? SEMICOLON
    | DECLARE variableName CONDITION FOR conditionValue SEMICOLON
    | DECLARE handlerAction HANDLER FOR conditionValue (COMMA conditionValue)* handlerStatement SEMICOLON
    | DECLARE cursorName CURSOR FOR selectStatement SEMICOLON
    ;

conditionValue
    : DECIMAL_LITERAL
    | SQLSTATE VALUE? stringLiteral
    | SQLWARNING
    | NOT FOUND
    | SQLEXCEPTION
    | ID
    ;

handlerAction
    : CONTINUE | EXIT | UNDO
    ;

handlerStatement
    : compoundStatement
    | sqlStatement
    ;

caseStatement
    : CASE expression?
      (WHEN expression THEN (sqlStatement SEMICOLON)+)+
      (ELSE (sqlStatement SEMICOLON)+)?
      END CASE
    ;

ifStatement
    : IF expression THEN (sqlStatement SEMICOLON)+
      (ELSEIF expression THEN (sqlStatement SEMICOLON)+)*
      (ELSE (sqlStatement SEMICOLON)+)?
      END IF
    ;

loopStatement
    : (label COLON)? LOOP
      (sqlStatement SEMICOLON)*
      END LOOP label?
    ;

repeatStatement
    : (label COLON)? REPEAT
      (sqlStatement SEMICOLON)*
      UNTIL expression
      END REPEAT label?
    ;

whileStatement
    : (label COLON)? WHILE expression DO
      (sqlStatement SEMICOLON)*
      END WHILE label?
    ;

leaveStatement
    : LEAVE label
    ;

iterateStatement
    : ITERATE label
    ;

returnStatement
    : RETURN expression
    ;

cursorStatement
    : OPEN cursorName
    | FETCH cursorName INTO variableName (COMMA variableName)*
    | CLOSE cursorName
    ;

// =============================================================================
// ADMINISTRATION STATEMENTS
// =============================================================================

administrationStatement
    : setStatement
    | showStatement
    | analyzeStatement
    | checkStatement
    | checksumStatement
    | optimizeStatement
    | repairStatement
    | installStatement
    | uninstallStatement
    | flushStatement
    | killStatement
    | resetStatement
    | restartStatement
    | shutdownStatement
    | cloneStatement
    | cacheIndexStatement
    | loadIndexStatement
    | explainStatement
    | describeStatement
    | helpStatement
    | useStatement
    | signalStatement
    | resignalStatement
    | getDiagnosticsStatement
    ;

// SET Statement
setStatement
    : SET setItem (COMMA setItem)*
    ;

setItem
    : variableAssignment
    | (CHARACTER SET | CHARSET) (charsetName | DEFAULT)
    | NAMES (charsetName (COLLATE collationName)? | DEFAULT)
    ;

variableAssignment
    : (GLOBAL | PERSIST | PERSIST_ONLY | SESSION)? variableName EQUAL expression
    | USER_VAR (EQUAL | ASSIGN_OP) expression
    | SYSTEM_VAR EQUAL expression
    ;

// SHOW Statement
showStatement
    : SHOW showTarget
    ;

showTarget
    : (FULL)? TABLES ((FROM | IN) databaseName)? showFilter?
    | (FULL)? COLUMNS (FROM | IN) tableName ((FROM | IN) databaseName)? showFilter?
    | (FULL)? PROCESSLIST
    | (GLOBAL | SESSION)? STATUS showFilter?
    | (GLOBAL | SESSION)? VARIABLES showFilter?
    | (STORAGE)? ENGINES
    | DATABASES showFilter?
    | SCHEMAS showFilter?
    | CREATE (DATABASE | SCHEMA) databaseName
    | CREATE TABLE tableName
    | CREATE VIEW viewName
    | CREATE PROCEDURE procedureName
    | CREATE FUNCTION functionName
    | CREATE TRIGGER triggerName
    | CREATE EVENT eventName
    | CREATE USER userName
    | EVENTS ((FROM | IN) databaseName)? showFilter?
    | FUNCTION STATUS showFilter?
    | PROCEDURE STATUS showFilter?
    | TRIGGERS ((FROM | IN) databaseName)? showFilter?
    | INDEX (FROM | IN) tableName ((FROM | IN) databaseName)?
    | KEYS (FROM | IN) tableName ((FROM | IN) databaseName)?
    | INDEXES (FROM | IN) tableName ((FROM | IN) databaseName)?
    | GRANTS (FOR userName (USING roleName (COMMA roleName)*)?)?
    | PRIVILEGES
    | WARNINGS limitClause?
    | ERRORS limitClause?
    | COUNT OPEN_PAREN ASTERISK CLOSE_PAREN (WARNINGS | ERRORS)
    | PLUGINS
    | PROFILE (ALL | BLOCK IO | CONTEXT SWITCHES | CPU | IPC | MEMORY | PAGE FAULTS | SOURCE | SWAPS)?
      (FOR QUERY DECIMAL_LITERAL)? limitClause?
    | PROFILES
    | (BINARY | MASTER) LOGS
    | BINLOG EVENTS (IN stringLiteral)? (FROM DECIMAL_LITERAL)? limitClause?
    | RELAYLOG EVENTS (IN stringLiteral)? (FROM DECIMAL_LITERAL)? limitClause? (FOR CHANNEL channelName)?
    | (REPLICA | SLAVE) HOSTS
    | (REPLICA | SLAVE) STATUS (FOR CHANNEL channelName)?
    | (MASTER | BINARY LOG) STATUS
    | CHARACTER SET showFilter?
    | COLLATION showFilter?
    | TABLE STATUS ((FROM | IN) databaseName)? showFilter?
    | OPEN TABLES ((FROM | IN) databaseName)? showFilter?
    | USER_STATISTICS
    | CLIENT_STATISTICS
    | INDEX_STATISTICS
    | TABLE_STATISTICS
    ;

showFilter
    : LIKE stringLiteral
    | WHERE expression
    ;

// ANALYZE Statement
analyzeStatement
    : ANALYZE (NO_WRITE_TO_BINLOG | LOCAL)? TABLE tableName (COMMA tableName)*
    | ANALYZE (NO_WRITE_TO_BINLOG | LOCAL)? TABLE tableName UPDATE HISTOGRAM ON columnName (COMMA columnName)*
      (WITH DECIMAL_LITERAL BUCKETS)?
    | ANALYZE (NO_WRITE_TO_BINLOG | LOCAL)? TABLE tableName DROP HISTOGRAM ON columnName (COMMA columnName)*
    ;

// CHECK Statement
checkStatement
    : CHECK TABLE tableName (COMMA tableName)* checkOption*
    ;

checkOption
    : FOR UPGRADE | QUICK | FAST | MEDIUM | EXTENDED | CHANGED
    ;

// CHECKSUM Statement
checksumStatement
    : CHECKSUM TABLE tableName (COMMA tableName)* (QUICK | EXTENDED)?
    ;

// OPTIMIZE Statement
optimizeStatement
    : OPTIMIZE (NO_WRITE_TO_BINLOG | LOCAL)? TABLE tableName (COMMA tableName)*
    ;

// REPAIR Statement
repairStatement
    : REPAIR (NO_WRITE_TO_BINLOG | LOCAL)? TABLE tableName (COMMA tableName)* repairOption*
    ;

repairOption
    : QUICK | EXTENDED | USE_FRM
    ;

// INSTALL/UNINSTALL Statements
installStatement
    : INSTALL PLUGIN pluginName SONAME stringLiteral
    | INSTALL COMPONENT componentName (COMMA componentName)* setStatement?
    ;

uninstallStatement
    : UNINSTALL PLUGIN pluginName
    | UNINSTALL COMPONENT componentName (COMMA componentName)*
    ;

// FLUSH Statement
flushStatement
    : FLUSH (NO_WRITE_TO_BINLOG | LOCAL)? flushOption (COMMA flushOption)*
    ;

flushOption
    : (BINARY | ENGINE | ERROR | GENERAL | RELAY | SLOW)? LOGS
    | HOSTS
    | OPTIMIZER_COSTS
    | PRIVILEGES
    | QUERY CACHE
    | STATUS
    | USER_RESOURCES
    | TABLES (tableName (COMMA tableName)*)? flushTableOption?
    ;

flushTableOption
    : WITH READ LOCK
    | FOR EXPORT
    ;

// KILL Statement
killStatement
    : KILL (CONNECTION | QUERY)? DECIMAL_LITERAL
    ;

// RESET Statement
resetStatement
    : RESET resetOption (COMMA resetOption)*
    ;

resetOption
    : (BINARY LOGS AND GTIDS | MASTER | REPLICA ALL? | SLAVE ALL? | QUERY CACHE)
      (TO DECIMAL_LITERAL)?
    ;

// RESTART Statement
restartStatement
    : RESTART
    ;

// SHUTDOWN Statement
shutdownStatement
    : SHUTDOWN
    ;

// CLONE Statement
cloneStatement
    : CLONE LOCAL DATA DIRECTORY EQUAL? stringLiteral
    | CLONE INSTANCE FROM userName AT stringLiteral COLON DECIMAL_LITERAL
      IDENTIFIED BY stringLiteral
      (DATA DIRECTORY EQUAL? stringLiteral)?
      (REQUIRE NO? SSL)?
    ;

// CACHE INDEX Statement
cacheIndexStatement
    : CACHE INDEX tableName (COMMA tableName)* (PARTITION OPEN_PAREN (partitionName (COMMA partitionName)* | ALL) CLOSE_PAREN)?
      IN cacheName
    ;

// LOAD INDEX Statement
loadIndexStatement
    : LOAD INDEX INTO CACHE tableName (COMMA tableName)* (PARTITION OPEN_PAREN (partitionName (COMMA partitionName)* | ALL) CLOSE_PAREN)?
      (IGNORE LEAVES)?
    ;

// EXPLAIN Statement
explainStatement
    : (EXPLAIN | DESCRIBE | DESC) explainTarget
    ;

explainTarget
    : tableName columnName?
    | explainFormat? (explainableStatement | FOR CONNECTION DECIMAL_LITERAL)
    ;

explainFormat
    : (FORMAT EQUAL (TRADITIONAL | JSON | TREE))?
    | ANALYZE (FORMAT EQUAL JSON (INTO USER_VAR)? (FOR (SCHEMA | DATABASE) databaseName)?)?  // MySQL 9.0+ JSON output
    | EXTENDED
    | PARTITIONS
    ;

explainableStatement
    : selectStatement
    | deleteStatement
    | insertStatement
    | replaceStatement
    | updateStatement
    ;

// DESCRIBE Statement
describeStatement
    : (DESCRIBE | DESC) tableName (columnName | stringLiteral)?
    ;

// HELP Statement
helpStatement
    : HELP stringLiteral
    ;

// USE Statement
useStatement
    : USE databaseName
    ;

// SIGNAL Statement
signalStatement
    : SIGNAL (SQLSTATE VALUE? stringLiteral | ID)
      (SET signalInfo (COMMA signalInfo)*)?
    ;

signalInfo
    : signalCondition EQUAL expression
    ;

signalCondition
    : CLASS_ORIGIN | SUBCLASS_ORIGIN | MESSAGE_TEXT | MYSQL_ERRNO
    | CONSTRAINT_CATALOG | CONSTRAINT_SCHEMA | CONSTRAINT_NAME
    | CATALOG_NAME | SCHEMA_NAME | TABLE_NAME | COLUMN_NAME | CURSOR_NAME
    ;

// RESIGNAL Statement
resignalStatement
    : RESIGNAL (SQLSTATE VALUE? stringLiteral | ID)?
      (SET signalInfo (COMMA signalInfo)*)?
    ;

// GET DIAGNOSTICS Statement
getDiagnosticsStatement
    : GET (CURRENT | STACKED)? DIAGNOSTICS
      ( (USER_VAR | variableName) EQUAL (NUMBER | ROW_COUNT) (COMMA (USER_VAR | variableName) EQUAL (NUMBER | ROW_COUNT))*
      | CONDITION (DECIMAL_LITERAL | USER_VAR | variableName) conditionInfo (COMMA conditionInfo)*
      )
    ;

conditionInfo
    : (USER_VAR | variableName) EQUAL signalCondition
    ;

// =============================================================================
// UTILITY STATEMENTS
// =============================================================================

utilityStatement
    : binlogStatement
    ;

binlogStatement
    : BINLOG stringLiteral
    ;

// =============================================================================
// MySQL AI / HeatWave ML STATEMENTS
// =============================================================================

mlStatement
    : mlTrainStatement
    | mlPredictStatement
    | mlExplainStatement
    | mlScoreStatement
    | mlModelStatement
    | mlGenerateStatement
    | mlEmbedStatement
    | mlNlSqlStatement
    ;

// ML_TRAIN - Train a machine learning model
mlTrainStatement
    : CALL (SYS DOT)? ML_TRAIN
      OPEN_PAREN
        stringLiteral COMMA                           // training table
        stringLiteral COMMA                           // target column
        jsonObject COMMA                              // options (task, etc.)
        USER_VAR                                      // model handle output
      CLOSE_PAREN
    ;

// ML_PREDICT_ROW / ML_PREDICT_TABLE - Generate predictions
mlPredictStatement
    : SELECT (SYS DOT)? ML_PREDICT_ROW
      OPEN_PAREN
        expression COMMA                              // row data (JSON)
        (USER_VAR | stringLiteral) COMMA              // model handle
        expression                                    // options
      CLOSE_PAREN (AS? alias)?
    | CALL (SYS DOT)? ML_PREDICT_TABLE
      OPEN_PAREN
        stringLiteral COMMA                           // input table
        (USER_VAR | stringLiteral) COMMA              // model handle
        stringLiteral COMMA                           // output table
        expression                                    // options
      CLOSE_PAREN
    ;

// ML_EXPLAIN_ROW / ML_EXPLAIN_TABLE - Generate prediction explanations
mlExplainStatement
    : SELECT (SYS DOT)? ML_EXPLAIN_ROW
      OPEN_PAREN
        expression COMMA                              // row data (JSON)
        (USER_VAR | stringLiteral) COMMA              // model handle
        expression                                    // options
      CLOSE_PAREN (AS? alias)?
    | CALL (SYS DOT)? ML_EXPLAIN_TABLE
      OPEN_PAREN
        stringLiteral COMMA                           // input table
        (USER_VAR | stringLiteral) COMMA              // model handle
        stringLiteral COMMA                           // output table
        expression                                    // options
      CLOSE_PAREN
    ;

// ML_SCORE - Score model performance
mlScoreStatement
    : CALL (SYS DOT)? ML_SCORE
      OPEN_PAREN
        stringLiteral COMMA                           // test table
        stringLiteral COMMA                           // target column
        (USER_VAR | stringLiteral) COMMA              // model handle
        stringLiteral COMMA                           // metric
        USER_VAR                                      // score output
        (COMMA expression)?                           // options
      CLOSE_PAREN
    ;

// ML_MODEL_LOAD / ML_MODEL_UNLOAD / ML_MODEL_IMPORT / ML_MODEL_EXPORT
mlModelStatement
    : CALL (SYS DOT)? ML_MODEL_LOAD
      OPEN_PAREN
        (USER_VAR | stringLiteral)                    // model handle
        (COMMA expression)?                           // options
      CLOSE_PAREN
    | CALL (SYS DOT)? ML_MODEL_UNLOAD
      OPEN_PAREN
        (USER_VAR | stringLiteral)                    // model handle
      CLOSE_PAREN
    | CALL (SYS DOT)? ML_MODEL_IMPORT
      OPEN_PAREN
        expression COMMA                              // model data
        expression                                    // options
      CLOSE_PAREN
    | CALL (SYS DOT)? ML_MODEL_EXPORT
      OPEN_PAREN
        (USER_VAR | stringLiteral) COMMA              // model handle
        USER_VAR                                      // output
      CLOSE_PAREN
    ;

// ML_GENERATE / ML_GENERATE_TABLE - Generate content using LLMs (HeatWave GenAI)
mlGenerateStatement
    : SELECT (SYS DOT)? ML_GENERATE
      OPEN_PAREN
        expression                                    // prompt
        (COMMA expression)?                           // options (model, etc.)
      CLOSE_PAREN (AS? alias)?
    | CALL (SYS DOT)? ML_GENERATE_TABLE
      OPEN_PAREN
        stringLiteral COMMA                           // input table
        stringLiteral COMMA                           // prompt column
        stringLiteral COMMA                           // output table
        expression                                    // options
      CLOSE_PAREN
    ;

// ML_EMBED / ML_EMBED_TABLE - Generate embeddings
mlEmbedStatement
    : SELECT (SYS DOT)? ML_EMBED
      OPEN_PAREN
        expression                                    // text to embed
        (COMMA expression)?                           // options
      CLOSE_PAREN (AS? alias)?
    | CALL (SYS DOT)? ML_EMBED_TABLE
      OPEN_PAREN
        stringLiteral COMMA                           // input table  
        stringLiteral COMMA                           // text column
        stringLiteral COMMA                           // output table
        expression                                    // options
      CLOSE_PAREN
    ;

// ML_NL_SQL - Natural language to SQL (NL2SQL)
mlNlSqlStatement
    : SELECT (SYS DOT)? ML_NL_SQL
      OPEN_PAREN
        expression                                    // natural language query
        (COMMA expression)?                           // options
      CLOSE_PAREN (AS? alias)?
    ;

// =============================================================================
// EXPRESSIONS
// =============================================================================

expression
    : notOperator=(NOT | EXCLAMATION) expression
    | expression logicalOperator expression
    | predicate IS NOT? (TRUE | FALSE | UNKNOWN | NULL)
    | predicate
    ;

predicate
    : predicate NOT? IN OPEN_PAREN (selectStatement | expressionList) CLOSE_PAREN
    | predicate NOT? BETWEEN predicate AND predicate
    | predicate NOT? LIKE predicate (ESCAPE stringLiteral)?
    | predicate NOT? (REGEXP | RLIKE) predicate
    | predicate SOUNDS LIKE predicate
    | predicate comparisonOperator predicate
    | predicate comparisonOperator (ALL | ANY | SOME) OPEN_PAREN selectStatement CLOSE_PAREN
    | primaryExpression MEMBER OF OPEN_PAREN primaryExpression CLOSE_PAREN
    | primaryExpression
    ;

primaryExpression
    : primaryExpression (PLUS | MINUS | ASTERISK | SLASH | DIV | MOD | PERCENT) primaryExpression
    | primaryExpression (AMPERSAND | PIPE | CARET | LEFT_SHIFT | RIGHT_SHIFT) primaryExpression
    | (PLUS | MINUS | TILDE | EXCLAMATION | BINARY) primaryExpression
    | NOT? EXISTS OPEN_PAREN selectStatement CLOSE_PAREN
    | OPEN_PAREN selectStatement CLOSE_PAREN
    | OPEN_PAREN expressionList CLOSE_PAREN
    | ROW OPEN_PAREN expressionList CLOSE_PAREN
    | functionCall
    | caseExpression
    | intervalExpression
    | literal
    | columnRef
    | variable
    | OPEN_PAREN expression CLOSE_PAREN
    | primaryExpression COLLATE collationName
    | primaryExpression jsonOperator stringLiteral
    ;

functionCall
    : aggregateFunction
    | windowFunction
    | specificFunction
    | vectorFunction                                  // MySQL 9.0+ Vector functions
    | builtInFunction
    | userDefinedFunction
    ;

aggregateFunction
    : (AVG | MAX | MIN | SUM | COUNT | BIT_AND | BIT_OR | BIT_XOR | STD | STDDEV | STDDEV_POP
       | STDDEV_SAMP | VAR_POP | VAR_SAMP | VARIANCE | GROUP_CONCAT | JSON_ARRAYAGG | JSON_OBJECTAGG)
      OPEN_PAREN aggregateModifier? (DISTINCT)? expressionList? orderByClause? 
      (SEPARATOR stringLiteral)? CLOSE_PAREN overClause?
    | COUNT OPEN_PAREN ASTERISK CLOSE_PAREN overClause?
    ;

aggregateModifier
    : ALL | DISTINCT
    ;

windowFunction
    : (ROW_NUMBER | RANK | DENSE_RANK | CUME_DIST | PERCENT_RANK | NTILE)
      OPEN_PAREN expressionList? CLOSE_PAREN overClause
    | (FIRST_VALUE | LAST_VALUE | NTH_VALUE | LAG | LEAD)
      OPEN_PAREN expression (COMMA expression)* nullTreatment? CLOSE_PAREN overClause
    ;

overClause
    : OVER (windowName | windowSpec)
    ;

windowSpec
    : OPEN_PAREN windowName? partitionClause? orderByClause? frameClause? CLOSE_PAREN
    ;

frameClause
    : frameUnits frameExtent exclusion?
    ;

frameUnits
    : ROWS | RANGE | GROUPS
    ;

frameExtent
    : frameBound
    | BETWEEN frameBound AND frameBound
    ;

frameBound
    : UNBOUNDED (PRECEDING | FOLLOWING)
    | CURRENT ROW
    | expression (PRECEDING | FOLLOWING)
    ;

exclusion
    : EXCLUDE (CURRENT ROW | GROUP | TIES | NO OTHERS)
    ;

nullTreatment
    : RESPECT NULLS | IGNORE NULLS
    ;

// MySQL 9.0+ Vector Functions
vectorFunction
    : STRING_TO_VECTOR OPEN_PAREN expression CLOSE_PAREN
    | TO_VECTOR OPEN_PAREN expression CLOSE_PAREN
    | VECTOR_TO_STRING OPEN_PAREN expression CLOSE_PAREN
    | FROM_VECTOR OPEN_PAREN expression CLOSE_PAREN
    | VECTOR_DIM OPEN_PAREN expression CLOSE_PAREN
    ;

specificFunction
    : CURRENT_DATE
    | CURRENT_TIME (OPEN_PAREN DECIMAL_LITERAL? CLOSE_PAREN)?
    | CURRENT_TIMESTAMP (OPEN_PAREN DECIMAL_LITERAL? CLOSE_PAREN)?
    | CURRENT_USER
    | LOCALTIME (OPEN_PAREN DECIMAL_LITERAL? CLOSE_PAREN)?
    | LOCALTIMESTAMP (OPEN_PAREN DECIMAL_LITERAL? CLOSE_PAREN)?
    | UTC_DATE
    | UTC_TIME
    | UTC_TIMESTAMP
    | CAST OPEN_PAREN expression AS dataType (ARRAY)? CLOSE_PAREN
    | CONVERT OPEN_PAREN expression (COMMA dataType | USING charsetName) CLOSE_PAREN
    | TRIM OPEN_PAREN ((BOTH | LEADING | TRAILING)? expression? FROM)? expression CLOSE_PAREN
    | WEIGHT_STRING OPEN_PAREN expression (AS (CHAR | BINARY) OPEN_PAREN DECIMAL_LITERAL CLOSE_PAREN)?
      (LEVEL levelList)? CLOSE_PAREN
    | EXTRACT OPEN_PAREN intervalUnit FROM expression CLOSE_PAREN
    | GET_FORMAT OPEN_PAREN (DATE | TIME | DATETIME | TIMESTAMP) COMMA stringLiteral CLOSE_PAREN
    | JSON_VALUE OPEN_PAREN expression COMMA stringLiteral 
      (RETURNING dataType)? (NULL | ERROR | DEFAULT expression ON EMPTY)?
      (NULL | ERROR | DEFAULT expression ON ERROR)? CLOSE_PAREN
    | POSITION OPEN_PAREN expression IN expression CLOSE_PAREN
    | SUBSTRING OPEN_PAREN expression (COMMA | FROM) expression ((COMMA | FOR) expression)? CLOSE_PAREN
    | VALUES OPEN_PAREN columnName CLOSE_PAREN
    ;

levelList
    : levelItem (COMMA levelItem)*
    ;

levelItem
    : DECIMAL_LITERAL (ASC | DESC | REVERSE)?
    | DECIMAL_LITERAL MINUS DECIMAL_LITERAL
    ;

builtInFunction
    : functionName OPEN_PAREN expressionList? CLOSE_PAREN
    ;

userDefinedFunction
    : functionName OPEN_PAREN expressionList? CLOSE_PAREN
    ;

caseExpression
    : CASE expression? (WHEN expression THEN expression)+ (ELSE expression)? END
    ;

intervalExpression
    : INTERVAL expression intervalUnit
    ;

intervalValue
    : expression intervalUnit
    ;

intervalUnit
    : MICROSECOND | SECOND | MINUTE | HOUR | DAY | WEEK | MONTH | QUARTER | YEAR
    | SECOND_MICROSECOND | MINUTE_MICROSECOND | MINUTE_SECOND
    | HOUR_MICROSECOND | HOUR_SECOND | HOUR_MINUTE
    | DAY_MICROSECOND | DAY_SECOND | DAY_MINUTE | DAY_HOUR
    | YEAR_MONTH
    ;

expressionList
    : expression (COMMA expression)*
    ;

// JSON Table Expression
jsonTableExpression
    : JSON_TABLE OPEN_PAREN expression COMMA stringLiteral COLUMNS 
      OPEN_PAREN jsonTableColumn (COMMA jsonTableColumn)* CLOSE_PAREN
      CLOSE_PAREN
    ;

jsonTableColumn
    : columnName (FOR ORDINALITY | dataType (PATH stringLiteral)? jsonTableColumnOption*)
    | NESTED PATH? stringLiteral COLUMNS OPEN_PAREN jsonTableColumn (COMMA jsonTableColumn)* CLOSE_PAREN
    ;

jsonTableColumnOption
    : (NULL | ERROR | DEFAULT expression) ON (EMPTY | ERROR)
    | EXISTS
    ;

// =============================================================================
// LITERALS
// =============================================================================

literal
    : stringLiteral
    | numericLiteral
    | booleanLiteral
    | hexLiteral
    | bitLiteral
    | NULL
    ;

stringLiteral
    : (SINGLE_QUOTED_STRING | DOUBLE_QUOTED_STRING)+
    | characterSet? (SINGLE_QUOTED_STRING | DOUBLE_QUOTED_STRING)+ (COLLATE collationName)?
    ;

numericLiteral
    : DECIMAL_LITERAL
    | REAL_LITERAL
    | (PLUS | MINUS) (DECIMAL_LITERAL | REAL_LITERAL)
    ;

booleanLiteral
    : TRUE | FALSE
    ;

hexLiteral
    : HEX_STRING
    ;

bitLiteral
    : BIT_STRING
    ;

// JSON Object
jsonObject
    : JSON_OBJECT OPEN_PAREN (jsonKeyValue (COMMA jsonKeyValue)*)? CLOSE_PAREN
    | OPEN_BRACE (jsonKeyValue (COMMA jsonKeyValue)*)? CLOSE_BRACE
    ;

jsonKeyValue
    : stringLiteral COMMA expression
    | stringLiteral COLON expression
    ;

// =============================================================================
// IDENTIFIERS AND REFERENCES
// =============================================================================

// Names
databaseName    : identifier;
tableName       : (databaseName DOT)? identifier;
viewName        : (databaseName DOT)? identifier;
columnName      : ((databaseName DOT)? identifier DOT)? identifier;
indexName       : identifier;
constraintName  : identifier;
procedureName   : (databaseName DOT)? identifier;
functionName    : (databaseName DOT)? identifier;
triggerName     : (databaseName DOT)? identifier;
eventName       : (databaseName DOT)? identifier;
serverName      : identifier;
tablespaceName  : identifier;
logfileGroupName: identifier;
userName        : stringLiteral (AT_SIGN (stringLiteral | PERCENT | identifier))?
                | identifier (AT_SIGN (stringLiteral | PERCENT | identifier))?
                ;
roleName        : identifier | stringLiteral;
engineName      : identifier;
parserName      : identifier;
charsetName     : identifier | stringLiteral | BINARY;
collationName   : identifier | stringLiteral;
partitionName   : identifier;
channelName     : stringLiteral;
cacheName       : identifier;
savepointName   : identifier;
pluginName      : identifier;
componentName   : stringLiteral;
windowName      : identifier;
cursorName      : identifier;
statementName   : identifier;
authPlugin      : identifier | stringLiteral;
wrapperName     : identifier | stringLiteral;
fkSymbol        : identifier;
label           : identifier;
alias           : identifier | stringLiteral;
parameterName   : identifier;
variableName    : identifier;

// Column reference
columnRef
    : ((databaseName DOT)? tableName DOT)? columnName
    ;

// Column list
columnList
    : columnName (COMMA columnName)*
    ;

// Variable
variable
    : USER_VAR
    | SYSTEM_VAR
    ;

// Identifier
identifier
    : ID
    | UNICODE_ID
    | BACKTICK_QUOTED_ID
    | nonReservedKeyword
    ;

// Subquery
subquery
    : OPEN_PAREN selectStatement CLOSE_PAREN
    ;

// JSON operators
jsonOperator
    : ARROW
    | DOUBLE_ARROW
    ;

// Comparison operators
comparisonOperator
    : EQUAL
    | GREATER_THAN
    | LESS_THAN
    | LESS_THAN_OR_EQUAL
    | GREATER_THAN_OR_EQUAL
    | NOT_EQUAL
    | NULL_SAFE_EQUAL
    ;

// Logical operators
logicalOperator
    : AND | DOUBLE_AMPERSAND
    | OR | DOUBLE_PIPE
    | XOR
    ;

// Column format
columnFormat
    : FIXED | DYNAMIC | DEFAULT
    ;

// Storage type
storageType
    : DISK | MEMORY
    ;

// Character set specification
characterSet
    : (CHARACTER SET | CHARSET | CHAR SET) charsetName
    | UNDERSCORE_CHARSET
    ;

fragment UNDERSCORE_CHARSET
    : '_' [a-zA-Z0-9]+
    ;

// Timestamp
timestamp
    : CURRENT_TIMESTAMP
    | stringLiteral
    | DECIMAL_LITERAL
    ;

// Definer clause
definerClause
    : DEFINER EQUAL userName
    ;

// If exists/not exists
ifExists
    : IF EXISTS
    ;

ifNotExists
    : IF NOT EXISTS
    ;

// MySQL Internal keywords (for MySQL internal structures)
INNODB:                         'INNODB';
MYSQL_MAIN:                     'mysql_main';
MYSQL_ADMIN:                    'mysql_admin';
ASSIGN_GTIDS_TO_ANONYMOUS_TRANSACTIONS: 'ASSIGN_GTIDS_TO_ANONYMOUS_TRANSACTIONS';

// Additional privilege types (MySQL 9.x)
APPLICATION_PASSWORD_ADMIN:     'APPLICATION_PASSWORD_ADMIN';
AUDIT_ABORT_EXEMPT:             'AUDIT_ABORT_EXEMPT';
AUDIT_ADMIN:                    'AUDIT_ADMIN';
AUTHENTICATION_POLICY_ADMIN:    'AUTHENTICATION_POLICY_ADMIN';
BACKUP_ADMIN:                   'BACKUP_ADMIN';
BINLOG_ADMIN:                   'BINLOG_ADMIN';
BINLOG_ENCRYPTION_ADMIN:        'BINLOG_ENCRYPTION_ADMIN';
CLONE_ADMIN:                    'CLONE_ADMIN';
CONNECTION_ADMIN:               'CONNECTION_ADMIN';
ENCRYPTION_KEY_ADMIN:           'ENCRYPTION_KEY_ADMIN';
FIREWALL_ADMIN:                 'FIREWALL_ADMIN';
FIREWALL_EXEMPT:                'FIREWALL_EXEMPT';
FIREWALL_USER:                  'FIREWALL_USER';
FLUSH_OPTIMIZER_COSTS:          'FLUSH_OPTIMIZER_COSTS';
FLUSH_STATUS:                   'FLUSH_STATUS';
FLUSH_TABLES:                   'FLUSH_TABLES';
FLUSH_USER_RESOURCES:           'FLUSH_USER_RESOURCES';
GROUP_REPLICATION_ADMIN:        'GROUP_REPLICATION_ADMIN';
GROUP_REPLICATION_STREAM:       'GROUP_REPLICATION_STREAM';
INNODB_REDO_LOG_ARCHIVE:        'INNODB_REDO_LOG_ARCHIVE';
INNODB_REDO_LOG_ENABLE:         'INNODB_REDO_LOG_ENABLE';
NDB_STORED_USER:                'NDB_STORED_USER';
PASSWORDLESS_USER_ADMIN:        'PASSWORDLESS_USER_ADMIN';
PERSIST_RO_VARIABLES_ADMIN:     'PERSIST_RO_VARIABLES_ADMIN';
REPLICATION_APPLIER:            'REPLICATION_APPLIER';
REPLICATION_SLAVE_ADMIN:        'REPLICATION_SLAVE_ADMIN';
RESOURCE_GROUP_ADMIN:           'RESOURCE_GROUP_ADMIN';
RESOURCE_GROUP_USER:            'RESOURCE_GROUP_USER';
ROLE_ADMIN:                     'ROLE_ADMIN';
SENSITIVE_VARIABLES_OBSERVER:   'SENSITIVE_VARIABLES_OBSERVER';
SERVICE_CONNECTION_ADMIN:       'SERVICE_CONNECTION_ADMIN';
SESSION_VARIABLES_ADMIN:        'SESSION_VARIABLES_ADMIN';
SET_ANY_DEFINER:                'SET_ANY_DEFINER';
SHOW_ROUTINE:                   'SHOW_ROUTINE';
SKIP_QUERY_REWRITE:             'SKIP_QUERY_REWRITE';
SYSTEM_USER:                    'SYSTEM_USER';
SYSTEM_VARIABLES_ADMIN:         'SYSTEM_VARIABLES_ADMIN';
TABLE_ENCRYPTION_ADMIN:         'TABLE_ENCRYPTION_ADMIN';
TELEMETRY_LOG_ADMIN:            'TELEMETRY_LOG_ADMIN';
TP_CONNECTION_ADMIN:            'TP_CONNECTION_ADMIN';
VERSION_TOKEN_ADMIN:            'VERSION_TOKEN_ADMIN';
XA_RECOVER_ADMIN:               'XA_RECOVER_ADMIN';
CREATE_SPATIAL_REFERENCE_SYSTEM:'CREATE_SPATIAL_REFERENCE_SYSTEM';

// Show targets (not reserved)
USER_STATISTICS:                'USER_STATISTICS';
CLIENT_STATISTICS:              'CLIENT_STATISTICS';
INDEX_STATISTICS:               'INDEX_STATISTICS';
TABLE_STATISTICS:               'TABLE_STATISTICS';

// Non-reserved keywords that can be used as identifiers
nonReservedKeyword
    : ACCOUNT | ACTION | ACTIVE | ADMIN | AFTER | AGAINST | AGGREGATE | ALGORITHM
    | ALWAYS | ANY | ARRAY | AT | ATTRIBUTE | AUTHENTICATION | AUTOEXTEND_SIZE
    | AUTO_INCREMENT | AVG | AVG_ROW_LENGTH | BACKUP | BEGIN | BINLOG | BIT
    | BLOCK | BOOL | BOOLEAN | BTREE | BUCKETS | BULK | BYTE | CACHE | CASCADED
    | CATALOG_NAME | CHAIN | CHALLENGE_RESPONSE | CHANGED | CHANNEL | CHARSET
    | CHECKSUM | CIPHER | CLASS_ORIGIN | CLIENT | CLONE | CLOSE | COALESCE | CODE
    | COLLATION | COLUMN_FORMAT | COLUMN_NAME | COLUMNS | COMMENT | COMMIT
    | COMMITTED | COMPACT | COMPLETION | COMPONENT | COMPRESSED | COMPRESSION
    | CONCURRENT | CONNECTION | CONSISTENT | CONSTRAINT_CATALOG | CONSTRAINT_NAME
    | CONSTRAINT_SCHEMA | CONTAINS | CONTEXT | CPU | CURRENT | CURSOR_NAME | DATA
    | DATAFILE | DATE | DATETIME | DAY | DEALLOCATE | DEFAULT_AUTH | DEFINER
    | DEFINITION | DELAY_KEY_WRITE | DESCRIPTION | DES_KEY_FILE | DIAGNOSTICS
    | DIRECTORY | DISABLE | DISCARD | DISK | DO | DUMPFILE | DUPLICATE | DYNAMIC
    | ENABLE | ENCRYPTION | END | ENDS | ENFORCED | ENGINE | ENGINES
    | ENGINE_ATTRIBUTE | ENUM | ERROR | ERRORS | ESCAPE | EVENT | EVENTS | EVERY
    | EXCHANGE | EXCLUDE | EXECUTE | EXPANSION | EXPIRE | EXPORT | EXTENDED
    | EXTENT_SIZE | FACTOR | FAILED_LOGIN_ATTEMPTS | FAST | FAULTS | FIELDS
    | FILE | FILE_BLOCK_SIZE | FILTER | FINISH | FIRST | FIXED | FLUSH | FOLLOWING
    | FOLLOWS | FORMAT | FOUND | FULL | GENERAL | GENERATE | GEOMCOLLECTION
    | GEOMETRY | GEOMETRYCOLLECTION | GET_FORMAT | GET_SOURCE_PUBLIC_KEY | GLOBAL
    | GRANTS | GROUP_REPLICATION | GTID_ONLY | HANDLER | HASH | HELP | HISTOGRAM
    | HISTORY | HOST | HOSTS | HOUR | IDENTIFIED | IGNORE_SERVER_IDS | IMPORT
    | INACTIVE | INDEX | INDEXES | INITIAL | INITIAL_SIZE | INITIATE | INSERT_METHOD
    | INSTALL | INSTANCE | INVISIBLE | INVOKER | IO | IO_THREAD | IPC | ISOLATION
    | ISSUER | JAVASCRIPT | JSON | KEY_BLOCK_SIZE | KEYRING | LANGUAGE | LAST | LEAVES
    | LESS | LEVEL | LINESTRING | LIST | LOCAL | LOCALTIME | LOCALTIMESTAMP | LOCK
    | LOCKED | LOCKS | LOGFILE | LOGS | MASTER | MAX_CONNECTIONS_PER_HOUR
    | MAX_QUERIES_PER_HOUR | MAX_ROWS | MAX_SIZE | MAX_UPDATES_PER_HOUR
    | MAX_USER_CONNECTIONS | MEDIUM | MEMBER | MEMORY | MERGE | MESSAGE_TEXT
    | MICROSECOND | MIGRATE | MINUTE | MIN_ROWS | MODE | MODIFY | MONTH
    | MULTILINESTRING | MULTIPOINT | MULTIPOLYGON | MUTEX | MYSQL_ERRNO | NAME
    | NAMES | NATIONAL | NCHAR | NDB | NDBCLUSTER | NESTED | NETWORK_NAMESPACE
    | NEVER | NEW | NEXT | NO | NODEGROUP | NONE | NOWAIT | NO_WAIT | NUMBER
    | NVARCHAR | OFF | OFFSET | OJ | OLD | ONE | ONLY | OPEN | OPTIONAL
    | OPTIMIZER_COSTS | OPTIONS | ORDINALITY | ORGANIZATION | OTHERS | OWNER
    | PACK_KEYS | PAGE | PARALLEL | PARSER | PARSE_GCOL_EXPR | PARTIAL | PARTITIONING
    | PARTITIONS | PASSWORD | PASSWORD_LOCK_TIME | PATH | PERSIST | PERSIST_ONLY
    | PHASE | PLUGIN | PLUGINS | PLUGIN_DIR | POINT | POLYGON | PORT | PRECEDES
    | PRECEDING | PREPARE | PRESERVE | PREV | PRIVILEGE_CHECKS_USER | PRIVILEGES
    | PROCESS | PROCESSLIST | PROFILE | PROFILES | PROXY | QUARTER | QUERY | QUICK
    | RANDOM | RAPID | READ_ONLY | REBUILD | RECOVER | REDO_BUFFER_SIZE | REDUNDANT
    | REFERENCE | REGISTRATION | RELAY | RELAYLOG | RELAY_LOG_FILE | RELAY_LOG_POS
    | RELAY_THREAD | RELOAD | REMOTE | REMOVE | REORGANIZE | REPAIR | REPEATABLE
    | REPLICA | REPLICAS | REPLICATE_DO_DB | REPLICATE_DO_TABLE | REPLICATE_IGNORE_DB
    | REPLICATE_IGNORE_TABLE | REPLICATE_REWRITE_DB | REPLICATE_WILD_DO_TABLE
    | REPLICATE_WILD_IGNORE_TABLE | REPLICATION | REQUIRE_ROW_FORMAT
    | REQUIRE_TABLE_PRIMARY_KEY_CHECK | RESET | RESOURCE | RESPECT | RESTART
    | RESTORE | RESUME | RETAIN | RETURNED_SQLSTATE | RETURNING | RETURNS | REUSE
    | REVERSE | ROLE | ROLLBACK | ROLLUP | ROTATE | ROUTINE | ROW_COUNT | ROW_FORMAT
    | RTREE | SAVEPOINT | SCHEDULE | SCHEMA_NAME | SECONDARY | SECONDARY_ENGINE
    | SECONDARY_ENGINE_ATTRIBUTE | SECONDARY_LOAD | SECONDARY_UNLOAD | SECOND
    | SECURITY | SERIAL | SERIALIZABLE | SERVER | SESSION | SHARE | SHUTDOWN | SIGNAL
    | SIGNED | SIMPLE | SKIP | SLAVE | SLOW | SNAPSHOT | SOCKET | SOME | SONAME
    | SOUNDS | SOURCE | SPATIAL | SQL | SQL_AFTER_GTIDS | SQL_AFTER_MTS_GAPS
    | SQL_BEFORE_GTIDS | SQL_BUFFER_RESULT | SQL_CACHE | SQL_NO_CACHE | SQL_THREAD
    | SQL_TSI_DAY | SQL_TSI_HOUR | SQL_TSI_MINUTE | SQL_TSI_MONTH | SQL_TSI_QUARTER
    | SQL_TSI_SECOND | SQL_TSI_WEEK | SQL_TSI_YEAR | SRID | STACKED | START
    | STARTS | STATS_AUTO_RECALC | STATS_PERSISTENT | STATS_SAMPLE_PAGES | STATUS
    | STOP | STORAGE | STORED | STRING | SUBCLASS_ORIGIN | SUBJECT | SUBPARTITION
    | SUBPARTITIONS | SUPER | SUSPEND | SWAPS | SWITCHES | SYSTEM | TABLE_CHECKSUM
    | TABLE_NAME | TABLESPACE | TEMPORARY | TEMPTABLE | TEXT | THAN | THREAD_PRIORITY
    | TIES | TIME | TIMESTAMP | TIMESTAMPADD | TIMESTAMPDIFF | TLS | TRANSACTION
    | TRIGGERS | TRUNCATE | TYPE | TYPES | UNBOUNDED | UNCOMMITTED | UNDEFINED
    | UNDO | UNDOFILE | UNDO_BUFFER_SIZE | UNICODE | UNINSTALL | UNKNOWN | UNTIL
    | UPGRADE | URL | URI | USER | USER_RESOURCES | USE_FRM | VALIDATION | VALUE
    | VARBINARY | VARIABLES | VCPU | VECTOR | VIEW | VIRTUAL | VISIBLE | WAIT
    | WARNINGS | WEEK | WEIGHT_STRING | WITHOUT | WORK | WRAPPER | X509 | XA | XID
    | XML | YEAR | ZEROFILL | ZONE
    // MySQL AI / HeatWave ML
    | ML_TRAIN | ML_PREDICT_ROW | ML_PREDICT_TABLE | ML_EXPLAIN_ROW | ML_EXPLAIN_TABLE
    | ML_SCORE | ML_MODEL_LOAD | ML_MODEL_UNLOAD | ML_MODEL_IMPORT | ML_MODEL_EXPORT
    | ML_GENERATE | ML_GENERATE_TABLE | ML_EMBED | ML_EMBED_TABLE | ML_NL_SQL
    // Vector functions
    | STRING_TO_VECTOR | TO_VECTOR | VECTOR_TO_STRING | FROM_VECTOR | VECTOR_DIM
    ;
