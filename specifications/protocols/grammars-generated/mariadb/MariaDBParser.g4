/*
 * MariaDB 12.2 Parser Grammar for ANTLR4
 * 
 * This grammar is designed for MariaDB 12.2 and includes all SQL statements,
 * expressions, and language constructs specific to the MariaDB SQL dialect.
 * 
 * Based on official MariaDB 12.2 documentation and syntax specifications.
 * 
 * License: MIT
 */

parser grammar MariaDBParser;

options { tokenVocab=MariaDBLexer; }

// ===========================================================================
// ROOT RULE
// ===========================================================================

root
    : sqlStatements? EOF
    ;

sqlStatements
    : (sqlStatement SEMICOLON? | emptyStatement_)*
      (sqlStatement SEMICOLON?)?
    ;

sqlStatement
    : ddlStatement
    | dmlStatement
    | transactionStatement
    | replicationStatement
    | preparedStatement
    | compoundStatement
    | administrationStatement
    | utilityStatement
    ;

emptyStatement_
    : SEMICOLON
    ;

// ===========================================================================
// DDL STATEMENTS (Data Definition Language)
// ===========================================================================

ddlStatement
    : createDatabase
    | createEvent
    | createIndex
    | createLogfileGroup
    | createProcedure
    | createFunction
    | createServer
    | createTable
    | createTablespaceInnodb
    | createTablespaceNdb
    | createTrigger
    | createView
    | createRole
    | createSequence
    | createPackage
    | createPackageBody
    | alterDatabase
    | alterEvent
    | alterFunction
    | alterInstance
    | alterLogfileGroup
    | alterProcedure
    | alterServer
    | alterTable
    | alterTablespace
    | alterView
    | alterSequence
    | dropDatabase
    | dropEvent
    | dropIndex
    | dropLogfileGroup
    | dropProcedure
    | dropFunction
    | dropServer
    | dropTable
    | dropTablespace
    | dropTrigger
    | dropView
    | dropRole
    | dropSequence
    | dropPackage
    | renameTable
    | truncateTable
    ;

// ---------------------------------------------------------------------------
// DATABASE
// ---------------------------------------------------------------------------

createDatabase
    : CREATE (DATABASE | SCHEMA) ifNotExists? uid createDatabaseOption*
    ;

createDatabaseOption
    : DEFAULT? (CHARACTER SET | CHARSET) EQUAL_SYMBOL? (charsetName | DEFAULT)
    | DEFAULT? COLLATE EQUAL_SYMBOL? (collationName | DEFAULT)
    | DEFAULT? ENCRYPTION EQUAL_SYMBOL? STRING_LITERAL
    | COMMENT EQUAL_SYMBOL? STRING_LITERAL
    ;

alterDatabase
    : ALTER (DATABASE | SCHEMA) uid? alterDatabaseOption+
    ;

alterDatabaseOption
    : createDatabaseOption
    | READ ONLY EQUAL_SYMBOL? (DEFAULT | ZERO_DECIMAL | ONE_DECIMAL)
    | UPGRADE DATA DIRECTORY NAME
    ;

dropDatabase
    : DROP (DATABASE | SCHEMA) ifExists? uid
    ;

// ---------------------------------------------------------------------------
// EVENT
// ---------------------------------------------------------------------------

createEvent
    : CREATE ownerStatement? EVENT ifNotExists? fullId
      ON SCHEDULE scheduleExpression
      (ON COMPLETION NOT? PRESERVE)?
      enableType?
      (COMMENT STRING_LITERAL)?
      DO routineBody
    ;

alterEvent
    : ALTER ownerStatement? EVENT fullId
      (ON SCHEDULE scheduleExpression)?
      (ON COMPLETION NOT? PRESERVE)?
      (RENAME TO fullId)?
      enableType?
      (COMMENT STRING_LITERAL)?
      (DO routineBody)?
    ;

dropEvent
    : DROP EVENT ifExists? fullId
    ;

scheduleExpression
    : AT timestampValue (PLUS INTERVAL intervalExpr)*
    | EVERY intervalExpr
      (STARTS timestampValue (PLUS INTERVAL intervalExpr)*)?
      (ENDS timestampValue (PLUS INTERVAL intervalExpr)*)?
    ;

timestampValue
    : CURRENT_TIMESTAMP
    | stringLiteral
    | decimalLiteral
    | expression
    ;

intervalExpr
    : expression intervalType
    ;

intervalType
    : MICROSECOND | SECOND | MINUTE | HOUR | DAY | WEEK | MONTH
    | QUARTER | YEAR | SECOND_MICROSECOND | MINUTE_MICROSECOND
    | MINUTE_SECOND | HOUR_MICROSECOND | HOUR_SECOND | HOUR_MINUTE
    | DAY_MICROSECOND | DAY_SECOND | DAY_MINUTE | DAY_HOUR
    | YEAR_MONTH
    ;

enableType
    : ENABLE
    | DISABLE
    | DISABLE ON SLAVE
    ;

// ---------------------------------------------------------------------------
// INDEX
// ---------------------------------------------------------------------------

createIndex
    : CREATE orReplace?
      indexCategory=(ONLINE | OFFLINE)?
      intimeAction=(UNIQUE | FULLTEXT | SPATIAL)?
      INDEX ifNotExists? uid indexType?
      ON tableName indexColumnNames
      indexOption*
      (
        ALGORITHM EQUAL_SYMBOL? algType=(DEFAULT | INPLACE | COPY | NOCOPY | INSTANT)
        | LOCK EQUAL_SYMBOL? lockType=(DEFAULT | NONE | SHARED | EXCLUSIVE)
      )*
      (WAIT decimalLiteral | NOWAIT)?
    ;

dropIndex
    : DROP INDEX ifExists? uid ON tableName
      (
        ALGORITHM EQUAL_SYMBOL? algType=(DEFAULT | INPLACE | COPY | NOCOPY | INSTANT)
        | LOCK EQUAL_SYMBOL? lockType=(DEFAULT | NONE | SHARED | EXCLUSIVE)
      )*
      (WAIT decimalLiteral | NOWAIT)?
    ;

indexColumnNames
    : LPAREN indexColumnName (COMMA indexColumnName)* RPAREN
    ;

indexColumnName
    : (uid | expression) (LPAREN decimalLiteral RPAREN)? sortType=(ASC | DESC)?
    ;

indexType
    : USING (BTREE | HASH | RTREE)
    ;

indexOption
    : KEY_BLOCK_SIZE EQUAL_SYMBOL? fileSizeLiteral
    | indexType
    | WITH PARSER uid
    | COMMENT STRING_LITERAL
    | (VISIBLE | INVISIBLE)
    | ENGINE_ATTRIBUTE EQUAL_SYMBOL? STRING_LITERAL
    | SECONDARY_ENGINE_ATTRIBUTE EQUAL_SYMBOL? STRING_LITERAL
    | CLUSTERING EQUAL_SYMBOL? (YES | NO)
    | IGNORED
    | NOT IGNORED
    ;

// ---------------------------------------------------------------------------
// LOGFILE GROUP
// ---------------------------------------------------------------------------

createLogfileGroup
    : CREATE LOGFILE GROUP uid
      ADD UNDOFILE STRING_LITERAL
      (INITIAL_SIZE EQUAL_SYMBOL? fileSizeLiteral)?
      (UNDO_BUFFER_SIZE EQUAL_SYMBOL? fileSizeLiteral)?
      (REDO_BUFFER_SIZE EQUAL_SYMBOL? fileSizeLiteral)?
      (NODEGROUP EQUAL_SYMBOL? uid)?
      WAIT?
      (COMMENT EQUAL_SYMBOL? STRING_LITERAL)?
      ENGINE EQUAL_SYMBOL? engineName
    ;

alterLogfileGroup
    : ALTER LOGFILE GROUP uid
      ADD UNDOFILE STRING_LITERAL
      (INITIAL_SIZE EQUAL_SYMBOL? fileSizeLiteral)?
      WAIT?
      ENGINE EQUAL_SYMBOL? engineName
    ;

dropLogfileGroup
    : DROP LOGFILE GROUP uid ENGINE EQUAL_SYMBOL? engineName
    ;

// ---------------------------------------------------------------------------
// PROCEDURE / FUNCTION
// ---------------------------------------------------------------------------

createProcedure
    : CREATE orReplace? ownerStatement?
      (AGGREGATE)? PROCEDURE ifNotExists? fullId
      LPAREN procedureParameter? (COMMA procedureParameter)* RPAREN
      routineOption*
      routineBody
    ;

createFunction
    : CREATE orReplace? ownerStatement?
      (AGGREGATE)? FUNCTION ifNotExists? fullId
      LPAREN functionParameter? (COMMA functionParameter)* RPAREN
      RETURNS dataType
      routineOption*
      (returnStatement | routineBody)
    ;

procedureParameter
    : direction=(IN | OUT | INOUT)? uid dataType
    ;

functionParameter
    : uid dataType
    ;

routineOption
    : COMMENT STRING_LITERAL
    | LANGUAGE SQL
    | NOT? DETERMINISTIC
    | (CONTAINS SQL | NO SQL | READS SQL DATA | MODIFIES SQL DATA)
    | SQL SECURITY securityContext=(DEFINER | INVOKER)
    ;

routineBody
    : blockStatement
    | sqlStatement
    ;

alterProcedure
    : ALTER PROCEDURE fullId routineOption*
    ;

alterFunction
    : ALTER FUNCTION fullId routineOption*
    ;

dropProcedure
    : DROP PROCEDURE ifExists? fullId
    ;

dropFunction
    : DROP FUNCTION ifExists? fullId
    ;

// ---------------------------------------------------------------------------
// SERVER
// ---------------------------------------------------------------------------

createServer
    : CREATE SERVER uid
      FOREIGN DATA WRAPPER wrapperName=(MYSQL | STRING_LITERAL)
      OPTIONS LPAREN serverOption (COMMA serverOption)* RPAREN
    ;

alterServer
    : ALTER SERVER uid OPTIONS
      LPAREN serverOption (COMMA serverOption)* RPAREN
    ;

dropServer
    : DROP SERVER ifExists? uid
    ;

serverOption
    : HOST STRING_LITERAL
    | DATABASE STRING_LITERAL
    | USER STRING_LITERAL
    | PASSWORD STRING_LITERAL
    | SOCKET STRING_LITERAL
    | OWNER STRING_LITERAL
    | PORT decimalLiteral
    ;

// ---------------------------------------------------------------------------
// TABLE
// ---------------------------------------------------------------------------

createTable
    : CREATE orReplace? TEMPORARY? TABLE ifNotExists? tableName
      (
        LIKE tableName
        | LPAREN LIKE tableName RPAREN
      )                                                           # copyCreateTable
    | CREATE orReplace? TEMPORARY? TABLE ifNotExists? tableName
      createDefinitions?
      (tableOption (COMMA? tableOption)*)?
      partitionDefinitions?
      (IGNORE | REPLACE)?
      AS? selectStatement                                         # queryCreateTable
    | CREATE orReplace? TEMPORARY? TABLE ifNotExists? tableName
      createDefinitions
      (tableOption (COMMA? tableOption)*)?
      partitionDefinitions?                                       # columnCreateTable
    ;

createDefinitions
    : LPAREN createDefinition (COMMA createDefinition)* RPAREN
    ;

createDefinition
    : uid columnDefinition                                        # columnDeclaration
    | tableConstraint                                             # constraintDeclaration
    | indexColumnDefinition                                       # indexDeclaration
    ;

columnDefinition
    : dataType columnConstraint*
    ;

columnConstraint
    : nullNotnull                                                 # nullColumnConstraint
    | DEFAULT defaultValue                                        # defaultColumnConstraint
    | (AUTO_INCREMENT | AUTOINCREMENT)                           # autoIncrementColumnConstraint
    | PRIMARY? KEY                                                # primaryKeyColumnConstraint
    | UNIQUE KEY?                                                 # uniqueKeyColumnConstraint
    | COMMENT STRING_LITERAL                                      # commentColumnConstraint
    | COLUMN_FORMAT colFormat=(FIXED | DYNAMIC | DEFAULT)        # formatColumnConstraint
    | STORAGE storageval=(DISK | MEMORY | DEFAULT)               # storageColumnConstraint
    | referenceDefinition                                         # referenceColumnConstraint
    | COLLATE collationName                                       # collateColumnConstraint
    | (GENERATED ALWAYS)? AS LPAREN expression RPAREN
      (VIRTUAL | STORED | PERSISTENT)?                           # generatedColumnConstraint
    | SERIAL DEFAULT VALUE                                        # serialDefaultColumnConstraint
    | (CONSTRAINT name=uid?)? 
      CHECK LPAREN expression RPAREN (ENFORCED | NOT ENFORCED)?   # checkColumnConstraint
    | (VISIBLE | INVISIBLE)                                       # visibilityColumnConstraint
    ;

tableConstraint
    : (CONSTRAINT name=uid?)?
      PRIMARY KEY index=uid? indexType?
      indexColumnNames indexOption*                               # primaryKeyTableConstraint
    | (CONSTRAINT name=uid?)?
      UNIQUE (INDEX | KEY)? index=uid? indexType?
      indexColumnNames indexOption*                               # uniqueKeyTableConstraint
    | (CONSTRAINT name=uid?)?
      FOREIGN KEY index=uid? indexColumnNames
      referenceDefinition                                         # foreignKeyTableConstraint
    | (CONSTRAINT name=uid?)?
      CHECK LPAREN expression RPAREN (ENFORCED | NOT ENFORCED)?   # checkTableConstraint
    ;

indexColumnDefinition
    : (INDEX | KEY) uid? indexType?
      indexColumnNames indexOption*                               # simpleIndexDeclaration
    | (FULLTEXT | SPATIAL)
      (INDEX | KEY)? uid?
      indexColumnNames indexOption*                               # specialIndexDeclaration
    ;

referenceDefinition
    : REFERENCES tableName indexColumnNames?
      (MATCH matchType=(FULL | PARTIAL | SIMPLE))?
      referenceAction?
    ;

referenceAction
    : ON DELETE onDelete=referenceControlType
      (ON UPDATE onUpdate=referenceControlType)?
    | ON UPDATE onUpdate=referenceControlType
      (ON DELETE onDelete=referenceControlType)?
    ;

referenceControlType
    : RESTRICT | CASCADE | SET NULL_LITERAL | NO ACTION | SET DEFAULT
    ;

tableOption
    : ENGINE EQUAL_SYMBOL? engineName                             # tableOptionEngine
    | AUTO_INCREMENT EQUAL_SYMBOL? decimalLiteral                 # tableOptionAutoIncrement
    | AVG_ROW_LENGTH EQUAL_SYMBOL? decimalLiteral                 # tableOptionAverage
    | DEFAULT? (CHARACTER SET | CHARSET) EQUAL_SYMBOL? 
      (charsetName | DEFAULT)                                     # tableOptionCharset
    | (CHECKSUM | TABLE_CHECKSUM) EQUAL_SYMBOL? boolLiteral       # tableOptionChecksum
    | DEFAULT? COLLATE EQUAL_SYMBOL? (collationName | DEFAULT)    # tableOptionCollate
    | COMMENT EQUAL_SYMBOL? STRING_LITERAL                        # tableOptionComment
    | COMPRESSION EQUAL_SYMBOL? (STRING_LITERAL | ID)             # tableOptionCompression
    | CONNECTION EQUAL_SYMBOL? STRING_LITERAL                     # tableOptionConnection
    | DATA DIRECTORY EQUAL_SYMBOL? STRING_LITERAL                 # tableOptionDataDirectory
    | DELAY_KEY_WRITE EQUAL_SYMBOL? boolLiteral                   # tableOptionDelay
    | ENCRYPTION EQUAL_SYMBOL? STRING_LITERAL                     # tableOptionEncryption
    | encryptedLiteral EQUAL_SYMBOL? (YES | NO)                   # tableOptionEncrypted
    | PAGE_COMPRESSED EQUAL_SYMBOL? (YES | NO | boolLiteral)      # tableOptionPageCompressed
    | PAGE_COMPRESSION_LEVEL EQUAL_SYMBOL? decimalLiteral         # tableOptionPageCompressionLevel
    | ENCRYPTION_KEY_ID EQUAL_SYMBOL? decimalLiteral              # tableOptionEncryptionKeyId
    | INDEX DIRECTORY EQUAL_SYMBOL? STRING_LITERAL                # tableOptionIndexDirectory
    | INSERT_METHOD EQUAL_SYMBOL? insertMethod=(NO | FIRST | LAST) # tableOptionInsertMethod
    | KEY_BLOCK_SIZE EQUAL_SYMBOL? fileSizeLiteral                # tableOptionKeyBlockSize
    | MAX_ROWS EQUAL_SYMBOL? decimalLiteral                       # tableOptionMaxRows
    | MIN_ROWS EQUAL_SYMBOL? decimalLiteral                       # tableOptionMinRows
    | PACK_KEYS EQUAL_SYMBOL? extBoolValue=(ZERO_DECIMAL | ONE_DECIMAL | DEFAULT)  # tableOptionPackKeys
    | PASSWORD EQUAL_SYMBOL? STRING_LITERAL                       # tableOptionPassword
    | ROW_FORMAT EQUAL_SYMBOL? rowFormat=(
        DEFAULT | DYNAMIC | FIXED | COMPRESSED
        | REDUNDANT | COMPACT | PAGE)                             # tableOptionRowFormat
    | START TRANSACTION                                           # tableOptionStartTransaction
    | SECONDARY_ENGINE_ATTRIBUTE EQUAL_SYMBOL? STRING_LITERAL     # tableOptionSecondaryEngineAttribute
    | STATS_AUTO_RECALC EQUAL_SYMBOL? extBoolValue=(DEFAULT | ZERO_DECIMAL | ONE_DECIMAL) # tableOptionRecalculation
    | STATS_PERSISTENT EQUAL_SYMBOL? extBoolValue=(DEFAULT | ZERO_DECIMAL | ONE_DECIMAL)  # tableOptionPersistent
    | STATS_SAMPLE_PAGES EQUAL_SYMBOL? (DEFAULT | decimalLiteral) # tableOptionSamplePage
    | TABLESPACE uid tablespaceStorage?                           # tableOptionTablespace
    | TABLE_TYPE EQUAL_SYMBOL? tableType                          # tableOptionTableType
    | TRANSACTIONAL EQUAL_SYMBOL? (ZERO_DECIMAL | ONE_DECIMAL)    # tableOptionTransactional
    | UNION EQUAL_SYMBOL? LPAREN tables RPAREN                    # tableOptionUnion
    | SEQUENCE EQUAL_SYMBOL? (ZERO_DECIMAL | ONE_DECIMAL)         # tableOptionSequence
    | WITH SYSTEM VERSIONING                                      # tableOptionWithSystemVersioning
    ;

tableType
    : MYSQL | ODBC
    ;

tablespaceStorage
    : STORAGE (DISK | MEMORY | DEFAULT)
    ;

partitionDefinitions
    : PARTITION BY partitionFunctionDefinition
      (PARTITIONS count=decimalLiteral)?
      (SUBPARTITION BY subpartitionFunctionDefinition
        (SUBPARTITIONS subCount=decimalLiteral)?
      )?
      (LPAREN partitionDefinition (COMMA partitionDefinition)* RPAREN)?
    ;

partitionFunctionDefinition
    : LINEAR? HASH LPAREN expression RPAREN                       # partitionFunctionHash
    | LINEAR? KEY (ALGORITHM EQUAL_SYMBOL algType=(ONE_DECIMAL | TWO_DECIMAL))?
      LPAREN uidList? RPAREN                                      # partitionFunctionKey
    | RANGE (LPAREN expression RPAREN | COLUMNS LPAREN uidList RPAREN) # partitionFunctionRange
    | LIST (LPAREN expression RPAREN | COLUMNS LPAREN uidList RPAREN)  # partitionFunctionList
    | SYSTEM_TIME (
        INTERVAL intervalExpr
        | LIMIT decimalLiteral
      )?                                                          # partitionFunctionSystemTime
    ;

subpartitionFunctionDefinition
    : LINEAR? HASH LPAREN expression RPAREN
    | LINEAR? KEY (ALGORITHM EQUAL_SYMBOL algType=(ONE_DECIMAL | TWO_DECIMAL))?
      LPAREN uidList RPAREN
    ;

partitionDefinition
    : PARTITION uid
      (VALUES
        (LESS THAN LPAREN (partitionDefinerAtom (COMMA partitionDefinerAtom)* | MAXVALUE) RPAREN
        | LESS THAN MAXVALUE
        | IN LPAREN partitionDefinerAtom (COMMA partitionDefinerAtom)* RPAREN
        )
      )?
      partitionOption*
      (LPAREN subpartitionDefinition (COMMA subpartitionDefinition)* RPAREN)?
    ;

partitionDefinerAtom
    : constant | expression | MAXVALUE
    ;

partitionOption
    : DEFAULT? STORAGE? ENGINE EQUAL_SYMBOL? engineName
    | COMMENT EQUAL_SYMBOL? STRING_LITERAL
    | DATA DIRECTORY EQUAL_SYMBOL? STRING_LITERAL
    | INDEX DIRECTORY EQUAL_SYMBOL? STRING_LITERAL
    | MAX_ROWS EQUAL_SYMBOL? decimalLiteral
    | MIN_ROWS EQUAL_SYMBOL? decimalLiteral
    | TABLESPACE EQUAL_SYMBOL? uid
    | NODEGROUP EQUAL_SYMBOL? uid
    ;

subpartitionDefinition
    : SUBPARTITION uid partitionOption*
    ;

alterTable
    : ALTER intimeAction? IGNORE? TABLE tableName (WAIT decimalLiteral | NOWAIT)?
      (alterTableOption (COMMA? alterTableOption)*)?
      partitionDefinitions?
    ;

alterTableOption
    : tableOption                                                 # alterByTableOption
    | ADD COLUMN? ifNotExists? uid columnDefinition (FIRST | AFTER uid)? # alterByAddColumn
    | ADD COLUMN? ifNotExists?
      LPAREN uid columnDefinition (COMMA uid columnDefinition)* RPAREN # alterByAddColumns
    | ADD indexColumnDefinition                                   # alterByAddIndex
    | ADD (CONSTRAINT name=uid?)? PRIMARY KEY
      index=uid? indexType? indexColumnNames indexOption*         # alterByAddPrimaryKey
    | ADD (CONSTRAINT name=uid?)? UNIQUE
      (INDEX | KEY)? indexName=uid? indexType?
      indexColumnNames indexOption*                               # alterByAddUniqueKey
    | ADD keyType=(FULLTEXT | SPATIAL)
      (INDEX | KEY)? uid?
      indexColumnNames indexOption*                               # alterByAddSpecialIndex
    | ADD (CONSTRAINT name=uid?)? FOREIGN KEY
      index=uid? indexColumnNames referenceDefinition             # alterByAddForeignKey
    | ADD (CONSTRAINT name=uid?)? CHECK LPAREN expression RPAREN  # alterByAddCheckTableConstraint
    | ALGORITHM EQUAL_SYMBOL? algType=(DEFAULT | INSTANT | INPLACE | COPY | NOCOPY) # alterBySetAlgorithm
    | ALTER COLUMN? uid
      (SET DEFAULT (defaultValue | LPAREN expression RPAREN) | DROP DEFAULT) # alterByChangeDefault
    | CHANGE COLUMN? ifExists? oldColumn=uid newColumn=uid columnDefinition
      (FIRST | AFTER afterColumn=uid)?                            # alterByChangeColumn
    | RENAME COLUMN oldColumn=uid TO newColumn=uid                # alterByRenameColumn
    | LOCK EQUAL_SYMBOL? lockType=(DEFAULT | NONE | SHARED | EXCLUSIVE) # alterByLock
    | MODIFY COLUMN? ifExists? uid columnDefinition (FIRST | AFTER uid)? # alterByModifyColumn
    | DROP COLUMN? ifExists? uid RESTRICT?                        # alterByDropColumn
    | DROP (CONSTRAINT | CHECK) ifExists? uid                     # alterByDropConstraintCheck
    | DROP PRIMARY KEY                                            # alterByDropPrimaryKey
    | DROP indexFormat=(INDEX | KEY) ifExists? uid                # alterByDropIndex
    | RENAME indexFormat=(INDEX | KEY) uid TO uid                 # alterByRenameIndex
    | ALTER INDEX uid (VISIBLE | INVISIBLE)                       # alterByAlterIndexVisibility
    | DROP FOREIGN KEY ifExists? uid                              # alterByDropForeignKey
    | DISABLE KEYS                                                # alterByDisableKeys
    | ENABLE KEYS                                                 # alterByEnableKeys
    | RENAME renameFormat=(TO | AS)? (uid | fullId)               # alterByRename
    | ORDER BY uidList                                            # alterByOrder
    | CONVERT TO CHARACTER SET charsetName (COLLATE collationName)? # alterByConvertCharset
    | DEFAULT? CHARACTER SET EQUAL_SYMBOL? charsetName (COLLATE EQUAL_SYMBOL? collationName)? # alterByDefaultCharset
    | DISCARD TABLESPACE                                          # alterByDiscardTablespace
    | IMPORT TABLESPACE                                           # alterByImportTablespace
    | FORCE                                                       # alterByForce
    | validationFormat=(WITHOUT | WITH) VALIDATION                # alterByValidate
    | ADD PARTITION ifNotExists? LPAREN partitionDefinition (COMMA partitionDefinition)* RPAREN # alterByAddPartition
    | DROP PARTITION ifExists? uidList                            # alterByDropPartition
    | DISCARD PARTITION (uidList | ALL) TABLESPACE                # alterByDiscardPartition
    | IMPORT PARTITION (uidList | ALL) TABLESPACE                 # alterByImportPartition
    | TRUNCATE PARTITION (uidList | ALL)                          # alterByTruncatePartition
    | COALESCE PARTITION decimalLiteral                           # alterByCoalescePartition
    | REORGANIZE PARTITION uidList INTO LPAREN partitionDefinition (COMMA partitionDefinition)* RPAREN # alterByReorganizePartition
    | EXCHANGE PARTITION uid WITH TABLE tableName validationFormat=(WITH | WITHOUT)? VALIDATION? # alterByExchangePartition
    | ANALYZE PARTITION (uidList | ALL)                           # alterByAnalyzePartition
    | CHECK PARTITION (uidList | ALL)                             # alterByCheckPartition
    | OPTIMIZE PARTITION (uidList | ALL)                          # alterByOptimizePartition
    | REBUILD PARTITION (uidList | ALL)                           # alterByRebuildPartition
    | REPAIR PARTITION (uidList | ALL)                            # alterByRepairPartition
    | REMOVE PARTITIONING                                         # alterByRemovePartitioning
    | UPGRADE PARTITIONING                                        # alterByUpgradePartitioning
    | ADD SYSTEM VERSIONING                                       # alterByAddSystemVersioning
    | DROP SYSTEM VERSIONING                                      # alterByDropSystemVersioning
    ;

dropTable
    : DROP TEMPORARY? TABLE ifExists?
      tables RESTRICT?
    ;

renameTable
    : RENAME TABLE renameTableClause (COMMA renameTableClause)*
    ;

renameTableClause
    : tableName TO tableName
    ;

truncateTable
    : TRUNCATE TABLE? tableName (WAIT decimalLiteral | NOWAIT)?
    ;

// ---------------------------------------------------------------------------
// TABLESPACE
// ---------------------------------------------------------------------------

createTablespaceInnodb
    : CREATE TABLESPACE uid
      ADD DATAFILE STRING_LITERAL
      (FILE_BLOCK_SIZE EQUAL_SYMBOL? fileSizeLiteral)?
      (ENGINE EQUAL_SYMBOL? engineName)?
    ;

createTablespaceNdb
    : CREATE TABLESPACE uid
      ADD DATAFILE STRING_LITERAL
      USE LOGFILE GROUP uid
      (EXTENT_SIZE EQUAL_SYMBOL? fileSizeLiteral)?
      (INITIAL_SIZE EQUAL_SYMBOL? fileSizeLiteral)?
      (AUTOEXTEND_SIZE EQUAL_SYMBOL? fileSizeLiteral)?
      (MAX_SIZE EQUAL_SYMBOL? fileSizeLiteral)?
      (NODEGROUP EQUAL_SYMBOL? uid)?
      WAIT?
      (COMMENT EQUAL_SYMBOL? STRING_LITERAL)?
      ENGINE EQUAL_SYMBOL? engineName
    ;

alterTablespace
    : ALTER TABLESPACE uid
      objectAction=(ADD | DROP) DATAFILE STRING_LITERAL
      (INITIAL_SIZE EQUAL_SYMBOL? fileSizeLiteral)?
      WAIT?
      ENGINE EQUAL_SYMBOL? engineName
    ;

dropTablespace
    : DROP TABLESPACE uid (ENGINE EQUAL_SYMBOL? engineName)?
    ;

// ---------------------------------------------------------------------------
// TRIGGER
// ---------------------------------------------------------------------------

createTrigger
    : CREATE orReplace? ownerStatement?
      TRIGGER ifNotExists? fullId
      triggerTime=(BEFORE | AFTER)
      triggerEvent=(INSERT | UPDATE | DELETE)
      ON tableName FOR EACH ROW
      ((FOLLOWS | PRECEDES) fullId)?
      routineBody
    ;

dropTrigger
    : DROP TRIGGER ifExists? fullId
    ;

// ---------------------------------------------------------------------------
// VIEW
// ---------------------------------------------------------------------------

createView
    : CREATE orReplace?
      (ALGORITHM EQUAL_SYMBOL algType=(UNDEFINED | MERGE | TEMPTABLE))?
      ownerStatement?
      (SQL SECURITY secContext=(DEFINER | INVOKER))?
      VIEW fullId (LPAREN uidList RPAREN)?
      AS selectStatement
      (WITH checkOption=(CASCADED | LOCAL)? CHECK OPTION)?
    ;

alterView
    : ALTER
      (ALGORITHM EQUAL_SYMBOL algType=(UNDEFINED | MERGE | TEMPTABLE))?
      ownerStatement?
      (SQL SECURITY secContext=(DEFINER | INVOKER))?
      VIEW fullId (LPAREN uidList RPAREN)?
      AS selectStatement
      (WITH checkOption=(CASCADED | LOCAL)? CHECK OPTION)?
    ;

dropView
    : DROP VIEW ifExists? fullId (COMMA fullId)* (RESTRICT | CASCADE)?
    ;

// ---------------------------------------------------------------------------
// ROLE
// ---------------------------------------------------------------------------

createRole
    : CREATE ROLE ifNotExists? roleName (COMMA roleName)*
      (WITH ADMIN (CURRENT_USER | CURRENT_ROLE | userOrRoleName))?
    ;

dropRole
    : DROP ROLE ifExists? roleName (COMMA roleName)*
    ;

// ---------------------------------------------------------------------------
// SEQUENCE
// ---------------------------------------------------------------------------

createSequence
    : CREATE orReplace? TEMPORARY? SEQUENCE ifNotExists? fullId
      sequenceOption*
    ;

sequenceOption
    : START WITH? EQUAL_SYMBOL? decimalLiteral
    | INCREMENT BY? EQUAL_SYMBOL? decimalLiteral
    | MINVALUE EQUAL_SYMBOL? decimalLiteral
    | NO MINVALUE
    | NOMINVALUE
    | MAXVALUE EQUAL_SYMBOL? decimalLiteral
    | NO MAXVALUE
    | NOMAXVALUE
    | CYCLE
    | NO CYCLE
    | NOCYCLE
    | CACHE EQUAL_SYMBOL? decimalLiteral
    | NO CACHE
    | NOCACHE
    | RESTART (WITH? EQUAL_SYMBOL? decimalLiteral)?
    ;

alterSequence
    : ALTER SEQUENCE ifExists? fullId sequenceOption*
    ;

dropSequence
    : DROP SEQUENCE ifExists? fullId (COMMA fullId)*
    ;

// ---------------------------------------------------------------------------
// PACKAGE (Oracle Mode)
// ---------------------------------------------------------------------------

createPackage
    : CREATE orReplace? PACKAGE ifNotExists? fullId
      (COMMENT STRING_LITERAL)?
      AS
      packageDeclaration*
      END uid?
    ;

createPackageBody
    : CREATE orReplace? PACKAGE BODY ifNotExists? fullId
      (COMMENT STRING_LITERAL)?
      AS
      packageDeclaration*
      packageRoutineDefinition*
      END uid?
    ;

packageDeclaration
    : variableDeclaration
    | cursorDeclaration
    | procedureDeclaration
    | functionDeclaration
    ;

variableDeclaration
    : uid dataType ((NOT NULL_LITERAL)? (DEFAULT | COLON_EQ) expression)? SEMICOLON
    ;

cursorDeclaration
    : CURSOR uid (LPAREN cursorParameter (COMMA cursorParameter)* RPAREN)?
      IS selectStatement SEMICOLON
    ;

cursorParameter
    : uid dataType
    ;

procedureDeclaration
    : PROCEDURE uid LPAREN (procedureParameter (COMMA procedureParameter)*)? RPAREN SEMICOLON
    ;

functionDeclaration
    : FUNCTION uid LPAREN (functionParameter (COMMA functionParameter)*)? RPAREN
      RETURN dataType SEMICOLON
    ;

packageRoutineDefinition
    : createProcedure SEMICOLON
    | createFunction SEMICOLON
    ;

dropPackage
    : DROP PACKAGE BODY? ifExists? fullId
    ;

// ===========================================================================
// DML STATEMENTS (Data Manipulation Language)
// ===========================================================================

dmlStatement
    : selectStatement
    | insertStatement
    | updateStatement
    | deleteStatement
    | replaceStatement
    | callStatement
    | loadDataStatement
    | loadXmlStatement
    | doStatement
    | handlerStatement
    | valuesStatement
    ;

// ---------------------------------------------------------------------------
// SELECT
// ---------------------------------------------------------------------------

selectStatement
    : queryExpression lockClause?
    | queryExpressionParens lockClause?
    | selectStatementWithInto
    ;

selectStatementWithInto
    : LPAREN selectStatementWithInto RPAREN
    | queryExpression intoClause lockClause?
    | queryExpression lockClause intoClause
    ;

queryExpression
    : withClause? queryExpressionBody orderByClause? limitClause?
    ;

queryExpressionBody
    : queryPrimary                                                # queryExpressionBodyPrimary
    | queryExpressionBody UNION allOrDistinct? queryPrimary       # queryExpressionBodyUnion
    | queryExpressionBody EXCEPT allOrDistinct? queryPrimary      # queryExpressionBodyExcept
    | queryExpressionBody INTERSECT allOrDistinct? queryPrimary   # queryExpressionBodyIntersect
    | queryExpressionBody MINUS allOrDistinct? queryPrimary       # queryExpressionBodyMinus
    ;

queryPrimary
    : querySpecification
    | queryExpressionParens
    | tableValueConstructor
    ;

queryExpressionParens
    : LPAREN queryExpressionParens RPAREN
    | LPAREN queryExpression lockClause? RPAREN
    ;

querySpecification
    : SELECT selectSpec* selectElements intoClause? fromClause? groupByClause?
      havingClause? windowClause? qualifyClause?
    ;

selectSpec
    : ALL
    | DISTINCT
    | DISTINCTROW
    | HIGH_PRIORITY
    | STRAIGHT_JOIN
    | SQL_SMALL_RESULT
    | SQL_BIG_RESULT
    | SQL_BUFFER_RESULT
    | SQL_CACHE
    | SQL_NO_CACHE
    | SQL_CALC_FOUND_ROWS
    ;

selectElements
    : (STAR | selectElement) (COMMA selectElement)*
    ;

selectElement
    : fullId DOT STAR                                             # selectStarElement
    | fullColumnName (AS? uid)?                                   # selectColumnElement
    | expression (AS? uid)?                                       # selectExpressionElement
    ;

intoClause
    : INTO assignmentField (COMMA assignmentField)*
    | INTO DUMPFILE STRING_LITERAL
    | INTO OUTFILE STRING_LITERAL
      (CHARACTER SET charsetName)?
      (fieldsFormat=(FIELDS | COLUMNS)
        selectFieldsInto+
      )?
      (LINES selectLinesInto+)?
    ;

selectFieldsInto
    : TERMINATED BY STRING_LITERAL
    | OPTIONALLY? ENCLOSED BY STRING_LITERAL
    | ESCAPED BY STRING_LITERAL
    ;

selectLinesInto
    : STARTING BY STRING_LITERAL
    | TERMINATED BY STRING_LITERAL
    ;

fromClause
    : FROM tableSources whereClause?
    ;

whereClause
    : WHERE expression
    ;

groupByClause
    : GROUP BY groupByItem (COMMA groupByItem)* (WITH ROLLUP)?
    ;

havingClause
    : HAVING expression
    ;

windowClause
    : WINDOW windowDefinition (COMMA windowDefinition)*
    ;

windowDefinition
    : uid AS LPAREN windowSpec RPAREN
    ;

qualifyClause
    : QUALIFY expression
    ;

groupByItem
    : expression sortOrder=(ASC | DESC)?
    ;

limitClause
    : LIMIT ((offset=limitClauseAtom COMMA)? limit=limitClauseAtom
            | limit=limitClauseAtom OFFSET offset=limitClauseAtom)
    ;

limitClauseAtom
    : decimalLiteral | mysqlVariable | simpleId
    ;

lockClause
    : FOR UPDATE (OF fullId (COMMA fullId)*)? lockWaitOption?
    | FOR SHARE (OF fullId (COMMA fullId)*)? lockWaitOption?
    | LOCK IN SHARE MODE
    ;

lockWaitOption
    : NOWAIT
    | WAIT decimalLiteral
    | SKIP_ LOCKED
    ;

withClause
    : WITH RECURSIVE? commonTableExpression (COMMA commonTableExpression)*
    ;

commonTableExpression
    : cteName=uid (LPAREN cteColumnList=uidList RPAREN)?
      AS LPAREN queryExpression RPAREN
    ;

tableValueConstructor
    : VALUES rowConstructor (COMMA rowConstructor)*
    ;

rowConstructor
    : ROW? LPAREN expressionOrDefault (COMMA expressionOrDefault)* RPAREN
    ;

// ---------------------------------------------------------------------------
// INSERT
// ---------------------------------------------------------------------------

insertStatement
    : INSERT (LOW_PRIORITY | DELAYED | HIGH_PRIORITY)?
      IGNORE? INTO? tableName
      (PARTITION LPAREN uidList RPAREN)?
      (
        (LPAREN uidList? RPAREN)? insertStatementValue
        | SET updatedElement (COMMA updatedElement)*
      )
      (AS uid)?
      (ON DUPLICATE KEY UPDATE updatedElement (COMMA updatedElement)*)?
      (RETURNING (STAR | selectElement (COMMA selectElement)*))?
    ;

insertStatementValue
    : selectStatement
    | (VALUES | VALUE) valuesOrValueList
    | TABLE tableName
    ;

valuesOrValueList
    : LPAREN expressionOrDefault (COMMA expressionOrDefault)* RPAREN
      (COMMA LPAREN expressionOrDefault (COMMA expressionOrDefault)* RPAREN)*
    ;

expressionOrDefault
    : expression | DEFAULT
    ;

updatedElement
    : fullColumnName EQUAL_SYMBOL (expression | DEFAULT)
    ;

assignmentField
    : uid | LOCAL_ID
    ;

// ---------------------------------------------------------------------------
// UPDATE
// ---------------------------------------------------------------------------

updateStatement
    : singleUpdateStatement
    | multipleUpdateStatement
    ;

singleUpdateStatement
    : UPDATE LOW_PRIORITY? IGNORE? tableName (AS? uid)?
      SET updatedElement (COMMA updatedElement)*
      whereClause? orderByClause? limitClause?
    ;

multipleUpdateStatement
    : UPDATE LOW_PRIORITY? IGNORE? tableSources
      SET updatedElement (COMMA updatedElement)*
      whereClause?
    ;

// ---------------------------------------------------------------------------
// DELETE
// ---------------------------------------------------------------------------

deleteStatement
    : singleDeleteStatement
    | multipleDeleteStatement
    ;

singleDeleteStatement
    : DELETE LOW_PRIORITY? QUICK? IGNORE?
      FROM tableName (PARTITION LPAREN uidList RPAREN)?
      whereClause? orderByClause? limitClause?
      (RETURNING (STAR | selectElement (COMMA selectElement)*))?
    ;

multipleDeleteStatement
    : DELETE LOW_PRIORITY? QUICK? IGNORE?
      (
        tableName (DOT STAR)? (COMMA tableName (DOT STAR)?)*
        FROM tableSources
      | FROM tableName (DOT STAR)? (COMMA tableName (DOT STAR)?)*
        USING tableSources
      )
      whereClause?
    ;

// ---------------------------------------------------------------------------
// REPLACE
// ---------------------------------------------------------------------------

replaceStatement
    : REPLACE (LOW_PRIORITY | DELAYED)?
      INTO? tableName
      (PARTITION LPAREN uidList RPAREN)?
      (
        (LPAREN uidList? RPAREN)? insertStatementValue
        | SET updatedElement (COMMA updatedElement)*
      )
    ;

// ---------------------------------------------------------------------------
// CALL
// ---------------------------------------------------------------------------

callStatement
    : CALL fullId (LPAREN (constants | expressions)? RPAREN)?
    ;

// ---------------------------------------------------------------------------
// LOAD DATA / LOAD XML
// ---------------------------------------------------------------------------

loadDataStatement
    : LOAD DATA (LOW_PRIORITY | CONCURRENT)? LOCAL? INFILE STRING_LITERAL
      (REPLACE | IGNORE)?
      INTO TABLE tableName
      (PARTITION LPAREN uidList RPAREN)?
      (CHARACTER SET charsetName)?
      ((FIELDS | COLUMNS) selectFieldsInto+)?
      (LINES selectLinesInto+)?
      (IGNORE decimalLiteral (LINES | ROWS))?
      (LPAREN assignmentField (COMMA assignmentField)* RPAREN)?
      (SET updatedElement (COMMA updatedElement)*)?
    ;

loadXmlStatement
    : LOAD XML (LOW_PRIORITY | CONCURRENT)? LOCAL? INFILE STRING_LITERAL
      (REPLACE | IGNORE)?
      INTO TABLE tableName
      (CHARACTER SET charsetName)?
      (ROWS IDENTIFIED BY STRING_LITERAL)?
      (IGNORE decimalLiteral (LINES | ROWS))?
      (LPAREN assignmentField (COMMA assignmentField)* RPAREN)?
      (SET updatedElement (COMMA updatedElement)*)?
    ;

// ---------------------------------------------------------------------------
// DO
// ---------------------------------------------------------------------------

doStatement
    : DO expressions
    ;

// ---------------------------------------------------------------------------
// HANDLER
// ---------------------------------------------------------------------------

handlerStatement
    : handlerOpenStatement
    | handlerReadIndexStatement
    | handlerReadStatement
    | handlerCloseStatement
    ;

handlerOpenStatement
    : HANDLER tableName OPEN (AS? uid)?
    ;

handlerReadIndexStatement
    : HANDLER tableName READ index=uid
      (comparisonOperator LPAREN constants RPAREN | readDirection=(FIRST | NEXT | PREV | LAST))
      whereClause? limitClause?
    ;

handlerReadStatement
    : HANDLER tableName READ readDirection=(FIRST | NEXT)
      whereClause? limitClause?
    ;

handlerCloseStatement
    : HANDLER tableName CLOSE
    ;

// ---------------------------------------------------------------------------
// VALUES
// ---------------------------------------------------------------------------

valuesStatement
    : VALUES rowConstructor (COMMA rowConstructor)*
      orderByClause? limitClause?
    ;

// ===========================================================================
// TRANSACTION STATEMENTS
// ===========================================================================

transactionStatement
    : startTransaction
    | beginWork
    | commitWork
    | rollbackWork
    | savepointStatement
    | rollbackStatement
    | releaseStatement
    | lockTables
    | unlockTables
    | setAutocommitStatement
    | setTransaction
    | xaTransaction
    ;

startTransaction
    : START TRANSACTION (transactionMode (COMMA transactionMode)*)?
    ;

beginWork
    : BEGIN WORK?
    ;

commitWork
    : COMMIT WORK? (AND NO? CHAIN)? (NO? RELEASE)?
    ;

rollbackWork
    : ROLLBACK WORK? (AND NO? CHAIN)? (NO? RELEASE)?
    ;

savepointStatement
    : SAVEPOINT uid
    ;

rollbackStatement
    : ROLLBACK WORK? TO SAVEPOINT? uid
    ;

releaseStatement
    : RELEASE SAVEPOINT uid
    ;

lockTables
    : LOCK TABLES lockTableElement (COMMA lockTableElement)*
      (WAIT decimalLiteral | NOWAIT)?
    ;

lockTableElement
    : tableName (AS? uid)? lockType
    ;

lockType
    : READ LOCAL?
    | LOW_PRIORITY? WRITE
    ;

unlockTables
    : UNLOCK TABLES
    ;

setAutocommitStatement
    : SET AUTOCOMMIT EQUAL_SYMBOL (ZERO_DECIMAL | ONE_DECIMAL)
    ;

setTransaction
    : SET (GLOBAL | SESSION)? TRANSACTION
      transactionOption (COMMA transactionOption)*
    ;

transactionMode
    : WITH CONSISTENT SNAPSHOT
    | READ WRITE
    | READ ONLY
    ;

transactionOption
    : ISOLATION LEVEL transactionLevel
    | READ WRITE
    | READ ONLY
    ;

transactionLevel
    : REPEATABLE READ
    | READ COMMITTED
    | READ UNCOMMITTED
    | SERIALIZABLE
    ;

// ---------------------------------------------------------------------------
// XA TRANSACTIONS
// ---------------------------------------------------------------------------

xaTransaction
    : xaStartTransaction
    | xaEndTransaction
    | xaPrepareStatement
    | xaCommitWork
    | xaRollbackWork
    | xaRecoverWork
    ;

xaStartTransaction
    : XA (START | BEGIN) xid (JOIN | RESUME)?
    ;

xaEndTransaction
    : XA END xid (SUSPEND (FOR MIGRATE)?)?
    ;

xaPrepareStatement
    : XA PREPARE xid
    ;

xaCommitWork
    : XA COMMIT xid (ONE PHASE)?
    ;

xaRollbackWork
    : XA ROLLBACK xid
    ;

xaRecoverWork
    : XA RECOVER (CONVERT XID)?
    ;

xid
    : gtrid=xuidString (COMMA bqual=xuidString (COMMA formatID=decimalLiteral)?)?
    ;

xuidString
    : STRING_LITERAL | BIT_STRING | HEXADECIMAL_LITERAL
    ;

// ===========================================================================
// REPLICATION STATEMENTS
// ===========================================================================

replicationStatement
    : changeMaster
    | changeReplicationFilter
    | purgeBinaryLogs
    | resetSlave
    | resetMaster
    | startSlave
    | stopSlave
    | startReplica
    | stopReplica
    | resetReplica
    | changeReplicationSource
    | showSlaveStatus
    | showReplicaStatus
    ;

changeMaster
    : CHANGE MASTER TO masterOption (COMMA masterOption)*
      (FOR CHANNEL STRING_LITERAL)?
    ;

changeReplicationSource
    : CHANGE REPLICATION SOURCE TO masterOption (COMMA masterOption)*
      (FOR CHANNEL STRING_LITERAL)?
    ;

masterOption
    : stringMasterOption EQUAL_SYMBOL STRING_LITERAL
    | decimalMasterOption EQUAL_SYMBOL decimalLiteral
    | boolMasterOption EQUAL_SYMBOL boolLiteral
    | MASTER_LOG_FILE EQUAL_SYMBOL STRING_LITERAL
    | MASTER_LOG_POS EQUAL_SYMBOL decimalLiteral
    | RELAY_LOG_FILE EQUAL_SYMBOL STRING_LITERAL
    | RELAY_LOG_POS EQUAL_SYMBOL decimalLiteral
    | IGNORE_SERVER_IDS EQUAL_SYMBOL LPAREN (uid (COMMA uid)*)? RPAREN
    | DO_DOMAIN_IDS EQUAL_SYMBOL LPAREN (uid (COMMA uid)*)? RPAREN
    | IGNORE_DOMAIN_IDS EQUAL_SYMBOL LPAREN (uid (COMMA uid)*)? RPAREN
    | MASTER_USE_GTID EQUAL_SYMBOL (SLAVE_POS | CURRENT_POS | NO)
    ;

stringMasterOption
    : MASTER_BIND | MASTER_HOST | MASTER_USER | MASTER_PASSWORD
    | MASTER_SSL_CA | MASTER_SSL_CAPATH | MASTER_SSL_CERT
    | MASTER_SSL_CRL | MASTER_SSL_CRLPATH | MASTER_SSL_KEY
    | MASTER_SSL_CIPHER | MASTER_TLS_VERSION
    ;

decimalMasterOption
    : MASTER_PORT | MASTER_CONNECT_RETRY | MASTER_RETRY_COUNT
    | MASTER_DELAY | MASTER_HEARTBEAT_PERIOD | MASTER_SERVER_ID
    ;

boolMasterOption
    : MASTER_AUTO_POSITION | MASTER_SSL | MASTER_SSL_VERIFY_SERVER_CERT
    ;

changeReplicationFilter
    : CHANGE REPLICATION FILTER replicationFilter (COMMA replicationFilter)*
    ;

replicationFilter
    : REPLICATE_DO_DB EQUAL_SYMBOL LPAREN uidList RPAREN
    | REPLICATE_IGNORE_DB EQUAL_SYMBOL LPAREN uidList RPAREN
    | REPLICATE_DO_TABLE EQUAL_SYMBOL LPAREN tables RPAREN
    | REPLICATE_IGNORE_TABLE EQUAL_SYMBOL LPAREN tables RPAREN
    | REPLICATE_WILD_DO_TABLE EQUAL_SYMBOL LPAREN simpleStrings RPAREN
    | REPLICATE_WILD_IGNORE_TABLE EQUAL_SYMBOL LPAREN simpleStrings RPAREN
    | REPLICATE_REWRITE_DB EQUAL_SYMBOL LPAREN tablePair (COMMA tablePair)* RPAREN
    ;

tablePair
    : LPAREN firstTable=tableName COMMA secondTable=tableName RPAREN
    ;

purgeBinaryLogs
    : PURGE (BINARY | MASTER) LOGS (TO STRING_LITERAL | BEFORE STRING_LITERAL)
    ;

resetSlave
    : RESET SLAVE ALL? (FOR CHANNEL STRING_LITERAL)?
    ;

resetReplica
    : RESET REPLICA ALL? (FOR CHANNEL STRING_LITERAL)?
    ;

resetMaster
    : RESET MASTER (TO decimalLiteral)?
    ;

startSlave
    : START SLAVE (threadType (COMMA threadType)*)?
      (UNTIL gtidSet=(UNTIL_CONDITION)? | UNTIL untilOption)?
      connectionOptions?
      (FOR CHANNEL STRING_LITERAL)?
    ;

startReplica
    : START REPLICA (threadType (COMMA threadType)*)?
      (FOR CHANNEL STRING_LITERAL)?
    ;

stopSlave
    : STOP SLAVE (threadType (COMMA threadType)*)?
      (FOR CHANNEL STRING_LITERAL)?
    ;

stopReplica
    : STOP REPLICA (threadType (COMMA threadType)*)?
      (FOR CHANNEL STRING_LITERAL)?
    ;

showSlaveStatus
    : SHOW (ALL? SLAVE | ALL? SLAVES) STATUS (FOR CHANNEL STRING_LITERAL)?
    ;

showReplicaStatus
    : SHOW REPLICA STATUS (FOR CHANNEL STRING_LITERAL)?
    ;

threadType
    : IO_THREAD | SQL_THREAD
    ;

untilOption
    : MASTER_LOG_FILE EQUAL_SYMBOL STRING_LITERAL COMMA MASTER_LOG_POS EQUAL_SYMBOL decimalLiteral
    | RELAY_LOG_FILE EQUAL_SYMBOL STRING_LITERAL COMMA RELAY_LOG_POS EQUAL_SYMBOL decimalLiteral
    | SQL_BEFORE_GTIDS EQUAL_SYMBOL gtidSet
    | SQL_AFTER_GTIDS EQUAL_SYMBOL gtidSet
    | SQL_AFTER_MTS_GAPS
    ;

gtidSet
    : uuidSet (COMMA uuidSet)*
    | STRING_LITERAL
    ;

uuidSet
    : STRING_LITERAL COLON intervalExpr (COLON intervalExpr)*
    ;

connectionOptions
    : USER EQUAL_SYMBOL STRING_LITERAL
    | PASSWORD EQUAL_SYMBOL STRING_LITERAL
    | DEFAULT_AUTH EQUAL_SYMBOL STRING_LITERAL
    | PLUGIN_DIR EQUAL_SYMBOL STRING_LITERAL
    ;

UNTIL_CONDITION: U N T I L '_' C O N D I T I O N;
SLAVE_POS: S L A V E '_' P O S;
CURRENT_POS: C U R R E N T '_' P O S;
REPLICATE_DO_DB: R E P L I C A T E '_' D O '_' D B;
REPLICATE_IGNORE_DB: R E P L I C A T E '_' I G N O R E '_' D B;
REPLICATE_DO_TABLE: R E P L I C A T E '_' D O '_' T A B L E;
REPLICATE_IGNORE_TABLE: R E P L I C A T E '_' I G N O R E '_' T A B L E;
REPLICATE_WILD_DO_TABLE: R E P L I C A T E '_' W I L D '_' D O '_' T A B L E;
REPLICATE_WILD_IGNORE_TABLE: R E P L I C A T E '_' W I L D '_' I G N O R E '_' T A B L E;
REPLICATE_REWRITE_DB: R E P L I C A T E '_' R E W R I T E '_' D B;
DEFAULT_AUTH: D E F A U L T '_' A U T H;

// ===========================================================================
// PREPARED STATEMENTS
// ===========================================================================

preparedStatement
    : prepareStatement
    | executeStatement
    | deallocatePrepare
    ;

prepareStatement
    : PREPARE uid FROM (STRING_LITERAL | LOCAL_ID | GLOBAL_ID)
    ;

executeStatement
    : EXECUTE uid (USING userVariables)?
    ;

deallocatePrepare
    : (DEALLOCATE | DROP) PREPARE uid
    ;

userVariables
    : LOCAL_ID (COMMA LOCAL_ID)*
    ;

// ===========================================================================
// COMPOUND STATEMENTS (Procedural SQL)
// ===========================================================================

compoundStatement
    : blockStatement
    | caseStatement
    | ifStatement
    | leaveStatement
    | loopStatement
    | repeatStatement
    | whileStatement
    | iterateStatement
    | returnStatement
    | cursorStatement
    | declareVariable
    | declareCondition
    | declareHandler
    | declareCursor
    | signalStatement
    | resignalStatement
    | getStatement
    ;

blockStatement
    : (uid COLON)? BEGIN
      (declareVariable SEMICOLON)*
      (declareCondition SEMICOLON)*
      (declareCursor SEMICOLON)*
      (declareHandler SEMICOLON)*
      procedureSqlStatement*
      END uid?
    ;

caseStatement
    : CASE expression?
      (WHEN expression THEN procedureSqlStatement+)+
      (ELSE procedureSqlStatement+)?
      END CASE
    ;

ifStatement
    : IF expression THEN procedureSqlStatement+
      (ELSEIF expression THEN procedureSqlStatement+)*
      (ELSE procedureSqlStatement+)?
      END IF
    ;

iterateStatement
    : ITERATE uid
    ;

leaveStatement
    : LEAVE uid
    ;

loopStatement
    : (uid COLON)?
      LOOP procedureSqlStatement+
      END LOOP uid?
    ;

repeatStatement
    : (uid COLON)?
      REPEAT procedureSqlStatement+
      UNTIL expression
      END REPEAT uid?
    ;

whileStatement
    : (uid COLON)?
      WHILE expression
      DO procedureSqlStatement+
      END WHILE uid?
    ;

returnStatement
    : RETURN expression
    ;

cursorStatement
    : CLOSE uid                                                   # closeCursor
    | FETCH NEXT? FROM? uid INTO uidList                         # fetchCursor
    | OPEN uid                                                    # openCursor
    ;

declareVariable
    : DECLARE uidList dataType (DEFAULT defaultValue)?
    ;

declareCondition
    : DECLARE uid CONDITION FOR (decimalLiteral | SQLSTATE VALUE? STRING_LITERAL)
    ;

declareCursor
    : DECLARE uid CURSOR FOR selectStatement
    ;

declareHandler
    : DECLARE handlerAction=(CONTINUE | EXIT | UNDO)
      HANDLER FOR handlerConditionValue (COMMA handlerConditionValue)*
      routineBody
    ;

handlerConditionValue
    : decimalLiteral                                              # handlerConditionCode
    | SQLSTATE VALUE? STRING_LITERAL                              # handlerConditionState
    | uid                                                         # handlerConditionName
    | SQLWARNING                                                  # handlerConditionWarning
    | NOT FOUND                                                   # handlerConditionNotfound
    | SQLEXCEPTION                                                # handlerConditionException
    ;

procedureSqlStatement
    : (compoundStatement | sqlStatement) SEMICOLON?
    ;

// ---------------------------------------------------------------------------
// SIGNAL / RESIGNAL / GET
// ---------------------------------------------------------------------------

signalStatement
    : SIGNAL (SQLSTATE VALUE? STRING_LITERAL | uid | SQLSTATE VALUE?)
      (SET signalInformationItem (COMMA signalInformationItem)*)?
    ;

resignalStatement
    : RESIGNAL (SQLSTATE VALUE? STRING_LITERAL | uid)?
      (SET signalInformationItem (COMMA signalInformationItem)*)?
    ;

signalInformationItem
    : signalInformationItemName EQUAL_SYMBOL expression
    ;

signalInformationItemName
    : CLASS_ORIGIN | SUBCLASS_ORIGIN | MESSAGE_TEXT | MYSQL_ERRNO
    | CONSTRAINT_CATALOG | CONSTRAINT_SCHEMA | CONSTRAINT_NAME
    | CATALOG_NAME | SCHEMA_NAME | TABLE_NAME | COLUMN_NAME | CURSOR_NAME
    ;

getStatement
    : GET (CURRENT | STACKED)? DIAGNOSTICS
      (getConditionCount | getConditionInformation)
    ;

getConditionCount
    : LOCAL_ID EQUAL_SYMBOL (NUMBER | ROW_COUNT)
    ;

getConditionInformation
    : CONDITION decimalLiteral getConditionInformationItem (COMMA getConditionInformationItem)*
    ;

getConditionInformationItem
    : LOCAL_ID EQUAL_SYMBOL signalInformationItemName
    ;

// ===========================================================================
// ADMINISTRATION STATEMENTS
// ===========================================================================

administrationStatement
    : alterUser
    | createUser
    | dropUser
    | grantStatement
    | grantProxy
    | renameUser
    | revokeStatement
    | revokeProxy
    | setPasswordStatement
    | setRoleStatement
    | analyzeTable
    | checkTable
    | checksumTable
    | optimizeTable
    | repairTable
    | installPlugin
    | uninstallPlugin
    | setStatement
    | showStatement
    | binlogStatement
    | cacheIndexStatement
    | flushStatement
    | killStatement
    | loadIndexIntoCache
    | resetStatement
    | shutdownStatement
    | helpStatement
    ;

// ---------------------------------------------------------------------------
// USER MANAGEMENT
// ---------------------------------------------------------------------------

createUser
    : CREATE USER ifNotExists?
      userAuthOption (COMMA userAuthOption)*
      (
        REQUIRE (tlsNone=NONE | tlsOption (AND? tlsOption)*)
      )?
      (WITH userResourceOption+)?
      (userPasswordOption | userLockOption)*
      (COMMENT STRING_LITERAL)?
    ;

alterUser
    : ALTER USER ifExists?
      userSpecification (COMMA userSpecification)*
      (
        REQUIRE (tlsNone=NONE | tlsOption (AND? tlsOption)*)
      )?
      (WITH userResourceOption+)?
      (userPasswordOption | userLockOption)*
    ;

dropUser
    : DROP USER ifExists? userName (COMMA userName)*
    ;

grantStatement
    : GRANT privelegeClause (COMMA privelegeClause)*
      ON privilegeObject?
      TO userAuthOption (COMMA userAuthOption)*
      (REQUIRE (tlsNone=NONE | tlsOption (AND? tlsOption)*))?
      (WITH (GRANT OPTION | userResourceOption)+)?
      (AS userName WITH ROLE roleOption)?
    ;

grantProxy
    : GRANT PROXY ON userName TO userName (COMMA userName)*
      (WITH GRANT OPTION)?
    ;

revokeStatement
    : REVOKE privelegeClause (COMMA privelegeClause)*
      ON privilegeObject? FROM userName (COMMA userName)*
    | REVOKE ALL PRIVILEGES? COMMA GRANT OPTION FROM userName (COMMA userName)*
    ;

revokeProxy
    : REVOKE PROXY ON userName FROM userName (COMMA userName)*
    ;

renameUser
    : RENAME USER renameUserClause (COMMA renameUserClause)*
    ;

renameUserClause
    : userName TO userName
    ;

userAuthOption
    : userName IDENTIFIED BY (PASSWORD)? STRING_LITERAL           # authOptionPassword
    | userName IDENTIFIED WITH authenticationRule                 # authOptionByPlugin
    | userName (IDENTIFIED (VIA | WITH) authenticationRule (OR authenticationRule)*)?  # authOptionVia
    | userName                                                    # authOptionPlain
    ;

authenticationRule
    : authPlugin ((BY | USING | AS) STRING_LITERAL)?
    ;

authPlugin
    : uid
    | STRING_LITERAL
    ;

userSpecification
    : userName (IDENTIFIED BY STRING_LITERAL)?
    ;

tlsOption
    : SSL
    | X509
    | CIPHER STRING_LITERAL
    | ISSUER STRING_LITERAL
    | SUBJECT STRING_LITERAL
    ;

userResourceOption
    : MAX_QUERIES_PER_HOUR decimalLiteral
    | MAX_UPDATES_PER_HOUR decimalLiteral
    | MAX_CONNECTIONS_PER_HOUR decimalLiteral
    | MAX_USER_CONNECTIONS decimalLiteral
    | MAX_STATEMENT_TIME decimalLiteral
    ;

userPasswordOption
    : PASSWORD EXPIRE (DEFAULT | NEVER | INTERVAL decimalLiteral DAY)?
    | PASSWORD HISTORY (DEFAULT | decimalLiteral)
    | PASSWORD REUSE INTERVAL (DEFAULT | decimalLiteral DAY)
    | PASSWORD REQUIRE CURRENT (DEFAULT | OPTIONAL)?
    | FAILED_LOGIN_ATTEMPTS decimalLiteral
    | PASSWORD_LOCK_TIME (decimalLiteral | UNBOUNDED)
    ;

userLockOption
    : ACCOUNT (LOCK | UNLOCK)
    ;

privelegeClause
    : privilege (LPAREN uidList RPAREN)?
    ;

privilege
    : ALL PRIVILEGES?
    | ALTER ROUTINE?
    | CREATE (ROUTINE | TEMPORARY TABLES | USER | VIEW | ROLE)?
    | DELETE
    | DELETE HISTORY
    | DROP ROLE?
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
    | REPLICATION (CLIENT | SLAVE | REPLICA | MASTER ADMIN | SLAVE ADMIN | REPLICA ADMIN)
    | SELECT
    | SHOW DATABASES
    | SHOW VIEW
    | SHUTDOWN
    | SUPER
    | TRIGGER
    | UPDATE
    | USAGE
    | BINLOG MONITOR
    | CONNECTION ADMIN
    | FEDERATED ADMIN
    | READ_ONLY ADMIN
    | REPLICATION SLAVE ADMIN
    | SET USER
    | SLAVE MONITOR
    ;

privilegeObject
    : privilegeLevel
    | TABLE? tableName
    | FUNCTION fullId
    | PROCEDURE fullId
    ;

privilegeLevel
    : STAR                                                        # currentSchemaPriviLevel
    | STAR DOT STAR                                               # globalPrivLevel
    | uid DOT STAR                                                # definiteSchemaPrivLevel
    | uid DOT uid                                                 # definiteFullTablePrivLevel
    | uid                                                         # definiteTablePrivLevel
    ;

setPasswordStatement
    : SET PASSWORD (FOR userName)?
      EQUAL_SYMBOL (PASSWORD LPAREN STRING_LITERAL RPAREN | STRING_LITERAL)
    ;

setRoleStatement
    : SET ROLE roleOption
    | SET DEFAULT ROLE (NONE | ALL | roleName (COMMA roleName)*)
      TO userName (COMMA userName)*
    ;

roleOption
    : DEFAULT
    | NONE
    | ALL (EXCEPT roleName (COMMA roleName)*)?
    | roleName (COMMA roleName)*
    ;

// ---------------------------------------------------------------------------
// TABLE MAINTENANCE
// ---------------------------------------------------------------------------

analyzeTable
    : ANALYZE actionOption=(NO_WRITE_TO_BINLOG | LOCAL)?
      (TABLE tables | TABLES tables (COMMA tables)*)
    ;

checkTable
    : CHECK TABLE tables checkTableOption*
    ;

checkTableOption
    : FOR UPGRADE | QUICK | FAST | MEDIUM | EXTENDED | CHANGED
    ;

checksumTable
    : CHECKSUM TABLE tables (QUICK | EXTENDED)?
    ;

optimizeTable
    : OPTIMIZE actionOption=(NO_WRITE_TO_BINLOG | LOCAL)?
      (TABLE | TABLES) tables
    ;

repairTable
    : REPAIR actionOption=(NO_WRITE_TO_BINLOG | LOCAL)?
      TABLE tables QUICK? EXTENDED? USE_FRM?
    ;

// ---------------------------------------------------------------------------
// PLUGIN
// ---------------------------------------------------------------------------

installPlugin
    : INSTALL PLUGIN uid SONAME STRING_LITERAL
    ;

uninstallPlugin
    : UNINSTALL PLUGIN uid
    ;

// ---------------------------------------------------------------------------
// SET
// ---------------------------------------------------------------------------

setStatement
    : SET variableClause (COMMA variableClause)*
    | setCharset
    | setNames
    | setTransaction
    ;

variableClause
    : (GLOBAL | PERSIST | PERSIST_ONLY | SESSION)? (uid | LOCAL_ID | GLOBAL_ID)
      EQUAL_SYMBOL (expression | DEFAULT)
    | LOCAL_ID COLON_EQ expression
    | GLOBAL_ID COLON_EQ expression
    ;

setCharset
    : SET (CHARACTER SET | CHARSET) (charsetName | DEFAULT)
    ;

setNames
    : SET NAMES (charsetName (COLLATE collationName)? | DEFAULT)
    ;

// ---------------------------------------------------------------------------
// SHOW
// ---------------------------------------------------------------------------

showStatement
    : SHOW showSchemaEntity showFilter?                           # showSchema
    | SHOW FULL? showTableEntity showFilter?                      # showTables
    | SHOW BINARY LOGS showFilter?                                # showBinaryLogs
    | SHOW BINLOG EVENTS (IN STRING_LITERAL)? (FROM decimalLiteral)? limitClause? # showBinlogEvents
    | SHOW (CHARACTER SET | CHARSET) showFilter?                  # showCharset
    | SHOW COLLATION showFilter?                                  # showCollation
    | SHOW FULL? COLUMNS (FROM | IN) tableName ((FROM | IN) uid)? showFilter? # showColumns
    | SHOW CREATE DATABASE uid                                    # showCreateDb
    | SHOW CREATE EVENT fullId                                    # showCreateEvent
    | SHOW CREATE FUNCTION fullId                                 # showCreateFunction
    | SHOW CREATE PROCEDURE fullId                                # showCreateProcedure
    | SHOW CREATE TABLE tableName                                 # showCreateTable
    | SHOW CREATE TRIGGER fullId                                  # showCreateTrigger
    | SHOW CREATE VIEW fullId                                     # showCreateView
    | SHOW CREATE SEQUENCE fullId                                 # showCreateSequence
    | SHOW CREATE PACKAGE fullId                                  # showCreatePackage
    | SHOW CREATE USER userName                                   # showCreateUser
    | SHOW ENGINE engineName engineOption=(STATUS | MUTEX)        # showEngine
    | SHOW STORAGE? ENGINES                                       # showEngines
    | SHOW ERRORS (LIMIT (offset=decimalLiteral COMMA)? rowCount=decimalLiteral)? # showErrors
    | SHOW COUNT LPAREN STAR RPAREN (ERRORS | WARNINGS)           # showCountErrors
    | SHOW EVENTS (IN uid)? showFilter?                           # showEvents
    | SHOW FUNCTION CODE fullId                                   # showFunctionCode
    | SHOW FUNCTION STATUS showFilter?                            # showFunctionStatus
    | SHOW GRANTS (FOR userName (USING roleName (COMMA roleName)*)?)?  # showGrants
    | SHOW (INDEX | INDEXES | KEYS) (FROM | IN) tableName ((FROM | IN) uid)? whereClause? # showIndex
    | SHOW MASTER STATUS                                          # showMasterStatus
    | SHOW OPEN TABLES (IN uid)? showFilter?                      # showOpenTables
    | SHOW PLUGINS                                                # showPlugins
    | SHOW PRIVILEGES                                             # showPrivileges
    | SHOW PROCEDURE CODE fullId                                  # showProcedureCode
    | SHOW PROCEDURE STATUS showFilter?                           # showProcedureStatus
    | SHOW FULL? PROCESSLIST                                      # showProcessList
    | SHOW PROFILE showProfileType (COMMA showProfileType)* (FOR QUERY decimalLiteral)? limitClause? # showProfile
    | SHOW PROFILES                                               # showProfiles
    | SHOW RELAYLOG EVENTS (IN STRING_LITERAL)? (FROM decimalLiteral)? limitClause? (FOR CHANNEL STRING_LITERAL)? # showRelayLogEvents
    | SHOW STATUS showFilter?                                     # showStatus
    | SHOW TABLE STATUS (IN uid)? showFilter?                     # showTableStatus
    | SHOW TRIGGERS (IN uid)? showFilter?                         # showTriggers
    | SHOW (GLOBAL | SESSION)? VARIABLES showFilter?              # showVariables
    | SHOW WARNINGS (LIMIT (offset=decimalLiteral COMMA)? rowCount=decimalLiteral)? # showWarnings
    | SHOW DATABASES showFilter?                                  # showDatabases
    | SHOW SCHEMAS showFilter?                                    # showSchemas
    ;

showSchemaEntity
    : (GLOBAL | SESSION)? STATUS
    | FULL? (COLUMNS | FIELDS) (FROM | IN) tableName
    ;

showTableEntity
    : TABLES ((FROM | IN) uid)?
    ;

showFilter
    : LIKE STRING_LITERAL
    | WHERE expression
    ;

showProfileType
    : ALL | BLOCK IO | CONTEXT SWITCHES | CPU | IPC | MEMORY
    | PAGE FAULTS | SOURCE | SWAPS
    ;

// ---------------------------------------------------------------------------
// OTHER ADMIN
// ---------------------------------------------------------------------------

binlogStatement
    : BINLOG STRING_LITERAL
    ;

cacheIndexStatement
    : CACHE INDEX tableIndexes (COMMA tableIndexes)*
      (PARTITION LPAREN (uidList | ALL) RPAREN)?
      IN uid
    ;

tableIndexes
    : tableName (INDEX | KEY)? LPAREN (uid (COMMA uid)* | PRIMARY)? RPAREN
    ;

flushStatement
    : FLUSH flushFormat=(NO_WRITE_TO_BINLOG | LOCAL)?
      flushOption (COMMA flushOption)*
    ;

flushOption
    : (BINARY | ENGINE | ERROR | GENERAL | HOSTS | LOGS | PRIVILEGES
       | OPTIMIZER_COSTS | QUERY CACHE | RELAY | SLOW | STATUS
       | USER_RESOURCES | DES_KEY_FILE | TABLES WITH? READ? LOCK?) # simpleFlushOption
    | TABLES tableName (COMMA tableName)* flushTableOption?        # tableFlushOption
    ;

flushTableOption
    : WITH READ LOCK
    | FOR EXPORT
    ;

killStatement
    : KILL connectionFormat=(CONNECTION | QUERY)?
      (decimalLiteral | expression)+
    ;

loadIndexIntoCache
    : LOAD INDEX INTO CACHE tableIndexes (COMMA tableIndexes)*
    ;

resetStatement
    : RESET QUERY CACHE
    ;

shutdownStatement
    : SHUTDOWN
    ;

helpStatement
    : HELP STRING_LITERAL
    ;

// ===========================================================================
// UTILITY STATEMENTS
// ===========================================================================

utilityStatement
    : simpleDescribeStatement
    | fullDescribeStatement
    | explainStatement
    | useStatement
    ;

simpleDescribeStatement
    : (EXPLAIN | DESCRIBE | DESC) tableName (uid | STRING_LITERAL)?
    ;

fullDescribeStatement
    : (EXPLAIN | DESCRIBE | DESC)
      (EXTENDED | PARTITIONS | FORMAT EQUAL_SYMBOL (TRADITIONAL | JSON | TREE))?
      (selectStatement | deleteStatement | insertStatement | replaceStatement | updateStatement)
    ;

explainStatement
    : (EXPLAIN | DESCRIBE | DESC)
      (ANALYZE | EXTENDED | PARTITIONS | FORMAT EQUAL_SYMBOL (TRADITIONAL | JSON | TREE))?
      FOR CONNECTION decimalLiteral
    ;

useStatement
    : USE uid
    ;

// ===========================================================================
// TABLE SOURCES
// ===========================================================================

tableSources
    : tableSource (COMMA tableSource)*
    ;

tableSource
    : tableSourceItem joinPart*
    ;

tableSourceItem
    : tableName (PARTITION LPAREN uidList RPAREN)? (AS? alias=uid)? indexHint*
      (FOR SYSTEM_TIME asOfClause)?                               # atomTableItem
    | (LATERAL? subquery | LPAREN parenthesizedTableSources RPAREN)
      (AS? alias=uid)?                                            # subqueryTableItem
    | LPAREN tableSources RPAREN                                  # tableSourcesItem
    ;

parenthesizedTableSources
    : tableSources
    ;

asOfClause
    : AS OF expression
    | BETWEEN expression AND expression
    | FROM expression TO expression
    | ALL
    ;

indexHint
    : indexHintAction=(USE | IGNORE | FORCE)
      keyFormat=(INDEX | KEY) (FOR indexHintType)?
      LPAREN uidList RPAREN
    ;

indexHintType
    : JOIN | ORDER BY | GROUP BY
    ;

joinPart
    : (INNER | CROSS)? JOIN tableSourceItem (ON expression | USING LPAREN uidList RPAREN)? # innerJoin
    | STRAIGHT_JOIN tableSourceItem (ON expression)?               # straightJoin
    | (LEFT | RIGHT) OUTER? JOIN tableSourceItem (ON expression | USING LPAREN uidList RPAREN) # outerJoin
    | NATURAL ((LEFT | RIGHT) OUTER?)? JOIN tableSourceItem       # naturalJoin
    ;

subquery
    : LPAREN selectStatement RPAREN
    ;

// ===========================================================================
// EXPRESSIONS
// ===========================================================================

expressions
    : expression (COMMA expression)*
    ;

expression
    : notOperator=(NOT | EXCLAMATION_SYMBOL) expression           # notExpression
    | expression logicalOperator expression                       # logicalExpression
    | predicate IS NOT? testValue=(TRUE | FALSE | UNKNOWN)        # isExpression
    | predicate                                                   # predicateExpression
    ;

predicate
    : predicate NOT? IN LPAREN (subquery | expressions) RPAREN    # inPredicate
    | predicate IS NOT? nullNotnull                               # isNullPredicate
    | left=predicate comparisonOperator right=predicate           # binaryComparisonPredicate
    | predicate comparisonOperator quantifier=(ALL | ANY | SOME) subquery # subqueryComparisonPredicate
    | predicate NOT? BETWEEN predicate AND predicate              # betweenPredicate
    | predicate SOUNDS LIKE predicate                             # soundsLikePredicate
    | predicate NOT? LIKE predicate (ESCAPE STRING_LITERAL)?      # likePredicate
    | predicate NOT? regex=(REGEXP | RLIKE) predicate             # regexpPredicate
    | predicate MEMBER OF LPAREN predicate RPAREN                 # memberOfPredicate
    | expressionAtom                                              # expressionAtomPredicate
    ;

expressionAtom
    : constant                                                    # constantExpressionAtom
    | fullColumnName                                              # fullColumnNameExpressionAtom
    | functionCall                                                # functionCallExpressionAtom
    | collateExpr                                                 # collateExpressionAtom
    | mysqlVariable                                               # mysqlVariableExpressionAtom
    | unaryOperator expressionAtom                                # unaryExpressionAtom
    | BINARY expressionAtom                                       # binaryExpressionAtom
    | LPAREN expression (COMMA expression)* RPAREN                # nestedExpressionAtom
    | ROW LPAREN expression (COMMA expression)+ RPAREN            # nestedRowExpressionAtom
    | EXISTS subquery                                             # existsExpressionAtom
    | subquery                                                    # subqueryExpressionAtom
    | INTERVAL expression intervalType                            # intervalExpressionAtom
    | left=expressionAtom bitOperator right=expressionAtom        # bitExpressionAtom
    | left=expressionAtom mathOperator right=expressionAtom       # mathExpressionAtom
    | left=expressionAtom jsonOperator right=expressionAtom       # jsonExpressionAtom
    | caseExpr                                                    # caseExpressionAtom
    ;

collateExpr
    : expressionAtom COLLATE collationName
    ;

caseExpr
    : CASE expression? (WHEN expression THEN expression)+
      (ELSE expression)? END
    ;

unaryOperator
    : EXCLAMATION_SYMBOL | BIT_NOT_OP | PLUS | MINUS_OP | NOT
    ;

comparisonOperator
    : EQUAL_SYMBOL | GREATER_SYMBOL | LESS_SYMBOL | LESS_OR_EQUAL
    | GREATER_OR_EQUAL | NOT_EQUAL | NULL_SAFE_EQUAL
    ;

logicalOperator
    : AND | AND_OP | XOR | OR | OR_OP
    ;

bitOperator
    : LEFT_SHIFT | RIGHT_SHIFT | BIT_AND_OP | BIT_XOR_OP | BIT_OR_OP
    ;

mathOperator
    : STAR | DIVIDE | MOD_OP | DIV | MOD | PLUS | MINUS_OP
    ;

jsonOperator
    : ARROW | DOUBLE_ARROW
    ;

// ===========================================================================
// FUNCTIONS
// ===========================================================================

functionCall
    : specificFunction                                            # specificFunctionCall
    | aggregateWindowedFunction                                   # aggregateFunctionCall
    | nonAggregateWindowedFunction                                # nonAggregateFunctionCall
    | scalarFunctionCall                                          # scalarFunctionCallExpr
    | fullId LPAREN functionArgs? RPAREN                          # udfFunctionCall
    | passwordFunctionClause                                      # passwordFunctionCall
    ;

specificFunction
    : (CURRENT_DATE | CURRENT_TIME | CURRENT_TIMESTAMP | LOCALTIME | LOCALTIMESTAMP | UTC_DATE | UTC_TIME | UTC_TIMESTAMP)
      (LPAREN decimalLiteral? RPAREN)?                            # simpleFunctionCall
    | CURRENT_USER LPAREN? RPAREN?                                # currentUser
    | CURRENT_ROLE LPAREN? RPAREN?                                # currentRole
    | CONVERT LPAREN expression COMMA convertedDataType RPAREN    # dataTypeFunctionCall
    | CONVERT LPAREN expression USING charsetName RPAREN          # charsetConvertFunctionCall
    | CAST LPAREN expression AS convertedDataType RPAREN          # castFunctionCall
    | VALUES LPAREN fullColumnName RPAREN                         # valuesFunctionCall
    | CASE expression? (WHEN expression THEN expression)+
      (ELSE expression)? END                                      # caseFunctionCall
    | CHAR LPAREN functionArgs (USING charsetName)? RPAREN        # charFunctionCall
    | POSITION LPAREN (expression | stringLiteral) IN (expression | stringLiteral) RPAREN # positionFunctionCall
    | SUBSTRING LPAREN stringLiteral FROM decimalLiteral (FOR decimalLiteral)? RPAREN # substringFunctionCall
    | TRIM LPAREN trimOption=(BOTH | LEADING | TRAILING)? 
      (expression | stringLiteral)? FROM? (expression | stringLiteral) RPAREN # trimFunctionCall
    | TRIM LPAREN (expression | stringLiteral) RPAREN             # simpleTrimFunctionCall
    | WEIGHT_STRING LPAREN expression (AS stringFormat=(CHAR | BINARY) LPAREN decimalLiteral RPAREN)? levelsInWeightString? RPAREN # weightFunctionCall
    | EXTRACT LPAREN intervalType FROM expression RPAREN          # extractFunctionCall
    | GET_FORMAT LPAREN dateTimeFormat=(DATE | TIME | DATETIME) COMMA stringLiteral RPAREN # getFormatFunctionCall
    | JSON_VALUE LPAREN expression COMMA expression (RETURNING convertedDataType)? onEmptyOrError* RPAREN # jsonValueFunctionCall
    | jsonTableFunction                                           # jsonTableFunctionCall
    ;

jsonTableFunction
    : JSON_TABLE LPAREN expression COMMA STRING_LITERAL
      COLUMNS LPAREN jsonTableColumn (COMMA jsonTableColumn)* RPAREN
      RPAREN (AS? alias=uid)?
    ;

jsonTableColumn
    : uid (FOR ORDINALITY | dataType (PATH STRING_LITERAL onEmptyOrError* | EXISTS PATH STRING_LITERAL))
    | NESTED PATH? STRING_LITERAL COLUMNS LPAREN jsonTableColumn (COMMA jsonTableColumn)* RPAREN
    ;

onEmptyOrError
    : (NULL_LITERAL | ERROR | DEFAULT expression) ON (EMPTY | ERROR)
    ;

levelsInWeightString
    : LEVEL (levelInWeightListElement (COMMA levelInWeightListElement)*)
    | LEVEL firstLevel=decimalLiteral MINUS_OP lastLevel=decimalLiteral
    ;

levelInWeightListElement
    : decimalLiteral orderType=(ASC | DESC | REVERSE)?
    ;

aggregateWindowedFunction
    : (AVG | MAX | MIN | SUM) LPAREN aggregator=(ALL | DISTINCT)? functionArg RPAREN overClause?
    | COUNT LPAREN (starArg=STAR | aggregator=ALL? functionArg | aggregator=DISTINCT functionArgs) RPAREN overClause?
    | (BIT_AND | BIT_OR | BIT_XOR | STD | STDDEV | STDDEV_POP | STDDEV_SAMP | VAR_POP | VAR_SAMP | VARIANCE)
      LPAREN aggregator=ALL? functionArg RPAREN overClause?
    | GROUP_CONCAT LPAREN aggregator=DISTINCT? functionArgs (ORDER BY orderByExpression (COMMA orderByExpression)*)? (SEPARATOR STRING_LITERAL)? RPAREN
    | JSON_ARRAYAGG LPAREN functionArg RPAREN overClause?
    | JSON_OBJECTAGG LPAREN functionArg COMMA functionArg RPAREN overClause?
    ;

nonAggregateWindowedFunction
    : (LAG | LEAD) LPAREN expression (COMMA expression (COMMA expression)?)? RPAREN overClause
    | (FIRST_VALUE | LAST_VALUE) LPAREN expression RPAREN overClause
    | (CUME_DIST | DENSE_RANK | PERCENT_RANK | RANK | ROW_NUMBER) LPAREN RPAREN overClause
    | NTH_VALUE LPAREN expression COMMA decimalLiteral RPAREN overClause
    | NTILE LPAREN decimalLiteral RPAREN overClause
    ;

overClause
    : OVER (windowName=uid | LPAREN windowSpec RPAREN)
    ;

windowSpec
    : windowName=uid?
      (PARTITION BY expression (COMMA expression)*)?
      (ORDER BY orderByExpression (COMMA orderByExpression)*)?
      frameClause?
    ;

frameClause
    : frameUnits=(ROWS | RANGE) (frameStart | frameRange)
    ;

frameStart
    : UNBOUNDED PRECEDING
    | decimalLiteral (PRECEDING | FOLLOWING)
    | CURRENT ROW
    ;

frameRange
    : BETWEEN frameRangeBound AND frameRangeBound
    ;

frameRangeBound
    : frameStart
    | UNBOUNDED FOLLOWING
    ;

scalarFunctionCall
    : scalarFunctionName LPAREN functionArgs? RPAREN
    ;

scalarFunctionName
    : functionNameBase
    | ASCII_SYM | CURDATE | CURTIME | CURTIME | DATE_ADD | DATE_SUB
    | IF | INSERT | MOD | PASSWORD | REPEAT | REPLACE | REVERSE
    | SCHEMA | SUBSTR | SUBSTRING | SYSDATE | TIME | TIMESTAMP | TRUNCATE
    | ADDDATE | SUBDATE | NOW | COALESCE | IFNULL | ISNULL | NULLIF
    | ABS | ACOS | ASIN | ATAN | ATAN2 | CEIL | CEILING | CONV | COS | COT
    | CRC32 | DEGREES | DIV | EXP | FLOOR | LN | LOG | LOG10 | LOG2 | MOD
    | PI | POW | POWER | RADIANS | RAND | ROUND | SIGN | SIN | SQRT | TAN
    ;

functionNameBase
    : ABS | ADDDATE | ADDTIME | AES_DECRYPT | AES_ENCRYPT | ASCII_SYM
    | BENCHMARK | BIN | BIT_COUNT | BIT_LENGTH
    | CHAR_LENGTH | CHARACTER_LENGTH | COERCIBILITY | COMPRESS | CONCAT | CONCAT_WS | CONNECTION_ID | CONV | CONVERT_TZ
    | DATABASE | DATE | DATE_FORMAT | DATEDIFF | DAY | DAYNAME | DAYOFMONTH | DAYOFWEEK | DAYOFYEAR | DECODE | DES_DECRYPT | DES_ENCRYPT
    | ELT | ENCODE | ENCRYPT | EXPORT_SET | EXTRACTVALUE
    | FIELD | FIND_IN_SET | FORMAT | FOUND_ROWS | FROM_BASE64 | FROM_DAYS | FROM_UNIXTIME
    | GEOMCOLLECTION | GEOMETRYCOLLECTION | GET_LOCK | GREATEST | GTID_SUBSET | GTID_SUBTRACT
    | HEX | HOUR
    | IFNULL | INET_ATON | INET_NTOA | INET6_ATON | INET6_NTOA | INSTR | IS_FREE_LOCK | IS_IPV4 | IS_IPV4_COMPAT | IS_IPV4_MAPPED | IS_IPV6 | IS_USED_LOCK
    | LAST_DAY | LAST_INSERT_ID | LCASE | LEAST | LEFT | LENGTH | LINESTRING | LN | LOAD_FILE | LOCATE | LOG | LOG10 | LOG2 | LOWER | LPAD | LTRIM
    | MAKE_SET | MAKEDATE | MAKETIME | MASTER_POS_WAIT | MAX | MD5 | MICROSECOND | MID | MIN | MINUTE | MONTH | MONTHNAME | MULTILINESTRING | MULTIPOINT | MULTIPOLYGON
    | NAME_CONST | NOW | NULLIF
    | OCT | OCTET_LENGTH | OLD_PASSWORD | ORD
    | PERIOD_ADD | PERIOD_DIFF | POINT | POLYGON | POSITION | POW | POWER
    | QUARTER | QUOTE
    | RADIANS | RAND | RANDOM_BYTES | RELEASE_ALL_LOCKS | RELEASE_LOCK | REPEAT | REPLACE | REVERSE | RIGHT | ROUND | ROW_COUNT | RPAD | RTRIM
    | SCHEMA | SEC_TO_TIME | SECOND | SESSION_USER | SHA | SHA1 | SHA2 | SIGN | SIN | SLEEP | SOUNDEX | SPACE | SQL_THREAD_WAIT_AFTER_GTIDS | SQRT | STD | STDDEV | STDDEV_POP | STDDEV_SAMP | STR_TO_DATE | STRCMP | SUBDATE | SUBSTR | SUBSTRING | SUBSTRING_INDEX | SUM | SYSDATE | SYSTEM_USER
    | TAN | TIME | TIMEDIFF | TIME_FORMAT | TIME_TO_SEC | TIMESTAMP | TIMESTAMPADD | TIMESTAMPDIFF | TO_BASE64 | TO_DAYS | TO_SECONDS | TRIM
    | UCASE | UNCOMPRESS | UNCOMPRESSED_LENGTH | UNHEX | UNIX_TIMESTAMP | UPDATEXML | UPPER | USER | UUID | UUID_SHORT
    | VALIDATE_PASSWORD_STRENGTH | VALUES | VAR_POP | VAR_SAMP | VARIANCE | VERSION
    | WAIT_FOR_EXECUTED_GTID_SET | WAIT_UNTIL_SQL_THREAD_AFTER_GTIDS | WEEK | WEEKDAY | WEEKOFYEAR | WEIGHT_STRING
    | YEAR | YEARWEEK
    ;

passwordFunctionClause
    : PASSWORD LPAREN functionArg RPAREN
    | OLD_PASSWORD LPAREN functionArg RPAREN
    ;

functionArgs
    : functionArg (COMMA functionArg)*
    ;

functionArg
    : constant | fullColumnName | functionCall | expression
    ;

// ===========================================================================
// DATA TYPES
// ===========================================================================

dataType
    : typeName=(CHAR | VARCHAR | TINYTEXT | TEXT | MEDIUMTEXT | LONGTEXT
       | NCHAR | NVARCHAR | LONG | CLOB | RAW | VARCHAR2)
      lengthOneDimension? BINARY?
      (CHARACTER SET charsetName)?
      (COLLATE collationName)?                                    # stringDataType
    | NATIONAL typeName=(VARCHAR | CHARACTER) lengthOneDimension? BINARY? # nationalStringDataType
    | NCHAR typeName=VARCHAR lengthOneDimension? BINARY?          # nationalVaryingStringDataType
    | typeName=(TINYINT | SMALLINT | MEDIUMINT | INT | INTEGER | BIGINT
       | MIDDLEINT | INT1 | INT2 | INT3 | INT4 | INT8 | NUMBER)
      lengthOneDimension? (SIGNED | UNSIGNED)? ZEROFILL?          # dimensionDataType
    | typeName=REAL lengthTwoDimension? (SIGNED | UNSIGNED)? ZEROFILL? # doubleDataType
    | typeName=(DOUBLE PRECISION? | FLOAT8) lengthTwoDimension? (SIGNED | UNSIGNED)? ZEROFILL? # doubleDataType
    | typeName=(DECIMAL | DEC | FIXED | NUMERIC | FLOAT | FLOAT4)
      lengthTwoOptionalDimension? (SIGNED | UNSIGNED)? ZEROFILL?  # dimensionDataType
    | typeName=(DATE | YEAR | TINYBLOB | BLOB | MEDIUMBLOB | LONGBLOB | BOOL | BOOLEAN | SERIAL) # simpleDataType
    | typeName=(BIT | TIME | DATETIME | TIMESTAMP)
      lengthOneDimension?                                         # dimensionDataType
    | typeName=BINARY lengthOneDimension?                         # binaryDataType
    | typeName=(VARBINARY | LONG VARBINARY) lengthOneDimension?   # varbinaryDataType
    | typeName=(ENUM | SET) collectionOptions BINARY?
      (CHARACTER SET charsetName)?
      (COLLATE collationName)?                                    # collectionDataType
    | typeName=(GEOMETRY | POINT | LINESTRING | POLYGON
       | MULTIPOINT | MULTILINESTRING | MULTIPOLYGON
       | GEOMETRYCOLLECTION)                                      # spatialDataType
    | typeName=JSON                                               # jsonDataType
    | typeName=VECTOR (LPAREN decimalLiteral RPAREN)?             # vectorDataType
    | typeName=(INET4 | INET6 | UUID)                            # networkDataType
    ;

convertedDataType
    : typeName=(BINARY | NCHAR | CHAR | VARCHAR) lengthOneDimension? (CHARACTER SET charsetName)?
    | typeName=(DECIMAL | DEC | NUMERIC) lengthTwoOptionalDimension?
    | typeName=(DATE | DATETIME | TIME | TIMESTAMP | YEAR)
    | typeName=(SIGNED | UNSIGNED) INTEGER?
    | typeName=DOUBLE PRECISION?
    | typeName=FLOAT lengthOneDimension?
    | typeName=JSON
    ;

lengthOneDimension
    : LPAREN decimalLiteral RPAREN
    ;

lengthTwoDimension
    : LPAREN decimalLiteral COMMA decimalLiteral RPAREN
    ;

lengthTwoOptionalDimension
    : LPAREN decimalLiteral (COMMA decimalLiteral)? RPAREN
    ;

collectionOptions
    : LPAREN STRING_LITERAL (COMMA STRING_LITERAL)* RPAREN
    ;

// ===========================================================================
// COMMON RULES
// ===========================================================================

// Names and identifiers
uid
    : simpleId
    | DOUBLE_QUOTE_ID
    | REVERSE_QUOTE_ID
    | CHARSET_REVERSE_QOUTE_STRING
    ;

simpleId
    : ID
    | charsetNameBase
    | transactionLevelBase
    | engineName
    | privilegesBase
    | intervalTypeBase
    | dataTypeBase
    | keywordsCanBeId
    | functionNameBase
    ;

fullId
    : uid (DOT uid)?
    ;

uidList
    : uid (COMMA uid)*
    ;

tableName
    : fullId
    ;

fullColumnName
    : uid (dottedId dottedId?)?
    ;

dottedId
    : DOT uid
    ;

tables
    : tableName (COMMA tableName)*
    ;

userName
    : STRING_LITERAL AT_SIGN STRING_LITERAL
    | STRING_LITERAL AT_SIGN uid
    | uid AT_SIGN STRING_LITERAL
    | uid AT_SIGN uid
    | STRING_LITERAL
    | uid
    | CURRENT_USER LPAREN? RPAREN?
    ;

userOrRoleName
    : userName | roleName
    ;

roleName
    : uid (AT_SIGN uid)?
    ;

mysqlVariable
    : LOCAL_ID
    | GLOBAL_ID
    ;

charsetName
    : BINARY
    | charsetNameBase
    | STRING_LITERAL
    | CHARSET_NAME
    ;

collationName
    : uid | STRING_LITERAL
    ;

engineName
    : ARCHIVE | BLACKHOLE | CSV | FEDERATED | INNODB | MEMORY | MRG_MYISAM | MYISAM | NDB | NDBCLUSTER | PERFORMANCE_SCHEMA | TOKUDB | CONNECT | COLUMNSTORE | ARIA | S3
    | uid
    ;

ownerStatement
    : DEFINER EQUAL_SYMBOL (userName | CURRENT_USER LPAREN? RPAREN?)
    ;

// Literals
constant
    : stringLiteral
    | decimalLiteral
    | hexadecimalLiteral
    | boolLiteral
    | REAL_LITERAL
    | BIT_STRING
    | NOT? nullLiteral=(NULL_LITERAL | NULL_SPEC_LITERAL)
    ;

stringLiteral
    : STRING_LITERAL+
    | START_NATIONAL_STRING_LITERAL STRING_LITERAL+
    | STRING_CHARSET_NAME STRING_LITERAL+
    | COLLATE collationName
    ;

decimalLiteral
    : DECIMAL_LITERAL | ZERO_DECIMAL | ONE_DECIMAL | TWO_DECIMAL | REAL_LITERAL
    ;

hexadecimalLiteral
    : HEXADECIMAL_LITERAL
    ;

boolLiteral
    : TRUE | FALSE
    ;

constants
    : constant (COMMA constant)*
    ;

nullNotnull
    : NOT? NULL_LITERAL
    ;

defaultValue
    : NULL_LITERAL
    | CURRENT_TIMESTAMP (LPAREN decimalLiteral? RPAREN)? (ON UPDATE CURRENT_TIMESTAMP (LPAREN decimalLiteral? RPAREN)?)?
    | unaryOperator? constant
    | expression
    ;

fileSizeLiteral
    : FILESIZE_LITERAL | decimalLiteral
    ;

orderByClause
    : ORDER BY orderByExpression (COMMA orderByExpression)*
    ;

orderByExpression
    : expression sortOrder=(ASC | DESC)? (NULLS (FIRST | LAST))?
    ;

simpleStrings
    : STRING_LITERAL (COMMA STRING_LITERAL)*
    ;

// Common keywords that can be used as identifiers
keywordsCanBeId
    : ACCOUNT | ACTION | ADMIN | AFTER | AGAINST | AGGREGATE | ALGORITHM | ALWAYS
    | ANY | AT | AUTOEXTEND_SIZE | AUTO_INCREMENT | AVG | AVG_ROW_LENGTH
    | BACKUP | BEGIN | BINLOG | BIT | BLOCK | BOOL | BOOLEAN | BTREE | BYTE
    | CACHE | CASCADED | CATALOG_NAME | CHAIN | CHANGED | CHANNEL | CHARSET
    | CHECKSUM | CIPHER | CLASS_ORIGIN | CLIENT | CLOSE | COALESCE | CODE
    | COLLATION | COLUMN_FORMAT | COLUMN_NAME | COLUMNS | COMMENT | COMMIT
    | COMMITTED | COMPACT | COMPLETION | COMPRESSED | COMPRESSION | CONCURRENT
    | CONNECTION | CONSISTENT | CONSTRAINT_CATALOG | CONSTRAINT_NAME
    | CONSTRAINT_SCHEMA | CONTAINS | CONTEXT | CONTRIBUTORS | COPY | CPU
    | CURRENT | CURSOR_NAME | CYCLE
    | DATA | DATAFILE | DATE | DATETIME | DAY | DEALLOCATE | DEFAULT_AUTH
    | DEFINER | DELAY_KEY_WRITE | DES_KEY_FILE | DIAGNOSTICS | DIRECTORY
    | DISABLE | DISCARD | DISK | DO | DUMPFILE | DUPLICATE | DYNAMIC
    | ENABLE | ENCRYPTION | END | ENDS | ENGINE | ENGINES | ENUM | ERROR
    | ERRORS | ESCAPE | EVENT | EVENTS | EVERY | EXCHANGE | EXCLUSIVE
    | EXECUTE | EXPANSION | EXPIRE | EXPORT | EXTENDED | EXTENT_SIZE
    | FAST | FAULTS | FIELDS | FILE_BLOCK_SIZE | FILTER | FIRST | FIXED
    | FLUSH | FOLLOWING | FOLLOWS | FORMAT | FOUND | FULL | FUNCTION
    | GENERATED | GET_FORMAT | GLOBAL | GRANTS | GROUP_REPLICATION
    | HANDLER | HARD | HASH | HELP | HOST | HOSTS | HOUR
    | IDENTIFIED | IMMEDIATE | IMPORT | INCREMENT | INDEXES | INITIAL_SIZE
    | INPLACE | INSERT_METHOD | INSTALL | INSTANCE | INSTANT | INVISIBLE | INVOKER
    | IO | IO_THREAD | IPC | ISOLATION | ISSUER | JSON | KEY_BLOCK_SIZE
    | LANGUAGE | LAST | LATERAL | LEAVES | LESS | LEVEL | LIST | LOCAL | LOCKS
    | LOGFILE | LOGS | MASTER | MASTER_AUTO_POSITION | MASTER_BIND
    | MASTER_CONNECT_RETRY | MASTER_DELAY | MASTER_HOST | MASTER_LOG_FILE
    | MASTER_LOG_POS | MASTER_PASSWORD | MASTER_PORT | MASTER_RETRY_COUNT
    | MASTER_SERVER_ID | MASTER_SSL | MASTER_SSL_CA | MASTER_SSL_CAPATH
    | MASTER_SSL_CERT | MASTER_SSL_CIPHER | MASTER_SSL_CRL | MASTER_SSL_CRLPATH
    | MASTER_SSL_KEY | MASTER_TLS_VERSION | MASTER_USER | MAX_CONNECTIONS_PER_HOUR
    | MAX_QUERIES_PER_HOUR | MAX_ROWS | MAX_SIZE | MAX_STATEMENT_TIME
    | MAX_UPDATES_PER_HOUR | MAX_USER_CONNECTIONS | MEDIUM | MEMORY | MERGE
    | MESSAGE_TEXT | MICROSECOND | MIGRATE | MIN_ROWS | MINUTE | MINVALUE | MODE
    | MODIFY | MONTH | MUTEX | MYSQL_ERRNO
    | NAME | NAMES | NATIONAL | NCHAR | NDB | NDBCLUSTER | NEVER | NEXT | NO
    | NOCACHE | NOCYCLE | NODEGROUP | NOMAXVALUE | NOMINVALUE | NONE | NOWAIT
    | NUMBER | NVARCHAR | OF | OFFLINE | OLD_PASSWORD | ONLINE | ONLY | OPEN
    | OPTIMIZER_COSTS | OPTIONS | OWNER
    | PACK_KEYS | PAGE | PARSER | PARTIAL | PARTITIONING | PARTITIONS | PASSWORD
    | PERSISTENT | PHASE | PLUGIN | PLUGINS | PLUGIN_DIR | PORT | PRECEDES
    | PRECEDING | PREPARE | PRESERVE | PREV | PRIVILEGES | PROCESSLIST
    | PROFILE | PROFILES | PROXY
    | QUARTER | QUERY | QUICK
    | REBUILD | RECOVER | REDO_BUFFER_SIZE | REDUNDANT | RELAY | RELAY_LOG_FILE
    | RELAY_LOG_POS | RELAYLOG | RELAY_THREAD | RELOAD | REMOVE | REORGANIZE
    | REPAIR | REPEATABLE | REPLICATION | REPLICAS | REPLICA | RESET | RESTART
    | RESTORE | RESUME | RETURNED_SQLSTATE | RETURNS | ROLE | ROLLBACK | ROLLUP
    | ROTATE | ROUTINE | ROW | ROW_COUNT | ROW_FORMAT | RTREE
    | SAVEPOINT | SCHEDULE | SCHEMA_NAME | SECOND | SECURITY | SEQUENCE
    | SERIAL | SERIALIZABLE | SERVER | SESSION | SHARE | SHARED | SHUTDOWN
    | SIGNED | SIMPLE | SKIP_ | SLAVE | SLAVES | SNAPSHOT | SOCKET | SOFT
    | SOME | SONAME | SOUNDS | SOURCE | SQL_AFTER_GTIDS | SQL_AFTER_MTS_GAPS
    | SQL_BEFORE_GTIDS | SQL_BUFFER_RESULT | SQL_CACHE | SQL_NO_CACHE
    | SQL_THREAD | STACKED | START | STARTS | STATUS | STOP | STORAGE | STORED
    | STRING | SUBJECT | SUBCLASS_ORIGIN | SUBPARTITION | SUBPARTITIONS | SUPER
    | SUSPEND | SWAPS | SWITCHES
    | TABLES | TABLESPACE | TABLE_CHECKSUM | TABLE_NAME | TEMPORARY | TEMPTABLE
    | TEXT | THAN | TIME | TIMESTAMP | TIMESTAMPADD | TIMESTAMPDIFF
    | TRANSACTION | TRANSACTIONAL | TRIGGERS | TRUNCATE | TYPE | TYPES
    | UNBOUNDED | UNCOMMITTED | UNDEFINED | UNDO_BUFFER_SIZE | UNDOFILE
    | UNICODE | UNINSTALL | UNKNOWN | UNTIL | UPGRADE | USER | USER_RESOURCES
    | USE_FRM | VALIDATION | VALUE | VARIABLES | VIEW | VIRTUAL | VISIBLE | WAIT
    | WARNINGS | WEEK | WEIGHT_STRING | WITHOUT | WORK | WRAPPER
    | X509 | XA | XML | YEAR
    // MariaDB specific
    | BODY | ELSIF | GOTO | HISTORY | MINUS | OTHERS | PACKAGE | PERIOD
    | RAISE | ROWNUM | ROWTYPE | SYSDATE | SYSTEM | SYSTEM_TIME | VERSIONING
    | MATERIALIZED | NESTED | ORDINALITY | PATH | QUALIFY
    | CUME_DIST | DENSE_RANK | FIRST_VALUE | LAG | LAST_VALUE | LEAD
    | NTH_VALUE | NTILE | PERCENT_RANK | RANK
    ;

// Common keywords bases
charsetNameBase
    : ARMSCII8 | ASCII_SYM | BIG5 | BINARY | CP1250 | CP1251 | CP1256 | CP1257
    | CP850 | CP852 | CP866 | CP932 | DEC8 | EUCJPMS | EUCKR | GB2312 | GBK
    | GEOSTD8 | GREEK | HEBREW | HP8 | KEYBCS2 | KOI8R | KOI8U | LATIN1
    | LATIN2 | LATIN5 | LATIN7 | MACCE | MACROMAN | SJIS | SWE7 | TIS620
    | UCS2 | UJIS | UTF16 | UTF16LE | UTF32 | UTF8 | UTF8MB3 | UTF8MB4
    ;

transactionLevelBase
    : REPEATABLE | COMMITTED | UNCOMMITTED | SERIALIZABLE
    ;

privilegesBase
    : TABLES | ROUTINE | EXECUTE | FILE | PROCESS | RELOAD | SHUTDOWN
    | SUPER | PRIVILEGES
    ;

intervalTypeBase
    : MICROSECOND | SECOND | MINUTE | HOUR | DAY | WEEK | MONTH | QUARTER
    | YEAR | SECOND_MICROSECOND | MINUTE_MICROSECOND | MINUTE_SECOND
    | HOUR_MICROSECOND | HOUR_SECOND | HOUR_MINUTE | DAY_MICROSECOND
    | DAY_SECOND | DAY_MINUTE | DAY_HOUR | YEAR_MONTH
    ;

dataTypeBase
    : DATE | TIME | TIMESTAMP | DATETIME | YEAR | ENUM | TEXT
    ;

// Utility
ifExists
    : IF EXISTS
    ;

ifNotExists
    : IF NOT EXISTS
    ;

orReplace
    : OR REPLACE
    ;

allOrDistinct
    : ALL | DISTINCT
    ;

// MariaDB specific
encryptedLiteral
    : ENCRYPTED
    ;

intimeAction
    : ONLINE | OFFLINE
    ;

// Additional engine names for MariaDB
ARCHIVE:                             A R C H I V E;
ARIA:                                A R I A;
BLACKHOLE:                           B L A C K H O L E;
COLUMNSTORE:                         C O L U M N S T O R E;
CONNECT:                             C O N N E C T;
CSV:                                 C S V;
FEDERATED:                           F E D E R A T E D;
INNODB:                              I N N O D B;
MRG_MYISAM:                          M R G '_' M Y I S A M;
MYISAM:                              M Y I S A M;
PERFORMANCE_SCHEMA:                  P E R F O R M A N C E '_' S C H E M A;
S3:                                  S '3';
TOKUDB:                              T O K U D B;

// Character set names
ARMSCII8:                            A R M S C I I '8';
BIG5:                                B I G '5';
CP1250:                              C P '1' '2' '5' '0';
CP1251:                              C P '1' '2' '5' '1';
CP1256:                              C P '1' '2' '5' '6';
CP1257:                              C P '1' '2' '5' '7';
CP850:                               C P '8' '5' '0';
CP852:                               C P '8' '5' '2';
CP866:                               C P '8' '6' '6';
CP932:                               C P '9' '3' '2';
DEC8:                                D E C '8';
EUCJPMS:                             E U C J P M S;
EUCKR:                               E U C K R;
GB2312:                              G B '2' '3' '1' '2';
GBK:                                 G B K;
GEOSTD8:                             G E O S T D '8';
GREEK:                               G R E E K;
HEBREW:                              H E B R E W;
HP8:                                 H P '8';
KEYBCS2:                             K E Y B C S '2';
KOI8R:                               K O I '8' R;
KOI8U:                               K O I '8' U;
LATIN1:                              L A T I N '1';
LATIN2:                              L A T I N '2';
LATIN5:                              L A T I N '5';
LATIN7:                              L A T I N '7';
MACCE:                               M A C C E;
MACROMAN:                            M A C R O M A N;
SJIS:                                S J I S;
SWE7:                                S W E '7';
TIS620:                              T I S '6' '2' '0';
UCS2:                                U C S '2';
UJIS:                                U J I S;
UTF16:                               U T F '1' '6';
UTF16LE:                             U T F '1' '6' L E;
UTF32:                               U T F '3' '2';
UTF8:                                U T F '8';
UTF8MB3:                             U T F '8' M B '3';
UTF8MB4:                             U T F '8' M B '4';

// Additional keywords
MYSQL:                               M Y S Q L;
ODBC:                                O D B C;
PAGE_COMPRESSED:                     P A G E '_' C O M P R E S S E D;
PAGE_COMPRESSION_LEVEL:              P A G E '_' C O M P R E S S I O N '_' L E V E L;
ENCRYPTED:                           E N C R Y P T E D;
ENCRYPTION_KEY_ID:                   E N C R Y P T I O N '_' K E Y '_' I D;
ENGINE_ATTRIBUTE:                    E N G I N E '_' A T T R I B U T E;
SECONDARY_ENGINE_ATTRIBUTE:          S E C O N D A R Y '_' E N G I N E '_' A T T R I B U T E;
CLUSTERING:                          C L U S T E R I N G;
YES:                                 Y E S;
IGNORED:                             I G N O R E D;
AUTOINCREMENT:                       A U T O I N C R E M E N T;
VIA:                                 V I A;
FAILED_LOGIN_ATTEMPTS:               F A I L E D '_' L O G I N '_' A T T E M P T S;
PASSWORD_LOCK_TIME:                  P A S S W O R D '_' L O C K '_' T I M E;
UNBOUNDED:                           U N B O U N D E D;
PERSIST:                             P E R S I S T;
PERSIST_ONLY:                        P E R S I S T '_' O N L Y;
ENFORCED:                            E N F O R C E D;
TREE:                                T R E E;
TRADITIONAL:                         T R A D I T I O N A L;
NULL_SPEC_LITERAL:                   '\\' N;
START_NATIONAL_STRING_LITERAL:       N '\'';

// Aggregate functions
GROUP_CONCAT:                        G R O U P '_' C O N C A T;
JSON_ARRAYAGG:                       J S O N '_' A R R A Y A G G;
JSON_OBJECTAGG:                      J S O N '_' O B J E C T A G G;
STD:                                 S T D;
STDDEV:                              S T D D E V;
STDDEV_POP:                          S T D D E V '_' P O P;
STDDEV_SAMP:                         S T D D E V '_' S A M P;
VAR_POP:                             V A R '_' P O P;
VAR_SAMP:                            V A R '_' S A M P;
BIT_AND:                             B I T '_' A N D;
BIT_OR:                              B I T '_' O R;
BIT_XOR:                             B I T '_' X O R;

// More function names
ADDDATE:                             A D D D A T E;
ADDTIME:                             A D D T I M E;
AES_DECRYPT:                         A E S '_' D E C R Y P T;
AES_ENCRYPT:                         A E S '_' E N C R Y P T;
BENCHMARK:                           B E N C H M A R K;
BIN:                                 B I N;
BIT_COUNT:                           B I T '_' C O U N T;
BIT_LENGTH:                          B I T '_' L E N G T H;
CHAR_LENGTH:                         C H A R '_' L E N G T H;
CHARACTER_LENGTH:                    C H A R A C T E R '_' L E N G T H;
COERCIBILITY:                        C O E R C I B I L I T Y;
CONCAT:                              C O N C A T;
CONCAT_WS:                           C O N C A T '_' W S;
CONNECTION_ID:                       C O N N E C T I O N '_' I D;
CONVERT_TZ:                          C O N V E R T '_' T Z;
CURDATE:                             C U R D A T E;
CURTIME:                             C U R T I M E;
DATE_FORMAT:                         D A T E '_' F O R M A T;
DATE_ADD:                            D A T E '_' A D D;
DATE_SUB:                            D A T E '_' S U B;
DATEDIFF:                            D A T E D I F F;
DAYNAME:                             D A Y N A M E;
DAYOFMONTH:                          D A Y O F M O N T H;
DAYOFWEEK:                           D A Y O F W E E K;
DAYOFYEAR:                           D A Y O F Y E A R;
DECODE:                              D E C O D E;
DES_DECRYPT:                         D E S '_' D E C R Y P T;
DES_ENCRYPT:                         D E S '_' E N C R Y P T;
ELT:                                 E L T;
ENCODE:                              E N C O D E;
EXPORT_SET:                          E X P O R T '_' S E T;
EXTRACTVALUE:                        E X T R A C T V A L U E;
FIND_IN_SET:                         F I N D '_' I N '_' S E T;
FOUND_ROWS:                          F O U N D '_' R O W S;
FROM_BASE64:                         F R O M '_' B A S E '6' '4';
FROM_DAYS:                           F R O M '_' D A Y S;
FROM_UNIXTIME:                       F R O M '_' U N I X T I M E;
GEOMCOLLECTION:                      G E O M C O L L E C T I O N;
GET_LOCK:                            G E T '_' L O C K;
GREATEST:                            G R E A T E S T;
GTID_SUBSET:                         G T I D '_' S U B S E T;
GTID_SUBTRACT:                       G T I D '_' S U B T R A C T;
INET_ATON:                           I N E T '_' A T O N;
INET_NTOA:                           I N E T '_' N T O A;
INET6_ATON:                          I N E T '6' '_' A T O N;
INET6_NTOA:                          I N E T '6' '_' N T O A;
INSTR:                               I N S T R;
IS_FREE_LOCK:                        I S '_' F R E E '_' L O C K;
IS_IPV4:                             I S '_' I P V '4';
IS_IPV4_COMPAT:                      I S '_' I P V '4' '_' C O M P A T;
IS_IPV4_MAPPED:                      I S '_' I P V '4' '_' M A P P E D;
IS_IPV6:                             I S '_' I P V '6';
IS_USED_LOCK:                        I S '_' U S E D '_' L O C K;
LAST_DAY:                            L A S T '_' D A Y;
LAST_INSERT_ID:                      L A S T '_' I N S E R T '_' I D;
LCASE:                               L C A S E;
LEAST:                               L E A S T;
LENGTH:                              L E N G T H;
LOAD_FILE:                           L O A D '_' F I L E;
LOCATE:                              L O C A T E;
LOWER:                               L O W E R;
LPAD:                                L P A D;
LTRIM:                               L T R I M;
MAKE_SET:                            M A K E '_' S E T;
MAKEDATE:                            M A K E D A T E;
MAKETIME:                            M A K E T I M E;
MASTER_POS_WAIT:                     M A S T E R '_' P O S '_' W A I T;
MD5:                                 M D '5';
MID:                                 M I D;
MONTHNAME:                           M O N T H N A M E;
NAME_CONST:                          N A M E '_' C O N S T;
NOW:                                 N O W;
OCT:                                 O C T;
OCTET_LENGTH:                        O C T E T '_' L E N G T H;
ORD:                                 O R D;
PERIOD_ADD:                          P E R I O D '_' A D D;
PERIOD_DIFF:                         P E R I O D '_' D I F F;
PI:                                  P I;
POSITION:                            P O S I T I O N;
QUOTE:                               Q U O T E;
RANDOM_BYTES:                        R A N D O M '_' B Y T E S;
RELEASE_ALL_LOCKS:                   R E L E A S E '_' A L L '_' L O C K S;
RELEASE_LOCK:                        R E L E A S E '_' L O C K;
ROW_COUNT:                           R O W '_' C O U N T;
RPAD:                                R P A D;
RTRIM:                               R T R I M;
SEC_TO_TIME:                         S E C '_' T O '_' T I M E;
SESSION_USER:                        S E S S I O N '_' U S E R;
SHA:                                 S H A;
SHA1:                                S H A '1';
SHA2:                                S H A '2';
SLEEP:                               S L E E P;
SOUNDEX:                             S O U N D E X;
SQL_THREAD_WAIT_AFTER_GTIDS:         S Q L '_' T H R E A D '_' W A I T '_' A F T E R '_' G T I D S;
STR_TO_DATE:                         S T R '_' T O '_' D A T E;
STRCMP:                              S T R C M P;
SUBDATE:                             S U B D A T E;
SUBSTRING_INDEX:                     S U B S T R I N G '_' I N D E X;
SUBTIME:                             S U B T I M E;
SYSTEM_USER:                         S Y S T E M '_' U S E R;
TIMEDIFF:                            T I M E D I F F;
TIME_FORMAT:                         T I M E '_' F O R M A T;
TIME_TO_SEC:                         T I M E '_' T O '_' S E C;
TO_BASE64:                           T O '_' B A S E '6' '4';
TO_DAYS:                             T O '_' D A Y S;
TO_SECONDS:                          T O '_' S E C O N D S;
UCASE:                               U C A S E;
UNCOMPRESS:                          U N C O M P R E S S;
UNCOMPRESSED_LENGTH:                 U N C O M P R E S S E D '_' L E N G T H;
UNHEX:                               U N H E X;
UNIX_TIMESTAMP:                      U N I X '_' T I M E S T A M P;
UPDATEXML:                           U P D A T E X M L;
UPPER:                               U P P E R;
UUID_SHORT:                          U U I D '_' S H O R T;
VALIDATE_PASSWORD_STRENGTH:          V A L I D A T E '_' P A S S W O R D '_' S T R E N G T H;
VERSION:                             V E R S I O N;
WAIT_FOR_EXECUTED_GTID_SET:          W A I T '_' F O R '_' E X E C U T E D '_' G T I D '_' S E T;
WAIT_UNTIL_SQL_THREAD_AFTER_GTIDS:   W A I T '_' U N T I L '_' S Q L '_' T H R E A D '_' A F T E R '_' G T I D S;
WEEKDAY:                             W E E K D A Y;
WEEKOFYEAR:                          W E E K O F Y E A R;
YEARWEEK:                            Y E A R W E E K;
ABS:                                 A B S;
ACOS:                                A C O S;
ASIN:                                A S I N;
ATAN:                                A T A N;
ATAN2:                               A T A N '2';
CEIL:                                C E I L;
CEILING:                             C E I L I N G;
CONV:                                C O N V;
COS:                                 C O S;
COT:                                 C O T;
CRC32:                               C R C '3' '2';
DEGREES:                             D E G R E E S;
EXP:                                 E X P;
FLOOR:                               F L O O R;
HEX:                                 H E X;
LN:                                  L N;
LOG:                                 L O G;
LOG10:                               L O G '1' '0';
LOG2:                                L O G '2';
POW:                                 P O W;
RADIANS:                             R A D I A N S;
RAND:                                R A N D;
SIGN:                                S I G N;
SIN:                                 S I N;
SQRT:                                S Q R T;
TAN:                                 T A N;
SUBSTR:                              S U B S T R;
COMPRESS:                            C O M P R E S S;
ENCRYPT:                             E N C R Y P T;
POWER:                               P O W E R;

// Privilege keywords
BINLOG_MONITOR:                      B I N L O G ' ' M O N I T O R;
CONNECTION_ADMIN:                    C O N N E C T I O N ' ' A D M I N;
FEDERATED_ADMIN:                     F E D E R A T E D ' ' A D M I N;
READ_ONLY:                           R E A D '_' O N L Y;
SET_USER:                            S E T ' ' U S E R;
SLAVE_MONITOR:                       S L A V E ' ' M O N I T O R;
AUTOCOMMIT:                          A U T O C O M M I T;
