/**
 * Redis Stack Module Commands Grammar for ANTLR4
 * 
 * This grammar defines commands for Redis Stack modules:
 * - RediSearch (FT.*)
 * - RedisJSON (JSON.*)
 * - RedisTimeSeries (TS.*)
 * - RedisBloom (BF.*, CF.*, CMS.*, TOPK.*, TDIGEST.*)
 * - RedisGraph (GRAPH.*)
 * - RedisGears (TFUNCTION, TFCALL)
 * 
 * Commands are sent over RESP as arrays of bulk strings.
 * This grammar helps parse command semantics after RESP decoding.
 * 
 * Author: Generated for LLM coding assistance
 * License: MIT
 */

grammar RedisModules;

// =============================================================================
// Entry Point
// =============================================================================

moduleCommand
    : searchCommand         // RediSearch (FT.*)
    | jsonCommand           // RedisJSON (JSON.*)  
    | timeSeriesCommand     // RedisTimeSeries (TS.*)
    | bloomFilterCommand    // RedisBloom - Bloom Filter (BF.*)
    | cuckooFilterCommand   // RedisBloom - Cuckoo Filter (CF.*)
    | countMinSketchCommand // RedisBloom - Count-Min Sketch (CMS.*)
    | topKCommand           // RedisBloom - Top-K (TOPK.*)
    | tDigestCommand        // RedisBloom - T-Digest (TDIGEST.*)
    | graphCommand          // RedisGraph (GRAPH.*)
    | gearsCommand          // RedisGears
    ;

// =============================================================================
// RediSearch Commands (FT.*)
// =============================================================================

searchCommand
    : ftAggregate
    | ftAliasAdd
    | ftAliasDel
    | ftAliasUpdate
    | ftAlter
    | ftConfig
    | ftCreate
    | ftCursor
    | ftDictAdd
    | ftDictDel
    | ftDictDump
    | ftDropIndex
    | ftExplain
    | ftInfo
    | ftProfile
    | ftSearch
    | ftSpellcheck
    | ftSugAdd
    | ftSugDel
    | ftSugGet
    | ftSugLen
    | ftSynDump
    | ftSynUpdate
    | ftTagVals
    ;

ftCreate
    : FT_CREATE indexName createOptions? SCHEMA fieldDefinition+
    ;

ftSearch
    : FT_SEARCH indexName query searchOptions*
    ;

ftAggregate
    : FT_AGGREGATE indexName query aggregateOptions*
    ;

ftAliasAdd    : FT_ALIASADD aliasName indexName;
ftAliasDel    : FT_ALIASDEL aliasName;
ftAliasUpdate : FT_ALIASUPDATE aliasName indexName;
ftAlter       : FT_ALTER indexName SCHEMA ADD fieldDefinition+;
ftConfig      : FT_CONFIG configAction optionName optionValue?;
ftCursor      : FT_CURSOR cursorAction indexName cursorId count?;
ftDictAdd     : FT_DICTADD dictName term+;
ftDictDel     : FT_DICTDEL dictName term+;
ftDictDump    : FT_DICTDUMP dictName;
ftDropIndex   : FT_DROPINDEX indexName DD?;
ftExplain     : FT_EXPLAIN indexName query;
ftInfo        : FT_INFO indexName;
ftProfile     : FT_PROFILE indexName profileMode LIMITED? QUERY query;
ftSpellcheck  : FT_SPELLCHECK indexName query spellcheckOptions*;
ftSugAdd      : FT_SUGADD key STRING score INCR? (PAYLOAD payload)?;
ftSugDel      : FT_SUGDEL key STRING;
ftSugGet      : FT_SUGGET key prefix FUZZY? WITHSCORES? WITHPAYLOADS? (MAX maxNum)?;
ftSugLen      : FT_SUGLEN key;
ftSynDump     : FT_SYNDUMP indexName;
ftSynUpdate   : FT_SYNUPDATE indexName synonymGroupId SKIPINITIALSCAN? term+;
ftTagVals     : FT_TAGVALS indexName fieldName;

createOptions
    : (ON dataType)?
      (PREFIX prefixCount prefix+)?
      (FILTER filterExpr)?
      (LANGUAGE defaultLang)?
      (LANGUAGE_FIELD langField)?
      (SCORE defaultScore)?
      (SCORE_FIELD scoreField)?
      (PAYLOAD_FIELD payloadField)?
      MAXTEXTFIELDS?
      (TEMPORARY seconds)?
      NOOFFSETS?
      NOHL?
      NOFIELDS?
      NOFREQS?
      (STOPWORDS stopwordCount stopword*)?
      SKIPINITIALSCAN?
    ;

fieldDefinition
    : fieldName fieldType fieldOptions*
    ;

fieldType
    : TEXT textOptions?
    | TAG tagOptions?
    | NUMERIC
    | GEO
    | VECTOR vectorAlgorithm vectorAttrs
    | GEOSHAPE
    ;

textOptions   : (NOSTEM)? (WEIGHT weight)? (PHONETIC matcher)?;
tagOptions    : (SEPARATOR sep)? CASESENSITIVE?;
vectorAlgorithm : FLAT | HNSW;
vectorAttrs   : attrCount (attrName attrValue)+;

fieldOptions
    : SORTABLE UNF?
    | NOINDEX
    | AS aliasName
    ;

searchOptions
    : NOCONTENT
    | VERBATIM
    | NOSTOPWORDS
    | WITHSCORES
    | WITHPAYLOADS
    | WITHSORTKEYS
    | FILTER numericField minVal maxVal
    | GEOFILTER geoField lon lat radius geoUnit
    | INKEYS keyCount key+
    | INFIELDS fieldCount fieldName+
    | RETURN returnCount (fieldName (AS aliasName)?)+
    | SUMMARIZE summarizeOpts?
    | HIGHLIGHT highlightOpts?
    | SLOP slopVal
    | TIMEOUT timeoutVal
    | INORDER
    | LANGUAGE langName
    | EXPANDER expanderName
    | SCORER scorerName
    | EXPLAINSCORE
    | PAYLOAD payloadVal
    | SORTBY sortField sortOrder?
    | LIMIT offsetVal countVal
    | PARAMS paramCount (paramName paramValue)+
    | DIALECT dialectVersion
    ;

aggregateOptions
    : VERBATIM
    | LOAD loadCount (fieldName (AS aliasName)?)+
    | TIMEOUT timeoutVal
    | GROUPBY groupCount property+ reduceClause*
    | SORTBY sortCount (property sortOrder?)+ (MAX maxVal)?
    | APPLY expr AS newField
    | LIMIT offsetVal countVal
    | FILTER filterExpr
    | WITHCURSOR cursorOpts?
    | PARAMS paramCount (paramName paramValue)+
    | DIALECT dialectVersion
    ;

reduceClause   : REDUCE reducerName argCount arg* (AS newField)?;
summarizeOpts  : (FIELDS fieldCount fieldName+)? (FRAGS fragCount)? (LEN fragLen)? (SEPARATOR fragSep)?;
highlightOpts  : (FIELDS fieldCount fieldName+)? (TAGS openTag closeTag)?;
cursorOpts     : (COUNT cursorCount)? (MAXIDLE maxIdleTime)?;
spellcheckOptions : (DISTANCE dist)? (TERMS termMode dictName term*)?;

configAction  : GET | SET | HELP;
cursorAction  : READ | DEL;
profileMode   : SEARCH | AGGREGATE;
termMode      : INCLUDE | EXCLUDE;
sortOrder     : ASC | DESC;
dataType      : HASH | JSON;

// =============================================================================
// RedisJSON Commands (JSON.*)
// =============================================================================

jsonCommand
    : jsonArrAppend
    | jsonArrIndex
    | jsonArrInsert
    | jsonArrLen
    | jsonArrPop
    | jsonArrTrim
    | jsonClear
    | jsonDebug
    | jsonDel
    | jsonForget
    | jsonGet
    | jsonMerge
    | jsonMGet
    | jsonMSet
    | jsonNumIncrBy
    | jsonNumMultBy
    | jsonObjKeys
    | jsonObjLen
    | jsonResp
    | jsonSet
    | jsonStrAppend
    | jsonStrLen
    | jsonToggle
    | jsonType
    ;

jsonArrAppend : JSON_ARRAPPEND key (path jsonValue+)+;
jsonArrIndex  : JSON_ARRINDEX key path jsonValue start? stop?;
jsonArrInsert : JSON_ARRINSERT key path index jsonValue+;
jsonArrLen    : JSON_ARRLEN key path?;
jsonArrPop    : JSON_ARRPOP key path? index?;
jsonArrTrim   : JSON_ARRTRIM key path start stop;
jsonClear     : JSON_CLEAR key path?;
jsonDebug     : JSON_DEBUG debugSubcommand key path?;
jsonDel       : JSON_DEL key path?;
jsonForget    : JSON_FORGET key path?;
jsonGet       : JSON_GET key jsonGetOptions? path*;
jsonMerge     : JSON_MERGE key path jsonValue;
jsonMGet      : JSON_MGET key+ path;
jsonMSet      : JSON_MSET (key path jsonValue)+;
jsonNumIncrBy : JSON_NUMINCRBY key path numValue;
jsonNumMultBy : JSON_NUMMULTBY key path numValue;
jsonObjKeys   : JSON_OBJKEYS key path?;
jsonObjLen    : JSON_OBJLEN key path?;
jsonResp      : JSON_RESP key path?;
jsonSet       : JSON_SET key path jsonValue jsonSetOption?;
jsonStrAppend : JSON_STRAPPEND key path? stringValue;
jsonStrLen    : JSON_STRLEN key path?;
jsonToggle    : JSON_TOGGLE key path;
jsonType      : JSON_TYPE key path?;

jsonGetOptions : (INDENT indentStr)? (NEWLINE newlineStr)? (SPACE spaceStr)? (FORMAT formatName)?;
jsonSetOption  : NX | XX;
debugSubcommand: MEMORY | HELP;

// =============================================================================
// RedisTimeSeries Commands (TS.*)
// =============================================================================

timeSeriesCommand
    : tsAdd
    | tsAlter
    | tsCreate
    | tsCreateRule
    | tsDecrBy
    | tsDel
    | tsDeleteRule
    | tsGet
    | tsInfo
    | tsIncrBy
    | tsMAdd
    | tsMGet
    | tsMRange
    | tsMRevRange
    | tsQueryIndex
    | tsRange
    | tsRevRange
    ;

tsAdd        : TS_ADD key timestamp tsValue tsAddOptions?;
tsAlter      : TS_ALTER key tsAlterOptions?;
tsCreate     : TS_CREATE key tsCreateOptions?;
tsCreateRule : TS_CREATERULE srcKey destKey AGGREGATION aggType bucketDuration createRuleOptions?;
tsDecrBy     : TS_DECRBY key tsValue tsIncrDecrOptions?;
tsDel        : TS_DEL key fromTimestamp toTimestamp;
tsDeleteRule : TS_DELETERULE srcKey destKey;
tsGet        : TS_GET key LATEST?;
tsInfo       : TS_INFO key DEBUG?;
tsIncrBy     : TS_INCRBY key tsValue tsIncrDecrOptions?;
tsMAdd       : TS_MADD (key timestamp tsValue)+;
tsMGet       : TS_MGET LATEST? tsFilterOptions? FILTER tsFilter+;
tsMRange     : TS_MRANGE fromTimestamp toTimestamp tsMrangeOptions?;
tsMRevRange  : TS_MREVRANGE fromTimestamp toTimestamp tsMrangeOptions?;
tsQueryIndex : TS_QUERYINDEX tsFilter+;
tsRange      : TS_RANGE key fromTimestamp toTimestamp tsRangeOptions?;
tsRevRange   : TS_REVRANGE key fromTimestamp toTimestamp tsRangeOptions?;

tsCreateOptions
    : (RETENTION retention)?
      (ENCODING tsEncoding)?
      (CHUNK_SIZE chunkSize)?
      (DUPLICATE_POLICY dupPolicy)?
      (LABELS (labelName labelValue)+)?
      (IGNORE maxTimeDiff maxValDiff)?
    ;

tsAddOptions
    : (RETENTION retention)?
      (ENCODING tsEncoding)?
      (CHUNK_SIZE chunkSize)?
      (DUPLICATE_POLICY dupPolicy)?
      (ON_DUPLICATE dupPolicy)?
      (LABELS (labelName labelValue)+)?
      (IGNORE maxTimeDiff maxValDiff)?
    ;

tsAlterOptions
    : (RETENTION retention)?
      (CHUNK_SIZE chunkSize)?
      (DUPLICATE_POLICY dupPolicy)?
      (LABELS (labelName labelValue)+)?
      (IGNORE maxTimeDiff maxValDiff)?
    ;

tsIncrDecrOptions
    : (TIMESTAMP timestamp)?
      (RETENTION retention)?
      (ENCODING tsEncoding)?
      (CHUNK_SIZE chunkSize)?
      (LABELS (labelName labelValue)+)?
      (IGNORE maxTimeDiff maxValDiff)?
    ;

createRuleOptions : (ALIGNTIMESTAMP alignTs)?;

tsRangeOptions
    : LATEST?
      (FILTER_BY_TS timestamp+)?
      (FILTER_BY_VALUE minVal maxVal)?
      (COUNT countVal)?
      (ALIGN alignVal)?
      (AGGREGATION aggType bucketDuration bucketTimestamp? EMPTY?)?
    ;

tsMrangeOptions
    : LATEST?
      (FILTER_BY_TS timestamp+)?
      (FILTER_BY_VALUE minVal maxVal)?
      labelSelection?
      (COUNT countVal)?
      (ALIGN alignVal)?
      (AGGREGATION aggType bucketDuration bucketTimestamp? EMPTY?)?
      FILTER tsFilter+
      (GROUPBY labelName REDUCE reducer)?
    ;

tsFilterOptions : WITHLABELS | SELECTED_LABELS labelName+;
labelSelection  : WITHLABELS | SELECTED_LABELS labelName+;
bucketTimestamp : BUCKETTIMESTAMP btValue;
tsEncoding      : COMPRESSED | UNCOMPRESSED;

aggType
    : AVG | SUM | MIN | MAX | RANGE | COUNT | FIRST | LAST
    | STD_P | STD_S | VAR_P | VAR_S | TWA
    ;

reducer : AVG | SUM | MIN | MAX | RANGE | COUNT | STD_P | STD_S | VAR_P | VAR_S;

// =============================================================================
// RedisBloom - Bloom Filter Commands (BF.*)
// =============================================================================

bloomFilterCommand
    : bfAdd
    | bfCard
    | bfExists
    | bfInfo
    | bfInsert
    | bfLoadChunk
    | bfMAdd
    | bfMExists
    | bfReserve
    | bfScanDump
    ;

bfAdd       : BF_ADD key item;
bfCard      : BF_CARD key;
bfExists    : BF_EXISTS key item;
bfInfo      : BF_INFO key bfInfoField?;
bfInsert    : BF_INSERT key bfInsertOptions? ITEMS item+;
bfLoadChunk : BF_LOADCHUNK key iterator chunkData;
bfMAdd      : BF_MADD key item+;
bfMExists   : BF_MEXISTS key item+;
bfReserve   : BF_RESERVE key errorRate capacity bfReserveOptions?;
bfScanDump  : BF_SCANDUMP key iterator;

bfInfoField      : CAPACITY | SIZE | FILTERS | ITEMS | EXPANSION;
bfInsertOptions  : (CAPACITY capacity)? (ERROR errorRate)? (EXPANSION expansion)? NOCREATE? NONSCALING?;
bfReserveOptions : (EXPANSION expansion)? NONSCALING?;

// =============================================================================
// RedisBloom - Cuckoo Filter Commands (CF.*)
// =============================================================================

cuckooFilterCommand
    : cfAdd
    | cfAddNx
    | cfCount
    | cfDel
    | cfExists
    | cfInfo
    | cfInsert
    | cfInsertNx
    | cfLoadChunk
    | cfMExists
    | cfReserve
    | cfScanDump
    ;

cfAdd       : CF_ADD key item;
cfAddNx     : CF_ADDNX key item;
cfCount     : CF_COUNT key item;
cfDel       : CF_DEL key item;
cfExists    : CF_EXISTS key item;
cfInfo      : CF_INFO key;
cfInsert    : CF_INSERT key cfInsertOptions? ITEMS item+;
cfInsertNx  : CF_INSERTNX key cfInsertOptions? ITEMS item+;
cfLoadChunk : CF_LOADCHUNK key iterator chunkData;
cfMExists   : CF_MEXISTS key item+;
cfReserve   : CF_RESERVE key capacity cfReserveOptions?;
cfScanDump  : CF_SCANDUMP key iterator;

cfInsertOptions  : (CAPACITY capacity)? NOCREATE?;
cfReserveOptions : (BUCKETSIZE bucketSize)? (MAXITERATIONS maxIter)? (EXPANSION expansion)?;

// =============================================================================
// RedisBloom - Count-Min Sketch Commands (CMS.*)
// =============================================================================

countMinSketchCommand
    : cmsIncrBy
    | cmsInfo
    | cmsInitByDim
    | cmsInitByProb
    | cmsMerge
    | cmsQuery
    ;

cmsIncrBy     : CMS_INCRBY key (item increment)+;
cmsInfo       : CMS_INFO key;
cmsInitByDim  : CMS_INITBYDIM key width depth;
cmsInitByProb : CMS_INITBYPROB key errorRate probability;
cmsMerge      : CMS_MERGE destKey numKeys srcKey+ (WEIGHTS weight+)?;
cmsQuery      : CMS_QUERY key item+;

// =============================================================================
// RedisBloom - Top-K Commands (TOPK.*)
// =============================================================================

topKCommand
    : topkAdd
    | topkCount
    | topkIncrBy
    | topkInfo
    | topkList
    | topkQuery
    | topkReserve
    ;

topkAdd     : TOPK_ADD key item+;
topkCount   : TOPK_COUNT key item+;
topkIncrBy  : TOPK_INCRBY key (item increment)+;
topkInfo    : TOPK_INFO key;
topkList    : TOPK_LIST key WITHCOUNT?;
topkQuery   : TOPK_QUERY key item+;
topkReserve : TOPK_RESERVE key topK width? depth? decay?;

// =============================================================================
// RedisBloom - T-Digest Commands (TDIGEST.*)
// =============================================================================

tDigestCommand
    : tdigestAdd
    | tdigestByRank
    | tdigestByRevRank
    | tdigestCdf
    | tdigestCreate
    | tdigestInfo
    | tdigestMax
    | tdigestMerge
    | tdigestMin
    | tdigestQuantile
    | tdigestRank
    | tdigestReset
    | tdigestRevRank
    | tdigestTrimmedMean
    ;

tdigestAdd         : TDIGEST_ADD key tdValue+;
tdigestByRank      : TDIGEST_BYRANK key rank+;
tdigestByRevRank   : TDIGEST_BYREVRANK key rank+;
tdigestCdf         : TDIGEST_CDF key tdValue+;
tdigestCreate      : TDIGEST_CREATE key (COMPRESSION compression)?;
tdigestInfo        : TDIGEST_INFO key;
tdigestMax         : TDIGEST_MAX key;
tdigestMerge       : TDIGEST_MERGE destKey numKeys srcKey+ (COMPRESSION compression)? OVERRIDE?;
tdigestMin         : TDIGEST_MIN key;
tdigestQuantile    : TDIGEST_QUANTILE key quantile+;
tdigestRank        : TDIGEST_RANK key tdValue+;
tdigestReset       : TDIGEST_RESET key;
tdigestRevRank     : TDIGEST_REVRANK key tdValue+;
tdigestTrimmedMean : TDIGEST_TRIMMED_MEAN key lowQuantile highQuantile;

// =============================================================================
// RedisGraph Commands (GRAPH.*)
// =============================================================================

graphCommand
    : graphConfig
    | graphConstraint
    | graphDelete
    | graphExplain
    | graphList
    | graphProfile
    | graphQuery
    | graphRoQuery
    | graphSlowlog
    ;

graphConfig     : GRAPH_CONFIG configAction configName configValue?;
graphConstraint : GRAPH_CONSTRAINT constraintAction graphKey constraintType graphTarget constraintProps;
graphDelete     : GRAPH_DELETE graphKey;
graphExplain    : GRAPH_EXPLAIN graphKey cypherQuery;
graphList       : GRAPH_LIST;
graphProfile    : GRAPH_PROFILE graphKey cypherQuery (TIMEOUT timeoutVal)?;
graphQuery      : GRAPH_QUERY graphKey cypherQuery graphQueryOptions?;
graphRoQuery    : GRAPH_RO_QUERY graphKey cypherQuery graphQueryOptions?;
graphSlowlog    : GRAPH_SLOWLOG graphKey;

constraintAction : CREATE | DROP;
constraintType   : MANDATORY | UNIQUE;
graphTarget      : ON (NODE labelName | RELATIONSHIP relType) PROPERTIES propCount property+;
constraintProps  : PROPERTIES propCount property+;
graphQueryOptions: (TIMEOUT timeoutVal)?;

// =============================================================================
// RedisGears Commands
// =============================================================================

gearsCommand
    : tFunction
    | tfCall
    | tfCallAsync
    ;

tFunction    : TFUNCTION tfSubcommand;
tfCall       : TFCALL functionName key* arg*;
tfCallAsync  : TFCALLASYNC functionName key* arg*;

tfSubcommand
    : LOAD REPLACE? (CONFIG configValue)? libraryCode
    | DELETE libraryName
    | LIST WITHCODE? VERBOSE? V? (LIBRARY libraryName)?
    ;

// =============================================================================
// Common Elements
// =============================================================================

key           : STRING;
indexName     : STRING;
aliasName     : STRING;
fieldName     : STRING;
dictName      : STRING;
term          : STRING;
query         : STRING;
prefix        : STRING;
filterExpr    : STRING;
expr          : STRING;
property      : STRING;
arg           : STRING;
item          : STRING;
path          : STRING;
jsonValue     : STRING;
stringValue   : STRING;
payload       : STRING;
payloadVal    : STRING;
cursorId      : INTEGER;
synonymGroupId: STRING;
configName    : STRING;
configValue   : STRING;
optionName    : STRING;
optionValue   : STRING;
graphKey      : STRING;
cypherQuery   : STRING;
labelName     : STRING;
labelValue    : STRING;
relType       : STRING;
functionName  : STRING;
libraryCode   : STRING;
libraryName   : STRING;
tsFilter      : STRING;
chunkData     : STRING;

// Numeric values
score         : NUMBER;
weight        : NUMBER;
numValue      : NUMBER;
tsValue       : NUMBER;
errorRate     : NUMBER;
probability   : NUMBER;
decay         : NUMBER;
compression   : INTEGER;
quantile      : NUMBER;
lowQuantile   : NUMBER;
highQuantile  : NUMBER;
tdValue       : NUMBER;

// Integer values
index         : INTEGER;
start         : INTEGER;
stop          : INTEGER;
count         : INTEGER;
countVal      : INTEGER;
offsetVal     : INTEGER;
seconds       : INTEGER;
timestamp     : INTEGER;
fromTimestamp : INTEGER | MINUS | PLUS;
toTimestamp   : INTEGER | MINUS | PLUS;
retention     : INTEGER;
chunkSize     : INTEGER;
bucketDuration: INTEGER;
alignTs       : INTEGER;
maxTimeDiff   : INTEGER;
maxValDiff    : NUMBER;
capacity      : INTEGER;
expansion     : INTEGER;
iterator      : INTEGER;
width         : INTEGER;
depth         : INTEGER;
bucketSize    : INTEGER;
maxIter       : INTEGER;
numKeys       : INTEGER;
topK          : INTEGER;
rank          : INTEGER;
propCount     : INTEGER;
prefixCount   : INTEGER;
stopwordCount : INTEGER;
keyCount      : INTEGER;
fieldCount    : INTEGER;
returnCount   : INTEGER;
loadCount     : INTEGER;
groupCount    : INTEGER;
sortCount     : INTEGER;
argCount      : INTEGER;
attrCount     : INTEGER;
paramCount    : INTEGER;
maxNum        : INTEGER;
cursorCount   : INTEGER;
maxIdleTime   : INTEGER;
dist          : INTEGER;
fragCount     : INTEGER;
fragLen       : INTEGER;
slopVal       : INTEGER;
timeoutVal    : INTEGER;
dialectVersion: INTEGER;
defaultScore  : NUMBER;

srcKey        : STRING;
destKey       : STRING;
sortField     : STRING;
numericField  : STRING;
geoField      : STRING;
lon           : NUMBER;
lat           : NUMBER;
radius        : NUMBER;
minVal        : NUMBER | STRING;
maxVal        : NUMBER | STRING;
alignVal      : INTEGER | STRING;
btValue       : STRING;
dupPolicy     : STRING;
reducer       : STRING;
aggType       : STRING;
stopword      : STRING;
langField     : STRING;
scoreField    : STRING;
payloadField  : STRING;
defaultLang   : STRING;
langName      : STRING;
expanderName  : STRING;
scorerName    : STRING;
newField      : STRING;
reducerName   : STRING;
paramName     : STRING;
paramValue    : STRING;
attrName      : STRING;
attrValue     : STRING;
matcher       : STRING;
sep           : STRING;
fragSep       : STRING;
openTag       : STRING;
closeTag      : STRING;
indentStr     : STRING;
newlineStr    : STRING;
spaceStr      : STRING;
formatName    : STRING;
geoUnit       : M | KM | FT | MI;

// =============================================================================
// Lexer Rules - Keywords
// =============================================================================

// RediSearch Commands
FT_AGGREGATE    : 'FT.AGGREGATE';
FT_ALIASADD     : 'FT.ALIASADD';
FT_ALIASDEL     : 'FT.ALIASDEL';
FT_ALIASUPDATE  : 'FT.ALIASUPDATE';
FT_ALTER        : 'FT.ALTER';
FT_CONFIG       : 'FT.CONFIG';
FT_CREATE       : 'FT.CREATE';
FT_CURSOR       : 'FT.CURSOR';
FT_DICTADD      : 'FT.DICTADD';
FT_DICTDEL      : 'FT.DICTDEL';
FT_DICTDUMP     : 'FT.DICTDUMP';
FT_DROPINDEX    : 'FT.DROPINDEX';
FT_EXPLAIN      : 'FT.EXPLAIN';
FT_INFO         : 'FT.INFO';
FT_PROFILE      : 'FT.PROFILE';
FT_SEARCH       : 'FT.SEARCH';
FT_SPELLCHECK   : 'FT.SPELLCHECK';
FT_SUGADD       : 'FT.SUGADD';
FT_SUGDEL       : 'FT.SUGDEL';
FT_SUGGET       : 'FT.SUGGET';
FT_SUGLEN       : 'FT.SUGLEN';
FT_SYNDUMP      : 'FT.SYNDUMP';
FT_SYNUPDATE    : 'FT.SYNUPDATE';
FT_TAGVALS      : 'FT.TAGVALS';

// RedisJSON Commands
JSON_ARRAPPEND  : 'JSON.ARRAPPEND';
JSON_ARRINDEX   : 'JSON.ARRINDEX';
JSON_ARRINSERT  : 'JSON.ARRINSERT';
JSON_ARRLEN     : 'JSON.ARRLEN';
JSON_ARRPOP     : 'JSON.ARRPOP';
JSON_ARRTRIM    : 'JSON.ARRTRIM';
JSON_CLEAR      : 'JSON.CLEAR';
JSON_DEBUG      : 'JSON.DEBUG';
JSON_DEL        : 'JSON.DEL';
JSON_FORGET     : 'JSON.FORGET';
JSON_GET        : 'JSON.GET';
JSON_MERGE      : 'JSON.MERGE';
JSON_MGET       : 'JSON.MGET';
JSON_MSET       : 'JSON.MSET';
JSON_NUMINCRBY  : 'JSON.NUMINCRBY';
JSON_NUMMULTBY  : 'JSON.NUMMULTBY';
JSON_OBJKEYS    : 'JSON.OBJKEYS';
JSON_OBJLEN     : 'JSON.OBJLEN';
JSON_RESP       : 'JSON.RESP';
JSON_SET        : 'JSON.SET';
JSON_STRAPPEND  : 'JSON.STRAPPEND';
JSON_STRLEN     : 'JSON.STRLEN';
JSON_TOGGLE     : 'JSON.TOGGLE';
JSON_TYPE       : 'JSON.TYPE';

// RedisTimeSeries Commands
TS_ADD          : 'TS.ADD';
TS_ALTER        : 'TS.ALTER';
TS_CREATE       : 'TS.CREATE';
TS_CREATERULE   : 'TS.CREATERULE';
TS_DECRBY       : 'TS.DECRBY';
TS_DEL          : 'TS.DEL';
TS_DELETERULE   : 'TS.DELETERULE';
TS_GET          : 'TS.GET';
TS_INFO         : 'TS.INFO';
TS_INCRBY       : 'TS.INCRBY';
TS_MADD         : 'TS.MADD';
TS_MGET         : 'TS.MGET';
TS_MRANGE       : 'TS.MRANGE';
TS_MREVRANGE    : 'TS.MREVRANGE';
TS_QUERYINDEX   : 'TS.QUERYINDEX';
TS_RANGE        : 'TS.RANGE';
TS_REVRANGE     : 'TS.REVRANGE';

// RedisBloom - Bloom Filter Commands
BF_ADD          : 'BF.ADD';
BF_CARD         : 'BF.CARD';
BF_EXISTS       : 'BF.EXISTS';
BF_INFO         : 'BF.INFO';
BF_INSERT       : 'BF.INSERT';
BF_LOADCHUNK    : 'BF.LOADCHUNK';
BF_MADD         : 'BF.MADD';
BF_MEXISTS      : 'BF.MEXISTS';
BF_RESERVE      : 'BF.RESERVE';
BF_SCANDUMP     : 'BF.SCANDUMP';

// RedisBloom - Cuckoo Filter Commands
CF_ADD          : 'CF.ADD';
CF_ADDNX        : 'CF.ADDNX';
CF_COUNT        : 'CF.COUNT';
CF_DEL          : 'CF.DEL';
CF_EXISTS       : 'CF.EXISTS';
CF_INFO         : 'CF.INFO';
CF_INSERT       : 'CF.INSERT';
CF_INSERTNX     : 'CF.INSERTNX';
CF_LOADCHUNK    : 'CF.LOADCHUNK';
CF_MEXISTS      : 'CF.MEXISTS';
CF_RESERVE      : 'CF.RESERVE';
CF_SCANDUMP     : 'CF.SCANDUMP';

// RedisBloom - Count-Min Sketch Commands
CMS_INCRBY      : 'CMS.INCRBY';
CMS_INFO        : 'CMS.INFO';
CMS_INITBYDIM   : 'CMS.INITBYDIM';
CMS_INITBYPROB  : 'CMS.INITBYPROB';
CMS_MERGE       : 'CMS.MERGE';
CMS_QUERY       : 'CMS.QUERY';

// RedisBloom - Top-K Commands
TOPK_ADD        : 'TOPK.ADD';
TOPK_COUNT      : 'TOPK.COUNT';
TOPK_INCRBY     : 'TOPK.INCRBY';
TOPK_INFO       : 'TOPK.INFO';
TOPK_LIST       : 'TOPK.LIST';
TOPK_QUERY      : 'TOPK.QUERY';
TOPK_RESERVE    : 'TOPK.RESERVE';

// RedisBloom - T-Digest Commands
TDIGEST_ADD         : 'TDIGEST.ADD';
TDIGEST_BYRANK      : 'TDIGEST.BYRANK';
TDIGEST_BYREVRANK   : 'TDIGEST.BYREVRANK';
TDIGEST_CDF         : 'TDIGEST.CDF';
TDIGEST_CREATE      : 'TDIGEST.CREATE';
TDIGEST_INFO        : 'TDIGEST.INFO';
TDIGEST_MAX         : 'TDIGEST.MAX';
TDIGEST_MERGE       : 'TDIGEST.MERGE';
TDIGEST_MIN         : 'TDIGEST.MIN';
TDIGEST_QUANTILE    : 'TDIGEST.QUANTILE';
TDIGEST_RANK        : 'TDIGEST.RANK';
TDIGEST_RESET       : 'TDIGEST.RESET';
TDIGEST_REVRANK     : 'TDIGEST.REVRANK';
TDIGEST_TRIMMED_MEAN: 'TDIGEST.TRIMMED_MEAN';

// RedisGraph Commands
GRAPH_CONFIG     : 'GRAPH.CONFIG';
GRAPH_CONSTRAINT : 'GRAPH.CONSTRAINT';
GRAPH_DELETE     : 'GRAPH.DELETE';
GRAPH_EXPLAIN    : 'GRAPH.EXPLAIN';
GRAPH_LIST       : 'GRAPH.LIST';
GRAPH_PROFILE    : 'GRAPH.PROFILE';
GRAPH_QUERY      : 'GRAPH.QUERY';
GRAPH_RO_QUERY   : 'GRAPH.RO_QUERY';
GRAPH_SLOWLOG    : 'GRAPH.SLOWLOG';

// RedisGears Commands
TFUNCTION   : 'TFUNCTION';
TFCALL      : 'TFCALL';
TFCALLASYNC : 'TFCALLASYNC';

// Common Keywords
SCHEMA          : 'SCHEMA';
ADD             : 'ADD';
ON              : 'ON';
HASH            : 'HASH';
JSON            : 'JSON';
PREFIX          : 'PREFIX';
FILTER          : 'FILTER';
LANGUAGE        : 'LANGUAGE';
LANGUAGE_FIELD  : 'LANGUAGE_FIELD';
SCORE           : 'SCORE';
SCORE_FIELD     : 'SCORE_FIELD';
PAYLOAD_FIELD   : 'PAYLOAD_FIELD';
MAXTEXTFIELDS   : 'MAXTEXTFIELDS';
TEMPORARY       : 'TEMPORARY';
NOOFFSETS       : 'NOOFFSETS';
NOHL            : 'NOHL';
NOFIELDS        : 'NOFIELDS';
NOFREQS         : 'NOFREQS';
STOPWORDS       : 'STOPWORDS';
SKIPINITIALSCAN : 'SKIPINITIALSCAN';
TEXT            : 'TEXT';
TAG             : 'TAG';
NUMERIC         : 'NUMERIC';
GEO             : 'GEO';
VECTOR          : 'VECTOR';
GEOSHAPE        : 'GEOSHAPE';
FLAT            : 'FLAT';
HNSW            : 'HNSW';
NOSTEM          : 'NOSTEM';
WEIGHT          : 'WEIGHT';
PHONETIC        : 'PHONETIC';
SEPARATOR       : 'SEPARATOR';
CASESENSITIVE   : 'CASESENSITIVE';
SORTABLE        : 'SORTABLE';
UNF             : 'UNF';
NOINDEX         : 'NOINDEX';
AS              : 'AS';
NOCONTENT       : 'NOCONTENT';
VERBATIM        : 'VERBATIM';
NOSTOPWORDS     : 'NOSTOPWORDS';
WITHSCORES      : 'WITHSCORES';
WITHPAYLOADS    : 'WITHPAYLOADS';
WITHSORTKEYS    : 'WITHSORTKEYS';
GEOFILTER       : 'GEOFILTER';
INKEYS          : 'INKEYS';
INFIELDS        : 'INFIELDS';
RETURN          : 'RETURN';
SUMMARIZE       : 'SUMMARIZE';
HIGHLIGHT       : 'HIGHLIGHT';
FIELDS          : 'FIELDS';
FRAGS           : 'FRAGS';
LEN             : 'LEN';
TAGS            : 'TAGS';
SLOP            : 'SLOP';
TIMEOUT         : 'TIMEOUT';
INORDER         : 'INORDER';
EXPANDER        : 'EXPANDER';
SCORER          : 'SCORER';
EXPLAINSCORE    : 'EXPLAINSCORE';
PAYLOAD         : 'PAYLOAD';
SORTBY          : 'SORTBY';
LIMIT           : 'LIMIT';
PARAMS          : 'PARAMS';
DIALECT         : 'DIALECT';
LOAD            : 'LOAD';
GROUPBY         : 'GROUPBY';
REDUCE          : 'REDUCE';
APPLY           : 'APPLY';
WITHCURSOR      : 'WITHCURSOR';
COUNT           : 'COUNT';
MAXIDLE         : 'MAXIDLE';
DISTANCE        : 'DISTANCE';
TERMS           : 'TERMS';
INCLUDE         : 'INCLUDE';
EXCLUDE         : 'EXCLUDE';
INCR            : 'INCR';
MAX             : 'MAX';
FUZZY           : 'FUZZY';
GET             : 'GET';
SET             : 'SET';
HELP            : 'HELP';
READ            : 'READ';
DEL             : 'DEL';
SEARCH          : 'SEARCH';
AGGREGATE       : 'AGGREGATE';
LIMITED         : 'LIMITED';
QUERY           : 'QUERY';
DD              : 'DD';
ASC             : 'ASC';
DESC            : 'DESC';
NX              : 'NX';
XX              : 'XX';
INDENT          : 'INDENT';
NEWLINE         : 'NEWLINE';
SPACE           : 'SPACE';
FORMAT          : 'FORMAT';
MEMORY          : 'MEMORY';

// TimeSeries Keywords
RETENTION       : 'RETENTION';
ENCODING        : 'ENCODING';
COMPRESSED      : 'COMPRESSED';
UNCOMPRESSED    : 'UNCOMPRESSED';
CHUNK_SIZE      : 'CHUNK_SIZE';
DUPLICATE_POLICY: 'DUPLICATE_POLICY';
ON_DUPLICATE    : 'ON_DUPLICATE';
LABELS          : 'LABELS';
IGNORE          : 'IGNORE';
AGGREGATION     : 'AGGREGATION';
ALIGNTIMESTAMP  : 'ALIGNTIMESTAMP';
LATEST          : 'LATEST';
DEBUG           : 'DEBUG';
TIMESTAMP       : 'TIMESTAMP';
FILTER_BY_TS    : 'FILTER_BY_TS';
FILTER_BY_VALUE : 'FILTER_BY_VALUE';
ALIGN           : 'ALIGN';
BUCKETTIMESTAMP : 'BUCKETTIMESTAMP';
EMPTY           : 'EMPTY';
WITHLABELS      : 'WITHLABELS';
SELECTED_LABELS : 'SELECTED_LABELS';
AVG             : 'AVG';
SUM             : 'SUM';
MIN             : 'MIN';
RANGE           : 'RANGE';
FIRST           : 'FIRST';
LAST            : 'LAST';
STD_P           : 'STD.P';
STD_S           : 'STD.S';
VAR_P           : 'VAR.P';
VAR_S           : 'VAR.S';
TWA             : 'TWA';

// Bloom Filter Keywords
CAPACITY        : 'CAPACITY';
SIZE            : 'SIZE';
FILTERS         : 'FILTERS';
ITEMS           : 'ITEMS';
EXPANSION       : 'EXPANSION';
ERROR           : 'ERROR';
NOCREATE        : 'NOCREATE';
NONSCALING      : 'NONSCALING';
BUCKETSIZE      : 'BUCKETSIZE';
MAXITERATIONS   : 'MAXITERATIONS';
WEIGHTS         : 'WEIGHTS';
WITHCOUNT       : 'WITHCOUNT';
COMPRESSION     : 'COMPRESSION';
OVERRIDE        : 'OVERRIDE';

// Graph Keywords
CREATE          : 'CREATE';
DROP            : 'DROP';
MANDATORY       : 'MANDATORY';
UNIQUE          : 'UNIQUE';
NODE            : 'NODE';
RELATIONSHIP    : 'RELATIONSHIP';
PROPERTIES      : 'PROPERTIES';

// Gears Keywords
REPLACE         : 'REPLACE';
CONFIG          : 'CONFIG';
DELETE          : 'DELETE';
LIST            : 'LIST';
WITHCODE        : 'WITHCODE';
VERBOSE         : 'VERBOSE';
V               : 'V';
LIBRARY         : 'LIBRARY';

// Units
M               : 'M' | 'm';
KM              : 'KM' | 'km';
FT              : 'FT' | 'ft';
MI              : 'MI' | 'mi';

// Operators
PLUS            : '+';
MINUS           : '-';
STAR            : '*';
EQUALS          : '=';
TILDE           : '~';
COLON           : ':';
DOT             : '.';
UNDERSCORE      : '_';
LPAREN          : '(';
RPAREN          : ')';
LBRACKET        : '[';
RBRACKET        : ']';

// Literals
INTEGER         : '-'? [0-9]+;
NUMBER          : '-'? [0-9]+ ('.' [0-9]+)? ([eE] [+-]? [0-9]+)?;
STRING          : ('"' (~["\r\n] | '\\"')* '"') | ('\'' (~['\r\n] | '\\\'')* '\'') | [a-zA-Z_][a-zA-Z0-9_$.-]*;

// Whitespace
WS              : [ \t\r\n]+ -> skip;

// Comments
COMMENT         : '#' ~[\r\n]* -> skip;
