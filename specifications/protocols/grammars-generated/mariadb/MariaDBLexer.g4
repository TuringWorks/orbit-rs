/*
 * MariaDB 12.2 Lexer Grammar for ANTLR4
 * 
 * This grammar is designed for MariaDB 12.2 and includes all reserved words,
 * keywords, operators, and lexical elements specific to MariaDB SQL dialect.
 * 
 * Based on official MariaDB 12.2 documentation and syntax specifications.
 * 
 * License: MIT
 */

lexer grammar MariaDBLexer;

channels { MYSQLCOMMENT, ERRORCHANNEL }

// ===========================================================================
// FRAGMENTS
// ===========================================================================

fragment A: [aA];
fragment B: [bB];
fragment C: [cC];
fragment D: [dD];
fragment E: [eE];
fragment F: [fF];
fragment G: [gG];
fragment H: [hH];
fragment I: [iI];
fragment J: [jJ];
fragment K: [kK];
fragment L: [lL];
fragment M: [mM];
fragment N: [nN];
fragment O: [oO];
fragment P: [pP];
fragment Q: [qQ];
fragment R: [rR];
fragment S: [sS];
fragment T: [tT];
fragment U: [uU];
fragment V: [vV];
fragment W: [wW];
fragment X: [xX];
fragment Y: [yY];
fragment Z: [zZ];

fragment DEC_DIGIT: [0-9];
fragment HEX_DIGIT: [0-9A-Fa-f];
fragment BIT_DIGIT: [01];
fragment EXPONENT_NUM_PART: E [-+]? DEC_DIGIT+;

fragment ID_LITERAL: [A-Za-z_$] [A-Za-z_$0-9]*;
fragment DQUOTA_STRING: '"' ( '\\'. | '""' | ~[\\"] )* '"';
fragment SQUOTA_STRING: '\'' ( '\\'. | '\'\'' | ~[\\'] )* '\'';
fragment BQUOTA_STRING: '`' ( '\\'. | '``' | ~[\\`] )* '`';

// ===========================================================================
// WHITESPACE AND COMMENTS
// ===========================================================================

SPACE: [ \t\r\n]+ -> channel(HIDDEN);
SPEC_MYSQL_COMMENT: '/*!' .+? '*/' -> channel(MYSQLCOMMENT);
COMMENT_INPUT: '/*' .*? '*/' -> channel(HIDDEN);
LINE_COMMENT: (
    ('-- ' | '#') ~[\r\n]* ('\r'? '\n' | EOF)
    | '--' ('\r'? '\n' | EOF)
) -> channel(HIDDEN);

// ===========================================================================
// RESERVED KEYWORDS (MariaDB 12.2)
// ===========================================================================

// A
ACCESSIBLE:                          A C C E S S I B L E;
ADD:                                 A D D;
ALL:                                 A L L;
ALTER:                               A L T E R;
ANALYZE:                             A N A L Y Z E;
AND:                                 A N D;
AS:                                  A S;
ASC:                                 A S C;
ASENSITIVE:                          A S E N S I T I V E;

// B
BEFORE:                              B E F O R E;
BETWEEN:                             B E T W E E N;
BIGINT:                              B I G I N T;
BINARY:                              B I N A R Y;
BLOB:                                B L O B;
BOTH:                                B O T H;
BY:                                  B Y;

// C
CALL:                                C A L L;
CASCADE:                             C A S C A D E;
CASE:                                C A S E;
CHANGE:                              C H A N G E;
CHAR:                                C H A R;
CHARACTER:                           C H A R A C T E R;
CHECK:                               C H E C K;
COLLATE:                             C O L L A T E;
COLUMN:                              C O L U M N;
CONDITION:                           C O N D I T I O N;
CONSTRAINT:                          C O N S T R A I N T;
CONTINUE:                            C O N T I N U E;
CONVERT:                             C O N V E R T;
CREATE:                              C R E A T E;
CROSS:                               C R O S S;
CURRENT_DATE:                        C U R R E N T '_' D A T E;
CURRENT_ROLE:                        C U R R E N T '_' R O L E;
CURRENT_TIME:                        C U R R E N T '_' T I M E;
CURRENT_TIMESTAMP:                   C U R R E N T '_' T I M E S T A M P;
CURRENT_USER:                        C U R R E N T '_' U S E R;
CURSOR:                              C U R S O R;

// D
DATABASE:                            D A T A B A S E;
DATABASES:                           D A T A B A S E S;
DAY_HOUR:                            D A Y '_' H O U R;
DAY_MICROSECOND:                     D A Y '_' M I C R O S E C O N D;
DAY_MINUTE:                          D A Y '_' M I N U T E;
DAY_SECOND:                          D A Y '_' S E C O N D;
DEC:                                 D E C;
DECIMAL:                             D E C I M A L;
DECLARE:                             D E C L A R E;
DEFAULT:                             D E F A U L T;
DELAYED:                             D E L A Y E D;
DELETE:                              D E L E T E;
DELETE_DOMAIN_ID:                    D E L E T E '_' D O M A I N '_' I D;
DESC:                                D E S C;
DESCRIBE:                            D E S C R I B E;
DETERMINISTIC:                       D E T E R M I N I S T I C;
DISTINCT:                            D I S T I N C T;
DISTINCTROW:                         D I S T I N C T R O W;
DIV:                                 D I V;
DO_DOMAIN_IDS:                       D O '_' D O M A I N '_' I D S;
DOUBLE:                              D O U B L E;
DROP:                                D R O P;
DUAL:                                D U A L;

// E
EACH:                                E A C H;
ELSE:                                E L S E;
ELSEIF:                              E L S E I F;
ENCLOSED:                            E N C L O S E D;
ESCAPED:                             E S C A P E D;
EXCEPT:                              E X C E P T;
EXISTS:                              E X I S T S;
EXIT:                                E X I T;
EXPLAIN:                             E X P L A I N;

// F
FALSE:                               F A L S E;
FETCH:                               F E T C H;
FLOAT:                               F L O A T;
FLOAT4:                              F L O A T '4';
FLOAT8:                              F L O A T '8';
FOR:                                 F O R;
FORCE:                               F O R C E;
FOREIGN:                             F O R E I G N;
FROM:                                F R O M;
FULLTEXT:                            F U L L T E X T;

// G
GENERAL:                             G E N E R A L;
GRANT:                               G R A N T;
GROUP:                               G R O U P;

// H
HAVING:                              H A V I N G;
HIGH_PRIORITY:                       H I G H '_' P R I O R I T Y;
HOUR_MICROSECOND:                    H O U R '_' M I C R O S E C O N D;
HOUR_MINUTE:                         H O U R '_' M I N U T E;
HOUR_SECOND:                         H O U R '_' S E C O N D;

// I
IF:                                  I F;
IGNORE:                              I G N O R E;
IGNORE_DOMAIN_IDS:                   I G N O R E '_' D O M A I N '_' I D S;
IGNORE_SERVER_IDS:                   I G N O R E '_' S E R V E R '_' I D S;
IN:                                  I N;
INDEX:                               I N D E X;
INFILE:                              I N F I L E;
INNER:                               I N N E R;
INOUT:                               I N O U T;
INSENSITIVE:                         I N S E N S I T I V E;
INSERT:                              I N S E R T;
INT:                                 I N T;
INT1:                                I N T '1';
INT2:                                I N T '2';
INT3:                                I N T '3';
INT4:                                I N T '4';
INT8:                                I N T '8';
INTEGER:                             I N T E G E R;
INTERSECT:                           I N T E R S E C T;
INTERVAL:                            I N T E R V A L;
INTO:                                I N T O;
IS:                                  I S;
ITERATE:                             I T E R A T E;

// J
JOIN:                                J O I N;

// K
KEY:                                 K E Y;
KEYS:                                K E Y S;
KILL:                                K I L L;

// L
LEADING:                             L E A D I N G;
LEAVE:                               L E A V E;
LEFT:                                L E F T;
LIKE:                                L I K E;
LIMIT:                               L I M I T;
LINEAR:                              L I N E A R;
LINES:                               L I N E S;
LOAD:                                L O A D;
LOCALTIME:                           L O C A L T I M E;
LOCALTIMESTAMP:                      L O C A L T I M E S T A M P;
LOCK:                                L O C K;
LONG:                                L O N G;
LONGBLOB:                            L O N G B L O B;
LONGTEXT:                            L O N G T E X T;
LOOP:                                L O O P;
LOW_PRIORITY:                        L O W '_' P R I O R I T Y;

// M
MASTER_HEARTBEAT_PERIOD:             M A S T E R '_' H E A R T B E A T '_' P E R I O D;
MASTER_SSL_VERIFY_SERVER_CERT:       M A S T E R '_' S S L '_' V E R I F Y '_' S E R V E R '_' C E R T;
MATCH:                               M A T C H;
MAXVALUE:                            M A X V A L U E;
MEDIUMBLOB:                          M E D I U M B L O B;
MEDIUMINT:                           M E D I U M I N T;
MEDIUMTEXT:                          M E D I U M T E X T;
MIDDLEINT:                           M I D D L E I N T;
MINUTE_MICROSECOND:                  M I N U T E '_' M I C R O S E C O N D;
MINUTE_SECOND:                       M I N U T E '_' S E C O N D;
MOD:                                 M O D;
MODIFIES:                            M O D I F I E S;

// N
NATURAL:                             N A T U R A L;
NOT:                                 N O T;
NO_WRITE_TO_BINLOG:                  N O '_' W R I T E '_' T O '_' B I N L O G;
NULL_LITERAL:                        N U L L;
NUMERIC:                             N U M E R I C;

// O
OFFSET:                              O F F S E T;
ON:                                  O N;
OPTIMIZE:                            O P T I M I Z E;
OPTION:                              O P T I O N;
OPTIONALLY:                          O P T I O N A L L Y;
OR:                                  O R;
ORDER:                               O R D E R;
OUT:                                 O U T;
OUTER:                               O U T E R;
OUTFILE:                             O U T F I L E;
OVER:                                O V E R;

// P
PAGE_CHECKSUM:                       P A G E '_' C H E C K S U M;
PARSE_VCOL_EXPR:                     P A R S E '_' V C O L '_' E X P R;
PARTITION:                           P A R T I T I O N;
PRECISION:                           P R E C I S I O N;
PRIMARY:                             P R I M A R Y;
PROCEDURE:                           P R O C E D U R E;
PURGE:                               P U R G E;

// R
RANGE:                               R A N G E;
READ:                                R E A D;
READS:                               R E A D S;
READ_WRITE:                          R E A D '_' W R I T E;
REAL:                                R E A L;
RECURSIVE:                           R E C U R S I V E;
REF_SYSTEM_ID:                       R E F '_' S Y S T E M '_' I D;
REFERENCES:                          R E F E R E N C E S;
REGEXP:                              R E G E X P;
RELEASE:                             R E L E A S E;
RENAME:                              R E N A M E;
REPEAT:                              R E P E A T;
REPLACE:                             R E P L A C E;
REQUIRE:                             R E Q U I R E;
RESIGNAL:                            R E S I G N A L;
RESTRICT:                            R E S T R I C T;
RETURN:                              R E T U R N;
RETURNING:                           R E T U R N I N G;
REVOKE:                              R E V O K E;
RIGHT:                               R I G H T;
RLIKE:                               R L I K E;
ROW_NUMBER:                          R O W '_' N U M B E R;
ROWS:                                R O W S;

// S
SCHEMA:                              S C H E M A;
SCHEMAS:                             S C H E M A S;
SECOND_MICROSECOND:                  S E C O N D '_' M I C R O S E C O N D;
SELECT:                              S E L E C T;
SENSITIVE:                           S E N S I T I V E;
SEPARATOR:                           S E P A R A T O R;
SET:                                 S E T;
SHOW:                                S H O W;
SIGNAL:                              S I G N A L;
SLOW:                                S L O W;
SMALLINT:                            S M A L L I N T;
SPATIAL:                             S P A T I A L;
SPECIFIC:                            S P E C I F I C;
SQL:                                 S Q L;
SQLEXCEPTION:                        S Q L E X C E P T I O N;
SQLSTATE:                            S Q L S T A T E;
SQLWARNING:                          S Q L W A R N I N G;
SQL_BIG_RESULT:                      S Q L '_' B I G '_' R E S U L T;
SQL_CALC_FOUND_ROWS:                 S Q L '_' C A L C '_' F O U N D '_' R O W S;
SQL_SMALL_RESULT:                    S Q L '_' S M A L L '_' R E S U L T;
SSL:                                 S S L;
STARTING:                            S T A R T I N G;
STATS_AUTO_RECALC:                   S T A T S '_' A U T O '_' R E C A L C;
STATS_PERSISTENT:                    S T A T S '_' P E R S I S T E N T;
STATS_SAMPLE_PAGES:                  S T A T S '_' S A M P L E '_' P A G E S;
STRAIGHT_JOIN:                       S T R A I G H T '_' J O I N;

// T
TABLE:                               T A B L E;
TERMINATED:                          T E R M I N A T E D;
THEN:                                T H E N;
TINYBLOB:                            T I N Y B L O B;
TINYINT:                             T I N Y I N T;
TINYTEXT:                            T I N Y T E X T;
TO:                                  T O;
TRAILING:                            T R A I L I N G;
TRIGGER:                             T R I G G E R;
TRUE:                                T R U E;

// U
UNDO:                                U N D O;
UNION:                               U N I O N;
UNIQUE:                              U N I Q U E;
UNLOCK:                              U N L O C K;
UNSIGNED:                            U N S I G N E D;
UPDATE:                              U P D A T E;
USAGE:                               U S A G E;
USE:                                 U S E;
USING:                               U S I N G;
UTC_DATE:                            U T C '_' D A T E;
UTC_TIME:                            U T C '_' T I M E;
UTC_TIMESTAMP:                       U T C '_' T I M E S T A M P;

// V
VALUES:                              V A L U E S;
VARBINARY:                           V A R B I N A R Y;
VARCHAR:                             V A R C H A R;
VARCHARACTER:                        V A R C H A R A C T E R;
VARYING:                             V A R Y I N G;
VECTOR:                              V E C T O R;

// W
WHEN:                                W H E N;
WHERE:                               W H E R E;
WHILE:                               W H I L E;
WINDOW:                              W I N D O W;
WITH:                                W I T H;
WRITE:                               W R I T E;

// X
XOR:                                 X O R;

// Y
YEAR_MONTH:                          Y E A R '_' M O N T H;

// Z
ZEROFILL:                            Z E R O F I L L;

// ===========================================================================
// NON-RESERVED KEYWORDS (Common in MariaDB)
// ===========================================================================

// A
ACCOUNT:                             A C C O U N T;
ACTION:                              A C T I O N;
ADMIN:                               A D M I N;
AFTER:                               A F T E R;
AGAINST:                             A G A I N S T;
AGGREGATE:                           A G G R E G A T E;
ALGORITHM:                           A L G O R I T H M;
ALWAYS:                              A L W A Y S;
ANY:                                 A N Y;
ASCII_SYM:                           A S C I I;
AT:                                  A T;
ATOMIC:                              A T O M I C;
AUTHORS:                             A U T H O R S;
AUTOEXTEND_SIZE:                     A U T O E X T E N D '_' S I Z E;
AUTO_INCREMENT:                      A U T O '_' I N C R E M E N T;
AVG:                                 A V G;
AVG_ROW_LENGTH:                      A V G '_' R O W '_' L E N G T H;

// B
BACKUP:                              B A C K U P;
BEGIN:                               B E G I N;
BINLOG:                              B I N L O G;
BIT:                                 B I T;
BLOCK:                               B L O C K;
BOOL:                                B O O L;
BOOLEAN:                             B O O L E A N;
BTREE:                               B T R E E;
BYTE:                                B Y T E;

// C
CACHE:                               C A C H E;
CASCADED:                            C A S C A D E D;
CATALOG_NAME:                        C A T A L O G '_' N A M E;
CHAIN:                               C H A I N;
CHANGED:                             C H A N G E D;
CHANNEL:                             C H A N N E L;
CHARSET:                             C H A R S E T;
CHECKSUM:                            C H E C K S U M;
CIPHER:                              C I P H E R;
CLASS_ORIGIN:                        C L A S S '_' O R I G I N;
CLIENT:                              C L I E N T;
CLOSE:                               C L O S E;
COALESCE:                            C O A L E S C E;
CODE:                                C O D E;
COLLATION:                           C O L L A T I O N;
COLUMN_FORMAT:                       C O L U M N '_' F O R M A T;
COLUMN_NAME:                         C O L U M N '_' N A M E;
COLUMNS:                             C O L U M N S;
COMMENT:                             C O M M E N T;
COMMIT:                              C O M M I T;
COMMITTED:                           C O M M I T T E D;
COMPACT:                             C O M P A C T;
COMPLETION:                          C O M P L E T I O N;
COMPRESSED:                          C O M P R E S S E D;
COMPRESSION:                         C O M P R E S S I O N;
CONCURRENT:                          C O N C U R R E N T;
CONNECTION:                          C O N N E C T I O N;
CONSISTENT:                          C O N S I S T E N T;
CONSTRAINT_CATALOG:                  C O N S T R A I N T '_' C A T A L O G;
CONSTRAINT_NAME:                     C O N S T R A I N T '_' N A M E;
CONSTRAINT_SCHEMA:                   C O N S T R A I N T '_' S C H E M A;
CONTAINS:                            C O N T A I N S;
CONTEXT:                             C O N T E X T;
CONTRIBUTORS:                        C O N T R I B U T O R S;
COPY:                                C O P Y;
CPU:                                 C P U;
CURRENT:                             C U R R E N T;
CURSOR_NAME:                         C U R S O R '_' N A M E;
CYCLE:                               C Y C L E;

// D
DATA:                                D A T A;
DATAFILE:                            D A T A F I L E;
DATE:                                D A T E;
DATETIME:                            D A T E T I M E;
DAY:                                 D A Y;
DEALLOCATE:                          D E A L L O C A T E;
DEFINER:                             D E F I N E R;
DELAY_KEY_WRITE:                     D E L A Y '_' K E Y '_' W R I T E;
DES_KEY_FILE:                        D E S '_' K E Y '_' F I L E;
DIAGNOSTICS:                         D I A G N O S T I C S;
DIRECTORY:                           D I R E C T O R Y;
DISABLE:                             D I S A B L E;
DISCARD:                             D I S C A R D;
DISK:                                D I S K;
DO:                                  D O;
DUMPFILE:                            D U M P F I L E;
DUPLICATE:                           D U P L I C A T E;
DYNAMIC:                             D Y N A M I C;

// E
ENABLE:                              E N A B L E;
ENCRYPTION:                          E N C R Y P T I O N;
END:                                 E N D;
ENDS:                                E N D S;
ENGINE:                              E N G I N E;
ENGINES:                             E N G I N E S;
ENUM:                                E N U M;
ERROR:                               E R R O R;
ERRORS:                              E R R O R S;
ESCAPE:                              E S C A P E;
EVENT:                               E V E N T;
EVENTS:                              E V E N T S;
EVERY:                               E V E R Y;
EXCHANGE:                            E X C H A N G E;
EXCLUSIVE:                           E X C L U S I V E;
EXECUTE:                             E X E C U T E;
EXPANSION:                           E X P A N S I O N;
EXPIRE:                              E X P I R E;
EXPORT:                              E X P O R T;
EXTENDED:                            E X T E N D E D;
EXTENT_SIZE:                         E X T E N T '_' S I Z E;

// F
FAST:                                F A S T;
FAULTS:                              F A U L T S;
FIELDS:                              F I E L D S;
FILE_BLOCK_SIZE:                     F I L E '_' B L O C K '_' S I Z E;
FILTER:                              F I L T E R;
FIRST:                               F I R S T;
FIXED:                               F I X E D;
FLUSH:                               F L U S H;
FOLLOWING:                           F O L L O W I N G;
FOLLOWS:                             F O L L O W S;
FORMAT:                              F O R M A T;
FOUND:                               F O U N D;
FULL:                                F U L L;
FUNCTION:                            F U N C T I O N;

// G
GENERATED:                           G E N E R A T E D;
GET_FORMAT:                          G E T '_' F O R M A T;
GLOBAL:                              G L O B A L;
GRANTS:                              G R A N T S;
GROUP_REPLICATION:                   G R O U P '_' R E P L I C A T I O N;

// H
HANDLER:                             H A N D L E R;
HARD:                                H A R D;
HASH:                                H A S H;
HELP:                                H E L P;
HOST:                                H O S T;
HOSTS:                               H O S T S;
HOUR:                                H O U R;

// I
IDENTIFIED:                          I D E N T I F I E D;
IMMEDIATE:                           I M M E D I A T E;
IMPORT:                              I M P O R T;
INCREMENT:                           I N C R E M E N T;
INDEXES:                             I N D E X E S;
INITIAL_SIZE:                        I N I T I A L '_' S I Z E;
INPLACE:                             I N P L A C E;
INSERT_METHOD:                       I N S E R T '_' M E T H O D;
INSTALL:                             I N S T A L L;
INSTANCE:                            I N S T A N C E;
INSTANT:                             I N S T A N T;
INVISIBLE:                           I N V I S I B L E;
INVOKER:                             I N V O K E R;
IO:                                  I O;
IO_THREAD:                           I O '_' T H R E A D;
IPC:                                 I P C;
ISOLATION:                           I S O L A T I O N;
ISSUER:                              I S S U E R;

// J
JSON:                                J S O N;

// K
KEY_BLOCK_SIZE:                      K E Y '_' B L O C K '_' S I Z E;

// L
LANGUAGE:                            L A N G U A G E;
LAST:                                L A S T;
LATERAL:                             L A T E R A L;
LEAVES:                              L E A V E S;
LESS:                                L E S S;
LEVEL:                               L E V E L;
LIST:                                L I S T;
LOCAL:                               L O C A L;
LOCKS:                               L O C K S;
LOGFILE:                             L O G F I L E;
LOGS:                                L O G S;

// M
MASTER:                              M A S T E R;
MASTER_AUTO_POSITION:                M A S T E R '_' A U T O '_' P O S I T I O N;
MASTER_BIND:                         M A S T E R '_' B I N D;
MASTER_CONNECT_RETRY:                M A S T E R '_' C O N N E C T '_' R E T R Y;
MASTER_DELAY:                        M A S T E R '_' D E L A Y;
MASTER_HOST:                         M A S T E R '_' H O S T;
MASTER_LOG_FILE:                     M A S T E R '_' L O G '_' F I L E;
MASTER_LOG_POS:                      M A S T E R '_' L O G '_' P O S;
MASTER_PASSWORD:                     M A S T E R '_' P A S S W O R D;
MASTER_PORT:                         M A S T E R '_' P O R T;
MASTER_RETRY_COUNT:                  M A S T E R '_' R E T R Y '_' C O U N T;
MASTER_SERVER_ID:                    M A S T E R '_' S E R V E R '_' I D;
MASTER_SSL:                          M A S T E R '_' S S L;
MASTER_SSL_CA:                       M A S T E R '_' S S L '_' C A;
MASTER_SSL_CAPATH:                   M A S T E R '_' S S L '_' C A P A T H;
MASTER_SSL_CERT:                     M A S T E R '_' S S L '_' C E R T;
MASTER_SSL_CIPHER:                   M A S T E R '_' S S L '_' C I P H E R;
MASTER_SSL_CRL:                      M A S T E R '_' S S L '_' C R L;
MASTER_SSL_CRLPATH:                  M A S T E R '_' S S L '_' C R L P A T H;
MASTER_SSL_KEY:                      M A S T E R '_' S S L '_' K E Y;
MASTER_TLS_VERSION:                  M A S T E R '_' T L S '_' V E R S I O N;
MASTER_USER:                         M A S T E R '_' U S E R;
MASTER_USE_GTID:                     M A S T E R '_' U S E '_' G T I D;
MAX_CONNECTIONS_PER_HOUR:            M A X '_' C O N N E C T I O N S '_' P E R '_' H O U R;
MAX_QUERIES_PER_HOUR:                M A X '_' Q U E R I E S '_' P E R '_' H O U R;
MAX_ROWS:                            M A X '_' R O W S;
MAX_SIZE:                            M A X '_' S I Z E;
MAX_STATEMENT_TIME:                  M A X '_' S T A T E M E N T '_' T I M E;
MAX_UPDATES_PER_HOUR:                M A X '_' U P D A T E S '_' P E R '_' H O U R;
MAX_USER_CONNECTIONS:                M A X '_' U S E R '_' C O N N E C T I O N S;
MEDIUM:                              M E D I U M;
MEMORY:                              M E M O R Y;
MERGE:                               M E R G E;
MESSAGE_TEXT:                        M E S S A G E '_' T E X T;
MICROSECOND:                         M I C R O S E C O N D;
MIGRATE:                             M I G R A T E;
MIN_ROWS:                            M I N '_' R O W S;
MINUTE:                              M I N U T E;
MINVALUE:                            M I N V A L U E;
MODE:                                M O D E;
MODIFY:                              M O D I F Y;
MONTH:                               M O N T H;
MUTEX:                               M U T E X;
MYSQL_ERRNO:                         M Y S Q L '_' E R R N O;

// N
NAME:                                N A M E;
NAMES:                               N A M E S;
NATIONAL:                            N A T I O N A L;
NCHAR:                               N C H A R;
NDB:                                 N D B;
NDBCLUSTER:                          N D B C L U S T E R;
NEVER:                               N E V E R;
NEXT:                                N E X T;
NO:                                  N O;
NOCACHE:                             N O C A C H E;
NOCYCLE:                             N O C Y C L E;
NODEGROUP:                           N O D E G R O U P;
NOMAXVALUE:                          N O M A X V A L U E;
NOMINVALUE:                          N O M I N V A L U E;
NONE:                                N O N E;
NOWAIT:                              N O W A I T;
NUMBER:                              N U M B E R;
NVARCHAR:                            N V A R C H A R;

// O
OF:                                  O F;
OFFLINE:                             O F F L I N E;
OLD_PASSWORD:                        O L D '_' P A S S W O R D;
ONLINE:                              O N L I N E;
ONLY:                                O N L Y;
OPEN:                                O P E N;
OPTIMIZER_COSTS:                     O P T I M I Z E R '_' C O S T S;
OPTIONS:                             O P T I O N S;
OWNER:                               O W N E R;

// P
PACK_KEYS:                           P A C K '_' K E Y S;
PAGE:                                P A G E;
PARSER:                              P A R S E R;
PARTIAL:                             P A R T I A L;
PARTITIONING:                        P A R T I T I O N I N G;
PARTITIONS:                          P A R T I T I O N S;
PASSWORD:                            P A S S W O R D;
PERSISTENT:                          P E R S I S T E N T;
PHASE:                               P H A S E;
PLUGIN:                              P L U G I N;
PLUGINS:                             P L U G I N S;
PLUGIN_DIR:                          P L U G I N '_' D I R;
PORT:                                P O R T;
PRECEDES:                            P R E C E D E S;
PRECEDING:                           P R E C E D I N G;
PREPARE:                             P R E P A R E;
PRESERVE:                            P R E S E R V E;
PREV:                                P R E V;
PRIVILEGES:                          P R I V I L E G E S;
PROCESSLIST:                         P R O C E S S L I S T;
PROFILE:                             P R O F I L E;
PROFILES:                            P R O F I L E S;
PROXY:                               P R O X Y;

// Q
QUARTER:                             Q U A R T E R;
QUERY:                               Q U E R Y;
QUICK:                               Q U I C K;

// R
REBUILD:                             R E B U I L D;
RECOVER:                             R E C O V E R;
REDO_BUFFER_SIZE:                    R E D O '_' B U F F E R '_' S I Z E;
REDUNDANT:                           R E D U N D A N T;
RELAY:                               R E L A Y;
RELAY_LOG_FILE:                      R E L A Y '_' L O G '_' F I L E;
RELAY_LOG_POS:                       R E L A Y '_' L O G '_' P O S;
RELAYLOG:                            R E L A Y L O G;
RELAY_THREAD:                        R E L A Y '_' T H R E A D;
RELOAD:                              R E L O A D;
REMOVE:                              R E M O V E;
REORGANIZE:                          R E O R G A N I Z E;
REPAIR:                              R E P A I R;
REPEATABLE:                          R E P E A T A B L E;
REPLICATION:                         R E P L I C A T I O N;
REPLICAS:                            R E P L I C A S;
REPLICA:                             R E P L I C A;
RESET:                               R E S E T;
RESTART:                             R E S T A R T;
RESTORE:                             R E S T O R E;
RESUME:                              R E S U M E;
RETURNED_SQLSTATE:                   R E T U R N E D '_' S Q L S T A T E;
RETURNS:                             R E T U R N S;
ROLE:                                R O L E;
ROLLBACK:                            R O L L B A C K;
ROLLUP:                              R O L L U P;
ROTATE:                              R O T A T E;
ROUTINE:                             R O U T I N E;
ROW:                                 R O W;
ROW_COUNT:                           R O W '_' C O U N T;
ROW_FORMAT:                          R O W '_' F O R M A T;
RTREE:                               R T R E E;

// S
SAVEPOINT:                           S A V E P O I N T;
SCHEDULE:                            S C H E D U L E;
SCHEMA_NAME:                         S C H E M A '_' N A M E;
SECOND:                              S E C O N D;
SECURITY:                            S E C U R I T Y;
SEQUENCE:                            S E Q U E N C E;
SERIAL:                              S E R I A L;
SERIALIZABLE:                        S E R I A L I Z A B L E;
SERVER:                              S E R V E R;
SESSION:                             S E S S I O N;
SHARE:                               S H A R E;
SHARED:                              S H A R E D;
SHUTDOWN:                            S H U T D O W N;
SIGNED:                              S I G N E D;
SIMPLE:                              S I M P L E;
SKIP_:                               S K I P;
SLAVE:                               S L A V E;
SLAVES:                              S L A V E S;
SNAPSHOT:                            S N A P S H O T;
SOCKET:                              S O C K E T;
SOFT:                                S O F T;
SOME:                                S O M E;
SONAME:                              S O N A M E;
SOUNDS:                              S O U N D S;
SOURCE:                              S O U R C E;
SQL_AFTER_GTIDS:                     S Q L '_' A F T E R '_' G T I D S;
SQL_AFTER_MTS_GAPS:                  S Q L '_' A F T E R '_' M T S '_' G A P S;
SQL_BEFORE_GTIDS:                    S Q L '_' B E F O R E '_' G T I D S;
SQL_BUFFER_RESULT:                   S Q L '_' B U F F E R '_' R E S U L T;
SQL_CACHE:                           S Q L '_' C A C H E;
SQL_NO_CACHE:                        S Q L '_' N O '_' C A C H E;
SQL_THREAD:                          S Q L '_' T H R E A D;
STACKED:                             S T A C K E D;
START:                               S T A R T;
STARTS:                              S T A R T S;
STATS_AUTO_RECALC_SYM:               S T A T S '_' A U T O '_' R E C A L C;
STATS_PERSISTENT_SYM:                S T A T S '_' P E R S I S T E N T;
STATS_SAMPLE_PAGES_SYM:              S T A T S '_' S A M P L E '_' P A G E S;
STATUS:                              S T A T U S;
STOP:                                S T O P;
STORAGE:                             S T O R A G E;
STORED:                              S T O R E D;
STRING:                              S T R I N G;
SUBJECT:                             S U B J E C T;
SUBCLASS_ORIGIN:                     S U B C L A S S '_' O R I G I N;
SUBPARTITION:                        S U B P A R T I T I O N;
SUBPARTITIONS:                       S U B P A R T I T I O N S;
SUPER:                               S U P E R;
SUSPEND:                             S U S P E N D;
SWAPS:                               S W A P S;
SWITCHES:                            S W I T C H E S;

// T
TABLES:                              T A B L E S;
TABLESPACE:                          T A B L E S P A C E;
TABLE_CHECKSUM:                      T A B L E '_' C H E C K S U M;
TABLE_NAME:                          T A B L E '_' N A M E;
TEMPORARY:                           T E M P O R A R Y;
TEMPTABLE:                           T E M P T A B L E;
TEXT:                                T E X T;
THAN:                                T H A N;
TIME:                                T I M E;
TIMESTAMP:                           T I M E S T A M P;
TIMESTAMPADD:                        T I M E S T A M P A D D;
TIMESTAMPDIFF:                       T I M E S T A M P D I F F;
TRANSACTION:                         T R A N S A C T I O N;
TRANSACTIONAL:                       T R A N S A C T I O N A L;
TRIGGERS:                            T R I G G E R S;
TRUNCATE:                            T R U N C A T E;
TYPE:                                T Y P E;
TYPES:                               T Y P E S;

// U
UNBOUNDED:                           U N B O U N D E D;
UNCOMMITTED:                         U N C O M M I T T E D;
UNDEFINED:                           U N D E F I N E D;
UNDO_BUFFER_SIZE:                    U N D O '_' B U F F E R '_' S I Z E;
UNDOFILE:                            U N D O F I L E;
UNICODE:                             U N I C O D E;
UNINSTALL:                           U N I N S T A L L;
UNKNOWN:                             U N K N O W N;
UNTIL:                               U N T I L;
UPGRADE:                             U P G R A D E;
USER:                                U S E R;
USER_RESOURCES:                      U S E R '_' R E S O U R C E S;
USE_FRM:                             U S E '_' F R M;

// V
VALIDATION:                          V A L I D A T I O N;
VALUE:                               V A L U E;
VARIABLES:                           V A R I A B L E S;
VIEW:                                V I E W;
VIRTUAL:                             V I R T U A L;
VISIBLE:                             V I S I B L E;
WAIT:                                W A I T;

// W
WARNINGS:                            W A R N I N G S;
WEEK:                                W E E K;
WEIGHT_STRING:                       W E I G H T '_' S T R I N G;
WITHOUT:                             W I T H O U T;
WORK:                                W O R K;
WRAPPER:                             W R A P P E R;

// X
X509:                                X '5' '0' '9';
XA:                                  X A;
XML:                                 X M L;

// Y
YEAR:                                Y E A R;

// ===========================================================================
// ORACLE MODE KEYWORDS (MariaDB 10.3+)
// ===========================================================================

BODY:                                B O D Y;
ELSIF:                               E L S I F;
GOTO:                                G O T O;
HISTORY:                             H I S T O R Y;
MINUS:                               M I N U S;
OTHERS:                              O T H E R S;
PACKAGE:                             P A C K A G E;
PERIOD:                              P E R I O D;
RAISE:                               R A I S E;
ROWNUM:                              R O W N U M;
ROWTYPE:                             R O W T Y P E;
SYSDATE:                             S Y S D A T E;
SYSTEM:                              S Y S T E M;
SYSTEM_TIME:                         S Y S T E M '_' T I M E;
VERSIONING:                          V E R S I O N I N G;

// ===========================================================================
// SPATIAL KEYWORDS
// ===========================================================================

GEOMETRY:                            G E O M E T R Y;
GEOMETRYCOLLECTION:                  G E O M E T R Y C O L L E C T I O N;
LINESTRING:                          L I N E S T R I N G;
MULTILINESTRING:                     M U L T I L I N E S T R I N G;
MULTIPOINT:                          M U L T I P O I N T;
MULTIPOLYGON:                        M U L T I P O L Y G O N;
POINT:                               P O I N T;
POLYGON:                             P O L Y G O N;

// ===========================================================================
// ADDITIONAL DATA TYPES
// ===========================================================================

INET4:                               I N E T '4';
INET6:                               I N E T '6';
UUID:                                U U I D;
CLOB:                                C L O B;
RAW:                                 R A W;

// ===========================================================================
// WINDOW FUNCTION KEYWORDS
// ===========================================================================

CUME_DIST:                           C U M E '_' D I S T;
DENSE_RANK:                          D E N S E '_' R A N K;
FIRST_VALUE:                         F I R S T '_' V A L U E;
LAG:                                 L A G;
LAST_VALUE:                          L A S T '_' V A L U E;
LEAD:                                L E A D;
NTH_VALUE:                           N T H '_' V A L U E;
NTILE:                               N T I L E;
PERCENT_RANK:                        P E R C E N T '_' R A N K;
RANK:                                R A N K;

// ===========================================================================
// COMMON TABLE EXPRESSION KEYWORDS
// ===========================================================================

MATERIALIZED:                        M A T E R I A L I Z E D;

// ===========================================================================
// JSON KEYWORDS
// ===========================================================================

JSON_TABLE:                          J S O N '_' T A B L E;
NESTED:                              N E S T E D;
ORDINALITY:                          O R D I N A L I T Y;
PATH:                                P A T H;

// ===========================================================================
// OPERATORS AND SYMBOLS
// ===========================================================================

// Comparison operators
EQUAL_SYMBOL:                        '=';
GREATER_SYMBOL:                      '>';
LESS_SYMBOL:                         '<';
EXCLAMATION_SYMBOL:                  '!';

// Logical operators
BIT_NOT_OP:                          '~';
BIT_OR_OP:                           '|';
BIT_AND_OP:                          '&';
BIT_XOR_OP:                          '^';

// Arithmetic operators
PLUS:                                '+';
MINUS_OP:                            '-';
STAR:                                '*';
DIVIDE:                              '/';
MOD_OP:                              '%';

// Compound operators
NOT_EQUAL:                           '!=' | '<>';
GREATER_OR_EQUAL:                    '>=';
LESS_OR_EQUAL:                       '<=';
NULL_SAFE_EQUAL:                     '<=>';
LEFT_SHIFT:                          '<<';
RIGHT_SHIFT:                         '>>';
AND_OP:                              '&&';
OR_OP:                               '||';
COLON_EQ:                            ':=';
PLUS_ASSIGN:                         '+=';
MINUS_ASSIGN:                        '-=';
MULT_ASSIGN:                         '*=';
DIV_ASSIGN:                          '/=';
MOD_ASSIGN:                          '%=';
AND_ASSIGN:                          '&=';
XOR_ASSIGN:                          '^=';
OR_ASSIGN:                           '|=';

// Other symbols
DOT:                                 '.';
LPAREN:                              '(';
RPAREN:                              ')';
COMMA:                               ',';
SEMICOLON:                           ';';
AT_SIGN:                             '@';
ZERO_DECIMAL:                        '0';
ONE_DECIMAL:                         '1';
TWO_DECIMAL:                         '2';
COLON:                               ':';
LBRACE:                              '{';
RBRACE:                              '}';
LBRACKET:                            '[';
RBRACKET:                            ']';
PARAM_MARKER:                        '?';
DOUBLE_AT_SIGN:                      '@@';
ARROW:                               '->';
DOUBLE_ARROW:                        '->>';

// ===========================================================================
// LITERALS
// ===========================================================================

// String literals
SINGLE_QUOTE_SYMB:                   '\'';
DOUBLE_QUOTE_SYMB:                   '"';
REVERSE_QUOTE_SYMB:                  '`';

// Charset introducer
CHARSET_REVERSE_QOUTE_STRING:        '_' BQUOTA_STRING;

// File size literals  
FILESIZE_LITERAL:                    DEC_DIGIT+ [KMGkmg];

// Numeric literals
DECIMAL_LITERAL:                     DEC_DIGIT+;
HEXADECIMAL_LITERAL:                 'x' '\'' HEX_DIGIT* '\'' 
                                     | '0' X HEX_DIGIT+
                                     | X '\'' HEX_DIGIT+ '\'';
BINARY_LITERAL:                      'b' '\'' BIT_DIGIT* '\''
                                     | '0' B BIT_DIGIT+
                                     | B '\'' BIT_DIGIT+ '\'';
REAL_LITERAL:                        DEC_DIGIT* '.' DEC_DIGIT+ EXPONENT_NUM_PART?
                                     | DEC_DIGIT+ '.' EXPONENT_NUM_PART?
                                     | DEC_DIGIT+ EXPONENT_NUM_PART;

// String literals
STRING_LITERAL:                      SQUOTA_STRING
                                     | DQUOTA_STRING
                                     | (STRING_CHARSET_NAME? SQUOTA_STRING)+
                                     | (STRING_CHARSET_NAME? DQUOTA_STRING)+;

fragment STRING_CHARSET_NAME:        '_' [A-Za-z0-9]+;

// Bit strings
BIT_STRING:                          BIT_DIGIT+;

// ===========================================================================
// IDENTIFIERS
// ===========================================================================

// Regular identifier
ID:                                  ID_LITERAL;

// Quoted identifier
DOUBLE_QUOTE_ID:                     DQUOTA_STRING;
REVERSE_QUOTE_ID:                    BQUOTA_STRING;

// Character set names for literal string
CHARSET_NAME:                        '_' ID_LITERAL;

// Session/Global variables
LOCAL_ID:                            '@' ID_LITERAL;
GLOBAL_ID:                           '@' '@' [A-Za-z_$] [A-Za-z_$0-9.]*;

// ===========================================================================
// ERROR HANDLING
// ===========================================================================

ERROR_RECONGNIGION:                  . -> channel(ERRORCHANNEL);
