/*
 * MongoDB Query Language (MQL) Lexer Grammar
 * ANTLR4 lexer for parsing MongoDB queries, aggregation pipelines, and updates
 */

lexer grammar MongoLexer;

// =============================================================================
// Structural Tokens
// =============================================================================

LBRACE          : '{' ;
RBRACE          : '}' ;
LBRACKET        : '[' ;
RBRACKET        : ']' ;
LPAREN          : '(' ;
RPAREN          : ')' ;
COLON           : ':' ;
COMMA           : ',' ;
DOT             : '.' ;
DOLLAR          : '$' ;

// =============================================================================
// Query Operators
// =============================================================================

// Comparison Operators
OP_EQ           : '$eq' ;
OP_GT           : '$gt' ;
OP_GTE          : '$gte' ;
OP_LT           : '$lt' ;
OP_LTE          : '$lte' ;
OP_NE           : '$ne' ;
OP_IN           : '$in' ;
OP_NIN          : '$nin' ;

// Logical Operators
OP_AND          : '$and' ;
OP_OR           : '$or' ;
OP_NOT          : '$not' ;
OP_NOR          : '$nor' ;

// Element Operators
OP_EXISTS       : '$exists' ;
OP_TYPE         : '$type' ;

// Evaluation Operators
OP_EXPR         : '$expr' ;
OP_JSONSCHEMA   : '$jsonSchema' ;
OP_MOD          : '$mod' ;
OP_REGEX        : '$regex' ;
OP_OPTIONS      : '$options' ;
OP_TEXT         : '$text' ;
OP_WHERE        : '$where' ;
OP_SEARCH       : '$search' ;
OP_LANGUAGE     : '$language' ;
OP_CASESENSITIVE: '$caseSensitive' ;
OP_DIACRITICSENSITIVE : '$diacriticSensitive' ;

// Array Operators
OP_ALL          : '$all' ;
OP_ELEMMATCH    : '$elemMatch' ;
OP_SIZE         : '$size' ;

// Bitwise Operators
OP_BITSALLCLEAR : '$bitsAllClear' ;
OP_BITSALLSET   : '$bitsAllSet' ;
OP_BITSANYCLEAR : '$bitsAnyClear' ;
OP_BITSANYSET   : '$bitsAnySet' ;

// Geospatial Operators
OP_GEOWITHIN    : '$geoWithin' ;
OP_GEOINTERSECTS: '$geoIntersects' ;
OP_NEAR         : '$near' ;
OP_NEARSPHERE   : '$nearSphere' ;
OP_GEOMETRY     : '$geometry' ;
OP_MAXDISTANCE  : '$maxDistance' ;
OP_MINDISTANCE  : '$minDistance' ;
OP_CENTER       : '$center' ;
OP_CENTERSPHERE : '$centerSphere' ;
OP_BOX          : '$box' ;
OP_POLYGON      : '$polygon' ;

// =============================================================================
// Update Operators
// =============================================================================

// Field Update Operators
OP_SET          : '$set' ;
OP_SETONINSERT  : '$setOnInsert' ;
OP_UNSET        : '$unset' ;
OP_INC          : '$inc' ;
OP_MUL          : '$mul' ;
OP_RENAME       : '$rename' ;
OP_MIN          : '$min' ;
OP_MAX          : '$max' ;
OP_CURRENTDATE  : '$currentDate' ;

// Array Update Operators
OP_PUSH         : '$push' ;
OP_PULL         : '$pull' ;
OP_PULLALL      : '$pullAll' ;
OP_POP          : '$pop' ;
OP_ADDTOSET     : '$addToSet' ;
OP_EACH         : '$each' ;
OP_SLICE        : '$slice' ;
OP_SORT         : '$sort' ;
OP_POSITION     : '$position' ;

// Bitwise Update Operators
OP_BIT          : '$bit' ;

// =============================================================================
// Aggregation Pipeline Stages
// =============================================================================

STAGE_ADDFIELDS     : '$addFields' ;
STAGE_BUCKET        : '$bucket' ;
STAGE_BUCKETAUTO    : '$bucketAuto' ;
STAGE_COLLSTATS     : '$collStats' ;
STAGE_COUNT         : '$count' ;
STAGE_CURRENTOP     : '$currentOp' ;
STAGE_DENSIFY       : '$densify' ;
STAGE_DOCUMENTS     : '$documents' ;
STAGE_FACET         : '$facet' ;
STAGE_FILL          : '$fill' ;
STAGE_GEONEAR       : '$geoNear' ;
STAGE_GRAPHLOOKUP   : '$graphLookup' ;
STAGE_GROUP         : '$group' ;
STAGE_INDEXSTATS    : '$indexStats' ;
STAGE_LIMIT         : '$limit' ;
STAGE_LISTLOCALSESSIONS : '$listLocalSessions' ;
STAGE_LISTSESSIONS  : '$listSessions' ;
STAGE_LOOKUP        : '$lookup' ;
STAGE_MATCH         : '$match' ;
STAGE_MERGE         : '$merge' ;
STAGE_OUT           : '$out' ;
STAGE_PLANSTATS     : '$planCacheStats' ;
STAGE_PROJECT       : '$project' ;
STAGE_REDACT        : '$redact' ;
STAGE_REPLACEROOT   : '$replaceRoot' ;
STAGE_REPLACEWITH   : '$replaceWith' ;
STAGE_SAMPLE        : '$sample' ;
STAGE_SEARCH        : '$search' ;
STAGE_SEARCHMETA    : '$searchMeta' ;
STAGE_SET           : '$set' ;
STAGE_SETWINDOWFIELDS : '$setWindowFields' ;
STAGE_SKIP          : '$skip' ;
STAGE_SORT          : '$sort' ;
STAGE_SORTBYCOUNT   : '$sortByCount' ;
STAGE_UNIONWITH     : '$unionWith' ;
STAGE_UNSET         : '$unset' ;
STAGE_UNWIND        : '$unwind' ;
STAGE_VECTORSEARCH  : '$vectorSearch' ;
STAGE_RANKFUSION    : '$rankFusion' ;
STAGE_SCOREFUSION   : '$scoreFusion' ;

// =============================================================================
// Aggregation Expressions
// =============================================================================

// Arithmetic Expressions
EXPR_ABS            : '$abs' ;
EXPR_ADD            : '$add' ;
EXPR_CEIL           : '$ceil' ;
EXPR_DIVIDE         : '$divide' ;
EXPR_EXP            : '$exp' ;
EXPR_FLOOR          : '$floor' ;
EXPR_LN             : '$ln' ;
EXPR_LOG            : '$log' ;
EXPR_LOG10          : '$log10' ;
EXPR_MOD            : '$mod' ;
EXPR_MULTIPLY       : '$multiply' ;
EXPR_POW            : '$pow' ;
EXPR_ROUND          : '$round' ;
EXPR_SQRT           : '$sqrt' ;
EXPR_SUBTRACT       : '$subtract' ;
EXPR_TRUNC          : '$trunc' ;

// Array Expressions
EXPR_ARRAYELEMAT    : '$arrayElemAt' ;
EXPR_ARRAYTOOBJECT  : '$arrayToObject' ;
EXPR_CONCATARRAYS   : '$concatArrays' ;
EXPR_FILTER         : '$filter' ;
EXPR_FIRST          : '$first' ;
EXPR_INDEXOFARRAY   : '$indexOfArray' ;
EXPR_ISARRAY        : '$isArray' ;
EXPR_LAST           : '$last' ;
EXPR_MAP            : '$map' ;
EXPR_OBJECTTOARRAY  : '$objectToArray' ;
EXPR_RANGE          : '$range' ;
EXPR_REDUCE         : '$reduce' ;
EXPR_REVERSEARR     : '$reverseArray' ;
EXPR_SIZEARR        : '$size' ;
EXPR_SLICEARR       : '$slice' ;
EXPR_ZIP            : '$zip' ;

// Boolean Expressions
EXPR_ANDEXPR        : '$and' ;
EXPR_NOTEXPR        : '$not' ;
EXPR_OREXPR         : '$or' ;

// Comparison Expressions
EXPR_CMP            : '$cmp' ;
EXPR_EQEXPR         : '$eq' ;
EXPR_GTEXPR         : '$gt' ;
EXPR_GTEEXPR        : '$gte' ;
EXPR_LTEXPR         : '$lt' ;
EXPR_LTEEXPR        : '$lte' ;
EXPR_NEEXPR         : '$ne' ;

// Conditional Expressions
EXPR_COND           : '$cond' ;
EXPR_IFNULL         : '$ifNull' ;
EXPR_SWITCH         : '$switch' ;

// Date Expressions
EXPR_DATEADD        : '$dateAdd' ;
EXPR_DATEDIFF       : '$dateDiff' ;
EXPR_DATEFROMPARTS  : '$dateFromParts' ;
EXPR_DATEFROMSTRING : '$dateFromString' ;
EXPR_DATESUBTRACT   : '$dateSubtract' ;
EXPR_DATETOPARTS    : '$dateToParts' ;
EXPR_DATETOSTRING   : '$dateToString' ;
EXPR_DATETRUNC      : '$dateTrunc' ;
EXPR_DAYOFMONTH     : '$dayOfMonth' ;
EXPR_DAYOFWEEK      : '$dayOfWeek' ;
EXPR_DAYOFYEAR      : '$dayOfYear' ;
EXPR_HOUR           : '$hour' ;
EXPR_ISODAYOFWEEK   : '$isoDayOfWeek' ;
EXPR_ISOWEEK        : '$isoWeek' ;
EXPR_ISOWEEKWYEAR   : '$isoWeekYear' ;
EXPR_MILLISECOND    : '$millisecond' ;
EXPR_MINUTE         : '$minute' ;
EXPR_MONTH          : '$month' ;
EXPR_SECOND         : '$second' ;
EXPR_TOISODATE      : '$toDate' ;
EXPR_WEEK           : '$week' ;
EXPR_YEAR           : '$year' ;

// Literal Expression
EXPR_LITERAL        : '$literal' ;

// Object Expressions
EXPR_MERGEOBJECTS   : '$mergeObjects' ;
EXPR_SETFIELD       : '$setField' ;
EXPR_GETFIELD       : '$getField' ;
EXPR_UNSETFIELD     : '$unsetField' ;

// Set Expressions
EXPR_ALLELEMENTSTRUE: '$allElementsTrue' ;
EXPR_ANYELEMENTTRUE : '$anyElementTrue' ;
EXPR_SETDIFF        : '$setDifference' ;
EXPR_SETEQUALS      : '$setEquals' ;
EXPR_SETINTERSECTION: '$setIntersection' ;
EXPR_SETISSUBSET    : '$setIsSubset' ;
EXPR_SETUNION       : '$setUnion' ;

// String Expressions
EXPR_CONCAT         : '$concat' ;
EXPR_INDEXOFBYTES   : '$indexOfBytes' ;
EXPR_INDEXOFCP      : '$indexOfCP' ;
EXPR_LTRIM          : '$ltrim' ;
EXPR_REGEXFIND      : '$regexFind' ;
EXPR_REGEXFINDALL   : '$regexFindAll' ;
EXPR_REGEXMATCH     : '$regexMatch' ;
EXPR_REPLACEONE     : '$replaceOne' ;
EXPR_REPLACEALL     : '$replaceAll' ;
EXPR_RTRIM          : '$rtrim' ;
EXPR_SPLIT          : '$split' ;
EXPR_STRCASECMP     : '$strcasecmp' ;
EXPR_STRLEN         : '$strLenBytes' ;
EXPR_STRLENCP       : '$strLenCP' ;
EXPR_SUBSTR         : '$substr' ;
EXPR_SUBSTRBYTES    : '$substrBytes' ;
EXPR_SUBSTRCP       : '$substrCP' ;
EXPR_TOLOWER        : '$toLower' ;
EXPR_TOSTRING       : '$toString' ;
EXPR_TRIM           : '$trim' ;
EXPR_TOUPPER        : '$toUpper' ;

// Type Expressions
EXPR_CONVERT        : '$convert' ;
EXPR_ISBOOL         : '$isBoolean' ;
EXPR_ISDATE         : '$isDate' ;
EXPR_ISNUMBER       : '$isNumber' ;
EXPR_TOBOOL         : '$toBool' ;
EXPR_TOBINDATA      : '$toBinData' ;
EXPR_TODECIMAL      : '$toDecimal' ;
EXPR_TODOUBLE       : '$toDouble' ;
EXPR_TOINT          : '$toInt' ;
EXPR_TOLONG         : '$toLong' ;
EXPR_TOOBJECTID     : '$toObjectId' ;
EXPR_TOUUID         : '$toUUID' ;
EXPR_TYPEEXPR       : '$type' ;

// Accumulators (for $group and window functions)
ACCUM_AVG           : '$avg' ;
ACCUM_BOTTOM        : '$bottom' ;
ACCUM_BOTTOMN       : '$bottomN' ;
ACCUM_COUNT         : '$count' ;
ACCUM_FIRSTN        : '$firstN' ;
ACCUM_LASTN         : '$lastN' ;
ACCUM_MAXN          : '$maxN' ;
ACCUM_MEDIAN        : '$median' ;
ACCUM_MINN          : '$minN' ;
ACCUM_PERCENTILE    : '$percentile' ;
ACCUM_PUSHACC       : '$push' ;
ACCUM_STDDEVPOP     : '$stdDevPop' ;
ACCUM_STDDEVSAMP    : '$stdDevSamp' ;
ACCUM_SUM           : '$sum' ;
ACCUM_TOP           : '$top' ;
ACCUM_TOPN          : '$topN' ;

// Window Functions
WINDOW_DENSERANK    : '$denseRank' ;
WINDOW_DERIVATIVE   : '$derivative' ;
WINDOW_DOCNUMBER    : '$documentNumber' ;
WINDOW_EXPMOVING    : '$expMovingAvg' ;
WINDOW_INTEGRAL     : '$integral' ;
WINDOW_LINEARFILL   : '$linearFill' ;
WINDOW_LOCF         : '$locf' ;
WINDOW_RANK         : '$rank' ;
WINDOW_SHIFT        : '$shift' ;

// Miscellaneous Expressions
EXPR_LET            : '$let' ;
EXPR_RAND           : '$rand' ;
EXPR_SAMPLERATE     : '$sampleRate' ;
EXPR_META           : '$meta' ;

// Bitwise Aggregation Expressions (MongoDB 6.3+)
EXPR_BITAND         : '$bitAnd' ;
EXPR_BITOR          : '$bitOr' ;
EXPR_BITXOR         : '$bitXor' ;
EXPR_BITNOT         : '$bitNot' ;

// Additional Expressions
EXPR_GETFIELD       : '$getField' ;
EXPR_SETFIELD       : '$setField' ;
EXPR_UNSETFIELD     : '$unsetField' ;
EXPR_SORTARRAY      : '$sortArray' ;
EXPR_MAXN           : '$maxN' ;
EXPR_MINN           : '$minN' ;
EXPR_FIRSTN         : '$firstN' ;
EXPR_LASTN          : '$lastN' ;
EXPR_TOPN           : '$topN' ;
EXPR_BOTTOMN        : '$bottomN' ;

// =============================================================================
// BSON Types
// =============================================================================

OBJECTID        : 'ObjectId' ;
NUMBERINT       : 'NumberInt' ;
NUMBERLONG      : 'NumberLong' ;
NUMBERDECIMAL   : 'NumberDecimal' ;
ISODATE         : 'ISODate' ;
DATE            : 'Date' ;
TIMESTAMP       : 'Timestamp' ;
BINDATA         : 'BinData' ;
UUID            : 'UUID' ;
MINKEY          : 'MinKey' ;
MAXKEY          : 'MaxKey' ;
DBREF           : 'DBRef' ;
CODE            : 'Code' ;
CODEWSCOPE      : 'CodeWithScope' ;
REGEX           : 'RegExp' ;

// =============================================================================
// Keywords
// =============================================================================

TRUE            : 'true' ;
FALSE           : 'false' ;
NULL            : 'null' ;
UNDEFINED       : 'undefined' ;
INFINITY        : 'Infinity' ;
NAN             : 'NaN' ;
NEW             : 'new' ;

// =============================================================================
// Literals
// =============================================================================

// Integer (including negative)
INTEGER
    : '-'? INT
    ;

// Floating point number
DECIMAL
    : '-'? INT '.' [0-9]+ EXP?
    | '-'? INT EXP
    | '-'? '.' [0-9]+ EXP?
    ;

// Hexadecimal
HEX
    : '0' [xX] HEX_DIGIT+
    ;

// String literals
STRING
    : '"' (ESC | SAFECODEPOINT)* '"'
    | '\'' (ESC_SINGLE | SAFECODEPOINT_SINGLE)* '\''
    ;

// Regular expression literal
REGEX_LITERAL
    : '/' REGEX_BODY '/' REGEX_FLAGS?
    ;

// =============================================================================
// Identifiers
// =============================================================================

// Field reference (starts with $)
FIELD_REF
    : '$' IDENTIFIER_CHARS ('.' IDENTIFIER_CHARS)*
    ;

// Variable reference (starts with $$)
VAR_REF
    : '$$' IDENTIFIER_CHARS
    ;

// General identifier
IDENTIFIER
    : IDENTIFIER_START IDENTIFIER_CHARS*
    ;

// Quoted identifier (for field names with special chars)
QUOTED_IDENTIFIER
    : '"' (~["\\\r\n] | ESC)+ '"'
    | '\'' (~['\\\r\n] | ESC_SINGLE)+ '\''
    ;

// =============================================================================
// Comments and Whitespace
// =============================================================================

// Single-line comment
LINE_COMMENT
    : '//' ~[\r\n]* -> skip
    ;

// Multi-line comment
BLOCK_COMMENT
    : '/*' .*? '*/' -> skip
    ;

// Whitespace
WS
    : [ \t\r\n\u000C]+ -> skip
    ;

// =============================================================================
// Fragment Rules
// =============================================================================

fragment INT
    : '0'
    | [1-9] [0-9]*
    ;

fragment EXP
    : [eE] [+\-]? INT
    ;

fragment HEX_DIGIT
    : [0-9a-fA-F]
    ;

fragment ESC
    : '\\' (["\\/bfnrt] | UNICODE)
    ;

fragment ESC_SINGLE
    : '\\' (['\\/bfnrt] | UNICODE)
    ;

fragment UNICODE
    : 'u' HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT
    ;

fragment SAFECODEPOINT
    : ~["\\\u0000-\u001F]
    ;

fragment SAFECODEPOINT_SINGLE
    : ~['\\\u0000-\u001F]
    ;

fragment IDENTIFIER_START
    : [a-zA-Z_]
    ;

fragment IDENTIFIER_CHARS
    : [a-zA-Z0-9_]
    ;

fragment REGEX_BODY
    : (REGEX_CHAR | REGEX_ESCAPE)+
    ;

fragment REGEX_CHAR
    : ~[/\\\r\n]
    ;

fragment REGEX_ESCAPE
    : '\\' .
    ;

fragment REGEX_FLAGS
    : [gimsuy]+
    ;
