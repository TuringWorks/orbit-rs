/**
 * RESP3 Parser Grammar for ANTLR4
 * 
 * Redis Serialization Protocol (RESP) Parser
 * Supports RESP2 and RESP3 protocol versions
 * 
 * Based on official Redis protocol specification:
 * https://redis.io/docs/latest/develop/reference/protocol-spec/
 * https://github.com/redis/redis-specifications/blob/master/protocol/RESP3.md
 * 
 * RESP Data Types Summary:
 * +-----------------------+------------------+----------+------------+
 * | Type                  | Protocol Version | Category | First Byte |
 * +-----------------------+------------------+----------+------------+
 * | Simple strings        | RESP2            | Simple   | +          |
 * | Simple Errors         | RESP2            | Simple   | -          |
 * | Integers              | RESP2            | Simple   | :          |
 * | Bulk strings          | RESP2            | Aggregate| $          |
 * | Arrays                | RESP2            | Aggregate| *          |
 * | Nulls                 | RESP3            | Simple   | _          |
 * | Booleans              | RESP3            | Simple   | #          |
 * | Doubles               | RESP3            | Simple   | ,          |
 * | Big numbers           | RESP3            | Simple   | (          |
 * | Bulk errors           | RESP3            | Aggregate| !          |
 * | Verbatim strings      | RESP3            | Aggregate| =          |
 * | Maps                  | RESP3            | Aggregate| %          |
 * | Attributes            | RESP3            | Aggregate| |          |
 * | Sets                  | RESP3            | Aggregate| ~          |
 * | Pushes                | RESP3            | Aggregate| >          |
 * +-----------------------+------------------+----------+------------+
 * 
 * Author: Generated for LLM coding assistance
 * License: MIT
 */

parser grammar RESPParser;

options {
    tokenVocab = RESPLexer;
}

// =============================================================================
// Entry Points
// =============================================================================

/**
 * A RESP stream consists of one or more RESP values
 */
respStream
    : respValue+ EOF
    ;

/**
 * A single RESP message/response
 */
respMessage
    : respValue EOF
    ;

// =============================================================================
// RESP Value - Any valid RESP type
// =============================================================================

/**
 * A RESP value can be any of the supported types
 */
respValue
    : simpleString          // +OK\r\n
    | simpleError           // -ERR message\r\n
    | integer               // :1000\r\n
    | bulkString            // $5\r\nhello\r\n
    | array                 // *2\r\n...
    | null_                 // _\r\n (RESP3)
    | boolean_              // #t\r\n or #f\r\n (RESP3)
    | double_               // ,1.23\r\n (RESP3)
    | bigNumber             // (3492890328409238509324850943850943825024385\r\n (RESP3)
    | bulkError             // !21\r\nSYNTAX invalid syntax\r\n (RESP3)
    | verbatimString        // =15\r\ntxt:Some string\r\n (RESP3)
    | map                   // %2\r\n... (RESP3)
    | attribute             // |1\r\n... (RESP3)
    | set                   // ~3\r\n... (RESP3)
    | push                  // >3\r\n... (RESP3)
    ;

// =============================================================================
// RESP2 Types
// =============================================================================

/**
 * Simple String: +<string>\r\n
 * Cannot contain CR or LF characters
 * Example: +OK\r\n
 */
simpleString
    : SIMPLE_STRING_PREFIX simpleStringContent SIMPLE_LINE_CRLF
    ;

simpleStringContent
    : SIMPLE_LINE_CONTENT?
    ;

/**
 * Simple Error: -<error message>\r\n
 * First word is error type (e.g., ERR, WRONGTYPE)
 * Example: -ERR unknown command 'foo'\r\n
 */
simpleError
    : SIMPLE_ERROR_PREFIX simpleErrorContent SIMPLE_LINE_CRLF
    ;

simpleErrorContent
    : SIMPLE_LINE_CONTENT?
    ;

/**
 * Integer: :[<+|->]<value>\r\n
 * Signed 64-bit integer
 * Example: :1000\r\n, :-50\r\n
 */
integer
    : INTEGER_PREFIX integerValue INTEGER_CRLF
    ;

integerValue
    : INTEGER_SIGN? INTEGER_DIGITS
    ;

/**
 * Bulk String: $<length>\r\n<data>\r\n
 * Binary-safe string with length prefix
 * Example: $5\r\nhello\r\n
 * Null: $-1\r\n (RESP2 null representation)
 */
bulkString
    : BULK_STRING_PREFIX bulkLength LENGTH_CRLF bulkData?
    ;

bulkLength
    : LENGTH_VALUE
    ;

bulkData
    : bulkDataContent CRLF
    ;

bulkDataContent
    : ~CRLF*
    ;

/**
 * Array: *<count>\r\n<element-1>...<element-n>
 * Ordered collection of RESP values
 * Example: *2\r\n$5\r\nhello\r\n$5\r\nworld\r\n
 * Null array: *-1\r\n (RESP2 null representation)
 */
array
    : ARRAY_PREFIX arrayLength LENGTH_CRLF arrayElements?
    ;

arrayLength
    : LENGTH_VALUE
    ;

arrayElements
    : respValue+
    ;

// =============================================================================
// RESP3 Types
// =============================================================================

/**
 * Null: _\r\n
 * Represents non-existent values
 */
null_
    : NULL_PREFIX NULL_CRLF
    ;

/**
 * Boolean: #<t|f>\r\n
 * Example: #t\r\n (true), #f\r\n (false)
 */
boolean_
    : BOOLEAN_PREFIX booleanValue BOOLEAN_CRLF
    ;

booleanValue
    : BOOLEAN_TRUE
    | BOOLEAN_FALSE
    ;

/**
 * Double: ,<floating-point>\r\n
 * Double-precision floating point
 * Supports inf, -inf, nan
 * Example: ,1.23\r\n, ,inf\r\n, ,-inf\r\n, ,nan\r\n
 */
double_
    : DOUBLE_PREFIX doubleValue DOUBLE_CRLF
    ;

doubleValue
    : DOUBLE_INF                                              // inf
    | DOUBLE_NEG_INF                                          // -inf
    | DOUBLE_NAN                                              // nan
    | DOUBLE_SIGN? DOUBLE_DIGITS doubleDecimal? doubleExponent?  // [+|-]digits[.digits][e[+|-]digits]
    ;

doubleDecimal
    : DOUBLE_DOT DOUBLE_DIGITS
    ;

doubleExponent
    : DOUBLE_EXPONENT DOUBLE_SIGN? DOUBLE_DIGITS
    ;

/**
 * Big Number: ([+|-]<number>\r\n
 * Arbitrary precision integer
 * Example: (3492890328409238509324850943850943825024385\r\n
 */
bigNumber
    : BIG_NUMBER_PREFIX bigNumberValue BIG_NUMBER_CRLF
    ;

bigNumberValue
    : BIG_NUMBER_SIGN? BIG_NUMBER_DIGITS
    ;

/**
 * Bulk Error: !<length>\r\n<error>\r\n
 * Error with binary-safe message
 * Example: !21\r\nSYNTAX invalid syntax\r\n
 */
bulkError
    : BULK_ERROR_PREFIX bulkLength LENGTH_CRLF bulkData?
    ;

/**
 * Verbatim String: =<length>\r\n<encoding>:<data>\r\n
 * String with encoding hint (3-char encoding + colon + data)
 * Example: =15\r\ntxt:Some string\r\n
 */
verbatimString
    : VERBATIM_STRING_PREFIX bulkLength LENGTH_CRLF verbatimData?
    ;

verbatimData
    : verbatimEncoding ':' verbatimContent CRLF
    ;

verbatimEncoding
    : . . .  // Exactly 3 characters
    ;

verbatimContent
    : ~CRLF*
    ;

/**
 * Map: %<count>\r\n<key-1><value-1>...<key-n><value-n>
 * Key-value pairs (dictionary/hash)
 * Example: %2\r\n+first\r\n:1\r\n+second\r\n:2\r\n
 */
map
    : MAP_PREFIX mapLength LENGTH_CRLF mapEntries?
    ;

mapLength
    : LENGTH_VALUE
    ;

mapEntries
    : mapEntry+
    ;

mapEntry
    : respValue respValue  // key, value
    ;

/**
 * Attribute: |<count>\r\n<key-1><value-1>...<key-n><value-n>
 * Out-of-band auxiliary data (like map but for metadata)
 * Example: |1\r\n+key-popularity\r\n%2\r\n...
 */
attribute
    : ATTRIBUTE_PREFIX attributeLength LENGTH_CRLF attributeEntries?
    ;

attributeLength
    : LENGTH_VALUE
    ;

attributeEntries
    : attributeEntry+
    ;

attributeEntry
    : respValue respValue  // key, value
    ;

/**
 * Set: ~<count>\r\n<element-1>...<element-n>
 * Unordered collection of unique elements
 * Example: ~3\r\n:1\r\n:2\r\n:3\r\n
 */
set
    : SET_PREFIX setLength LENGTH_CRLF setElements?
    ;

setLength
    : LENGTH_VALUE
    ;

setElements
    : respValue+
    ;

/**
 * Push: ><count>\r\n<element-1>...<element-n>
 * Out-of-band push data (Pub/Sub, invalidations, etc.)
 * First element is always the push type
 * Example: >3\r\n$7\r\nmessage\r\n$7\r\nchannel\r\n$7\r\npayload\r\n
 */
push
    : PUSH_PREFIX pushLength LENGTH_CRLF pushElements?
    ;

pushLength
    : LENGTH_VALUE
    ;

pushElements
    : respValue+  // First element is push type
    ;

// =============================================================================
// Streamed Types (RESP3 Extension)
// =============================================================================

/**
 * Streamed String Part: ;<length>\r\n<data>\r\n
 * Part of a streamed bulk string
 * Empty part (;0\r\n) signals end of stream
 */
streamedStringPart
    : STREAMED_STRING_PREFIX bulkLength LENGTH_CRLF bulkData?
    ;

// =============================================================================
// Redis Command (Client to Server)
// =============================================================================

/**
 * Redis commands are sent as arrays of bulk strings
 * Example: *3\r\n$3\r\nSET\r\n$3\r\nkey\r\n$5\r\nvalue\r\n
 */
redisCommand
    : array
    ;

/**
 * Inline command format (for telnet sessions)
 * Example: PING
 */
inlineCommand
    : SIMPLE_LINE_CONTENT SIMPLE_LINE_CRLF
    ;
