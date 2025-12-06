/*
 * CypherParser.g4 - ANTLR4 Parser Grammar for Neo4j Cypher 5.x
 * 
 * Based on openCypher specification and Neo4j Cypher Manual
 * Compatible with Neo4j 5.x and Cypher 25
 * 
 * Licensed under Apache License 2.0
 */

parser grammar CypherParser;

options {
    tokenVocab = CypherLexer;
}

// =============================================================================
// Top-level Rules
// =============================================================================

cypher
    : statement ( SEMICOLON statement )* SEMICOLON? EOF
    ;

statement
    : query
    | command
    ;

query
    : regularQuery
    | standaloneCall
    ;

regularQuery
    : singleQuery ( unionClause )*
    ;

unionClause
    : UNION ALL? singleQuery
    ;

singleQuery
    : clause+
    ;

// =============================================================================
// Clauses
// =============================================================================

clause
    : matchClause
    | unwindClause
    | mergeClause
    | createClause
    | setClause
    | deleteClause
    | removeClause
    | withClause
    | returnClause
    | orderByClause
    | skipClause
    | limitClause
    | whereClause
    | foreachClause
    | loadCSVClause
    | callClause
    | subqueryClause
    ;

// MATCH clause
matchClause
    : OPTIONAL? MATCH pattern whereClause?
    ;

// UNWIND clause
unwindClause
    : UNWIND expression AS variable
    ;

// MERGE clause
mergeClause
    : MERGE patternPart ( mergeAction )*
    ;

mergeAction
    : ON MATCH setClause
    | ON CREATE setClause
    ;

// CREATE clause
createClause
    : CREATE pattern
    ;

// SET clause
setClause
    : SET setItem ( COMMA setItem )*
    ;

setItem
    : propertyExpression EQ expression
    | variable EQ expression
    | variable PLUS_EQ expression
    | variable nodeLabels
    ;

// DELETE clause
deleteClause
    : DETACH? DELETE expression ( COMMA expression )*
    ;

// REMOVE clause
removeClause
    : REMOVE removeItem ( COMMA removeItem )*
    ;

removeItem
    : variable nodeLabels
    | propertyExpression
    ;

// WITH clause
withClause
    : WITH DISTINCT? returnBody whereClause?
    ;

// RETURN clause
returnClause
    : RETURN DISTINCT? returnBody
    ;

returnBody
    : returnItems orderByClause? skipClause? limitClause?
    ;

returnItems
    : ASTERISK
    | returnItem ( COMMA returnItem )*
    ;

returnItem
    : expression ( AS variable )?
    ;

// ORDER BY clause
orderByClause
    : ORDER BY sortItem ( COMMA sortItem )*
    ;

sortItem
    : expression ( ASC | ASCENDING | DESC | DESCENDING )?
    ;

// SKIP clause
skipClause
    : SKIP_ expression
    ;

// LIMIT clause
limitClause
    : LIMIT expression
    ;

// WHERE clause
whereClause
    : WHERE expression
    ;

// FOREACH clause
foreachClause
    : FOREACH LPAREN variable IN expression BAR clause+ RPAREN
    ;

// LOAD CSV clause
loadCSVClause
    : LOAD CSV ( WITH HEADERS )? FROM expression AS variable 
      ( FIELDTERMINATOR STRING_LITERAL )?
    ;

// CALL clause (procedure call)
callClause
    : CALL procedureInvocation ( YIELD yieldItems )?
    ;

standaloneCall
    : CALL procedureInvocation ( YIELD ( ASTERISK | yieldItems ) )?
    ;

procedureInvocation
    : procedureName LPAREN ( expression ( COMMA expression )* )? RPAREN
    ;

procedureName
    : namespace symbolicName
    ;

namespace
    : ( symbolicName DOT )*
    ;

yieldItems
    : yieldItem ( COMMA yieldItem )* whereClause?
    ;

yieldItem
    : procedureResultField ( AS variable )?
    ;

procedureResultField
    : symbolicName
    ;

// Subquery (CALL { ... })
subqueryClause
    : CALL LBRACE regularQuery RBRACE
    ;

// =============================================================================
// Patterns
// =============================================================================

pattern
    : patternPart ( COMMA patternPart )*
    ;

patternPart
    : ( variable EQ )? anonymousPatternPart
    ;

anonymousPatternPart
    : shortestPathPattern
    | patternElement
    ;

shortestPathPattern
    : SHORTESTPATH LPAREN patternElement RPAREN
    | ALLSHORTESTPATHS LPAREN patternElement RPAREN
    ;

patternElement
    : nodePattern ( patternElementChain )*
    | LPAREN patternElement RPAREN
    ;

patternElementChain
    : relationshipPattern nodePattern
    ;

// Node pattern
nodePattern
    : LPAREN ( variable )? ( nodeLabels )? ( properties )? ( whereClause )? RPAREN
    ;

// Relationship pattern
relationshipPattern
    : leftArrowHead? dash relationshipDetail? dash rightArrowHead?
    ;

leftArrowHead
    : ARROW_LEFT_HEAD
    | ARROW_LEFT
    ;

rightArrowHead
    : ARROW_RIGHT_HEAD
    | ARROW_RIGHT
    ;

dash
    : DASH
    ;

relationshipDetail
    : LBRACKET ( variable )? ( relationshipTypes )? ( rangeLiteral )? ( properties )? ( whereClause )? RBRACKET
    ;

// Quantified path patterns (GQL additions)
rangeLiteral
    : ASTERISK ( integerLiteral )? ( DOTDOT ( integerLiteral )? )?
    ;

// Labels and Types
nodeLabels
    : nodeLabel+
    ;

nodeLabel
    : COLON labelName
    ;

labelName
    : symbolicName
    ;

relationshipTypes
    : COLON relTypeName ( BAR COLON? relTypeName )*
    ;

relTypeName
    : symbolicName
    ;

// Properties
properties
    : mapLiteral
    | parameter
    ;

// =============================================================================
// Expressions
// =============================================================================

expression
    : orExpression
    ;

orExpression
    : xorExpression ( OR xorExpression )*
    ;

xorExpression
    : andExpression ( XOR andExpression )*
    ;

andExpression
    : notExpression ( AND notExpression )*
    ;

notExpression
    : NOT* comparisonExpression
    ;

comparisonExpression
    : addOrSubtractExpression ( comparisonOperator addOrSubtractExpression )?
    ;

comparisonOperator
    : EQ
    | NEQ
    | NEQ2
    | LT
    | GT
    | LTE
    | GTE
    ;

addOrSubtractExpression
    : multiplyDivideModuloExpression ( ( PLUS | MINUS ) multiplyDivideModuloExpression )*
    ;

multiplyDivideModuloExpression
    : powerOfExpression ( ( ASTERISK | SLASH | PERCENT ) powerOfExpression )*
    ;

powerOfExpression
    : unaryAddOrSubtractExpression ( CARET unaryAddOrSubtractExpression )*
    ;

unaryAddOrSubtractExpression
    : ( PLUS | MINUS )* stringListNullOperatorExpression
    ;

stringListNullOperatorExpression
    : propertyOrLabelsExpression ( stringOperator | listOperator | nullOperator )*
    ;

stringOperator
    : ( STARTS | ENDS ) WITH propertyOrLabelsExpression
    | CONTAINS propertyOrLabelsExpression
    | ( EQ | NEQ | NEQ2 ) TILDE? propertyOrLabelsExpression
    ;

listOperator
    : IN propertyOrLabelsExpression
    | LBRACKET expression RBRACKET
    | LBRACKET expression? DOTDOT expression? RBRACKET
    ;

nullOperator
    : IS NOT? NULL
    | IS NOT? NORMALIZED ( NFC | NFD | NFKC | NFKD )?
    ;

propertyOrLabelsExpression
    : atom ( propertyLookup | nodeLabels )*
    ;

propertyLookup
    : DOT propertyKeyName
    ;

propertyExpression
    : atom ( propertyLookup )+
    ;

propertyKeyName
    : symbolicName
    ;

// =============================================================================
// Atoms
// =============================================================================

atom
    : literal
    | parameter
    | caseExpression
    | countExpression
    | existsExpression
    | listComprehension
    | patternComprehension
    | quantifier
    | parenthesizedExpression
    | functionInvocation
    | variable
    ;

// Literals
literal
    : numberLiteral
    | STRING_LITERAL
    | booleanLiteral
    | NULL
    | mapLiteral
    | listLiteral
    ;

numberLiteral
    : integerLiteral
    | doubleLiteral
    ;

integerLiteral
    : DECIMAL_INTEGER
    | HEX_INTEGER
    | OCTAL_INTEGER
    ;

doubleLiteral
    : FLOAT_LITERAL
    | INFINITY
    | INF
    | NAN
    ;

booleanLiteral
    : TRUE
    | FALSE
    ;

// Map literal
mapLiteral
    : LBRACE ( propertyKeyName COLON expression ( COMMA propertyKeyName COLON expression )* )? RBRACE
    ;

// List literal
listLiteral
    : LBRACKET ( expression ( COMMA expression )* )? RBRACKET
    ;

// CASE expression
caseExpression
    : CASE ( expression )? ( caseAlternative )+ ( ELSE expression )? END
    ;

caseAlternative
    : WHEN expression THEN expression
    ;

// COUNT subquery
countExpression
    : COUNT LBRACE ( regularQuery | pattern whereClause? ) RBRACE
    ;

// EXISTS subquery
existsExpression
    : EXISTS LBRACE ( regularQuery | pattern whereClause? ) RBRACE
    ;

// List comprehension
listComprehension
    : LBRACKET filterExpression ( BAR expression )? RBRACKET
    ;

filterExpression
    : idInColl whereClause?
    ;

idInColl
    : variable IN expression
    ;

// Pattern comprehension
patternComprehension
    : LBRACKET ( variable EQ )? patternElement whereClause? BAR expression RBRACKET
    ;

// Quantifiers (ALL, ANY, NONE, SINGLE)
quantifier
    : ( ALL | ANY | NONE | SINGLE ) LPAREN filterExpression RPAREN
    ;

// Parenthesized expression
parenthesizedExpression
    : LPAREN expression RPAREN
    ;

// Function invocation
functionInvocation
    : functionName LPAREN DISTINCT? ( expression ( COMMA expression )* )? RPAREN
    ;

functionName
    : namespace symbolicName
    ;

// Parameter
parameter
    : PARAMETER
    | DOLLAR symbolicName
    ;

// Variable
variable
    : symbolicName
    ;

// Symbolic name (identifiers)
symbolicName
    : IDENTIFIER
    | ESCAPED_IDENTIFIER
    | reservedWord
    ;

// Reserved words that can be used as identifiers in some contexts
reservedWord
    : COUNT
    | FILTER
    | EXTRACT
    | ANY
    | NONE
    | SINGLE
    | ALL
    ;

// =============================================================================
// Commands (DDL/Admin)
// =============================================================================

command
    : createCommand
    | dropCommand
    | alterCommand
    | showCommand
    | grantCommand
    | revokeCommand
    | denyCommand
    ;

// CREATE commands
createCommand
    : createIndex
    | createConstraint
    | createDatabase
    | createUser
    | createRole
    | createAlias
    ;

createIndex
    : CREATE ( RANGE | TEXT | POINT | FULLTEXT | VECTOR | LOOKUP )? INDEX ( indexName )? 
      ( IF NOT EXISTS )? 
      FOR pattern ON LPAREN ( propertyKeyName | EACH LBRACKET propertyKeyName ( COMMA propertyKeyName )* RBRACKET ) RPAREN
      ( OPTIONS mapLiteral )?
    ;

createConstraint
    : CREATE CONSTRAINT ( constraintName )? ( IF NOT EXISTS )?
      ( FOR | ON ) pattern ( ASSERT | REQUIRE ) constraintExpression
      ( OPTIONS mapLiteral )?
    ;

constraintExpression
    : propertyExpression IS ( NOT NULL | UNIQUE | NODE KEY )
    | LPAREN propertyExpression ( COMMA propertyExpression )* RPAREN IS NODE KEY
    | expression IS TYPED typeName
    ;

createDatabase
    : CREATE DATABASE symbolicName ( IF NOT EXISTS )? ( OPTIONS mapLiteral )? ( WAIT ( integerLiteral SECONDS? )? | NOWAIT )?
    ;

createUser
    : CREATE USER symbolicName ( IF NOT EXISTS )?
      SET ( PLAINTEXT | ENCRYPTED )? PASSWORD expression
      ( CHANGE NOT? REQUIRED )?
      ( SET STATUS ( ACTIVE | SUSPENDED ) )?
      ( SET HOME DATABASE symbolicName )?
    ;

createRole
    : CREATE ROLE symbolicName ( IF NOT EXISTS )?
      ( AS COPY OF symbolicName )?
    ;

createAlias
    : CREATE ALIAS symbolicName ( IF NOT EXISTS )?
      FOR DATABASE symbolicName ( AT STRING_LITERAL USER symbolicName PASSWORD expression ( DRIVER mapLiteral )? )?
    ;

// DROP commands
dropCommand
    : dropIndex
    | dropConstraint
    | dropDatabase
    | dropUser
    | dropRole
    | dropAlias
    ;

dropIndex
    : DROP INDEX indexName ( IF EXISTS )?
    ;

dropConstraint
    : DROP CONSTRAINT constraintName ( IF EXISTS )?
    ;

dropDatabase
    : DROP DATABASE symbolicName ( IF EXISTS )? ( DUMP | DESTROY )? ( WAIT ( integerLiteral SECONDS? )? | NOWAIT )?
    ;

dropUser
    : DROP USER symbolicName ( IF EXISTS )?
    ;

dropRole
    : DROP ROLE symbolicName ( IF EXISTS )?
    ;

dropAlias
    : DROP ALIAS symbolicName ( IF EXISTS )? FOR DATABASE
    ;

// ALTER commands
alterCommand
    : alterDatabase
    | alterUser
    | alterAlias
    ;

alterDatabase
    : ALTER DATABASE symbolicName ( IF EXISTS )? alterDatabaseOption+
    ;

alterDatabaseOption
    : SET ACCESS ( READ ONLY | READ WRITE )
    | SET ( TOPOLOGY | DEFAULT )? integerLiteral ( PRIMARY | PRIMARIES )
    | SET ( TOPOLOGY | DEFAULT )? integerLiteral ( SECONDARY | SECONDARIES )
    | SET OPTION symbolicName expression
    | REMOVE OPTION symbolicName
    ;

alterUser
    : ALTER USER symbolicName ( IF EXISTS )? alterUserOption+
    ;

alterUserOption
    : SET ( PLAINTEXT | ENCRYPTED )? PASSWORD ( FROM expression )? TO? expression
    | SET PASSWORD CHANGE NOT? REQUIRED
    | SET STATUS ( ACTIVE | SUSPENDED )
    | SET HOME DATABASE symbolicName
    | REMOVE HOME DATABASE
    ;

alterAlias
    : ALTER ALIAS symbolicName ( IF EXISTS )? alterAliasTarget
    ;

alterAliasTarget
    : SET DATABASE TARGET symbolicName ( AT STRING_LITERAL )? ( PROPERTIES mapLiteral )?
    | SET DATABASE USER symbolicName
    | SET DATABASE PASSWORD expression
    | SET DATABASE DRIVER mapLiteral
    ;

// SHOW commands
showCommand
    : showDatabases
    | showDatabase
    | showIndexes
    | showConstraints
    | showProcedures
    | showFunctions
    | showUsers
    | showRoles
    | showPrivileges
    | showSettings
    | showTransactions
    | showServers
    | showAliases
    ;

showDatabases
    : SHOW ( DEFAULT_ DATABASE | DATABASES | HOME DATABASE ) ( YIELD ( ASTERISK | yieldItems ) )? ( WHERE expression )? ( RETURN returnItems )?
    ;

showDatabase
    : SHOW DATABASE symbolicName ( YIELD ( ASTERISK | yieldItems ) )? ( WHERE expression )? ( RETURN returnItems )?
    ;

showIndexes
    : SHOW ( ALL | RANGE | TEXT | POINT | FULLTEXT | VECTOR | LOOKUP )? INDEX ( ES )?
      ( YIELD ( ASTERISK | yieldItems ) )? ( WHERE expression )? ( RETURN returnItems )?
    ;

showConstraints
    : SHOW ( ALL | NODE | RELATIONSHIP | REL | UNIQUE | KEY | EXIST | PROPERTY )? CONSTRAINT S?
      ( YIELD ( ASTERISK | yieldItems ) )? ( WHERE expression )? ( RETURN returnItems )?
    ;

showProcedures
    : SHOW PROCEDURE S? ( EXECUTABLE ( BY ( symbolicName | CURRENT USER ) )? )?
      ( YIELD ( ASTERISK | yieldItems ) )? ( WHERE expression )? ( RETURN returnItems )?
    ;

showFunctions
    : SHOW ( ALL | BUILT IN | USER DEFINED )? FUNCTION S? ( EXECUTABLE ( BY ( symbolicName | CURRENT USER ) )? )?
      ( YIELD ( ASTERISK | yieldItems ) )? ( WHERE expression )? ( RETURN returnItems )?
    ;

showUsers
    : SHOW ( CURRENT USER | USER S? )
      ( YIELD ( ASTERISK | yieldItems ) )? ( WHERE expression )? ( RETURN returnItems )?
    ;

showRoles
    : SHOW ( ALL | POPULATED )? ROLE S? ( WITH USER S? )?
      ( YIELD ( ASTERISK | yieldItems ) )? ( WHERE expression )? ( RETURN returnItems )?
    ;

showPrivileges
    : SHOW ( ALL | USER symbolicName? | ROLE symbolicName )? PRIVILEGE S? ( AS ( COMMAND S? | REVOKE COMMAND S? ) )?
      ( YIELD ( ASTERISK | yieldItems ) )? ( WHERE expression )? ( RETURN returnItems )?
    ;

showSettings
    : SHOW SETTING S? STRING_LITERAL?
      ( YIELD ( ASTERISK | yieldItems ) )? ( WHERE expression )? ( RETURN returnItems )?
    ;

showTransactions
    : SHOW TRANSACTION S? ( STRING_LITERAL ( COMMA STRING_LITERAL )* )?
      ( YIELD ( ASTERISK | yieldItems ) )? ( WHERE expression )? ( RETURN returnItems )?
    ;

showServers
    : SHOW SERVER S?
      ( YIELD ( ASTERISK | yieldItems ) )? ( WHERE expression )? ( RETURN returnItems )?
    ;

showAliases
    : SHOW ALIAS ( ES )? ( symbolicName )? FOR DATABASE S?
      ( YIELD ( ASTERISK | yieldItems ) )? ( WHERE expression )? ( RETURN returnItems )?
    ;

// GRANT commands
grantCommand
    : grantPrivilege
    | grantRole
    ;

grantPrivilege
    : GRANT privilegeType ON graphScope TO roleNames
    ;

grantRole
    : GRANT ROLE S? roleNames TO symbolicName ( COMMA symbolicName )*
    ;

// REVOKE commands
revokeCommand
    : revokePrivilege
    | revokeRole
    ;

revokePrivilege
    : REVOKE ( GRANT | DENY )? privilegeType ON graphScope FROM roleNames
    ;

revokeRole
    : REVOKE ROLE S? roleNames FROM symbolicName ( COMMA symbolicName )*
    ;

// DENY commands
denyCommand
    : DENY privilegeType ON graphScope TO roleNames
    ;

// Privilege specifications
privilegeType
    : graphPrivilege
    | dbmsPrivilege
    | databasePrivilege
    ;

graphPrivilege
    : ( TRAVERSE | READ | MATCH | WRITE | CREATE | DELETE | SET LABEL | REMOVE LABEL | SET PROPERTY | MERGE | ALL GRAPH? PRIVILEGES? ) 
      ( LBRACE ASTERISK RBRACE | LBRACE propertyKeyName ( COMMA propertyKeyName )* RBRACE )?
      graphQualifier?
    ;

graphQualifier
    : ( NODE | NODES | RELATIONSHIP | RELATIONSHIPS | REL | RELS ) ( ASTERISK | LPAREN ( labelName ( BAR labelName )* )? RPAREN )
    | ELEMENTS ASTERISK
    ;

dbmsPrivilege
    : ( CREATE | DROP | ALTER | ASSIGN | REMOVE | SHOW | SET | EXECUTE | ALL DBMS? PRIVILEGES? )
      ( ROLE | DATABASE | USER | CONSTRAINT | INDEX | ALIAS | PRIVILEGE )?
    ;

databasePrivilege
    : ( ACCESS | START | STOP | CREATE INDEX | DROP INDEX | SHOW INDEX | CREATE CONSTRAINT | DROP CONSTRAINT | SHOW CONSTRAINT | 
        CREATE NEW NODE? LABEL | CREATE NEW RELATIONSHIP? TYPE | CREATE NEW PROPERTY? NAME | SHOW TRANSACTION | 
        TERMINATE TRANSACTION | ALL DATABASE? PRIVILEGES? )
    ;

graphScope
    : ( DEFAULT_ | HOME ) GRAPH
    | GRAPH S? ( ASTERISK | symbolicName ( COMMA symbolicName )* )
    | ( DEFAULT_ | HOME ) DATABASE
    | DATABASE S? ( ASTERISK | symbolicName ( COMMA symbolicName )* )
    | DBMS
    ;

roleNames
    : symbolicName ( COMMA symbolicName )*
    ;

// Index and Constraint names
indexName
    : symbolicName
    ;

constraintName
    : symbolicName
    ;

// Type names (GQL type system)
typeName
    : BOOLEAN
    | STRING
    | INTEGER
    | INT
    | SIGNED INTEGER
    | FLOAT
    | DATE
    | LOCAL TIME
    | ZONED TIME
    | TIME WITH TIMEZONE
    | TIME WITHOUT TIMEZONE
    | LOCAL DATETIME
    | ZONED DATETIME
    | DATETIME WITH TIMEZONE
    | DATETIME WITHOUT TIMEZONE
    | TIMESTAMP WITH TIMEZONE
    | TIMESTAMP WITHOUT TIMEZONE
    | DURATION
    | POINT
    | NODE
    | RELATIONSHIP
    | MAP
    | LIST LT typeName GT
    | ARRAY LT typeName GT
    | PATH
    | ANY
    | PROPERTY VALUE
    | NOTHING
    ;

// Additional keywords for SHOW commands
FILTER      : F I L T E R ;
EXTRACT     : E X T R A C T ;
RANGE       : R A N G E ;
TEXT        : T E X T ;
FULLTEXT    : F U L L T E X T ;
VECTOR      : V E C T O R ;
LOOKUP      : L O O K U P ;
EACH        : E A C H ;
SECONDS     : S E C O N D S ;
NOWAIT      : N O W A I T ;
WAIT        : W A I T ;
DUMP        : D U M P ;
DESTROY     : D E S T R O Y ;
TOPOLOGY    : T O P O L O G Y ;
PRIMARY     : P R I M A R Y ;
PRIMARIES   : P R I M A R I E S ;
SECONDARY   : S E C O N D A R Y ;
SECONDARIES : S E C O N D A R I E S ;
READ        : R E A D ;
WRITE       : W R I T E ;
ONLY        : O N L Y ;
BUILT       : B U I L T ;
DEFINED     : D E F I N E D ;
POPULATED   : P O P U L A T E D ;
EXECUTABLE  : E X E C U T A B L E ;
ASSIGN      : A S S I G N ;
TRAVERSE    : T R A V E R S E ;
RELS        : R E L S ;
NODES       : N O D E S ;
RELATIONSHIPS : R E L A T I O N S H I P S ;
VALUE       : V A L U E ;
S           : S ;
ES          : E S ;
DRIVER      : D R I V E R ;
TARGET      : T A R G E T ;
AT          : A T ;
TIMEZONE    : T I M E Z O N E ;
TILDE       : '~' ;
WITHOUT     : W I T H O U T ;
