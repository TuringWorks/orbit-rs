//! Lexical analyzer (tokenizer) for OrbitQL
//!
//! This module provides the `Lexer` struct that converts OrbitQL query strings
//! into a stream of tokens for parsing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Token types in OrbitQL
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TokenType {
    // Literals
    Integer,
    Float,
    String,
    Boolean,
    Null,

    // Identifiers
    Identifier,
    Parameter, // $param or @param

    // Keywords
    Select,
    From,
    Where,
    Join,
    Inner,
    Left,
    Right,
    Full,
    Cross,
    On,
    Group,
    By,
    Having,
    Order,
    Limit,
    Offset,
    Insert,
    Into,
    Values,
    Update,
    Set,
    Delete,
    Create,
    Drop,
    Table,
    Index,
    View,
    Function,
    Trigger,
    Schema,
    If,
    Exists,
    Not,
    And,
    Or,
    Like,
    ILike,
    In,
    Between,
    Is,
    As,
    Distinct,
    All,
    Any,
    Some,
    Case,
    When,
    Then,
    Else,
    End,
    Fetch,
    Timeout,
    Live,
    Diff,
    Begin,
    Commit,
    Rollback,
    Relate,
    With,
    Recursive,

    // Graph keywords
    Node,
    Edge,
    Path,
    Connected,

    // Time-series keywords
    Metrics,
    Aggregate,
    Window,
    Range,
    Now,
    Interval,

    // ML keywords
    Model,
    Train,
    Predict,
    Using,
    Algorithm,
    Features,
    Target,
    Evaluate,
    Score,
    Fit,
    Transform,

    // Operators
    Equal,              // =
    NotEqual,           // !=, <>
    LessThan,           // <
    LessThanOrEqual,    // <=
    GreaterThan,        // >
    GreaterThanOrEqual, // >=
    Plus,               // +
    Minus,              // -
    Multiply,           // *
    Divide,             // /
    Modulo,             // %
    PlusEqual,          // +=
    MinusEqual,         // -=
    MultiplyEqual,      // *=
    DivideEqual,        // /=

    // Graph operators
    ArrowRight, // ->
    ArrowLeft,  // <-
    ArrowBoth,  // <>

    // Punctuation
    LeftParen,    // (
    RightParen,   // )
    LeftBracket,  // [
    RightBracket, // ]
    LeftBrace,    // {
    RightBrace,   // }
    Comma,        // ,
    Semicolon,    // ;
    Dot,          // .
    Colon,        // :
    DoubleColon,  // ::
    Question,     // ?
    At,           // @
    Dollar,       // $
    Hash,         // #

    // Special
    Whitespace,
    Comment,
    Newline,
    Eof,
}

/// A token with its type, value, and position
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub token_type: TokenType,
    pub value: String,
    pub line: usize,
    pub column: usize,
    pub position: usize,
}

impl Token {
    pub fn new(
        token_type: TokenType,
        value: String,
        line: usize,
        column: usize,
        position: usize,
    ) -> Self {
        Self {
            token_type,
            value,
            line,
            column,
            position,
        }
    }

    pub fn is_keyword(&self) -> bool {
        use TokenType::*;
        matches!(
            self.token_type,
            Select
                | From
                | Where
                | Join
                | Inner
                | Left
                | Right
                | Full
                | Cross
                | On
                | Group
                | By
                | Having
                | Order
                | Limit
                | Offset
                | Insert
                | Into
                | Values
                | Update
                | Set
                | Delete
                | Create
                | Drop
                | Table
                | Index
                | View
                | Function
                | Trigger
                | Schema
                | If
                | Exists
                | Not
                | And
                | Or
                | Like
                | ILike
                | In
                | Between
                | Is
                | As
                | Distinct
                | All
                | Any
                | Some
                | Case
                | When
                | Then
                | Else
                | End
                | Fetch
                | Timeout
                | Live
                | Diff
                | Begin
                | Commit
                | Rollback
                | Relate
                | With
                | Recursive
                | Node
                | Edge
                | Path
                | Connected
                | Metrics
                | Aggregate
                | Window
                | Range
                | Now
                | Interval
                | Model
                | Train
                | Predict
                | Using
                | Algorithm
                | Features
                | Target
                | Evaluate
                | Score
                | Fit
                | Transform
        )
    }

    pub fn is_operator(&self) -> bool {
        use TokenType::*;
        matches!(
            self.token_type,
            Equal
                | NotEqual
                | LessThan
                | LessThanOrEqual
                | GreaterThan
                | GreaterThanOrEqual
                | Plus
                | Minus
                | Multiply
                | Divide
                | Modulo
                | PlusEqual
                | MinusEqual
                | MultiplyEqual
                | DivideEqual
                | ArrowRight
                | ArrowLeft
                | ArrowBoth
        )
    }

    pub fn is_literal(&self) -> bool {
        use TokenType::*;
        matches!(self.token_type, Integer | Float | String | Boolean | Null)
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}({})", self.token_type, self.value)
    }
}

/// Lexer errors
#[derive(Debug, Clone, PartialEq)]
pub enum LexError {
    UnexpectedCharacter {
        char: char,
        line: usize,
        column: usize,
    },
    UnterminatedString {
        line: usize,
        column: usize,
    },
    InvalidNumber {
        value: String,
        line: usize,
        column: usize,
    },
    InvalidEscape {
        sequence: String,
        line: usize,
        column: usize,
    },
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexError::UnexpectedCharacter { char, line, column } => {
                write!(
                    f,
                    "Unexpected character '{char}' at line {line}, column {column}"
                )
            }
            LexError::UnterminatedString { line, column } => {
                write!(f, "Unterminated string at line {line}, column {column}")
            }
            LexError::InvalidNumber {
                value,
                line,
                column,
            } => {
                write!(
                    f,
                    "Invalid number '{value}' at line {line}, column {column}"
                )
            }
            LexError::InvalidEscape {
                sequence,
                line,
                column,
            } => {
                write!(
                    f,
                    "Invalid escape sequence '{sequence}' at line {line}, column {column}"
                )
            }
        }
    }
}

impl std::error::Error for LexError {}

/// OrbitQL lexer/tokenizer
pub struct Lexer {
    keywords: HashMap<String, TokenType>,
}

impl Lexer {
    pub fn new() -> Self {
        let mut keywords = HashMap::new();

        // SQL keywords
        keywords.insert("SELECT".to_string(), TokenType::Select);
        keywords.insert("FROM".to_string(), TokenType::From);
        keywords.insert("WHERE".to_string(), TokenType::Where);
        keywords.insert("JOIN".to_string(), TokenType::Join);
        keywords.insert("INNER".to_string(), TokenType::Inner);
        keywords.insert("LEFT".to_string(), TokenType::Left);
        keywords.insert("RIGHT".to_string(), TokenType::Right);
        keywords.insert("FULL".to_string(), TokenType::Full);
        keywords.insert("CROSS".to_string(), TokenType::Cross);
        keywords.insert("ON".to_string(), TokenType::On);
        keywords.insert("GROUP".to_string(), TokenType::Group);
        keywords.insert("BY".to_string(), TokenType::By);
        keywords.insert("HAVING".to_string(), TokenType::Having);
        keywords.insert("ORDER".to_string(), TokenType::Order);
        keywords.insert("LIMIT".to_string(), TokenType::Limit);
        keywords.insert("OFFSET".to_string(), TokenType::Offset);
        keywords.insert("INSERT".to_string(), TokenType::Insert);
        keywords.insert("INTO".to_string(), TokenType::Into);
        keywords.insert("VALUES".to_string(), TokenType::Values);
        keywords.insert("UPDATE".to_string(), TokenType::Update);
        keywords.insert("SET".to_string(), TokenType::Set);
        keywords.insert("DELETE".to_string(), TokenType::Delete);
        keywords.insert("CREATE".to_string(), TokenType::Create);
        keywords.insert("DROP".to_string(), TokenType::Drop);
        keywords.insert("TABLE".to_string(), TokenType::Table);
        keywords.insert("INDEX".to_string(), TokenType::Index);
        keywords.insert("VIEW".to_string(), TokenType::View);
        keywords.insert("FUNCTION".to_string(), TokenType::Function);
        keywords.insert("TRIGGER".to_string(), TokenType::Trigger);
        keywords.insert("SCHEMA".to_string(), TokenType::Schema);
        keywords.insert("IF".to_string(), TokenType::If);
        keywords.insert("EXISTS".to_string(), TokenType::Exists);
        keywords.insert("NOT".to_string(), TokenType::Not);
        keywords.insert("AND".to_string(), TokenType::And);
        keywords.insert("OR".to_string(), TokenType::Or);
        keywords.insert("LIKE".to_string(), TokenType::Like);
        keywords.insert("ILIKE".to_string(), TokenType::ILike);
        keywords.insert("IN".to_string(), TokenType::In);
        keywords.insert("BETWEEN".to_string(), TokenType::Between);
        keywords.insert("IS".to_string(), TokenType::Is);
        keywords.insert("AS".to_string(), TokenType::As);
        keywords.insert("DISTINCT".to_string(), TokenType::Distinct);
        keywords.insert("ALL".to_string(), TokenType::All);
        keywords.insert("ANY".to_string(), TokenType::Any);
        keywords.insert("SOME".to_string(), TokenType::Some);
        keywords.insert("CASE".to_string(), TokenType::Case);
        keywords.insert("WHEN".to_string(), TokenType::When);
        keywords.insert("THEN".to_string(), TokenType::Then);
        keywords.insert("ELSE".to_string(), TokenType::Else);
        keywords.insert("END".to_string(), TokenType::End);
        keywords.insert("FETCH".to_string(), TokenType::Fetch);
        keywords.insert("TIMEOUT".to_string(), TokenType::Timeout);
        keywords.insert("LIVE".to_string(), TokenType::Live);
        keywords.insert("DIFF".to_string(), TokenType::Diff);
        keywords.insert("BEGIN".to_string(), TokenType::Begin);
        keywords.insert("COMMIT".to_string(), TokenType::Commit);
        keywords.insert("ROLLBACK".to_string(), TokenType::Rollback);
        keywords.insert("RELATE".to_string(), TokenType::Relate);
        keywords.insert("WITH".to_string(), TokenType::With);
        keywords.insert("RECURSIVE".to_string(), TokenType::Recursive);

        // Graph keywords
        keywords.insert("NODE".to_string(), TokenType::Node);
        keywords.insert("EDGE".to_string(), TokenType::Edge);
        keywords.insert("PATH".to_string(), TokenType::Path);
        keywords.insert("CONNECTED".to_string(), TokenType::Connected);

        // Time-series keywords
        keywords.insert("METRICS".to_string(), TokenType::Metrics);
        keywords.insert("AGGREGATE".to_string(), TokenType::Aggregate);
        keywords.insert("WINDOW".to_string(), TokenType::Window);
        keywords.insert("RANGE".to_string(), TokenType::Range);
        keywords.insert("NOW".to_string(), TokenType::Now);
        keywords.insert("INTERVAL".to_string(), TokenType::Interval);

        // ML keywords
        keywords.insert("MODEL".to_string(), TokenType::Model);
        keywords.insert("TRAIN".to_string(), TokenType::Train);
        keywords.insert("PREDICT".to_string(), TokenType::Predict);
        keywords.insert("USING".to_string(), TokenType::Using);
        keywords.insert("ALGORITHM".to_string(), TokenType::Algorithm);
        keywords.insert("FEATURES".to_string(), TokenType::Features);
        keywords.insert("TARGET".to_string(), TokenType::Target);
        keywords.insert("EVALUATE".to_string(), TokenType::Evaluate);
        keywords.insert("SCORE".to_string(), TokenType::Score);
        keywords.insert("FIT".to_string(), TokenType::Fit);
        keywords.insert("TRANSFORM".to_string(), TokenType::Transform);

        // Boolean and null literals
        keywords.insert("TRUE".to_string(), TokenType::Boolean);
        keywords.insert("FALSE".to_string(), TokenType::Boolean);
        keywords.insert("NULL".to_string(), TokenType::Null);

        Self { keywords }
    }

    /// Tokenize an OrbitQL query string
    pub fn tokenize(&self, input: &str) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        let mut chars = input.chars().peekable();
        let mut line = 1;
        let mut column = 1;
        let mut position = 0;

        while let Some(&ch) = chars.peek() {
            match ch {
                // Whitespace
                ' ' | '\t' | '\r' => {
                    chars.next();
                    column += 1;
                    position += 1;
                }

                // Newlines
                '\n' => {
                    chars.next();
                    line += 1;
                    column = 1;
                    position += 1;
                }

                // Comments
                '-' if chars.clone().nth(1) == Some('-') => {
                    // Single line comment
                    chars.next(); // consume first -
                    chars.next(); // consume second -
                    let start_pos = position;
                    position += 2;
                    column += 2;

                    let mut comment = String::new();
                    while let Some(&ch) = chars.peek() {
                        if ch == '\n' {
                            break;
                        }
                        comment.push(ch);
                        chars.next();
                        column += 1;
                        position += 1;
                    }

                    let comment_len = comment.len();
                    tokens.push(Token::new(
                        TokenType::Comment,
                        comment,
                        line,
                        column - comment_len,
                        start_pos,
                    ));
                }

                '/' if chars.clone().nth(1) == Some('*') => {
                    // Multi-line comment
                    chars.next(); // consume /
                    chars.next(); // consume *
                    let start_pos = position;
                    let start_column = column;
                    position += 2;
                    column += 2;

                    let mut comment = String::new();
                    let mut found_end = false;

                    while let Some(ch) = chars.next() {
                        if ch == '*' && chars.peek() == Some(&'/') {
                            chars.next(); // consume /
                            position += 2;
                            column += 2;
                            found_end = true;
                            break;
                        }

                        if ch == '\n' {
                            line += 1;
                            column = 1;
                        } else {
                            column += 1;
                        }
                        position += 1;
                        comment.push(ch);
                    }

                    if !found_end {
                        return Err(LexError::UnterminatedString { line, column });
                    }

                    tokens.push(Token::new(
                        TokenType::Comment,
                        comment,
                        line,
                        start_column,
                        start_pos,
                    ));
                }

                // String literals
                '\'' | '"' => {
                    let quote_char = ch;
                    chars.next();
                    let start_pos = position;
                    let start_column = column;
                    position += 1;
                    column += 1;

                    let mut string_value = String::new();
                    let mut escaped = false;
                    let mut terminated = false;

                    for ch in chars.by_ref() {
                        position += 1;
                        column += 1;

                        if escaped {
                            match ch {
                                'n' => string_value.push('\n'),
                                't' => string_value.push('\t'),
                                'r' => string_value.push('\r'),
                                '\\' => string_value.push('\\'),
                                '\'' => string_value.push('\''),
                                '"' => string_value.push('"'),
                                _ => {
                                    return Err(LexError::InvalidEscape {
                                        sequence: format!("\\{ch}"),
                                        line,
                                        column: column - 1,
                                    });
                                }
                            }
                            escaped = false;
                        } else if ch == '\\' {
                            escaped = true;
                        } else if ch == quote_char {
                            tokens.push(Token::new(
                                TokenType::String,
                                string_value,
                                line,
                                start_column,
                                start_pos,
                            ));
                            terminated = true;
                            break;
                        } else {
                            string_value.push(ch);
                            if ch == '\n' {
                                line += 1;
                                column = 1;
                            }
                        }
                    }

                    if !terminated {
                        return Err(LexError::UnterminatedString { line, column });
                    }
                }

                // Numbers
                '0'..='9' => {
                    let start_pos = position;
                    let start_column = column;
                    let mut number = String::new();
                    let mut is_float = false;

                    while let Some(&ch) = chars.peek() {
                        if ch.is_ascii_digit() {
                            number.push(ch);
                            chars.next();
                            position += 1;
                            column += 1;
                        } else if ch == '.' && !is_float {
                            // Check if next character is a digit (not another dot or letter)
                            if let Some(next_ch) = chars.clone().nth(1) {
                                if next_ch.is_ascii_digit() {
                                    is_float = true;
                                    number.push(ch);
                                    chars.next();
                                    position += 1;
                                    column += 1;
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    let token_type = if is_float {
                        TokenType::Float
                    } else {
                        TokenType::Integer
                    };
                    tokens.push(Token::new(
                        token_type,
                        number,
                        line,
                        start_column,
                        start_pos,
                    ));
                }

                // Identifiers and keywords
                'a'..='z' | 'A'..='Z' | '_' => {
                    let start_pos = position;
                    let start_column = column;
                    let mut identifier = String::new();

                    while let Some(&ch) = chars.peek() {
                        if ch.is_ascii_alphanumeric() || ch == '_' {
                            identifier.push(ch);
                            chars.next();
                            position += 1;
                            column += 1;
                        } else {
                            break;
                        }
                    }

                    let token_type = self
                        .keywords
                        .get(&identifier.to_uppercase())
                        .cloned()
                        .unwrap_or(TokenType::Identifier);

                    tokens.push(Token::new(
                        token_type,
                        identifier,
                        line,
                        start_column,
                        start_pos,
                    ));
                }

                // Parameters
                '$' | '@' => {
                    chars.next();
                    let start_pos = position;
                    let start_column = column;
                    position += 1;
                    column += 1;

                    let mut param_name = String::new();
                    param_name.push(ch);

                    while let Some(&ch) = chars.peek() {
                        if ch.is_ascii_alphanumeric() || ch == '_' {
                            param_name.push(ch);
                            chars.next();
                            position += 1;
                            column += 1;
                        } else {
                            break;
                        }
                    }

                    tokens.push(Token::new(
                        TokenType::Parameter,
                        param_name,
                        line,
                        start_column,
                        start_pos,
                    ));
                }

                // Multi-character operators
                '<' => {
                    chars.next();
                    let start_pos = position;
                    let start_column = column;
                    position += 1;
                    column += 1;

                    match chars.peek() {
                        Some('=') => {
                            chars.next();
                            position += 1;
                            column += 1;
                            tokens.push(Token::new(
                                TokenType::LessThanOrEqual,
                                "<=".to_string(),
                                line,
                                start_column,
                                start_pos,
                            ));
                        }
                        Some('>') => {
                            chars.next();
                            position += 1;
                            column += 1;
                            tokens.push(Token::new(
                                TokenType::NotEqual,
                                "<>".to_string(),
                                line,
                                start_column,
                                start_pos,
                            ));
                        }
                        Some('-') => {
                            chars.next();
                            position += 1;
                            column += 1;
                            tokens.push(Token::new(
                                TokenType::ArrowLeft,
                                "<-".to_string(),
                                line,
                                start_column,
                                start_pos,
                            ));
                        }
                        _ => {
                            tokens.push(Token::new(
                                TokenType::LessThan,
                                "<".to_string(),
                                line,
                                start_column,
                                start_pos,
                            ));
                        }
                    }
                }

                '>' => {
                    chars.next();
                    let start_pos = position;
                    let start_column = column;
                    position += 1;
                    column += 1;

                    match chars.peek() {
                        Some('=') => {
                            chars.next();
                            position += 1;
                            column += 1;
                            tokens.push(Token::new(
                                TokenType::GreaterThanOrEqual,
                                ">=".to_string(),
                                line,
                                start_column,
                                start_pos,
                            ));
                        }
                        _ => {
                            tokens.push(Token::new(
                                TokenType::GreaterThan,
                                ">".to_string(),
                                line,
                                start_column,
                                start_pos,
                            ));
                        }
                    }
                }

                '!' => {
                    chars.next();
                    let start_pos = position;
                    let start_column = column;
                    position += 1;
                    column += 1;

                    if chars.peek() == Some(&'=') {
                        chars.next();
                        position += 1;
                        column += 1;
                        tokens.push(Token::new(
                            TokenType::NotEqual,
                            "!=".to_string(),
                            line,
                            start_column,
                            start_pos,
                        ));
                    } else {
                        return Err(LexError::UnexpectedCharacter {
                            char: '!',
                            line,
                            column: column - 1,
                        });
                    }
                }

                '+' => {
                    chars.next();
                    let start_pos = position;
                    let start_column = column;
                    position += 1;
                    column += 1;

                    if chars.peek() == Some(&'=') {
                        chars.next();
                        position += 1;
                        column += 1;
                        tokens.push(Token::new(
                            TokenType::PlusEqual,
                            "+=".to_string(),
                            line,
                            start_column,
                            start_pos,
                        ));
                    } else {
                        tokens.push(Token::new(
                            TokenType::Plus,
                            "+".to_string(),
                            line,
                            start_column,
                            start_pos,
                        ));
                    }
                }

                // Single-character operators and punctuation
                '=' => {
                    self.consume_single_char(
                        &mut chars,
                        &mut tokens,
                        TokenType::Equal,
                        line,
                        column,
                        position,
                    );
                    column += 1;
                    position += 1;
                }
                '*' => {
                    self.consume_single_char(
                        &mut chars,
                        &mut tokens,
                        TokenType::Multiply,
                        line,
                        column,
                        position,
                    );
                    column += 1;
                    position += 1;
                }
                '/' => {
                    self.consume_single_char(
                        &mut chars,
                        &mut tokens,
                        TokenType::Divide,
                        line,
                        column,
                        position,
                    );
                    column += 1;
                    position += 1;
                }
                '%' => {
                    self.consume_single_char(
                        &mut chars,
                        &mut tokens,
                        TokenType::Modulo,
                        line,
                        column,
                        position,
                    );
                    column += 1;
                    position += 1;
                }
                '(' => {
                    self.consume_single_char(
                        &mut chars,
                        &mut tokens,
                        TokenType::LeftParen,
                        line,
                        column,
                        position,
                    );
                    column += 1;
                    position += 1;
                }
                ')' => {
                    self.consume_single_char(
                        &mut chars,
                        &mut tokens,
                        TokenType::RightParen,
                        line,
                        column,
                        position,
                    );
                    column += 1;
                    position += 1;
                }
                '[' => {
                    self.consume_single_char(
                        &mut chars,
                        &mut tokens,
                        TokenType::LeftBracket,
                        line,
                        column,
                        position,
                    );
                    column += 1;
                    position += 1;
                }
                ']' => {
                    self.consume_single_char(
                        &mut chars,
                        &mut tokens,
                        TokenType::RightBracket,
                        line,
                        column,
                        position,
                    );
                    column += 1;
                    position += 1;
                }
                '{' => {
                    self.consume_single_char(
                        &mut chars,
                        &mut tokens,
                        TokenType::LeftBrace,
                        line,
                        column,
                        position,
                    );
                    column += 1;
                    position += 1;
                }
                '}' => {
                    self.consume_single_char(
                        &mut chars,
                        &mut tokens,
                        TokenType::RightBrace,
                        line,
                        column,
                        position,
                    );
                    column += 1;
                    position += 1;
                }
                ',' => {
                    self.consume_single_char(
                        &mut chars,
                        &mut tokens,
                        TokenType::Comma,
                        line,
                        column,
                        position,
                    );
                    column += 1;
                    position += 1;
                }
                ';' => {
                    self.consume_single_char(
                        &mut chars,
                        &mut tokens,
                        TokenType::Semicolon,
                        line,
                        column,
                        position,
                    );
                    column += 1;
                    position += 1;
                }
                '.' => {
                    self.consume_single_char(
                        &mut chars,
                        &mut tokens,
                        TokenType::Dot,
                        line,
                        column,
                        position,
                    );
                    column += 1;
                    position += 1;
                }
                ':' => {
                    chars.next();
                    let start_pos = position;
                    let start_column = column;
                    position += 1;
                    column += 1;

                    if chars.peek() == Some(&':') {
                        chars.next();
                        position += 1;
                        column += 1;
                        tokens.push(Token::new(
                            TokenType::DoubleColon,
                            "::".to_string(),
                            line,
                            start_column,
                            start_pos,
                        ));
                    } else {
                        tokens.push(Token::new(
                            TokenType::Colon,
                            ":".to_string(),
                            line,
                            start_column,
                            start_pos,
                        ));
                    }
                }
                '?' => {
                    self.consume_single_char(
                        &mut chars,
                        &mut tokens,
                        TokenType::Question,
                        line,
                        column,
                        position,
                    );
                    column += 1;
                    position += 1;
                }
                '#' => {
                    self.consume_single_char(
                        &mut chars,
                        &mut tokens,
                        TokenType::Hash,
                        line,
                        column,
                        position,
                    );
                    column += 1;
                    position += 1;
                }

                // Handle minus and arrow operators
                '-' => {
                    chars.next();
                    let start_pos = position;
                    let start_column = column;
                    position += 1;
                    column += 1;

                    match chars.peek() {
                        Some('=') => {
                            chars.next();
                            position += 1;
                            column += 1;
                            tokens.push(Token::new(
                                TokenType::MinusEqual,
                                "-=".to_string(),
                                line,
                                start_column,
                                start_pos,
                            ));
                        }
                        Some('>') => {
                            chars.next();
                            position += 1;
                            column += 1;
                            tokens.push(Token::new(
                                TokenType::ArrowRight,
                                "->".to_string(),
                                line,
                                start_column,
                                start_pos,
                            ));
                        }
                        _ => {
                            tokens.push(Token::new(
                                TokenType::Minus,
                                "-".to_string(),
                                line,
                                start_column,
                                start_pos,
                            ));
                        }
                    }
                }

                _ => {
                    return Err(LexError::UnexpectedCharacter {
                        char: ch,
                        line,
                        column,
                    });
                }
            }
        }

        tokens.push(Token::new(
            TokenType::Eof,
            String::new(),
            line,
            column,
            position,
        ));
        Ok(tokens)
    }

    fn consume_single_char<I>(
        &self,
        chars: &mut std::iter::Peekable<I>,
        tokens: &mut Vec<Token>,
        token_type: TokenType,
        line: usize,
        column: usize,
        position: usize,
    ) where
        I: Iterator<Item = char>,
    {
        if let Some(ch) = chars.next() {
            tokens.push(Token::new(
                token_type,
                ch.to_string(),
                line,
                column,
                position,
            ));
        }
    }
}

impl Default for Lexer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let lexer = Lexer::new();
        let tokens = lexer
            .tokenize("SELECT * FROM users WHERE age > 18")
            .unwrap();

        assert_eq!(tokens.len(), 9); // Including EOF
        assert_eq!(tokens[0].token_type, TokenType::Select);
        assert_eq!(tokens[1].token_type, TokenType::Multiply);
        assert_eq!(tokens[2].token_type, TokenType::From);
        assert_eq!(tokens[3].token_type, TokenType::Identifier);
        assert_eq!(tokens[3].value, "users");
        assert_eq!(tokens[4].token_type, TokenType::Where);
        assert_eq!(tokens[5].token_type, TokenType::Identifier);
        assert_eq!(tokens[5].value, "age");
        assert_eq!(tokens[6].token_type, TokenType::GreaterThan);
        assert_eq!(tokens[7].token_type, TokenType::Integer);
        assert_eq!(tokens[7].value, "18");
        // tokens[8] should be EOF
        assert_eq!(tokens[8].token_type, TokenType::Eof);
    }

    #[test]
    fn test_string_literals() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("'hello world' 'test string'").unwrap();

        assert_eq!(tokens.len(), 3); // Including EOF
        assert_eq!(tokens[0].token_type, TokenType::String);
        assert_eq!(tokens[0].value, "hello world");
        assert_eq!(tokens[1].token_type, TokenType::String);
        assert_eq!(tokens[1].value, "test string");
        assert_eq!(tokens[2].token_type, TokenType::Eof);
    }

    #[test]
    fn test_numbers() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("123 45.67 0.5").unwrap();

        assert_eq!(tokens.len(), 4); // Including EOF
        assert_eq!(tokens[0].token_type, TokenType::Integer);
        assert_eq!(tokens[0].value, "123");
        assert_eq!(tokens[1].token_type, TokenType::Float);
        assert_eq!(tokens[1].value, "45.67");
        assert_eq!(tokens[2].token_type, TokenType::Float);
        assert_eq!(tokens[2].value, "0.5");
    }

    #[test]
    fn test_graph_operators() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("->friends<-follows<>").unwrap();

        assert_eq!(tokens.len(), 6); // Including EOF
        assert_eq!(tokens[0].token_type, TokenType::ArrowRight);
        assert_eq!(tokens[0].value, "->");
        assert_eq!(tokens[1].token_type, TokenType::Identifier);
        assert_eq!(tokens[1].value, "friends");
        assert_eq!(tokens[2].token_type, TokenType::ArrowLeft);
        assert_eq!(tokens[2].value, "<-");
        assert_eq!(tokens[3].token_type, TokenType::Identifier);
        assert_eq!(tokens[3].value, "follows");
        assert_eq!(tokens[4].token_type, TokenType::NotEqual); // <> is parsed as NotEqual
        assert_eq!(tokens[4].value, "<>");
        assert_eq!(tokens[5].token_type, TokenType::Eof);
    }

    #[test]
    fn test_parameters() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("$name @age").unwrap();

        assert_eq!(tokens.len(), 3); // Including EOF
        assert_eq!(tokens[0].token_type, TokenType::Parameter);
        assert_eq!(tokens[0].value, "$name");
        assert_eq!(tokens[1].token_type, TokenType::Parameter);
        assert_eq!(tokens[1].value, "@age");
    }

    #[test]
    fn test_comments() {
        let lexer = Lexer::new();
        let tokens = lexer
            .tokenize("SELECT * -- this is a comment\nFROM users")
            .unwrap();

        // Comments are tokenized but typically filtered out by the parser
        assert!(tokens.iter().any(|t| t.token_type == TokenType::Comment));
        assert!(tokens.iter().any(|t| t.token_type == TokenType::Select));
        assert!(tokens.iter().any(|t| t.token_type == TokenType::From));
    }
}
