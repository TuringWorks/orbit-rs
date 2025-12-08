/*
 * MySQL 9.5 Lexer Grammar for ANTLR4
 * Includes MySQL AI/HeatWave GenAI extensions
 * 
 * Copyright 2025 - Open Source Grammar
 * Licensed under MIT License
 */

lexer grammar MySQLLexer;

options {
    caseInsensitive = true;
}

// =============================================================================
// KEYWORDS - Alphabetically ordered
// =============================================================================

// A
ACCESSIBLE:                     'ACCESSIBLE';
ACCOUNT:                        'ACCOUNT';
ACTION:                         'ACTION';
ACTIVE:                         'ACTIVE';
ADD:                            'ADD';
ADMIN:                          'ADMIN';
AFTER:                          'AFTER';
AGAINST:                        'AGAINST';
AGGREGATE:                      'AGGREGATE';
ALGORITHM:                      'ALGORITHM';
ALL:                            'ALL';
ALTER:                          'ALTER';
ALWAYS:                         'ALWAYS';
ANALYZE:                        'ANALYZE';
AND:                            'AND';
ANY:                            'ANY';
ARRAY:                          'ARRAY';
AS:                             'AS';
ASC:                            'ASC';
ASCII:                          'ASCII';
ASENSITIVE:                     'ASENSITIVE';
AT:                             'AT';
ATTRIBUTE:                      'ATTRIBUTE';
AUTHENTICATION:                 'AUTHENTICATION';
AUTOEXTEND_SIZE:                'AUTOEXTEND_SIZE';
AUTO_INCREMENT:                 'AUTO_INCREMENT';
AVG:                            'AVG';
AVG_ROW_LENGTH:                 'AVG_ROW_LENGTH';

// B
BACKUP:                         'BACKUP';
BEFORE:                         'BEFORE';
BEGIN:                          'BEGIN';
BETWEEN:                        'BETWEEN';
BIGINT:                         'BIGINT';
BINARY:                         'BINARY';
BINLOG:                         'BINLOG';
BIT:                            'BIT';
BLOB:                           'BLOB';
BLOCK:                          'BLOCK';
BOOL:                           'BOOL';
BOOLEAN:                        'BOOLEAN';
BOTH:                           'BOTH';
BTREE:                          'BTREE';
BUCKETS:                        'BUCKETS';
BULK:                           'BULK';
BY:                             'BY';
BYTE:                           'BYTE';

// C
CACHE:                          'CACHE';
CALL:                           'CALL';
CASCADE:                        'CASCADE';
CASCADED:                       'CASCADED';
CASE:                           'CASE';
CATALOG_NAME:                   'CATALOG_NAME';
CHAIN:                          'CHAIN';
CHALLENGE_RESPONSE:             'CHALLENGE_RESPONSE';
CHANGE:                         'CHANGE';
CHANGED:                        'CHANGED';
CHANNEL:                        'CHANNEL';
CHAR:                           'CHAR';
CHARACTER:                      'CHARACTER';
CHARSET:                        'CHARSET';
CHECK:                          'CHECK';
CHECKSUM:                       'CHECKSUM';
CIPHER:                         'CIPHER';
CLASS_ORIGIN:                   'CLASS_ORIGIN';
CLIENT:                         'CLIENT';
CLONE:                          'CLONE';
CLOSE:                          'CLOSE';
COALESCE:                       'COALESCE';
CODE:                           'CODE';
COLLATE:                        'COLLATE';
COLLATION:                      'COLLATION';
COLUMN:                         'COLUMN';
COLUMNS:                        'COLUMNS';
COLUMN_FORMAT:                  'COLUMN_FORMAT';
COLUMN_NAME:                    'COLUMN_NAME';
COMMENT:                        'COMMENT';
COMMIT:                         'COMMIT';
COMMITTED:                      'COMMITTED';
COMPACT:                        'COMPACT';
COMPLETION:                     'COMPLETION';
COMPONENT:                      'COMPONENT';
COMPRESSED:                     'COMPRESSED';
COMPRESSION:                    'COMPRESSION';
CONCURRENT:                     'CONCURRENT';
CONDITION:                      'CONDITION';
CONNECTION:                     'CONNECTION';
CONSISTENT:                     'CONSISTENT';
CONSTRAINT:                     'CONSTRAINT';
CONSTRAINT_CATALOG:             'CONSTRAINT_CATALOG';
CONSTRAINT_NAME:                'CONSTRAINT_NAME';
CONSTRAINT_SCHEMA:              'CONSTRAINT_SCHEMA';
CONTAINS:                       'CONTAINS';
CONTEXT:                        'CONTEXT';
CONTINUE:                       'CONTINUE';
CONVERT:                        'CONVERT';
CPU:                            'CPU';
CREATE:                         'CREATE';
CROSS:                          'CROSS';
CUBE:                           'CUBE';
CUME_DIST:                      'CUME_DIST';
CURRENT:                        'CURRENT';
CURRENT_DATE:                   'CURRENT_DATE';
CURRENT_TIME:                   'CURRENT_TIME';
CURRENT_TIMESTAMP:              'CURRENT_TIMESTAMP';
CURRENT_USER:                   'CURRENT_USER';
CURSOR:                         'CURSOR';
CURSOR_NAME:                    'CURSOR_NAME';

// D
DATA:                           'DATA';
DATABASE:                       'DATABASE';
DATABASES:                      'DATABASES';
DATAFILE:                       'DATAFILE';
DATE:                           'DATE';
DATETIME:                       'DATETIME';
DAY:                            'DAY';
DAY_HOUR:                       'DAY_HOUR';
DAY_MICROSECOND:                'DAY_MICROSECOND';
DAY_MINUTE:                     'DAY_MINUTE';
DAY_SECOND:                     'DAY_SECOND';
DEALLOCATE:                     'DEALLOCATE';
DEC:                            'DEC';
DECIMAL:                        'DECIMAL';
DECLARE:                        'DECLARE';
DEFAULT:                        'DEFAULT';
DEFAULT_AUTH:                   'DEFAULT_AUTH';
DEFINER:                        'DEFINER';
DEFINITION:                     'DEFINITION';
DELAYED:                        'DELAYED';
DELAY_KEY_WRITE:                'DELAY_KEY_WRITE';
DELETE:                         'DELETE';
DENSE_RANK:                     'DENSE_RANK';
DESC:                           'DESC';
DESCRIBE:                       'DESCRIBE';
DESCRIPTION:                    'DESCRIPTION';
DES_KEY_FILE:                   'DES_KEY_FILE';
DETERMINISTIC:                  'DETERMINISTIC';
DIAGNOSTICS:                    'DIAGNOSTICS';
DIRECTORY:                      'DIRECTORY';
DISABLE:                        'DISABLE';
DISCARD:                        'DISCARD';
DISK:                           'DISK';
DISTINCT:                       'DISTINCT';
DISTINCTROW:                    'DISTINCTROW';
DIV:                            'DIV';
DO:                             'DO';
DOUBLE:                         'DOUBLE';
DROP:                           'DROP';
DUAL:                           'DUAL';
DUMPFILE:                       'DUMPFILE';
DUPLICATE:                      'DUPLICATE';
DYNAMIC:                        'DYNAMIC';

// E
EACH:                           'EACH';
ELSE:                           'ELSE';
ELSEIF:                         'ELSEIF';
EMPTY:                          'EMPTY';
ENABLE:                         'ENABLE';
ENCLOSED:                       'ENCLOSED';
ENCRYPTION:                     'ENCRYPTION';
END:                            'END';
ENDS:                           'ENDS';
ENFORCED:                       'ENFORCED';
ENGINE:                         'ENGINE';
ENGINES:                        'ENGINES';
ENGINE_ATTRIBUTE:               'ENGINE_ATTRIBUTE';
ENUM:                           'ENUM';
ERROR:                          'ERROR';
ERRORS:                         'ERRORS';
ESCAPE:                         'ESCAPE';
ESCAPED:                        'ESCAPED';
EVENT:                          'EVENT';
EVENTS:                         'EVENTS';
EVERY:                          'EVERY';
EXCEPT:                         'EXCEPT';
EXCHANGE:                       'EXCHANGE';
EXCLUDE:                        'EXCLUDE';
EXECUTE:                        'EXECUTE';
EXISTS:                         'EXISTS';
EXIT:                           'EXIT';
EXPANSION:                      'EXPANSION';
EXPIRE:                         'EXPIRE';
EXPLAIN:                        'EXPLAIN';
EXPORT:                         'EXPORT';
EXTENDED:                       'EXTENDED';
EXTENT_SIZE:                    'EXTENT_SIZE';

// F
FACTOR:                         'FACTOR';
FAILED_LOGIN_ATTEMPTS:          'FAILED_LOGIN_ATTEMPTS';
FALSE:                          'FALSE';
FAST:                           'FAST';
FAULTS:                         'FAULTS';
FETCH:                          'FETCH';
FIELDS:                         'FIELDS';
FILE:                           'FILE';
FILE_BLOCK_SIZE:                'FILE_BLOCK_SIZE';
FILTER:                         'FILTER';
FINISH:                         'FINISH';
FIRST:                          'FIRST';
FIRST_VALUE:                    'FIRST_VALUE';
FIXED:                          'FIXED';
FLOAT:                          'FLOAT';
FLOAT4:                         'FLOAT4';
FLOAT8:                         'FLOAT8';
FLUSH:                          'FLUSH';
FOLLOWING:                      'FOLLOWING';
FOLLOWS:                        'FOLLOWS';
FOR:                            'FOR';
FORCE:                          'FORCE';
FOREIGN:                        'FOREIGN';
FORMAT:                         'FORMAT';
FOUND:                          'FOUND';
FROM:                           'FROM';
FULL:                           'FULL';
FULLTEXT:                       'FULLTEXT';
FUNCTION:                       'FUNCTION';

// G
GENERAL:                        'GENERAL';
GENERATE:                       'GENERATE';
GENERATED:                      'GENERATED';
GEOMCOLLECTION:                 'GEOMCOLLECTION';
GEOMETRY:                       'GEOMETRY';
GEOMETRYCOLLECTION:             'GEOMETRYCOLLECTION';
GET:                            'GET';
GET_FORMAT:                     'GET_FORMAT';
GET_SOURCE_PUBLIC_KEY:          'GET_SOURCE_PUBLIC_KEY';
GLOBAL:                         'GLOBAL';
GRANT:                          'GRANT';
GRANTS:                         'GRANTS';
GROUP:                          'GROUP';
GROUPING:                       'GROUPING';
GROUPS:                         'GROUPS';
GROUP_REPLICATION:              'GROUP_REPLICATION';
GTID_ONLY:                      'GTID_ONLY';

// H
HANDLER:                        'HANDLER';
HASH:                           'HASH';
HAVING:                         'HAVING';
HELP:                           'HELP';
HIGH_PRIORITY:                  'HIGH_PRIORITY';
HISTOGRAM:                      'HISTOGRAM';
HISTORY:                        'HISTORY';
HOST:                           'HOST';
HOSTS:                          'HOSTS';
HOUR:                           'HOUR';
HOUR_MICROSECOND:               'HOUR_MICROSECOND';
HOUR_MINUTE:                    'HOUR_MINUTE';
HOUR_SECOND:                    'HOUR_SECOND';

// I
IDENTIFIED:                     'IDENTIFIED';
IF:                             'IF';
IGNORE:                         'IGNORE';
IGNORE_SERVER_IDS:              'IGNORE_SERVER_IDS';
IMPORT:                         'IMPORT';
IN:                             'IN';
INACTIVE:                       'INACTIVE';
INDEX:                          'INDEX';
INDEXES:                        'INDEXES';
INFILE:                         'INFILE';
INITIAL:                        'INITIAL';
INITIAL_SIZE:                   'INITIAL_SIZE';
INITIATE:                       'INITIATE';
INNER:                          'INNER';
INOUT:                          'INOUT';
INSENSITIVE:                    'INSENSITIVE';
INSERT:                         'INSERT';
INSERT_METHOD:                  'INSERT_METHOD';
INSTALL:                        'INSTALL';
INSTANCE:                       'INSTANCE';
INT:                            'INT';
INT1:                           'INT1';
INT2:                           'INT2';
INT3:                           'INT3';
INT4:                           'INT4';
INT8:                           'INT8';
INTEGER:                        'INTEGER';
INTERSECT:                      'INTERSECT';
INTERVAL:                       'INTERVAL';
INTO:                           'INTO';
INVISIBLE:                      'INVISIBLE';
INVOKER:                        'INVOKER';
IO:                             'IO';
IO_AFTER_GTIDS:                 'IO_AFTER_GTIDS';
IO_BEFORE_GTIDS:                'IO_BEFORE_GTIDS';
IO_THREAD:                      'IO_THREAD';
IPC:                            'IPC';
IS:                             'IS';
ISOLATION:                      'ISOLATION';
ISSUER:                         'ISSUER';
ITERATE:                        'ITERATE';

// J
JAVASCRIPT:                     'JAVASCRIPT';
JOIN:                           'JOIN';
JSON:                           'JSON';
JSON_TABLE:                     'JSON_TABLE';
JSON_VALUE:                     'JSON_VALUE';

// K
KEY:                            'KEY';
KEYRING:                        'KEYRING';
KEYS:                           'KEYS';
KEY_BLOCK_SIZE:                 'KEY_BLOCK_SIZE';
KILL:                           'KILL';

// L
LAG:                            'LAG';
LANGUAGE:                       'LANGUAGE';
LAST:                           'LAST';
LAST_VALUE:                     'LAST_VALUE';
LATERAL:                        'LATERAL';
LEAD:                           'LEAD';
LEADING:                        'LEADING';
LEAVE:                          'LEAVE';
LEAVES:                         'LEAVES';
LEFT:                           'LEFT';
LESS:                           'LESS';
LEVEL:                          'LEVEL';
LIKE:                           'LIKE';
LIMIT:                          'LIMIT';
LINEAR:                         'LINEAR';
LINES:                          'LINES';
LINESTRING:                     'LINESTRING';
LIST:                           'LIST';
LOAD:                           'LOAD';
LOCAL:                          'LOCAL';
LOCALTIME:                      'LOCALTIME';
LOCALTIMESTAMP:                 'LOCALTIMESTAMP';
LOCK:                           'LOCK';
LOCKED:                         'LOCKED';
LOCKS:                          'LOCKS';
LOGFILE:                        'LOGFILE';
LOGS:                           'LOGS';
LONG:                           'LONG';
LONGBLOB:                       'LONGBLOB';
LONGTEXT:                       'LONGTEXT';
LOOP:                           'LOOP';
LOW_PRIORITY:                   'LOW_PRIORITY';

// M
MASTER:                         'MASTER';
MASTER_AUTO_POSITION:           'MASTER_AUTO_POSITION';
MASTER_BIND:                    'MASTER_BIND';
MASTER_COMPRESSION_ALGORITHMS:  'MASTER_COMPRESSION_ALGORITHMS';
MASTER_CONNECT_RETRY:           'MASTER_CONNECT_RETRY';
MASTER_DELAY:                   'MASTER_DELAY';
MASTER_HEARTBEAT_PERIOD:        'MASTER_HEARTBEAT_PERIOD';
MASTER_HOST:                    'MASTER_HOST';
MASTER_LOG_FILE:                'MASTER_LOG_FILE';
MASTER_LOG_POS:                 'MASTER_LOG_POS';
MASTER_PASSWORD:                'MASTER_PASSWORD';
MASTER_PORT:                    'MASTER_PORT';
MASTER_PUBLIC_KEY_PATH:         'MASTER_PUBLIC_KEY_PATH';
MASTER_RETRY_COUNT:             'MASTER_RETRY_COUNT';
MASTER_SERVER_ID:               'MASTER_SERVER_ID';
MASTER_SSL:                     'MASTER_SSL';
MASTER_SSL_CA:                  'MASTER_SSL_CA';
MASTER_SSL_CAPATH:              'MASTER_SSL_CAPATH';
MASTER_SSL_CERT:                'MASTER_SSL_CERT';
MASTER_SSL_CIPHER:              'MASTER_SSL_CIPHER';
MASTER_SSL_CRL:                 'MASTER_SSL_CRL';
MASTER_SSL_CRLPATH:             'MASTER_SSL_CRLPATH';
MASTER_SSL_KEY:                 'MASTER_SSL_KEY';
MASTER_SSL_VERIFY_SERVER_CERT:  'MASTER_SSL_VERIFY_SERVER_CERT';
MASTER_TLS_CIPHERSUITES:        'MASTER_TLS_CIPHERSUITES';
MASTER_TLS_VERSION:             'MASTER_TLS_VERSION';
MASTER_USER:                    'MASTER_USER';
MASTER_ZSTD_COMPRESSION_LEVEL:  'MASTER_ZSTD_COMPRESSION_LEVEL';
MATCH:                          'MATCH';
MAXVALUE:                       'MAXVALUE';
MAX_CONNECTIONS_PER_HOUR:       'MAX_CONNECTIONS_PER_HOUR';
MAX_QUERIES_PER_HOUR:           'MAX_QUERIES_PER_HOUR';
MAX_ROWS:                       'MAX_ROWS';
MAX_SIZE:                       'MAX_SIZE';
MAX_UPDATES_PER_HOUR:           'MAX_UPDATES_PER_HOUR';
MAX_USER_CONNECTIONS:           'MAX_USER_CONNECTIONS';
MEDIUM:                         'MEDIUM';
MEDIUMBLOB:                     'MEDIUMBLOB';
MEDIUMINT:                      'MEDIUMINT';
MEDIUMTEXT:                     'MEDIUMTEXT';
MEMBER:                         'MEMBER';
MEMORY:                         'MEMORY';
MERGE:                          'MERGE';
MESSAGE_TEXT:                   'MESSAGE_TEXT';
MICROSECOND:                    'MICROSECOND';
MIDDLEINT:                      'MIDDLEINT';
MIGRATE:                        'MIGRATE';
MINUTE:                         'MINUTE';
MINUTE_MICROSECOND:             'MINUTE_MICROSECOND';
MINUTE_SECOND:                  'MINUTE_SECOND';
MIN_ROWS:                       'MIN_ROWS';
MOD:                            'MOD';
MODE:                           'MODE';
MODIFIES:                       'MODIFIES';
MODIFY:                         'MODIFY';
MONTH:                          'MONTH';
MULTILINESTRING:                'MULTILINESTRING';
MULTIPOINT:                     'MULTIPOINT';
MULTIPOLYGON:                   'MULTIPOLYGON';
MUTEX:                          'MUTEX';
MYSQL_ERRNO:                    'MYSQL_ERRNO';

// MySQL AI / HeatWave ML Keywords
ML_TRAIN:                       'ML_TRAIN';
ML_PREDICT_ROW:                 'ML_PREDICT_ROW';
ML_PREDICT_TABLE:               'ML_PREDICT_TABLE';
ML_EXPLAIN_ROW:                 'ML_EXPLAIN_ROW';
ML_EXPLAIN_TABLE:               'ML_EXPLAIN_TABLE';
ML_SCORE:                       'ML_SCORE';
ML_MODEL_LOAD:                  'ML_MODEL_LOAD';
ML_MODEL_UNLOAD:                'ML_MODEL_UNLOAD';
ML_MODEL_IMPORT:                'ML_MODEL_IMPORT';
ML_MODEL_EXPORT:                'ML_MODEL_EXPORT';
ML_GENERATE:                    'ML_GENERATE';
ML_GENERATE_TABLE:              'ML_GENERATE_TABLE';
ML_EMBED:                       'ML_EMBED';
ML_EMBED_TABLE:                 'ML_EMBED_TABLE';
ML_NL_SQL:                      'ML_NL_SQL';

// N
NAME:                           'NAME';
NAMES:                          'NAMES';
NATIONAL:                       'NATIONAL';
NATURAL:                        'NATURAL';
NCHAR:                          'NCHAR';
NDB:                            'NDB';
NDBCLUSTER:                     'NDBCLUSTER';
NESTED:                         'NESTED';
NETWORK_NAMESPACE:              'NETWORK_NAMESPACE';
NEVER:                          'NEVER';
NEW:                            'NEW';
NEXT:                           'NEXT';
NO:                             'NO';
NODEGROUP:                      'NODEGROUP';
NONE:                           'NONE';
NOT:                            'NOT';
NOWAIT:                         'NOWAIT';
NO_WAIT:                        'NO_WAIT';
NO_WRITE_TO_BINLOG:             'NO_WRITE_TO_BINLOG';
NTH_VALUE:                      'NTH_VALUE';
NTILE:                          'NTILE';
NULL:                           'NULL';
NULLS:                          'NULLS';
NUMBER:                         'NUMBER';
NUMERIC:                        'NUMERIC';
NVARCHAR:                       'NVARCHAR';

// O
OF:                             'OF';
OFF:                            'OFF';
OFFSET:                         'OFFSET';
OJ:                             'OJ';
OLD:                            'OLD';
ON:                             'ON';
ONE:                            'ONE';
ONLY:                           'ONLY';
OPEN:                           'OPEN';
OPTIMIZE:                       'OPTIMIZE';
OPTIMIZER_COSTS:                'OPTIMIZER_COSTS';
OPTION:                         'OPTION';
OPTIONAL:                       'OPTIONAL';
OPTIONALLY:                     'OPTIONALLY';
OPTIONS:                        'OPTIONS';
OR:                             'OR';
ORDER:                          'ORDER';
ORDINALITY:                     'ORDINALITY';
ORGANIZATION:                   'ORGANIZATION';
OTHERS:                         'OTHERS';
OUT:                            'OUT';
OUTER:                          'OUTER';
OUTFILE:                        'OUTFILE';
OVER:                           'OVER';
OWNER:                          'OWNER';

// P
PACK_KEYS:                      'PACK_KEYS';
PAGE:                           'PAGE';
PARALLEL:                       'PARALLEL';
PARSER:                         'PARSER';
PARSE_GCOL_EXPR:                'PARSE_GCOL_EXPR';
PARTIAL:                        'PARTIAL';
PARTITION:                      'PARTITION';
PARTITIONING:                   'PARTITIONING';
PARTITIONS:                     'PARTITIONS';
PASSWORD:                       'PASSWORD';
PASSWORD_LOCK_TIME:             'PASSWORD_LOCK_TIME';
PATH:                           'PATH';
PERCENT_RANK:                   'PERCENT_RANK';
PERSIST:                        'PERSIST';
PERSIST_ONLY:                   'PERSIST_ONLY';
PHASE:                          'PHASE';
PLUGIN:                         'PLUGIN';
PLUGINS:                        'PLUGINS';
PLUGIN_DIR:                     'PLUGIN_DIR';
POINT:                          'POINT';
POLYGON:                        'POLYGON';
PORT:                           'PORT';
PRECEDES:                       'PRECEDES';
PRECEDING:                      'PRECEDING';
PRECISION:                      'PRECISION';
PREPARE:                        'PREPARE';
PRESERVE:                       'PRESERVE';
PREV:                           'PREV';
PRIMARY:                        'PRIMARY';
PRIVILEGE_CHECKS_USER:          'PRIVILEGE_CHECKS_USER';
PRIVILEGES:                     'PRIVILEGES';
PROCEDURE:                      'PROCEDURE';
PROCESS:                        'PROCESS';
PROCESSLIST:                    'PROCESSLIST';
PROFILE:                        'PROFILE';
PROFILES:                       'PROFILES';
PROXY:                          'PROXY';
PURGE:                          'PURGE';

// Q
QUARTER:                        'QUARTER';
QUERY:                          'QUERY';
QUICK:                          'QUICK';

// R
RANDOM:                         'RANDOM';
RANGE:                          'RANGE';
RANK:                           'RANK';
RAPID:                          'RAPID';
READ:                           'READ';
READS:                          'READS';
READ_ONLY:                      'READ_ONLY';
READ_WRITE:                     'READ_WRITE';
REAL:                           'REAL';
REBUILD:                        'REBUILD';
RECOVER:                        'RECOVER';
RECURSIVE:                      'RECURSIVE';
REDO_BUFFER_SIZE:               'REDO_BUFFER_SIZE';
REDUNDANT:                      'REDUNDANT';
REFERENCE:                      'REFERENCE';
REFERENCES:                     'REFERENCES';
REGEXP:                         'REGEXP';
REGISTRATION:                   'REGISTRATION';
RELAY:                          'RELAY';
RELAYLOG:                       'RELAYLOG';
RELAY_LOG_FILE:                 'RELAY_LOG_FILE';
RELAY_LOG_POS:                  'RELAY_LOG_POS';
RELAY_THREAD:                   'RELAY_THREAD';
RELEASE:                        'RELEASE';
RELOAD:                         'RELOAD';
REMOTE:                         'REMOTE';
REMOVE:                         'REMOVE';
RENAME:                         'RENAME';
REORGANIZE:                     'REORGANIZE';
REPAIR:                         'REPAIR';
REPEAT:                         'REPEAT';
REPEATABLE:                     'REPEATABLE';
REPLACE:                        'REPLACE';
REPLICA:                        'REPLICA';
REPLICAS:                       'REPLICAS';
REPLICATE_DO_DB:                'REPLICATE_DO_DB';
REPLICATE_DO_TABLE:             'REPLICATE_DO_TABLE';
REPLICATE_IGNORE_DB:            'REPLICATE_IGNORE_DB';
REPLICATE_IGNORE_TABLE:         'REPLICATE_IGNORE_TABLE';
REPLICATE_REWRITE_DB:           'REPLICATE_REWRITE_DB';
REPLICATE_WILD_DO_TABLE:        'REPLICATE_WILD_DO_TABLE';
REPLICATE_WILD_IGNORE_TABLE:    'REPLICATE_WILD_IGNORE_TABLE';
REPLICATION:                    'REPLICATION';
REQUIRE:                        'REQUIRE';
REQUIRE_ROW_FORMAT:             'REQUIRE_ROW_FORMAT';
REQUIRE_TABLE_PRIMARY_KEY_CHECK:'REQUIRE_TABLE_PRIMARY_KEY_CHECK';
RESET:                          'RESET';
RESIGNAL:                       'RESIGNAL';
RESOURCE:                       'RESOURCE';
RESPECT:                        'RESPECT';
RESTART:                        'RESTART';
RESTORE:                        'RESTORE';
RESTRICT:                       'RESTRICT';
RESUME:                         'RESUME';
RETAIN:                         'RETAIN';
RETURN:                         'RETURN';
RETURNED_SQLSTATE:              'RETURNED_SQLSTATE';
RETURNING:                      'RETURNING';
RETURNS:                        'RETURNS';
REUSE:                          'REUSE';
REVERSE:                        'REVERSE';
REVOKE:                         'REVOKE';
RIGHT:                          'RIGHT';
RLIKE:                          'RLIKE';
ROLE:                           'ROLE';
ROLLBACK:                       'ROLLBACK';
ROLLUP:                         'ROLLUP';
ROTATE:                         'ROTATE';
ROUTINE:                        'ROUTINE';
ROW:                            'ROW';
ROWS:                           'ROWS';
ROW_COUNT:                      'ROW_COUNT';
ROW_FORMAT:                     'ROW_FORMAT';
ROW_NUMBER:                     'ROW_NUMBER';
RTREE:                          'RTREE';

// S
SAVEPOINT:                      'SAVEPOINT';
SCHEDULE:                       'SCHEDULE';
SCHEMA:                         'SCHEMA';
SCHEMAS:                        'SCHEMAS';
SCHEMA_NAME:                    'SCHEMA_NAME';
SECOND:                         'SECOND';
SECONDARY:                      'SECONDARY';
SECONDARY_ENGINE:               'SECONDARY_ENGINE';
SECONDARY_ENGINE_ATTRIBUTE:     'SECONDARY_ENGINE_ATTRIBUTE';
SECONDARY_LOAD:                 'SECONDARY_LOAD';
SECONDARY_UNLOAD:               'SECONDARY_UNLOAD';
SECOND_MICROSECOND:             'SECOND_MICROSECOND';
SECURITY:                       'SECURITY';
SELECT:                         'SELECT';
SENSITIVE:                      'SENSITIVE';
SEPARATOR:                      'SEPARATOR';
SERIAL:                         'SERIAL';
SERIALIZABLE:                   'SERIALIZABLE';
SERVER:                         'SERVER';
SESSION:                        'SESSION';
SET:                            'SET';
SHARE:                          'SHARE';
SHOW:                           'SHOW';
SHUTDOWN:                       'SHUTDOWN';
SIGNAL:                         'SIGNAL';
SIGNED:                         'SIGNED';
SIMPLE:                         'SIMPLE';
SKIP:                           'SKIP';
SLAVE:                          'SLAVE';
SLOW:                           'SLOW';
SMALLINT:                       'SMALLINT';
SNAPSHOT:                       'SNAPSHOT';
SOCKET:                         'SOCKET';
SOME:                           'SOME';
SONAME:                         'SONAME';
SOUNDS:                         'SOUNDS';
SOURCE:                         'SOURCE';
SOURCE_AUTO_POSITION:           'SOURCE_AUTO_POSITION';
SOURCE_BIND:                    'SOURCE_BIND';
SOURCE_COMPRESSION_ALGORITHMS:  'SOURCE_COMPRESSION_ALGORITHMS';
SOURCE_CONNECT_RETRY:           'SOURCE_CONNECT_RETRY';
SOURCE_DELAY:                   'SOURCE_DELAY';
SOURCE_HEARTBEAT_PERIOD:        'SOURCE_HEARTBEAT_PERIOD';
SOURCE_HOST:                    'SOURCE_HOST';
SOURCE_LOG_FILE:                'SOURCE_LOG_FILE';
SOURCE_LOG_POS:                 'SOURCE_LOG_POS';
SOURCE_PASSWORD:                'SOURCE_PASSWORD';
SOURCE_PORT:                    'SOURCE_PORT';
SOURCE_PUBLIC_KEY_PATH:         'SOURCE_PUBLIC_KEY_PATH';
SOURCE_RETRY_COUNT:             'SOURCE_RETRY_COUNT';
SOURCE_SSL:                     'SOURCE_SSL';
SOURCE_SSL_CA:                  'SOURCE_SSL_CA';
SOURCE_SSL_CAPATH:              'SOURCE_SSL_CAPATH';
SOURCE_SSL_CERT:                'SOURCE_SSL_CERT';
SOURCE_SSL_CIPHER:              'SOURCE_SSL_CIPHER';
SOURCE_SSL_CRL:                 'SOURCE_SSL_CRL';
SOURCE_SSL_CRLPATH:             'SOURCE_SSL_CRLPATH';
SOURCE_SSL_KEY:                 'SOURCE_SSL_KEY';
SOURCE_SSL_VERIFY_SERVER_CERT:  'SOURCE_SSL_VERIFY_SERVER_CERT';
SOURCE_TLS_CIPHERSUITES:        'SOURCE_TLS_CIPHERSUITES';
SOURCE_TLS_VERSION:             'SOURCE_TLS_VERSION';
SOURCE_USER:                    'SOURCE_USER';
SOURCE_ZSTD_COMPRESSION_LEVEL:  'SOURCE_ZSTD_COMPRESSION_LEVEL';
SPATIAL:                        'SPATIAL';
SPECIFIC:                       'SPECIFIC';
SQL:                            'SQL';
SQLEXCEPTION:                   'SQLEXCEPTION';
SQLSTATE:                       'SQLSTATE';
SQLWARNING:                     'SQLWARNING';
SQL_AFTER_GTIDS:                'SQL_AFTER_GTIDS';
SQL_AFTER_MTS_GAPS:             'SQL_AFTER_MTS_GAPS';
SQL_BEFORE_GTIDS:               'SQL_BEFORE_GTIDS';
SQL_BIG_RESULT:                 'SQL_BIG_RESULT';
SQL_BUFFER_RESULT:              'SQL_BUFFER_RESULT';
SQL_CACHE:                      'SQL_CACHE';
SQL_CALC_FOUND_ROWS:            'SQL_CALC_FOUND_ROWS';
SQL_NO_CACHE:                   'SQL_NO_CACHE';
SQL_SMALL_RESULT:               'SQL_SMALL_RESULT';
SQL_THREAD:                     'SQL_THREAD';
SQL_TSI_DAY:                    'SQL_TSI_DAY';
SQL_TSI_HOUR:                   'SQL_TSI_HOUR';
SQL_TSI_MINUTE:                 'SQL_TSI_MINUTE';
SQL_TSI_MONTH:                  'SQL_TSI_MONTH';
SQL_TSI_QUARTER:                'SQL_TSI_QUARTER';
SQL_TSI_SECOND:                 'SQL_TSI_SECOND';
SQL_TSI_WEEK:                   'SQL_TSI_WEEK';
SQL_TSI_YEAR:                   'SQL_TSI_YEAR';
SRID:                           'SRID';
SSL:                            'SSL';
STACKED:                        'STACKED';
START:                          'START';
STARTING:                       'STARTING';
STARTS:                         'STARTS';
STATS_AUTO_RECALC:              'STATS_AUTO_RECALC';
STATS_PERSISTENT:               'STATS_PERSISTENT';
STATS_SAMPLE_PAGES:             'STATS_SAMPLE_PAGES';
STATUS:                         'STATUS';
STOP:                           'STOP';
STORAGE:                        'STORAGE';
STORED:                         'STORED';
STRAIGHT_JOIN:                  'STRAIGHT_JOIN';
STREAM:                         'STREAM';
STRING:                         'STRING';
SUBCLASS_ORIGIN:                'SUBCLASS_ORIGIN';
SUBJECT:                        'SUBJECT';
SUBPARTITION:                   'SUBPARTITION';
SUBPARTITIONS:                  'SUBPARTITIONS';
SUPER:                          'SUPER';
SUSPEND:                        'SUSPEND';
SWAPS:                          'SWAPS';
SWITCHES:                       'SWITCHES';
SYSTEM:                         'SYSTEM';

// T
TABLE:                          'TABLE';
TABLES:                         'TABLES';
TABLESPACE:                     'TABLESPACE';
TABLE_CHECKSUM:                 'TABLE_CHECKSUM';
TABLE_NAME:                     'TABLE_NAME';
TEMPORARY:                      'TEMPORARY';
TEMPTABLE:                      'TEMPTABLE';
TERMINATED:                     'TERMINATED';
TEXT:                           'TEXT';
THAN:                           'THAN';
THEN:                           'THEN';
THREAD_PRIORITY:                'THREAD_PRIORITY';
TIES:                           'TIES';
TIME:                           'TIME';
TIMESTAMP:                      'TIMESTAMP';
TIMESTAMPADD:                   'TIMESTAMPADD';
TIMESTAMPDIFF:                  'TIMESTAMPDIFF';
TINYBLOB:                       'TINYBLOB';
TINYINT:                        'TINYINT';
TINYTEXT:                       'TINYTEXT';
TLS:                            'TLS';
TO:                             'TO';
TRAILING:                       'TRAILING';
TRANSACTION:                    'TRANSACTION';
TRIGGER:                        'TRIGGER';
TRIGGERS:                       'TRIGGERS';
TRUE:                           'TRUE';
TRUNCATE:                       'TRUNCATE';
TYPE:                           'TYPE';
TYPES:                          'TYPES';

// U
UNBOUNDED:                      'UNBOUNDED';
UNCOMMITTED:                    'UNCOMMITTED';
UNDEFINED:                      'UNDEFINED';
UNDO:                           'UNDO';
UNDOFILE:                       'UNDOFILE';
UNDO_BUFFER_SIZE:               'UNDO_BUFFER_SIZE';
UNICODE:                        'UNICODE';
UNINSTALL:                      'UNINSTALL';
UNION:                          'UNION';
UNIQUE:                         'UNIQUE';
UNKNOWN:                        'UNKNOWN';
UNLOCK:                         'UNLOCK';
UNREGISTER:                     'UNREGISTER';
UNSIGNED:                       'UNSIGNED';
UNTIL:                          'UNTIL';
UPDATE:                         'UPDATE';
UPGRADE:                        'UPGRADE';
URL:                            'URL';
URI:                            'URI';
USAGE:                          'USAGE';
USE:                            'USE';
USER:                           'USER';
USER_RESOURCES:                 'USER_RESOURCES';
USE_FRM:                        'USE_FRM';
USING:                          'USING';
UTC_DATE:                       'UTC_DATE';
UTC_TIME:                       'UTC_TIME';
UTC_TIMESTAMP:                  'UTC_TIMESTAMP';

// V
VALIDATION:                     'VALIDATION';
VALUE:                          'VALUE';
VALUES:                         'VALUES';
VARBINARY:                      'VARBINARY';
VARCHAR:                        'VARCHAR';
VARCHARACTER:                   'VARCHARACTER';
VARIABLES:                      'VARIABLES';
VARYING:                        'VARYING';
VCPU:                           'VCPU';

// VECTOR type (MySQL 9.0+)
VECTOR:                         'VECTOR';

// Vector functions
STRING_TO_VECTOR:               'STRING_TO_VECTOR';
TO_VECTOR:                      'TO_VECTOR';
VECTOR_TO_STRING:               'VECTOR_TO_STRING';
FROM_VECTOR:                    'FROM_VECTOR';
VECTOR_DIM:                     'VECTOR_DIM';

VIEW:                           'VIEW';
VIRTUAL:                        'VIRTUAL';
VISIBLE:                        'VISIBLE';

// W
WAIT:                           'WAIT';
WARNINGS:                       'WARNINGS';
WEEK:                           'WEEK';
WEIGHT_STRING:                  'WEIGHT_STRING';
WHEN:                           'WHEN';
WHERE:                          'WHERE';
WHILE:                          'WHILE';
WINDOW:                         'WINDOW';
WITH:                           'WITH';
WITHOUT:                        'WITHOUT';
WORK:                           'WORK';
WRAPPER:                        'WRAPPER';
WRITE:                          'WRITE';

// X
X509:                           'X509';
XA:                             'XA';
XID:                            'XID';
XML:                            'XML';
XOR:                            'XOR';

// Y
YEAR:                           'YEAR';
YEAR_MONTH:                     'YEAR_MONTH';

// Z
ZEROFILL:                       'ZEROFILL';
ZONE:                           'ZONE';

// =============================================================================
// OPERATORS AND SYMBOLS
// =============================================================================

// Comparison operators
EQUAL:                          '=';
ASSIGN_OP:                      ':=';
NULL_SAFE_EQUAL:                '<=>';
GREATER_THAN:                   '>';
GREATER_THAN_OR_EQUAL:          '>=';
LESS_THAN:                      '<';
LESS_THAN_OR_EQUAL:             '<=';
NOT_EQUAL:                      '<>' | '!=';

// Arithmetic operators
PLUS:                           '+';
MINUS:                          '-';
ASTERISK:                       '*';
SLASH:                          '/';
PERCENT:                        '%';

// Bitwise operators
AMPERSAND:                      '&';
PIPE:                           '|';
CARET:                          '^';
TILDE:                          '~';
LEFT_SHIFT:                     '<<';
RIGHT_SHIFT:                    '>>';

// Logical operators
EXCLAMATION:                    '!';
DOUBLE_AMPERSAND:               '&&';
DOUBLE_PIPE:                    '||';

// Punctuation
OPEN_PAREN:                     '(';
CLOSE_PAREN:                    ')';
OPEN_BRACE:                     '{';
CLOSE_BRACE:                    '}';
OPEN_BRACKET:                   '[';
CLOSE_BRACKET:                  ']';
COMMA:                          ',';
DOT:                            '.';
SEMICOLON:                      ';';
COLON:                          ':';
DOUBLE_COLON:                   '::';
AT_SIGN:                        '@';
DOUBLE_AT_SIGN:                 '@@';
POUND:                          '#';
QUESTION:                       '?';
ARROW:                          '->';
DOUBLE_ARROW:                   '->>';
BACK_ARROW:                     '<-';

// =============================================================================
// LITERALS
// =============================================================================

// String literals
SINGLE_QUOTED_STRING:           '\'' (~'\'' | '\\' . | '\'\'')* '\'';
DOUBLE_QUOTED_STRING:           '"' (~'"' | '\\' . | '""')* '"';

// Bit value literal
BIT_STRING:                     [bB] '\'' [01]+ '\'';

// Hex literal  
HEX_STRING:                     [xX] '\'' [0-9a-fA-F]+ '\''
                              | '0' [xX] [0-9a-fA-F]+;

// Numeric literals
DECIMAL_LITERAL:                [0-9]+;
REAL_LITERAL:                   ([0-9]+ '.' [0-9]* | '.' [0-9]+) ([eE] [+-]? [0-9]+)?
                              | [0-9]+ [eE] [+-]? [0-9]+;

// =============================================================================
// IDENTIFIERS
// =============================================================================

// Backtick quoted identifier
BACKTICK_QUOTED_ID:             '`' (~'`' | '``')* '`';

// Regular identifier
ID:                             [a-zA-Z_$] [a-zA-Z0-9_$]*;

// Unicode identifier
UNICODE_ID:                     [\p{L}] [\p{L}\p{N}_$]*;

// =============================================================================
// VARIABLES
// =============================================================================

// User-defined variable
USER_VAR:                       '@' [a-zA-Z_$] [a-zA-Z0-9_$]*
                              | '@' '\'' (~'\'' | '\\' . | '\'\'')* '\''
                              | '@' '"' (~'"' | '\\' . | '""')* '"'
                              | '@' '`' (~'`' | '``')* '`';

// System variable
SYSTEM_VAR:                     '@@' ([a-zA-Z_$] [a-zA-Z0-9_$]* '.')? [a-zA-Z_$] [a-zA-Z0-9_$]*;

// =============================================================================
// COMMENTS
// =============================================================================

// Single-line comment
SINGLE_LINE_COMMENT:            ('--' [ \t]? | '#') ~[\r\n]* -> channel(HIDDEN);

// Multi-line comment
MULTI_LINE_COMMENT:             '/*' .*? '*/' -> channel(HIDDEN);

// MySQL hint comment
HINT_COMMENT:                   '/*+' .*? '*/';

// MySQL version comment
VERSION_COMMENT:                '/*!' [0-9]* .*? '*/';

// =============================================================================
// WHITESPACE
// =============================================================================

WHITESPACE:                     [ \t\r\n]+ -> channel(HIDDEN);

// =============================================================================
// ERROR HANDLING
// =============================================================================

// Catch-all for any unrecognized characters
ERROR_TOKEN:                    .;
