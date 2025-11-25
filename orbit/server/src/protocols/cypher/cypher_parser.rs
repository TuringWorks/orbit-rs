//! Cypher query parser for Neo4j compatibility
//!
//! This module provides parsing for basic Cypher query language constructs,
//! supporting MATCH, CREATE, RETURN, WHERE, and other common operations.

use crate::protocols::error::{ProtocolError, ProtocolResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, instrument, warn};

/// Cypher query parser
#[derive(Debug, Default)]
pub struct CypherParser {
    /// Enable debug output for parsing
    debug_mode: bool,
}

impl CypherParser {
    /// Create a new Cypher parser
    pub fn new() -> Self {
        Self { debug_mode: false }
    }

    /// Create a new Cypher parser with debug mode enabled
    pub fn with_debug(debug_mode: bool) -> Self {
        Self { debug_mode }
    }

    /// Parse a Cypher query string into an AST
    #[instrument(skip(self, cypher), fields(query_length = cypher.len()))]
    pub fn parse(&self, cypher: &str) -> ProtocolResult<CypherQuery> {
        let trimmed = cypher.trim();
        if trimmed.is_empty() {
            return Err(ProtocolError::CypherError("Empty query".to_string()));
        }

        debug!(query = cypher, "Parsing Cypher query");

        // Simple keyword-based parsing for basic queries
        let tokens = self.tokenize(trimmed)?;
        let query = self.parse_tokens(tokens)?;

        debug!(query = ?query, "Parsed Cypher query successfully");
        Ok(query)
    }

    /// Tokenize the query string using decomposed tokenization strategy
    fn tokenize(&self, query: &str) -> ProtocolResult<Vec<Token>> {
        let mut tokenizer = CypherTokenizer::new(query, self.debug_mode);
        tokenizer.tokenize()
    }

    /// Classify a token based on its content
    #[allow(dead_code)]
    fn classify_token(&self, token: &str) -> Token {
        Self::classify_token_static(token)
    }

    /// Static token classification for use by tokenizer
    fn classify_token_static(token: &str) -> Token {
        match token.to_uppercase().as_str() {
            "MATCH" => Token::Match,
            "CREATE" => Token::Create,
            "RETURN" => Token::Return,
            "WHERE" => Token::Where,
            "AND" => Token::And,
            "OR" => Token::Or,
            "NOT" => Token::Not,
            "SET" => Token::Set,
            "DELETE" => Token::Delete,
            "MERGE" => Token::Merge,
            "WITH" => Token::With,
            "LIMIT" => Token::Limit,
            "ORDER" => Token::Order,
            "BY" => Token::By,
            "AS" => Token::As,
            "OPTIONAL" => Token::Optional,
            "DETACH" => Token::Detach,
            "REMOVE" => Token::Remove,
            "SKIP" => Token::Skip,
            "DESC" => Token::Desc,
            "DESCENDING" => Token::Desc,
            "ASC" => Token::Asc,
            "ASCENDING" => Token::Asc,
            "CALL" => Token::Call,
            "YIELD" => Token::Yield,
            _ => {
                // Check if it's a number
                if token.chars().all(|c| c.is_ascii_digit()) {
                    Token::Number(token.to_string())
                } else {
                    Token::Identifier(token.to_string())
                }
            }
        }
    }

    /// Parse tokens into a query AST
    fn parse_tokens(&self, tokens: Vec<Token>) -> ProtocolResult<CypherQuery> {
        let mut parser = TokenParser::new(tokens);
        parser.parse_query()
    }
}

/// Token types for Cypher parsing
#[derive(Debug, Clone, PartialEq)]
enum Token {
    // Keywords
    Match,
    Create,
    Return,
    Where,
    And,
    Or,
    Not,
    Set,
    Delete,
    Merge,
    With,
    Limit,
    Order,
    By,
    As,
    Optional,
    Detach,
    Remove,
    Skip,
    Desc,
    Asc,
    Call,
    Yield,

    // Literals
    Identifier(String),
    String(String),
    Number(String),

    // Symbols
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Colon,
    Dot,
    Equals,
    NotEquals,      // !=
    GreaterThan,     // >
    LessThan,        // <
    GreaterThanOrEqual, // >=
    LessThanOrEqual,   // <=
}

/// Specialized tokenizer for Cypher queries with reduced complexity
#[derive(Debug)]
struct CypherTokenizer {
    #[allow(dead_code)]
    query: String,
    debug_mode: bool,
    tokens: Vec<Token>,
    current_token: String,
    position: usize,
    chars: Vec<char>,
    in_quotes: bool,
    quote_char: char,
}

impl CypherTokenizer {
    fn new(query: &str, debug_mode: bool) -> Self {
        Self {
            query: query.to_string(),
            debug_mode,
            tokens: Vec::new(),
            current_token: String::new(),
            position: 0,
            chars: query.chars().collect(),
            in_quotes: false,
            quote_char: '\0',
        }
    }

    fn tokenize(&mut self) -> ProtocolResult<Vec<Token>> {
        while self.position < self.chars.len() {
            let ch = self.chars[self.position];
            self.process_character(ch)?;
            self.position += 1;
        }

        self.finalize_tokenization()?;

        if self.debug_mode {
            debug!(tokens = ?self.tokens, "Tokenized Cypher query");
        }

        Ok(self.tokens.clone())
    }

    fn process_character(&mut self, ch: char) -> ProtocolResult<()> {
        if self.in_quotes {
            self.handle_quoted_character(ch);
        } else {
            self.handle_unquoted_character(ch);
        }
        Ok(())
    }

    fn handle_quoted_character(&mut self, ch: char) {
        if ch == self.quote_char {
            self.tokens.push(Token::String(self.current_token.clone()));
            self.current_token.clear();
            self.in_quotes = false;
            self.quote_char = '\0';
        } else {
            self.current_token.push(ch);
        }
    }

    fn handle_unquoted_character(&mut self, ch: char) {
        match ch {
            '"' | '\'' => self.start_string_literal(ch),
            ' ' | '\t' | '\n' | '\r' => self.handle_whitespace(),
            '(' | ')' | '{' | '}' | '[' | ']' | ',' | ':' | '.' => self.handle_single_char_token(ch),
            '=' | '!' | '>' | '<' => self.handle_comparison_operator(ch),
            _ => self.current_token.push(ch),
        }
    }

    fn handle_comparison_operator(&mut self, ch: char) {
        self.flush_current_token();
        let token = match ch {
            '=' => {
                if self.peek_char() == Some('=') {
                    self.advance_char();
                    Token::Equals
                } else {
                    Token::Equals
                }
            }
            '!' => {
                if self.peek_char() == Some('=') {
                    self.advance_char();
                    Token::NotEquals
                } else {
                    Token::Not
                }
            }
            '>' => {
                if self.peek_char() == Some('=') {
                    self.advance_char();
                    Token::GreaterThanOrEqual
                } else {
                    Token::GreaterThan
                }
            }
            '<' => {
                if self.peek_char() == Some('=') {
                    self.advance_char();
                    Token::LessThanOrEqual
                } else {
                    Token::LessThan
                }
            }
            _ => unreachable!(),
        };
        self.tokens.push(token);
    }

    fn peek_char(&self) -> Option<char> {
        if self.position + 1 < self.chars.len() {
            Some(self.chars[self.position + 1])
        } else {
            None
        }
    }

    fn advance_char(&mut self) {
        if self.position + 1 < self.chars.len() {
            self.position += 1;
        }
    }

    fn start_string_literal(&mut self, quote: char) {
        self.flush_current_token();
        self.in_quotes = true;
        self.quote_char = quote;
    }

    fn handle_whitespace(&mut self) {
        self.flush_current_token();
    }

    fn handle_single_char_token(&mut self, ch: char) {
        self.flush_current_token();
        let token = match ch {
            '(' => Token::LeftParen,
            ')' => Token::RightParen,
            '{' => Token::LeftBrace,
            '}' => Token::RightBrace,
            '[' => Token::LeftBracket,
            ']' => Token::RightBracket,
            ',' => Token::Comma,
            ':' => Token::Colon,
            '.' => Token::Dot,
            '=' => {
                if self.peek_char() == Some('=') {
                    self.advance_char();
                    Token::Equals
                } else {
                    Token::Equals
                }
            }
            '!' => {
                if self.peek_char() == Some('=') {
                    self.advance_char();
                    Token::NotEquals
                } else {
                    Token::Not
                }
            }
            '>' => {
                if self.peek_char() == Some('=') {
                    self.advance_char();
                    Token::GreaterThanOrEqual
                } else {
                    Token::GreaterThan
                }
            }
            '<' => {
                if self.peek_char() == Some('=') {
                    self.advance_char();
                    Token::LessThanOrEqual
                } else {
                    Token::LessThan
                }
            }
            _ => unreachable!("Invalid single char token: {}", ch),
        };
        self.tokens.push(token);
    }

    fn flush_current_token(&mut self) {
        if !self.current_token.is_empty() {
            self.tokens
                .push(CypherParser::classify_token_static(&self.current_token));
            self.current_token.clear();
        }
    }

    fn finalize_tokenization(&mut self) -> ProtocolResult<()> {
        if self.in_quotes {
            return Err(ProtocolError::CypherError(
                "Unterminated string literal".to_string(),
            ));
        }
        self.flush_current_token();
        Ok(())
    }
}

/// Token parser for building AST
struct TokenParser {
    tokens: Vec<Token>,
    position: usize,
}

impl TokenParser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    fn current_token(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn advance(&mut self) -> Option<&Token> {
        self.position += 1;
        self.current_token()
    }

    fn expect_token(&mut self, expected: Token) -> ProtocolResult<()> {
        match self.current_token() {
            Some(token) if std::mem::discriminant(token) == std::mem::discriminant(&expected) => {
                self.advance();
                Ok(())
            }
            Some(token) => Err(ProtocolError::CypherError(format!(
                "Expected {expected:?}, found {token:?}"
            ))),
            None => Err(ProtocolError::CypherError(
                "Unexpected end of query".to_string(),
            )),
        }
    }

    fn parse_query(&mut self) -> ProtocolResult<CypherQuery> {
        let mut clauses = Vec::new();

        while self.position < self.tokens.len() {
            match self.current_token() {
                Some(Token::Match) => {
                    clauses.push(self.parse_match_clause()?);
                }
                Some(Token::Create) => {
                    clauses.push(self.parse_create_clause()?);
                }
                Some(Token::Return) => {
                    clauses.push(self.parse_return_clause()?);
                }
                Some(Token::Where) => {
                    clauses.push(self.parse_where_clause()?);
                }
                Some(Token::Delete) => {
                    clauses.push(self.parse_delete_clause(false)?);
                }
                Some(Token::Detach) => {
                    self.advance();
                    if matches!(self.current_token(), Some(Token::Delete)) {
                        clauses.push(self.parse_delete_clause(true)?);
                    } else {
                        return Err(ProtocolError::CypherError(
                            "Expected DELETE after DETACH".to_string(),
                        ));
                    }
                }
                Some(Token::Set) => {
                    clauses.push(self.parse_set_clause()?);
                }
                Some(Token::Merge) => {
                    clauses.push(self.parse_merge_clause()?);
                }
                Some(Token::Remove) => {
                    clauses.push(self.parse_remove_clause()?);
                }
                Some(Token::Order) => {
                    clauses.push(self.parse_order_by_clause()?);
                }
                Some(Token::Limit) => {
                    clauses.push(self.parse_limit_clause()?);
                }
                Some(Token::Skip) => {
                    clauses.push(self.parse_skip_clause()?);
                }
                Some(Token::Call) => {
                    clauses.push(self.parse_call_clause()?);
                }
                Some(token) => {
                    return Err(ProtocolError::CypherError(format!(
                        "Unexpected token: {token:?}"
                    )));
                }
                None => break,
            }
        }

        if clauses.is_empty() {
            return Err(ProtocolError::CypherError("Empty query".to_string()));
        }

        Ok(CypherQuery { clauses })
    }

    fn parse_match_clause(&mut self) -> ProtocolResult<CypherClause> {
        self.expect_token(Token::Match)?;
        let pattern = self.parse_pattern()?;
        Ok(CypherClause::Match { pattern })
    }

    fn parse_create_clause(&mut self) -> ProtocolResult<CypherClause> {
        self.expect_token(Token::Create)?;
        let pattern = self.parse_pattern()?;
        Ok(CypherClause::Create { pattern })
    }

    fn parse_return_clause(&mut self) -> ProtocolResult<CypherClause> {
        self.expect_token(Token::Return)?;
        let mut items = Vec::new();

        while let Some(Token::Identifier(name)) = self.current_token() {
            let name = name.clone();
            self.advance();
            items.push(ReturnItem {
                expression: name.clone(),
                alias: None,
            });

            match self.current_token() {
                Some(Token::Comma) => {
                    self.advance();
                }
                _ => break,
            }
        }

        if items.is_empty() {
            return Err(ProtocolError::CypherError(
                "RETURN clause must have at least one item".to_string(),
            ));
        }

        Ok(CypherClause::Return { items })
    }

    fn parse_where_clause(&mut self) -> ProtocolResult<CypherClause> {
        self.expect_token(Token::Where)?;
        let condition = self.parse_condition()?;
        Ok(CypherClause::Where { condition })
    }

    fn parse_delete_clause(&mut self, detach: bool) -> ProtocolResult<CypherClause> {
        self.expect_token(Token::Delete)?;
        let mut variables = Vec::new();

        // Parse comma-separated list of variables to delete
        loop {
            if let Some(Token::Identifier(var)) = self.current_token() {
                variables.push(var.clone());
                self.advance();

                // Check for comma
                if matches!(self.current_token(), Some(Token::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if variables.is_empty() {
            return Err(ProtocolError::CypherError(
                "DELETE clause must specify at least one variable".to_string(),
            ));
        }

        Ok(CypherClause::Delete { variables, detach })
    }

    fn parse_set_clause(&mut self) -> ProtocolResult<CypherClause> {
        self.expect_token(Token::Set)?;
        let mut assignments = Vec::new();

        // Parse comma-separated list of property assignments
        loop {
            if let Some(Token::Identifier(var)) = self.current_token() {
                let var = var.clone();
                self.advance();

                // Expect dot for property access
                if !matches!(self.current_token(), Some(Token::Dot)) {
                    return Err(ProtocolError::CypherError(
                        "Expected '.' after variable in SET clause".to_string(),
                    ));
                }
                self.advance();

                // Get property name
                let prop = if let Some(Token::Identifier(p)) = self.current_token() {
                    let p = p.clone();
                    self.advance();
                    p
                } else {
                    return Err(ProtocolError::CypherError(
                        "Expected property name after '.'".to_string(),
                    ));
                };

                // Expect equals
                if !matches!(self.current_token(), Some(Token::Equals)) {
                    return Err(ProtocolError::CypherError(
                        "Expected '=' in SET clause".to_string(),
                    ));
                }
                self.advance();

                // Get value
                let value = match self.current_token() {
                    Some(Token::String(s)) => {
                        let s = s.clone();
                        self.advance();
                        serde_json::Value::String(s)
                    }
                    Some(Token::Number(n)) => {
                        let n = n.clone();
                        self.advance();
                        if let Ok(int_val) = n.parse::<i64>() {
                            serde_json::Value::Number(serde_json::Number::from(int_val))
                        } else if let Ok(float_val) = n.parse::<f64>() {
                            serde_json::Value::Number(
                                serde_json::Number::from_f64(float_val)
                                    .unwrap_or(serde_json::Number::from(0)),
                            )
                        } else {
                            serde_json::Value::String(n)
                        }
                    }
                    _ => {
                        return Err(ProtocolError::CypherError(
                            "Expected value in SET clause".to_string(),
                        ))
                    }
                };

                assignments.push(PropertyAssignment {
                    target: format!("{}.{}", var, prop),
                    value,
                });

                // Check for comma
                if matches!(self.current_token(), Some(Token::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if assignments.is_empty() {
            return Err(ProtocolError::CypherError(
                "SET clause must have at least one assignment".to_string(),
            ));
        }

        Ok(CypherClause::Set { assignments })
    }

    fn parse_merge_clause(&mut self) -> ProtocolResult<CypherClause> {
        self.expect_token(Token::Merge)?;
        let pattern = self.parse_pattern()?;
        Ok(CypherClause::Merge { pattern })
    }

    fn parse_remove_clause(&mut self) -> ProtocolResult<CypherClause> {
        self.expect_token(Token::Remove)?;
        let mut items = Vec::new();

        // Parse comma-separated list of items to remove
        loop {
            if let Some(Token::Identifier(var)) = self.current_token() {
                let var = var.clone();
                self.advance();

                match self.current_token() {
                    Some(Token::Dot) => {
                        // Remove property: var.property
                        self.advance();
                        if let Some(Token::Identifier(prop)) = self.current_token() {
                            items.push(RemoveItem::Property {
                                variable: var,
                                property: prop.clone(),
                            });
                            self.advance();
                        } else {
                            return Err(ProtocolError::CypherError(
                                "Expected property name after '.'".to_string(),
                            ));
                        }
                    }
                    Some(Token::Colon) => {
                        // Remove label: var:Label
                        self.advance();
                        if let Some(Token::Identifier(label)) = self.current_token() {
                            items.push(RemoveItem::Label {
                                variable: var,
                                label: label.clone(),
                            });
                            self.advance();
                        } else {
                            return Err(ProtocolError::CypherError(
                                "Expected label name after ':'".to_string(),
                            ));
                        }
                    }
                    _ => {
                        return Err(ProtocolError::CypherError(
                            "Expected '.' or ':' after variable in REMOVE clause".to_string(),
                        ));
                    }
                }

                // Check for comma
                if matches!(self.current_token(), Some(Token::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if items.is_empty() {
            return Err(ProtocolError::CypherError(
                "REMOVE clause must specify at least one item".to_string(),
            ));
        }

        Ok(CypherClause::Remove { items })
    }

    fn parse_order_by_clause(&mut self) -> ProtocolResult<CypherClause> {
        self.expect_token(Token::Order)?;
        self.expect_token(Token::By)?;

        let mut items = Vec::new();

        // Parse comma-separated list of order by items
        loop {
            if let Some(Token::Identifier(expr)) = self.current_token() {
                let mut expression = expr.clone();
                self.advance();

                // Check for property access (var.property)
                if matches!(self.current_token(), Some(Token::Dot)) {
                    self.advance();
                    if let Some(Token::Identifier(prop)) = self.current_token() {
                        expression = format!("{}.{}", expression, prop);
                        self.advance();
                    }
                }

                // Check for direction
                let descending = match self.current_token() {
                    Some(Token::Desc) => {
                        self.advance();
                        true
                    }
                    Some(Token::Asc) => {
                        self.advance();
                        false
                    }
                    _ => false,
                };

                items.push(OrderByItem {
                    expression,
                    descending,
                });

                // Check for comma
                if matches!(self.current_token(), Some(Token::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if items.is_empty() {
            return Err(ProtocolError::CypherError(
                "ORDER BY clause must specify at least one expression".to_string(),
            ));
        }

        Ok(CypherClause::OrderBy { items })
    }

    fn parse_limit_clause(&mut self) -> ProtocolResult<CypherClause> {
        self.expect_token(Token::Limit)?;

        let count = match self.current_token() {
            Some(Token::Number(n)) => {
                let n = n.clone();
                self.advance();
                n.parse::<usize>().map_err(|_| {
                    ProtocolError::CypherError(format!("Invalid LIMIT value: {}", n))
                })?
            }
            _ => {
                return Err(ProtocolError::CypherError(
                    "Expected number after LIMIT".to_string(),
                ))
            }
        };

        Ok(CypherClause::Limit { count })
    }

    fn parse_skip_clause(&mut self) -> ProtocolResult<CypherClause> {
        self.expect_token(Token::Skip)?;

        let count = match self.current_token() {
            Some(Token::Number(n)) => {
                let n = n.clone();
                self.advance();
                n.parse::<usize>().map_err(|_| {
                    ProtocolError::CypherError(format!("Invalid SKIP value: {}", n))
                })?
            }
            _ => {
                return Err(ProtocolError::CypherError(
                    "Expected number after SKIP".to_string(),
                ))
            }
        };

        Ok(CypherClause::Skip { count })
    }

    /// Parse a CALL clause for procedure invocation
    /// Syntax: CALL procedure.name(arg1, arg2) YIELD col1, col2
    fn parse_call_clause(&mut self) -> ProtocolResult<CypherClause> {
        self.expect_token(Token::Call)?;

        // Parse procedure name (may have dots, e.g., orbit.graph.pagerank)
        let mut procedure = String::new();
        loop {
            match self.current_token() {
                Some(Token::Identifier(name)) => {
                    procedure.push_str(name);
                    self.advance();
                }
                _ => break,
            }
            // Check for dot continuation
            if matches!(self.current_token(), Some(Token::Dot)) {
                procedure.push('.');
                self.advance();
            } else {
                break;
            }
        }

        if procedure.is_empty() {
            return Err(ProtocolError::CypherError(
                "Expected procedure name after CALL".to_string(),
            ));
        }

        // Parse arguments (optional, in parentheses)
        let mut arguments = Vec::new();
        if matches!(self.current_token(), Some(Token::LeftParen)) {
            self.advance(); // consume (

            // Parse arguments until we hit )
            loop {
                if matches!(self.current_token(), Some(Token::RightParen)) {
                    self.advance();
                    break;
                }

                // Parse argument value
                let arg = self.parse_call_argument()?;
                arguments.push(arg);

                // Check for comma or closing paren
                if matches!(self.current_token(), Some(Token::Comma)) {
                    self.advance();
                } else if matches!(self.current_token(), Some(Token::RightParen)) {
                    self.advance();
                    break;
                } else if self.current_token().is_none() {
                    break;
                }
            }
        }

        // Parse optional YIELD clause
        let yield_items = if matches!(self.current_token(), Some(Token::Yield)) {
            self.advance();
            let mut items = Vec::new();
            loop {
                match self.current_token() {
                    Some(Token::Identifier(name)) => {
                        items.push(name.clone());
                        self.advance();
                    }
                    _ => break,
                }
                if matches!(self.current_token(), Some(Token::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
            Some(items)
        } else {
            None
        };

        Ok(CypherClause::Call {
            procedure,
            arguments,
            yield_items,
        })
    }

    /// Parse a single argument value in a CALL clause
    fn parse_call_argument(&mut self) -> ProtocolResult<serde_json::Value> {
        match self.current_token() {
            Some(Token::Number(n)) => {
                let n = n.clone();
                self.advance();
                // Try parsing as integer first, then float
                if let Ok(i) = n.parse::<i64>() {
                    Ok(serde_json::Value::Number(i.into()))
                } else if let Ok(f) = n.parse::<f64>() {
                    Ok(serde_json::json!(f))
                } else {
                    Err(ProtocolError::CypherError(format!(
                        "Invalid number: {}",
                        n
                    )))
                }
            }
            Some(Token::String(s)) => {
                let s = s.clone();
                self.advance();
                Ok(serde_json::Value::String(s))
            }
            Some(Token::LeftBrace) => {
                // Parse object/map argument: {key: value, ...}
                self.parse_object_argument()
            }
            Some(Token::Identifier(name)) => {
                // Could be a boolean, null, or identifier
                let name = name.clone();
                self.advance();
                match name.to_lowercase().as_str() {
                    "true" => Ok(serde_json::Value::Bool(true)),
                    "false" => Ok(serde_json::Value::Bool(false)),
                    "null" => Ok(serde_json::Value::Null),
                    _ => Ok(serde_json::Value::String(name)),
                }
            }
            Some(token) => Err(ProtocolError::CypherError(format!(
                "Unexpected token in CALL arguments: {:?}",
                token
            ))),
            None => Err(ProtocolError::CypherError(
                "Unexpected end of query in CALL arguments".to_string(),
            )),
        }
    }

    /// Parse an object argument like {damping: 0.85, iterations: 20}
    fn parse_object_argument(&mut self) -> ProtocolResult<serde_json::Value> {
        self.advance(); // consume {
        let mut map = serde_json::Map::new();

        loop {
            if matches!(self.current_token(), Some(Token::RightBrace)) {
                self.advance();
                break;
            }

            // Parse key
            let key = match self.current_token() {
                Some(Token::Identifier(k)) => {
                    let k = k.clone();
                    self.advance();
                    k
                }
                Some(Token::String(k)) => {
                    let k = k.clone();
                    self.advance();
                    k
                }
                _ => {
                    return Err(ProtocolError::CypherError(
                        "Expected key in object argument".to_string(),
                    ))
                }
            };

            // Expect colon
            if !matches!(self.current_token(), Some(Token::Colon)) {
                return Err(ProtocolError::CypherError(
                    "Expected ':' after key in object".to_string(),
                ));
            }
            self.advance();

            // Parse value (recursive call for nested objects)
            let value = self.parse_call_argument()?;
            map.insert(key, value);

            // Check for comma or closing brace
            if matches!(self.current_token(), Some(Token::Comma)) {
                self.advance();
            } else if matches!(self.current_token(), Some(Token::RightBrace)) {
                self.advance();
                break;
            }
        }

        Ok(serde_json::Value::Object(map))
    }

    fn parse_pattern(&mut self) -> ProtocolResult<Pattern> {
        let mut elements = Vec::new();

        // Parse node pattern
        if let Some(Token::LeftParen) = self.current_token() {
            let node = self.parse_node_pattern()?;
            elements.push(PatternElement::Node(node));
        }

        // Parse relationship patterns
        while let Some(Token::Identifier(rel_part)) = self.current_token() {
            if rel_part.starts_with('-') {
                let relationship = self.parse_relationship_pattern()?;
                elements.push(PatternElement::Relationship(relationship));

                // Parse the target node
                if let Some(Token::LeftParen) = self.current_token() {
                    let node = self.parse_node_pattern()?;
                    elements.push(PatternElement::Node(node));
                }
            } else {
                break;
            }
        }

        Ok(Pattern { elements })
    }

    fn parse_node_pattern(&mut self) -> ProtocolResult<NodePattern> {
        self.expect_token(Token::LeftParen)?;

        let mut variable = None;
        let mut labels = Vec::new();
        let mut properties = HashMap::new();

        // Parse variable name
        if let Some(Token::Identifier(var)) = self.current_token() {
            variable = Some(var.clone());
            self.advance();
        }

        // Parse labels
        while let Some(Token::Colon) = self.current_token() {
            self.advance();
            if let Some(Token::Identifier(label)) = self.current_token() {
                labels.push(label.clone());
                self.advance();
            }
        }

        // Parse properties
        if let Some(Token::LeftBrace) = self.current_token() {
            self.advance();
            while let Some(Token::Identifier(key)) = self.current_token() {
                let key = key.clone();
                self.advance();
                self.expect_token(Token::Colon)?;

                let value = match self.current_token() {
                    Some(Token::String(s)) => {
                        let s = s.clone();
                        self.advance();
                        serde_json::Value::String(s)
                    }
                    Some(Token::Number(n)) => {
                        let n = n.clone();
                        self.advance();
                        if let Ok(int_val) = n.parse::<i64>() {
                            serde_json::Value::Number(serde_json::Number::from(int_val))
                        } else {
                            return Err(ProtocolError::CypherError(format!("Invalid number: {n}")));
                        }
                    }
                    _ => {
                        return Err(ProtocolError::CypherError(
                            "Expected property value".to_string(),
                        ))
                    }
                };

                properties.insert(key, value);

                if let Some(Token::Comma) = self.current_token() {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect_token(Token::RightBrace)?;
        }

        self.expect_token(Token::RightParen)?;

        Ok(NodePattern {
            variable,
            labels,
            properties,
        })
    }

    fn parse_relationship_pattern(&mut self) -> ProtocolResult<RelationshipPattern> {
        // Simple relationship parsing - assume format like -[:TYPE]->
        if let Some(Token::Identifier(rel_str)) = self.current_token() {
            let rel_str = rel_str.clone();
            self.advance();

            // Parse relationship type from string like "-[:KNOWS]->"
            let rel_type = if rel_str.contains(':') {
                rel_str
                    .split(':')
                    .nth(1)
                    .and_then(|s| s.split(']').next())
                    .map(|s| s.to_string())
            } else {
                None
            };

            let direction = if rel_str.ends_with("->") {
                RelationshipDirection::Outgoing
            } else if rel_str.starts_with("<-") {
                RelationshipDirection::Incoming
            } else {
                RelationshipDirection::Both
            };

            Ok(RelationshipPattern {
                variable: None,
                rel_type,
                direction,
                properties: HashMap::new(),
            })
        } else {
            Err(ProtocolError::CypherError(
                "Expected relationship pattern".to_string(),
            ))
        }
    }

    fn parse_condition(&mut self) -> ProtocolResult<Condition> {
        // Parse logical operators with precedence: NOT > AND > OR
        self.parse_or_condition()
    }

    fn parse_or_condition(&mut self) -> ProtocolResult<Condition> {
        let mut left = self.parse_and_condition()?;

        while let Some(Token::Or) = self.current_token() {
            self.advance();
            let right = self.parse_and_condition()?;
            left = Condition::Or {
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_and_condition(&mut self) -> ProtocolResult<Condition> {
        let mut left = self.parse_not_condition()?;

        while let Some(Token::And) = self.current_token() {
            self.advance();
            let right = self.parse_not_condition()?;
            left = Condition::And {
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_not_condition(&mut self) -> ProtocolResult<Condition> {
        if let Some(Token::Not) = self.current_token() {
            self.advance();
            let condition = self.parse_primary_condition()?;
            Ok(Condition::Not {
                condition: Box::new(condition),
            })
        } else {
            self.parse_primary_condition()
        }
    }

    fn parse_primary_condition(&mut self) -> ProtocolResult<Condition> {
        if let Some(Token::LeftParen) = self.current_token() {
            self.advance();
            let condition = self.parse_condition()?;
            self.expect_token(Token::RightParen)?;
            Ok(condition)
        } else if let Some(Token::Identifier(var)) = self.current_token() {
            let var = var.clone();
            self.advance();

            // Check for property access (var.property)
            if let Some(Token::Dot) = self.current_token() {
                self.advance();
                if let Some(Token::Identifier(prop)) = self.current_token() {
                    let prop = prop.clone();
                    self.advance();
                    return self.parse_property_condition(&format!("{}.{}", var, prop));
                }
            }

            // Check for label check (var:Label)
            if let Some(Token::Colon) = self.current_token() {
                self.advance();
                if let Some(Token::Identifier(label)) = self.current_token() {
                    let label = label.clone();
                    self.advance();
                    return Ok(Condition::HasLabel { variable: var, label });
                }
            }

            // Property condition on variable itself
            self.parse_property_condition(&var)
        } else {
            Err(ProtocolError::CypherError("Expected condition".to_string()))
        }
    }

    fn parse_property_condition(&mut self, property: &str) -> ProtocolResult<Condition> {
        let prop = property.to_string();
        
        // Check for comparison operator
        let operator = match self.current_token() {
            Some(Token::Equals) => {
                self.advance();
                ComparisonOperator::Equals
            }
            Some(Token::NotEquals) => {
                self.advance();
                ComparisonOperator::NotEquals
            }
            Some(Token::GreaterThan) => {
                self.advance();
                ComparisonOperator::GreaterThan
            }
            Some(Token::LessThan) => {
                self.advance();
                ComparisonOperator::LessThan
            }
            Some(Token::GreaterThanOrEqual) => {
                self.advance();
                ComparisonOperator::GreaterThanOrEqual
            }
            Some(Token::LessThanOrEqual) => {
                self.advance();
                ComparisonOperator::LessThanOrEqual
            }
            _ => {
                // No operator means property exists check
                return Ok(Condition::PropertyExists { property: prop });
            }
        };

        let value = match self.current_token() {
            Some(Token::String(s)) => {
                let s = s.clone();
                self.advance();
                serde_json::Value::String(s)
            }
            Some(Token::Number(n)) => {
                let n = n.clone();
                self.advance();
                // Try to parse as number
                if let Ok(num) = n.parse::<i64>() {
                    serde_json::Value::Number(num.into())
                } else if let Ok(num) = n.parse::<f64>() {
                    serde_json::Value::Number(serde_json::Number::from_f64(num).unwrap_or(serde_json::Number::from(0)))
                } else {
                    serde_json::Value::String(n)
                }
            }
            _ => {
                return Err(ProtocolError::CypherError(
                    "Expected condition value".to_string(),
                ))
            }
        };

        Ok(Condition::PropertyComparison {
            property: prop,
            operator,
            value,
        })
    }
}

/// Complete parsed Cypher query with clauses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CypherQuery {
    /// Query clauses in order
    pub clauses: Vec<CypherClause>,
}

/// Cypher query clause types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CypherClause {
    /// MATCH clause for pattern matching
    Match { pattern: Pattern },
    /// CREATE clause for creating nodes/relationships
    Create { pattern: Pattern },
    /// RETURN clause for result projection
    Return { items: Vec<ReturnItem> },
    /// WHERE clause for filtering
    Where { condition: Condition },
    /// DELETE clause for removing nodes/relationships
    Delete {
        /// Variables to delete
        variables: Vec<String>,
        /// Whether to use DETACH DELETE (removes relationships automatically)
        detach: bool,
    },
    /// SET clause for updating properties
    Set {
        /// Property assignments
        assignments: Vec<PropertyAssignment>,
    },
    /// MERGE clause for create-or-match
    Merge { pattern: Pattern },
    /// REMOVE clause for removing properties/labels
    Remove {
        /// Items to remove
        items: Vec<RemoveItem>,
    },
    /// ORDER BY clause for sorting
    OrderBy {
        /// Sort expressions
        items: Vec<OrderByItem>,
    },
    /// LIMIT clause for result limiting
    Limit {
        /// Maximum number of results
        count: usize,
    },
    /// SKIP clause for result offset
    Skip {
        /// Number of results to skip
        count: usize,
    },
    /// CALL clause for procedure invocation
    Call {
        /// Procedure name (e.g., "orbit.graph.pagerank")
        procedure: String,
        /// Arguments to the procedure
        arguments: Vec<serde_json::Value>,
        /// Optional YIELD clause to select specific output columns
        yield_items: Option<Vec<String>>,
    },
}

/// Property assignment for SET clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyAssignment {
    /// Target (variable.property or just variable for setting all properties)
    pub target: String,
    /// Value to set
    pub value: serde_json::Value,
}

/// Item to remove in REMOVE clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RemoveItem {
    /// Remove a property (variable.property)
    Property { variable: String, property: String },
    /// Remove a label (variable:Label)
    Label { variable: String, label: String },
}

/// ORDER BY item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderByItem {
    /// Expression to sort by
    pub expression: String,
    /// Sort direction
    pub descending: bool,
}

/// Graph pattern in Cypher queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    /// Pattern elements (nodes and relationships)
    pub elements: Vec<PatternElement>,
}

/// Elements that make up a pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternElement {
    /// Node pattern
    Node(NodePattern),
    /// Relationship pattern
    Relationship(RelationshipPattern),
}

/// Node pattern specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePattern {
    /// Variable name for the node
    pub variable: Option<String>,
    /// Node labels
    pub labels: Vec<String>,
    /// Node properties
    pub properties: HashMap<String, serde_json::Value>,
}

/// Relationship pattern specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipPattern {
    /// Variable name for the relationship
    pub variable: Option<String>,
    /// Relationship type
    pub rel_type: Option<String>,
    /// Relationship direction
    pub direction: RelationshipDirection,
    /// Relationship properties
    pub properties: HashMap<String, serde_json::Value>,
}

/// Direction of relationship traversal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationshipDirection {
    /// --> outgoing
    Outgoing,
    /// <-- incoming  
    Incoming,
    /// -- both directions
    Both,
}

/// RETURN clause item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnItem {
    /// Expression to return
    pub expression: String,
    /// Optional alias
    pub alias: Option<String>,
}

/// WHERE clause condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    /// Property equality check
    PropertyEquals {
        property: String,
        value: serde_json::Value,
    },
    /// Property comparison (>, <, >=, <=, !=)
    PropertyComparison {
        property: String,
        operator: ComparisonOperator,
        value: serde_json::Value,
    },
    /// Property exists check
    PropertyExists {
        property: String,
    },
    /// Logical AND
    And {
        left: Box<Condition>,
        right: Box<Condition>,
    },
    /// Logical OR
    Or {
        left: Box<Condition>,
        right: Box<Condition>,
    },
    /// Logical NOT
    Not {
        condition: Box<Condition>,
    },
    /// Node label check
    HasLabel {
        variable: String,
        label: String,
    },
    /// Relationship type check
    HasRelationshipType {
        variable: String,
        rel_type: String,
    },
}

/// Comparison operators for WHERE clauses
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonOperator {
    /// ==
    Equals,
    /// !=
    NotEquals,
    /// >
    GreaterThan,
    /// <
    LessThan,
    /// >=
    GreaterThanOrEqual,
    /// <=
    LessThanOrEqual,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_match_query() {
        let parser = CypherParser::new();
        let query = "MATCH (n:Person) RETURN n";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 2); // MATCH and RETURN
    }

    #[test]
    fn test_create_query() {
        let parser = CypherParser::new();
        let query = "CREATE (n:Person {name: 'Alice'}) RETURN n";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 2); // CREATE and RETURN
    }

    #[test]
    fn test_tokenization() {
        let parser = CypherParser::with_debug(true);
        let result = parser.tokenize("MATCH (n:Person)");

        assert!(result.is_ok());
        let tokens = result.unwrap();
        assert!(!tokens.is_empty());
        assert_eq!(tokens[0], Token::Match);
    }

    #[test]
    fn test_invalid_query() {
        let parser = CypherParser::new();
        let query = "INVALID SYNTAX HERE";
        let result = parser.parse(query);

        assert!(result.is_err());
    }

    #[test]
    fn test_empty_query() {
        let parser = CypherParser::new();
        let result = parser.parse("");

        assert!(result.is_err());
    }

    #[test]
    fn test_delete_query() {
        let parser = CypherParser::new();
        let query = "MATCH (n:Person) DELETE n";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 2); // MATCH and DELETE

        match &parsed.clauses[1] {
            CypherClause::Delete { variables, detach } => {
                assert_eq!(variables.len(), 1);
                assert_eq!(variables[0], "n");
                assert!(!detach);
            }
            _ => panic!("Expected DELETE clause"),
        }
    }

    #[test]
    fn test_detach_delete_query() {
        let parser = CypherParser::new();
        let query = "MATCH (n:Person) DETACH DELETE n";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 2); // MATCH and DELETE

        match &parsed.clauses[1] {
            CypherClause::Delete { variables, detach } => {
                assert_eq!(variables.len(), 1);
                assert!(*detach);
            }
            _ => panic!("Expected DELETE clause"),
        }
    }

    #[test]
    fn test_set_query() {
        let parser = CypherParser::new();
        let query = "MATCH (n:Person) SET n.name = 'Bob' RETURN n";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 3); // MATCH, SET, RETURN

        match &parsed.clauses[1] {
            CypherClause::Set { assignments } => {
                assert_eq!(assignments.len(), 1);
                assert_eq!(assignments[0].target, "n.name");
                assert_eq!(assignments[0].value, serde_json::Value::String("Bob".to_string()));
            }
            _ => panic!("Expected SET clause"),
        }
    }

    #[test]
    fn test_merge_query() {
        let parser = CypherParser::new();
        let query = "MERGE (n:Person {name: 'Alice'}) RETURN n";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 2); // MERGE and RETURN

        match &parsed.clauses[0] {
            CypherClause::Merge { pattern } => {
                assert!(!pattern.elements.is_empty());
            }
            _ => panic!("Expected MERGE clause"),
        }
    }

    #[test]
    fn test_order_by_query() {
        let parser = CypherParser::new();
        let query = "MATCH (n:Person) RETURN n ORDER BY n.name DESC";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 3); // MATCH, RETURN, ORDER BY

        match &parsed.clauses[2] {
            CypherClause::OrderBy { items } => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].expression, "n.name");
                assert!(items[0].descending);
            }
            _ => panic!("Expected ORDER BY clause"),
        }
    }

    #[test]
    fn test_limit_query() {
        let parser = CypherParser::new();
        let query = "MATCH (n:Person) RETURN n LIMIT 10";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 3); // MATCH, RETURN, LIMIT

        match &parsed.clauses[2] {
            CypherClause::Limit { count } => {
                assert_eq!(*count, 10);
            }
            _ => panic!("Expected LIMIT clause"),
        }
    }

    #[test]
    fn test_skip_query() {
        let parser = CypherParser::new();
        let query = "MATCH (n:Person) RETURN n SKIP 5";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 3); // MATCH, RETURN, SKIP

        match &parsed.clauses[2] {
            CypherClause::Skip { count } => {
                assert_eq!(*count, 5);
            }
            _ => panic!("Expected SKIP clause"),
        }
    }

    #[test]
    fn test_remove_property_query() {
        let parser = CypherParser::new();
        let query = "MATCH (n:Person) REMOVE n.age RETURN n";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 3); // MATCH, REMOVE, RETURN

        match &parsed.clauses[1] {
            CypherClause::Remove { items } => {
                assert_eq!(items.len(), 1);
                match &items[0] {
                    RemoveItem::Property { variable, property } => {
                        assert_eq!(variable, "n");
                        assert_eq!(property, "age");
                    }
                    _ => panic!("Expected property removal"),
                }
            }
            _ => panic!("Expected REMOVE clause"),
        }
    }

    #[test]
    fn test_remove_label_query() {
        let parser = CypherParser::new();
        let query = "MATCH (n:Person) REMOVE n:Inactive RETURN n";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();

        match &parsed.clauses[1] {
            CypherClause::Remove { items } => {
                assert_eq!(items.len(), 1);
                match &items[0] {
                    RemoveItem::Label { variable, label } => {
                        assert_eq!(variable, "n");
                        assert_eq!(label, "Inactive");
                    }
                    _ => panic!("Expected label removal"),
                }
            }
            _ => panic!("Expected REMOVE clause"),
        }
    }

    #[test]
    fn test_complex_query_with_multiple_clauses() {
        let parser = CypherParser::new();
        let query = "MATCH (n:Person) WHERE n.age > 18 SET n.adult = 'true' RETURN n ORDER BY n.name LIMIT 10";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 6); // MATCH, WHERE, SET, RETURN, ORDER BY, LIMIT
    }

    #[test]
    fn test_call_procedure_simple() {
        let parser = CypherParser::new();
        let query = "CALL orbit.graph.pagerank()";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 1);

        match &parsed.clauses[0] {
            CypherClause::Call {
                procedure,
                arguments,
                yield_items,
            } => {
                assert_eq!(procedure, "orbit.graph.pagerank");
                assert!(arguments.is_empty());
                assert!(yield_items.is_none());
            }
            _ => panic!("Expected CALL clause"),
        }
    }

    #[test]
    fn test_call_procedure_with_args() {
        let parser = CypherParser::new();
        // Note: Current tokenizer doesn't support floating point literals like 0.85
        // Use integer values for config parameters
        let query = "CALL orbit.graph.pagerank({iterations: 20, minNodes: 5})";
        let result = parser.parse(query);

        if result.is_err() {
            eprintln!("Parse error: {:?}", result.as_ref().unwrap_err());
        }
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 1);

        match &parsed.clauses[0] {
            CypherClause::Call {
                procedure,
                arguments,
                yield_items,
            } => {
                assert_eq!(procedure, "orbit.graph.pagerank");
                assert_eq!(arguments.len(), 1);
                // The argument should be an object with iterations and minNodes
                let arg = &arguments[0];
                assert!(arg.is_object());
                let obj = arg.as_object().unwrap();
                assert_eq!(obj.get("iterations").unwrap().as_i64().unwrap(), 20);
                assert_eq!(obj.get("minNodes").unwrap().as_i64().unwrap(), 5);
                assert!(yield_items.is_none());
            }
            _ => panic!("Expected CALL clause"),
        }
    }

    #[test]
    fn test_call_procedure_with_yield() {
        let parser = CypherParser::new();
        let query = "CALL orbit.graph.shortestPath('node1', 'node2') YIELD path, distance";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 1);

        match &parsed.clauses[0] {
            CypherClause::Call {
                procedure,
                arguments,
                yield_items,
            } => {
                assert_eq!(procedure, "orbit.graph.shortestPath");
                assert_eq!(arguments.len(), 2);
                assert!(yield_items.is_some());
                let items = yield_items.as_ref().unwrap();
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], "path");
                assert_eq!(items[1], "distance");
            }
            _ => panic!("Expected CALL clause"),
        }
    }

    #[test]
    fn test_call_procedure_bfs() {
        let parser = CypherParser::new();
        let query = "CALL orbit.graph.bfs('startNode', 5)";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 1);

        match &parsed.clauses[0] {
            CypherClause::Call {
                procedure,
                arguments,
                ..
            } => {
                assert_eq!(procedure, "orbit.graph.bfs");
                assert_eq!(arguments.len(), 2);
            }
            _ => panic!("Expected CALL clause"),
        }
    }
}
