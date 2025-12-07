/*
 * OrbitQL Lexer Grammar for ANTLR4
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

lexer grammar OrbitQLLexer;

options {
    caseInsensitive = true;
}

channels {
    COMMENTS_CHANNEL,
    WHITESPACE_CHANNEL
}

// =============================================================================
// KEYWORDS - Reserved Words
// =============================================================================

// A
ACTION: 'ACTION';
ADD: 'ADD';
AFTER: 'AFTER';
ALGORITHM: 'ALGORITHM';
ALL: 'ALL';
ALTER: 'ALTER';
AND: 'AND';
ARRAY: 'ARRAY';
AS: 'AS';
ASC: 'ASC';
AT: 'AT';

// B
BEFORE: 'BEFORE';
BEGIN: 'BEGIN';
BETWEEN: 'BETWEEN';
BIGINT: 'BIGINT';
BINARY: 'BINARY';
BOOLEAN: 'BOOLEAN';
BOTH: 'BOTH';
BUCKET: 'BUCKET';
BY: 'BY';

// C
CASCADE: 'CASCADE';
CASE: 'CASE';
CAST: 'CAST';
CATBOOST: 'CATBOOST';
CHECK: 'CHECK';
CLUSTER: 'CLUSTER';
COALESCE: 'COALESCE';
COLLATE: 'COLLATE';
COLUMN: 'COLUMN';
COLUMNS: 'COLUMNS';
COMMIT: 'COMMIT';
CONCAT: 'CONCAT';
CONFLICT: 'CONFLICT';
CONNECTED: 'CONNECTED';
CONSTRAINT: 'CONSTRAINT';
CONTAINS: 'CONTAINS';
COUNT: 'COUNT';
CREATE: 'CREATE';
CROSS: 'CROSS';
CURRENT: 'CURRENT';
CURRENT_DATE: 'CURRENT_DATE';
CURRENT_TIME: 'CURRENT_TIME';
CURRENT_TIMESTAMP: 'CURRENT_TIMESTAMP';

// D
DATA: 'DATA';
DATABASE: 'DATABASE';
DATE: 'DATE';
DATETIME: 'DATETIME';
DAY: 'DAY';
DAYS: 'DAYS';
DEC: 'DEC';
DECIMAL: 'DECIMAL';
DEFAULT: 'DEFAULT';
DELETE: 'DELETE';
DESC: 'DESC';
DIFF: 'DIFF';
DISTINCT: 'DISTINCT';
DO: 'DO';
DOUBLE: 'DOUBLE';
DROP: 'DROP';
DURATION: 'DURATION';

// E
EACH: 'EACH';
EDGE: 'EDGE';
ELSE: 'ELSE';
EMBED: 'EMBED';
END: 'END';
ENDS_WITH: 'ENDS_WITH';
ENTITIES: 'ENTITIES';
ESCAPE: 'ESCAPE';
EVALUATE: 'EVALUATE';
EVENT: 'EVENT';
EXCEPT: 'EXCEPT';
EXECUTE: 'EXECUTE';
EXISTS: 'EXISTS';
EXPLAIN: 'EXPLAIN';
EXTRACT: 'EXTRACT';

// F
FALSE: 'FALSE';
FEATURES: 'FEATURES';
FETCH: 'FETCH';
FILTER: 'FILTER';
FIRST: 'FIRST';
FIT: 'FIT';
FLOAT: 'FLOAT';
FOLLOWING: 'FOLLOWING';
FOR: 'FOR';
FORCE: 'FORCE';
FOREIGN: 'FOREIGN';
FORECAST: 'FORECAST';
FROM: 'FROM';
FULL: 'FULL';
FUNCTION: 'FUNCTION';

// G
GENERATED: 'GENERATED';
GEOMETRY: 'GEOMETRY';
GEOMETRYCOLLECTION: 'GEOMETRYCOLLECTION';
GLOBAL: 'GLOBAL';
GRADIENT: 'GRADIENT';
GRAPH: 'GRAPH';
GRAPHRAG: 'GRAPHRAG';
GROUP: 'GROUP';

// H
HAVING: 'HAVING';
HOUR: 'HOUR';
HOURS: 'HOURS';

// I
IF: 'IF';
ILIKE: 'ILIKE';
IMMEDIATE: 'IMMEDIATE';
IN: 'IN';
INBOUND: 'INBOUND';
INDEX: 'INDEX';
INNER: 'INNER';
INSERT: 'INSERT';
INT: 'INT';
INTEGER: 'INTEGER';
INTERSECT: 'INTERSECT';
INTERVAL: 'INTERVAL';
INTO: 'INTO';
IS: 'IS';

// J
JOIN: 'JOIN';
JSON: 'JSON';

// K
KEY: 'KEY';
KMEANS: 'KMEANS';

// L
LANGUAGE: 'LANGUAGE';
LAST: 'LAST';
LATERAL: 'LATERAL';
LEFT: 'LEFT';
LENGTH: 'LENGTH';
LIGHTGBM: 'LIGHTGBM';
LIKE: 'LIKE';
LIMIT: 'LIMIT';
LINEAR: 'LINEAR';
LINESTRING: 'LINESTRING';
LIST: 'LIST';
LIVE: 'LIVE';
LOCAL: 'LOCAL';
LOGISTIC: 'LOGISTIC';
LOWER: 'LOWER';

// M
MATCH: 'MATCH';
MAX: 'MAX';
MAX_DEPTH: 'MAX_DEPTH';
METRIC: 'METRIC';
MILLISECOND: 'MILLISECOND';
MILLISECONDS: 'MILLISECONDS';
MIN: 'MIN';
MINUTE: 'MINUTE';
MINUTES: 'MINUTES';
MODEL: 'MODEL';
MONTH: 'MONTH';
MONTHS: 'MONTHS';
MULTILINESTRING: 'MULTILINESTRING';
MULTIPOINT: 'MULTIPOINT';
MULTIPOLYGON: 'MULTIPOLYGON';

// N
NATURAL: 'NATURAL';
NODE: 'NODE';
NORMALIZE: 'NORMALIZE';
NOT: 'NOT';
NOTHING: 'NOTHING';
NOW: 'NOW';
NULL: 'NULL';
NULLIF: 'NULLIF';
NULLS: 'NULLS';
NUMERIC: 'NUMERIC';

// O
OBJECT: 'OBJECT';
OF: 'OF';
OFFSET: 'OFFSET';
ON: 'ON';
ONLY: 'ONLY';
OR: 'OR';
ORDER: 'ORDER';
OUTER: 'OUTER';
OUTBOUND: 'OUTBOUND';
OVER: 'OVER';

// P
PARALLEL: 'PARALLEL';
PARTITION: 'PARTITION';
PATH: 'PATH';
PATHS: 'PATHS';
PCA: 'PCA';
POINT: 'POINT';
POLYGON: 'POLYGON';
PRECEDING: 'PRECEDING';
PRECISION: 'PRECISION';
PREDICT: 'PREDICT';
PRIMARY: 'PRIMARY';
PROCEDURE: 'PROCEDURE';

// Q
QUERY: 'QUERY';

// R
RANDOM: 'RANDOM';
RANGE: 'RANGE';
REAL: 'REAL';
REASON: 'REASON';
RECURSIVE: 'RECURSIVE';
REFERENCES: 'REFERENCES';
REGRESSION: 'REGRESSION';
RELATE: 'RELATE';
REPLACE: 'REPLACE';
RESTRICT: 'RESTRICT';
RETURN: 'RETURN';
RETURNING: 'RETURNING';
RETURNS: 'RETURNS';
RIGHT: 'RIGHT';
ROLLBACK: 'ROLLBACK';
ROW: 'ROW';
ROWS: 'ROWS';

// S
SCHEMA: 'SCHEMA';
SCORE: 'SCORE';
SCROLL: 'SCROLL';
SEARCH: 'SEARCH';
SECOND: 'SECOND';
SECONDS: 'SECONDS';
SELECT: 'SELECT';
SEQUENCE: 'SEQUENCE';
SERIALIZABLE: 'SERIALIZABLE';
SERIES: 'SERIES';
SESSION: 'SESSION';
SET: 'SET';
SHARE: 'SHARE';
SHOW: 'SHOW';
SIMILAR: 'SIMILAR';
SIMILARITY: 'SIMILARITY';
SIMPLE: 'SIMPLE';
SIZE: 'SIZE';
SLIDE: 'SLIDE';
SMALLINT: 'SMALLINT';
SOME: 'SOME';
SPATIAL: 'SPATIAL';
SQL: 'SQL';
START: 'START';
STARTS_WITH: 'STARTS_WITH';
STATS: 'STATS';
STDDEV: 'STDDEV';
STEPS: 'STEPS';
STORED: 'STORED';
STRING: 'STRING';
SUBSTRING: 'SUBSTRING';
SUM: 'SUM';
SYMMETRIC: 'SYMMETRIC';

// T
TABLE: 'TABLE';
TARGET: 'TARGET';
TEMP: 'TEMP';
TEMPORARY: 'TEMPORARY';
TEXT: 'TEXT';
THEN: 'THEN';
TIME: 'TIME';
TIMEOUT: 'TIMEOUT';
TIMESTAMP: 'TIMESTAMP';
TIME_BUCKET: 'TIME_BUCKET';
TO: 'TO';
TRAIN: 'TRAIN';
TRANSACTION: 'TRANSACTION';
TRANSFORM: 'TRANSFORM';
TRAVERSE: 'TRAVERSE';
TRIGGER: 'TRIGGER';
TRIM: 'TRIM';
TRUE: 'TRUE';
TYPE: 'TYPE';

// U
UNBOUNDED: 'UNBOUNDED';
UNION: 'UNION';
UNIQUE: 'UNIQUE';
UPDATE: 'UPDATE';
UPPER: 'UPPER';
USING: 'USING';
UUID: 'UUID';

// V
VALUE: 'VALUE';
VALUES: 'VALUES';
VARCHAR: 'VARCHAR';
VARIANCE: 'VARIANCE';
VARYING: 'VARYING';
VECTOR: 'VECTOR';
VIEW: 'VIEW';
VIRTUAL: 'VIRTUAL';

// W
WATERMARK: 'WATERMARK';
WEEK: 'WEEK';
WEEKS: 'WEEKS';
WHEN: 'WHEN';
WHERE: 'WHERE';
WINDOW: 'WINDOW';
WITH: 'WITH';
WITHOUT: 'WITHOUT';
WORK: 'WORK';

// X
XGBOOST: 'XGBOOST';

// Y
YEAR: 'YEAR';
YEARS: 'YEARS';

// Z
ZONE: 'ZONE';

// =============================================================================
// ML FUNCTION KEYWORDS
// =============================================================================

ML_TRAIN_MODEL: 'ML_TRAIN_MODEL';
ML_PREDICT: 'ML_PREDICT';
ML_EVALUATE_MODEL: 'ML_EVALUATE_MODEL';
ML_DROP_MODEL: 'ML_DROP_MODEL';
ML_LIST_MODELS: 'ML_LIST_MODELS';
ML_MODEL_INFO: 'ML_MODEL_INFO';
ML_XGBOOST: 'ML_XGBOOST';
ML_LIGHTGBM: 'ML_LIGHTGBM';
ML_CATBOOST: 'ML_CATBOOST';
ML_ADABOOST: 'ML_ADABOOST';
ML_GRADIENT_BOOSTING: 'ML_GRADIENT_BOOSTING';
ML_LINEAR_REGRESSION: 'ML_LINEAR_REGRESSION';
ML_LOGISTIC_REGRESSION: 'ML_LOGISTIC_REGRESSION';
ML_RANDOM_FOREST: 'ML_RANDOM_FOREST';
ML_KMEANS: 'ML_KMEANS';
ML_PCA: 'ML_PCA';
ML_NORMALIZE: 'ML_NORMALIZE';
ML_ENCODE_CATEGORICAL: 'ML_ENCODE_CATEGORICAL';
ML_EMBED_TEXT: 'ML_EMBED_TEXT';
ML_SIMILARITY_SEARCH: 'ML_SIMILARITY_SEARCH';
ML_VECTOR_CLUSTER: 'ML_VECTOR_CLUSTER';
ML_FORECAST: 'ML_FORECAST';
ML_ANOMALY_DETECTION: 'ML_ANOMALY_DETECTION';
ML_CORRELATION: 'ML_CORRELATION';
ML_FEATURE_SELECTION: 'ML_FEATURE_SELECTION';

// =============================================================================
// SPATIAL FUNCTION KEYWORDS
// =============================================================================

ST_POINT: 'ST_POINT';
ST_MAKEPOINT: 'ST_MAKEPOINT';
ST_LINESTRING: 'ST_LINESTRING';
ST_POLYGON: 'ST_POLYGON';
ST_GEOMFROMTEXT: 'ST_GEOMFROMTEXT';
ST_GEOMFROMGEOJSON: 'ST_GEOMFROMGEOJSON';
ST_ASTEXT: 'ST_ASTEXT';
ST_ASGEOJSON: 'ST_ASGEOJSON';
ST_ASWKB: 'ST_ASWKB';
ST_CONTAINS: 'ST_CONTAINS';
ST_WITHIN: 'ST_WITHIN';
ST_INTERSECTS: 'ST_INTERSECTS';
ST_OVERLAPS: 'ST_OVERLAPS';
ST_TOUCHES: 'ST_TOUCHES';
ST_CROSSES: 'ST_CROSSES';
ST_DISJOINT: 'ST_DISJOINT';
ST_EQUALS: 'ST_EQUALS';
ST_DWITHIN: 'ST_DWITHIN';
ST_DISTANCE: 'ST_DISTANCE';
ST_LENGTH: 'ST_LENGTH';
ST_AREA: 'ST_AREA';
ST_PERIMETER: 'ST_PERIMETER';
ST_BUFFER: 'ST_BUFFER';
ST_CENTROID: 'ST_CENTROID';
ST_ENVELOPE: 'ST_ENVELOPE';
ST_UNION: 'ST_UNION';
ST_INTERSECTION: 'ST_INTERSECTION';
ST_DIFFERENCE: 'ST_DIFFERENCE';
ST_SIMPLIFY: 'ST_SIMPLIFY';
ST_TRANSFORM: 'ST_TRANSFORM';
ST_SRID: 'ST_SRID';
ST_X: 'ST_X';
ST_Y: 'ST_Y';
ST_Z: 'ST_Z';
ST_KMEANS: 'ST_KMEANS';

// =============================================================================
// AGGREGATE FUNCTION KEYWORDS
// =============================================================================

AVG: 'AVG';

// =============================================================================
// OPERATORS
// =============================================================================

// Comparison operators
EQUALS: '=';
NOT_EQUALS: '<>' | '!=';
LESS_THAN: '<';
GREATER_THAN: '>';
LESS_THAN_OR_EQUALS: '<=';
GREATER_THAN_OR_EQUALS: '>=';

// Arithmetic operators
PLUS: '+';
MINUS: '-';
ASTERISK: '*';
SLASH: '/';
PERCENT: '%';
CARET: '^';

// Compound assignment operators
PLUS_EQUALS: '+=';
MINUS_EQUALS: '-=';
ASTERISK_EQUALS: '*=';
SLASH_EQUALS: '/=';

// Logical operators (as keywords above)

// JSON operators
JSON_EXTRACT: '->';
JSON_EXTRACT_TEXT: '->>';
JSON_PATH_EXTRACT: '#>';
JSON_PATH_EXTRACT_TEXT: '#>>';
JSON_CONTAINS: '@>';
JSON_CONTAINED_BY: '<@';

// Graph edge operators
ARROW_RIGHT: '->';
ARROW_LEFT: '<-';
ARROW_BOTH: '<->';

// Array operators
ARRAY_CONCAT: '||';

// Range operator
DOUBLE_DOT: '..';

// Type cast operator
DOUBLE_COLON: '::';

// =============================================================================
// PUNCTUATION
// =============================================================================

OPEN_PAREN: '(';
CLOSE_PAREN: ')';
OPEN_BRACKET: '[';
CLOSE_BRACKET: ']';
OPEN_BRACE: '{';
CLOSE_BRACE: '}';
COMMA: ',';
SEMICOLON: ';';
DOT: '.';
COLON: ':';
AT_SIGN: '@';
DOLLAR: '$';
HASH: '#';
QUESTION_MARK: '?';
UNDERSCORE: '_';
BACKTICK: '`';
AMPERSAND: '&';
PIPE: '|';
TILDE: '~';
EXCLAMATION: '!';

// =============================================================================
// LITERALS
// =============================================================================

// Integer literal
INTEGER_LITERAL
    : DIGIT+
    ;

// Floating-point literal
FLOAT_LITERAL
    : DIGIT+ DOT DIGIT* EXPONENT?
    | DOT DIGIT+ EXPONENT?
    | DIGIT+ EXPONENT
    ;

// Duration literal (short form: 3h, 30m, 7d, 1y)
DURATION_LITERAL
    : DIGIT+ [hHmMsS]
    | DIGIT+ [dD]
    | DIGIT+ ('ms' | 'MS')
    ;

// String literal (single-quoted)
STRING_LITERAL
    : '\'' ( ~'\'' | '\'\'' )* '\''
    ;

// Double-quoted string literal
DOUBLE_QUOTED_STRING
    : '"' ( ~'"' | '""' )* '"'
    ;

// Dollar-quoted string (PostgreSQL-style)
DOLLAR_STRING
    : DOLLAR_TAG .*? DOLLAR_TAG
    ;

fragment DOLLAR_TAG
    : '$' IDENTIFIER_BODY? '$'
    ;

// Hex string literal
HEX_STRING
    : [xX] '\'' HEX_DIGIT+ '\''
    ;

// Binary string literal
BIT_STRING
    : [bB] '\'' [01]+ '\''
    ;

// =============================================================================
// IDENTIFIERS
// =============================================================================

// Regular identifier
IDENTIFIER
    : IDENTIFIER_START IDENTIFIER_BODY*
    ;

// Quoted identifier (double quotes)
QUOTED_IDENTIFIER
    : '"' ( ~'"' | '""' )+ '"'
    ;

// Backtick-quoted identifier (MySQL-style)
BACKTICK_IDENTIFIER
    : '`' ( ~'`' | '``' )+ '`'
    ;

// Parameter placeholder
NAMED_PARAMETER
    : '@' IDENTIFIER_BODY+
    | '$' IDENTIFIER_BODY+
    | ':' IDENTIFIER_BODY+
    ;

// Positional parameter
POSITIONAL_PARAMETER
    : '$' DIGIT+
    ;

// Record ID (table:id format used in graph operations)
RECORD_ID
    : IDENTIFIER_BODY+ ':' IDENTIFIER_BODY+
    ;

// =============================================================================
// FRAGMENTS
// =============================================================================

fragment IDENTIFIER_START
    : [a-zA-Z_]
    | '\u0080'..'\uFFFF'  // Extended Unicode support
    ;

fragment IDENTIFIER_BODY
    : [a-zA-Z0-9_]
    | '\u0080'..'\uFFFF'
    ;

fragment DIGIT
    : [0-9]
    ;

fragment HEX_DIGIT
    : [0-9a-fA-F]
    ;

fragment EXPONENT
    : [eE] [+-]? DIGIT+
    ;

// =============================================================================
// COMMENTS
// =============================================================================

// Single-line comment
LINE_COMMENT
    : '--' ~[\r\n]* -> channel(COMMENTS_CHANNEL)
    ;

// Multi-line comment
BLOCK_COMMENT
    : '/*' .*? '*/' -> channel(COMMENTS_CHANNEL)
    ;

// =============================================================================
// WHITESPACE
// =============================================================================

WHITESPACE
    : [ \t\r\n]+ -> channel(WHITESPACE_CHANNEL)
    ;

// =============================================================================
// CATCH-ALL FOR UNKNOWN CHARACTERS
// =============================================================================

UNKNOWN_CHAR
    : .
    ;
