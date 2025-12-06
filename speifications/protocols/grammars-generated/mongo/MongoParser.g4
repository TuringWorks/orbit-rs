/*
 * MongoDB Query Language (MQL) Parser Grammar
 * ANTLR4 parser for parsing MongoDB queries, aggregation pipelines, and updates
 */

parser grammar MongoParser;

options {
    tokenVocab = MongoLexer;
}

// =============================================================================
// Entry Points
// =============================================================================

// Main entry point - can be any MongoDB operation
mongoStatement
    : query                     // Find query
    | aggregationPipeline       // Aggregation pipeline
    | updateDocument            // Update operation
    | document                  // General document/object
    | EOF
    ;

// =============================================================================
// Document Structure
// =============================================================================

document
    : LBRACE documentBody? RBRACE
    ;

documentBody
    : pair (COMMA pair)*
    ;

pair
    : key COLON value
    ;

key
    : STRING
    | QUOTED_IDENTIFIER
    | IDENTIFIER
    | operatorKey
    | stageKey
    | expressionKey
    | accumulatorKey
    ;

value
    : literal
    | document
    | array
    | fieldReference
    | variableReference
    | bsonType
    | queryOperator
    | updateOperator
    | aggregationExpression
    | accumulatorExpression
    ;

array
    : LBRACKET arrayElements? RBRACKET
    ;

arrayElements
    : value (COMMA value)*
    ;

// =============================================================================
// Query Operations
// =============================================================================

query
    : LBRACE queryBody? RBRACE
    ;

queryBody
    : queryCondition (COMMA queryCondition)*
    ;

queryCondition
    : fieldQuery
    | logicalQuery
    | evaluationQuery
    | commentQuery
    ;

// Field-level query
fieldQuery
    : fieldName COLON fieldValue
    ;

fieldName
    : STRING
    | QUOTED_IDENTIFIER
    | IDENTIFIER
    | dottedFieldName
    ;

dottedFieldName
    : IDENTIFIER (DOT (IDENTIFIER | INTEGER))+
    ;

fieldValue
    : literal
    | document
    | array
    | queryOperatorExpression
    | bsonType
    ;

// =============================================================================
// Query Operators
// =============================================================================

queryOperator
    : comparisonOperator
    | logicalOperator
    | elementOperator
    | evaluationOperator
    | arrayQueryOperator
    | bitwiseQueryOperator
    | geospatialOperator
    ;

queryOperatorExpression
    : LBRACE queryOperatorPair (COMMA queryOperatorPair)* RBRACE
    ;

queryOperatorPair
    : comparisonOperator COLON value
    | elementOperator COLON value
    | evaluationOperator COLON value
    | arrayQueryOperator COLON value
    | bitwiseQueryOperator COLON value
    | geospatialOperator COLON value
    | OP_NOT COLON queryOperatorExpression
    ;

// Comparison operators
comparisonOperator
    : OP_EQ
    | OP_GT
    | OP_GTE
    | OP_LT
    | OP_LTE
    | OP_NE
    | OP_IN
    | OP_NIN
    ;

// Logical operators
logicalOperator
    : OP_AND
    | OP_OR
    | OP_NOT
    | OP_NOR
    ;

logicalQuery
    : OP_AND COLON LBRACKET queryArray RBRACKET
    | OP_OR COLON LBRACKET queryArray RBRACKET
    | OP_NOR COLON LBRACKET queryArray RBRACKET
    ;

queryArray
    : query (COMMA query)*
    ;

// Element operators
elementOperator
    : OP_EXISTS
    | OP_TYPE
    ;

// Evaluation operators
evaluationOperator
    : OP_EXPR
    | OP_JSONSCHEMA
    | OP_MOD
    | OP_REGEX
    | OP_OPTIONS
    | OP_TEXT
    | OP_WHERE
    ;

evaluationQuery
    : OP_EXPR COLON aggregationExpression
    | OP_TEXT COLON textSearchDocument
    | OP_WHERE COLON (STRING | functionExpression)
    | OP_JSONSCHEMA COLON document
    ;

textSearchDocument
    : LBRACE textSearchBody RBRACE
    ;

textSearchBody
    : textSearchPair (COMMA textSearchPair)*
    ;

textSearchPair
    : OP_SEARCH COLON STRING
    | OP_LANGUAGE COLON STRING
    | OP_CASESENSITIVE COLON booleanLiteral
    | OP_DIACRITICSENSITIVE COLON booleanLiteral
    ;

// Array query operators
arrayQueryOperator
    : OP_ALL
    | OP_ELEMMATCH
    | OP_SIZE
    ;

// Bitwise query operators
bitwiseQueryOperator
    : OP_BITSALLCLEAR
    | OP_BITSALLSET
    | OP_BITSANYCLEAR
    | OP_BITSANYSET
    ;

// Geospatial operators
geospatialOperator
    : OP_GEOWITHIN
    | OP_GEOINTERSECTS
    | OP_NEAR
    | OP_NEARSPHERE
    ;

geospatialQuery
    : geospatialOperator COLON geospatialDocument
    ;

geospatialDocument
    : LBRACE geospatialBody RBRACE
    ;

geospatialBody
    : geospatialPair (COMMA geospatialPair)*
    ;

geospatialPair
    : OP_GEOMETRY COLON geometryDocument
    | OP_MAXDISTANCE COLON numericLiteral
    | OP_MINDISTANCE COLON numericLiteral
    | OP_CENTER COLON array
    | OP_CENTERSPHERE COLON array
    | OP_BOX COLON array
    | OP_POLYGON COLON array
    ;

geometryDocument
    : LBRACE geometryBody RBRACE
    ;

geometryBody
    : geometryPair (COMMA geometryPair)*
    ;

geometryPair
    : IDENTIFIER COLON value
    ;

commentQuery
    : STRING COLON value  // For $comment or custom fields
    ;

// =============================================================================
// Update Operations
// =============================================================================

updateDocument
    : LBRACE updateBody RBRACE
    ;

updateBody
    : updatePair (COMMA updatePair)*
    ;

updatePair
    : fieldUpdateOperator COLON document
    | arrayUpdateOperator COLON document
    | bitwiseUpdateOperator COLON document
    ;

// Field update operators
fieldUpdateOperator
    : OP_SET
    | OP_SETONINSERT
    | OP_UNSET
    | OP_INC
    | OP_MUL
    | OP_RENAME
    | OP_MIN
    | OP_MAX
    | OP_CURRENTDATE
    ;

// Array update operators
arrayUpdateOperator
    : OP_PUSH
    | OP_PULL
    | OP_PULLALL
    | OP_POP
    | OP_ADDTOSET
    ;

// Array update modifiers
arrayUpdateModifier
    : OP_EACH
    | OP_SLICE
    | OP_SORT
    | OP_POSITION
    ;

// Bitwise update operators
bitwiseUpdateOperator
    : OP_BIT
    ;

updateOperator
    : fieldUpdateOperator
    | arrayUpdateOperator
    | bitwiseUpdateOperator
    | arrayUpdateModifier
    ;

// =============================================================================
// Aggregation Pipeline
// =============================================================================

aggregationPipeline
    : LBRACKET pipelineStages? RBRACKET
    ;

pipelineStages
    : pipelineStage (COMMA pipelineStage)*
    ;

pipelineStage
    : LBRACE stageKey COLON stageValue RBRACE
    ;

stageKey
    : STAGE_ADDFIELDS
    | STAGE_BUCKET
    | STAGE_BUCKETAUTO
    | STAGE_COLLSTATS
    | STAGE_COUNT
    | STAGE_CURRENTOP
    | STAGE_DENSIFY
    | STAGE_DOCUMENTS
    | STAGE_FACET
    | STAGE_FILL
    | STAGE_GEONEAR
    | STAGE_GRAPHLOOKUP
    | STAGE_GROUP
    | STAGE_INDEXSTATS
    | STAGE_LIMIT
    | STAGE_LISTLOCALSESSIONS
    | STAGE_LISTSESSIONS
    | STAGE_LOOKUP
    | STAGE_MATCH
    | STAGE_MERGE
    | STAGE_OUT
    | STAGE_PLANSTATS
    | STAGE_PROJECT
    | STAGE_RANKFUSION
    | STAGE_REDACT
    | STAGE_REPLACEROOT
    | STAGE_REPLACEWITH
    | STAGE_SAMPLE
    | STAGE_SCOREFUSION
    | STAGE_SEARCH
    | STAGE_SEARCHMETA
    | STAGE_SET
    | STAGE_SETWINDOWFIELDS
    | STAGE_SKIP
    | STAGE_SORT
    | STAGE_SORTBYCOUNT
    | STAGE_UNIONWITH
    | STAGE_UNSET
    | STAGE_UNWIND
    | STAGE_VECTORSEARCH
    ;

stageValue
    : document
    | array
    | STRING
    | fieldReference
    | numericLiteral
    ;

// $match stage
matchStage
    : STAGE_MATCH COLON query
    ;

// $project stage
projectStage
    : STAGE_PROJECT COLON projectDocument
    ;

projectDocument
    : LBRACE projectBody RBRACE
    ;

projectBody
    : projectPair (COMMA projectPair)*
    ;

projectPair
    : fieldName COLON projectValue
    ;

projectValue
    : numericLiteral           // 0 or 1 for inclusion/exclusion
    | booleanLiteral           // true/false for inclusion/exclusion
    | fieldReference           // Field reference
    | aggregationExpression    // Computed field
    | STRING                   // Literal string rename
    ;

// $group stage
groupStage
    : STAGE_GROUP COLON groupDocument
    ;

groupDocument
    : LBRACE groupBody RBRACE
    ;

groupBody
    : idPair (COMMA accumulatorPair)*
    ;

idPair
    : '_id' COLON groupIdValue
    | STRING COLON groupIdValue  // For "_id" as string
    ;

groupIdValue
    : NULL
    | fieldReference
    | document
    | aggregationExpression
    ;

accumulatorPair
    : fieldName COLON accumulatorDocument
    ;

accumulatorDocument
    : LBRACE accumulatorKey COLON accumulatorValue RBRACE
    ;

accumulatorKey
    : ACCUM_AVG
    | ACCUM_BOTTOM
    | ACCUM_BOTTOMN
    | ACCUM_COUNT
    | ACCUM_FIRSTN
    | ACCUM_LASTN
    | ACCUM_MAXN
    | ACCUM_MEDIAN
    | ACCUM_MINN
    | ACCUM_PERCENTILE
    | ACCUM_PUSHACC
    | ACCUM_STDDEVPOP
    | ACCUM_STDDEVSAMP
    | ACCUM_SUM
    | ACCUM_TOP
    | ACCUM_TOPN
    | EXPR_FIRST
    | EXPR_LAST
    | OP_MAX
    | OP_MIN
    ;

accumulatorValue
    : fieldReference
    | aggregationExpression
    | numericLiteral
    | document
    | array
    ;

accumulatorExpression
    : LBRACE accumulatorKey COLON accumulatorValue RBRACE
    ;

// $sort stage
sortStage
    : STAGE_SORT COLON sortDocument
    ;

sortDocument
    : LBRACE sortBody RBRACE
    ;

sortBody
    : sortPair (COMMA sortPair)*
    ;

sortPair
    : fieldName COLON sortDirection
    ;

sortDirection
    : INTEGER       // 1 or -1
    | document      // { $meta: "textScore" }
    ;

// $lookup stage
lookupStage
    : STAGE_LOOKUP COLON lookupDocument
    ;

lookupDocument
    : LBRACE lookupBody RBRACE
    ;

lookupBody
    : lookupPair (COMMA lookupPair)*
    ;

lookupPair
    : 'from' COLON STRING
    | 'localField' COLON STRING
    | 'foreignField' COLON STRING
    | 'as' COLON STRING
    | 'let' COLON document
    | 'pipeline' COLON aggregationPipeline
    | IDENTIFIER COLON value
    ;

// $unwind stage
unwindStage
    : STAGE_UNWIND COLON unwindValue
    ;

unwindValue
    : fieldReference
    | STRING
    | unwindDocument
    ;

unwindDocument
    : LBRACE unwindBody RBRACE
    ;

unwindBody
    : unwindPair (COMMA unwindPair)*
    ;

unwindPair
    : 'path' COLON (fieldReference | STRING)
    | 'includeArrayIndex' COLON STRING
    | 'preserveNullAndEmptyArrays' COLON booleanLiteral
    | IDENTIFIER COLON value
    ;

// $graphLookup stage
graphLookupStage
    : STAGE_GRAPHLOOKUP COLON graphLookupDocument
    ;

graphLookupDocument
    : LBRACE graphLookupBody RBRACE
    ;

graphLookupBody
    : graphLookupPair (COMMA graphLookupPair)*
    ;

graphLookupPair
    : 'from' COLON STRING
    | 'startWith' COLON (fieldReference | aggregationExpression)
    | 'connectFromField' COLON STRING
    | 'connectToField' COLON STRING
    | 'as' COLON STRING
    | 'maxDepth' COLON INTEGER
    | 'depthField' COLON STRING
    | 'restrictSearchWithMatch' COLON query
    | IDENTIFIER COLON value
    ;

// $bucket stage
bucketStage
    : STAGE_BUCKET COLON bucketDocument
    ;

bucketDocument
    : LBRACE bucketBody RBRACE
    ;

bucketBody
    : bucketPair (COMMA bucketPair)*
    ;

bucketPair
    : 'groupBy' COLON (fieldReference | aggregationExpression)
    | 'boundaries' COLON array
    | 'default' COLON literal
    | 'output' COLON document
    | IDENTIFIER COLON value
    ;

// $facet stage
facetStage
    : STAGE_FACET COLON facetDocument
    ;

facetDocument
    : LBRACE facetBody RBRACE
    ;

facetBody
    : facetPair (COMMA facetPair)*
    ;

facetPair
    : fieldName COLON aggregationPipeline
    ;

// $setWindowFields stage
setWindowFieldsStage
    : STAGE_SETWINDOWFIELDS COLON setWindowFieldsDocument
    ;

setWindowFieldsDocument
    : LBRACE setWindowFieldsBody RBRACE
    ;

setWindowFieldsBody
    : setWindowFieldsPair (COMMA setWindowFieldsPair)*
    ;

setWindowFieldsPair
    : 'partitionBy' COLON (fieldReference | document)
    | 'sortBy' COLON sortDocument
    | 'output' COLON windowOutputDocument
    | IDENTIFIER COLON value
    ;

windowOutputDocument
    : LBRACE windowOutputBody RBRACE
    ;

windowOutputBody
    : windowOutputPair (COMMA windowOutputPair)*
    ;

windowOutputPair
    : fieldName COLON windowFunction
    ;

windowFunction
    : LBRACE windowFunctionBody RBRACE
    ;

windowFunctionBody
    : windowFunctionPair (COMMA windowFunctionPair)*
    ;

windowFunctionPair
    : windowFunctionKey COLON value
    | 'window' COLON windowSpec
    ;

windowFunctionKey
    : WINDOW_DENSERANK
    | WINDOW_DERIVATIVE
    | WINDOW_DOCNUMBER
    | WINDOW_EXPMOVING
    | WINDOW_INTEGRAL
    | WINDOW_LINEARFILL
    | WINDOW_LOCF
    | WINDOW_RANK
    | WINDOW_SHIFT
    | accumulatorKey
    ;

windowSpec
    : LBRACE windowSpecBody RBRACE
    ;

windowSpecBody
    : windowSpecPair (COMMA windowSpecPair)*
    ;

windowSpecPair
    : 'documents' COLON array
    | 'range' COLON array
    | 'unit' COLON STRING
    | IDENTIFIER COLON value
    ;

// $vectorSearch stage (MongoDB Atlas / MongoDB 8.2+ Community)
vectorSearchStage
    : STAGE_VECTORSEARCH COLON vectorSearchDocument
    ;

vectorSearchDocument
    : LBRACE vectorSearchBody RBRACE
    ;

vectorSearchBody
    : vectorSearchPair (COMMA vectorSearchPair)*
    ;

vectorSearchPair
    : 'index' COLON STRING
    | 'path' COLON (STRING | fieldReference)
    | 'queryVector' COLON array
    | 'numCandidates' COLON INTEGER
    | 'limit' COLON INTEGER
    | 'filter' COLON document
    | 'exact' COLON booleanLiteral
    | IDENTIFIER COLON value
    ;

// $rankFusion stage (MongoDB 8.0+)
rankFusionStage
    : STAGE_RANKFUSION COLON rankFusionDocument
    ;

rankFusionDocument
    : LBRACE rankFusionBody RBRACE
    ;

rankFusionBody
    : rankFusionPair (COMMA rankFusionPair)*
    ;

rankFusionPair
    : 'input' COLON rankFusionInputDocument
    | 'scoreDetails' COLON booleanLiteral
    | IDENTIFIER COLON value
    ;

rankFusionInputDocument
    : LBRACE rankFusionInput (COMMA rankFusionInput)* RBRACE
    ;

rankFusionInput
    : IDENTIFIER COLON rankFusionPipelineSpec
    ;

rankFusionPipelineSpec
    : LBRACE rankFusionPipelinePair (COMMA rankFusionPipelinePair)* RBRACE
    ;

rankFusionPipelinePair
    : 'pipeline' COLON aggregationPipeline
    | 'weight' COLON numericLiteral
    | IDENTIFIER COLON value
    ;

// $scoreFusion stage (MongoDB 8.2+)
scoreFusionStage
    : STAGE_SCOREFUSION COLON scoreFusionDocument
    ;

scoreFusionDocument
    : LBRACE scoreFusionBody RBRACE
    ;

scoreFusionBody
    : scoreFusionPair (COMMA scoreFusionPair)*
    ;

scoreFusionPair
    : 'input' COLON scoreFusionInputDocument
    | 'combination' COLON combinationDocument
    | 'scoreDetails' COLON booleanLiteral
    | IDENTIFIER COLON value
    ;

scoreFusionInputDocument
    : LBRACE scoreFusionInput (COMMA scoreFusionInput)* RBRACE
    ;

scoreFusionInput
    : IDENTIFIER COLON scoreFusionPipelineSpec
    ;

scoreFusionPipelineSpec
    : LBRACE scoreFusionPipelinePair (COMMA scoreFusionPipelinePair)* RBRACE
    ;

scoreFusionPipelinePair
    : 'pipeline' COLON aggregationPipeline
    | 'weight' COLON numericLiteral
    | 'normalization' COLON normalizationDocument
    | IDENTIFIER COLON value
    ;

combinationDocument
    : LBRACE combinationPair (COMMA combinationPair)* RBRACE
    ;

combinationPair
    : 'method' COLON STRING
    | 'expression' COLON aggregationExpression
    | IDENTIFIER COLON value
    ;

normalizationDocument
    : LBRACE normalizationPair (COMMA normalizationPair)* RBRACE
    ;

normalizationPair
    : 'method' COLON STRING
    | IDENTIFIER COLON value
    ;

// =============================================================================
// Aggregation Expressions
// =============================================================================

aggregationExpression
    : LBRACE expressionKey COLON expressionValue RBRACE
    | fieldReference
    | literal
    ;

expressionKey
    : arithmeticExprKey
    | arrayExprKey
    | booleanExprKey
    | comparisonExprKey
    | conditionalExprKey
    | dateExprKey
    | objectExprKey
    | setExprKey
    | stringExprKey
    | typeExprKey
    | miscExprKey
    ;

expressionValue
    : fieldReference
    | array
    | document
    | literal
    | aggregationExpression
    ;

// Arithmetic expression keys
arithmeticExprKey
    : EXPR_ABS
    | EXPR_ADD
    | EXPR_CEIL
    | EXPR_DIVIDE
    | EXPR_EXP
    | EXPR_FLOOR
    | EXPR_LN
    | EXPR_LOG
    | EXPR_LOG10
    | EXPR_MOD
    | EXPR_MULTIPLY
    | EXPR_POW
    | EXPR_ROUND
    | EXPR_SQRT
    | EXPR_SUBTRACT
    | EXPR_TRUNC
    ;

// Array expression keys
arrayExprKey
    : EXPR_ARRAYELEMAT
    | EXPR_ARRAYTOOBJECT
    | EXPR_CONCATARRAYS
    | EXPR_FILTER
    | EXPR_FIRST
    | EXPR_INDEXOFARRAY
    | EXPR_ISARRAY
    | EXPR_LAST
    | EXPR_MAP
    | EXPR_OBJECTTOARRAY
    | EXPR_RANGE
    | EXPR_REDUCE
    | EXPR_REVERSEARR
    | EXPR_SIZEARR
    | EXPR_SLICEARR
    | EXPR_ZIP
    ;

// Boolean expression keys
booleanExprKey
    : EXPR_ANDEXPR
    | EXPR_NOTEXPR
    | EXPR_OREXPR
    ;

// Comparison expression keys
comparisonExprKey
    : EXPR_CMP
    | EXPR_EQEXPR
    | EXPR_GTEXPR
    | EXPR_GTEEXPR
    | EXPR_LTEXPR
    | EXPR_LTEEXPR
    | EXPR_NEEXPR
    ;

// Conditional expression keys
conditionalExprKey
    : EXPR_COND
    | EXPR_IFNULL
    | EXPR_SWITCH
    ;

// Date expression keys
dateExprKey
    : EXPR_DATEADD
    | EXPR_DATEDIFF
    | EXPR_DATEFROMPARTS
    | EXPR_DATEFROMSTRING
    | EXPR_DATESUBTRACT
    | EXPR_DATETOPARTS
    | EXPR_DATETOSTRING
    | EXPR_DATETRUNC
    | EXPR_DAYOFMONTH
    | EXPR_DAYOFWEEK
    | EXPR_DAYOFYEAR
    | EXPR_HOUR
    | EXPR_ISODAYOFWEEK
    | EXPR_ISOWEEK
    | EXPR_ISOWEEKWYEAR
    | EXPR_MILLISECOND
    | EXPR_MINUTE
    | EXPR_MONTH
    | EXPR_SECOND
    | EXPR_TOISODATE
    | EXPR_WEEK
    | EXPR_YEAR
    ;

// Object expression keys
objectExprKey
    : EXPR_MERGEOBJECTS
    | EXPR_SETFIELD
    | EXPR_GETFIELD
    | EXPR_UNSETFIELD
    ;

// Set expression keys
setExprKey
    : EXPR_ALLELEMENTSTRUE
    | EXPR_ANYELEMENTTRUE
    | EXPR_SETDIFF
    | EXPR_SETEQUALS
    | EXPR_SETINTERSECTION
    | EXPR_SETISSUBSET
    | EXPR_SETUNION
    ;

// String expression keys
stringExprKey
    : EXPR_CONCAT
    | EXPR_INDEXOFBYTES
    | EXPR_INDEXOFCP
    | EXPR_LTRIM
    | EXPR_REGEXFIND
    | EXPR_REGEXFINDALL
    | EXPR_REGEXMATCH
    | EXPR_REPLACEONE
    | EXPR_REPLACEALL
    | EXPR_RTRIM
    | EXPR_SPLIT
    | EXPR_STRCASECMP
    | EXPR_STRLEN
    | EXPR_STRLENCP
    | EXPR_SUBSTR
    | EXPR_SUBSTRBYTES
    | EXPR_SUBSTRCP
    | EXPR_TOLOWER
    | EXPR_TOSTRING
    | EXPR_TRIM
    | EXPR_TOUPPER
    ;

// Type expression keys
typeExprKey
    : EXPR_CONVERT
    | EXPR_ISBOOL
    | EXPR_ISDATE
    | EXPR_ISNUMBER
    | EXPR_TOBOOL
    | EXPR_TOBINDATA
    | EXPR_TODECIMAL
    | EXPR_TODOUBLE
    | EXPR_TOINT
    | EXPR_TOLONG
    | EXPR_TOOBJECTID
    | EXPR_TOUUID
    | EXPR_TYPEEXPR
    ;

// Miscellaneous expression keys
miscExprKey
    : EXPR_LET
    | EXPR_LITERAL
    | EXPR_RAND
    | EXPR_SAMPLERATE
    | EXPR_META
    | EXPR_BITAND
    | EXPR_BITOR
    | EXPR_BITXOR
    | EXPR_BITNOT
    | EXPR_SORTARRAY
    ;

// Specific expression structures
condExpression
    : LBRACE EXPR_COND COLON condValue RBRACE
    ;

condValue
    : array                      // [if, then, else] form
    | condDocument               // {if:, then:, else:} form
    ;

condDocument
    : LBRACE condBody RBRACE
    ;

condBody
    : condPair (COMMA condPair)*
    ;

condPair
    : 'if' COLON aggregationExpression
    | 'then' COLON aggregationExpression
    | 'else' COLON aggregationExpression
    ;

switchExpression
    : LBRACE EXPR_SWITCH COLON switchDocument RBRACE
    ;

switchDocument
    : LBRACE switchBody RBRACE
    ;

switchBody
    : switchPair (COMMA switchPair)*
    ;

switchPair
    : 'branches' COLON array
    | 'default' COLON aggregationExpression
    ;

letExpression
    : LBRACE EXPR_LET COLON letDocument RBRACE
    ;

letDocument
    : LBRACE letBody RBRACE
    ;

letBody
    : letPair (COMMA letPair)*
    ;

letPair
    : 'vars' COLON document
    | 'in' COLON aggregationExpression
    ;

filterExpression
    : LBRACE EXPR_FILTER COLON filterDocument RBRACE
    ;

filterDocument
    : LBRACE filterBody RBRACE
    ;

filterBody
    : filterPair (COMMA filterPair)*
    ;

filterPair
    : 'input' COLON (fieldReference | aggregationExpression)
    | 'cond' COLON aggregationExpression
    | 'as' COLON STRING
    | 'limit' COLON (INTEGER | aggregationExpression)
    ;

mapExpression
    : LBRACE EXPR_MAP COLON mapDocument RBRACE
    ;

mapDocument
    : LBRACE mapBody RBRACE
    ;

mapBody
    : mapPair (COMMA mapPair)*
    ;

mapPair
    : 'input' COLON (fieldReference | aggregationExpression)
    | 'as' COLON STRING
    | 'in' COLON aggregationExpression
    ;

reduceExpression
    : LBRACE EXPR_REDUCE COLON reduceDocument RBRACE
    ;

reduceDocument
    : LBRACE reduceBody RBRACE
    ;

reduceBody
    : reducePair (COMMA reducePair)*
    ;

reducePair
    : 'input' COLON (fieldReference | aggregationExpression)
    | 'initialValue' COLON value
    | 'in' COLON aggregationExpression
    ;

// =============================================================================
// BSON Types
// =============================================================================

bsonType
    : objectIdType
    | dateType
    | numberIntType
    | numberLongType
    | numberDecimalType
    | timestampType
    | binDataType
    | uuidType
    | minKeyType
    | maxKeyType
    | dbRefType
    | regexType
    | codeType
    ;

objectIdType
    : OBJECTID LPAREN STRING? RPAREN
    | NEW? OBJECTID LPAREN STRING? RPAREN
    ;

dateType
    : ISODATE LPAREN STRING? RPAREN
    | DATE LPAREN (STRING | INTEGER)? RPAREN
    | NEW? DATE LPAREN (STRING | INTEGER | (INTEGER COMMA INTEGER COMMA INTEGER))? RPAREN
    ;

numberIntType
    : NUMBERINT LPAREN (INTEGER | STRING) RPAREN
    ;

numberLongType
    : NUMBERLONG LPAREN (INTEGER | STRING) RPAREN
    ;

numberDecimalType
    : NUMBERDECIMAL LPAREN (DECIMAL | INTEGER | STRING) RPAREN
    ;

timestampType
    : TIMESTAMP LPAREN INTEGER COMMA INTEGER RPAREN
    ;

binDataType
    : BINDATA LPAREN INTEGER COMMA STRING RPAREN
    ;

uuidType
    : UUID LPAREN STRING RPAREN
    ;

minKeyType
    : MINKEY LPAREN RPAREN
    | MINKEY
    ;

maxKeyType
    : MAXKEY LPAREN RPAREN
    | MAXKEY
    ;

dbRefType
    : DBREF LPAREN STRING COMMA (STRING | objectIdType) RPAREN
    ;

regexType
    : REGEX LPAREN STRING (COMMA STRING)? RPAREN
    | REGEX_LITERAL
    ;

codeType
    : CODE LPAREN STRING RPAREN
    | CODEWSCOPE LPAREN STRING COMMA document RPAREN
    ;

// =============================================================================
// Field and Variable References
// =============================================================================

fieldReference
    : FIELD_REF
    | DOLLAR fieldPath
    ;

fieldPath
    : IDENTIFIER (DOT (IDENTIFIER | INTEGER))*
    ;

variableReference
    : VAR_REF
    ;

// =============================================================================
// Literals
// =============================================================================

literal
    : stringLiteral
    | numericLiteral
    | booleanLiteral
    | nullLiteral
    | undefinedLiteral
    | infinityLiteral
    | nanLiteral
    ;

stringLiteral
    : STRING
    ;

numericLiteral
    : INTEGER
    | DECIMAL
    | HEX
    ;

booleanLiteral
    : TRUE
    | FALSE
    ;

nullLiteral
    : NULL
    ;

undefinedLiteral
    : UNDEFINED
    ;

infinityLiteral
    : INFINITY
    ;

nanLiteral
    : NAN
    ;

// =============================================================================
// Operator Keys (for use as document keys)
// =============================================================================

operatorKey
    : comparisonOperator
    | logicalOperator
    | elementOperator
    | evaluationOperator
    | arrayQueryOperator
    | bitwiseQueryOperator
    | geospatialOperator
    | fieldUpdateOperator
    | arrayUpdateOperator
    | bitwiseUpdateOperator
    | arrayUpdateModifier
    ;

// =============================================================================
// Function Expressions (for $where)
// =============================================================================

functionExpression
    : 'function' LPAREN RPAREN LBRACE functionBody RBRACE
    ;

functionBody
    : ~(LBRACE | RBRACE)*
    | ~(LBRACE | RBRACE)* LBRACE functionBody RBRACE ~(LBRACE | RBRACE)*
    ;
