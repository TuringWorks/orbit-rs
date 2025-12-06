/**
 * RESP3 Combined Grammar for ANTLR4
 * 
 * Redis Serialization Protocol (RESP) - Combined Lexer and Parser
 * Supports RESP2 and RESP3 protocol versions
 * 
 * This is a combined grammar that includes both lexer and parser rules.
 * For more complex scenarios, use the separate RESPLexer.g4 and RESPParser.g4
 * 
 * Based on official Redis protocol specification:
 * https://redis.io/docs/latest/develop/reference/protocol-spec/
 * https://github.com/redis/redis-specifications/blob/master/protocol/RESP3.md
 * 
 * RESP Data Types:
 * +-------+-------------------+------------------------------------------+
 * | Byte  | Type              | Description                              |
 * +-------+-------------------+------------------------------------------+
 * | +     | Simple String     | Non-binary string                        |
 * | -     | Simple Error      | Error message                            |
 * | :     | Integer           | Signed 64-bit integer                    |
 * | $     | Bulk String       | Binary-safe string with length           |
 * | *     | Array             | Ordered collection                       |
 * | _     | Null (RESP3)      | Null value                               |
 * | #     | Boolean (RESP3)   | True or false                            |
 * | ,     | Double (RESP3)    | Double-precision float                   |
 * | (     | Big Number (RESP3)| Arbitrary precision integer              |
 * | !     | Bulk Error (RESP3)| Binary-safe error                        |
 * | =     | Verbatim (RESP3)  | String with encoding hint                |
 * | %     | Map (RESP3)       | Key-value dictionary                     |
 * | |     | Attribute (RESP3) | Out-of-band metadata                     |
 * | ~     | Set (RESP3)       | Unordered unique collection              |
 * | >     | Push (RESP3)      | Out-of-band push data                    |
 * +-------+-------------------+------------------------------------------+
 * 
 * Author: Generated for LLM coding assistance
 * License: MIT
 */

grammar RESP;

// =============================================================================
// Parser Rules
// =============================================================================

/** Entry point for parsing a stream of RESP messages */
stream
    : message* EOF
    ;

/** Single RESP message */
message
    : value CRLF?
    ;

/** Any RESP value */
value
    : simpleString
    | simpleError
    | integer
    | bulkString
    | array
    | null_
    | boolean_
    | double_
    | bigNumber
    | bulkError
    | verbatimString
    | map
    | attribute
    | set
    | push
    ;

// -----------------------------------------------------------------------------
// RESP2 Types
// -----------------------------------------------------------------------------

/** Simple String: +<string>\r\n */
simpleString
    : PLUS stringContent CRLF
    ;

/** Simple Error: -<error>\r\n */
simpleError
    : MINUS stringContent CRLF
    ;

/** Integer: :[+|-]<digits>\r\n */
integer
    : COLON SIGN? DIGITS CRLF
    ;

/** Bulk String: $<length>\r\n<data>\r\n or $-1\r\n for null */
bulkString
    : DOLLAR length CRLF (data CRLF)?
    ;

/** Array: *<count>\r\n<elements> or *-1\r\n for null */
array
    : ASTERISK length CRLF value*
    ;

// -----------------------------------------------------------------------------
// RESP3 Types
// -----------------------------------------------------------------------------

/** Null: _\r\n */
null_
    : UNDERSCORE CRLF
    ;

/** Boolean: #t\r\n or #f\r\n */
boolean_
    : HASH BOOLEAN_VALUE CRLF
    ;

/** Double: ,[+|-]<digits>[.digits][e[+|-]digits]\r\n or ,inf, ,-inf, ,nan */
double_
    : COMMA doubleValue CRLF
    ;

doubleValue
    : INF
    | NEG_INF
    | NAN
    | SIGN? DIGITS (DOT DIGITS)? (EXPONENT SIGN? DIGITS)?
    ;

/** Big Number: ([+|-]<digits>\r\n */
bigNumber
    : LPAREN SIGN? DIGITS CRLF
    ;

/** Bulk Error: !<length>\r\n<error>\r\n */
bulkError
    : EXCLAIM length CRLF data CRLF
    ;

/** Verbatim String: =<length>\r\n<enc>:<data>\r\n */
verbatimString
    : EQUALS length CRLF verbatimData CRLF
    ;

verbatimData
    : ENCODING_PREFIX data
    ;

/** Map: %<count>\r\n<key><value>... */
map
    : PERCENT length CRLF mapEntry*
    ;

mapEntry
    : value value
    ;

/** Attribute: |<count>\r\n<key><value>... */
attribute
    : PIPE length CRLF mapEntry*
    ;

/** Set: ~<count>\r\n<elements> */
set
    : TILDE length CRLF value*
    ;

/** Push: ><count>\r\n<elements> */
push
    : GT length CRLF value*
    ;

// -----------------------------------------------------------------------------
// Common Rules
// -----------------------------------------------------------------------------

length
    : MINUS? DIGITS
    ;

stringContent
    : STRING_CHAR*
    ;

data
    : DATA_CHAR*
    ;

// =============================================================================
// Lexer Rules
// =============================================================================

// Type prefix tokens
PLUS        : '+';
MINUS       : '-';
COLON       : ':';
DOLLAR      : '$';
ASTERISK    : '*';
UNDERSCORE  : '_';
HASH        : '#';
COMMA       : ',';
LPAREN      : '(';
EXCLAIM     : '!';
EQUALS      : '=';
PERCENT     : '%';
PIPE        : '|';
TILDE       : '~';
GT          : '>';
SEMICOLON   : ';';

// Common tokens
CRLF        : '\r\n';
DOT         : '.';
SIGN        : [+\-];

// Boolean values
BOOLEAN_VALUE
    : 't'
    | 'f'
    ;

// Special double values
INF         : 'inf';
NEG_INF     : '-inf';
NAN         : 'nan';

// Exponent marker
EXPONENT    : [eE];

// Digits
DIGITS      : [0-9]+;

// Verbatim encoding prefix (3 chars + colon)
ENCODING_PREFIX
    : [a-zA-Z] [a-zA-Z] [a-zA-Z] ':'
    ;

// String characters (no CR or LF)
STRING_CHAR
    : ~[\r\n]
    ;

// Data characters (binary safe, read by length)
DATA_CHAR
    : .
    ;

// Whitespace (usually not skipped in RESP)
WS          : [ \t]+ -> skip;
