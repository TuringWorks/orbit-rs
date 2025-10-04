//! SQL Parser for ANSI SQL compliance
//!
//! This module provides a recursive descent parser that can handle
//! all major SQL constructs with proper error handling and recovery.

pub mod dcl;
pub mod ddl;
pub mod dml;
pub mod expressions;
pub mod select;
pub mod tcl;
pub mod utilities;

use crate::error::{ProtocolError, ProtocolResult};
use crate::postgres_wire::sql::{
    ast::Statement,
    lexer::{Lexer, Token},
};

/// Parse result type
pub type ParseResult<T> = Result<T, ParseError>;

/// Parse error information
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
    pub expected: Vec<String>,
    pub found: Option<Token>,
}

impl From<ParseError> for ProtocolError {
    fn from(err: ParseError) -> Self {
        ProtocolError::PostgresError(format!("SQL Parse Error: {}", err.message))
    }
}

/// Main SQL Parser
pub struct SqlParser {
    tokens: Vec<Token>,
    position: usize,
    current_token: Option<Token>,
}

impl SqlParser {
    /// Create a new SQL parser
    pub fn new() -> Self {
        Self {
            tokens: Vec::new(),
            position: 0,
            current_token: None,
        }
    }

    /// Parse a SQL statement from string input
    pub fn parse(&mut self, input: &str) -> ProtocolResult<Statement> {
        // Tokenize the input
        let mut lexer = Lexer::new(input);
        self.tokens = lexer.tokenize();
        self.position = 0;
        self.current_token = self.tokens.first().cloned();

        // Parse the statement
        self.parse_statement().map_err(|e| e.into())
    }

    /// Parse a top-level SQL statement
    fn parse_statement(&mut self) -> ParseResult<Statement> {
        match &self.current_token {
            // DDL Statements
            Some(Token::Create) => self.parse_create_statement(),
            Some(Token::Alter) => self.parse_alter_statement(),
            Some(Token::Drop) => self.parse_drop_statement(),

            // DML Statements
            Some(Token::Select) => self.parse_select_statement(),
            Some(Token::Insert) => self.parse_insert_statement(),
            Some(Token::Update) => self.parse_update_statement(),
            Some(Token::Delete) => self.parse_delete_statement(),

            // DCL Statements
            Some(Token::Grant) => self.parse_grant_statement(),
            Some(Token::Revoke) => self.parse_revoke_statement(),

            // TCL Statements
            Some(Token::Begin) => self.parse_begin_statement(),
            Some(Token::Commit) => self.parse_commit_statement(),
            Some(Token::Rollback) => self.parse_rollback_statement(),
            Some(Token::Savepoint) => self.parse_savepoint_statement(),

            Some(token) => Err(ParseError {
                message: format!("Unexpected token at start of statement: {:?}", token),
                position: self.position,
                expected: vec![
                    "CREATE".to_string(),
                    "ALTER".to_string(),
                    "DROP".to_string(),
                    "SELECT".to_string(),
                    "INSERT".to_string(),
                    "UPDATE".to_string(),
                    "DELETE".to_string(),
                    "GRANT".to_string(),
                    "REVOKE".to_string(),
                    "BEGIN".to_string(),
                    "COMMIT".to_string(),
                    "ROLLBACK".to_string(),
                ],
                found: Some(token.clone()),
            }),

            None => Err(ParseError {
                message: "Empty SQL statement".to_string(),
                position: self.position,
                expected: vec!["SQL statement".to_string()],
                found: None,
            }),
        }
    }

    /// Advance to the next token
    fn advance(&mut self) -> ParseResult<()> {
        self.position += 1;
        self.current_token = self.tokens.get(self.position).cloned();
        Ok(())
    }

    /// Peek at the next token without advancing
    #[allow(dead_code)]
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position + 1)
    }

    /// Check if current token matches expected token and advance if so
    fn expect(&mut self, expected: Token) -> ParseResult<()> {
        if let Some(token) = &self.current_token {
            if std::mem::discriminant(token) == std::mem::discriminant(&expected) {
                self.advance()?;
                Ok(())
            } else {
                Err(ParseError {
                    message: format!("Expected {:?}, found {:?}", expected, token),
                    position: self.position,
                    expected: vec![format!("{:?}", expected)],
                    found: Some(token.clone()),
                })
            }
        } else {
            Err(ParseError {
                message: format!("Expected {:?}, found EOF", expected),
                position: self.position,
                expected: vec![format!("{:?}", expected)],
                found: None,
            })
        }
    }

    /// Check if current token matches any of the expected tokens
    fn matches(&self, tokens: &[Token]) -> bool {
        if let Some(current) = &self.current_token {
            tokens
                .iter()
                .any(|token| std::mem::discriminant(current) == std::mem::discriminant(token))
        } else {
            false
        }
    }

    /// Parse CREATE statements
    fn parse_create_statement(&mut self) -> ParseResult<Statement> {
        self.expect(Token::Create)?;

        match &self.current_token {
            Some(Token::Table) => ddl::parse_create_table(self),
            Some(Token::Index) => ddl::parse_create_index(self),
            Some(Token::View) => ddl::parse_create_view(self),
            Some(Token::Schema) => ddl::parse_create_schema(self),
            Some(Token::Extension) => ddl::parse_create_extension(self),

            Some(token) => Err(ParseError {
                message: format!("Unexpected token after CREATE: {:?}", token),
                position: self.position,
                expected: vec![
                    "TABLE".to_string(),
                    "INDEX".to_string(),
                    "VIEW".to_string(),
                    "SCHEMA".to_string(),
                    "EXTENSION".to_string(),
                ],
                found: Some(token.clone()),
            }),

            None => Err(ParseError {
                message: "Expected object type after CREATE".to_string(),
                position: self.position,
                expected: vec!["TABLE, INDEX, VIEW, SCHEMA, or EXTENSION".to_string()],
                found: None,
            }),
        }
    }

    /// Parse ALTER statements
    fn parse_alter_statement(&mut self) -> ParseResult<Statement> {
        self.expect(Token::Alter)?;

        match &self.current_token {
            Some(Token::Table) => ddl::parse_alter_table(self),

            Some(token) => Err(ParseError {
                message: format!("Unexpected token after ALTER: {:?}", token),
                position: self.position,
                expected: vec!["TABLE".to_string()],
                found: Some(token.clone()),
            }),

            None => Err(ParseError {
                message: "Expected object type after ALTER".to_string(),
                position: self.position,
                expected: vec!["TABLE".to_string()],
                found: None,
            }),
        }
    }

    /// Parse DROP statements
    fn parse_drop_statement(&mut self) -> ParseResult<Statement> {
        self.expect(Token::Drop)?;

        match &self.current_token {
            Some(Token::Table) => ddl::parse_drop_table(self),
            Some(Token::Index) => ddl::parse_drop_index(self),
            Some(Token::View) => ddl::parse_drop_view(self),
            Some(Token::Schema) => ddl::parse_drop_schema(self),
            Some(Token::Extension) => ddl::parse_drop_extension(self),

            Some(token) => Err(ParseError {
                message: format!("Unexpected token after DROP: {:?}", token),
                position: self.position,
                expected: vec![
                    "TABLE".to_string(),
                    "INDEX".to_string(),
                    "VIEW".to_string(),
                    "SCHEMA".to_string(),
                    "EXTENSION".to_string(),
                ],
                found: Some(token.clone()),
            }),

            None => Err(ParseError {
                message: "Expected object type after DROP".to_string(),
                position: self.position,
                expected: vec!["TABLE, INDEX, VIEW, SCHEMA, or EXTENSION".to_string()],
                found: None,
            }),
        }
    }

    // DML statement implementations
    fn parse_select_statement(&mut self) -> ParseResult<Statement> {
        dml::parse_select(self)
    }

    fn parse_insert_statement(&mut self) -> ParseResult<Statement> {
        dml::parse_insert(self)
    }

    fn parse_update_statement(&mut self) -> ParseResult<Statement> {
        dml::parse_update(self)
    }

    fn parse_delete_statement(&mut self) -> ParseResult<Statement> {
        dml::parse_delete(self)
    }

    fn parse_grant_statement(&mut self) -> ParseResult<Statement> {
        dcl::parse_grant(self)
    }

    fn parse_revoke_statement(&mut self) -> ParseResult<Statement> {
        dcl::parse_revoke(self)
    }

    fn parse_begin_statement(&mut self) -> ParseResult<Statement> {
        tcl::parse_begin(self)
    }

    fn parse_commit_statement(&mut self) -> ParseResult<Statement> {
        tcl::parse_commit(self)
    }

    fn parse_rollback_statement(&mut self) -> ParseResult<Statement> {
        tcl::parse_rollback(self)
    }

    fn parse_savepoint_statement(&mut self) -> ParseResult<Statement> {
        tcl::parse_savepoint(self)
    }
}

impl Default for SqlParser {
    fn default() -> Self {
        Self::new()
    }
}
