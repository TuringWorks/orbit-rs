/*
 * CypherLexer.g4 - ANTLR4 Lexer Grammar for Neo4j Cypher 5.x
 * 
 * Based on openCypher specification and Neo4j Cypher Manual
 * Compatible with Neo4j 5.x and Cypher 25
 * 
 * Licensed under Apache License 2.0
 */

lexer grammar CypherLexer;

// =============================================================================
// Keywords - Reserved Words
// =============================================================================

// Clauses
CALL        : C A L L ;
CREATE      : C R E A T E ;
DELETE      : D E L E T E ;
DETACH      : D E T A C H ;
FOREACH     : F O R E A C H ;
LOAD        : L O A D ;
MATCH       : M A T C H ;
MERGE       : M E R G E ;
OPTIONAL    : O P T I O N A L ;
REMOVE      : R E M O V E ;
RETURN      : R E T U R N ;
SET         : S E T ;
UNWIND      : U N W I N D ;
WITH        : W I T H ;

// Sub-clauses
LIMIT       : L I M I T ;
ORDER       : O R D E R ;
SKIP_       : S K I P ;
WHERE       : W H E R E ;

// Modifiers
ASC         : A S C ;
ASCENDING   : A S C E N D I N G ;
BY          : B Y ;
DESC        : D E S C ;
DESCENDING  : D E S C E N D I N G ;
ON          : O N ;

// Expressions
ALL         : A L L ;
AND         : A N D ;
AS          : A S ;
CASE        : C A S E ;
CONTAINS    : C O N T A I N S ;
COUNT       : C O U N T ;
DISTINCT    : D I S T I N C T ;
ELSE        : E L S E ;
END         : E N D ;
ENDS        : E N D S ;
EXISTS      : E X I S T S ;
IN          : I N ;
IS          : I S ;
NONE        : N O N E ;
NOT         : N O T ;
NULL        : N U L L ;
OR          : O R ;
SINGLE      : S I N G L E ;
STARTS      : S T A R T S ;
THEN        : T H E N ;
WHEN        : W H E N ;
XOR         : X O R ;

// Graph Patterns
NODE        : N O D E ;
RELATIONSHIP : R E L A T I O N S H I P ;
REL         : R E L ;

// Literals
TRUE        : T R U E ;
FALSE       : F A L S E ;

// Schema
CONSTRAINT  : C O N S T R A I N T ;
DROP        : D R O P ;
INDEX       : I N D E X ;
UNIQUE      : U N I Q U E ;

// Hints
USING       : U S I N G ;
JOIN        : J O I N ;
SCAN        : S C A N ;

// Database Management
DATABASE    : D A T A B A S E ;
DATABASES   : D A T A B A S E S ;
DEFAULT_    : D E F A U L T ;
SHOW        : S H O W ;
YIELD       : Y I E L D ;

// Transaction
BEGIN       : B E G I N ;
COMMIT      : C O M M I T ;
ROLLBACK    : R O L L B A C K ;

// Administrative
ALTER       : A L T E R ;
ASSERT      : A S S E R T ;
CATALOG     : C A T A L O G ;
DENY        : D E N Y ;
GRAPH       : G R A P H ;
GRANT       : G R A N T ;
REVOKE      : R E V O K E ;
ROLE        : R O L E ;
ROLES       : R O L E S ;
START       : S T A R T ;
STOP        : S T O P ;
USER        : U S E R ;
USERS       : U S E R S ;

// Aliases
ALIAS       : A L I A S ;
ALIASES     : A L I A S E S ;

// Server Management
SERVER      : S E R V E R ;
SERVERS     : S E R V E R S ;

// Properties
PROPERTIES  : P R O P E R T I E S ;
PROPERTY    : P R O P E R T Y ;
KEY         : K E Y ;
KEYS        : K E Y S ;

// Labels and Types
LABELS      : L A B E L S ;
LABEL       : L A B E L ;
TYPE        : T Y P E ;
TYPES       : T Y P E S ;

// Access Control
ACCESS      : A C C E S S ;
ADMIN       : A D M I N ;
ADMINISTRATOR : A D M I N I S T R A T O R ;
PRIVILEGE   : P R I V I L E G E ;
PRIVILEGES  : P R I V I L E G E S ;
EXECUTE     : E X E C U T E ;
BOOSTED     : B O O S T E D ;
IMPERSONATE : I M P E R S O N A T E ;

// Functions
FUNCTION    : F U N C T I O N ;
FUNCTIONS   : F U N C T I O N S ;
PROCEDURE   : P R O C E D U R E ;
PROCEDURES  : P R O C E D U R E S ;

// Settings
SETTING     : S E T T I N G ;
SETTINGS    : S E T T I N G S ;

// Options
OPTIONS     : O P T I O N S ;
OPTION      : O P T I O N ;

// Paths
PATH        : P A T H ;
PATHS       : P A T H S ;
SHORTEST    : S H O R T E S T ;
SHORTESTPATH : S H O R T E S T P A T H ;
ALLSHORTESTPATHS : A L L S H O R T E S T P A T H S ;
ANY         : A N Y ;
GROUPS      : G R O U P S ;
GROUP       : G R O U P ;

// Patterns (GQL/Cypher 25)
DIFFERENT   : D I F F E R E N T ;
BINDINGS    : B I N D I N G S ;
REPEATABLE  : R E P E A T A B L E ;
ELEMENTS    : E L E M E N T S ;

// Data Import
CSV         : C S V ;
HEADERS     : H E A D E R S ;
FROM        : F R O M ;
FIELDTERMINATOR : F I E L D T E R M I N A T O R ;

// Misc Keywords
IF          : I F ;
ELSE_       : E L S E ;
TERMINATE   : T E R M I N A T E ;
TERMINATED  : T E R M I N A T E D ;
CURRENT     : C U R R E N T ;
CHANGE      : C H A N G E ;
PASSWORD    : P A S S W O R D ;
ENCRYPTED   : E N C R Y P T E D ;
PLAINTEXT   : P L A I N T E X T ;
REQUIRED    : R E Q U I R E D ;
HOME        : H O M E ;
STATUS      : S T A T U S ;
ACTIVE      : A C T I V E ;
SUSPENDED   : S U S P E N D E D ;
NAME        : N A M E ;
NEW         : N E W ;
COPY        : C O P Y ;
MOVE        : M O V E ;
OF          : O F ;
TO          : T O ;
FOR         : F O R ;
EXIST       : E X I S T ;
NORMALIZE   : N O R M A L I Z E ;
NORMALIZED  : N O R M A L I Z E D ;
NFC         : N F C ;
NFD         : N F D ;
NFKC        : N F K C ;
NFKD        : N F K D ;
COLLECT     : C O L L E C T ;
UNION       : U N I O N ;

// Type Keywords (GQL/Cypher type system)
BOOLEAN     : B O O L E A N ;
STRING      : S T R I N G ;
INTEGER     : I N T E G E R ;
INT         : I N T ;
SIGNED      : S I G N E D ;
FLOAT       : F L O A T ;
DATE        : D A T E ;
LOCAL       : L O C A L ;
TIME        : T I M E ;
DATETIME    : D A T E T I M E ;
TIMESTAMP   : T I M E S T A M P ;
ZONED       : Z O N E D ;
DURATION    : D U R A T I O N ;
POINT       : P O I N T ;
LIST        : L I S T ;
MAP         : M A P ;
NOTHING     : N O T H I N G ;
ARRAY       : A R R A Y ;
VERTEX      : V E R T E X ;
EDGE        : E D G E ;

// Infinity and NaN
INFINITY    : I N F I N I T Y ;
INF         : I N F ;
NAN         : N A N ;

// Quantifiers
WHERE_      : W H E R E ;

// =============================================================================
// Operators and Punctuation
// =============================================================================

// Comparison
EQ          : '=' ;
NEQ         : '<>' ;
NEQ2        : '!=' ;
LT          : '<' ;
GT          : '>' ;
LTE         : '<=' ;
GTE         : '>=' ;

// Arithmetic
PLUS        : '+' ;
MINUS       : '-' ;
ASTERISK    : '*' ;
SLASH       : '/' ;
PERCENT     : '%' ;
CARET       : '^' ;

// String operators
PLUS_EQ     : '+=' ;

// Null-safe operators
NULLSAFE    : '??' ;

// Pattern
LPAREN      : '(' ;
RPAREN      : ')' ;
LBRACKET    : '[' ;
RBRACKET    : ']' ;
LBRACE      : '{' ;
RBRACE      : '}' ;

// Separators
COMMA       : ',' ;
DOT         : '.' ;
DOTDOT      : '..' ;
COLON       : ':' ;
SEMICOLON   : ';' ;
BAR         : '|' ;
DOLLAR      : '$' ;

// Relationship arrows
ARROW_LEFT          : '<-' ;
ARROW_RIGHT         : '->' ;
ARROW_LEFT_HEAD     : '<' ;
ARROW_RIGHT_HEAD    : '>' ;
DASH                : '-' ;

// Quantified Path Patterns (GQL)
QUESTION    : '?' ;
AMPERSAND   : '&' ;
EXCLAMATION : '!' ;

// =============================================================================
// Literals
// =============================================================================

// Unsigned Integer Literal
UNSIGNED_DECIMAL_INTEGER
    : DIGIT+
    ;

// Decimal Integer (with optional sign handled at parser level)
DECIMAL_INTEGER
    : DIGIT+
    ;

// Hexadecimal Integer
HEX_INTEGER
    : '0' X HEX_DIGIT+
    ;

// Octal Integer
OCTAL_INTEGER
    : '0' O OCTAL_DIGIT+
    ;

// Floating Point Number
FLOAT_LITERAL
    : DIGIT+ '.' DIGIT* EXPONENT?
    | '.' DIGIT+ EXPONENT?
    | DIGIT+ EXPONENT
    ;

fragment EXPONENT
    : E SIGN? DIGIT+
    ;

fragment SIGN
    : [+\-]
    ;

fragment DIGIT
    : [0-9]
    ;

fragment HEX_DIGIT
    : [0-9a-fA-F]
    ;

fragment OCTAL_DIGIT
    : [0-7]
    ;

// String Literals
STRING_LITERAL
    : '\'' ( ESC_SINGLE | ~['\\] )* '\''
    | '"' ( ESC_DOUBLE | ~["\\] )* '"'
    ;

fragment ESC_SINGLE
    : '\\' ( [\\'/bfnrt] | UNICODE_ESC )
    | '\'\''
    ;

fragment ESC_DOUBLE
    : '\\' ( [\\"/bfnrt] | UNICODE_ESC )
    | '""'
    ;

fragment UNICODE_ESC
    : 'u' HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT
    | 'U' HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT HEX_DIGIT
    ;

// =============================================================================
// Identifiers
// =============================================================================

// Unescaped identifier (symbolic name)
IDENTIFIER
    : ID_START ID_CONTINUE*
    ;

// Escaped identifier (backtick-quoted)
ESCAPED_IDENTIFIER
    : '`' ( ~[`] | '``' )+ '`'
    ;

fragment ID_START
    : [a-zA-Z_]
    | '\u00C0'..'\u00D6'
    | '\u00D8'..'\u00F6'
    | '\u00F8'..'\u02FF'
    | '\u0370'..'\u037D'
    | '\u037F'..'\u1FFF'
    | '\u200C'..'\u200D'
    | '\u2070'..'\u218F'
    | '\u2C00'..'\u2FEF'
    | '\u3001'..'\uD7FF'
    | '\uF900'..'\uFDCF'
    | '\uFDF0'..'\uFFFD'
    ;

fragment ID_CONTINUE
    : ID_START
    | [0-9]
    | '\u00B7'
    | '\u0300'..'\u036F'
    | '\u203F'..'\u2040'
    ;

// Parameter (prefixed with $)
PARAMETER
    : DOLLAR ( IDENTIFIER | DECIMAL_INTEGER )
    ;

// =============================================================================
// Comments and Whitespace
// =============================================================================

// Single-line comment
LINE_COMMENT
    : '//' ~[\r\n]* -> channel(HIDDEN)
    ;

// Multi-line comment
BLOCK_COMMENT
    : '/*' .*? '*/' -> channel(HIDDEN)
    ;

// Whitespace
WS
    : [ \t\r\n\u000C]+ -> channel(HIDDEN)
    ;

// =============================================================================
// Case-insensitive letter fragments
// =============================================================================

fragment A : [aA];
fragment B : [bB];
fragment C : [cC];
fragment D : [dD];
fragment E : [eE];
fragment F : [fF];
fragment G : [gG];
fragment H : [hH];
fragment I : [iI];
fragment J : [jJ];
fragment K : [kK];
fragment L : [lL];
fragment M : [mM];
fragment N : [nN];
fragment O : [oO];
fragment P : [pP];
fragment Q : [qQ];
fragment R : [rR];
fragment S : [sS];
fragment T : [tT];
fragment U : [uU];
fragment V : [vV];
fragment W : [wW];
fragment X : [xX];
fragment Y : [yY];
fragment Z : [zZ];
