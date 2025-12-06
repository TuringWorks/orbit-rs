//! AQL (ArangoDB Query Language) parser for multi-model database queries
//!
//! This module provides comprehensive AQL parsing capabilities supporting
//! document queries, graph traversals, aggregations, and complex operations.
//!
//! ## References
//! - AQL Spec: `specifications/protocols/arangodb_aql_reference.md`
//! - ANTLR4 Grammar: <https://github.com/TuringWorks/grammars-v4/tree/master/aql>

use crate::protocols::aql::data_model::AqlValue;
use crate::protocols::error::{ProtocolError, ProtocolResult};
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

    /// Tokenize the AQL query string using decomposed tokenization strategy
    fn tokenize(&self, query: &str) -> ProtocolResult<Vec<AqlToken>> {
        let mut tokenizer = AqlTokenizer::new(query, self.debug_mode);
        tokenizer.tokenize()
    }

    /// Classify a token based on its content
    #[allow(dead_code)]
    fn classify_token(&self, token: &str) -> AqlToken {
        Self::classify_token_static(token)
    }

    /// Static token classification for use by tokenizer
    fn classify_token_static(token: &str) -> AqlToken {
        match token.to_uppercase().as_str() {
            // Core query keywords
            "FOR" => AqlToken::For,
            "IN" => AqlToken::In,
            "RETURN" => AqlToken::Return,
            "FILTER" => AqlToken::Filter,
            "LET" => AqlToken::Let,
            "COLLECT" => AqlToken::Collect,
            "SORT" => AqlToken::Sort,
            "LIMIT" => AqlToken::Limit,

            // Data modification keywords
            "INSERT" => AqlToken::Insert,
            "UPDATE" => AqlToken::Update,
            "REPLACE" => AqlToken::Replace,
            "REMOVE" => AqlToken::Remove,
            "UPSERT" => AqlToken::Upsert,
            "WITH" => AqlToken::With,
            "INTO" => AqlToken::Into,

            // Logical operators
            "AND" => AqlToken::And,
            "OR" => AqlToken::Or,
            "NOT" => AqlToken::Not,

            // Literals
            "NULL" => AqlToken::Null,
            "TRUE" => AqlToken::Bool(true),
            "FALSE" => AqlToken::Bool(false),

            // Sort order
            "ASC" | "ASCENDING" => AqlToken::Asc,
            "DESC" | "DESCENDING" => AqlToken::Desc,

            // Collection operations
            "DISTINCT" => AqlToken::Distinct,
            "AGGREGATE" => AqlToken::Aggregate,

            // Graph traversal keywords
            "OUTBOUND" => AqlToken::Outbound,
            "INBOUND" => AqlToken::Inbound,
            "ANY" => AqlToken::Any,
            "ALL" => AqlToken::All,
            "NONE" => AqlToken::None,
            "GRAPH" => AqlToken::Graph,
            "SHORTEST_PATH" => AqlToken::ShortestPath,
            "K_SHORTEST_PATHS" => AqlToken::KShortestPaths,
            "ALL_SHORTEST_PATHS" => AqlToken::AllShortestPaths,
            "K_PATHS" => AqlToken::KPaths,
            "PRUNE" => AqlToken::Prune,

            // Search and full-text
            "SEARCH" => AqlToken::Search,
            "LIKE" => AqlToken::Like,

            // Window functions
            "WINDOW" => AqlToken::Window,

            // Frame keywords
            "ROWS" => AqlToken::Rows,
            "RANGE" => AqlToken::RangeKeyword,
            "GROUPS" => AqlToken::Groups,
            "PRECEDING" => AqlToken::Preceding,
            "FOLLOWING" => AqlToken::Following,
            "UNBOUNDED" => AqlToken::Unbounded,
            "CURRENT" => AqlToken::Current,
            "ROW" => AqlToken::Row,
            "PARTITION" => AqlToken::Partition,
            "BY" => AqlToken::By,
            "OVER" => AqlToken::Over,

            // Window function names
            "ROW_NUMBER" => AqlToken::RowNumber,
            "RANK" => AqlToken::RankFunc,
            "DENSE_RANK" => AqlToken::DenseRank,
            "PERCENT_RANK" => AqlToken::PercentRank,
            "CUME_DIST" => AqlToken::CumeDist,
            "NTILE" => AqlToken::Ntile,
            "LAG" => AqlToken::LagFunc,
            "LEAD" => AqlToken::LeadFunc,
            "FIRST_VALUE" => AqlToken::FirstValue,
            "LAST_VALUE" => AqlToken::LastValue,
            "NTH_VALUE" => AqlToken::NthValue,

            // Options and configuration
            "OPTIONS" => AqlToken::Options,

            // Additional keywords
            "COUNT" => AqlToken::Count,
            "KEEP" => AqlToken::Keep,

            // Aggregate function names
            "SUM" => AqlToken::Sum,
            "AVG" => AqlToken::Avg,
            "MIN" => AqlToken::MinFunc,
            "MAX" => AqlToken::MaxFunc,
            "COUNT_DISTINCT" => AqlToken::CountDistinct,
            "COLLECT_ARRAY" | "PUSH" => AqlToken::CollectArray,
            "COLLECT_UNIQUE" | "UNIQUE" => AqlToken::CollectUnique,
            "STDDEV" | "STDDEV_POP" => AqlToken::Stddev,
            "VARIANCE" | "VARIANCE_POP" => AqlToken::VarianceFunc,
            "STDDEV_SAMPLE" => AqlToken::StddevSample,
            "VARIANCE_SAMPLE" => AqlToken::VarianceSample,

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
///
/// Represents all valid tokens in the AQL language, including
/// keywords, operators, literals, and symbols.
#[derive(Debug, Clone, PartialEq)]
enum AqlToken {
    // Core query keywords
    For,
    In,
    Return,
    Filter,
    Let,
    Collect,
    Sort,
    Limit,

    // Data modification keywords
    Insert,
    Update,
    Replace,
    Remove,
    Upsert,
    With,
    Into,

    // Logical operators
    And,
    Or,
    Not,

    // Literals
    Null,
    Bool(bool),

    // Sort order
    Asc,
    Desc,

    // Collection operations
    Distinct,
    Aggregate,
    Count,
    Keep,

    // Graph traversal keywords
    Outbound,
    Inbound,
    Any,
    All,
    None,
    Graph,
    ShortestPath,
    KShortestPaths,
    AllShortestPaths,
    KPaths,
    Prune,

    // Search and full-text
    Search,
    Like,

    // Window functions
    Window,

    // Frame keywords
    Rows,
    RangeKeyword,
    Groups,
    Preceding,
    Following,
    Unbounded,
    Current,
    Row,
    Partition,
    By,
    Over,

    // Window function names
    RowNumber,
    RankFunc,
    DenseRank,
    PercentRank,
    CumeDist,
    Ntile,
    LagFunc,
    LeadFunc,
    FirstValue,
    LastValue,
    NthValue,

    // Aggregate functions
    Sum,
    Avg,
    MinFunc,
    MaxFunc,
    CountDistinct,
    CollectArray,
    CollectUnique,
    Stddev,
    VarianceFunc,
    StddevSample,
    VarianceSample,

    // Options and configuration
    Options,

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
    Plus,
    Minus,
    Star,
    Slash,
    Percent,

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

/// Specialized tokenizer for AQL queries with reduced complexity
#[derive(Debug)]
struct AqlTokenizer {
    #[allow(dead_code)]
    query: String,
    debug_mode: bool,
    tokens: Vec<AqlToken>,
    current_token: String,
    position: usize,
    chars: Vec<char>,
    in_quotes: bool,
    quote_char: char,
}

impl AqlTokenizer {
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

    fn tokenize(&mut self) -> ProtocolResult<Vec<AqlToken>> {
        while self.position < self.chars.len() {
            let ch = self.chars[self.position];
            self.process_character(ch)?;
            self.position += 1;
        }

        self.finalize_tokenization()?;

        if self.debug_mode {
            debug!(tokens = ?self.tokens, "Tokenized AQL query");
        }

        Ok(self.tokens.clone())
    }

    fn process_character(&mut self, ch: char) -> ProtocolResult<()> {
        if self.in_quotes {
            self.handle_quoted_character(ch);
        } else {
            self.handle_unquoted_character(ch)?;
        }
        Ok(())
    }

    fn handle_quoted_character(&mut self, ch: char) {
        if ch == self.quote_char {
            self.tokens
                .push(AqlToken::String(self.current_token.clone()));
            self.current_token.clear();
            self.in_quotes = false;
            self.quote_char = '\0';
        } else {
            self.current_token.push(ch);
        }
    }

    fn handle_unquoted_character(&mut self, ch: char) -> ProtocolResult<()> {
        match ch {
            '"' | '\'' => self.start_string_literal(ch),
            ' ' | '\t' | '\n' | '\r' => self.handle_whitespace(),
            '(' | ')' | '{' | '}' | '[' | ']' | ',' | ':' | '+' | '-' | '*' | '/' | '%' => {
                self.handle_single_char_token(ch)
            }
            '=' => self.handle_equals_operator(),
            '!' => self.handle_exclamation_operator()?,
            '<' => self.handle_less_operator(),
            '>' => self.handle_greater_operator(),
            '.' => self.handle_dot_operator(),
            _ => self.current_token.push(ch),
        }
        Ok(())
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
            '(' => AqlToken::LeftParen,
            ')' => AqlToken::RightParen,
            '{' => AqlToken::LeftBrace,
            '}' => AqlToken::RightBrace,
            '[' => AqlToken::LeftBracket,
            ']' => AqlToken::RightBracket,
            ',' => AqlToken::Comma,
            ':' => AqlToken::Colon,
            '+' => AqlToken::Plus,
            '-' => AqlToken::Minus,
            '*' => AqlToken::Star,
            '/' => AqlToken::Slash,
            '%' => AqlToken::Percent,
            _ => unreachable!("Invalid single char token: {}", ch),
        };
        self.tokens.push(token);
    }

    fn handle_equals_operator(&mut self) {
        self.flush_current_token();
        if self.peek_char() == Some('=') {
            self.position += 1; // consume second =
            self.tokens.push(AqlToken::Equals);
        } else {
            self.tokens.push(AqlToken::Assignment);
        }
    }

    fn handle_exclamation_operator(&mut self) -> ProtocolResult<()> {
        if self.peek_char() == Some('=') {
            self.flush_current_token();
            self.position += 1; // consume =
            self.tokens.push(AqlToken::NotEquals);
        } else {
            self.current_token.push('!');
        }
        Ok(())
    }

    fn handle_less_operator(&mut self) {
        self.flush_current_token();
        if self.peek_char() == Some('=') {
            self.position += 1; // consume =
            self.tokens.push(AqlToken::LessOrEqual);
        } else {
            self.tokens.push(AqlToken::Less);
        }
    }

    fn handle_greater_operator(&mut self) {
        self.flush_current_token();
        if self.peek_char() == Some('=') {
            self.position += 1; // consume =
            self.tokens.push(AqlToken::GreaterOrEqual);
        } else {
            self.tokens.push(AqlToken::Greater);
        }
    }

    fn handle_dot_operator(&mut self) {
        if self.peek_char() == Some('.') {
            self.flush_current_token();
            self.position += 1; // consume second .
            self.tokens.push(AqlToken::Range);
        } else {
            self.flush_current_token();
            self.tokens.push(AqlToken::Dot);
        }
    }

    fn peek_char(&self) -> Option<char> {
        if self.position + 1 < self.chars.len() {
            Some(self.chars[self.position + 1])
        } else {
            None
        }
    }

    fn flush_current_token(&mut self) {
        if !self.current_token.is_empty() {
            self.tokens
                .push(AqlParser::classify_token_static(&self.current_token));
            self.current_token.clear();
        }
    }

    fn finalize_tokenization(&mut self) -> ProtocolResult<()> {
        if self.in_quotes {
            return Err(ProtocolError::ParseError(
                "Unterminated string literal".to_string(),
            ));
        }
        self.flush_current_token();
        Ok(())
    }
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
                Some(AqlToken::Window) => {
                    clauses.push(self.parse_window_clause()?);
                }
                Some(AqlToken::Search) => {
                    clauses.push(self.parse_search_clause()?);
                }
                Some(AqlToken::Upsert) => {
                    clauses.push(self.parse_upsert_clause()?);
                }
                Some(AqlToken::Replace) => {
                    clauses.push(self.parse_replace_clause()?);
                }
                Some(token) => {
                    return Err(ProtocolError::ParseError(format!(
                        "Unexpected token: {token:?}"
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

        // Parse optional PRUNE clause with proper backtracking
        let prune = if matches!(self.current_token(), Some(AqlToken::Prune)) {
            self.advance(); // consume PRUNE

            // Save position for backtracking - we need to look ahead to determine
            // if we have "PRUNE var: condition" or just "PRUNE condition"
            let saved_position = self.position;

            let prune_var = if let Some(AqlToken::Identifier(name)) = self.current_token() {
                let name = name.clone();
                self.advance();
                if matches!(self.current_token(), Some(AqlToken::Colon)) {
                    self.advance(); // consume :
                                    // This is a prune variable binding like "PRUNE v: v.depth > 3"
                    Some(name)
                } else {
                    // Not a prune var, just the condition start - backtrack!
                    // Restore position so the identifier can be parsed as part of the condition
                    self.position = saved_position;
                    None
                }
            } else {
                None
            };
            // Parse expression which can contain comparisons, AND, OR, NOT
            let expression = self.parse_expression()?;
            Some(PruneClause {
                condition: AqlCondition::Expression(expression),
                prune_var,
            })
        } else {
            None
        };

        // Parse optional OPTIONS clause
        let options = if matches!(self.current_token(), Some(AqlToken::Options)) {
            self.advance(); // consume OPTIONS
            Some(self.parse_traversal_options()?)
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
                options,
                prune,
            })
        } else {
            Ok(AqlClause::For {
                variable,
                data_source,
            })
        }
    }

    /// Parse traversal options object { bfs: true, uniqueVertices: "path", ... }
    fn parse_traversal_options(&mut self) -> ProtocolResult<TraversalOptions> {
        let mut options = TraversalOptions::default();

        // Expect opening brace
        if !matches!(self.current_token(), Some(AqlToken::LeftBrace)) {
            return Err(ProtocolError::ParseError(
                "Expected { for OPTIONS".to_string(),
            ));
        }
        self.advance(); // consume {

        while !matches!(self.current_token(), Some(AqlToken::RightBrace)) {
            let key = match self.current_token() {
                Some(AqlToken::Identifier(k)) | Some(AqlToken::String(k)) => {
                    let k = k.clone();
                    self.advance();
                    k
                }
                _ => return Err(ProtocolError::ParseError("Expected option key".to_string())),
            };

            // Expect colon
            if !matches!(self.current_token(), Some(AqlToken::Colon)) {
                return Err(ProtocolError::ParseError(
                    "Expected : after option key".to_string(),
                ));
            }
            self.advance(); // consume :

            match key.as_str() {
                "bfs" | "order" => {
                    match self.current_token() {
                        Some(AqlToken::Bool(true)) => {
                            options.order = TraversalOrder::Bfs;
                            self.advance();
                        }
                        Some(AqlToken::Bool(false)) => {
                            options.order = TraversalOrder::Dfs;
                            self.advance();
                        }
                        Some(AqlToken::String(s)) => {
                            options.order = match s.to_lowercase().as_str() {
                                "bfs" => TraversalOrder::Bfs,
                                "dfs" => TraversalOrder::Dfs,
                                "weighted" => TraversalOrder::Weighted,
                                _ => TraversalOrder::Bfs,
                            };
                            self.advance();
                        }
                        _ => {
                            self.advance(); // skip unknown value
                        }
                    }
                }
                "uniqueVertices" => {
                    if let Some(AqlToken::String(s)) = self.current_token() {
                        options.unique_vertices = match s.to_lowercase().as_str() {
                            "none" => UniquenessLevel::None,
                            "path" => UniquenessLevel::Path,
                            "global" => UniquenessLevel::Global,
                            _ => UniquenessLevel::None,
                        };
                        self.advance();
                    } else {
                        self.advance();
                    }
                }
                "uniqueEdges" => {
                    if let Some(AqlToken::String(s)) = self.current_token() {
                        options.unique_edges = match s.to_lowercase().as_str() {
                            "none" => UniquenessLevel::None,
                            "path" => UniquenessLevel::Path,
                            "global" => UniquenessLevel::Global,
                            _ => UniquenessLevel::None,
                        };
                        self.advance();
                    } else {
                        self.advance();
                    }
                }
                "parallelism" => {
                    if let Some(AqlToken::Number(n)) = self.current_token() {
                        options.parallelism = Some(n.as_u64().unwrap_or(1) as u32);
                        self.advance();
                    } else {
                        self.advance();
                    }
                }
                "maxItemsPerLevel" => {
                    if let Some(AqlToken::Number(n)) = self.current_token() {
                        options.max_items_per_level = Some(n.as_u64().unwrap_or(1000));
                        self.advance();
                    } else {
                        self.advance();
                    }
                }
                _ => {
                    // Skip unknown options
                    self.advance();
                }
            }

            // Skip comma if present
            if matches!(self.current_token(), Some(AqlToken::Comma)) {
                self.advance();
            }
        }

        // Consume closing brace
        if matches!(self.current_token(), Some(AqlToken::RightBrace)) {
            self.advance();
        }

        Ok(options)
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
        // Parse expression which can contain comparisons, AND, OR, NOT
        let expression = self.parse_expression()?;
        Ok(AqlClause::Filter {
            condition: AqlCondition::Expression(expression),
        })
    }

    fn parse_collect_clause(&mut self) -> ProtocolResult<AqlClause> {
        self.advance(); // consume COLLECT

        let mut groups = Vec::new();
        let mut into = None;
        let mut keep = None;
        let mut aggregates = None;
        let mut count_into = None;

        // Parse grouping variables
        while let Some(AqlToken::Identifier(name)) = self.current_token() {
            // Check for special keywords and clause boundaries
            let upper_name = name.to_uppercase();
            if upper_name == "INTO"
                || upper_name == "KEEP"
                || upper_name == "AGGREGATE"
                || upper_name == "WITH"
                || upper_name == "RETURN"
                || upper_name == "FOR"
                || upper_name == "FILTER"
                || upper_name == "SORT"
                || upper_name == "LIMIT"
                || upper_name == "LET"
            {
                break;
            }

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

        // Parse optional INTO clause
        if matches!(self.current_token(), Some(AqlToken::Into)) {
            self.advance();
            if let Some(AqlToken::Identifier(name)) = self.current_token() {
                into = Some(name.clone());
                self.advance();
            }
        }

        // Parse optional KEEP clause
        if matches!(self.current_token(), Some(AqlToken::Keep)) {
            self.advance();
            let mut keep_vars = Vec::new();
            loop {
                if let Some(AqlToken::Identifier(name)) = self.current_token() {
                    keep_vars.push(name.clone());
                    self.advance();
                    if matches!(self.current_token(), Some(AqlToken::Comma)) {
                        self.advance();
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            keep = Some(keep_vars);
        }

        // Parse optional AGGREGATE clause
        if matches!(self.current_token(), Some(AqlToken::Aggregate)) {
            self.advance();
            let mut agg_list = Vec::new();
            loop {
                if let Some(AqlToken::Identifier(name)) = self.current_token() {
                    let variable = name.clone();
                    self.advance();

                    // Expect =
                    if !matches!(self.current_token(), Some(AqlToken::Assignment)) {
                        break;
                    }
                    self.advance();

                    // Parse aggregate function
                    let (function, expression) = self.parse_collect_aggregate()?;
                    agg_list.push(CollectAggregate {
                        variable,
                        function,
                        expression,
                    });

                    if matches!(self.current_token(), Some(AqlToken::Comma)) {
                        self.advance();
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            aggregates = Some(agg_list);
        }

        // Parse optional COUNT clause
        if matches!(self.current_token(), Some(AqlToken::Count)) {
            self.advance();
            // Check for "INTO" after COUNT
            if matches!(self.current_token(), Some(AqlToken::Into)) {
                self.advance();
            }
            if let Some(AqlToken::Identifier(name)) = self.current_token() {
                count_into = Some(name.clone());
                self.advance();
            }
        }

        Ok(AqlClause::Collect {
            groups,
            into,
            keep,
            aggregates,
            count_into,
        })
    }

    /// Parse aggregate function in COLLECT clause
    fn parse_collect_aggregate(&mut self) -> ProtocolResult<(AggregateFunction, AqlExpression)> {
        let func = match self.current_token() {
            Some(AqlToken::Count) => {
                self.advance();
                AggregateFunction::Count
            }
            Some(AqlToken::Sum) => {
                self.advance();
                AggregateFunction::Sum
            }
            Some(AqlToken::Avg) => {
                self.advance();
                AggregateFunction::Avg
            }
            Some(AqlToken::MinFunc) => {
                self.advance();
                AggregateFunction::Min
            }
            Some(AqlToken::MaxFunc) => {
                self.advance();
                AggregateFunction::Max
            }
            Some(AqlToken::CollectArray) => {
                self.advance();
                AggregateFunction::CollectArray
            }
            Some(AqlToken::CollectUnique) => {
                self.advance();
                AggregateFunction::CollectUnique
            }
            Some(AqlToken::CountDistinct) => {
                self.advance();
                AggregateFunction::CountDistinct
            }
            Some(AqlToken::Stddev) => {
                self.advance();
                AggregateFunction::Stddev
            }
            Some(AqlToken::VarianceFunc) => {
                self.advance();
                AggregateFunction::Variance
            }
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected aggregate function".to_string(),
                ))
            }
        };

        // Expect (
        if !matches!(self.current_token(), Some(AqlToken::LeftParen)) {
            return Err(ProtocolError::ParseError(
                "Expected ( after aggregate function".to_string(),
            ));
        }
        self.advance();

        let expr = self.parse_expression()?;

        // Expect )
        if matches!(self.current_token(), Some(AqlToken::RightParen)) {
            self.advance();
        }

        Ok((func, expr))
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

        // Parse optional OPTIONS
        let options = if matches!(self.current_token(), Some(AqlToken::Options)) {
            self.advance();
            Some(self.parse_options_object()?)
        } else {
            None
        };

        Ok(AqlClause::Insert {
            document,
            collection,
            options,
        })
    }

    /// Parse an OPTIONS object { key: value, ... } into HashMap
    fn parse_options_object(&mut self) -> ProtocolResult<HashMap<String, AqlValue>> {
        let mut options = HashMap::new();

        if !matches!(self.current_token(), Some(AqlToken::LeftBrace)) {
            return Err(ProtocolError::ParseError(
                "Expected { for OPTIONS".to_string(),
            ));
        }
        self.advance();

        while !matches!(self.current_token(), Some(AqlToken::RightBrace)) {
            let key = match self.current_token() {
                Some(AqlToken::Identifier(k)) | Some(AqlToken::String(k)) => {
                    let k = k.clone();
                    self.advance();
                    k
                }
                _ => return Err(ProtocolError::ParseError("Expected option key".to_string())),
            };

            if matches!(self.current_token(), Some(AqlToken::Colon)) {
                self.advance();
            }

            let value = match self.current_token() {
                Some(AqlToken::Bool(b)) => {
                    let b = *b;
                    self.advance();
                    AqlValue::Bool(b)
                }
                Some(AqlToken::Number(n)) => {
                    let n = n.clone();
                    self.advance();
                    AqlValue::Number(n)
                }
                Some(AqlToken::String(s)) => {
                    let s = s.clone();
                    self.advance();
                    AqlValue::String(s)
                }
                Some(AqlToken::Null) => {
                    self.advance();
                    AqlValue::Null
                }
                _ => {
                    self.advance();
                    AqlValue::Null
                }
            };

            options.insert(key, value);

            if matches!(self.current_token(), Some(AqlToken::Comma)) {
                self.advance();
            }
        }

        if matches!(self.current_token(), Some(AqlToken::RightBrace)) {
            self.advance();
        }

        Ok(options)
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

        // Parse optional OPTIONS
        let options = if matches!(self.current_token(), Some(AqlToken::Options)) {
            self.advance();
            Some(self.parse_options_object()?)
        } else {
            None
        };

        Ok(AqlClause::Update {
            key,
            document,
            collection,
            options,
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

    /// Parse WINDOW clause for window functions
    fn parse_window_clause(&mut self) -> ProtocolResult<AqlClause> {
        self.advance(); // consume WINDOW

        let mut windows = Vec::new();

        // Parse window specifications
        loop {
            // Parse variable name
            let variable = match self.current_token() {
                Some(AqlToken::Identifier(name)) => {
                    let name = name.clone();
                    self.advance();
                    name
                }
                _ => break,
            };

            // Expect assignment
            if !matches!(self.current_token(), Some(AqlToken::Assignment)) {
                return Err(ProtocolError::ParseError(
                    "Expected = after window variable".to_string(),
                ));
            }
            self.advance();

            // Parse window function
            let function = self.parse_window_function()?;

            // Parse OVER clause
            if !matches!(self.current_token(), Some(AqlToken::Over)) {
                return Err(ProtocolError::ParseError(
                    "Expected OVER after window function".to_string(),
                ));
            }
            self.advance();

            // Expect ( for window specification
            if !matches!(self.current_token(), Some(AqlToken::LeftParen)) {
                return Err(ProtocolError::ParseError(
                    "Expected ( after OVER".to_string(),
                ));
            }
            self.advance();

            // Parse optional PARTITION BY
            let partition_by = if matches!(self.current_token(), Some(AqlToken::Partition)) {
                self.advance(); // PARTITION
                if !matches!(self.current_token(), Some(AqlToken::By)) {
                    return Err(ProtocolError::ParseError(
                        "Expected BY after PARTITION".to_string(),
                    ));
                }
                self.advance(); // BY

                let mut partitions = Vec::new();
                loop {
                    let expr = self.parse_expression()?;
                    partitions.push(expr);
                    if !matches!(self.current_token(), Some(AqlToken::Comma)) {
                        break;
                    }
                    self.advance();
                }
                Some(partitions)
            } else {
                None
            };

            // Parse optional ORDER BY
            let order_by = if matches!(self.current_token(), Some(AqlToken::Sort)) {
                self.advance(); // SORT (ORDER)
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
                    if !matches!(self.current_token(), Some(AqlToken::Comma)) {
                        break;
                    }
                    self.advance();
                }
                Some(items)
            } else {
                None
            };

            // Parse optional frame specification
            let frame = self.parse_window_frame()?;

            // Expect closing paren
            if !matches!(self.current_token(), Some(AqlToken::RightParen)) {
                return Err(ProtocolError::ParseError(
                    "Expected ) to close OVER clause".to_string(),
                ));
            }
            self.advance();

            windows.push(WindowClause {
                variable,
                function,
                partition_by,
                order_by,
                frame,
            });

            // Check for comma (multiple windows)
            if !matches!(self.current_token(), Some(AqlToken::Comma)) {
                break;
            }
            self.advance();
        }

        Ok(AqlClause::Window { windows })
    }

    /// Parse window function name and arguments
    fn parse_window_function(&mut self) -> ProtocolResult<WindowFunction> {
        let func = match self.current_token() {
            Some(AqlToken::RowNumber) => {
                self.advance();
                self.expect_paren_pair()?;
                WindowFunction::RowNumber
            }
            Some(AqlToken::RankFunc) => {
                self.advance();
                self.expect_paren_pair()?;
                WindowFunction::Rank
            }
            Some(AqlToken::DenseRank) => {
                self.advance();
                self.expect_paren_pair()?;
                WindowFunction::DenseRank
            }
            Some(AqlToken::PercentRank) => {
                self.advance();
                self.expect_paren_pair()?;
                WindowFunction::PercentRank
            }
            Some(AqlToken::CumeDist) => {
                self.advance();
                self.expect_paren_pair()?;
                WindowFunction::CumeDist
            }
            Some(AqlToken::Ntile) => {
                self.advance();
                if !matches!(self.current_token(), Some(AqlToken::LeftParen)) {
                    return Err(ProtocolError::ParseError(
                        "Expected ( after NTILE".to_string(),
                    ));
                }
                self.advance();
                let n = match self.current_token() {
                    Some(AqlToken::Number(num)) => {
                        let n = num.as_u64().unwrap_or(1) as u32;
                        self.advance();
                        n
                    }
                    _ => 1,
                };
                if matches!(self.current_token(), Some(AqlToken::RightParen)) {
                    self.advance();
                }
                WindowFunction::Ntile(n)
            }
            Some(AqlToken::LagFunc) => {
                self.advance();
                self.parse_lag_lead_function(true)?
            }
            Some(AqlToken::LeadFunc) => {
                self.advance();
                self.parse_lag_lead_function(false)?
            }
            Some(AqlToken::FirstValue) => {
                self.advance();
                if !matches!(self.current_token(), Some(AqlToken::LeftParen)) {
                    return Err(ProtocolError::ParseError(
                        "Expected ( after FIRST_VALUE".to_string(),
                    ));
                }
                self.advance();
                let expr = self.parse_expression()?;
                if matches!(self.current_token(), Some(AqlToken::RightParen)) {
                    self.advance();
                }
                WindowFunction::FirstValue(Box::new(expr))
            }
            Some(AqlToken::LastValue) => {
                self.advance();
                if !matches!(self.current_token(), Some(AqlToken::LeftParen)) {
                    return Err(ProtocolError::ParseError(
                        "Expected ( after LAST_VALUE".to_string(),
                    ));
                }
                self.advance();
                let expr = self.parse_expression()?;
                if matches!(self.current_token(), Some(AqlToken::RightParen)) {
                    self.advance();
                }
                WindowFunction::LastValue(Box::new(expr))
            }
            Some(AqlToken::NthValue) => {
                self.advance();
                if !matches!(self.current_token(), Some(AqlToken::LeftParen)) {
                    return Err(ProtocolError::ParseError(
                        "Expected ( after NTH_VALUE".to_string(),
                    ));
                }
                self.advance();
                let expr = self.parse_expression()?;
                if matches!(self.current_token(), Some(AqlToken::Comma)) {
                    self.advance();
                }
                let n = match self.current_token() {
                    Some(AqlToken::Number(num)) => {
                        let n = num.as_u64().unwrap_or(1) as u32;
                        self.advance();
                        n
                    }
                    _ => 1,
                };
                if matches!(self.current_token(), Some(AqlToken::RightParen)) {
                    self.advance();
                }
                WindowFunction::NthValue {
                    expression: Box::new(expr),
                    n,
                }
            }
            // Aggregate functions
            Some(AqlToken::Sum) => {
                self.advance();
                self.parse_aggregate_window_function(AggregateFunction::Sum)?
            }
            Some(AqlToken::Avg) => {
                self.advance();
                self.parse_aggregate_window_function(AggregateFunction::Avg)?
            }
            Some(AqlToken::MinFunc) => {
                self.advance();
                self.parse_aggregate_window_function(AggregateFunction::Min)?
            }
            Some(AqlToken::MaxFunc) => {
                self.advance();
                self.parse_aggregate_window_function(AggregateFunction::Max)?
            }
            Some(AqlToken::Count) => {
                self.advance();
                self.parse_aggregate_window_function(AggregateFunction::Count)?
            }
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected window function".to_string(),
                ))
            }
        };

        Ok(func)
    }

    fn expect_paren_pair(&mut self) -> ProtocolResult<()> {
        if matches!(self.current_token(), Some(AqlToken::LeftParen)) {
            self.advance();
            if matches!(self.current_token(), Some(AqlToken::RightParen)) {
                self.advance();
            }
        }
        Ok(())
    }

    fn parse_lag_lead_function(&mut self, is_lag: bool) -> ProtocolResult<WindowFunction> {
        if !matches!(self.current_token(), Some(AqlToken::LeftParen)) {
            return Err(ProtocolError::ParseError(
                "Expected ( after LAG/LEAD".to_string(),
            ));
        }
        self.advance();

        let expr = self.parse_expression()?;

        let mut offset = 1;
        let mut default = None;

        if matches!(self.current_token(), Some(AqlToken::Comma)) {
            self.advance();
            if let Some(AqlToken::Number(n)) = self.current_token() {
                offset = n.as_u64().unwrap_or(1) as u32;
                self.advance();
            }

            if matches!(self.current_token(), Some(AqlToken::Comma)) {
                self.advance();
                let default_expr = self.parse_expression()?;
                default = Some(Box::new(default_expr));
            }
        }

        if matches!(self.current_token(), Some(AqlToken::RightParen)) {
            self.advance();
        }

        if is_lag {
            Ok(WindowFunction::Lag {
                expression: Box::new(expr),
                offset,
                default,
            })
        } else {
            Ok(WindowFunction::Lead {
                expression: Box::new(expr),
                offset,
                default,
            })
        }
    }

    fn parse_aggregate_window_function(
        &mut self,
        func: AggregateFunction,
    ) -> ProtocolResult<WindowFunction> {
        if !matches!(self.current_token(), Some(AqlToken::LeftParen)) {
            return Err(ProtocolError::ParseError(
                "Expected ( after aggregate function".to_string(),
            ));
        }
        self.advance();

        let expr = self.parse_expression()?;

        if matches!(self.current_token(), Some(AqlToken::RightParen)) {
            self.advance();
        }

        Ok(WindowFunction::Aggregate {
            function: func,
            expression: Box::new(expr),
        })
    }

    fn parse_window_frame(&mut self) -> ProtocolResult<Option<WindowFrame>> {
        let frame_type = match self.current_token() {
            Some(AqlToken::Rows) => {
                self.advance();
                WindowFrameType::Rows
            }
            Some(AqlToken::RangeKeyword) => {
                self.advance();
                WindowFrameType::Range
            }
            Some(AqlToken::Groups) => {
                self.advance();
                WindowFrameType::Groups
            }
            _ => return Ok(None),
        };

        // Parse start bound
        let start = self.parse_frame_bound()?;

        // Check for AND (BETWEEN ... AND ...)
        let end = if matches!(self.current_token(), Some(AqlToken::And)) {
            self.advance();
            self.parse_frame_bound()?
        } else {
            WindowFrameBound::CurrentRow
        };

        Ok(Some(WindowFrame {
            frame_type,
            start,
            end,
        }))
    }

    fn parse_frame_bound(&mut self) -> ProtocolResult<WindowFrameBound> {
        match self.current_token() {
            Some(AqlToken::Unbounded) => {
                self.advance();
                if matches!(self.current_token(), Some(AqlToken::Preceding)) {
                    self.advance();
                    Ok(WindowFrameBound::Unbounded)
                } else if matches!(self.current_token(), Some(AqlToken::Following)) {
                    self.advance();
                    Ok(WindowFrameBound::Unbounded)
                } else {
                    Ok(WindowFrameBound::Unbounded)
                }
            }
            Some(AqlToken::Current) => {
                self.advance();
                if matches!(self.current_token(), Some(AqlToken::Row)) {
                    self.advance();
                }
                Ok(WindowFrameBound::CurrentRow)
            }
            Some(AqlToken::Number(n)) => {
                let val = n.as_u64().unwrap_or(0) as u32;
                self.advance();
                if matches!(self.current_token(), Some(AqlToken::Preceding)) {
                    self.advance();
                    Ok(WindowFrameBound::Preceding(val))
                } else if matches!(self.current_token(), Some(AqlToken::Following)) {
                    self.advance();
                    Ok(WindowFrameBound::Following(val))
                } else {
                    Ok(WindowFrameBound::Preceding(val))
                }
            }
            _ => Ok(WindowFrameBound::CurrentRow),
        }
    }

    /// Parse SEARCH clause for full-text search
    fn parse_search_clause(&mut self) -> ProtocolResult<AqlClause> {
        self.advance(); // consume SEARCH

        let expression = self.parse_expression()?;

        // Optional analyzer specification
        let analyzer = if matches!(self.current_token(), Some(AqlToken::Options)) {
            self.advance();
            // Parse options object for analyzer
            if matches!(self.current_token(), Some(AqlToken::LeftBrace)) {
                self.advance();
                let mut analyzer_name = None;
                while !matches!(self.current_token(), Some(AqlToken::RightBrace)) {
                    if let Some(AqlToken::Identifier(key)) = self.current_token() {
                        if key == "analyzer" {
                            self.advance();
                            if matches!(self.current_token(), Some(AqlToken::Colon)) {
                                self.advance();
                            }
                            if let Some(AqlToken::String(name)) = self.current_token() {
                                analyzer_name = Some(name.clone());
                                self.advance();
                            }
                        } else {
                            self.advance();
                        }
                    } else {
                        self.advance();
                    }
                    if matches!(self.current_token(), Some(AqlToken::Comma)) {
                        self.advance();
                    }
                }
                if matches!(self.current_token(), Some(AqlToken::RightBrace)) {
                    self.advance();
                }
                analyzer_name
            } else {
                None
            }
        } else {
            None
        };

        Ok(AqlClause::Search {
            expression,
            analyzer,
        })
    }

    /// Parse UPSERT clause
    fn parse_upsert_clause(&mut self) -> ProtocolResult<AqlClause> {
        self.advance(); // consume UPSERT

        let search = self.parse_expression()?;

        // Expect INSERT
        if !matches!(self.current_token(), Some(AqlToken::Insert)) {
            return Err(ProtocolError::ParseError(
                "Expected INSERT in UPSERT clause".to_string(),
            ));
        }
        self.advance();

        let insert = self.parse_expression()?;

        // Expect UPDATE or REPLACE
        let update_or_replace = if matches!(self.current_token(), Some(AqlToken::Update)) {
            self.advance();
            let expr = self.parse_expression()?;
            UpsertAction::Update(expr)
        } else if matches!(self.current_token(), Some(AqlToken::Replace)) {
            self.advance();
            let expr = self.parse_expression()?;
            UpsertAction::Replace(expr)
        } else {
            return Err(ProtocolError::ParseError(
                "Expected UPDATE or REPLACE in UPSERT clause".to_string(),
            ));
        };

        // Expect IN
        if !matches!(self.current_token(), Some(AqlToken::In)) {
            return Err(ProtocolError::ParseError(
                "Expected IN in UPSERT clause".to_string(),
            ));
        }
        self.advance();

        let collection = match self.current_token() {
            Some(AqlToken::Identifier(name)) => {
                let name = name.clone();
                self.advance();
                name
            }
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected collection name in UPSERT clause".to_string(),
                ))
            }
        };

        Ok(AqlClause::Upsert {
            search,
            insert,
            update_or_replace,
            collection,
        })
    }

    /// Parse REPLACE clause
    fn parse_replace_clause(&mut self) -> ProtocolResult<AqlClause> {
        self.advance(); // consume REPLACE

        let key = self.parse_expression()?;

        // Expect WITH
        if !matches!(self.current_token(), Some(AqlToken::With)) {
            return Err(ProtocolError::ParseError(
                "Expected WITH in REPLACE clause".to_string(),
            ));
        }
        self.advance();

        let document = self.parse_expression()?;

        // Expect IN
        if !matches!(self.current_token(), Some(AqlToken::In)) {
            return Err(ProtocolError::ParseError(
                "Expected IN in REPLACE clause".to_string(),
            ));
        }
        self.advance();

        let collection = match self.current_token() {
            Some(AqlToken::Identifier(name)) => {
                let name = name.clone();
                self.advance();
                name
            }
            _ => {
                return Err(ProtocolError::ParseError(
                    "Expected collection name in REPLACE clause".to_string(),
                ))
            }
        };

        Ok(AqlClause::Replace {
            key,
            document,
            collection,
        })
    }

    fn parse_expression(&mut self) -> ProtocolResult<AqlExpression> {
        self.parse_or_expression()
    }

    /// Parse OR expression (lowest precedence)
    fn parse_or_expression(&mut self) -> ProtocolResult<AqlExpression> {
        let mut left = self.parse_and_expression()?;

        while matches!(self.current_token(), Some(AqlToken::Or)) {
            self.advance(); // consume OR
            let right = self.parse_and_expression()?;
            left = AqlExpression::BinaryOp {
                op: "OR".to_string(),
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse AND expression
    fn parse_and_expression(&mut self) -> ProtocolResult<AqlExpression> {
        let mut left = self.parse_not_expression()?;

        while matches!(self.current_token(), Some(AqlToken::And)) {
            self.advance(); // consume AND
            let right = self.parse_not_expression()?;
            left = AqlExpression::BinaryOp {
                op: "AND".to_string(),
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse NOT expression
    fn parse_not_expression(&mut self) -> ProtocolResult<AqlExpression> {
        if matches!(self.current_token(), Some(AqlToken::Not)) {
            self.advance(); // consume NOT
            let expr = self.parse_not_expression()?;
            Ok(AqlExpression::UnaryOp {
                op: "NOT".to_string(),
                expr: Box::new(expr),
            })
        } else {
            self.parse_comparison_expression()
        }
    }

    /// Parse comparison expression (==, !=, <, <=, >, >=)
    fn parse_comparison_expression(&mut self) -> ProtocolResult<AqlExpression> {
        let mut left = self.parse_additive_expression()?;

        // Check for comparison operators
        while matches!(
            self.current_token(),
            Some(AqlToken::Equals)
                | Some(AqlToken::NotEquals)
                | Some(AqlToken::Less)
                | Some(AqlToken::LessOrEqual)
                | Some(AqlToken::Greater)
                | Some(AqlToken::GreaterOrEqual)
        ) {
            let op = match self.current_token() {
                Some(AqlToken::Equals) => "==",
                Some(AqlToken::NotEquals) => "!=",
                Some(AqlToken::Less) => "<",
                Some(AqlToken::LessOrEqual) => "<=",
                Some(AqlToken::Greater) => ">",
                Some(AqlToken::GreaterOrEqual) => ">=",
                _ => unreachable!(),
            };
            self.advance(); // consume operator
            let right = self.parse_additive_expression()?;
            left = AqlExpression::BinaryOp {
                op: op.to_string(),
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse additive expression (+, -)
    fn parse_additive_expression(&mut self) -> ProtocolResult<AqlExpression> {
        let mut left = self.parse_multiplicative_expression()?;

        while matches!(
            self.current_token(),
            Some(AqlToken::Plus) | Some(AqlToken::Minus)
        ) {
            let op = match self.current_token() {
                Some(AqlToken::Plus) => "+",
                Some(AqlToken::Minus) => "-",
                _ => unreachable!(),
            };
            self.advance(); // consume operator
            let right = self.parse_multiplicative_expression()?;
            left = AqlExpression::BinaryOp {
                op: op.to_string(),
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse multiplicative expression (*, /, %)
    fn parse_multiplicative_expression(&mut self) -> ProtocolResult<AqlExpression> {
        let mut left = self.parse_primary_expression()?;

        while matches!(
            self.current_token(),
            Some(AqlToken::Star) | Some(AqlToken::Slash) | Some(AqlToken::Percent)
        ) {
            let op = match self.current_token() {
                Some(AqlToken::Star) => "*",
                Some(AqlToken::Slash) => "/",
                Some(AqlToken::Percent) => "%",
                _ => unreachable!(),
            };
            self.advance(); // consume operator
            let right = self.parse_primary_expression()?;
            left = AqlExpression::BinaryOp {
                op: op.to_string(),
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse primary expression (literals, identifiers, function calls)
    fn parse_primary_expression(&mut self) -> ProtocolResult<AqlExpression> {
        match self.current_token() {
            Some(AqlToken::Identifier(name)) => {
                let name = name.clone();
                self.advance();

                // Check for function call
                if matches!(self.current_token(), Some(AqlToken::LeftParen)) {
                    self.advance(); // consume (
                    let mut args = Vec::new();

                    // Parse arguments
                    if !matches!(self.current_token(), Some(AqlToken::RightParen)) {
                        args.push(self.parse_expression()?);

                        while matches!(self.current_token(), Some(AqlToken::Comma)) {
                            self.advance(); // consume ,
                            args.push(self.parse_expression()?);
                        }
                    }

                    // Expect closing paren
                    match self.current_token() {
                        Some(AqlToken::RightParen) => self.advance(),
                        _ => {
                            return Err(ProtocolError::ParseError(
                                "Expected ) after function arguments".to_string(),
                            ))
                        }
                    };

                    Ok(AqlExpression::FunctionCall { name, args })
                }
                // Check for property access
                else if matches!(self.current_token(), Some(AqlToken::Dot)) {
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
            Some(AqlToken::LeftParen) => {
                self.advance(); // consume (
                let expr = self.parse_expression()?;
                match self.current_token() {
                    Some(AqlToken::RightParen) => self.advance(),
                    _ => {
                        return Err(ProtocolError::ParseError(
                            "Expected ) after expression".to_string(),
                        ))
                    }
                };
                Ok(expr)
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

            // Check for colon (explicit key: value) or shorthand ({key} = {key: key})
            let value = if matches!(self.current_token(), Some(AqlToken::Colon)) {
                self.advance(); // consume :
                self.parse_expression()?
            } else {
                // Shorthand syntax: {vertex, edge} => {vertex: vertex, edge: edge}
                AqlExpression::Variable(key.clone())
            };
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

    #[allow(dead_code)]
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
        /// Traversal options (bfs/dfs, uniqueness constraints)
        options: Option<TraversalOptions>,
        /// Optional PRUNE condition
        prune: Option<PruneClause>,
    },
    /// FOR clause for shortest path
    ForShortestPath {
        /// Variable to bind path result
        path_var: String,
        /// Shortest path query specification
        query: ShortestPathQuery,
    },
    /// FOR clause for K shortest paths
    ForKShortestPaths {
        /// Variable to bind path result
        path_var: String,
        /// K shortest paths query specification
        query: KShortestPathsQuery,
    },
    /// FOR clause for all shortest paths
    ForAllShortestPaths {
        /// Variable to bind path result
        path_var: String,
        /// Start vertex
        start_vertex: AqlExpression,
        /// Target vertex
        target_vertex: AqlExpression,
        /// Traversal direction
        direction: TraversalDirection,
        /// Graph source
        graph_source: GraphSource,
    },
    /// LET clause for variable assignment
    Let {
        variable: String,
        expression: AqlExpression,
    },
    /// FILTER clause for filtering
    Filter { condition: AqlCondition },
    /// COLLECT clause for grouping
    Collect {
        groups: Vec<CollectGroup>,
        /// INTO clause for grouping into array
        into: Option<String>,
        /// KEEP clause for preserving variables
        keep: Option<Vec<String>>,
        /// AGGREGATE clause
        aggregates: Option<Vec<CollectAggregate>>,
        /// COUNT clause
        count_into: Option<String>,
    },
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
        /// Optional options
        options: Option<HashMap<String, AqlValue>>,
    },
    /// UPDATE clause for modifying documents
    Update {
        key: AqlExpression,
        document: AqlExpression,
        collection: String,
        /// Optional options
        options: Option<HashMap<String, AqlValue>>,
    },
    /// REPLACE clause for replacing documents
    Replace {
        key: AqlExpression,
        document: AqlExpression,
        collection: String,
    },
    /// REMOVE clause for deleting documents
    Remove {
        key: AqlExpression,
        collection: String,
    },
    /// UPSERT clause for insert-or-update
    Upsert {
        search: AqlExpression,
        insert: AqlExpression,
        update_or_replace: UpsertAction,
        collection: String,
    },
    /// WINDOW clause for window functions
    Window {
        /// Window specifications
        windows: Vec<WindowClause>,
    },
    /// SEARCH clause for full-text search (ArangoSearch)
    Search {
        /// Search expression/condition
        expression: AqlExpression,
        /// Optional analyzer
        analyzer: Option<String>,
    },
}

/// COLLECT aggregate specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectAggregate {
    /// Variable to bind aggregate result
    pub variable: String,
    /// Aggregate function
    pub function: AggregateFunction,
    /// Expression to aggregate
    pub expression: AqlExpression,
}

/// UPSERT action type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpsertAction {
    /// UPDATE existing document
    Update(AqlExpression),
    /// REPLACE existing document
    Replace(AqlExpression),
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
    /// Function call
    FunctionCall {
        name: String,
        args: Vec<AqlExpression>,
    },
    /// Binary operation (e.g., a + b, a AND b)
    BinaryOp {
        op: String,
        left: Box<AqlExpression>,
        right: Box<AqlExpression>,
    },
    /// Unary operation (e.g., NOT a, -b)
    UnaryOp {
        op: String,
        expr: Box<AqlExpression>,
    },
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
    /// Expression-based condition (supports AND, OR, NOT)
    Expression(AqlExpression),
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

/// Graph traversal options (ArangoDB-compatible)
///
/// Controls how graph traversals are executed including
/// uniqueness constraints and traversal strategy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraversalOptions {
    /// Traversal order strategy
    pub order: TraversalOrder,
    /// Uniqueness constraint for vertices
    pub unique_vertices: UniquenessLevel,
    /// Uniqueness constraint for edges
    pub unique_edges: UniquenessLevel,
    /// Edge collections to traverse (empty = all)
    pub edge_collections: Vec<EdgeCollectionConfig>,
    /// Maximum number of items per traversal level
    pub max_items_per_level: Option<u64>,
    /// Parallelism factor for traversal
    pub parallelism: Option<u32>,
}

/// Traversal order strategy
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum TraversalOrder {
    /// Breadth-first search (default)
    #[default]
    Bfs,
    /// Depth-first search
    Dfs,
    /// Weighted traversal (uses edge weights)
    Weighted,
}

/// Uniqueness constraint levels for traversal
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum UniquenessLevel {
    /// No uniqueness constraint
    #[default]
    None,
    /// Unique within current path
    Path,
    /// Globally unique across all paths
    Global,
}

/// Edge collection configuration for traversal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeCollectionConfig {
    /// Collection name
    pub collection: String,
    /// Direction override for this collection
    pub direction: Option<TraversalDirection>,
}

/// PRUNE clause for early traversal termination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruneClause {
    /// Condition that triggers pruning
    pub condition: AqlCondition,
    /// Prune variable (optional vertex/edge/path reference)
    pub prune_var: Option<String>,
}

/// WINDOW clause for window functions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowClause {
    /// Variable to bind window result
    pub variable: String,
    /// Window function to apply
    pub function: WindowFunction,
    /// Partition specification
    pub partition_by: Option<Vec<AqlExpression>>,
    /// Order specification within partition
    pub order_by: Option<Vec<SortItem>>,
    /// Window frame specification
    pub frame: Option<WindowFrame>,
}

/// Supported window functions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WindowFunction {
    /// Row number within partition (1-based)
    RowNumber,
    /// Rank with gaps for ties
    Rank,
    /// Rank without gaps
    DenseRank,
    /// Percent rank
    PercentRank,
    /// Cumulative distribution
    CumeDist,
    /// N-tile distribution
    Ntile(u32),
    /// Value from N rows before current
    Lag {
        expression: Box<AqlExpression>,
        offset: u32,
        default: Option<Box<AqlExpression>>,
    },
    /// Value from N rows after current
    Lead {
        expression: Box<AqlExpression>,
        offset: u32,
        default: Option<Box<AqlExpression>>,
    },
    /// First value in window
    FirstValue(Box<AqlExpression>),
    /// Last value in window
    LastValue(Box<AqlExpression>),
    /// Nth value in window
    NthValue {
        expression: Box<AqlExpression>,
        n: u32,
    },
    /// Aggregate function over window
    Aggregate {
        function: AggregateFunction,
        expression: Box<AqlExpression>,
    },
}

/// Aggregate functions for COLLECT and WINDOW
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregateFunction {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    CountDistinct,
    CollectArray,
    CollectUnique,
    Stddev,
    Variance,
    StddevSample,
    VarianceSample,
}

/// Window frame specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowFrame {
    /// Frame type
    pub frame_type: WindowFrameType,
    /// Start bound
    pub start: WindowFrameBound,
    /// End bound (defaults to CURRENT ROW if not specified)
    pub end: WindowFrameBound,
}

/// Window frame type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WindowFrameType {
    /// Row-based frame
    Rows,
    /// Range-based frame (value-based)
    Range,
    /// Groups-based frame
    Groups,
}

/// Window frame bound specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WindowFrameBound {
    /// No bound (beginning or end of partition)
    Unbounded,
    /// Current row
    CurrentRow,
    /// N rows/values preceding
    Preceding(u32),
    /// N rows/values following
    Following(u32),
}

/// Shortest path query specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortestPathQuery {
    /// Start vertex expression
    pub start_vertex: AqlExpression,
    /// Target vertex expression
    pub target_vertex: AqlExpression,
    /// Direction of traversal
    pub direction: TraversalDirection,
    /// Graph name or edge collections
    pub graph_source: GraphSource,
    /// Path options
    pub options: ShortestPathOptions,
}

/// K shortest paths query specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KShortestPathsQuery {
    /// Start vertex expression
    pub start_vertex: AqlExpression,
    /// Target vertex expression
    pub target_vertex: AqlExpression,
    /// Direction of traversal
    pub direction: TraversalDirection,
    /// Graph name or edge collections
    pub graph_source: GraphSource,
    /// Number of paths to return
    pub k: u32,
    /// Path options
    pub options: ShortestPathOptions,
}

/// Graph source (named graph or edge collections)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphSource {
    /// Named graph
    Graph(String),
    /// Explicit edge collections
    EdgeCollections(Vec<String>),
}

/// Options for shortest path algorithms
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShortestPathOptions {
    /// Weight attribute for edges
    pub weight_attribute: Option<String>,
    /// Default weight if attribute missing
    pub default_weight: f64,
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
