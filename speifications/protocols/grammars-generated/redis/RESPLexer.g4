/**
 * RESP3 Lexer Grammar for ANTLR4
 * 
 * Redis Serialization Protocol (RESP) Lexer
 * Supports RESP2 and RESP3 protocol versions
 * 
 * Based on official Redis protocol specification:
 * https://redis.io/docs/latest/develop/reference/protocol-spec/
 * https://github.com/redis/redis-specifications/blob/master/protocol/RESP3.md
 * 
 * Author: Generated for LLM coding assistance
 * License: MIT
 */

lexer grammar RESPLexer;

// =============================================================================
// Type Prefix Tokens - First byte determines the type
// =============================================================================

// RESP2 Types
SIMPLE_STRING_PREFIX    : '+' -> pushMode(SIMPLE_LINE_MODE);
SIMPLE_ERROR_PREFIX     : '-' -> pushMode(SIMPLE_LINE_MODE);
INTEGER_PREFIX          : ':' -> pushMode(INTEGER_MODE);
BULK_STRING_PREFIX      : '$' -> pushMode(LENGTH_MODE);
ARRAY_PREFIX            : '*' -> pushMode(LENGTH_MODE);

// RESP3 Types
NULL_PREFIX             : '_' -> pushMode(NULL_MODE);
BOOLEAN_PREFIX          : '#' -> pushMode(BOOLEAN_MODE);
DOUBLE_PREFIX           : ',' -> pushMode(DOUBLE_MODE);
BIG_NUMBER_PREFIX       : '(' -> pushMode(BIG_NUMBER_MODE);
BULK_ERROR_PREFIX       : '!' -> pushMode(LENGTH_MODE);
VERBATIM_STRING_PREFIX  : '=' -> pushMode(LENGTH_MODE);
MAP_PREFIX              : '%' -> pushMode(LENGTH_MODE);
ATTRIBUTE_PREFIX        : '|' -> pushMode(LENGTH_MODE);
SET_PREFIX              : '~' -> pushMode(LENGTH_MODE);
PUSH_PREFIX             : '>' -> pushMode(LENGTH_MODE);

// Streamed string part (RESP3)
STREAMED_STRING_PREFIX  : ';' -> pushMode(LENGTH_MODE);

// =============================================================================
// Default Mode - Skip whitespace between messages (for testing/debugging)
// =============================================================================

CRLF                    : '\r\n';
WS                      : [ \t]+ -> skip;

// =============================================================================
// Simple Line Mode - For simple strings and errors
// =============================================================================

mode SIMPLE_LINE_MODE;

SIMPLE_LINE_CONTENT     : ~[\r\n]+;
SIMPLE_LINE_CRLF        : '\r\n' -> popMode;

// =============================================================================
// Integer Mode - For signed 64-bit integers
// =============================================================================

mode INTEGER_MODE;

INTEGER_SIGN            : [+\-];
INTEGER_DIGITS          : [0-9]+;
INTEGER_CRLF            : '\r\n' -> popMode;

// =============================================================================
// Length Mode - For bulk strings, arrays, maps, etc.
// =============================================================================

mode LENGTH_MODE;

LENGTH_VALUE            : '-'? [0-9]+;
LENGTH_CRLF             : '\r\n' -> popMode;

// =============================================================================
// Null Mode - RESP3 null type
// =============================================================================

mode NULL_MODE;

NULL_CRLF               : '\r\n' -> popMode;

// =============================================================================
// Boolean Mode - RESP3 boolean type
// =============================================================================

mode BOOLEAN_MODE;

BOOLEAN_TRUE            : 't';
BOOLEAN_FALSE           : 'f';
BOOLEAN_CRLF            : '\r\n' -> popMode;

// =============================================================================
// Double Mode - RESP3 double-precision floating point
// =============================================================================

mode DOUBLE_MODE;

DOUBLE_INF              : 'inf';
DOUBLE_NEG_INF          : '-inf';
DOUBLE_NAN              : 'nan';
DOUBLE_SIGN             : [+\-];
DOUBLE_DIGITS           : [0-9]+;
DOUBLE_DOT              : '.';
DOUBLE_EXPONENT         : [eE];
DOUBLE_CRLF             : '\r\n' -> popMode;

// =============================================================================
// Big Number Mode - RESP3 arbitrary precision integers
// =============================================================================

mode BIG_NUMBER_MODE;

BIG_NUMBER_SIGN         : [+\-];
BIG_NUMBER_DIGITS       : [0-9]+;
BIG_NUMBER_CRLF         : '\r\n' -> popMode;

// =============================================================================
// Bulk Data Mode - For reading binary bulk data
// =============================================================================

mode BULK_DATA_MODE;

// This mode is typically handled programmatically after parsing length
BULK_DATA_BYTE          : .;
BULK_DATA_CRLF          : '\r\n' -> popMode;
