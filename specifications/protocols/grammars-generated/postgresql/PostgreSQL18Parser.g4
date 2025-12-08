/*
 * PostgreSQL 18 Parser Grammar for ANTLR4
 * 
 * This grammar covers PostgreSQL 18 (released September 2025) SQL syntax.
 * Based on the official PostgreSQL documentation and SQL:2023 standard.
 * 
 * New in PostgreSQL 18:
 * - Virtual generated columns (VIRTUAL keyword, now default)
 * - UUIDv7 function support
 * - OLD/NEW aliases in RETURNING clauses
 * - Temporal constraints (WITHOUT OVERLAPS, PERIOD)
 * - Enhanced COPY options (REJECT_LIMIT, LOG_VERBOSITY)
 * - NOT VALID for NOT NULL constraints
 * - ENFORCED/NOT ENFORCED for CHECK constraints
 * - CREATE FOREIGN TABLE ... LIKE syntax
 * - Skip scan optimization hints
 * 
 * License: MIT
 * Copyright (c) 2025
 */

parser grammar PostgreSQL18Parser;

options {
    tokenVocab = PostgreSQL18Lexer;
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
    : ddlStatement
    | dmlStatement
    | dclStatement
    | transactionStatement
    | sessionStatement
    | utilityStatement
    | plpgsqlStatement
    | SEMICOLON
    ;

// -----------------------------------------------------------------------------
// DDL STATEMENTS (Data Definition Language)
// -----------------------------------------------------------------------------

ddlStatement
    : createStatement
    | alterStatement
    | dropStatement
    | truncateStatement
    | commentStatement
    | securityLabelStatement
    ;

createStatement
    : createDatabaseStatement
    | createSchemaStatement
    | createTableStatement
    | createTableAsStatement
    | createForeignTableStatement
    | createViewStatement
    | createMaterializedViewStatement
    | createIndexStatement
    | createSequenceStatement
    | createTypeStatement
    | createDomainStatement
    | createFunctionStatement
    | createProcedureStatement
    | createTriggerStatement
    | createRuleStatement
    | createPolicyStatement
    | createRoleStatement
    | createExtensionStatement
    | createPublicationStatement
    | createSubscriptionStatement
    | createServerStatement
    | createForeignDataWrapperStatement
    | createUserMappingStatement
    | createEventTriggerStatement
    | createTextSearchStatement
    | createOperatorStatement
    | createAggregateStatement
    | createCastStatement
    | createCollationStatement
    | createConversionStatement
    | createLanguageStatement
    | createTablespaceStatement
    | createAccessMethodStatement
    | createStatisticsStatement
    | createTransformStatement
    ;

// CREATE DATABASE
createDatabaseStatement
    : CREATE DATABASE ifNotExists? identifier
      createDatabaseOption*
    ;

createDatabaseOption
    : WITH? (
        OWNER EQUALS? identifier
      | TEMPLATE EQUALS? identifier
      | ENCODING EQUALS? stringLiteral
      | LOCALE EQUALS? stringLiteral
      | LC_COLLATE EQUALS? stringLiteral
      | LC_CTYPE EQUALS? stringLiteral
      | ICU_LOCALE EQUALS? stringLiteral
      | ICU_RULES EQUALS? stringLiteral
      | LOCALE_PROVIDER EQUALS? identifier
      | COLLATION_VERSION EQUALS? stringLiteral
      | TABLESPACE EQUALS? identifier
      | ALLOW_CONNECTIONS EQUALS? booleanLiteral
      | CONNECTION LIMIT EQUALS? signedInteger
      | IS_TEMPLATE EQUALS? booleanLiteral
      | OID EQUALS? INTEGER_LITERAL
      | STRATEGY EQUALS? identifier
      | BUILTIN_LOCALE EQUALS? stringLiteral
    )
    ;

// CREATE SCHEMA
createSchemaStatement
    : CREATE SCHEMA ifNotExists? schemaNameClause
      (AUTHORIZATION roleSpecification)?
      schemaElement*
    ;

schemaNameClause
    : identifier
    | AUTHORIZATION roleSpecification
    ;

schemaElement
    : createTableStatement
    | createViewStatement
    | createIndexStatement
    | createSequenceStatement
    | createTriggerStatement
    | grantStatement
    ;

// CREATE TABLE
createTableStatement
    : CREATE createTableType? TABLE ifNotExists? tableName
      (OPEN_PAREN tableElementList CLOSE_PAREN)?
      inheritClause?
      partitionByClause?
      usingAccessMethod?
      withStorageParameters?
      onCommitAction?
      tableSpaceClause?
    | CREATE createTableType? TABLE ifNotExists? tableName
      OF typeName
      (OPEN_PAREN typedTableElementList CLOSE_PAREN)?
      partitionByClause?
      usingAccessMethod?
      withStorageParameters?
      onCommitAction?
      tableSpaceClause?
    | CREATE createTableType? TABLE ifNotExists? tableName
      PARTITION OF qualifiedName
      (OPEN_PAREN typedTableElementList CLOSE_PAREN)?
      partitionBoundSpec
      partitionByClause?
      usingAccessMethod?
      withStorageParameters?
      onCommitAction?
      tableSpaceClause?
    ;

createTableType
    : GLOBAL? (TEMPORARY | TEMP)
    | LOCAL? (TEMPORARY | TEMP)
    | UNLOGGED
    ;

tableElementList
    : tableElement (COMMA tableElement)*
    ;

tableElement
    : columnDefinition
    | tableConstraint
    | tableLikeClause
    ;

columnDefinition
    : identifier dataType? collateClause? columnConstraint*
    ;

columnConstraint
    : constraintName? columnConstraintElement constraintAttributes?
    ;

constraintName
    : CONSTRAINT identifier
    ;

columnConstraintElement
    : NOT NULL notValidClause?                              // PostgreSQL 18: NOT VALID support
    | NULL
    | UNIQUE nullsTreatment? indexParameters?
    | PRIMARY KEY indexParameters?
    | CHECK OPEN_PAREN expression CLOSE_PAREN noInheritClause? enforcedClause?  // PostgreSQL 18: ENFORCED
    | REFERENCES tableName columnNameList? referencesMatch? referentialActions?
    | GENERATED generatedWhen AS (
        IDENTITY identityOptions?
      | OPEN_PAREN expression CLOSE_PAREN generatedStorageType?  // PostgreSQL 18: VIRTUAL default
      )
    | DEFAULT expression
    | COLLATE identifier
    | compressionMethod
    | storageType
    ;

// PostgreSQL 18: Virtual generated columns (VIRTUAL is now default)
generatedStorageType
    : STORED
    | VIRTUAL
    ;

generatedWhen
    : ALWAYS
    | BY DEFAULT
    ;

identityOptions
    : OPEN_PAREN sequenceOption* CLOSE_PAREN
    ;

// PostgreSQL 18: ENFORCED/NOT ENFORCED for CHECK constraints
enforcedClause
    : ENFORCED
    | NOT ENFORCED
    ;

// PostgreSQL 18: NOT VALID for NOT NULL constraints
notValidClause
    : NOT VALID
    ;

noInheritClause
    : NO INHERIT
    ;

nullsTreatment
    : NULLS NOT? DISTINCT
    ;

constraintAttributes
    : constraintDeferrable constraintInitiallyDeferred?
    | constraintInitiallyDeferred constraintDeferrable?
    ;

constraintDeferrable
    : NOT? DEFERRABLE
    ;

constraintInitiallyDeferred
    : INITIALLY (DEFERRED | IMMEDIATE)
    ;

tableConstraint
    : constraintName? tableConstraintElement constraintAttributes?
    ;

tableConstraintElement
    : UNIQUE nullsTreatment? OPEN_PAREN columnNameListWithOverlaps CLOSE_PAREN indexParameters?  // PostgreSQL 18: WITHOUT OVERLAPS
    | PRIMARY KEY OPEN_PAREN columnNameListWithOverlaps CLOSE_PAREN indexParameters?            // PostgreSQL 18: WITHOUT OVERLAPS
    | CHECK OPEN_PAREN expression CLOSE_PAREN noInheritClause? enforcedClause?
    | FOREIGN KEY OPEN_PAREN columnNameList CLOSE_PAREN
      REFERENCES tableName columnNameList? referencesMatch? referentialActions?
      foreignKeyPeriod?                                    // PostgreSQL 18: PERIOD for temporal FK
    | EXCLUDE usingIndexMethod?
      OPEN_PAREN excludeElement (COMMA excludeElement)* CLOSE_PAREN
      indexParameters? excludeWhereClause?
    ;

// PostgreSQL 18: Temporal constraints with WITHOUT OVERLAPS
columnNameListWithOverlaps
    : columnNameWithOverlaps (COMMA columnNameWithOverlaps)*
    ;

columnNameWithOverlaps
    : identifier (WITHOUT OVERLAPS)?
    ;

// PostgreSQL 18: PERIOD clause for temporal foreign keys
foreignKeyPeriod
    : PERIOD identifier
    ;

referencesMatch
    : MATCH (FULL | PARTIAL | SIMPLE)
    ;

referentialActions
    : onDeleteAction onUpdateAction?
    | onUpdateAction onDeleteAction?
    ;

onDeleteAction
    : ON DELETE referentialAction
    ;

onUpdateAction
    : ON UPDATE referentialAction
    ;

referentialAction
    : NO ACTION
    | RESTRICT
    | CASCADE
    | SET NULL columnNameList?
    | SET DEFAULT columnNameList?
    ;

excludeElement
    : (identifier | OPEN_PAREN expression CLOSE_PAREN) excludeOperatorClass? (ASC | DESC)? (NULLS (FIRST | LAST))? WITH excludeOperator
    ;

excludeOperatorClass
    : identifier
    ;

excludeOperator
    : EQUALS
    | NOT_EQUALS
    | LESS_THAN
    | GREATER_THAN
    | LESS_THAN_OR_EQUALS
    | GREATER_THAN_OR_EQUALS
    | AMPERSAND AMPERSAND     // overlap
    ;

excludeWhereClause
    : WHERE OPEN_PAREN expression CLOSE_PAREN
    ;

tableLikeClause
    : LIKE tableName likeOption*
    ;

likeOption
    : (INCLUDING | EXCLUDING) (
        COMMENTS
      | COMPRESSION
      | CONSTRAINTS
      | DEFAULTS
      | GENERATED
      | IDENTITY
      | INDEXES
      | STATISTICS
      | STORAGE
      | ALL
      )
    ;

inheritClause
    : INHERITS OPEN_PAREN tableNameList CLOSE_PAREN
    ;

partitionByClause
    : PARTITION BY partitionStrategy OPEN_PAREN partitionKeyList CLOSE_PAREN
    ;

partitionStrategy
    : RANGE
    | LIST
    | HASH
    ;

partitionKeyList
    : partitionKey (COMMA partitionKey)*
    ;

partitionKey
    : identifier
    | OPEN_PAREN expression CLOSE_PAREN
    ;

partitionBoundSpec
    : FOR VALUES partitionBoundSpecValue
    | DEFAULT
    ;

partitionBoundSpecValue
    : IN OPEN_PAREN expressionList CLOSE_PAREN
    | FROM OPEN_PAREN partitionBoundList CLOSE_PAREN TO OPEN_PAREN partitionBoundList CLOSE_PAREN
    | WITH OPEN_PAREN MODULUS INTEGER_LITERAL COMMA REMAINDER INTEGER_LITERAL CLOSE_PAREN
    ;

partitionBoundList
    : partitionBoundValue (COMMA partitionBoundValue)*
    ;

partitionBoundValue
    : literal
    | MINVALUE
    | MAXVALUE
    ;

usingAccessMethod
    : USING identifier
    ;

withStorageParameters
    : WITH OPEN_PAREN storageParameter (COMMA storageParameter)* CLOSE_PAREN
    | WITHOUT OIDS
    ;

storageParameter
    : identifier (EQUALS (literal | identifier))?
    | identifier DOT identifier (EQUALS (literal | identifier))?
    ;

onCommitAction
    : ON COMMIT (PRESERVE ROWS | DELETE ROWS | DROP)
    ;

tableSpaceClause
    : TABLESPACE identifier
    ;

typedTableElementList
    : typedTableElement (COMMA typedTableElement)*
    ;

typedTableElement
    : identifier WITH OPTIONS columnConstraint*
    | tableConstraint
    ;

// CREATE TABLE AS
createTableAsStatement
    : CREATE createTableType? TABLE ifNotExists? tableName
      columnNameList?
      usingAccessMethod?
      withStorageParameters?
      onCommitAction?
      tableSpaceClause?
      AS selectStatement
      withDataClause?
    ;

withDataClause
    : WITH NO? DATA
    ;

// CREATE FOREIGN TABLE (PostgreSQL 18: LIKE support)
createForeignTableStatement
    : CREATE FOREIGN TABLE ifNotExists? tableName
      OPEN_PAREN (foreignTableElement (COMMA foreignTableElement)*)? CLOSE_PAREN
      inheritClause?
      SERVER identifier
      foreignTableOptions?
    | CREATE FOREIGN TABLE ifNotExists? tableName
      PARTITION OF tableName
      (OPEN_PAREN foreignTableElement (COMMA foreignTableElement)* CLOSE_PAREN)?
      partitionBoundSpec
      SERVER identifier
      foreignTableOptions?
    | CREATE FOREIGN TABLE ifNotExists? tableName   // PostgreSQL 18: LIKE support
      LIKE tableName likeOption*
      SERVER identifier
      foreignTableOptions?
    ;

foreignTableElement
    : columnDefinition
    | tableConstraint
    | tableLikeClause
    ;

foreignTableOptions
    : OPTIONS OPEN_PAREN foreignTableOption (COMMA foreignTableOption)* CLOSE_PAREN
    ;

foreignTableOption
    : identifier stringLiteral
    ;

// CREATE VIEW
createViewStatement
    : CREATE (OR REPLACE)? (TEMP | TEMPORARY)? RECURSIVE? VIEW ifNotExists? viewName
      columnNameList?
      withViewOptions?
      AS selectStatement
      withCheckOption?
    ;

withViewOptions
    : WITH OPEN_PAREN viewOption (COMMA viewOption)* CLOSE_PAREN
    ;

viewOption
    : identifier (EQUALS literal)?
    ;

withCheckOption
    : WITH (CASCADED | LOCAL)? CHECK OPTION
    ;

// CREATE MATERIALIZED VIEW
createMaterializedViewStatement
    : CREATE UNLOGGED? MATERIALIZED VIEW ifNotExists? viewName
      columnNameList?
      usingAccessMethod?
      withStorageParameters?
      tableSpaceClause?
      AS selectStatement
      withDataClause?
    ;

// CREATE INDEX
createIndexStatement
    : CREATE UNIQUE? INDEX CONCURRENTLY? ifNotExists? indexName?
      ON ONLY? tableName
      usingIndexMethod?
      OPEN_PAREN indexColumnList CLOSE_PAREN
      includeClause?
      nullsDistinctClause?
      withStorageParameters?
      tableSpaceClause?
      indexWhereClause?
    ;

usingIndexMethod
    : USING identifier
    ;

indexColumnList
    : indexColumn (COMMA indexColumn)*
    ;

indexColumn
    : (identifier | OPEN_PAREN expression CLOSE_PAREN) operatorClass? (ASC | DESC)? (NULLS (FIRST | LAST))?
    ;

operatorClass
    : identifier
    | identifier OPEN_PAREN operatorClassParameter (COMMA operatorClassParameter)* CLOSE_PAREN
    ;

operatorClassParameter
    : identifier EQUALS literal
    ;

includeClause
    : INCLUDE OPEN_PAREN columnNameList CLOSE_PAREN
    ;

nullsDistinctClause
    : NULLS NOT? DISTINCT
    ;

indexWhereClause
    : WHERE OPEN_PAREN expression CLOSE_PAREN
    ;

indexParameters
    : USING INDEX (identifier | indexParametersList)?
    | indexParametersList
    ;

indexParametersList
    : (usingIndexMethod | withStorageParameters | tableSpaceClause)+
    ;

// CREATE SEQUENCE
createSequenceStatement
    : CREATE (TEMP | TEMPORARY)? SEQUENCE ifNotExists? sequenceName
      sequenceOption*
    ;

sequenceOption
    : AS dataType
    | INCREMENT BY? signedInteger
    | MINVALUE signedInteger
    | NO MINVALUE
    | MAXVALUE signedInteger
    | NO MAXVALUE
    | START WITH? signedInteger
    | CACHE INTEGER_LITERAL
    | NO? CYCLE
    | OWNED BY (tableName DOT identifier | NONE)
    ;

// CREATE TYPE
createTypeStatement
    : CREATE TYPE qualifiedName AS (
        ENUM OPEN_PAREN stringLiteralList CLOSE_PAREN
      | RANGE OPEN_PAREN rangeTypeOption (COMMA rangeTypeOption)* CLOSE_PAREN
      | OPEN_PAREN typedColumnList CLOSE_PAREN
      )
    | CREATE TYPE qualifiedName (OPEN_PAREN typeOption (COMMA typeOption)* CLOSE_PAREN)?
    ;

typedColumnList
    : typedColumn (COMMA typedColumn)*
    ;

typedColumn
    : identifier dataType collateClause?
    ;

typeOption
    : INPUT EQUALS identifier
    | OUTPUT EQUALS identifier
    | RECEIVE EQUALS identifier
    | SEND EQUALS identifier
    | TYPMOD_IN EQUALS identifier
    | TYPMOD_OUT EQUALS identifier
    | ANALYZE EQUALS identifier
    | SUBSCRIPT EQUALS identifier
    | INTERNALLENGTH EQUALS (signedInteger | VARIABLE)
    | PASSEDBYVALUE
    | ALIGNMENT EQUALS identifier
    | STORAGE EQUALS identifier
    | LIKE EQUALS identifier
    | CATEGORY EQUALS stringLiteral
    | PREFERRED EQUALS booleanLiteral
    | DEFAULT EQUALS stringLiteral
    | ELEMENT EQUALS identifier
    | DELIMITER EQUALS stringLiteral
    | COLLATABLE EQUALS booleanLiteral
    ;

rangeTypeOption
    : SUBTYPE EQUALS dataType
    | SUBTYPE_OPCLASS EQUALS identifier
    | COLLATION EQUALS identifier
    | CANONICAL EQUALS identifier
    | SUBTYPE_DIFF EQUALS identifier
    | MULTIRANGE_TYPE_NAME EQUALS identifier
    ;

// CREATE DOMAIN
createDomainStatement
    : CREATE DOMAIN qualifiedName AS? dataType
      collateClause?
      domainConstraint*
    ;

domainConstraint
    : constraintName? domainConstraintElement
    ;

domainConstraintElement
    : NOT NULL notValidClause?
    | NULL
    | CHECK OPEN_PAREN expression CLOSE_PAREN noInheritClause?
    | DEFAULT expression
    | COLLATE identifier
    ;

// CREATE FUNCTION
createFunctionStatement
    : CREATE (OR REPLACE)? FUNCTION qualifiedName
      OPEN_PAREN functionArgsList? CLOSE_PAREN
      (RETURNS returnType)?
      functionAttribute*
    ;

functionArgsList
    : functionArg (COMMA functionArg)*
    ;

functionArg
    : argMode? identifier? dataType (DEFAULT expression)?
    | argMode? identifier? dataType EQUALS expression
    ;

argMode
    : IN
    | OUT
    | INOUT
    | VARIADIC
    ;

returnType
    : dataType
    | TABLE OPEN_PAREN typedColumnList CLOSE_PAREN
    | SETOF dataType
    ;

functionAttribute
    : LANGUAGE identifier
    | TRANSFORM transformList
    | WINDOW
    | IMMUTABLE
    | STABLE
    | VOLATILE
    | NOT? LEAKPROOF
    | CALLED ON NULL INPUT
    | RETURNS NULL ON NULL INPUT
    | STRICT
    | EXTERNAL? SECURITY (INVOKER | DEFINER)
    | PARALLEL (UNSAFE | RESTRICTED | SAFE)
    | COST numericLiteral
    | ROWS numericLiteral
    | SUPPORT identifier
    | SET identifier (TO | EQUALS) (literal | identifier | DEFAULT)
    | AS functionBody
    | RETURN expression
    | BEGIN ATOMIC statementList END
    ;

transformList
    : FOR TYPE dataType (COMMA FOR TYPE dataType)*
    ;

functionBody
    : stringLiteral (COMMA stringLiteral)?
    ;

// CREATE PROCEDURE
createProcedureStatement
    : CREATE (OR REPLACE)? PROCEDURE qualifiedName
      OPEN_PAREN functionArgsList? CLOSE_PAREN
      procedureAttribute*
    ;

procedureAttribute
    : LANGUAGE identifier
    | TRANSFORM transformList
    | EXTERNAL? SECURITY (INVOKER | DEFINER)
    | SET identifier (TO | EQUALS) (literal | identifier | DEFAULT)
    | AS functionBody
    | BEGIN ATOMIC statementList END
    ;

// CREATE TRIGGER
createTriggerStatement
    : CREATE (OR REPLACE)? (CONSTRAINT)? TRIGGER identifier
      triggerTiming triggerEvent (OR triggerEvent)*
      ON tableName
      referencingClause?
      triggerForSpec?
      triggerWhen?
      EXECUTE (FUNCTION | PROCEDURE) functionCall
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
    | TRUNCATE
    ;

referencingClause
    : REFERENCING referencingElement+
    ;

referencingElement
    : (OLD | NEW) TABLE AS? identifier
    ;

triggerForSpec
    : FOR EACH? (ROW | STATEMENT)
    ;

triggerWhen
    : WHEN OPEN_PAREN expression CLOSE_PAREN
    ;

// CREATE RULE
createRuleStatement
    : CREATE (OR REPLACE)? RULE identifier
      AS ON ruleEvent TO tableName
      ruleWhere?
      DO INSTEAD? ruleAction
    ;

ruleEvent
    : SELECT
    | INSERT
    | UPDATE
    | DELETE
    ;

ruleWhere
    : WHERE expression
    ;

ruleAction
    : NOTHING
    | statement
    | OPEN_PAREN statement (SEMICOLON statement)* SEMICOLON? CLOSE_PAREN
    ;

// CREATE POLICY
createPolicyStatement
    : CREATE POLICY identifier ON tableName
      policyForClause?
      policyToClause?
      policyUsingClause?
      policyWithCheckClause?
    ;

policyForClause
    : FOR (ALL | SELECT | INSERT | UPDATE | DELETE)
    ;

policyToClause
    : TO roleSpecificationList
    ;

policyUsingClause
    : USING OPEN_PAREN expression CLOSE_PAREN
    ;

policyWithCheckClause
    : WITH CHECK OPEN_PAREN expression CLOSE_PAREN
    ;

// CREATE ROLE
createRoleStatement
    : CREATE (ROLE | USER | GROUP) identifier
      (WITH? roleOption*)?
    ;

roleOption
    : SUPERUSER
    | NOSUPERUSER
    | CREATEDB
    | NOCREATEDB
    | CREATEROLE
    | NOCREATEROLE
    | INHERIT
    | NOINHERIT
    | LOGIN
    | NOLOGIN
    | REPLICATION
    | NOREPLICATION
    | BYPASSRLS
    | NOBYPASSRLS
    | CONNECTION LIMIT signedInteger
    | ENCRYPTED? PASSWORD (stringLiteral | NULL)
    | VALID UNTIL stringLiteral
    | IN (ROLE | GROUP) roleSpecificationList
    | (ROLE | USER | GROUP) roleSpecificationList
    | ADMIN roleSpecificationList
    | SYSID INTEGER_LITERAL
    ;

// CREATE EXTENSION
createExtensionStatement
    : CREATE EXTENSION ifNotExists? identifier
      (WITH? extensionOption*)?
    ;

extensionOption
    : SCHEMA identifier
    | VERSION stringLiteral
    | CASCADE
    ;

// CREATE PUBLICATION
createPublicationStatement
    : CREATE PUBLICATION identifier
      publicationForClause?
      publicationParameters?
    ;

publicationForClause
    : FOR ALL TABLES
    | FOR publicationTableList
    | FOR TABLES IN SCHEMA schemaNameList
    ;

publicationTableList
    : TABLE publicationTable (COMMA publicationTable)*
    ;

publicationTable
    : ONLY? tableName (ASTERISK)?
      (OPEN_PAREN columnNameList CLOSE_PAREN)?
      (WHERE OPEN_PAREN expression CLOSE_PAREN)?
    ;

publicationParameters
    : WITH OPEN_PAREN publicationParameter (COMMA publicationParameter)* CLOSE_PAREN
    ;

publicationParameter
    : identifier EQUALS literal
    ;

// CREATE SUBSCRIPTION
createSubscriptionStatement
    : CREATE SUBSCRIPTION identifier
      CONNECTION stringLiteral
      PUBLICATION identifierList
      (WITH OPEN_PAREN subscriptionParameter (COMMA subscriptionParameter)* CLOSE_PAREN)?
    ;

subscriptionParameter
    : identifier EQUALS literal
    ;

// CREATE SERVER
createServerStatement
    : CREATE SERVER ifNotExists? identifier
      (TYPE stringLiteral)?
      (VERSION stringLiteral)?
      FOREIGN DATA WRAPPER identifier
      (OPTIONS OPEN_PAREN serverOption (COMMA serverOption)* CLOSE_PAREN)?
    ;

serverOption
    : identifier stringLiteral
    ;

// CREATE FOREIGN DATA WRAPPER
createForeignDataWrapperStatement
    : CREATE FOREIGN DATA WRAPPER identifier
      (HANDLER identifier | NO HANDLER)?
      (VALIDATOR identifier | NO VALIDATOR)?
      (OPTIONS OPEN_PAREN foreignDataWrapperOption (COMMA foreignDataWrapperOption)* CLOSE_PAREN)?
    ;

foreignDataWrapperOption
    : identifier stringLiteral
    ;

// CREATE USER MAPPING
createUserMappingStatement
    : CREATE USER MAPPING ifNotExists? FOR (identifier | USER | CURRENT_ROLE | CURRENT_USER | PUBLIC)
      SERVER identifier
      (OPTIONS OPEN_PAREN userMappingOption (COMMA userMappingOption)* CLOSE_PAREN)?
    ;

userMappingOption
    : identifier stringLiteral
    ;

// CREATE EVENT TRIGGER
createEventTriggerStatement
    : CREATE EVENT TRIGGER identifier
      ON identifier
      (WHEN filterVariable (AND filterVariable)*)?
      EXECUTE (FUNCTION | PROCEDURE) functionCall
    ;

filterVariable
    : identifier IN OPEN_PAREN stringLiteralList CLOSE_PAREN
    ;

// CREATE TEXT SEARCH
createTextSearchStatement
    : CREATE TEXT SEARCH (
        CONFIGURATION qualifiedName (OPEN_PAREN textSearchOption (COMMA textSearchOption)* CLOSE_PAREN)?
      | DICTIONARY qualifiedName OPEN_PAREN textSearchOption (COMMA textSearchOption)* CLOSE_PAREN
      | PARSER qualifiedName OPEN_PAREN textSearchOption (COMMA textSearchOption)* CLOSE_PAREN
      | TEMPLATE qualifiedName OPEN_PAREN textSearchOption (COMMA textSearchOption)* CLOSE_PAREN
      )
    ;

textSearchOption
    : identifier EQUALS literal
    ;

// CREATE OPERATOR
createOperatorStatement
    : CREATE OPERATOR operatorName
      OPEN_PAREN operatorOption (COMMA operatorOption)* CLOSE_PAREN
    ;

operatorName
    : identifier
    | OPERATOR OPEN_PAREN qualifiedOperatorName CLOSE_PAREN
    ;

qualifiedOperatorName
    : identifier? DOT? (
        EQUALS
      | NOT_EQUALS
      | LESS_THAN
      | GREATER_THAN
      | LESS_THAN_OR_EQUALS
      | GREATER_THAN_OR_EQUALS
      | PLUS
      | MINUS
      | ASTERISK
      | SLASH
      | PERCENT
      | CARET
      | AMPERSAND
      | PIPE
      | TILDE
      | identifier
      )
    ;

operatorOption
    : FUNCTION EQUALS qualifiedName
    | PROCEDURE EQUALS qualifiedName
    | LEFTARG EQUALS dataType
    | RIGHTARG EQUALS dataType
    | COMMUTATOR EQUALS operatorName
    | NEGATOR EQUALS operatorName
    | RESTRICT EQUALS qualifiedName
    | JOIN EQUALS qualifiedName
    | HASHES
    | MERGES
    ;

// CREATE AGGREGATE
createAggregateStatement
    : CREATE (OR REPLACE)? AGGREGATE qualifiedName
      OPEN_PAREN aggregateArgList CLOSE_PAREN
      OPEN_PAREN aggregateOption (COMMA aggregateOption)* CLOSE_PAREN
    ;

aggregateArgList
    : (ASTERISK | (argMode? dataType (COMMA argMode? dataType)*))? (ORDER BY argMode? dataType (COMMA argMode? dataType)*)?
    ;

aggregateOption
    : SFUNC EQUALS qualifiedName
    | STYPE EQUALS dataType
    | SSPACE EQUALS INTEGER_LITERAL
    | FINALFUNC EQUALS qualifiedName
    | FINALFUNC_EXTRA
    | FINALFUNC_MODIFY EQUALS (READ_ONLY | SHAREABLE | READ_WRITE)
    | COMBINEFUNC EQUALS qualifiedName
    | SERIALFUNC EQUALS qualifiedName
    | DESERIALFUNC EQUALS qualifiedName
    | INITCOND EQUALS literal
    | MSFUNC EQUALS qualifiedName
    | MINVFUNC EQUALS qualifiedName
    | MSTYPE EQUALS dataType
    | MSSPACE EQUALS INTEGER_LITERAL
    | MFINALFUNC EQUALS qualifiedName
    | MFINALFUNC_EXTRA
    | MFINALFUNC_MODIFY EQUALS (READ_ONLY | SHAREABLE | READ_WRITE)
    | MINITCOND EQUALS literal
    | SORTOP EQUALS operatorName
    | PARALLEL EQUALS (SAFE | RESTRICTED | UNSAFE)
    | HYPOTHETICAL
    ;

// CREATE CAST
createCastStatement
    : CREATE CAST OPEN_PAREN dataType AS dataType CLOSE_PAREN
      (WITH FUNCTION qualifiedName argTypeList? | WITH INOUT | WITHOUT FUNCTION)
      (AS ASSIGNMENT | AS IMPLICIT)?
    ;

argTypeList
    : OPEN_PAREN dataTypeList? CLOSE_PAREN
    ;

// CREATE COLLATION
createCollationStatement
    : CREATE COLLATION ifNotExists? qualifiedName
      (FROM qualifiedName | OPEN_PAREN collationOption (COMMA collationOption)* CLOSE_PAREN)
    ;

collationOption
    : LOCALE EQUALS stringLiteral
    | LC_COLLATE EQUALS stringLiteral
    | LC_CTYPE EQUALS stringLiteral
    | PROVIDER EQUALS identifier
    | DETERMINISTIC EQUALS booleanLiteral
    | RULES EQUALS stringLiteral
    | VERSION EQUALS stringLiteral
    ;

// CREATE CONVERSION
createConversionStatement
    : CREATE DEFAULT? CONVERSION qualifiedName
      FOR stringLiteral TO stringLiteral
      FROM qualifiedName
    ;

// CREATE LANGUAGE
createLanguageStatement
    : CREATE (OR REPLACE)? TRUSTED? PROCEDURAL? LANGUAGE identifier
      (HANDLER qualifiedName (INLINE qualifiedName)? (VALIDATOR qualifiedName)?)?
    ;

// CREATE TABLESPACE
createTablespaceStatement
    : CREATE TABLESPACE identifier
      (OWNER roleSpecification)?
      LOCATION stringLiteral
      (WITH OPEN_PAREN tablespaceOption (COMMA tablespaceOption)* CLOSE_PAREN)?
    ;

tablespaceOption
    : identifier EQUALS literal
    ;

// CREATE ACCESS METHOD
createAccessMethodStatement
    : CREATE ACCESS METHOD identifier
      TYPE (INDEX | TABLE)
      HANDLER qualifiedName
    ;

// CREATE STATISTICS
createStatisticsStatement
    : CREATE STATISTICS ifNotExists? qualifiedName
      statisticsKind?
      ON expressionList
      FROM tableName
    ;

statisticsKind
    : OPEN_PAREN identifierList CLOSE_PAREN
    ;

// CREATE TRANSFORM
createTransformStatement
    : CREATE (OR REPLACE)? TRANSFORM FOR dataType
      LANGUAGE identifier
      OPEN_PAREN transformFunction COMMA transformFunction CLOSE_PAREN
    ;

transformFunction
    : (FROM | TO) SQL WITH FUNCTION qualifiedName argTypeList?
    ;

// =============================================================================
// EXPRESSIONS (continued)
// =============================================================================

aggregateExpression
    : aggregateName OPEN_PAREN setQuantifier? expressionList orderByClause? CLOSE_PAREN filterClause? overClause?
    | aggregateName OPEN_PAREN ASTERISK CLOSE_PAREN filterClause? overClause?
    | GROUPING OPEN_PAREN expressionList CLOSE_PAREN
    ;

aggregateName
    : COUNT | SUM | AVG | MIN | MAX | identifier
    ;

windowFunctionCall
    : windowFunctionName OPEN_PAREN expressionList? CLOSE_PAREN overClause
    ;

windowFunctionName
    : ROW_NUMBER | RANK | DENSE_RANK | PERCENT_RANK | CUME_DIST
    | NTILE | LAG | LEAD | FIRST_VALUE | LAST_VALUE | NTH_VALUE
    | identifier
    ;

subqueryExpression
    : OPEN_PAREN selectStatement CLOSE_PAREN
    | ARRAY OPEN_PAREN selectStatement CLOSE_PAREN
    ;

arrayExpression
    : ARRAY OPEN_BRACKET expressionList? CLOSE_BRACKET
    ;

rowExpression
    : ROW OPEN_PAREN expressionList? CLOSE_PAREN
    | OPEN_PAREN expression COMMA expressionList CLOSE_PAREN
    ;

// =============================================================================
// DATA TYPES
// =============================================================================

dataType
    : simpleType arrayDimension*
    | SETOF simpleType arrayDimension*
    ;

simpleType
    : numericType
    | characterType
    | dateTimeType
    | booleanType
    | binaryType
    | jsonType
    | xmlType
    | uuidType
    | intervalType
    | qualifiedName
    ;

numericType
    : SMALLINT | INTEGER | INT | BIGINT
    | DECIMAL (OPEN_PAREN INTEGER_LITERAL (COMMA INTEGER_LITERAL)? CLOSE_PAREN)?
    | NUMERIC (OPEN_PAREN INTEGER_LITERAL (COMMA INTEGER_LITERAL)? CLOSE_PAREN)?
    | REAL | DOUBLE PRECISION
    | FLOAT (OPEN_PAREN INTEGER_LITERAL CLOSE_PAREN)?
    ;

characterType
    : (CHARACTER | CHAR) VARYING? (OPEN_PAREN INTEGER_LITERAL CLOSE_PAREN)?
    | VARCHAR (OPEN_PAREN INTEGER_LITERAL CLOSE_PAREN)?
    | TEXT
    ;

dateTimeType
    : DATE
    | TIME (OPEN_PAREN INTEGER_LITERAL CLOSE_PAREN)? ((WITH | WITHOUT) TIME ZONE)?
    | TIMESTAMP (OPEN_PAREN INTEGER_LITERAL CLOSE_PAREN)? ((WITH | WITHOUT) TIME ZONE)?
    ;

intervalType
    : INTERVAL intervalField? (OPEN_PAREN INTEGER_LITERAL CLOSE_PAREN)?
    ;

intervalField
    : YEAR (TO MONTH)?
    | MONTH
    | DAY (TO (HOUR | MINUTE | SECOND))?
    | HOUR (TO (MINUTE | SECOND))?
    | MINUTE (TO SECOND)?
    | SECOND
    ;

booleanType
    : BOOLEAN
    ;

binaryType
    : BIT VARYING? (OPEN_PAREN INTEGER_LITERAL CLOSE_PAREN)?
    ;

jsonType
    : JSON
    ;

xmlType
    : XML
    ;

uuidType
    : UUID
    ;

arrayDimension
    : OPEN_BRACKET INTEGER_LITERAL? CLOSE_BRACKET
    ;

dataTypeList
    : dataType (COMMA dataType)*
    ;

// =============================================================================
// COMMON RULES
// =============================================================================

identifier
    : IDENTIFIER
    | QUOTED_IDENTIFIER
    | UNICODE_IDENTIFIER
    | nonReservedKeyword
    ;

nonReservedKeyword
    : ABORT | ABSOLUTE | ACCESS | ACTION | ADD | ADMIN | AFTER | AGGREGATE | ALSO | ALTER
    | ALWAYS | ASSERTION | ASSIGNMENT | AT | ATOMIC | ATTACH | ATTRIBUTE | BACKWARD
    | BEFORE | BEGIN | BY | CACHE | CALL | CALLED | CASCADE | CASCADED | CATALOG
    | CHAIN | CHARACTERISTICS | CHECKPOINT | CLASS | CLOSE | CLUSTER | COLUMNS | COMMENT
    | COMMENTS | COMMIT | COMMITTED | COMPRESSION | CONFIGURATION | CONFLICT | CONNECTION
    | CONSTRAINTS | CONTENT | CONTINUE | CONVERSION | COPY | COST | CSV | CUBE | CURRENT
    | CURSOR | CYCLE | DATA | DATABASE | DAY | DEALLOCATE | DECLARE | DEFAULTS | DEFERRED
    | DEFINER | DELETE | DELIMITER | DELIMITERS | DEPENDS | DETACH | DICTIONARY
    | DISABLE | DISCARD | DOCUMENT | DOMAIN | DOUBLE | DROP | EACH | ENABLE | ENCODING
    | ENCRYPTED | ENFORCED | ENUM | ESCAPE | EVENT | EXCLUDE | EXCLUDING | EXCLUSIVE | EXECUTE
    | EXPLAIN | EXPRESSION | EXTENSION | EXTERNAL | FAMILY | FILTER | FINALIZE | FIRST
    | FOLLOWING | FORCE | FORMAT | FORWARD | FUNCTION | FUNCTIONS | GENERATED | GLOBAL
    | GRANTED | GROUPS | HANDLER | HEADER | HOLD | HOUR | IDENTITY | IF | IMMEDIATE
    | IMMUTABLE | IMPLICIT | IMPORT | INCLUDE | INCLUDING | INCREMENT | INDEX
    | INDEXES | INHERIT | INHERITS | INLINE | INPUT | INSENSITIVE | INSERT | INSTEAD
    | INVOKER | ISOLATION | KEY | KEYS | LABEL | LANGUAGE | LARGE | LAST | LEAKPROOF
    | LEVEL | LISTEN | LOAD | LOCAL | LOCATION | LOCK | LOCKED | LOGGED | LOG_VERBOSITY
    | MAPPING | MATCH | MATCHED | MATERIALIZED | MAXVALUE | MERGE | METHOD | MINUTE | MINVALUE
    | MODE | MONTH | MOVE | NAME | NAMES | NEW | NEXT | NO | NONE
    | NOTHING | NOTIFY | NOWAIT | NULLS | OBJECT | OF | OFF | OIDS | OLD | OPERATOR | OPTION
    | OPTIONS | ORDINALITY | OTHERS | OVER | OVERRIDING | OWNED | OWNER | PARALLEL | PARSER
    | PARTIAL | PARTITION | PASSING | PASSWORD | PERIOD | PLANS | POLICY | PRECEDING
    | PREPARE | PREPARED | PRESERVE | PRIOR | PRIVILEGES | PROCEDURAL | PROCEDURE | PROCEDURES
    | PROGRAM | PUBLICATION | QUOTE | QUOTES | RANGE | READ | REASSIGN | RECURSIVE
    | REF | REFERENCING | REFRESH | REINDEX | REJECT_LIMIT | RELATIVE | RELEASE | RENAME
    | REPEATABLE | REPLACE | REPLICA | RESET | RESTART | RESTRICT | RETURN | RETURNS | REVOKE
    | ROLE | ROLLBACK | ROLLUP | ROUTINE | ROUTINES | ROW | ROWS | RULE | SAVEPOINT | SCALAR
    | SCHEMA | SCHEMAS | SCROLL | SEARCH | SECOND | SECURITY | SEQUENCE | SEQUENCES
    | SERIALIZABLE | SERVER | SESSION | SET | SETOF | SETS | SHARE | SHOW | SILENT | SIMPLE
    | SKIP_ | SNAPSHOT | SQL | STABLE | STANDALONE | START | STATEMENT | STATISTICS | STDIN
    | STDOUT | STORAGE | STORED | STRICT | STRIP | SUBSCRIPTION | SUPPORT | SYSID | SYSTEM
    | TABLES | TABLESPACE | TEMP | TEMPLATE | TEMPORARY | TEXT | TIES | TRANSACTION
    | TRANSFORM | TRIGGER | TRUNCATE | TRUSTED | TYPE | TYPES | UNBOUNDED | UNCOMMITTED
    | UNCONDITIONAL | UNENCRYPTED | UNKNOWN | UNLISTEN | UNLOGGED | UNTIL | UPDATE | VACUUM
    | VALID | VALIDATE | VALIDATOR | VALUE | VALUES | VARYING | VERSION | VIEW | VIEWS
    | VIRTUAL | VOLATILE | WHITESPACE | WITHIN | WITHOUT | WORK | WRAPPER | WRITE | XML
    | YEAR | YES | ZONE | COUNT | SUM | AVG | MIN | MAX
    ;

qualifiedName
    : identifier (DOT identifier)*
    ;

qualifiedNameList
    : qualifiedName (COMMA qualifiedName)*
    ;

tableName
    : qualifiedName
    ;

tableNameList
    : tableName (COMMA tableName)*
    ;

viewName
    : qualifiedName
    ;

indexName
    : identifier
    ;

sequenceName
    : qualifiedName
    ;

schemaNameList
    : identifier (COMMA identifier)*
    ;

columnNameList
    : OPEN_PAREN identifier (COMMA identifier)* CLOSE_PAREN
    | identifier (COMMA identifier)*
    ;

typeName
    : qualifiedName
    ;

roleSpecification
    : identifier
    | CURRENT_ROLE
    | CURRENT_USER
    | SESSION_USER
    | PUBLIC
    ;

roleSpecificationList
    : roleSpecification (COMMA roleSpecification)*
    ;

identifierList
    : identifier (COMMA identifier)*
    ;

// Literals
literal
    : stringLiteral
    | numericLiteral
    | booleanLiteral
    | nullLiteral
    | bitStringLiteral
    | hexStringLiteral
    ;

stringLiteral
    : STRING_LITERAL
    | ESCAPE_STRING
    | UNICODE_STRING
    | DOLLAR_STRING
    ;

stringLiteralList
    : stringLiteral (COMMA stringLiteral)*
    ;

numericLiteral
    : INTEGER_LITERAL
    | NUMERIC_LITERAL
    | HEX_LITERAL
    | BINARY_LITERAL
    | OCTAL_LITERAL
    ;

signedInteger
    : PLUS? INTEGER_LITERAL
    | MINUS INTEGER_LITERAL
    ;

booleanLiteral
    : TRUE
    | FALSE
    | ON
    | OFF
    ;

nullLiteral
    : NULL
    ;

bitStringLiteral
    : BIT_STRING
    ;

hexStringLiteral
    : HEX_STRING
    ;

expressionList
    : expression (COMMA expression)*
    ;

ifExists
    : IF EXISTS
    ;

ifNotExists
    : IF NOT EXISTS
    ;

collateClause
    : COLLATE identifier
    ;

compressionMethod
    : COMPRESSION identifier
    ;

storageType
    : STORAGE (PLAIN | EXTERNAL | EXTENDED | MAIN | DEFAULT)
    ;
 COALESCE OPEN_PAREN expressionList CLOSE_PAREN
    | NULLIF OPEN_PAREN expression COMMA expression CLOSE_PAREN
    | GREATEST OPEN_PAREN expressionList CLOSE_PAREN
    | LEAST OPEN_PAREN expressionList CLOSE_PAREN
    | EXTRACT OPEN_PAREN extractField FROM expression CLOSE_PAREN
    | OVERLAY OPEN_PAREN expression PLACING expression FROM expression (FOR expression)? CLOSE_PAREN
    | POSITION OPEN_PAREN expression IN expression CLOSE_PAREN
    | SUBSTRING OPEN_PAREN expression (FROM expression)? (FOR expression)? CLOSE_PAREN
    | TRIM OPEN_PAREN (LEADING | TRAILING | BOTH)? expression? FROM? expression CLOSE_PAREN
    ;

extractField
    : YEAR | MONTH | DAY | HOUR | MINUTE | SECOND
    | identifier
    ;

aggregateExpression
    : aggregateName OPEN_PAREN setQuantifier? expressionList orderByClause? CLOSE_PAREN
      filterClause? overClause?
    | aggregateName OPEN_PAREN ASTERISK CLOSE_PAREN filterClause? overClause?
    | GROUPING OPEN_PAREN expressionList CLOSE_PAREN
    ;

aggregateName
    : COUNT | SUM | AVG | MIN | MAX
    | identifier
    ;

windowFunctionCall
    : windowFunctionName OPEN_PAREN expressionList? CLOSE_PAREN overClause
    ;

windowFunctionName
    : ROW_NUMBER | RANK | DENSE_RANK | PERCENT_RANK | CUME_DIST
    | NTILE | LAG | LEAD | FIRST_VALUE | LAST_VALUE | NTH_VALUE
    | identifier
    ;

subqueryExpression
    : OPEN_PAREN selectStatement CLOSE_PAREN
    | ARRAY OPEN_PAREN selectStatement CLOSE_PAREN
    ;

arrayExpression
    : ARRAY OPEN_BRACKET expressionList? CLOSE_BRACKET
    | ARRAY OPEN_BRACKET arrayExpression (COMMA arrayExpression)* CLOSE_BRACKET
    ;

rowExpression
    : ROW OPEN_PAREN expressionList? CLOSE_PAREN
    | OPEN_PAREN expression COMMA expressionList CLOSE_PAREN
    ;

// =============================================================================
// DATA TYPES
// =============================================================================

dataType
    : simpleType arrayDimension*
    | SETOF simpleType arrayDimension*
    ;

simpleType
    : numericType
    | characterType
    | dateTimeType
    | booleanType
    | binaryType
    | jsonType
    | xmlType
    | uuidType
    | intervalType
    | qualifiedName
    ;

numericType
    : SMALLINT
    | INTEGER | INT
    | BIGINT
    | DECIMAL (OPEN_PAREN INTEGER_LITERAL (COMMA INTEGER_LITERAL)? CLOSE_PAREN)?
    | NUMERIC (OPEN_PAREN INTEGER_LITERAL (COMMA INTEGER_LITERAL)? CLOSE_PAREN)?
    | REAL
    | DOUBLE PRECISION
    | FLOAT (OPEN_PAREN INTEGER_LITERAL CLOSE_PAREN)?
    | SERIAL
    ;

characterType
    : (CHARACTER | CHAR) VARYING? (OPEN_PAREN INTEGER_LITERAL CLOSE_PAREN)?
    | VARCHAR (OPEN_PAREN INTEGER_LITERAL CLOSE_PAREN)?
    | TEXT
    | NAME
    ;

dateTimeType
    : DATE
    | TIME (OPEN_PAREN INTEGER_LITERAL CLOSE_PAREN)? ((WITH | WITHOUT) TIME ZONE)?
    | TIMESTAMP (OPEN_PAREN INTEGER_LITERAL CLOSE_PAREN)? ((WITH | WITHOUT) TIME ZONE)?
    ;

intervalType
    : INTERVAL intervalField? (OPEN_PAREN INTEGER_LITERAL CLOSE_PAREN)?
    ;

intervalField
    : YEAR (TO MONTH)?
    | MONTH
    | DAY (TO (HOUR | MINUTE | SECOND))?
    | HOUR (TO (MINUTE | SECOND))?
    | MINUTE (TO SECOND)?
    | SECOND
    ;

booleanType
    : BOOLEAN
    ;

binaryType
    : BIT VARYING? (OPEN_PAREN INTEGER_LITERAL CLOSE_PAREN)?
    ;

jsonType
    : JSON
    ;

xmlType
    : XML
    ;

uuidType
    : identifier  // UUID type
    ;

arrayDimension
    : OPEN_BRACKET INTEGER_LITERAL? CLOSE_BRACKET
    ;

dataTypeList
    : dataType (COMMA dataType)*
    ;

// =============================================================================
// COMMON RULES
// =============================================================================

// Identifiers
identifier
    : IDENTIFIER
    | QUOTED_IDENTIFIER
    | UNICODE_IDENTIFIER
    | nonReservedKeyword
    ;

// Non-reserved keywords that can be used as identifiers
nonReservedKeyword
    : ABORT | ABSOLUTE | ACCESS | ACTION | ADD | ADMIN | AFTER | AGGREGATE | ALSO | ALTER
    | ALWAYS | ASENSITIVE | ASSERTION | ASSIGNMENT | AT | ATOMIC | ATTACH | ATTRIBUTE | BACKWARD
    | BEFORE | BEGIN | BREADTH | BY | CACHE | CALL | CALLED | CASCADE | CASCADED | CATALOG
    | CHAIN | CHARACTERISTICS | CHECKPOINT | CLASS | CLOSE | CLUSTER | COLUMNS | COMMENT
    | COMMENTS | COMMIT | COMMITTED | COMPRESSION | CONFIGURATION | CONFLICT | CONNECTION
    | CONSTRAINTS | CONTENT | CONTINUE | CONVERSION | COPY | COST | CSV | CUBE | CURRENT
    | CURSOR | CYCLE | DATA | DATABASE | DAY | DEALLOCATE | DECLARE | DEFAULTS | DEFERRED
    | DEFINER | DELETE | DELIMITER | DELIMITERS | DEPENDS | DEPTH | DETACH | DICTIONARY
    | DISABLE | DISCARD | DOCUMENT | DOMAIN | DOUBLE | DROP | EACH | ENABLE | ENCODING
    | ENCRYPTED | ENFORCED | ENUM | ESCAPE | EVENT | EXCLUDE | EXCLUDING | EXCLUSIVE | EXECUTE
    | EXPLAIN | EXPRESSION | EXTENSION | EXTERNAL | FAMILY | FILTER | FINALIZE | FIRST
    | FOLLOWING | FORCE | FORMAT | FORWARD | FUNCTION | FUNCTIONS | GENERATED | GLOBAL
    | GRANTED | GROUPS | HANDLER | HEADER | HOLD | HOUR | IDENTITY | IF | IMMEDIATE
    | IMMUTABLE | IMPLICIT | IMPORT | INCLUDE | INCLUDING | INCREMENT | INDENT | INDEX
    | INDEXES | INHERIT | INHERITS | INLINE | INPUT | INSENSITIVE | INSERT | INSTEAD
    | INVOKER | ISOLATION | JSON | KEY | KEYS | LABEL | LANGUAGE | LARGE | LAST | LEAKPROOF
    | LEVEL | LISTEN | LOAD | LOCAL | LOCATION | LOCK | LOCKED | LOGGED | LOG_VERBOSITY
    | MAPPING | MATCH | MATCHED | MATERIALIZED | MAXVALUE | MERGE | METHOD | MINUTE | MINVALUE
    | MODE | MONTH | MOVE | NAME | NAMES | NEW | NEXT | NFC | NFD | NFKC | NFKD | NO | NONE
    | NOTHING | NOTIFY | NOWAIT | NULLS | OBJECT | OF | OFF | OIDS | OLD | OPERATOR | OPTION
    | OPTIONS | ORDINALITY | OTHERS | OVER | OVERRIDING | OWNED | OWNER | PARALLEL | PARSER
    | PARTIAL | PARTITION | PASSING | PASSWORD | PATH | PERIOD | PLANS | POLICY | PRECEDING
    | PREPARE | PREPARED | PRESERVE | PRIOR | PRIVILEGES | PROCEDURAL | PROCEDURE | PROCEDURES
    | PROGRAM | PUBLICATION | QUOTE | QUOTES | RANGE | READ | REASSIGN | RECHECK | RECURSIVE
    | REF | REFERENCING | REFRESH | REINDEX | REJECT_LIMIT | RELATIVE | RELEASE | RENAME
    | REPEATABLE | REPLACE | REPLICA | RESET | RESTART | RESTRICT | RETURN | RETURNS | REVOKE
    | ROLE | ROLLBACK | ROLLUP | ROUTINE | ROUTINES | ROW | ROWS | RULE | SAVEPOINT | SCALAR
    | SCHEMA | SCHEMAS | SCROLL | SEARCH | SECOND | SECURITY | SEQUENCE | SEQUENCES
    | SERIALIZABLE | SERVER | SESSION | SET | SETOF | SETS | SHARE | SHOW | SILENT | SIMPLE
    | SKIP_ | SNAPSHOT | SQL | STABLE | STANDALONE | START | STATEMENT | STATISTICS | STDIN
    | STDOUT | STORAGE | STORED | STRICT | STRIP | SUBSCRIPTION | SUPPORT | SYSID | SYSTEM
    | TABLES | TABLESPACE | TEMP | TEMPLATE | TEMPORARY | TEXT | TIES | TRANSACTION
    | TRANSFORM | TRIGGER | TRUNCATE | TRUSTED | TYPE | TYPES | UNBOUNDED | UNCOMMITTED
    | UNCONDITIONAL | UNENCRYPTED | UNKNOWN | UNLISTEN | UNLOGGED | UNTIL | UPDATE | VACUUM
    | VALID | VALIDATE | VALIDATOR | VALUE | VALUES | VARYING | VERSION | VIEW | VIEWS
    | VIRTUAL | VOLATILE | WHITESPACE | WITHIN | WITHOUT | WORK | WRAPPER | WRITE | XML
    | YEAR | YES | ZONE
    | COUNT | SUM | AVG | MIN | MAX
    | ROW_NUMBER | RANK | DENSE_RANK | PERCENT_RANK | CUME_DIST | NTILE | LAG | LEAD
    | FIRST_VALUE | LAST_VALUE | NTH_VALUE
    | COSTS | BUFFERS | TIMING | SUMMARY | YAML
    | MAINTAIN
    ;

qualifiedName
    : identifier (DOT identifier)*
    ;

qualifiedNameList
    : qualifiedName (COMMA qualifiedName)*
    ;

tableName
    : qualifiedName
    ;

tableNameList
    : tableName (COMMA tableName)*
    ;

viewName
    : qualifiedName
    ;

indexName
    : identifier
    ;

sequenceName
    : qualifiedName
    ;

schemaNameList
    : identifier (COMMA identifier)*
    ;

columnNameList
    : OPEN_PAREN identifier (COMMA identifier)* CLOSE_PAREN
    | identifier (COMMA identifier)*
    ;

typeName
    : qualifiedName
    ;

roleSpecification
    : identifier
    | CURRENT_ROLE
    | CURRENT_USER
    | SESSION_USER
    | PUBLIC
    ;

roleSpecificationList
    : roleSpecification (COMMA roleSpecification)*
    ;

identifierList
    : identifier (COMMA identifier)*
    ;

// Literals
literal
    : stringLiteral
    | numericLiteral
    | booleanLiteral
    | nullLiteral
    | bitStringLiteral
    | hexStringLiteral
    ;

stringLiteral
    : STRING_LITERAL
    | ESCAPE_STRING
    | UNICODE_STRING
    | DOLLAR_STRING
    ;

stringLiteralList
    : stringLiteral (COMMA stringLiteral)*
    ;

numericLiteral
    : INTEGER_LITERAL
    | NUMERIC_LITERAL
    | HEX_LITERAL
    | BINARY_LITERAL
    | OCTAL_LITERAL
    ;

signedInteger
    : PLUS? INTEGER_LITERAL
    | MINUS INTEGER_LITERAL
    ;

booleanLiteral
    : TRUE
    | FALSE
    | ON
    | OFF
    ;

nullLiteral
    : NULL
    ;

bitStringLiteral
    : BIT_STRING
    ;

hexStringLiteral
    : HEX_STRING
    ;

// Expression list
expressionList
    : expression (COMMA expression)*
    ;

// Common clauses
ifExists
    : IF EXISTS
    ;

ifNotExists
    : IF NOT EXISTS
    ;

collateClause
    : COLLATE identifier
    ;

compressionMethod
    : COMPRESSION identifier
    ;

storageType
    : STORAGE (PLAIN | EXTERNAL | EXTENDED | MAIN | DEFAULT)
    ;
