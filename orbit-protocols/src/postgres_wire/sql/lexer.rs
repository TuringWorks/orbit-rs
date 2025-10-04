//! SQL Lexer for tokenizing SQL input
//!
//! This module provides a comprehensive lexer that can handle all SQL tokens
//! including keywords, identifiers, literals, operators, and vector extensions.

use std::collections::HashMap;

/// SQL tokens
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords - DDL
    Create,
    Alter,
    Drop,
    Table,
    Index,
    View,
    Schema,
    Extension,
    Constraint,
    Primary,
    Key,
    Foreign,
    References,
    Check,
    Unique,
    NotNull,
    Null,
    Default,
    Add,
    Column,
    Materialized,
    Authorization,
    Version,
    Cascade,

    // Keywords - DML
    Select,
    Insert,
    Update,
    Delete,
    From,
    Into,
    Values,
    Set,
    Where,
    Group,
    By,
    Having,
    Order,
    Limit,
    Offset,
    Join,
    Inner,
    Left,
    Right,
    Full,
    Outer,
    Cross,
    On,
    Using,
    Natural,
    Union,
    Intersect,
    Except,
    All,
    Distinct,
    Asc,
    Desc,
    Nulls,
    First,
    Last,

    // Keywords - DCL/TCL
    Grant,
    Revoke,
    Begin,
    Commit,
    Rollback,
    Savepoint,
    Transaction,
    Isolation,
    Level,
    Read,
    Write,
    Only,
    Uncommitted,
    Committed,
    Repeatable,
    Serializable,
    To,
    Option,
    For,
    Restrict,
    Public,
    Execute,
    Usage,
    Function,
    Sequence,
    Database,
    Work,
    No,
    Chain,
    Release,
    Do,

    // Keywords - Functions and operators
    Case,
    When,
    Then,
    Else,
    End,
    In,
    Between,
    Like,
    ILike,
    Similar,
    Is,
    Not,
    And,
    Or,
    Exists,
    Any,
    Some,
    Cast,
    As,
    If,
    Replace,

    // Aggregate Functions
    Count,
    Sum,
    Avg,
    Min,
    Max,

    // Window Functions
    Over,
    Partition,
    RowNumber,
    Rank,
    DenseRank,
    PercentRank,
    CumeDist,
    Ntile,
    Lag,
    Lead,
    FirstValue,
    LastValue,
    NthValue,
    Rows,
    Range,
    Unbounded,
    Preceding,
    Following,
    CurrentRow,

    // Keywords - Data types
    Boolean,
    SmallInt,
    Integer,
    BigInt,
    Decimal,
    Numeric,
    Real,
    DoublePrecision,
    Char,
    Varchar,
    Text,
    Bytea,
    Date,
    Time,
    Timestamp,
    Interval,
    Json,
    Jsonb,
    Array,
    Uuid,
    Vector,
    HalfVec,
    SparseVec,
    With,
    Without,
    Zone,

    // Keywords - Vector specific
    IvfFlat,
    Hnsw,
    Lists,
    M,
    EfConstruction,

    // Identifiers and literals
    Identifier(String),
    QuotedIdentifier(String),
    StringLiteral(String),
    NumericLiteral(String),
    BooleanLiteral(bool),

    // Operators - Arithmetic
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
    Power,

    // Operators - Comparison
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,

    // Operators - Vector
    VectorDistance,       // <->
    VectorInnerProduct,   // <#>
    VectorCosineDistance, // <=>

    // Operators - Other
    Concat,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    BitwiseNot,
    LeftShift,
    RightShift,

    // Punctuation
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    Comma,
    Semicolon,
    Dot,
    Colon,

    // Special tokens
    Parameter(u32),
    Eof,
    Whitespace,
    Comment(String),
}

/// Lexer for SQL input
pub struct Lexer {
    input: Vec<char>,
    position: usize,
    current_char: Option<char>,
    keywords: HashMap<String, Token>,
}

impl Lexer {
    /// Create a new lexer with SQL input
    pub fn new(input: &str) -> Self {
        let chars: Vec<char> = input.chars().collect();
        let current_char = chars.first().copied();

        let mut lexer = Self {
            input: chars,
            position: 0,
            current_char,
            keywords: HashMap::new(),
        };

        lexer.init_keywords();
        lexer
    }

    /// Initialize keyword mapping
    fn init_keywords(&mut self) {
        let keywords = [
            // DDL Keywords
            ("CREATE", Token::Create),
            ("ALTER", Token::Alter),
            ("DROP", Token::Drop),
            ("TABLE", Token::Table),
            ("INDEX", Token::Index),
            ("VIEW", Token::View),
            ("SCHEMA", Token::Schema),
            ("EXTENSION", Token::Extension),
            ("CONSTRAINT", Token::Constraint),
            ("PRIMARY", Token::Primary),
            ("KEY", Token::Key),
            ("FOREIGN", Token::Foreign),
            ("REFERENCES", Token::References),
            ("CHECK", Token::Check),
            ("UNIQUE", Token::Unique),
            ("NULL", Token::Null),
            ("DEFAULT", Token::Default),
            ("ADD", Token::Add),
            ("COLUMN", Token::Column),
            ("MATERIALIZED", Token::Materialized),
            ("AUTHORIZATION", Token::Authorization),
            ("VERSION", Token::Version),
            ("CASCADE", Token::Cascade),
            // DML Keywords
            ("SELECT", Token::Select),
            ("INSERT", Token::Insert),
            ("UPDATE", Token::Update),
            ("DELETE", Token::Delete),
            ("FROM", Token::From),
            ("INTO", Token::Into),
            ("VALUES", Token::Values),
            ("SET", Token::Set),
            ("WHERE", Token::Where),
            ("GROUP", Token::Group),
            ("BY", Token::By),
            ("HAVING", Token::Having),
            ("ORDER", Token::Order),
            ("LIMIT", Token::Limit),
            ("OFFSET", Token::Offset),
            ("JOIN", Token::Join),
            ("INNER", Token::Inner),
            ("LEFT", Token::Left),
            ("RIGHT", Token::Right),
            ("FULL", Token::Full),
            ("OUTER", Token::Outer),
            ("CROSS", Token::Cross),
            ("ON", Token::On),
            ("USING", Token::Using),
            ("NATURAL", Token::Natural),
            ("UNION", Token::Union),
            ("INTERSECT", Token::Intersect),
            ("EXCEPT", Token::Except),
            ("ALL", Token::All),
            ("DISTINCT", Token::Distinct),
            ("ASC", Token::Asc),
            ("DESC", Token::Desc),
            ("NULLS", Token::Nulls),
            ("FIRST", Token::First),
            ("LAST", Token::Last),
            // Control Keywords
            ("GRANT", Token::Grant),
            ("REVOKE", Token::Revoke),
            ("BEGIN", Token::Begin),
            ("COMMIT", Token::Commit),
            ("ROLLBACK", Token::Rollback),
            ("SAVEPOINT", Token::Savepoint),
            ("TRANSACTION", Token::Transaction),
            ("ISOLATION", Token::Isolation),
            ("LEVEL", Token::Level),
            ("READ", Token::Read),
            ("WRITE", Token::Write),
            ("ONLY", Token::Only),
            ("UNCOMMITTED", Token::Uncommitted),
            ("COMMITTED", Token::Committed),
            ("REPEATABLE", Token::Repeatable),
            ("SERIALIZABLE", Token::Serializable),
            ("TO", Token::To),
            ("OPTION", Token::Option),
            ("FOR", Token::For),
            ("RESTRICT", Token::Restrict),
            ("PUBLIC", Token::Public),
            ("EXECUTE", Token::Execute),
            ("USAGE", Token::Usage),
            ("FUNCTION", Token::Function),
            ("SEQUENCE", Token::Sequence),
            ("DATABASE", Token::Database),
            ("WORK", Token::Work),
            ("NO", Token::No),
            ("CHAIN", Token::Chain),
            ("RELEASE", Token::Release),
            ("DO", Token::Do),
            // Expression Keywords
            ("CASE", Token::Case),
            ("WHEN", Token::When),
            ("THEN", Token::Then),
            ("ELSE", Token::Else),
            ("END", Token::End),
            ("IN", Token::In),
            ("BETWEEN", Token::Between),
            ("LIKE", Token::Like),
            ("ILIKE", Token::ILike),
            ("SIMILAR", Token::Similar),
            ("IS", Token::Is),
            ("NOT", Token::Not),
            ("AND", Token::And),
            ("OR", Token::Or),
            ("EXISTS", Token::Exists),
            ("ANY", Token::Any),
            ("SOME", Token::Some),
            ("CAST", Token::Cast),
            ("AS", Token::As),
            ("IF", Token::If),
            ("REPLACE", Token::Replace),
            // Aggregate Functions
            ("COUNT", Token::Count),
            ("SUM", Token::Sum),
            ("AVG", Token::Avg),
            ("MIN", Token::Min),
            ("MAX", Token::Max),
            // Window Functions
            ("OVER", Token::Over),
            ("PARTITION", Token::Partition),
            ("ROW_NUMBER", Token::RowNumber),
            ("RANK", Token::Rank),
            ("DENSE_RANK", Token::DenseRank),
            ("PERCENT_RANK", Token::PercentRank),
            ("CUME_DIST", Token::CumeDist),
            ("NTILE", Token::Ntile),
            ("LAG", Token::Lag),
            ("LEAD", Token::Lead),
            ("FIRST_VALUE", Token::FirstValue),
            ("LAST_VALUE", Token::LastValue),
            ("NTH_VALUE", Token::NthValue),
            ("ROWS", Token::Rows),
            ("RANGE", Token::Range),
            ("UNBOUNDED", Token::Unbounded),
            ("PRECEDING", Token::Preceding),
            ("FOLLOWING", Token::Following),
            ("CURRENT", Token::CurrentRow),
            // Data Types
            ("BOOLEAN", Token::Boolean),
            ("SMALLINT", Token::SmallInt),
            ("INTEGER", Token::Integer),
            ("BIGINT", Token::BigInt),
            ("DECIMAL", Token::Decimal),
            ("NUMERIC", Token::Numeric),
            ("REAL", Token::Real),
            ("DOUBLE", Token::DoublePrecision),
            ("PRECISION", Token::DoublePrecision), // Handle "DOUBLE PRECISION"
            ("CHAR", Token::Char),
            ("VARCHAR", Token::Varchar),
            ("TEXT", Token::Text),
            ("BYTEA", Token::Bytea),
            ("DATE", Token::Date),
            ("TIME", Token::Time),
            ("TIMESTAMP", Token::Timestamp),
            ("INTERVAL", Token::Interval),
            ("JSON", Token::Json),
            ("JSONB", Token::Jsonb),
            ("ARRAY", Token::Array),
            ("UUID", Token::Uuid),
            ("WITH", Token::With),
            ("WITHOUT", Token::Without),
            ("ZONE", Token::Zone),
            // Vector Types
            ("VECTOR", Token::Vector),
            ("HALFVEC", Token::HalfVec),
            ("SPARSEVEC", Token::SparseVec),
            ("IVFFLAT", Token::IvfFlat),
            ("HNSW", Token::Hnsw),
            ("LISTS", Token::Lists),
            ("EF_CONSTRUCTION", Token::EfConstruction),
            // Boolean literals
            ("TRUE", Token::BooleanLiteral(true)),
            ("FALSE", Token::BooleanLiteral(false)),
        ];

        for (keyword, token) in keywords.iter() {
            self.keywords.insert(keyword.to_string(), token.clone());
        }
    }

    /// Advance to the next character
    fn advance(&mut self) {
        self.position += 1;
        self.current_char = self.input.get(self.position).copied();
    }

    /// Peek at the next character without advancing
    fn peek(&self) -> Option<char> {
        self.input.get(self.position + 1).copied()
    }

    /// Peek at character at offset without advancing
    #[allow(dead_code)]
    fn peek_offset(&self, offset: usize) -> Option<char> {
        self.input.get(self.position + offset).copied()
    }

    /// Skip whitespace characters
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current_char {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Read a single-line comment
    fn read_line_comment(&mut self) -> Token {
        let mut comment = String::new();

        // Skip the "--"
        self.advance();
        self.advance();

        while let Some(ch) = self.current_char {
            if ch == '\n' || ch == '\r' {
                break;
            }
            comment.push(ch);
            self.advance();
        }

        Token::Comment(comment)
    }

    /// Read a multi-line comment
    fn read_multiline_comment(&mut self) -> Token {
        let mut comment = String::new();

        // Skip the "/*"
        self.advance();
        self.advance();

        while let Some(ch) = self.current_char {
            if ch == '*' && self.peek() == Some('/') {
                // Skip "*/"
                self.advance();
                self.advance();
                break;
            }
            comment.push(ch);
            self.advance();
        }

        Token::Comment(comment)
    }

    /// Read a string literal
    fn read_string_literal(&mut self) -> Token {
        let quote_char = self.current_char.unwrap();
        let mut string = String::new();
        self.advance(); // Skip opening quote

        while let Some(ch) = self.current_char {
            if ch == quote_char {
                // Check for escaped quote
                if self.peek() == Some(quote_char) {
                    string.push(ch);
                    self.advance();
                    self.advance();
                } else {
                    // End of string
                    self.advance();
                    break;
                }
            } else if ch == '\\' {
                // Handle escape sequences
                self.advance();
                if let Some(escaped) = self.current_char {
                    match escaped {
                        'n' => string.push('\n'),
                        't' => string.push('\t'),
                        'r' => string.push('\r'),
                        '\\' => string.push('\\'),
                        '\'' => string.push('\''),
                        '"' => string.push('"'),
                        _ => {
                            string.push('\\');
                            string.push(escaped);
                        }
                    }
                    self.advance();
                }
            } else {
                string.push(ch);
                self.advance();
            }
        }

        Token::StringLiteral(string)
    }

    /// Read a quoted identifier
    fn read_quoted_identifier(&mut self) -> Token {
        let mut identifier = String::new();
        self.advance(); // Skip opening quote

        while let Some(ch) = self.current_char {
            if ch == '"' {
                if self.peek() == Some('"') {
                    // Escaped quote
                    identifier.push('"');
                    self.advance();
                    self.advance();
                } else {
                    // End of identifier
                    self.advance();
                    break;
                }
            } else {
                identifier.push(ch);
                self.advance();
            }
        }

        Token::QuotedIdentifier(identifier)
    }

    /// Read a numeric literal
    fn read_numeric_literal(&mut self) -> Token {
        let mut number = String::new();

        // Read integer part
        while let Some(ch) = self.current_char {
            if ch.is_ascii_digit() {
                number.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        // Check for decimal point
        if self.current_char == Some('.') && self.peek().is_some_and(|c| c.is_ascii_digit()) {
            number.push('.');
            self.advance();

            // Read fractional part
            while let Some(ch) = self.current_char {
                if ch.is_ascii_digit() {
                    number.push(ch);
                    self.advance();
                } else {
                    break;
                }
            }
        }

        // Check for scientific notation
        if let Some(ch) = self.current_char {
            if ch == 'e' || ch == 'E' {
                number.push(ch);
                self.advance();

                // Check for sign
                if let Some(sign) = self.current_char {
                    if sign == '+' || sign == '-' {
                        number.push(sign);
                        self.advance();
                    }
                }

                // Read exponent
                while let Some(ch) = self.current_char {
                    if ch.is_ascii_digit() {
                        number.push(ch);
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
        }

        Token::NumericLiteral(number)
    }

    /// Read an identifier or keyword
    fn read_identifier(&mut self) -> Token {
        let mut identifier = String::new();

        while let Some(ch) = self.current_char {
            if ch.is_alphanumeric() || ch == '_' || ch == '$' {
                identifier.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        // Check if it's a keyword
        if let Some(keyword) = self.keywords.get(&identifier.to_uppercase()) {
            keyword.clone()
        } else {
            Token::Identifier(identifier)
        }
    }

    /// Read a parameter placeholder ($1, $2, etc.)
    fn read_parameter(&mut self) -> Token {
        self.advance(); // Skip '$'
        let mut number = String::new();

        while let Some(ch) = self.current_char {
            if ch.is_ascii_digit() {
                number.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        if let Ok(param_num) = number.parse::<u32>() {
            Token::Parameter(param_num)
        } else {
            // If parsing fails, treat as identifier
            Token::Identifier(format!("${}", number))
        }
    }

    /// Get the next token
    pub fn next_token(&mut self) -> Token {
        loop {
            match self.current_char {
                None => return Token::Eof,
                Some(ch) => {
                    if ch.is_whitespace() {
                        self.skip_whitespace();
                        continue;
                    }

                    match ch {
                        // Comments
                        '-' if self.peek() == Some('-') => return self.read_line_comment(),
                        '/' if self.peek() == Some('*') => return self.read_multiline_comment(),

                        // String literals
                        '\'' | '"' if ch == '\'' => return self.read_string_literal(),

                        // Quoted identifiers
                        '"' => return self.read_quoted_identifier(),

                        // Numeric literals
                        c if c.is_ascii_digit() => return self.read_numeric_literal(),
                        '.' if self.peek().is_some_and(|c| c.is_ascii_digit()) => {
                            return self.read_numeric_literal()
                        }

                        // Parameters
                        '$' => return self.read_parameter(),

                        // Identifiers and keywords
                        c if c.is_alphabetic() || c == '_' => return self.read_identifier(),

                        // Operators and punctuation
                        '+' => {
                            self.advance();
                            return Token::Plus;
                        }
                        '-' => {
                            self.advance();
                            return Token::Minus;
                        }
                        '*' => {
                            self.advance();
                            return Token::Multiply;
                        }
                        '/' => {
                            self.advance();
                            return Token::Divide;
                        }
                        '%' => {
                            self.advance();
                            return Token::Modulo;
                        }
                        '^' => {
                            self.advance();
                            return Token::Power;
                        }

                        '=' => {
                            self.advance();
                            return Token::Equal;
                        }
                        '!' if self.peek() == Some('=') => {
                            self.advance();
                            self.advance();
                            return Token::NotEqual;
                        }
                        '<' => {
                            self.advance();
                            return match self.current_char {
                                Some('=') => {
                                    if self.peek() == Some('>') {
                                        // <=> vector cosine distance
                                        self.advance();
                                        self.advance();
                                        Token::VectorCosineDistance
                                    } else {
                                        self.advance();
                                        Token::LessThanOrEqual
                                    }
                                }
                                Some('>') => {
                                    self.advance();
                                    Token::NotEqual
                                }
                                Some('-') if self.peek() == Some('>') => {
                                    self.advance();
                                    self.advance();
                                    Token::VectorDistance
                                }
                                Some('#') if self.peek() == Some('>') => {
                                    self.advance();
                                    self.advance();
                                    Token::VectorInnerProduct
                                }
                                Some('<') => {
                                    self.advance();
                                    Token::LeftShift
                                }
                                _ => Token::LessThan,
                            };
                        }
                        '>' => {
                            self.advance();
                            return match self.current_char {
                                Some('=') => {
                                    self.advance();
                                    Token::GreaterThanOrEqual
                                }
                                Some('>') => {
                                    self.advance();
                                    Token::RightShift
                                }
                                _ => Token::GreaterThan,
                            };
                        }

                        '|' if self.peek() == Some('|') => {
                            self.advance();
                            self.advance();
                            return Token::Concat;
                        }
                        '|' => {
                            self.advance();
                            return Token::BitwiseOr;
                        }
                        '&' => {
                            self.advance();
                            return Token::BitwiseAnd;
                        }
                        '~' => {
                            self.advance();
                            return Token::BitwiseNot;
                        }

                        '(' => {
                            self.advance();
                            return Token::LeftParen;
                        }
                        ')' => {
                            self.advance();
                            return Token::RightParen;
                        }
                        '[' => {
                            self.advance();
                            return Token::LeftBracket;
                        }
                        ']' => {
                            self.advance();
                            return Token::RightBracket;
                        }
                        '{' => {
                            self.advance();
                            return Token::LeftBrace;
                        }
                        '}' => {
                            self.advance();
                            return Token::RightBrace;
                        }
                        ',' => {
                            self.advance();
                            return Token::Comma;
                        }
                        ';' => {
                            self.advance();
                            return Token::Semicolon;
                        }
                        '.' => {
                            self.advance();
                            return Token::Dot;
                        }
                        ':' => {
                            self.advance();
                            return Token::Colon;
                        }

                        _ => {
                            // Unknown character, skip it
                            self.advance();
                            continue;
                        }
                    }
                }
            }
        }
    }

    /// Tokenize the entire input
    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        loop {
            let token = self.next_token();
            let is_eof = matches!(token, Token::Eof);
            tokens.push(token);

            if is_eof {
                break;
            }
        }

        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let mut lexer = Lexer::new("SELECT * FROM table WHERE id = 1");
        let tokens = lexer.tokenize();

        assert_eq!(tokens[0], Token::Select);
        assert_eq!(tokens[1], Token::Multiply);
        assert_eq!(tokens[2], Token::From);
        assert_eq!(tokens[3], Token::Identifier("table".to_string()));
        assert_eq!(tokens[4], Token::Where);
        assert_eq!(tokens[5], Token::Identifier("id".to_string()));
        assert_eq!(tokens[6], Token::Equal);
        assert_eq!(tokens[7], Token::NumericLiteral("1".to_string()));
        assert_eq!(tokens[8], Token::Eof);
    }

    #[test]
    fn test_vector_operators() {
        let mut lexer = Lexer::new("embedding <-> query_vector");
        let tokens = lexer.tokenize();

        assert_eq!(tokens[0], Token::Identifier("embedding".to_string()));
        assert_eq!(tokens[1], Token::VectorDistance);
        assert_eq!(tokens[2], Token::Identifier("query_vector".to_string()));
    }

    #[test]
    fn test_string_literals() {
        let mut lexer = Lexer::new("'hello world' \"quoted identifier\"");
        let tokens = lexer.tokenize();

        assert_eq!(tokens[0], Token::StringLiteral("hello world".to_string()));
        assert_eq!(
            tokens[1],
            Token::QuotedIdentifier("quoted identifier".to_string())
        );
    }
}
