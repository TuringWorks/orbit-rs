//! Parser for OrbitQL
//!
//! This module provides the `Parser` struct that converts a stream of tokens
//! into an Abstract Syntax Tree (AST) for OrbitQL queries.

use crate::orbitql::ast::*;
use crate::orbitql::lexer::{LexError, Token, TokenType};
use crate::orbitql::QueryValue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Parser errors
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParseError {
    UnexpectedToken {
        expected: Vec<TokenType>,
        found: Token,
    },
    UnexpectedEndOfInput {
        expected: Vec<TokenType>,
    },
    InvalidExpression {
        message: String,
        token: Token,
    },
    LexError(String), // Wrapped LexError
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnexpectedToken { expected, found } => {
                write!(
                    f,
                    "Unexpected token {} at line {}, column {}. Expected one of: {:?}",
                    found.value, found.line, found.column, expected
                )
            }
            ParseError::UnexpectedEndOfInput { expected } => {
                write!(
                    f,
                    "Unexpected end of input. Expected one of: {:?}",
                    expected
                )
            }
            ParseError::InvalidExpression { message, token } => {
                write!(
                    f,
                    "Invalid expression: {} at line {}, column {}",
                    message, token.line, token.column
                )
            }
            ParseError::LexError(msg) => write!(f, "Lexer error: {}", msg),
        }
    }
}

impl std::error::Error for ParseError {}

impl From<LexError> for ParseError {
    fn from(error: LexError) -> Self {
        ParseError::LexError(error.to_string())
    }
}

/// OrbitQL parser
pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            tokens: Vec::new(),
            current: 0,
        }
    }

    /// Parse a list of tokens into a statement
    pub fn parse(&mut self, tokens: Vec<Token>) -> Result<Statement, ParseError> {
        self.tokens = tokens
            .into_iter()
            .filter(|t| !matches!(t.token_type, TokenType::Comment))
            .collect();
        self.current = 0;
        self.parse_statement()
    }

    /// Parse a single statement
    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        let token = self.peek()?;

        match token.token_type {
            TokenType::With => Ok(Statement::Select(self.parse_select_with_cte()?)),
            TokenType::Select => Ok(Statement::Select(self.parse_select()?)),
            TokenType::Insert => Ok(Statement::Insert(self.parse_insert()?)),
            TokenType::Update => Ok(Statement::Update(self.parse_update()?)),
            TokenType::Delete => Ok(Statement::Delete(self.parse_delete()?)),
            TokenType::Create => Ok(Statement::Create(self.parse_create()?)),
            TokenType::Drop => Ok(Statement::Drop(self.parse_drop()?)),
            TokenType::Begin | TokenType::Commit | TokenType::Rollback => {
                Ok(Statement::Transaction(self.parse_transaction()?))
            }
            TokenType::Relate => Ok(Statement::Relate(self.parse_relate()?)),
            TokenType::Live => Ok(Statement::Live(self.parse_live()?)),
            _ => Err(ParseError::UnexpectedToken {
                expected: vec![
                    TokenType::With,
                    TokenType::Select,
                    TokenType::Insert,
                    TokenType::Update,
                    TokenType::Delete,
                    TokenType::Create,
                    TokenType::Drop,
                    TokenType::Begin,
                    TokenType::Commit,
                    TokenType::Rollback,
                    TokenType::Relate,
                    TokenType::Live,
                ],
                found: token.clone(),
            }),
        }
    }

    /// Parse SELECT statement
    fn parse_select(&mut self) -> Result<SelectStatement, ParseError> {
        self.expect(TokenType::Select)?;

        // Parse fields
        let fields = self.parse_select_fields()?;

        // Parse FROM clause (optional for some queries like `SELECT 1`)
        let from = if self.matches(&[TokenType::From]) {
            self.advance();
            self.parse_from_clauses()?
        } else {
            Vec::new()
        };

        // Parse optional clauses
        let mut stmt = SelectStatement {
            with_clauses: Vec::new(),
            fields,
            from,
            where_clause: None,
            join_clauses: Vec::new(),
            group_by: Vec::new(),
            having: None,
            order_by: Vec::new(),
            limit: None,
            offset: None,
            fetch: Vec::new(),
            timeout: None,
        };

        // WHERE clause
        if self.matches(&[TokenType::Where]) {
            self.advance();
            stmt.where_clause = Some(self.parse_expression()?);
        }

        // JOIN clauses
        while self.matches(&[
            TokenType::Join,
            TokenType::Inner,
            TokenType::Left,
            TokenType::Right,
            TokenType::Full,
            TokenType::Cross,
        ]) {
            stmt.join_clauses.push(self.parse_join()?);
        }

        // GROUP BY clause
        if self.matches(&[TokenType::Group]) {
            self.advance();
            self.expect(TokenType::By)?;
            stmt.group_by = self.parse_expression_list()?;
        }

        // HAVING clause
        if self.matches(&[TokenType::Having]) {
            self.advance();
            stmt.having = Some(self.parse_expression()?);
        }

        // ORDER BY clause
        if self.matches(&[TokenType::Order]) {
            self.advance();
            self.expect(TokenType::By)?;
            stmt.order_by = self.parse_order_by_list()?;
        }

        // LIMIT clause
        if self.matches(&[TokenType::Limit]) {
            self.advance();
            let limit_expr = self.parse_expression()?;
            if let Expression::Literal(QueryValue::Integer(n)) = limit_expr {
                stmt.limit = Some(n as u64);
            } else {
                return Err(ParseError::InvalidExpression {
                    message: "LIMIT must be an integer".to_string(),
                    token: self.previous().clone(),
                });
            }
        }

        // OFFSET clause
        if self.matches(&[TokenType::Offset]) {
            self.advance();
            let offset_expr = self.parse_expression()?;
            if let Expression::Literal(QueryValue::Integer(n)) = offset_expr {
                stmt.offset = Some(n as u64);
            } else {
                return Err(ParseError::InvalidExpression {
                    message: "OFFSET must be an integer".to_string(),
                    token: self.previous().clone(),
                });
            }
        }

        // FETCH clause
        if self.matches(&[TokenType::Fetch]) {
            self.advance();
            stmt.fetch = self.parse_fetch_list()?;
        }

        Ok(stmt)
    }

    /// Parse SELECT field list
    fn parse_select_fields(&mut self) -> Result<Vec<SelectField>, ParseError> {
        let mut fields = Vec::new();

        // Handle SELECT *
        if self.matches(&[TokenType::Multiply]) {
            self.advance();
            fields.push(SelectField::All);

            // Check for more fields after *
            if self.matches(&[TokenType::Comma]) {
                self.advance();
                fields.extend(self.parse_select_fields()?);
            }
            return Ok(fields);
        }

        // Parse first field
        fields.push(self.parse_select_field()?);

        // Parse additional fields
        while self.matches(&[TokenType::Comma]) {
            self.advance();
            fields.push(self.parse_select_field()?);
        }

        Ok(fields)
    }

    /// Parse a single SELECT field
    fn parse_select_field(&mut self) -> Result<SelectField, ParseError> {
        // Check for table.* pattern
        if self.peek()?.token_type == TokenType::Identifier
            && self.peek_ahead(1)?.token_type == TokenType::Dot
            && self.peek_ahead(2)?.token_type == TokenType::Multiply
        {
            let table_name = self.advance().value.clone();
            self.advance(); // consume .
            self.advance(); // consume *
            return Ok(SelectField::AllFrom(table_name));
        }

        // Parse expression with optional alias
        let expr = self.parse_expression()?;

        let alias = if self.matches(&[TokenType::As]) {
            self.advance();
            Some(self.expect_identifier()?.value.clone())
        } else if self.peek()?.token_type == TokenType::Identifier
            && !self.is_reserved_keyword(&self.peek()?.value)
        {
            // Implicit alias (identifier without AS)
            Some(self.advance().value.clone())
        } else {
            None
        };

        Ok(SelectField::Expression { expr, alias })
    }

    /// Parse FROM clauses
    fn parse_from_clauses(&mut self) -> Result<Vec<FromClause>, ParseError> {
        let mut clauses = Vec::new();

        clauses.push(self.parse_from_clause()?);

        while self.matches(&[TokenType::Comma]) {
            self.advance();
            clauses.push(self.parse_from_clause()?);
        }

        Ok(clauses)
    }

    /// Parse a single FROM clause
    fn parse_from_clause(&mut self) -> Result<FromClause, ParseError> {
        // For now, just handle table names
        // TODO: Add support for graph patterns, time series, subqueries
        let name = self.expect_identifier_or_keyword()?.value.clone();

        let alias = if self.matches(&[TokenType::As]) {
            self.advance();
            Some(self.expect_identifier_or_keyword()?.value.clone())
        } else if matches!(
            self.peek()?.token_type,
            TokenType::Identifier
                | TokenType::Metrics
                | TokenType::Aggregate
                | TokenType::Window
                | TokenType::Range
                | TokenType::Node
                | TokenType::Edge
                | TokenType::Path
        ) && !self.is_reserved_keyword(&self.peek()?.value)
        {
            Some(self.advance().value.clone())
        } else {
            None
        };

        Ok(FromClause::Table { name, alias })
    }

    /// Parse JOIN clause
    fn parse_join(&mut self) -> Result<JoinClause, ParseError> {
        let join_type = match self.peek()?.token_type {
            TokenType::Join => {
                self.advance();
                JoinType::Inner
            }
            TokenType::Inner => {
                self.advance();
                self.expect(TokenType::Join)?;
                JoinType::Inner
            }
            TokenType::Left => {
                self.advance();
                self.expect(TokenType::Join)?;
                JoinType::Left
            }
            TokenType::Right => {
                self.advance();
                self.expect(TokenType::Join)?;
                JoinType::Right
            }
            TokenType::Full => {
                self.advance();
                self.expect(TokenType::Join)?;
                JoinType::Full
            }
            TokenType::Cross => {
                self.advance();
                self.expect(TokenType::Join)?;
                JoinType::Cross
            }
            _ => {
                return Err(ParseError::UnexpectedToken {
                    expected: vec![
                        TokenType::Join,
                        TokenType::Inner,
                        TokenType::Left,
                        TokenType::Right,
                        TokenType::Full,
                        TokenType::Cross,
                    ],
                    found: self.peek()?.clone(),
                });
            }
        };

        let target = self.parse_from_clause()?;

        self.expect(TokenType::On)?;
        let condition = self.parse_expression()?;

        Ok(JoinClause {
            join_type,
            target,
            condition,
        })
    }

    /// Parse ORDER BY list
    fn parse_order_by_list(&mut self) -> Result<Vec<OrderByClause>, ParseError> {
        let mut clauses = Vec::new();

        clauses.push(self.parse_order_by_clause()?);

        while self.matches(&[TokenType::Comma]) {
            self.advance();
            clauses.push(self.parse_order_by_clause()?);
        }

        Ok(clauses)
    }

    /// Parse ORDER BY clause
    fn parse_order_by_clause(&mut self) -> Result<OrderByClause, ParseError> {
        let expression = self.parse_expression()?;

        let direction = if self.matches(&[TokenType::Identifier]) {
            let token = self.peek()?.clone();
            match token.value.to_uppercase().as_str() {
                "ASC" => {
                    self.advance();
                    SortDirection::Asc
                }
                "DESC" => {
                    self.advance();
                    SortDirection::Desc
                }
                _ => SortDirection::Asc, // Default
            }
        } else {
            SortDirection::Asc // Default
        };

        Ok(OrderByClause {
            expression,
            direction,
        })
    }

    /// Parse FETCH list
    fn parse_fetch_list(&mut self) -> Result<Vec<FetchClause>, ParseError> {
        let mut fields = Vec::new();

        fields.push(self.expect_identifier()?.value.clone());

        while self.matches(&[TokenType::Comma]) {
            self.advance();
            fields.push(self.expect_identifier()?.value.clone());
        }

        Ok(vec![FetchClause { fields }])
    }

    /// Parse expressions
    fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_or()
    }

    /// Parse OR expression
    fn parse_or(&mut self) -> Result<Expression, ParseError> {
        let mut expr = self.parse_and()?;

        while self.matches(&[TokenType::Or]) {
            let operator = BinaryOperator::Or;
            self.advance();
            let right = self.parse_and()?;
            expr = Expression::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    /// Parse AND expression
    fn parse_and(&mut self) -> Result<Expression, ParseError> {
        let mut expr = self.parse_equality()?;

        while self.matches(&[TokenType::And]) {
            let operator = BinaryOperator::And;
            self.advance();
            let right = self.parse_equality()?;
            expr = Expression::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    /// Parse equality expression
    fn parse_equality(&mut self) -> Result<Expression, ParseError> {
        let mut expr = self.parse_comparison()?;

        while let Ok(token) = self.peek() {
            let operator = match token.token_type {
                TokenType::Equal => BinaryOperator::Equal,
                TokenType::NotEqual => BinaryOperator::NotEqual,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison()?;
            expr = Expression::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    /// Parse comparison expression
    fn parse_comparison(&mut self) -> Result<Expression, ParseError> {
        let mut expr = self.parse_term()?;

        while let Ok(token) = self.peek() {
            let operator = match token.token_type {
                TokenType::LessThan => BinaryOperator::LessThan,
                TokenType::LessThanOrEqual => BinaryOperator::LessThanOrEqual,
                TokenType::GreaterThan => BinaryOperator::GreaterThan,
                TokenType::GreaterThanOrEqual => BinaryOperator::GreaterThanOrEqual,
                TokenType::Like => BinaryOperator::Like,
                TokenType::ILike => BinaryOperator::ILike,
                TokenType::In => BinaryOperator::In,
                TokenType::Is => {
                    self.advance(); // consume IS

                    // Check for NOT after IS
                    let is_not = if self.matches(&[TokenType::Not]) {
                        self.advance(); // consume NOT
                        true
                    } else {
                        false
                    };

                    let right = self.parse_term()?;

                    let operator = if is_not {
                        BinaryOperator::IsNot
                    } else {
                        BinaryOperator::Is
                    };

                    expr = Expression::Binary {
                        left: Box::new(expr),
                        operator,
                        right: Box::new(right),
                    };
                    continue;
                }
                _ => break,
            };
            self.advance();
            let right = self.parse_term()?;
            expr = Expression::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    /// Parse term expression (addition/subtraction)
    fn parse_term(&mut self) -> Result<Expression, ParseError> {
        let mut expr = self.parse_factor()?;

        while let Ok(token) = self.peek() {
            let operator = match token.token_type {
                TokenType::Plus => BinaryOperator::Add,
                TokenType::Minus => BinaryOperator::Subtract,
                _ => break,
            };
            self.advance();
            let right = self.parse_factor()?;
            expr = Expression::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    /// Parse factor expression (multiplication/division)
    fn parse_factor(&mut self) -> Result<Expression, ParseError> {
        let mut expr = self.parse_unary()?;

        while let Ok(token) = self.peek() {
            let operator = match token.token_type {
                TokenType::Multiply => BinaryOperator::Multiply,
                TokenType::Divide => BinaryOperator::Divide,
                TokenType::Modulo => BinaryOperator::Modulo,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            expr = Expression::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    /// Parse unary expression
    fn parse_unary(&mut self) -> Result<Expression, ParseError> {
        let token = self.peek()?;
        match token.token_type {
            TokenType::Not => {
                self.advance();
                Ok(Expression::Unary {
                    operator: UnaryOperator::Not,
                    operand: Box::new(self.parse_unary()?),
                })
            }
            TokenType::Minus => {
                self.advance();
                Ok(Expression::Unary {
                    operator: UnaryOperator::Minus,
                    operand: Box::new(self.parse_unary()?),
                })
            }
            TokenType::Plus => {
                self.advance();
                Ok(Expression::Unary {
                    operator: UnaryOperator::Plus,
                    operand: Box::new(self.parse_unary()?),
                })
            }
            _ => self.parse_primary(),
        }
    }

    /// Parse primary expression
    fn parse_primary(&mut self) -> Result<Expression, ParseError> {
        let token = self.peek()?.clone();

        match token.token_type {
            TokenType::Integer => {
                self.advance();
                let value =
                    token
                        .value
                        .parse::<i64>()
                        .map_err(|_| ParseError::InvalidExpression {
                            message: format!("Invalid integer: {}", token.value),
                            token: token.clone(),
                        })?;
                Ok(Expression::Literal(QueryValue::Integer(value)))
            }
            TokenType::Float => {
                self.advance();
                let value =
                    token
                        .value
                        .parse::<f64>()
                        .map_err(|_| ParseError::InvalidExpression {
                            message: format!("Invalid float: {}", token.value),
                            token: token.clone(),
                        })?;
                Ok(Expression::Literal(QueryValue::Float(value)))
            }
            TokenType::String => {
                self.advance();
                Ok(Expression::Literal(QueryValue::String(token.value)))
            }
            TokenType::Boolean => {
                self.advance();
                let value = token.value.to_uppercase() == "TRUE";
                Ok(Expression::Literal(QueryValue::Boolean(value)))
            }
            TokenType::Null => {
                self.advance();
                Ok(Expression::Literal(QueryValue::Null))
            }
            TokenType::Parameter => {
                self.advance();
                Ok(Expression::Parameter(token.value))
            }
            TokenType::Identifier => {
                self.advance();
                let token_value = token.value.clone();
                let mut expr = Expression::Identifier(token_value);

                // Handle field access (e.g., user.name)
                while self.matches(&[TokenType::Dot]) {
                    self.advance();
                    let field = self.expect_identifier()?.value.clone();
                    expr = Expression::FieldAccess {
                        object: Box::new(expr),
                        field,
                    };
                }

                // Handle function calls
                if self.matches(&[TokenType::LeftParen]) {
                    self.advance();

                    if let Expression::Identifier(name) = &expr {
                        // Check for aggregate functions with DISTINCT
                        let is_aggregate = matches!(
                            name.to_uppercase().as_str(),
                            "COUNT" | "SUM" | "AVG" | "MIN" | "MAX"
                        );

                        if is_aggregate {
                            // Check for DISTINCT
                            let distinct = if self.matches(&[TokenType::Distinct]) {
                                self.advance();
                                true
                            } else {
                                false
                            };

                            // Parse aggregate expression
                            let expression = if name.to_uppercase() == "COUNT"
                                && self.matches(&[TokenType::Multiply])
                            {
                                self.advance(); // consume *
                                None
                            } else if self.matches(&[TokenType::RightParen]) {
                                None // No argument for COUNT()
                            } else {
                                Some(Box::new(self.parse_expression()?))
                            };

                            self.expect(TokenType::RightParen)?;

                            let aggregate_func = match name.to_uppercase().as_str() {
                                "COUNT" => AggregateFunction::Count,
                                "SUM" => AggregateFunction::Sum,
                                "AVG" => AggregateFunction::Avg,
                                "MIN" => AggregateFunction::Min,
                                "MAX" => AggregateFunction::Max,
                                _ => {
                                    return Err(ParseError::InvalidExpression {
                                        message: format!("Unknown aggregate function: {}", name),
                                        token: token.clone(),
                                    })
                                }
                            };

                            expr = Expression::Aggregate {
                                function: aggregate_func,
                                expression,
                                distinct,
                            };
                        } else {
                            // Regular function call
                            let args = if self.matches(&[TokenType::RightParen]) {
                                Vec::new()
                            } else {
                                self.parse_expression_list()?
                            };
                            self.expect(TokenType::RightParen)?;

                            // Special handling for specific functions
                            match name.to_uppercase().as_str() {
                                "COALESCE" | "ISNULL" | "NVL" => {
                                    // These are null-handling functions
                                    expr = Expression::Function {
                                        name: name.clone(),
                                        args,
                                    };
                                }
                                _ => {
                                    expr = Expression::Function {
                                        name: name.clone(),
                                        args,
                                    };
                                }
                            }
                        }
                    } else {
                        return Err(ParseError::InvalidExpression {
                            message: "Invalid function call".to_string(),
                            token,
                        });
                    }
                }

                Ok(expr)
            }
            TokenType::Now => {
                self.advance();
                // Expect parentheses for NOW()
                self.expect(TokenType::LeftParen)?;
                self.expect(TokenType::RightParen)?;
                Ok(Expression::Function {
                    name: "NOW".to_string(),
                    args: Vec::new(),
                })
            }
            TokenType::Interval => {
                self.advance();
                // Parse INTERVAL '1 day' or INTERVAL '2 hours'
                let interval_str = if self.matches(&[TokenType::String]) {
                    self.advance().value.clone()
                } else {
                    return Err(ParseError::InvalidExpression {
                        message: "INTERVAL must be followed by a string literal".to_string(),
                        token: self.peek()?.clone(),
                    });
                };
                Ok(Expression::Function {
                    name: "INTERVAL".to_string(),
                    args: vec![Expression::Literal(QueryValue::String(interval_str))],
                })
            }
            TokenType::Case => {
                self.advance();
                self.parse_case_expression()
            }
            TokenType::LeftParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(TokenType::RightParen)?;
                Ok(expr)
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: vec![
                    TokenType::Integer,
                    TokenType::Float,
                    TokenType::String,
                    TokenType::Boolean,
                    TokenType::Null,
                    TokenType::Parameter,
                    TokenType::Identifier,
                    TokenType::LeftParen,
                    TokenType::Now,
                    TokenType::Interval,
                    TokenType::Case,
                ],
                found: token,
            }),
        }
    }

    /// Parse expression list (comma-separated)
    fn parse_expression_list(&mut self) -> Result<Vec<Expression>, ParseError> {
        let mut expressions = Vec::new();

        expressions.push(self.parse_expression()?);

        while self.matches(&[TokenType::Comma]) {
            self.advance();
            expressions.push(self.parse_expression()?);
        }

        Ok(expressions)
    }

    /// Parse INSERT statement
    fn parse_insert(&mut self) -> Result<InsertStatement, ParseError> {
        self.expect(TokenType::Insert)?;
        self.expect(TokenType::Into)?;

        let into = self.expect_identifier()?.value.clone();

        // TODO: Implement full INSERT parsing
        Ok(InsertStatement {
            into,
            fields: None,
            values: InsertValues::Object(HashMap::new()),
            on_conflict: None,
        })
    }

    /// Parse UPDATE statement
    fn parse_update(&mut self) -> Result<UpdateStatement, ParseError> {
        self.expect(TokenType::Update)?;
        let table = self.expect_identifier()?.value.clone();

        // TODO: Implement full UPDATE parsing
        Ok(UpdateStatement {
            table,
            assignments: Vec::new(),
            where_clause: None,
        })
    }

    /// Parse DELETE statement
    fn parse_delete(&mut self) -> Result<DeleteStatement, ParseError> {
        self.expect(TokenType::Delete)?;
        self.expect(TokenType::From)?;

        let from = self.expect_identifier()?.value.clone();

        let where_clause = if self.matches(&[TokenType::Where]) {
            self.advance();
            Some(self.parse_expression()?)
        } else {
            None
        };

        Ok(DeleteStatement { from, where_clause })
    }

    /// Parse CREATE statement
    fn parse_create(&mut self) -> Result<CreateStatement, ParseError> {
        self.expect(TokenType::Create)?;

        // TODO: Implement full CREATE parsing
        let object_type = CreateObjectType::Table;
        let name = "temp".to_string();
        let definition = CreateDefinition::Table {
            fields: Vec::new(),
            constraints: Vec::new(),
        };

        Ok(CreateStatement {
            object_type,
            name,
            definition,
        })
    }

    /// Parse DROP statement
    fn parse_drop(&mut self) -> Result<DropStatement, ParseError> {
        self.expect(TokenType::Drop)?;

        // TODO: Implement full DROP parsing
        Ok(DropStatement {
            object_type: CreateObjectType::Table,
            name: "temp".to_string(),
            if_exists: false,
            cascade: false,
        })
    }

    /// Parse transaction statement
    fn parse_transaction(&mut self) -> Result<TransactionStatement, ParseError> {
        let token = self.advance();
        match token.token_type {
            TokenType::Begin => Ok(TransactionStatement::Begin),
            TokenType::Commit => Ok(TransactionStatement::Commit),
            TokenType::Rollback => Ok(TransactionStatement::Rollback),
            _ => Err(ParseError::UnexpectedToken {
                expected: vec![TokenType::Begin, TokenType::Commit, TokenType::Rollback],
                found: token.clone(),
            }),
        }
    }

    /// Parse RELATE statement
    fn parse_relate(&mut self) -> Result<RelateStatement, ParseError> {
        self.expect(TokenType::Relate)?;

        // TODO: Implement full RELATE parsing
        Ok(RelateStatement {
            from: Expression::Identifier("from".to_string()),
            edge_type: "edge".to_string(),
            to: Expression::Identifier("to".to_string()),
            properties: None,
        })
    }

    /// Parse LIVE statement
    fn parse_live(&mut self) -> Result<LiveStatement, ParseError> {
        self.expect(TokenType::Live)?;

        let diff = if self.matches(&[TokenType::Diff]) {
            self.advance();
            true
        } else {
            false
        };

        let query = Box::new(self.parse_select()?);

        Ok(LiveStatement { query, diff })
    }

    // Helper methods

    /// Check if current token matches any of the given types
    fn matches(&self, types: &[TokenType]) -> bool {
        if let Ok(token) = self.peek() {
            types.contains(&token.token_type)
        } else {
            false
        }
    }

    /// Advance to next token and return the previous one
    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    /// Check if we're at the end of tokens
    fn is_at_end(&self) -> bool {
        self.current >= self.tokens.len()
            || self.peek().map_or(true, |t| t.token_type == TokenType::Eof)
    }

    /// Peek at current token
    fn peek(&self) -> Result<&Token, ParseError> {
        if self.current < self.tokens.len() {
            Ok(&self.tokens[self.current])
        } else {
            Err(ParseError::UnexpectedEndOfInput {
                expected: vec![TokenType::Identifier], // Generic expectation
            })
        }
    }

    /// Peek ahead by n tokens
    fn peek_ahead(&self, n: usize) -> Result<&Token, ParseError> {
        let index = self.current + n;
        if index < self.tokens.len() {
            Ok(&self.tokens[index])
        } else {
            Err(ParseError::UnexpectedEndOfInput {
                expected: vec![TokenType::Identifier],
            })
        }
    }

    /// Get previous token
    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }

    /// Expect a specific token type
    fn expect(&mut self, token_type: TokenType) -> Result<&Token, ParseError> {
        if self.matches(std::slice::from_ref(&token_type)) {
            Ok(self.advance())
        } else {
            Err(ParseError::UnexpectedToken {
                expected: vec![token_type],
                found: self.peek()?.clone(),
            })
        }
    }

    /// Expect an identifier token
    fn expect_identifier(&mut self) -> Result<&Token, ParseError> {
        if self.matches(&[TokenType::Identifier]) {
            Ok(self.advance())
        } else {
            Err(ParseError::UnexpectedToken {
                expected: vec![TokenType::Identifier],
                found: self.peek()?.clone(),
            })
        }
    }

    /// Expect an identifier token, allowing certain keywords to be used as identifiers
    fn expect_identifier_or_keyword(&mut self) -> Result<&Token, ParseError> {
        let token = self.peek()?;
        match token.token_type {
            TokenType::Identifier => Ok(self.advance()),
            // Allow certain keywords to be used as table/column names
            TokenType::Metrics
            | TokenType::Aggregate
            | TokenType::Window
            | TokenType::Range
            | TokenType::Node
            | TokenType::Edge
            | TokenType::Path => Ok(self.advance()),
            _ => Err(ParseError::UnexpectedToken {
                expected: vec![TokenType::Identifier],
                found: token.clone(),
            }),
        }
    }

    /// Check if a string is a reserved keyword
    fn is_reserved_keyword(&self, word: &str) -> bool {
        matches!(
            word.to_uppercase().as_str(),
            "SELECT"
                | "FROM"
                | "WHERE"
                | "JOIN"
                | "INNER"
                | "LEFT"
                | "RIGHT"
                | "FULL"
                | "CROSS"
                | "ON"
                | "GROUP"
                | "BY"
                | "HAVING"
                | "ORDER"
                | "LIMIT"
                | "OFFSET"
                | "INSERT"
                | "INTO"
                | "VALUES"
                | "UPDATE"
                | "SET"
                | "DELETE"
                | "CREATE"
                | "DROP"
                | "TABLE"
                | "INDEX"
                | "VIEW"
                | "FUNCTION"
                | "TRIGGER"
                | "SCHEMA"
                | "IF"
                | "EXISTS"
                | "NOT"
                | "AND"
                | "OR"
                | "LIKE"
                | "ILIKE"
                | "IN"
                | "BETWEEN"
                | "IS"
                | "AS"
                | "DISTINCT"
                | "ALL"
                | "ANY"
                | "SOME"
                | "CASE"
                | "WHEN"
                | "THEN"
                | "ELSE"
                | "END"
                | "FETCH"
                | "TIMEOUT"
                | "LIVE"
                | "DIFF"
                | "BEGIN"
                | "COMMIT"
                | "ROLLBACK"
                | "RELATE"
                | "NODE"
                | "EDGE"
                | "PATH"
                | "CONNECTED"
                | "METRICS"
                | "AGGREGATE"
                | "WINDOW"
                | "RANGE"
                | "NOW"
                | "INTERVAL"
                | "WITH"
                | "RECURSIVE"
        )
    }

    /// Parse SELECT statement with CTE support
    fn parse_select_with_cte(&mut self) -> Result<SelectStatement, ParseError> {
        self.expect(TokenType::With)?;

        let mut with_clauses = Vec::new();

        // Parse first CTE
        with_clauses.push(self.parse_with_clause()?);

        // Parse additional CTEs
        while self.matches(&[TokenType::Comma]) {
            self.advance();
            with_clauses.push(self.parse_with_clause()?);
        }

        // Now parse the main SELECT
        self.expect(TokenType::Select)?;
        let fields = self.parse_select_fields()?;

        let from = if self.matches(&[TokenType::From]) {
            self.advance();
            self.parse_from_clauses()?
        } else {
            Vec::new()
        };

        let mut stmt = SelectStatement {
            with_clauses,
            fields,
            from,
            where_clause: None,
            join_clauses: Vec::new(),
            group_by: Vec::new(),
            having: None,
            order_by: Vec::new(),
            limit: None,
            offset: None,
            fetch: Vec::new(),
            timeout: None,
        };

        // Parse the rest of the clauses like normal SELECT
        if self.matches(&[TokenType::Where]) {
            self.advance();
            stmt.where_clause = Some(self.parse_expression()?);
        }

        while self.matches(&[
            TokenType::Join,
            TokenType::Inner,
            TokenType::Left,
            TokenType::Right,
            TokenType::Full,
            TokenType::Cross,
        ]) {
            stmt.join_clauses.push(self.parse_join()?);
        }

        if self.matches(&[TokenType::Group]) {
            self.advance();
            self.expect(TokenType::By)?;
            stmt.group_by = self.parse_expression_list()?;
        }

        if self.matches(&[TokenType::Having]) {
            self.advance();
            stmt.having = Some(self.parse_expression()?);
        }

        if self.matches(&[TokenType::Order]) {
            self.advance();
            self.expect(TokenType::By)?;
            stmt.order_by = self.parse_order_by_list()?;
        }

        if self.matches(&[TokenType::Limit]) {
            self.advance();
            let limit_expr = self.parse_expression()?;
            if let Expression::Literal(QueryValue::Integer(n)) = limit_expr {
                stmt.limit = Some(n as u64);
            } else {
                return Err(ParseError::InvalidExpression {
                    message: "LIMIT must be an integer".to_string(),
                    token: self.previous().clone(),
                });
            }
        }

        if self.matches(&[TokenType::Offset]) {
            self.advance();
            let offset_expr = self.parse_expression()?;
            if let Expression::Literal(QueryValue::Integer(n)) = offset_expr {
                stmt.offset = Some(n as u64);
            } else {
                return Err(ParseError::InvalidExpression {
                    message: "OFFSET must be an integer".to_string(),
                    token: self.previous().clone(),
                });
            }
        }

        if self.matches(&[TokenType::Fetch]) {
            self.advance();
            stmt.fetch = self.parse_fetch_list()?;
        }

        Ok(stmt)
    }

    /// Parse a WITH clause (CTE)
    fn parse_with_clause(&mut self) -> Result<WithClause, ParseError> {
        let recursive = if self.matches(&[TokenType::Recursive]) {
            self.advance();
            true
        } else {
            false
        };

        let name = self.expect_identifier_or_keyword()?.value.clone();

        // Optional column list
        let columns = if self.matches(&[TokenType::LeftParen]) {
            self.advance();
            let mut cols = Vec::new();
            cols.push(self.expect_identifier_or_keyword()?.value.clone());

            while self.matches(&[TokenType::Comma]) {
                self.advance();
                cols.push(self.expect_identifier_or_keyword()?.value.clone());
            }

            self.expect(TokenType::RightParen)?;
            Some(cols)
        } else {
            None
        };

        self.expect(TokenType::As)?;
        self.expect(TokenType::LeftParen)?;

        let query = Box::new(self.parse_select()?);

        self.expect(TokenType::RightParen)?;

        Ok(WithClause {
            name,
            columns,
            query,
            recursive,
        })
    }

    /// Parse CASE expression
    fn parse_case_expression(&mut self) -> Result<Expression, ParseError> {
        let mut when_clauses = Vec::new();

        // Parse WHEN clauses
        while self.matches(&[TokenType::When]) {
            self.advance();
            let condition = self.parse_expression()?;
            self.expect(TokenType::Then)?;
            let result = self.parse_expression()?;

            when_clauses.push(WhenClause { condition, result });
        }

        // Parse optional ELSE clause
        let else_clause = if self.matches(&[TokenType::Else]) {
            self.advance();
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };

        self.expect(TokenType::End)?;

        Ok(Expression::Case {
            when_clauses,
            else_clause,
        })
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbitql::lexer::Lexer;

    #[test]
    fn test_parse_simple_select() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("SELECT * FROM users").unwrap();

        let mut parser = Parser::new();
        let stmt = parser.parse(tokens).unwrap();

        if let Statement::Select(select) = stmt {
            assert_eq!(select.fields.len(), 1);
            assert!(matches!(select.fields[0], SelectField::All));
            assert_eq!(select.from.len(), 1);
            if let FromClause::Table { name, alias } = &select.from[0] {
                assert_eq!(name, "users");
                assert_eq!(*alias, None);
            } else {
                panic!("Expected table from clause");
            }
        } else {
            panic!("Expected SELECT statement");
        }
    }

    #[test]
    fn test_parse_select_with_where() {
        let lexer = Lexer::new();
        let tokens = lexer
            .tokenize("SELECT name FROM users WHERE age > 18")
            .unwrap();

        let mut parser = Parser::new();
        let stmt = parser.parse(tokens).unwrap();

        if let Statement::Select(select) = stmt {
            assert!(select.where_clause.is_some());
            if let Some(Expression::Binary {
                left,
                operator,
                right,
            }) = &select.where_clause
            {
                assert!(matches!(**left, Expression::Identifier(_)));
                assert!(matches!(operator, BinaryOperator::GreaterThan));
                assert!(matches!(
                    **right,
                    Expression::Literal(QueryValue::Integer(18))
                ));
            } else {
                panic!("Expected binary expression in WHERE clause");
            }
        } else {
            panic!("Expected SELECT statement");
        }
    }

    #[test]
    fn test_parse_select_with_limit() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("SELECT * FROM users LIMIT 10").unwrap();

        let mut parser = Parser::new();
        let stmt = parser.parse(tokens).unwrap();

        if let Statement::Select(select) = stmt {
            assert_eq!(select.limit, Some(10));
        } else {
            panic!("Expected SELECT statement");
        }
    }

    #[test]
    fn test_parse_delete() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("DELETE FROM users WHERE age < 13").unwrap();

        let mut parser = Parser::new();
        let stmt = parser.parse(tokens).unwrap();

        if let Statement::Delete(delete) = stmt {
            assert_eq!(delete.from, "users");
            assert!(delete.where_clause.is_some());
        } else {
            panic!("Expected DELETE statement");
        }
    }

    #[test]
    fn test_parse_transaction() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("BEGIN").unwrap();

        let mut parser = Parser::new();
        let stmt = parser.parse(tokens).unwrap();

        if let Statement::Transaction(TransactionStatement::Begin) = stmt {
            // Success
        } else {
            panic!("Expected BEGIN transaction statement");
        }
    }
}
