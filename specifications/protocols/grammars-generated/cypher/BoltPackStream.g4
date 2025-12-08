/*
 * BoltPackStream.g4 - ANTLR4 Grammar for Neo4j Bolt Protocol PackStream
 * 
 * This grammar defines the textual representation of Bolt Protocol messages
 * for debugging, logging, testing, and documentation purposes.
 * 
 * The actual Bolt Protocol is a binary protocol (PackStream).
 * This grammar parses human-readable representations of Bolt messages.
 * 
 * Based on Neo4j Bolt Protocol Specification (versions 1.0 - 5.x)
 * 
 * Licensed under Apache License 2.0
 */

grammar BoltPackStream;

// =============================================================================
// Top-level Rules
// =============================================================================

// A stream of Bolt messages
boltStream
    : message* EOF
    ;

// Single Bolt message
message
    : requestMessage
    | responseMessage
    ;

// =============================================================================
// Request Messages (Client -> Server)
// =============================================================================

requestMessage
    : helloMessage
    | logonMessage
    | logoffMessage
    | goodbyeMessage
    | resetMessage
    | runMessage
    | discardMessage
    | pullMessage
    | beginMessage
    | commitMessage
    | rollbackMessage
    | routeMessage
    | telemetryMessage
    // Legacy messages (Bolt v1-v2)
    | initMessage
    | ackFailureMessage
    ;

// HELLO message (Bolt 3+)
helloMessage
    : HELLO extra=dictionary
    ;

// INIT message (Bolt 1-2, replaced by HELLO in v3)
initMessage
    : INIT userAgent=stringLiteral authToken=dictionary
    ;

// LOGON message (Bolt 5.1+)
logonMessage
    : LOGON auth=dictionary
    ;

// LOGOFF message (Bolt 5.1+)
logoffMessage
    : LOGOFF
    ;

// GOODBYE message (Bolt 3+)
goodbyeMessage
    : GOODBYE
    ;

// RESET message
resetMessage
    : RESET
    ;

// ACK_FAILURE message (Bolt 1-2, replaced by RESET in v3)
ackFailureMessage
    : ACK_FAILURE
    ;

// RUN message
runMessage
    : RUN query=stringLiteral parameters=dictionary ( extra=dictionary )?
    ;

// DISCARD message (DISCARD_ALL in Bolt 1-3)
discardMessage
    : ( DISCARD | DISCARD_ALL ) ( extra=dictionary )?
    ;

// PULL message (PULL_ALL in Bolt 1-3)
pullMessage
    : ( PULL | PULL_ALL ) ( extra=dictionary )?
    ;

// BEGIN message (Bolt 3+)
beginMessage
    : BEGIN extra=dictionary
    ;

// COMMIT message (Bolt 3+)
commitMessage
    : COMMIT
    ;

// ROLLBACK message (Bolt 3+)
rollbackMessage
    : ROLLBACK
    ;

// ROUTE message (Bolt 4.3+)
routeMessage
    : ROUTE routing=dictionary bookmarks=list ( database=stringLiteral | extra=dictionary )
    ;

// TELEMETRY message (Bolt 5.4+)
telemetryMessage
    : TELEMETRY api=integerLiteral
    ;

// =============================================================================
// Response Messages (Server -> Client)
// =============================================================================

responseMessage
    : successMessage
    | failureMessage
    | ignoredMessage
    | recordMessage
    ;

// SUCCESS message
successMessage
    : SUCCESS metadata=dictionary
    ;

// FAILURE message
failureMessage
    : FAILURE metadata=dictionary
    ;

// IGNORED message
ignoredMessage
    : IGNORED
    ;

// RECORD message
recordMessage
    : RECORD data=list
    ;

// =============================================================================
// PackStream Data Types
// =============================================================================

// Any PackStream value
value
    : nullLiteral
    | booleanLiteral
    | integerLiteral
    | floatLiteral
    | stringLiteral
    | byteArray
    | list
    | dictionary
    | structure
    ;

// Null
nullLiteral
    : NULL
    ;

// Boolean
booleanLiteral
    : TRUE
    | FALSE
    ;

// Integer (signed 64-bit)
integerLiteral
    : MINUS? INTEGER
    | HEX_INTEGER
    ;

// Float (64-bit IEEE 754)
floatLiteral
    : MINUS? FLOAT
    | INFINITY
    | MINUS INFINITY
    | NAN
    ;

// String (UTF-8)
stringLiteral
    : STRING
    ;

// Byte array
byteArray
    : BYTES LBRACKET ( HEX_BYTE ( COMMA HEX_BYTE )* )? RBRACKET
    ;

// List
list
    : LBRACKET ( value ( COMMA value )* )? RBRACKET
    ;

// Dictionary (Map)
dictionary
    : LBRACE ( dictionaryEntry ( COMMA dictionaryEntry )* )? RBRACE
    ;

dictionaryEntry
    : key=stringLiteral COLON val=value
    ;

// Structure (PackStream extension type)
structure
    : STRUCTURE LPAREN tag=integerLiteral RPAREN LBRACE ( value ( COMMA value )* )? RBRACE
    | structureType LPAREN ( value ( COMMA value )* )? RPAREN
    ;

// Named structure types (Bolt semantics)
structureType
    : NODE
    | RELATIONSHIP
    | UNBOUNDED_RELATIONSHIP
    | PATH
    | DATE
    | TIME
    | LOCAL_TIME
    | DATETIME
    | DATETIME_ZONE_ID
    | LOCAL_DATETIME
    | DURATION
    | POINT_2D
    | POINT_3D
    ;

// =============================================================================
// Bolt Structure Semantics
// =============================================================================

/*
 * Structure Tags (as defined in Bolt Protocol):
 * 
 * Graph Types:
 *   0x4E (78)  - Node
 *   0x52 (82)  - Relationship
 *   0x72 (114) - UnboundRelationship
 *   0x50 (80)  - Path
 * 
 * Temporal Types:
 *   0x44 (68)  - Date
 *   0x54 (84)  - Time
 *   0x74 (116) - LocalTime
 *   0x49 (73)  - DateTime (with timezone offset) [Bolt 5.0+]
 *   0x69 (105) - DateTimeZoneId [Bolt 5.0+]
 *   0x64 (100) - LocalDateTime
 *   0x46 (70)  - DateTime (legacy, Bolt 1-4.4)
 *   0x66 (102) - DateTimeZoneId (legacy, Bolt 1-4.4)
 *   0x45 (69)  - Duration
 * 
 * Spatial Types:
 *   0x58 (88)  - Point2D
 *   0x59 (89)  - Point3D
 * 
 * Message Types (Tag Bytes):
 *   0x01 - HELLO/INIT
 *   0x02 - GOODBYE
 *   0x0E - ACK_FAILURE
 *   0x0F - RESET
 *   0x10 - RUN
 *   0x11 - BEGIN
 *   0x12 - COMMIT
 *   0x13 - ROLLBACK
 *   0x2F - DISCARD
 *   0x3F - PULL
 *   0x54 - TELEMETRY
 *   0x66 - ROUTE
 *   0x6A - LOGON
 *   0x6B - LOGOFF
 *   0x70 - SUCCESS
 *   0x71 - RECORD
 *   0x7E - IGNORED
 *   0x7F - FAILURE
 */

// =============================================================================
// Lexer Rules
// =============================================================================

// Message type keywords
HELLO       : 'HELLO' ;
INIT        : 'INIT' ;
LOGON       : 'LOGON' ;
LOGOFF      : 'LOGOFF' ;
GOODBYE     : 'GOODBYE' ;
RESET       : 'RESET' ;
ACK_FAILURE : 'ACK_FAILURE' ;
RUN         : 'RUN' ;
DISCARD     : 'DISCARD' ;
DISCARD_ALL : 'DISCARD_ALL' ;
PULL        : 'PULL' ;
PULL_ALL    : 'PULL_ALL' ;
BEGIN       : 'BEGIN' ;
COMMIT      : 'COMMIT' ;
ROLLBACK    : 'ROLLBACK' ;
ROUTE       : 'ROUTE' ;
TELEMETRY   : 'TELEMETRY' ;

// Response keywords
SUCCESS     : 'SUCCESS' ;
FAILURE     : 'FAILURE' ;
IGNORED     : 'IGNORED' ;
RECORD      : 'RECORD' ;

// Structure type keywords
NODE        : 'Node' ;
RELATIONSHIP : 'Relationship' ;
UNBOUNDED_RELATIONSHIP : 'UnboundRelationship' ;
PATH        : 'Path' ;
DATE        : 'Date' ;
TIME        : 'Time' ;
LOCAL_TIME  : 'LocalTime' ;
DATETIME    : 'DateTime' ;
DATETIME_ZONE_ID : 'DateTimeZoneId' ;
LOCAL_DATETIME : 'LocalDateTime' ;
DURATION    : 'Duration' ;
POINT_2D    : 'Point2D' | 'Point' ;
POINT_3D    : 'Point3D' ;

// Generic structure
STRUCTURE   : 'Structure' ;
BYTES       : 'Bytes' | 'bytes' | 'b' ;

// Literal keywords
NULL        : 'null' | 'NULL' ;
TRUE        : 'true' | 'TRUE' ;
FALSE       : 'false' | 'FALSE' ;
INFINITY    : 'Infinity' | 'INFINITY' | 'inf' | 'INF' ;
NAN         : 'NaN' | 'NAN' | 'nan' ;

// Punctuation
LBRACE      : '{' ;
RBRACE      : '}' ;
LBRACKET    : '[' ;
RBRACKET    : ']' ;
LPAREN      : '(' ;
RPAREN      : ')' ;
COLON       : ':' ;
COMMA       : ',' ;
MINUS       : '-' ;

// Numeric literals
INTEGER
    : '0'
    | [1-9] [0-9]*
    ;

HEX_INTEGER
    : '0' [xX] [0-9a-fA-F]+
    ;

FLOAT
    : [0-9]+ '.' [0-9]* EXPONENT?
    | '.' [0-9]+ EXPONENT?
    | [0-9]+ EXPONENT
    ;

fragment EXPONENT
    : [eE] [+-]? [0-9]+
    ;

// Hex byte (for byte arrays)
HEX_BYTE
    : [0-9a-fA-F] [0-9a-fA-F]
    ;

// String literal
STRING
    : '"' ( ESC | ~["\\\r\n] )* '"'
    | '\'' ( ESC | ~['\\\r\n] )* '\''
    ;

fragment ESC
    : '\\' [btnfr"'\\]
    | '\\' 'u' [0-9a-fA-F] [0-9a-fA-F] [0-9a-fA-F] [0-9a-fA-F]
    ;

// Whitespace and comments
WS
    : [ \t\r\n]+ -> skip
    ;

LINE_COMMENT
    : '//' ~[\r\n]* -> skip
    ;

BLOCK_COMMENT
    : '/*' .*? '*/' -> skip
    ;

// Client/Server markers (for protocol traces)
CLIENT_MARKER
    : 'C:' -> skip
    ;

SERVER_MARKER
    : 'S:' -> skip
    ;
