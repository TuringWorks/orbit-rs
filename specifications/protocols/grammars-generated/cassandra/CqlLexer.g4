/*
 * CQL Lexer Grammar for Apache Cassandra 5.0 and ScyllaDB
 * 
 * This lexer grammar defines all tokens for the Cassandra Query Language (CQL).
 * It supports:
 * - Apache Cassandra 5.0 features (vector types, SAI, dynamic data masking)
 * - ScyllaDB extensions (BYPASS CACHE, USING TIMEOUT, etc.)
 * 
 * Licensed under Apache License 2.0
 * 
 * Based on CQL specification from Apache Cassandra documentation
 * and ScyllaDB CQL extensions documentation.
 */

lexer grammar CqlLexer;

// =============================================================================
// Keywords - DDL (Data Definition Language)
// =============================================================================

K_ADD:              A D D;
K_AGGREGATE:        A G G R E G A T E;
K_ALL:              A L L;
K_ALLOW:            A L L O W;
K_ALTER:            A L T E R;
K_AND:              A N D;
K_ANY:              A N Y;
K_APPLY:            A P P L Y;
K_AS:               A S;
K_ASC:              A S C;
K_AUTHORIZE:        A U T H O R I Z E;
K_BATCH:            B A T C H;
K_BEGIN:            B E G I N;
K_BY:               B Y;
K_CALLED:           C A L L E D;
K_CAST:             C A S T;
K_CLUSTERING:       C L U S T E R I N G;
K_COLUMNFAMILY:     C O L U M N F A M I L Y;
K_COMPACT:          C O M P A C T;
K_CONTAINS:         C O N T A I N S;
K_CREATE:           C R E A T E;
K_CUSTOM:           C U S T O M;
K_DEFAULT:          D E F A U L T;
K_DELETE:           D E L E T E;
K_DESC:             D E S C;
K_DESCRIBE:         D E S C R I B E;
K_DISTINCT:         D I S T I N C T;
K_DROP:             D R O P;
K_DURABLE_WRITES:   D U R A B L E '_' W R I T E S;
K_ENTRIES:          E N T R I E S;
K_EXECUTE:          E X E C U T E;
K_EXISTS:           E X I S T S;
K_FILTERING:        F I L T E R I N G;
K_FINALFUNC:        F I N A L F U N C;
K_FROM:             F R O M;
K_FROZEN:           F R O Z E N;
K_FULL:             F U L L;
K_FUNCTION:         F U N C T I O N;
K_FUNCTIONS:        F U N C T I O N S;
K_GRANT:            G R A N T;
K_GROUP:            G R O U P;
K_IF:               I F;
K_IN:               I N;
K_INDEX:            I N D E X;
K_INFINITY:         I N F I N I T Y;
K_INITCOND:         I N I T C O N D;
K_INPUT:            I N P U T;
K_INSERT:           I N S E R T;
K_INTO:             I N T O;
K_IS:               I S;
K_JSON:             J S O N;
K_KEY:              K E Y;
K_KEYS:             K E Y S;
K_KEYSPACE:         K E Y S P A C E;
K_KEYSPACES:        K E Y S P A C E S;
K_LANGUAGE:         L A N G U A G E;
K_LIKE:             L I K E;
K_LIMIT:            L I M I T;
K_LIST:             L I S T;
K_LOGIN:            L O G I N;
K_MAP:              M A P;
K_MASKED:           M A S K E D;  // Cassandra 5.0
K_MATERIALIZED:     M A T E R I A L I Z E D;
K_MBEAN:            M B E A N;
K_MBEANS:           M B E A N S;
K_MODIFY:           M O D I F Y;
K_NAN:              N A N;
K_NEGATIVE_INFINITY: '-' I N F I N I T Y;
K_NEGATIVE_NAN:     '-' N A N;
K_NOLOGIN:          N O L O G I N;
K_NORECURSIVE:      N O R E C U R S I V E;
K_NOSUPERUSER:      N O S U P E R U S E R;
K_NOT:              N O T;
K_NULL:             N U L L;
K_OF:               O F;
K_ON:               O N;
K_OPTIONS:          O P T I O N S;
K_OR:               O R;
K_ORDER:            O R D E R;
K_PARTITION:        P A R T I T I O N;
K_PASSWORD:         P A S S W O R D;
K_PER:              P E R;
K_PERMISSION:       P E R M I S S I O N;
K_PERMISSIONS:      P E R M I S S I O N S;
K_PRIMARY:          P R I M A R Y;
K_RENAME:           R E N A M E;
K_REPLACE:          R E P L A C E;
K_REPLICATION:      R E P L I C A T I O N;
K_RESTRICT:         R E S T R I C T;
K_RETURNS:          R E T U R N S;
K_REVOKE:           R E V O K E;
K_ROLE:             R O L E;
K_ROLES:            R O L E S;
K_SELECT:           S E L E C T;
K_SET:              S E T;
K_SFUNC:            S F U N C;
K_STATIC:           S T A T I C;
K_STORAGE:          S T O R A G E;
K_STYPE:            S T Y P E;
K_SUPERUSER:        S U P E R U S E R;
K_TABLE:            T A B L E;
K_TABLES:           T A B L E S;
K_TEXT:             T E X T;
K_TIMESTAMP:        T I M E S T A M P;
K_TO:               T O;
K_TOKEN:            T O K E N;
K_TRIGGER:          T R I G G E R;
K_TRUNCATE:         T R U N C A T E;
K_TTL:              T T L;
K_TUPLE:            T U P L E;
K_TYPE:             T Y P E;
K_UNLOGGED:         U N L O G G E D;
K_UNMASK:           U N M A S K;  // Cassandra 5.0
K_UNSET:            U N S E T;
K_UPDATE:           U P D A T E;
K_USE:              U S E;
K_USER:             U S E R;
K_USERS:            U S E R S;
K_USING:            U S I N G;
K_VALUES:           V A L U E S;
K_VIEW:             V I E W;
K_WHERE:            W H E R E;
K_WITH:             W I T H;
K_WRITETIME:        W R I T E T I M E;

// =============================================================================
// Keywords - Data Types
// =============================================================================

K_ASCII:            A S C I I;
K_BIGINT:           B I G I N T;
K_BLOB:             B L O B;
K_BOOLEAN:          B O O L E A N;
K_COUNTER:          C O U N T E R;
K_DATE:             D A T E;
K_DECIMAL:          D E C I M A L;
K_DOUBLE:           D O U B L E;
K_DURATION:         D U R A T I O N;
K_FLOAT:            F L O A T;
K_INET:             I N E T;
K_INT:              I N T;
K_SMALLINT:         S M A L L I N T;
K_TIME:             T I M E;
K_TIMEUUID:         T I M E U U I D;
K_TINYINT:          T I N Y I N T;
K_UUID:             U U I D;
K_VARCHAR:          V A R C H A R;
K_VARINT:           V A R I N T;
K_VECTOR:           V E C T O R;  // Cassandra 5.0 Vector Search

// =============================================================================
// Keywords - Boolean
// =============================================================================

K_TRUE:             T R U E;
K_FALSE:            F A L S E;

// =============================================================================
// Cassandra 5.0 Keywords - Vector Search / SAI
// =============================================================================

K_ANN:              A N N;           // Approximate Nearest Neighbor
K_SIMILARITY:       S I M I L A R I T Y;
K_COSINE:           C O S I N E;
K_DOT_PRODUCT:      D O T '_' P R O D U C T;
K_EUCLIDEAN:        E U C L I D E A N;

// =============================================================================
// Cassandra 5.0 Keywords - Dynamic Data Masking
// =============================================================================

K_MASK:             M A S K;
K_UNMASK:           U N M A S K;
K_SELECT_MASKED:    S E L E C T '_' M A S K E D;

// =============================================================================
// Cassandra 5.0 Keywords - Math Functions
// =============================================================================

K_ABS:              A B S;
K_EXP:              E X P;
K_LOG:              L O G;
K_LOG10:            L O G '1' '0';
K_ROUND:            R O U N D;

// =============================================================================
// ScyllaDB Extension Keywords
// =============================================================================

K_BYPASS:           B Y P A S S;     // BYPASS CACHE
K_CACHE:            C A C H E;
K_TIMEOUT:          T I M E O U T;   // USING TIMEOUT
K_PRUNE:            P R U N E;       // PRUNE MATERIALIZED VIEW
K_SYNCHRONOUS_UPDATES: S Y N C H R O N O U S '_' U P D A T E S;
K_REDUCEFUNC:       R E D U C E F U N C;  // UDA extension
K_INTERNALS:        I N T E R N A L S;    // DESCRIBE WITH INTERNALS
K_PASSWORDS:        P A S S W O R D S;    // AND PASSWORDS
K_PAXOS_GRACE_SECONDS: P A X O S '_' G R A C E '_' S E C O N D S;
K_PER_PARTITION_RATE_LIMIT: P E R '_' P A R T I T I O N '_' R A T E '_' L I M I T;
K_MAX_READS_PER_SECOND: M A X '_' R E A D S '_' P E R '_' S E C O N D;
K_MAX_WRITES_PER_SECOND: M A X '_' W R I T E S '_' P E R '_' S E C O N D;
K_SERVICE:          S E R V I C E;   // Service Levels
K_LEVEL:            L E V E L;
K_EFFECTIVE:        E F F E C T I V E;
K_WORKLOAD_TYPE:    W O R K L O A D '_' T Y P E;

// ScyllaDB Internal Functions (for sstableloader)
K_SCYLLA_TIMEUUID_LIST_INDEX: S C Y L L A '_' T I M E U U I D '_' L I S T '_' I N D E X;
K_SCYLLA_COUNTER_SHARD_LIST: S C Y L L A '_' C O U N T E R '_' S H A R D '_' L I S T;
K_SCYLLA_CLUSTERING_BOUND: S C Y L L A '_' C L U S T E R I N G '_' B O U N D;

// =============================================================================
// Operators and Symbols
// =============================================================================

// Comparison Operators
LT:                 '<';
LE:                 '<=';
GT:                 '>';
GE:                 '>=';
EQ:                 '=';
NE:                 '!=' | '<>';

// Arithmetic Operators
PLUS:               '+';
MINUS:              '-';
STAR:               '*';
SLASH:              '/';
PERCENT:            '%';

// Assignment and Other Operators
COLON:              ':';
SEMICOLON:          ';';
COMMA:              ',';
DOT:                '.';
LPAREN:             '(';
RPAREN:             ')';
LBRACE:             '{';
RBRACE:             '}';
LBRACKET:           '[';
RBRACKET:           ']';
QMARK:              '?';
RANGE:              '..';

// =============================================================================
// Literals
// =============================================================================

// Boolean Literals (handled as keywords K_TRUE and K_FALSE)

// UUID Literal
UUID
    : HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT
      '-' HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT
      '-' HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT
      '-' HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT
      '-' HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT
          HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT
    ;

// Hexadecimal Blob
HEXNUMBER
    : '0' [xX] HEX_DIGIT+
    ;

// Integer Literal (decimal)
INTEGER
    : '-'? DIGIT+
    ;

// Floating Point Literal
FLOAT
    : '-'? DIGIT+ '.' DIGIT* EXPONENT?
    | '-'? '.' DIGIT+ EXPONENT?
    | '-'? DIGIT+ EXPONENT
    ;

// Duration Literal (ISO 8601 and Cassandra-specific)
DURATION
    : '-'? DIGIT+ ('y' | 'mo' | 'w' | 'd' | 'h' | 'm' | 's' | 'ms' | 'us' | 'µs' | 'ns')
    | '-'? 'P' (DIGIT+ 'Y')? (DIGIT+ 'M')? (DIGIT+ 'D')? ('T' (DIGIT+ 'H')? (DIGIT+ 'M')? (DIGIT+ 'S')?)?
    ;

// String Literals
STRING_LITERAL
    : '\'' ( ~'\'' | '\'\'' )* '\''
    ;

// Dollar-quoted string (for UDF bodies)
DOLLAR_STRING
    : '$$' .*? '$$'
    ;

// Identifiers
IDENT
    : LETTER (LETTER | DIGIT | '_')*
    ;

// Quoted Identifier
QUOTED_IDENT
    : '"' ( ~'"' | '""' )* '"'
    ;

// Bind Marker (named)
NAMED_BIND_MARKER
    : ':' IDENT
    ;

// =============================================================================
// Comments
// =============================================================================

SINGLE_LINE_COMMENT
    : ('--' | '//') ~[\r\n]* -> channel(HIDDEN)
    ;

MULTI_LINE_COMMENT
    : '/*' .*? '*/' -> channel(HIDDEN)
    ;

// =============================================================================
// Whitespace
// =============================================================================

WS
    : [ \t\r\n]+ -> channel(HIDDEN)
    ;

// =============================================================================
// Fragment Rules
// =============================================================================

fragment LETTER
    : [a-zA-Z]
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

// Case-insensitive letter fragments
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
