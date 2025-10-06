//! AQL (ArangoDB Query Language) parser for multi-model database queries
//!
//! This module provides comprehensive AQL parsing capabilities supporting
//! document queries, graph traversals, aggregations, and complex operations.

use crate::aql::data_model::AqlValue;
use crate::error::{ProtocolError, ProtocolResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, instrument, warn};

/// AQL query parser
#[derive(Debug, Default)]
pub struct AqlParser {
    /// Enable debug output for parsing
    debug_mode: bool,
}

impl AqlParser {
    /// Create a new AQL parser
    pub fn new() -> Self {
        Self { debug_mode: false }
    }

    /// Create a new AQL parser with debug mode enabled
    pub fn with_debug(debug_mode: bool) -> Self {
        Self { debug_mode }
    }

    /// Parse an AQL query string into an AST
    #[instrument(skip(self, aql), fields(query_length = aql.len()))]
    pub fn parse(&self, aql: &str) -> ProtocolResult<AqlQuery> {
        let trimmed = aql.trim();
        if trimmed.is_empty() {
            return Err(ProtocolError::ParseError("Empty query".to_string()));
        }

        debug!(query = aql, "Parsing AQL query");

        // Tokenize the query
        let tokens = self.tokenize(trimmed)?;
        let query = self.parse_tokens(tokens)?;

        debug!(query = ?query, "Parsed AQL query successfully");
        Ok(query)
    }

    /// Tokenize the AQL query string
    fn tokenize(&self, query: &str) -> ProtocolResult<Vec<AqlToken>> {
        let mut tokens = Vec::new();
        let mut current_token = String::new();
        let mut in_quotes = false;
        let mut quote_char = '\0';
        let mut chars = query.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '"' | '\'' if !in_quotes => {
                    if !current_token.is_empty() {
                        tokens.push(self.classify_token(&current_token));
                        current_token.clear();
                    }
                    in_quotes = true;
                    quote_char = ch;
                }
                '"' | '\'' if in_quotes && ch == quote_char => {
                    tokens.push(AqlToken::String(current_token.clone()));
                    current_token.clear();
                    in_quotes = false;
                }
                ' ' | '\t' | '\n' | '\r' if !in_quotes => {
                    if !current_token.is_empty() {
                        tokens.push(self.classify_token(&current_token));
                        current_token.clear();
                    }
                }
                '(' if !in_quotes => {
                    if !current_token.is_empty() {
                        tokens.push(self.classify_token(&current_token));
                        current_token.clear();
                    }
                    tokens.push(AqlToken::LeftParen);
                }
                ')' if !in_quotes => {
                    if !current_token.is_empty() {
                        tokens.push(self.classify_token(&current_token));
                        current_token.clear();
                    }
                    tokens.push(AqlToken::RightParen);
                }
                '{' if !in_quotes => {
                    if !current_token.is_empty() {
                        tokens.push(self.classify_token(&current_token));
                        current_token.clear();
                    }
                    tokens.push(AqlToken::LeftBrace);
                }
                '}' if !in_quotes => {
                    if !current_token.is_empty() {
                        tokens.push(self.classify_token(&current_token));
                        current_token.clear();
                    }
                    tokens.push(AqlToken::RightBrace);
                }
                '[' if !in_quotes => {
                    if !current_token.is_empty() {
                        tokens.push(self.classify_token(&current_token));
                        current_token.clear();
                    }
                    tokens.push(AqlToken::LeftBracket);
                }
                ']' if !in_quotes => {
                    if !current_token.is_empty() {
                        tokens.push(self.classify_token(&current_token));
                        current_token.clear();
                    }
                    tokens.push(AqlToken::RightBracket);
                }
                ',' if !in_quotes => {
                    if !current_token.is_empty() {
                        tokens.push(self.classify_token(&current_token));
                        current_token.clear();
                    }
                    tokens.push(AqlToken::Comma);
                }
                ':' if !in_quotes => {
                    if !current_token.is_empty() {
                        tokens.push(self.classify_token(&current_token));
                        current_token.clear();
                    }
                    tokens.push(AqlToken::Colon);
                }
                '=' if !in_quotes => {
                    if !current_token.is_empty() {
                        tokens.push(self.classify_token(&current_token));
                        current_token.clear();
                    }
                    // Check for == operator
                    if chars.peek() == Some(&'=') {
                        chars.next(); // consume second =
                        tokens.push(AqlToken::Equals);
                    } else {
                        tokens.push(AqlToken::Assignment);
                    }
                }
                '!' if !in_quotes => {
                    if chars.peek() == Some(&'=') {
                        if !current_token.is_empty() {
                            tokens.push(self.classify_token(&current_token));
                            current_token.clear();
                        }
                        chars.next(); // consume =
                        tokens.push(AqlToken::NotEquals);
                    } else {
                        current_token.push(ch);
                    }
                }
                '<' if !in_quotes => {
                    if !current_token.is_empty() {
                        tokens.push(self.classify_token(&current_token));
                        current_token.clear();
                    }
                    if chars.peek() == Some(&'=') {
                        chars.next(); // consume =
                        tokens.push(AqlToken::LessOrEqual);
                    } else {
                        tokens.push(AqlToken::Less);
                    }
                }
                '>' if !in_quotes => {
                    if !current_token.is_empty() {
                        tokens.push(self.classify_token(&current_token));
                        current_token.clear();
                    }
                    if chars.peek() == Some(&'=') {
                        chars.next(); // consume =
                        tokens.push(AqlToken::GreaterOrEqual);
                    } else {
                        tokens.push(AqlToken::Greater);
                    }
                }
                '.' if !in_quotes => {
                    if chars.peek() == Some(&'.') {
                        if !current_token.is_empty() {
                            tokens.push(self.classify_token(&current_token));
                            current_token.clear();
                        }
                        chars.next(); // consume second .
                        tokens.push(AqlToken::Range);
                    } else {
                        if !current_token.is_empty() {
                            tokens.push(self.classify_token(&current_token));
                            current_token.clear();
                        }
                        tokens.push(AqlToken::Dot);
                    }
                }
                _ => {
                    current_token.push(ch);
                }
            }
        }

        if !current_token.is_empty() {
            if in_quotes {
                return Err(ProtocolError::ParseError(
                    "Unterminated string literal".to_string(),
                ));
            }
            tokens.push(self.classify_token(&current_token));
        }

        if self.debug_mode {
            debug!(tokens = ?tokens, "Tokenized AQL query");
        }

        Ok(tokens)
    }

    /// Classify a token based on its content
    fn classify_token(&self, token: &str) -> AqlToken {
        match token.to_uppercase().as_str() {
            "FOR" => AqlToken::For,
            "IN" => AqlToken::In,
            "RETURN" => AqlToken::Return,
            "FILTER" => AqlToken::Filter,
            "LET" => AqlToken::Let,
            "COLLECT" => AqlToken::Collect,
            "SORT" => AqlToken::Sort,
            "LIMIT" => AqlToken::Limit,
            "INSERT" => AqlToken::Insert,
            "UPDATE" => AqlToken::Update,
            "REPLACE" => AqlToken::Replace,
            "REMOVE" => AqlToken::Remove,
            "UPSERT" => AqlToken::Upsert,
            "WITH" => AqlToken::With,
            "INTO" => AqlToken::Into,
            "AND" => AqlToken::And,
            "OR" => AqlToken::Or,
            "NOT" => AqlToken::Not,
            "NULL" => AqlToken::Null,
            "TRUE" => AqlToken::Bool(true),
            "FALSE" => AqlToken::Bool(false),
            "ASC" | "ASCENDING" => AqlToken::Asc,
            "DESC" | "DESCENDING" => AqlToken::Desc,
            "DISTINCT" => AqlToken::Distinct,
            "AGGREGATE" => AqlToken::Aggregate,
            "OUTBOUND" => AqlToken::Outbound,
            "INBOUND" => AqlToken::Inbound,
            "ANY" => AqlToken::Any,
            "GRAPH" => AqlToken::Graph,
            "SHORTEST_PATH" => AqlToken::ShortestPath,
            "K_SHORTEST_PATHS" => AqlToken::KShortestPaths,
            _ => {
                // Check if it's a number
                if let Ok(int_val) = token.parse::<i64>() {
                    AqlToken::Number(serde_json::Number::from(int_val))
                } else if let Ok(float_val) = token.parse::<f64>() {
                    if let Some(num) = serde_json::Number::from_f64(float_val) {
                        AqlToken::Number(num)
                    } else {
                        AqlToken::Identifier(token.to_string())
                    }
                } else {
                    AqlToken::Identifier(token.to_string())
                }
            }
        }
    }

    /// Parse tokens into an AQL query AST
    fn parse_tokens(&self, tokens: Vec<AqlToken>) -> ProtocolResult<AqlQuery> {
        let mut parser = AqlTokenParser::new(tokens);
        parser.parse_query()
    }
}

/// AQL token types
#[derive(Debug, Clone, PartialEq)]
enum AqlToken {
    // Keywords
    For,
    In,
    Return,
    Filter,
    Let,
    Collect,
    Sort,
    Limit,
    Insert,
    Update,
    Replace,
    Remove,
    Upsert,
    With,
    Into,
    And,
    Or,
    Not,
    Null,
    Bool(bool),
    Asc,
    Desc,
    Distinct,
    Aggregate,

    // Graph traversal keywords
    Outbound,
    Inbound,
    Any,
    Graph,
    ShortestPath,
    KShortestPaths,

    // Literals
    Identifier(String),
    String(String),
    Number(serde_json::Number),

    // Operators
    Assignment,
    Equals,
    NotEquals,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    Range,

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
}

/// Token parser for building AQL AST
struct AqlTokenParser {
    tokens: Vec<AqlToken>,
    position: usize,
}

impl AqlTokenParser {
    fn new(tokens: Vec<AqlToken>) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    fn current_token(&self) -> Option<&AqlToken> {
        self.tokens.get(self.position)
    }

    fn advance(&mut self) -> Option<&AqlToken> {
        self.position += 1;
        self.current_token()
    }

    fn parse_query(&mut self) -> ProtocolResult<AqlQuery> {
        let mut clauses = Vec::new();

        while self.position < self.tokens.len() {
            match self.current_token() {
                Some(AqlToken::For) => {
                    clauses.push(self.parse_for_clause()?);
                }
                Some(AqlToken::Let) => {
                    clauses.push(self.parse_let_clause()?);
                }
                Some(AqlToken::Filter) => {
                    clauses.push(self.parse_filter_clause()?);
                }
                Some(AqlToken::Collect) => {
                    clauses.push(self.parse_collect_clause()?);
                }
                Some(AqlToken::Sort) => {
                    clauses.push(self.parse_sort_clause()?);
                }
                Some(AqlToken::Limit) => {
                    clauses.push(self.parse_limit_clause()?);
                }
                Some(AqlToken::Return) => {
                    clauses.push(self.parse_return_clause()?);
                }
                Some(AqlToken::Insert) => {
                    clauses.push(self.parse_insert_clause()?);
                }
                Some(AqlToken::Update) => {
                    clauses.push(self.parse_update_clause()?);
                }
                Some(AqlToken::Remove) => {
                    clauses.push(self.parse_remove_clause()?);
                }
                Some(token) => {
                    return Err(ProtocolError::ParseError(format!(
                        "Unexpected token: {:?}",
                        token
                    )));
                }
                None => break,
            }
        }

        if clauses.is_empty() {
            return Err(ProtocolError::ParseError("Empty query".to_string()));
        }

        Ok(AqlQuery { clauses })
    }

    fn parse_for_clause(&mut self) -> ProtocolResult<AqlClause> {
        self.advance(); // consume FOR

        // Parse variable name
        let variable = match self.current_token() {
            Some(AqlToken::Identifier(name)) => {
                let name = name.clone();
                self.advance();
                name
            }
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected variable name in FOR clause".to_string(),
                ))
            }
        };

        // Check for traversal syntax (vertex, edge, path)
        let mut edge_var = None;
        let mut path_var = None;

        if matches!(self.current_token(), Some(AqlToken::Comma)) {
            self.advance(); // consume comma
            edge_var = match self.current_token() {
                Some(AqlToken::Identifier(name)) => {
                    let name = name.clone();
                    self.advance();
                    Some(name)
                }
                _ => {
                    return Err(ProtocolError::ParseError(
                        "Expected edge variable".to_string(),
                    ))
                }
            };

            if matches!(self.current_token(), Some(AqlToken::Comma)) {
                self.advance(); // consume comma
                path_var = match self.current_token() {
                    Some(AqlToken::Identifier(name)) => {
                        let name = name.clone();
                        self.advance();
                        Some(name)
                    }
                    _ => {
                        return Err(ProtocolError::ParseError(
                            "Expected path variable".to_string(),
                        ))
                    }
                };
            }
        }

        // Expect IN keyword
        match self.current_token() {
            Some(AqlToken::In) => self.advance(),
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected IN keyword in FOR clause".to_string(),
                ))
            }
        };

        // Check for traversal depth range (e.g., 1..3)
        let mut min_depth = None;
        let mut max_depth = None;

        if let Some(AqlToken::Number(n)) = self.current_token() {
            min_depth = Some(n.as_u64().unwrap_or(1) as u32);
            self.advance();

            if matches!(self.current_token(), Some(AqlToken::Range)) {
                self.advance(); // consume ..
                if let Some(AqlToken::Number(n)) = self.current_token() {
                    max_depth = Some(n.as_u64().unwrap_or(10) as u32);
                    self.advance();
                }
            }
        }

        // Check for direction keywords
        let direction = match self.current_token() {
            Some(AqlToken::Outbound) => {
                self.advance();
                Some(TraversalDirection::Outbound)
            }
            Some(AqlToken::Inbound) => {
                self.advance();
                Some(TraversalDirection::Inbound)
            }
            Some(AqlToken::Any) => {
                self.advance();
                Some(TraversalDirection::Any)
            }
            _ => None,
        };

        // Parse the data source
        let data_source = match self.current_token() {
            Some(AqlToken::Identifier(name)) => {
                let name = name.clone();
                self.advance();
                name
            }
            Some(AqlToken::String(name)) => {
                let name = name.clone();
                self.advance();
                name
            }
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected data source in FOR clause".to_string(),
                ))
            }
        };

        // Check for GRAPH keyword for graph traversals
        let graph_name = if matches!(self.current_token(), Some(AqlToken::Graph)) {
            self.advance(); // consume GRAPH
            match self.current_token() {
                Some(AqlToken::Identifier(name)) | Some(AqlToken::String(name)) => {
                    let name = name.clone();
                    self.advance();
                    Some(name)
                }
                _ => {
                    return Err(ProtocolError::ParseError(
                        "Expected graph name after GRAPH keyword".to_string(),
                    ))
                }
            }
        } else {
            None
        };

        // Determine if this is a traversal or simple iteration
        if direction.is_some() || graph_name.is_some() || edge_var.is_some() {
            Ok(AqlClause::ForTraversal {
                vertex_var: variable,
                edge_var,
                path_var,
                min_depth,
                max_depth,
                direction: direction.unwrap_or(TraversalDirection::Any),
                start_vertex: data_source,
                graph_name,
            })
        } else {
            Ok(AqlClause::For {
                variable,
                data_source,
            })
        }
    }

    fn parse_let_clause(&mut self) -> ProtocolResult<AqlClause> {
        self.advance(); // consume LET

        let variable = match self.current_token() {
            Some(AqlToken::Identifier(name)) => {
                let name = name.clone();
                self.advance();
                name
            }
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected variable name in LET clause".to_string(),
                ))
            }
        };

        // Expect assignment operator
        match self.current_token() {
            Some(AqlToken::Assignment) => self.advance(),
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected = in LET clause".to_string(),
                ))
            }
        };

        // Parse expression (simplified)
        let expression = self.parse_expression()?;

        Ok(AqlClause::Let {
            variable,
            expression,
        })
    }

    fn parse_filter_clause(&mut self) -> ProtocolResult<AqlClause> {
        self.advance(); // consume FILTER
        let condition = self.parse_condition()?;
        Ok(AqlClause::Filter { condition })
    }

    fn parse_collect_clause(&mut self) -> ProtocolResult<AqlClause> {
        self.advance(); // consume COLLECT

        let mut groups = Vec::new();

        while let Some(AqlToken::Identifier(name)) = self.current_token() {
            let variable = {
                let name = name.clone();
                self.advance();
                name
            };

            // Optional assignment
            let expression = if matches!(self.current_token(), Some(AqlToken::Assignment)) {
                self.advance();
                Some(self.parse_expression()?)
            } else {
                None
            };

            groups.push(CollectGroup {
                variable,
                expression,
            });

            if matches!(self.current_token(), Some(AqlToken::Comma)) {
                self.advance();
            } else {
                break;
            }
        }

        Ok(AqlClause::Collect { groups })
    }

    fn parse_sort_clause(&mut self) -> ProtocolResult<AqlClause> {
        self.advance(); // consume SORT

        let mut items = Vec::new();

        loop {
            let expression = self.parse_expression()?;
            let direction = match self.current_token() {
                Some(AqlToken::Asc) => {
                    self.advance();
                    SortDirection::Asc
                }
                Some(AqlToken::Desc) => {
                    self.advance();
                    SortDirection::Desc
                }
                _ => SortDirection::Asc,
            };

            items.push(SortItem {
                expression,
                direction,
            });

            if matches!(self.current_token(), Some(AqlToken::Comma)) {
                self.advance();
            } else {
                break;
            }
        }

        Ok(AqlClause::Sort { items })
    }

    fn parse_limit_clause(&mut self) -> ProtocolResult<AqlClause> {
        self.advance(); // consume LIMIT

        let offset = match self.current_token() {
            Some(AqlToken::Number(n)) => {
                let val = n.as_u64().unwrap_or(0);
                self.advance();

                // Check if there's a comma for offset, count syntax
                if matches!(self.current_token(), Some(AqlToken::Comma)) {
                    self.advance();
                    Some(val as u32)
                } else {
                    // Single number means count only
                    return Ok(AqlClause::Limit {
                        offset: None,
                        count: val as u32,
                    });
                }
            }
            _ => None,
        };

        let count = match self.current_token() {
            Some(AqlToken::Number(n)) => {
                let val = n.as_u64().unwrap_or(0) as u32;
                self.advance();
                val
            }
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected count in LIMIT clause".to_string(),
                ))
            }
        };

        Ok(AqlClause::Limit { offset, count })
    }

    fn parse_return_clause(&mut self) -> ProtocolResult<AqlClause> {
        self.advance(); // consume RETURN

        let distinct = if matches!(self.current_token(), Some(AqlToken::Distinct)) {
            self.advance();
            true
        } else {
            false
        };

        let expression = self.parse_expression()?;

        Ok(AqlClause::Return {
            distinct,
            expression,
        })
    }

    fn parse_insert_clause(&mut self) -> ProtocolResult<AqlClause> {
        self.advance(); // consume INSERT

        let document = self.parse_expression()?;

        // Expect INTO
        match self.current_token() {
            Some(AqlToken::Into) => self.advance(),
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected INTO in INSERT clause".to_string(),
                ))
            }
        };

        let collection = match self.current_token() {
            Some(AqlToken::Identifier(name)) => {
                let name = name.clone();
                self.advance();
                name
            }
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected collection name in INSERT clause".to_string(),
                ))
            }
        };

        Ok(AqlClause::Insert {
            document,
            collection,
        })
    }

    fn parse_update_clause(&mut self) -> ProtocolResult<AqlClause> {
        self.advance(); // consume UPDATE

        let key = self.parse_expression()?;

        // Expect WITH
        match self.current_token() {
            Some(AqlToken::With) => self.advance(),
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected WITH in UPDATE clause".to_string(),
                ))
            }
        };

        let document = self.parse_expression()?;

        // Expect IN
        match self.current_token() {
            Some(AqlToken::In) => self.advance(),
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected IN in UPDATE clause".to_string(),
                ))
            }
        };

        let collection = match self.current_token() {
            Some(AqlToken::Identifier(name)) => {
                let name = name.clone();
                self.advance();
                name
            }
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected collection name in UPDATE clause".to_string(),
                ))
            }
        };

        Ok(AqlClause::Update {
            key,
            document,
            collection,
        })
    }

    fn parse_remove_clause(&mut self) -> ProtocolResult<AqlClause> {
        self.advance(); // consume REMOVE

        let key = self.parse_expression()?;

        // Expect IN
        match self.current_token() {
            Some(AqlToken::In) => self.advance(),
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected IN in REMOVE clause".to_string(),
                ))
            }
        };

        let collection = match self.current_token() {
            Some(AqlToken::Identifier(name)) => {
                let name = name.clone();
                self.advance();
                name
            }
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected collection name in REMOVE clause".to_string(),
                ))
            }
        };

        Ok(AqlClause::Remove { key, collection })
    }

    fn parse_expression(&mut self) -> ProtocolResult<AqlExpression> {
        // Simplified expression parsing
        match self.current_token() {
            Some(AqlToken::Identifier(name)) => {
                let name = name.clone();
                self.advance();

                // Check for property access
                if matches!(self.current_token(), Some(AqlToken::Dot)) {
                    self.advance(); // consume .
                    let property = match self.current_token() {
                        Some(AqlToken::Identifier(prop)) => {
                            let prop = prop.clone();
                            self.advance();
                            prop
                        }
                        _ => {
                            return Err(ProtocolError::ParseError(
                                "Expected property name after .".to_string(),
                            ))
                        }
                    };
                    Ok(AqlExpression::PropertyAccess {
                        object: name,
                        property,
                    })
                } else {
                    Ok(AqlExpression::Variable(name))
                }
            }
            Some(AqlToken::String(s)) => {
                let s = s.clone();
                self.advance();
                Ok(AqlExpression::Literal(AqlValue::String(s)))
            }
            Some(AqlToken::Number(n)) => {
                let n = n.clone();
                self.advance();
                Ok(AqlExpression::Literal(AqlValue::Number(n)))
            }
            Some(AqlToken::Bool(b)) => {
                let b = *b;
                self.advance();
                Ok(AqlExpression::Literal(AqlValue::Bool(b)))
            }
            Some(AqlToken::Null) => {
                self.advance();
                Ok(AqlExpression::Literal(AqlValue::Null))
            }
            Some(AqlToken::LeftBrace) => self.parse_object_expression(),
            Some(AqlToken::LeftBracket) => self.parse_array_expression(),
            _ => Err(ProtocolError::ParseError("Expected expression".to_string())),
        }
    }

    fn parse_object_expression(&mut self) -> ProtocolResult<AqlExpression> {
        self.advance(); // consume {

        let mut properties = HashMap::new();

        while !matches!(self.current_token(), Some(AqlToken::RightBrace)) {
            let key = match self.current_token() {
                Some(AqlToken::Identifier(k)) | Some(AqlToken::String(k)) => {
                    let k = k.clone();
                    self.advance();
                    k
                }
                _ => {
                    return Err(ProtocolError::ParseError(
                        "Expected property key".to_string(),
                    ))
                }
            };

            // Expect colon
            match self.current_token() {
                Some(AqlToken::Colon) => self.advance(),
                _ => {
                    return Err(ProtocolError::ParseError(
                        "Expected : in object literal".to_string(),
                    ))
                }
            };

            let value = self.parse_expression()?;
            properties.insert(key, value);

            if matches!(self.current_token(), Some(AqlToken::Comma)) {
                self.advance();
            } else {
                break;
            }
        }

        // Consume closing brace
        match self.current_token() {
            Some(AqlToken::RightBrace) => self.advance(),
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected } to close object".to_string(),
                ))
            }
        };

        Ok(AqlExpression::Object(properties))
    }

    fn parse_array_expression(&mut self) -> ProtocolResult<AqlExpression> {
        self.advance(); // consume [

        let mut elements = Vec::new();

        while !matches!(self.current_token(), Some(AqlToken::RightBracket)) {
            let element = self.parse_expression()?;
            elements.push(element);

            if matches!(self.current_token(), Some(AqlToken::Comma)) {
                self.advance();
            } else {
                break;
            }
        }

        // Consume closing bracket
        match self.current_token() {
            Some(AqlToken::RightBracket) => self.advance(),
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected ] to close array".to_string(),
                ))
            }
        };

        Ok(AqlExpression::Array(elements))
    }

    fn parse_condition(&mut self) -> ProtocolResult<AqlCondition> {
        let left = self.parse_expression()?;

        let operator = match self.current_token() {
            Some(AqlToken::Equals) => {
                self.advance();
                ComparisonOperator::Equals
            }
            Some(AqlToken::NotEquals) => {
                self.advance();
                ComparisonOperator::NotEquals
            }
            Some(AqlToken::Less) => {
                self.advance();
                ComparisonOperator::Less
            }
            Some(AqlToken::LessOrEqual) => {
                self.advance();
                ComparisonOperator::LessOrEqual
            }
            Some(AqlToken::Greater) => {
                self.advance();
                ComparisonOperator::Greater
            }
            Some(AqlToken::GreaterOrEqual) => {
                self.advance();
                ComparisonOperator::GreaterOrEqual
            }
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected comparison operator".to_string(),
                ))
            }
        };

        let right = self.parse_expression()?;

        Ok(AqlCondition::Comparison {
            left,
            operator,
            right,
        })
    }
}

/// Complete parsed AQL query with clauses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AqlQuery {
    /// Query clauses in order
    pub clauses: Vec<AqlClause>,
}

/// AQL query clause types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AqlClause {
    /// FOR clause for iteration
    For {
        variable: String,
        data_source: String,
    },
    /// FOR clause for graph traversal
    ForTraversal {
        vertex_var: String,
        edge_var: Option<String>,
        path_var: Option<String>,
        min_depth: Option<u32>,
        max_depth: Option<u32>,
        direction: TraversalDirection,
        start_vertex: String,
        graph_name: Option<String>,
    },
    /// LET clause for variable assignment
    Let {
        variable: String,
        expression: AqlExpression,
    },
    /// FILTER clause for filtering
    Filter { condition: AqlCondition },
    /// COLLECT clause for grouping
    Collect { groups: Vec<CollectGroup> },
    /// SORT clause for ordering
    Sort { items: Vec<SortItem> },
    /// LIMIT clause for pagination
    Limit { offset: Option<u32>, count: u32 },
    /// RETURN clause for result projection
    Return {
        distinct: bool,
        expression: AqlExpression,
    },
    /// INSERT clause for creating documents
    Insert {
        document: AqlExpression,
        collection: String,
    },
    /// UPDATE clause for modifying documents
    Update {
        key: AqlExpression,
        document: AqlExpression,
        collection: String,
    },
    /// REMOVE clause for deleting documents
    Remove {
        key: AqlExpression,
        collection: String,
    },
}

/// AQL expression types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AqlExpression {
    /// Variable reference
    Variable(String),
    /// Literal value
    Literal(AqlValue),
    /// Property access (obj.prop)
    PropertyAccess { object: String, property: String },
    /// Object literal
    Object(HashMap<String, AqlExpression>),
    /// Array literal
    Array(Vec<AqlExpression>),
    // TODO: Add more expression types (function calls, arithmetic, etc.)
}

/// AQL condition types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AqlCondition {
    /// Comparison condition
    Comparison {
        left: AqlExpression,
        operator: ComparisonOperator,
        right: AqlExpression,
    },
    // TODO: Add logical conditions (AND, OR, NOT)
}

/// Comparison operators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    Equals,
    NotEquals,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

/// Traversal direction for graph operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TraversalDirection {
    Outbound,
    Inbound,
    Any,
}

/// COLLECT group specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectGroup {
    pub variable: String,
    pub expression: Option<AqlExpression>,
}

/// SORT item specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortItem {
    pub expression: AqlExpression,
    pub direction: SortDirection,
}

/// Sort direction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_for_query() {
        let parser = AqlParser::new();
        let query = "FOR doc IN users RETURN doc";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 2); // FOR and RETURN
    }

    #[test]
    fn test_filter_query() {
        let parser = AqlParser::new();
        let query = "FOR doc IN users FILTER doc.age > 25 RETURN doc";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 3); // FOR, FILTER, and RETURN
    }

    #[test]
    fn test_graph_traversal_query() {
        let parser = AqlParser::new();
        let query =
            "FOR vertex, edge, path IN 1..3 OUTBOUND 'users/john' GRAPH 'social' RETURN vertex";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 2); // FOR_TRAVERSAL and RETURN
    }

    #[test]
    fn test_insert_query() {
        let parser = AqlParser::new();
        let query = "INSERT {name: 'Alice', age: 30} INTO users";
        let result = parser.parse(query);

        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 1); // INSERT
    }

    #[test]
    fn test_tokenization() {
        let parser = AqlParser::with_debug(true);
        let result = parser.tokenize("FOR doc IN users");

        assert!(result.is_ok());
        let tokens = result.unwrap();
        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_invalid_query() {
        let parser = AqlParser::new();
        let query = "INVALID SYNTAX HERE";
        let result = parser.parse(query);

        assert!(result.is_err());
    }

    #[test]
    fn test_empty_query() {
        let parser = AqlParser::new();
        let result = parser.parse("");

        assert!(result.is_err());
    }
}
