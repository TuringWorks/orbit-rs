//! Parser for OrbitQL
//!
//! This module provides the `Parser` struct that converts a stream of tokens
//! into an Abstract Syntax Tree (AST) for OrbitQL queries.

use crate::orbitql::ast::{
    AggregateFunction, AlterFieldAction, AlterIndexAction, AlterStatement, AlterTableAction,
    BinaryOperator, CallStatement, Constraint, CreateDefinition, CreateObjectType, CreateStatement,
    DataType, DefineStatement, DeleteStatement, DropStatement, EdgeDirection, Expression,
    FetchClause, FieldConstraint, FieldDefinition, FromClause, FunctionLanguage, FunctionParameter,
    FunctionVolatility, GraphPath, GraphStep, IndexField, InsertStatement, InsertValues,
    JoinClause, JoinType, LiveStatement, MergeAction, MergeStatement, OrderByClause, Parameter,
    ParameterMode, PermissionLevel, Permissions, RelateStatement, RemoveStatement,
    SavepointStatement, SelectField, SelectStatement, SortDirection, Statement,
    TransactionStatement, TraverseStatement, TruncateStatement, UnaryOperator, UpdateAssignment,
    UpdateStatement, UpsertStatement, VectorDistance, WhenClause, WithClause,
};
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
                write!(f, "Unexpected end of input. Expected one of: {expected:?}")
            }
            ParseError::InvalidExpression { message, token } => {
                write!(
                    f,
                    "Invalid expression: {} at line {}, column {}",
                    message, token.line, token.column
                )
            }
            ParseError::LexError(msg) => write!(f, "Lexer error: {msg}"),
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
            TokenType::Traverse => Ok(Statement::Traverse(self.parse_traverse()?)),
            TokenType::Call => Ok(Statement::Call(self.parse_call()?)),
            TokenType::Define => Ok(Statement::Define(Box::new(self.parse_define()?))),
            TokenType::Remove => Ok(Statement::Remove(self.parse_remove()?)),
            TokenType::Upsert => Ok(Statement::Upsert(self.parse_upsert()?)),
            TokenType::Merge => Ok(Statement::Merge(self.parse_merge()?)),
            TokenType::Alter => Ok(Statement::Alter(self.parse_alter()?)),
            TokenType::Truncate => Ok(Statement::Truncate(self.parse_truncate()?)),
            TokenType::Savepoint | TokenType::Release => {
                Ok(Statement::Savepoint(self.parse_savepoint()?))
            }
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
                    TokenType::Traverse,
                    TokenType::Call,
                    TokenType::Define,
                    TokenType::Remove,
                    TokenType::Upsert,
                    TokenType::Merge,
                    TokenType::Alter,
                    TokenType::Truncate,
                    TokenType::Savepoint,
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

        // Check for graph traversal pattern: ->edge_type->node.field
        if self.matches(&[TokenType::ArrowRight]) {
            let path = self.parse_graph_path()?;
            let alias = if self.matches(&[TokenType::As]) {
                self.advance();
                Some(self.expect_identifier()?.value.clone())
            } else {
                None
            };
            return Ok(SelectField::Graph { path, alias });
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
                TokenType::In => {
                    self.advance(); // consume IN
                                    // IN expects a list of values in parentheses: IN (value1, value2, ...)
                    self.expect(TokenType::LeftParen)?;
                    let values = self.parse_expression_list()?;
                    self.expect(TokenType::RightParen)?;

                    // Create an Array expression for the IN values
                    let right = Expression::Array(values);

                    expr = Expression::Binary {
                        left: Box::new(expr),
                        operator: BinaryOperator::In,
                        right: Box::new(right),
                    };
                    continue;
                }
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

        // Check if this is a keyword that can be used as an identifier
        let is_keyword_as_identifier = token.is_keyword() && {
            // Allow keywords to be used as identifiers in expression contexts
            // (table aliases, column names, etc.)
            // Keywords with special parsing rules (like NOW, INTERVAL) must be restricted
            let restricted = [
                "SELECT",
                "FROM",
                "WHERE",
                "JOIN",
                "INNER",
                "LEFT",
                "RIGHT",
                "FULL",
                "CROSS",
                "ON",
                "GROUP",
                "BY",
                "HAVING",
                "ORDER",
                "LIMIT",
                "OFFSET",
                "INSERT",
                "INTO",
                "VALUES",
                "UPDATE",
                "SET",
                "DELETE",
                "CREATE",
                "DROP",
                "TABLE",
                "AS",
                "AND",
                "OR",
                "NOT",
                "IN",
                "BETWEEN",
                "IS",
                "LIKE",
                "CASE",
                "WHEN",
                "THEN",
                "ELSE",
                "END",
                "WITH",
                "RECURSIVE",
                "NOW",
                "INTERVAL",
            ];
            !restricted.contains(&token.value.to_uppercase().as_str())
        };

        // Handle identifiers and keywords that can be used as identifiers
        if token.token_type == TokenType::Identifier || is_keyword_as_identifier {
            self.advance();
            let mut token_value = token.value.clone();
            let mut expr = Expression::Identifier(token_value.clone());

            // Handle namespaced function calls (e.g., time::now(), string::len())
            while self.matches(&[TokenType::DoubleColon]) {
                self.advance();
                let next_token = self.expect_identifier_or_keyword()?;
                token_value = format!("{}::{}", token_value, next_token.value);
                expr = Expression::Identifier(token_value.clone());
            }

            // Handle record IDs (e.g., user:user1, post:123)
            if self.matches(&[TokenType::Colon]) {
                self.advance();
                let record_id = self.expect_identifier_or_keyword()?.value.clone();
                token_value = format!("{}:{}", token_value, record_id);
                expr = Expression::Identifier(token_value.clone());
            }

            // Handle field access (e.g., user.name, f.from, t.to, target.name)
            // Allow reserved keywords as field names after dot
            while self.matches(&[TokenType::Dot]) {
                self.advance();
                let field = self.expect_field_name()?.value.clone();
                expr = Expression::FieldAccess {
                    object: Box::new(expr),
                    field,
                };
            }

            // Handle function calls
            if self.matches(&[TokenType::LeftParen]) {
                self.advance();

                if let Expression::Identifier(name) = &expr {
                    // Check for aggregate functions
                    let is_aggregate = matches!(
                        name.to_uppercase().as_str(),
                        "COUNT" | "SUM" | "AVG" | "MIN" | "MAX" | "ARRAY_AGG" | "STRING_AGG"
                    );

                    if is_aggregate {
                        // Check for DISTINCT
                        let distinct = if self.matches(&[TokenType::Distinct]) {
                            self.advance();
                            true
                        } else {
                            false
                        };

                        // Check for * (as in COUNT(*))
                        let args = if self.matches(&[TokenType::Multiply]) {
                            self.advance();
                            Vec::new() // Empty args indicates *
                        } else if self.matches(&[TokenType::RightParen]) {
                            // Empty argument list like COUNT()
                            Vec::new()
                        } else {
                            self.parse_expression_list()?
                        };

                        self.expect(TokenType::RightParen)?;

                        return Ok(Expression::Aggregate {
                            function: match name.to_uppercase().as_str() {
                                "COUNT" => AggregateFunction::Count,
                                "SUM" => AggregateFunction::Sum,
                                "AVG" => AggregateFunction::Avg,
                                "MIN" => AggregateFunction::Min,
                                "MAX" => AggregateFunction::Max,
                                "ARRAY_AGG" => AggregateFunction::ArrayAgg,
                                "STRING_AGG" => AggregateFunction::StringAgg,
                                _ => unreachable!(),
                            },
                            args,
                            distinct,
                        });
                    }

                    // Regular function call
                    let args = if self.matches(&[TokenType::RightParen]) {
                        Vec::new()
                    } else {
                        let args = self.parse_expression_list()?;
                        self.expect(TokenType::RightParen)?;
                        args
                    };

                    return Ok(Expression::Function {
                        name: name.clone(),
                        args,
                    });
                }
            }

            return Ok(expr);
        }

        // Handle other token types
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
            TokenType::DurationLiteral => {
                self.advance();
                // Parse duration literal like "3h", "2m", "1d", "30s", "100ms"
                let duration = Self::parse_duration_literal(&token.value).map_err(|_| {
                    ParseError::InvalidExpression {
                        message: format!("Invalid duration literal: {}", token.value),
                        token: token.clone(),
                    }
                })?;
                Ok(Expression::Literal(QueryValue::Duration(duration)))
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
    /// Supports: CREATE [OR REPLACE] TABLE|INDEX|VIEW|FUNCTION|PROCEDURE ...
    fn parse_create(&mut self) -> Result<CreateStatement, ParseError> {
        self.expect(TokenType::Create)?;

        // Check for OR REPLACE
        let or_replace = if self.matches(&[TokenType::Or]) {
            self.advance();
            self.expect(TokenType::Replace)?;
            true
        } else {
            false
        };

        let token = self.peek()?;
        match token.token_type {
            TokenType::Table => self.parse_create_table(),
            TokenType::Index => self.parse_create_index(),
            TokenType::View => self.parse_create_view(),
            TokenType::Function => self.parse_create_function(or_replace),
            TokenType::Procedure => self.parse_create_procedure(or_replace),
            TokenType::Schema => self.parse_create_schema(),
            _ => Err(ParseError::UnexpectedToken {
                expected: vec![
                    TokenType::Table,
                    TokenType::Index,
                    TokenType::View,
                    TokenType::Function,
                    TokenType::Procedure,
                    TokenType::Schema,
                ],
                found: token.clone(),
            }),
        }
    }

    /// Parse CREATE TABLE statement
    fn parse_create_table(&mut self) -> Result<CreateStatement, ParseError> {
        self.expect(TokenType::Table)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();

        // Parse column definitions
        self.expect(TokenType::LeftParen)?;
        let mut fields = Vec::new();
        let mut constraints = Vec::new();

        loop {
            // Check for table-level constraints
            if self.matches(&[
                TokenType::Primary,
                TokenType::Foreign,
                TokenType::Unique,
                TokenType::Check,
                TokenType::Constraint,
            ]) {
                constraints.push(self.parse_table_constraint()?);
            } else {
                // Parse column definition
                fields.push(self.parse_field_definition()?);
            }

            if self.matches(&[TokenType::Comma]) {
                self.advance();
            } else {
                break;
            }
        }

        self.expect(TokenType::RightParen)?;

        Ok(CreateStatement {
            object_type: CreateObjectType::Table,
            name,
            definition: CreateDefinition::Table {
                fields,
                constraints,
            },
        })
    }

    /// Parse CREATE INDEX statement
    fn parse_create_index(&mut self) -> Result<CreateStatement, ParseError> {
        let unique = if self.matches(&[TokenType::Unique]) {
            self.advance();
            true
        } else {
            false
        };

        self.expect(TokenType::Index)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();

        self.expect(TokenType::On)?;
        let table_name = self.expect_identifier_or_keyword()?.value.clone();

        self.expect(TokenType::LeftParen)?;
        let mut index_fields = Vec::new();
        loop {
            let field = self.expect_identifier_or_keyword()?.value.clone();
            index_fields.push(field);
            if self.matches(&[TokenType::Comma]) {
                self.advance();
            } else {
                break;
            }
        }
        self.expect(TokenType::RightParen)?;

        Ok(CreateStatement {
            object_type: CreateObjectType::Index,
            name,
            definition: CreateDefinition::Index {
                on: table_name,
                fields: index_fields,
                unique,
            },
        })
    }

    /// Parse CREATE VIEW statement
    fn parse_create_view(&mut self) -> Result<CreateStatement, ParseError> {
        self.expect(TokenType::View)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();

        self.expect(TokenType::As)?;
        let query = self.parse_select()?;

        Ok(CreateStatement {
            object_type: CreateObjectType::View,
            name,
            definition: CreateDefinition::View {
                query: Box::new(query),
            },
        })
    }

    /// Parse CREATE FUNCTION statement
    /// Syntax: CREATE [OR REPLACE] FUNCTION name(params) RETURNS type [LANGUAGE lang] [IMMUTABLE|STABLE|VOLATILE] AS $$ body $$
    fn parse_create_function(&mut self, or_replace: bool) -> Result<CreateStatement, ParseError> {
        self.expect(TokenType::Function)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();

        // Parse parameters
        self.expect(TokenType::LeftParen)?;
        let parameters = self.parse_function_parameters()?;
        self.expect(TokenType::RightParen)?;

        // Parse RETURNS clause
        self.expect(TokenType::Returns)?;
        let return_type = self.parse_data_type()?;

        // Parse optional LANGUAGE clause
        let language = if self.matches(&[TokenType::Language]) {
            self.advance();
            Some(self.parse_function_language()?)
        } else {
            None
        };

        // Parse optional volatility (IMMUTABLE, STABLE, VOLATILE)
        let volatility = self.parse_function_volatility()?;

        // Parse AS keyword and body
        self.expect(TokenType::As)?;
        let body = self.parse_function_body()?;

        Ok(CreateStatement {
            object_type: CreateObjectType::Function,
            name,
            definition: CreateDefinition::Function {
                or_replace,
                parameters,
                return_type,
                language,
                volatility,
                body,
            },
        })
    }

    /// Parse CREATE PROCEDURE statement
    /// Syntax: CREATE [OR REPLACE] PROCEDURE name(params) [LANGUAGE lang] AS $$ body $$
    fn parse_create_procedure(&mut self, or_replace: bool) -> Result<CreateStatement, ParseError> {
        self.expect(TokenType::Procedure)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();

        // Parse parameters
        self.expect(TokenType::LeftParen)?;
        let parameters = self.parse_function_parameters()?;
        self.expect(TokenType::RightParen)?;

        // Parse optional LANGUAGE clause
        let language = if self.matches(&[TokenType::Language]) {
            self.advance();
            Some(self.parse_function_language()?)
        } else {
            None
        };

        // Parse AS keyword and body
        self.expect(TokenType::As)?;
        let body = self.parse_function_body()?;

        Ok(CreateStatement {
            object_type: CreateObjectType::Procedure,
            name,
            definition: CreateDefinition::Procedure {
                or_replace,
                parameters,
                language,
                body,
            },
        })
    }

    /// Parse CREATE SCHEMA statement
    fn parse_create_schema(&mut self) -> Result<CreateStatement, ParseError> {
        self.expect(TokenType::Schema)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();

        Ok(CreateStatement {
            object_type: CreateObjectType::Schema,
            name,
            definition: CreateDefinition::Table {
                fields: Vec::new(),
                constraints: Vec::new(),
            },
        })
    }

    /// Parse function parameters
    fn parse_function_parameters(&mut self) -> Result<Vec<Parameter>, ParseError> {
        let mut parameters = Vec::new();

        if self.matches(&[TokenType::RightParen]) {
            return Ok(parameters);
        }

        loop {
            // Check for parameter mode (IN, OUT, INOUT, VARIADIC)
            // Note: IN is already a token (for IN clause), OUT is separate
            let mode = if self.check_identifier_value("IN") {
                self.advance();
                if self.matches(&[TokenType::Out]) {
                    self.advance();
                    Some(ParameterMode::InOut)
                } else {
                    Some(ParameterMode::In)
                }
            } else if self.matches(&[TokenType::Out]) {
                self.advance();
                Some(ParameterMode::Out)
            } else {
                None
            };

            let name = self.expect_identifier_or_keyword()?.value.clone();
            let data_type = self.parse_data_type()?;

            // Check for DEFAULT clause
            let default = if self.matches(&[TokenType::Default]) {
                self.advance();
                Some(self.parse_expression()?)
            } else {
                None
            };

            parameters.push(Parameter {
                name,
                data_type,
                default,
                mode,
            });

            if self.matches(&[TokenType::Comma]) {
                self.advance();
            } else {
                break;
            }
        }

        Ok(parameters)
    }

    /// Parse function language (SQL, ORBITQL, JAVASCRIPT, PYTHON, etc.)
    fn parse_function_language(&mut self) -> Result<FunctionLanguage, ParseError> {
        let token = self.advance();
        let lang_str = token.value.to_uppercase();

        Ok(match lang_str.as_str() {
            "SQL" => FunctionLanguage::Sql,
            "ORBITQL" => FunctionLanguage::OrbitQL,
            "JAVASCRIPT" | "JS" => FunctionLanguage::JavaScript,
            "PYTHON" | "PY" => FunctionLanguage::Python,
            _ => FunctionLanguage::Other(token.value.clone()),
        })
    }

    /// Parse function volatility (IMMUTABLE, STABLE, VOLATILE)
    fn parse_function_volatility(&mut self) -> Result<Option<FunctionVolatility>, ParseError> {
        if self.matches(&[TokenType::Immutable]) {
            self.advance();
            Ok(Some(FunctionVolatility::Immutable))
        } else if self.matches(&[TokenType::Stable]) {
            self.advance();
            Ok(Some(FunctionVolatility::Stable))
        } else if self.matches(&[TokenType::Volatile]) {
            self.advance();
            Ok(Some(FunctionVolatility::Volatile))
        } else {
            Ok(None)
        }
    }

    /// Parse function body (string literal, either quoted or $$ delimited)
    fn parse_function_body(&mut self) -> Result<String, ParseError> {
        let token = self.peek()?;
        let token_type = token.token_type.clone();
        let token_value = token.value.clone();

        // Handle PostgreSQL-style $$ ... $$ delimiter
        // The lexer tokenizes $ as TokenType::Parameter with value "$"
        if (token_type == TokenType::Parameter && token_value == "$")
            || token_type == TokenType::Dollar
        {
            self.advance(); // consume first $

            // Check if next is also $ (forming $$)
            let next = self.peek()?;
            let is_dollar_quote = (next.token_type == TokenType::Parameter && next.value == "$")
                || next.token_type == TokenType::Dollar;

            if is_dollar_quote {
                self.advance(); // consume second $

                // Collect all tokens until we find $$
                let mut body_tokens = Vec::new();
                loop {
                    let current = self.peek()?;
                    if current.token_type == TokenType::Eof {
                        return Err(ParseError::UnexpectedEndOfInput {
                            expected: vec![TokenType::Parameter],
                        });
                    }

                    // Check for closing $$
                    let is_closing_dollar = (current.token_type == TokenType::Parameter
                        && current.value == "$")
                        || current.token_type == TokenType::Dollar;
                    if is_closing_dollar {
                        let next_pos = self.current + 1;
                        if next_pos < self.tokens.len() {
                            let next_token = &self.tokens[next_pos];
                            let next_is_dollar = (next_token.token_type == TokenType::Parameter
                                && next_token.value == "$")
                                || next_token.token_type == TokenType::Dollar;
                            if next_is_dollar {
                                self.advance(); // consume first $
                                self.advance(); // consume second $
                                break;
                            }
                        }
                    }

                    body_tokens.push(self.advance().value.clone());
                }

                return Ok(body_tokens.join(" "));
            }

            // Single $ - return just that
            return Ok(token_value);
        }

        // Handle regular string
        if token_type == TokenType::String {
            return Ok(self.advance().value.clone());
        }

        // Accept any token value as body for flexibility
        Ok(self.advance().value.clone())
    }

    /// Parse field definition for CREATE TABLE
    fn parse_field_definition(&mut self) -> Result<FieldDefinition, ParseError> {
        let name = self.expect_identifier_or_keyword()?.value.clone();
        let data_type = self.parse_data_type()?;

        let mut nullable = true;
        let mut default = None;
        let mut constraints = Vec::new();

        // Parse column constraints
        while self.matches(&[
            TokenType::Not,
            TokenType::Default,
            TokenType::Primary,
            TokenType::Unique,
            TokenType::Check,
            TokenType::References,
        ]) {
            if self.matches(&[TokenType::Not]) {
                self.advance();
                self.expect(TokenType::Null)?;
                nullable = false;
                constraints.push(FieldConstraint::NotNull);
            } else if self.matches(&[TokenType::Default]) {
                self.advance();
                default = Some(self.parse_expression()?);
            } else if self.matches(&[TokenType::Primary]) {
                self.advance();
                self.expect(TokenType::Key)?;
                // Primary key implies NOT NULL
                nullable = false;
            } else if self.matches(&[TokenType::Unique]) {
                self.advance();
                constraints.push(FieldConstraint::Unique);
            } else if self.matches(&[TokenType::Check]) {
                self.advance();
                self.expect(TokenType::LeftParen)?;
                let expr = self.parse_expression()?;
                self.expect(TokenType::RightParen)?;
                constraints.push(FieldConstraint::Check(expr));
            } else if self.matches(&[TokenType::References]) {
                self.advance();
                let table = self.expect_identifier_or_keyword()?.value.clone();
                self.expect(TokenType::LeftParen)?;
                let field = self.expect_identifier_or_keyword()?.value.clone();
                self.expect(TokenType::RightParen)?;
                constraints.push(FieldConstraint::ForeignKey { table, field });
            }
        }

        Ok(FieldDefinition {
            name,
            data_type,
            nullable,
            default,
            constraints,
        })
    }

    /// Parse table-level constraint
    fn parse_table_constraint(&mut self) -> Result<Constraint, ParseError> {
        // Skip optional CONSTRAINT name
        if self.matches(&[TokenType::Constraint]) {
            self.advance();
            let _constraint_name = self.expect_identifier_or_keyword()?.value.clone();
        }

        if self.matches(&[TokenType::Primary]) {
            self.advance();
            self.expect(TokenType::Key)?;
            self.expect(TokenType::LeftParen)?;
            let mut columns = Vec::new();
            loop {
                columns.push(self.expect_identifier_or_keyword()?.value.clone());
                if self.matches(&[TokenType::Comma]) {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(TokenType::RightParen)?;
            Ok(Constraint::PrimaryKey(columns))
        } else if self.matches(&[TokenType::Unique]) {
            self.advance();
            self.expect(TokenType::LeftParen)?;
            let mut columns = Vec::new();
            loop {
                columns.push(self.expect_identifier_or_keyword()?.value.clone());
                if self.matches(&[TokenType::Comma]) {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(TokenType::RightParen)?;
            Ok(Constraint::Unique(columns))
        } else if self.matches(&[TokenType::Foreign]) {
            self.advance();
            self.expect(TokenType::Key)?;
            self.expect(TokenType::LeftParen)?;
            let mut fields = Vec::new();
            loop {
                fields.push(self.expect_identifier_or_keyword()?.value.clone());
                if self.matches(&[TokenType::Comma]) {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(TokenType::RightParen)?;
            self.expect(TokenType::References)?;
            let references_table = self.expect_identifier_or_keyword()?.value.clone();
            self.expect(TokenType::LeftParen)?;
            let mut references_fields = Vec::new();
            loop {
                references_fields.push(self.expect_identifier_or_keyword()?.value.clone());
                if self.matches(&[TokenType::Comma]) {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(TokenType::RightParen)?;
            Ok(Constraint::ForeignKey {
                fields,
                references_table,
                references_fields,
            })
        } else if self.matches(&[TokenType::Check]) {
            self.advance();
            self.expect(TokenType::LeftParen)?;
            let expr = self.parse_expression()?;
            self.expect(TokenType::RightParen)?;
            Ok(Constraint::Check(expr))
        } else {
            let token = self.peek()?;
            Err(ParseError::UnexpectedToken {
                expected: vec![
                    TokenType::Primary,
                    TokenType::Unique,
                    TokenType::Foreign,
                    TokenType::Check,
                ],
                found: token.clone(),
            })
        }
    }

    /// Parse data type
    fn parse_data_type(&mut self) -> Result<DataType, ParseError> {
        let token = self.advance();
        let type_name = token.value.to_uppercase();

        Ok(match type_name.as_str() {
            "BOOLEAN" | "BOOL" => DataType::Boolean,
            "INTEGER" | "INT" | "INT4" | "BIGINT" | "INT8" | "SMALLINT" | "INT2" => {
                DataType::Integer
            }
            "FLOAT" | "DOUBLE" | "REAL" | "FLOAT4" | "FLOAT8" | "DECIMAL" | "NUMERIC" => {
                DataType::Float
            }
            "STRING" | "TEXT" | "VARCHAR" | "CHAR" => {
                // Check for length specification
                if self.matches(&[TokenType::LeftParen]) {
                    self.advance();
                    let len_token = self.advance();
                    let max_length = len_token.value.parse::<u32>().ok();
                    self.expect(TokenType::RightParen)?;
                    DataType::String { max_length }
                } else {
                    DataType::String { max_length: None }
                }
            }
            "DATETIME" | "TIMESTAMP" | "TIMESTAMPTZ" | "DATE" | "TIME" => DataType::DateTime,
            "DURATION" | "INTERVAL" => DataType::Duration,
            "UUID" => DataType::Uuid,
            "JSON" | "JSONB" => DataType::Json,
            "OBJECT" | "RECORD" => DataType::Object,
            "GEOMETRY" => DataType::Geometry,
            "POINT" => DataType::Point,
            "LINESTRING" => DataType::LineString,
            "POLYGON" => DataType::Polygon,
            "MULTIPOINT" => DataType::MultiPoint,
            "MULTILINESTRING" => DataType::MultiLineString,
            "MULTIPOLYGON" => DataType::MultiPolygon,
            "GEOMETRYCOLLECTION" => DataType::GeometryCollection,
            "ARRAY" => {
                // Parse ARRAY<type> syntax
                if self.matches(&[TokenType::LessThan]) {
                    self.advance();
                    let inner_type = self.parse_data_type()?;
                    self.expect(TokenType::GreaterThan)?;
                    DataType::Array(Box::new(inner_type))
                } else {
                    DataType::Array(Box::new(DataType::Any))
                }
            }
            _ => DataType::Any,
        })
    }

    /// Parse DROP statement
    fn parse_drop(&mut self) -> Result<DropStatement, ParseError> {
        self.expect(TokenType::Drop)?;

        // Check for IF EXISTS
        let if_exists = if self.matches(&[TokenType::If]) {
            self.advance();
            self.expect(TokenType::Exists)?;
            true
        } else {
            false
        };

        // Parse object type
        let token = self.peek()?;
        let object_type = match token.token_type {
            TokenType::Table => {
                self.advance();
                CreateObjectType::Table
            }
            TokenType::Index => {
                self.advance();
                CreateObjectType::Index
            }
            TokenType::View => {
                self.advance();
                CreateObjectType::View
            }
            TokenType::Function => {
                self.advance();
                CreateObjectType::Function
            }
            TokenType::Procedure => {
                self.advance();
                CreateObjectType::Procedure
            }
            TokenType::Schema => {
                self.advance();
                CreateObjectType::Schema
            }
            _ => {
                return Err(ParseError::UnexpectedToken {
                    expected: vec![
                        TokenType::Table,
                        TokenType::Index,
                        TokenType::View,
                        TokenType::Function,
                        TokenType::Procedure,
                        TokenType::Schema,
                    ],
                    found: token.clone(),
                });
            }
        };

        let name = self.expect_identifier_or_keyword()?.value.clone();

        // Check for CASCADE or RESTRICT
        let cascade = if self.matches(&[TokenType::Cascade]) {
            self.advance();
            true
        } else if self.matches(&[TokenType::Restrict]) {
            self.advance();
            false
        } else {
            false
        };

        Ok(DropStatement {
            object_type,
            name,
            if_exists,
            cascade,
        })
    }

    /// Parse CALL statement
    /// Syntax: CALL procedure_name(arg1, arg2, ...)
    fn parse_call(&mut self) -> Result<CallStatement, ParseError> {
        self.expect(TokenType::Call)?;

        let procedure_name = self.expect_identifier_or_keyword()?.value.clone();

        self.expect(TokenType::LeftParen)?;

        let mut arguments = Vec::new();
        if !self.matches(&[TokenType::RightParen]) {
            loop {
                let expr = self.parse_expression()?;
                arguments.push(expr);

                if self.matches(&[TokenType::Comma]) {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        self.expect(TokenType::RightParen)?;

        Ok(CallStatement {
            procedure_name,
            arguments,
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
    /// Syntax: RELATE <from_node>-><edge_type>-><to_node> [SET <properties>]
    fn parse_relate(&mut self) -> Result<RelateStatement, ParseError> {
        self.expect(TokenType::Relate)?;

        // Parse from node (e.g., user:user1)
        let from = self.parse_expression()?;

        // Expect -> arrow
        self.expect(TokenType::ArrowRight)?;

        // Parse edge type
        let edge_type = self.expect_identifier_or_keyword()?.value.clone();

        // Expect -> arrow
        self.expect(TokenType::ArrowRight)?;

        // Parse to node (e.g., user:user2)
        let to = self.parse_expression()?;

        // Optional SET clause for properties
        let properties = if self.matches(&[TokenType::Set]) {
            self.advance();
            // Skip the properties block (we accept anything between { })
            if self.matches(&[TokenType::LeftBrace]) {
                self.advance();
                // Skip until we find the closing brace
                while !self.matches(&[TokenType::RightBrace]) && !self.is_at_end() {
                    self.advance();
                }
                self.expect(TokenType::RightBrace)?;
            }
            // Return empty properties map (properties are skipped for now)
            Some(std::collections::HashMap::new())
        } else {
            None
        };

        Ok(RelateStatement {
            from,
            edge_type,
            to,
            properties,
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

    /// Parse TRAVERSE statement
    /// Syntax: TRAVERSE <edge_type> FROM <node_expr> [MAX_DEPTH <n>] [WHERE <condition>]
    fn parse_traverse(&mut self) -> Result<TraverseStatement, ParseError> {
        self.expect(TokenType::Traverse)?;

        // Parse edge type (e.g., "follows")
        let edge_type = self.expect_identifier_or_keyword()?.value.clone();

        // Expect FROM keyword
        self.expect(TokenType::From)?;

        // Parse starting node expression (e.g., "user:user1")
        let from_node = self.parse_expression()?;

        // Parse optional MAX_DEPTH
        let max_depth = if self.matches(&[TokenType::MaxDepth]) {
            self.advance();
            let depth_expr = self.parse_expression()?;
            if let Expression::Literal(QueryValue::Integer(n)) = depth_expr {
                Some(n as u32)
            } else {
                return Err(ParseError::InvalidExpression {
                    message: "MAX_DEPTH must be an integer".to_string(),
                    token: self.previous().clone(),
                });
            }
        } else {
            None
        };

        // Parse optional WHERE clause
        let where_clause = if self.matches(&[TokenType::Where]) {
            self.advance();
            Some(self.parse_expression()?)
        } else {
            None
        };

        Ok(TraverseStatement {
            edge_type,
            from_node,
            max_depth,
            where_clause,
        })
    }

    /// Parse a graph path expression like ->follows->user.name
    /// Used in SELECT fields for graph traversals
    fn parse_graph_path(&mut self) -> Result<GraphPath, ParseError> {
        let mut steps = Vec::new();

        // Parse sequence of edge traversals: ->edge_type->node.field
        while self.matches(&[TokenType::ArrowRight, TokenType::ArrowLeft]) {
            let direction = if self.peek()?.token_type == TokenType::ArrowRight {
                EdgeDirection::Outgoing
            } else {
                EdgeDirection::Incoming
            };
            self.advance(); // consume -> or <-

            // Parse edge type or node label
            let label = self.expect_identifier_or_keyword()?.value.clone();

            // Add edge step
            steps.push(GraphStep::Edge {
                direction,
                label: Some(label.clone()),
                properties: None,
            });

            // Check if there's a node access after this (e.g., ->user.name)
            // If no more arrows, this is the final node
            if !self.matches(&[TokenType::ArrowRight, TokenType::ArrowLeft]) {
                // Check for field access (e.g., .name)
                if self.matches(&[TokenType::Dot]) {
                    self.advance();
                    let field = self.expect_identifier_or_keyword()?.value.clone();
                    // Create a node step with the field as a property expression
                    steps.push(GraphStep::Node {
                        label: Some(label),
                        properties: Some(Expression::Identifier(field)),
                    });
                } else {
                    // Just a node reference
                    steps.push(GraphStep::Node {
                        label: Some(label),
                        properties: None,
                    });
                }
            }
        }

        Ok(GraphPath { steps })
    }

    /// Parse a duration literal string like "3h", "2m", "1d", "30s", "100ms"
    fn parse_duration_literal(s: &str) -> Result<std::time::Duration, &'static str> {
        let s = s.trim();
        // Find where the numeric part ends
        let num_end = s
            .chars()
            .position(|c| !c.is_ascii_digit())
            .unwrap_or(s.len());
        if num_end == 0 {
            return Err("No numeric value found");
        }
        let num_str = &s[..num_end];
        let suffix = &s[num_end..];

        let value: u64 = num_str.parse().map_err(|_| "Invalid number")?;

        let duration = match suffix {
            "ns" => std::time::Duration::from_nanos(value),
            "us" | "µs" => std::time::Duration::from_micros(value),
            "ms" => std::time::Duration::from_millis(value),
            "s" => std::time::Duration::from_secs(value),
            "m" => std::time::Duration::from_secs(value * 60),
            "h" => std::time::Duration::from_secs(value * 3600),
            "d" => std::time::Duration::from_secs(value * 86400),
            _ => return Err("Unknown duration suffix"),
        };
        Ok(duration)
    }

    /// Parse DEFINE statement (SurrealDB-style schema definition)
    /// Supports: DEFINE NAMESPACE, DATABASE, TABLE, FIELD, INDEX, EVENT, FUNCTION, ANALYZER, USER, SCOPE, PARAM
    fn parse_define(&mut self) -> Result<DefineStatement, ParseError> {
        self.expect(TokenType::Define)?;

        let token = self.peek()?;
        match token.token_type {
            TokenType::Namespace => self.parse_define_namespace(),
            TokenType::Database => self.parse_define_database(),
            TokenType::Table => self.parse_define_table(),
            TokenType::Field => self.parse_define_field(),
            TokenType::Index => self.parse_define_index(),
            TokenType::Event => self.parse_define_event(),
            TokenType::Function => self.parse_define_function(),
            TokenType::Analyzer => self.parse_define_analyzer(),
            TokenType::User => self.parse_define_user(),
            TokenType::Scope => self.parse_define_scope(),
            TokenType::Param => self.parse_define_param(),
            _ => Err(ParseError::UnexpectedToken {
                expected: vec![
                    TokenType::Namespace,
                    TokenType::Database,
                    TokenType::Table,
                    TokenType::Field,
                    TokenType::Index,
                    TokenType::Event,
                    TokenType::Function,
                    TokenType::Analyzer,
                    TokenType::User,
                    TokenType::Scope,
                    TokenType::Param,
                ],
                found: token.clone(),
            }),
        }
    }

    /// Parse DEFINE NAMESPACE name
    fn parse_define_namespace(&mut self) -> Result<DefineStatement, ParseError> {
        self.expect(TokenType::Namespace)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();
        Ok(DefineStatement::Namespace { name })
    }

    /// Parse DEFINE DATABASE name
    fn parse_define_database(&mut self) -> Result<DefineStatement, ParseError> {
        self.expect(TokenType::Database)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();
        Ok(DefineStatement::Database { name })
    }

    /// Parse DEFINE TABLE name [SCHEMAFULL|SCHEMALESS] [DROP] [AS SELECT ...] [PERMISSIONS ...]
    fn parse_define_table(&mut self) -> Result<DefineStatement, ParseError> {
        self.expect(TokenType::Table)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();

        // Parse optional SCHEMAFULL or SCHEMALESS
        let schemafull = if self.matches(&[TokenType::Schemafull]) {
            self.advance();
            true
        } else if self.matches(&[TokenType::Schemaless]) {
            self.advance();
            false
        } else {
            false
        };

        // Parse optional DROP
        let drop = if self.matches(&[TokenType::Drop]) {
            self.advance();
            true
        } else {
            false
        };

        // Parse optional AS SELECT
        let as_select = if self.matches(&[TokenType::As]) {
            self.advance();
            self.expect(TokenType::Select)?;
            // Rewind one token to let parse_select handle it
            self.current -= 1;
            Some(Box::new(self.parse_select()?))
        } else {
            None
        };

        // Parse optional PERMISSIONS
        let permissions = self.parse_optional_permissions()?;

        Ok(DefineStatement::Table {
            name,
            schemafull,
            drop,
            as_select,
            permissions,
        })
    }

    /// Parse DEFINE FIELD name ON [TABLE] table [TYPE type] [DEFAULT expr] [ASSERT expr] [PERMISSIONS ...]
    fn parse_define_field(&mut self) -> Result<DefineStatement, ParseError> {
        self.expect(TokenType::Field)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();

        self.expect(TokenType::On)?;
        // Optional TABLE keyword
        if self.matches(&[TokenType::Table]) {
            self.advance();
        }
        let table = self.expect_identifier_or_keyword()?.value.clone();

        // Parse optional TYPE
        let data_type = if self.check_identifier_value("TYPE") {
            self.advance();
            Some(self.parse_data_type()?)
        } else {
            None
        };

        // Parse optional DEFAULT
        let default = if self.matches(&[TokenType::Default]) {
            self.advance();
            Some(self.parse_expression()?)
        } else {
            None
        };

        // Parse optional ASSERT
        let assert = if self.check_identifier_value("ASSERT") {
            self.advance();
            Some(self.parse_expression()?)
        } else {
            None
        };

        // Parse optional PERMISSIONS
        let permissions = self.parse_optional_permissions()?;

        Ok(DefineStatement::Field {
            name,
            table,
            data_type,
            default,
            assert,
            permissions,
        })
    }

    /// Parse DEFINE INDEX name ON [TABLE] table FIELDS|COLUMNS field1, field2 [UNIQUE] [SEARCH ANALYZER name]
    fn parse_define_index(&mut self) -> Result<DefineStatement, ParseError> {
        self.expect(TokenType::Index)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();

        self.expect(TokenType::On)?;
        // Optional TABLE keyword
        if self.matches(&[TokenType::Table]) {
            self.advance();
        }
        let table = self.expect_identifier_or_keyword()?.value.clone();

        // FIELDS or COLUMNS keyword
        if self.check_identifier_value("FIELDS") || self.check_identifier_value("COLUMNS") {
            self.advance();
        }

        // Parse field list
        let mut fields = Vec::new();
        loop {
            let field_name = self.expect_identifier_or_keyword()?.value.clone();
            let direction = if self.check_identifier_value("ASC") {
                self.advance();
                Some(SortDirection::Asc)
            } else if self.check_identifier_value("DESC") {
                self.advance();
                Some(SortDirection::Desc)
            } else {
                None
            };
            fields.push(IndexField {
                name: field_name,
                direction,
            });

            if !self.matches(&[TokenType::Comma]) {
                break;
            }
            self.advance();
        }

        // Parse optional UNIQUE
        let unique = if self.matches(&[TokenType::Unique]) {
            self.advance();
            true
        } else {
            false
        };

        // Parse optional SEARCH ANALYZER
        let search_analyzer = if self.check_identifier_value("SEARCH") {
            self.advance();
            if self.matches(&[TokenType::Analyzer]) {
                self.advance();
                Some(self.expect_identifier_or_keyword()?.value.clone())
            } else {
                None
            }
        } else {
            None
        };

        // Parse optional VECTOR distance DIMENSION n
        let (vector_distance, vector_dimension) = if self.check_identifier_value("VECTOR") {
            self.advance();
            let distance = self.parse_vector_distance()?;
            let dimension = if self.check_identifier_value("DIMENSION") {
                self.advance();
                let dim_expr = self.parse_expression()?;
                if let Expression::Literal(QueryValue::Integer(n)) = dim_expr {
                    Some(n as u32)
                } else {
                    None
                }
            } else {
                None
            };
            (Some(distance), dimension)
        } else {
            (None, None)
        };

        Ok(DefineStatement::Index {
            name,
            table,
            fields,
            unique,
            search_analyzer,
            vector_distance,
            vector_dimension,
        })
    }

    /// Parse vector distance type (COSINE, EUCLIDEAN, etc.)
    fn parse_vector_distance(&mut self) -> Result<VectorDistance, ParseError> {
        let token = self.advance();
        match token.value.to_uppercase().as_str() {
            "COSINE" => Ok(VectorDistance::Cosine),
            "EUCLIDEAN" => Ok(VectorDistance::Euclidean),
            "MANHATTAN" => Ok(VectorDistance::Manhattan),
            "HAMMING" => Ok(VectorDistance::Hamming),
            "JACCARD" => Ok(VectorDistance::Jaccard),
            _ => Err(ParseError::InvalidExpression {
                message: format!(
                    "Unknown vector distance type: {}. Expected COSINE, EUCLIDEAN, MANHATTAN, HAMMING, or JACCARD",
                    token.value
                ),
                token: token.clone(),
            }),
        }
    }

    /// Parse DEFINE EVENT name ON [TABLE] table WHEN expr THEN statement(s)
    fn parse_define_event(&mut self) -> Result<DefineStatement, ParseError> {
        self.expect(TokenType::Event)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();

        self.expect(TokenType::On)?;
        // Optional TABLE keyword
        if self.matches(&[TokenType::Table]) {
            self.advance();
        }
        let table = self.expect_identifier_or_keyword()?.value.clone();

        // WHEN clause
        self.expect(TokenType::When)?;
        let when = self.parse_expression()?;

        // THEN clause
        self.expect(TokenType::Then)?;

        // Parse statement(s) - can be a single statement or { statement; statement; }
        let then = if self.matches(&[TokenType::LeftBrace]) {
            self.advance();
            let mut statements = Vec::new();
            while !self.matches(&[TokenType::RightBrace]) && !self.is_at_end() {
                statements.push(self.parse_statement()?);
                // Optional semicolon
                if self.matches(&[TokenType::Semicolon]) {
                    self.advance();
                }
            }
            self.expect(TokenType::RightBrace)?;
            statements
        } else {
            vec![self.parse_statement()?]
        };

        Ok(DefineStatement::Event {
            name,
            table,
            when,
            then,
        })
    }

    /// Parse DEFINE FUNCTION name(params) { body }
    fn parse_define_function(&mut self) -> Result<DefineStatement, ParseError> {
        self.expect(TokenType::Function)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();

        // Parse parameter list
        self.expect(TokenType::LeftParen)?;
        let mut parameters = Vec::new();
        if !self.matches(&[TokenType::RightParen]) {
            loop {
                let param_name = if self.matches(&[TokenType::Parameter]) {
                    // $param style
                    let token = self.advance();
                    token.value.clone()
                } else {
                    self.expect_identifier_or_keyword()?.value.clone()
                };

                // Optional type annotation
                let data_type = if self.matches(&[TokenType::Colon]) {
                    self.advance();
                    Some(self.parse_data_type()?)
                } else {
                    None
                };

                // Optional default value
                let default = if self.matches(&[TokenType::Equal]) {
                    self.advance();
                    Some(self.parse_expression()?)
                } else {
                    None
                };

                parameters.push(FunctionParameter {
                    name: param_name,
                    data_type,
                    default,
                });

                if !self.matches(&[TokenType::Comma]) {
                    break;
                }
                self.advance();
            }
        }
        self.expect(TokenType::RightParen)?;

        // Parse body
        self.expect(TokenType::LeftBrace)?;
        let mut body = Vec::new();
        while !self.matches(&[TokenType::RightBrace]) && !self.is_at_end() {
            body.push(self.parse_statement()?);
            // Optional semicolon
            if self.matches(&[TokenType::Semicolon]) {
                self.advance();
            }
        }
        self.expect(TokenType::RightBrace)?;

        Ok(DefineStatement::Function {
            name,
            parameters,
            body,
        })
    }

    /// Parse DEFINE ANALYZER name TOKENIZERS a, b FILTERS c, d
    fn parse_define_analyzer(&mut self) -> Result<DefineStatement, ParseError> {
        self.expect(TokenType::Analyzer)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();

        // Parse TOKENIZERS
        let tokenizers = if self.check_identifier_value("TOKENIZERS") {
            self.advance();
            self.parse_identifier_list()?
        } else {
            Vec::new()
        };

        // Parse FILTERS
        let filters = if self.check_identifier_value("FILTERS") {
            self.advance();
            self.parse_identifier_list()?
        } else {
            Vec::new()
        };

        Ok(DefineStatement::Analyzer {
            name,
            tokenizers,
            filters,
        })
    }

    /// Parse DEFINE USER name ON namespace|database PASSWORD "password" ROLES role1, role2
    fn parse_define_user(&mut self) -> Result<DefineStatement, ParseError> {
        self.expect(TokenType::User)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();

        self.expect(TokenType::On)?;
        let on = self.expect_identifier_or_keyword()?.value.clone();

        // Parse optional PASSWORD
        let password = if self.check_identifier_value("PASSWORD") {
            self.advance();
            let token = self.advance();
            Some(token.value.clone())
        } else {
            None
        };

        // Parse optional ROLES
        let roles = if self.check_identifier_value("ROLES") {
            self.advance();
            self.parse_identifier_list()?
        } else {
            Vec::new()
        };

        Ok(DefineStatement::User {
            name,
            on,
            password,
            roles,
        })
    }

    /// Parse DEFINE SCOPE name [SESSION duration] [SIGNIN statement] [SIGNUP statement]
    fn parse_define_scope(&mut self) -> Result<DefineStatement, ParseError> {
        self.expect(TokenType::Scope)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();

        // Parse optional SESSION
        let session = if self.check_identifier_value("SESSION") {
            self.advance();
            let duration_str = self.advance().value.clone();
            Self::parse_duration_literal(&duration_str).ok()
        } else {
            None
        };

        // Parse optional SIGNIN
        let signin = if self.check_identifier_value("SIGNIN") {
            self.advance();
            Some(Box::new(self.parse_statement()?))
        } else {
            None
        };

        // Parse optional SIGNUP
        let signup = if self.check_identifier_value("SIGNUP") {
            self.advance();
            Some(Box::new(self.parse_statement()?))
        } else {
            None
        };

        Ok(DefineStatement::Scope {
            name,
            session,
            signin,
            signup,
        })
    }

    /// Parse DEFINE PARAM $name VALUE expr
    fn parse_define_param(&mut self) -> Result<DefineStatement, ParseError> {
        self.expect(TokenType::Param)?;

        // Parse parameter name (may or may not have $ prefix)
        let name = if self.matches(&[TokenType::Parameter]) {
            let token = self.advance();
            token.value.clone()
        } else {
            self.expect_identifier_or_keyword()?.value.clone()
        };

        // VALUE keyword
        if self.check_identifier_value("VALUE") {
            self.advance();
        }

        let value = self.parse_expression()?;

        Ok(DefineStatement::Param { name, value })
    }

    /// Parse optional PERMISSIONS clause
    fn parse_optional_permissions(&mut self) -> Result<Option<Permissions>, ParseError> {
        if !self.check_identifier_value("PERMISSIONS") {
            return Ok(None);
        }
        self.advance();

        // Handle FULL or NONE shortcuts
        if self.check_identifier_value("FULL") {
            self.advance();
            return Ok(Some(Permissions {
                select: Some(PermissionLevel::Full),
                create: Some(PermissionLevel::Full),
                update: Some(PermissionLevel::Full),
                delete: Some(PermissionLevel::Full),
            }));
        }
        if self.check_identifier_value("NONE") {
            self.advance();
            return Ok(Some(Permissions {
                select: Some(PermissionLevel::None),
                create: Some(PermissionLevel::None),
                update: Some(PermissionLevel::None),
                delete: Some(PermissionLevel::None),
            }));
        }

        // Parse individual permissions: FOR select, create, update, delete WHERE expr
        let mut permissions = Permissions {
            select: None,
            create: None,
            update: None,
            delete: None,
        };

        while self.check_identifier_value("FOR") {
            self.advance();
            let perm_type = self.expect_identifier_or_keyword()?.value.to_uppercase();

            // Parse WHERE clause for this permission
            let level = if self.matches(&[TokenType::Where]) {
                self.advance();
                let _expr = self.parse_expression()?;
                // For now, treat any WHERE as a conditional (expression-based) permission
                Some(PermissionLevel::Full)
            } else if self.check_identifier_value("FULL") {
                self.advance();
                Some(PermissionLevel::Full)
            } else if self.check_identifier_value("NONE") {
                self.advance();
                Some(PermissionLevel::None)
            } else {
                Some(PermissionLevel::Full)
            };

            match perm_type.as_str() {
                "SELECT" => permissions.select = level,
                "CREATE" => permissions.create = level,
                "UPDATE" => permissions.update = level,
                "DELETE" => permissions.delete = level,
                _ => {}
            }
        }

        Ok(Some(permissions))
    }

    /// Parse a comma-separated list of identifiers
    fn parse_identifier_list(&mut self) -> Result<Vec<String>, ParseError> {
        let mut identifiers = Vec::new();
        loop {
            identifiers.push(self.expect_identifier_or_keyword()?.value.clone());
            if !self.matches(&[TokenType::Comma]) {
                break;
            }
            self.advance();
        }
        Ok(identifiers)
    }

    /// Parse REMOVE statement (SurrealDB-style schema removal)
    fn parse_remove(&mut self) -> Result<RemoveStatement, ParseError> {
        self.expect(TokenType::Remove)?;

        let token = self.peek()?;
        match token.token_type {
            TokenType::Namespace => {
                self.advance();
                let name = self.expect_identifier_or_keyword()?.value.clone();
                Ok(RemoveStatement::Namespace { name })
            }
            TokenType::Database => {
                self.advance();
                let name = self.expect_identifier_or_keyword()?.value.clone();
                Ok(RemoveStatement::Database { name })
            }
            TokenType::Table => {
                self.advance();
                let name = self.expect_identifier_or_keyword()?.value.clone();
                Ok(RemoveStatement::Table { name })
            }
            TokenType::Field => {
                self.advance();
                let name = self.expect_identifier_or_keyword()?.value.clone();
                self.expect(TokenType::On)?;
                if self.matches(&[TokenType::Table]) {
                    self.advance();
                }
                let table = self.expect_identifier_or_keyword()?.value.clone();
                Ok(RemoveStatement::Field { name, table })
            }
            TokenType::Index => {
                self.advance();
                let name = self.expect_identifier_or_keyword()?.value.clone();
                self.expect(TokenType::On)?;
                if self.matches(&[TokenType::Table]) {
                    self.advance();
                }
                let table = self.expect_identifier_or_keyword()?.value.clone();
                Ok(RemoveStatement::Index { name, table })
            }
            TokenType::Event => {
                self.advance();
                let name = self.expect_identifier_or_keyword()?.value.clone();
                self.expect(TokenType::On)?;
                if self.matches(&[TokenType::Table]) {
                    self.advance();
                }
                let table = self.expect_identifier_or_keyword()?.value.clone();
                Ok(RemoveStatement::Event { name, table })
            }
            TokenType::Function => {
                self.advance();
                let name = self.expect_identifier_or_keyword()?.value.clone();
                Ok(RemoveStatement::Function { name })
            }
            TokenType::Analyzer => {
                self.advance();
                let name = self.expect_identifier_or_keyword()?.value.clone();
                Ok(RemoveStatement::Analyzer { name })
            }
            TokenType::User => {
                self.advance();
                let name = self.expect_identifier_or_keyword()?.value.clone();
                self.expect(TokenType::On)?;
                let on = self.expect_identifier_or_keyword()?.value.clone();
                Ok(RemoveStatement::User { name, on })
            }
            TokenType::Scope => {
                self.advance();
                let name = self.expect_identifier_or_keyword()?.value.clone();
                Ok(RemoveStatement::Scope { name })
            }
            TokenType::Param => {
                self.advance();
                let name = if self.matches(&[TokenType::Parameter]) {
                    self.advance().value.clone()
                } else {
                    self.expect_identifier_or_keyword()?.value.clone()
                };
                Ok(RemoveStatement::Param { name })
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: vec![
                    TokenType::Namespace,
                    TokenType::Database,
                    TokenType::Table,
                    TokenType::Field,
                    TokenType::Index,
                    TokenType::Event,
                    TokenType::Function,
                    TokenType::Analyzer,
                    TokenType::User,
                    TokenType::Scope,
                    TokenType::Param,
                ],
                found: token.clone(),
            }),
        }
    }

    /// Parse UPSERT statement
    /// UPSERT INTO table (columns) VALUES (...) [WHERE ...]
    fn parse_upsert(&mut self) -> Result<UpsertStatement, ParseError> {
        self.expect(TokenType::Upsert)?;

        // Optional INTO keyword
        if self.matches(&[TokenType::Into]) {
            self.advance();
        }

        let table = self.expect_identifier_or_keyword()?.value.clone();

        // Parse the insert data
        let data = self.parse_insert_values()?;

        // Optional WHERE clause
        let where_clause = if self.matches(&[TokenType::Where]) {
            self.advance();
            Some(self.parse_expression()?)
        } else {
            None
        };

        Ok(UpsertStatement {
            table,
            data,
            where_clause,
        })
    }

    /// Parse insert values (columns and values)
    fn parse_insert_values(&mut self) -> Result<InsertValues, ParseError> {
        // Parse column list if present
        let columns = if self.matches(&[TokenType::LeftParen]) {
            self.advance();
            let mut cols = Vec::new();
            loop {
                cols.push(self.expect_identifier_or_keyword()?.value.clone());
                if !self.matches(&[TokenType::Comma]) {
                    break;
                }
                self.advance();
            }
            self.expect(TokenType::RightParen)?;
            Some(cols)
        } else {
            None
        };

        // VALUES keyword
        self.expect(TokenType::Values)?;

        // Parse value rows
        let mut rows = Vec::new();
        loop {
            self.expect(TokenType::LeftParen)?;
            let mut row = Vec::new();
            loop {
                row.push(self.parse_expression()?);
                if !self.matches(&[TokenType::Comma]) {
                    break;
                }
                self.advance();
            }
            self.expect(TokenType::RightParen)?;
            rows.push(row);

            if !self.matches(&[TokenType::Comma]) {
                break;
            }
            self.advance();
        }

        // Note: InsertValues::Values doesn't track column names separately
        // The columns are only used for semantic validation later
        let _ = columns; // Suppress unused warning
        Ok(InsertValues::Values(rows))
    }

    /// Parse MERGE statement
    /// MERGE INTO target USING source ON condition WHEN MATCHED THEN ... WHEN NOT MATCHED THEN ...
    fn parse_merge(&mut self) -> Result<MergeStatement, ParseError> {
        self.expect(TokenType::Merge)?;
        self.expect(TokenType::Into)?;

        let into = self.expect_identifier_or_keyword()?.value.clone();

        self.expect(TokenType::Using)?;
        let using = self.parse_simple_table_ref()?;

        self.expect(TokenType::On)?;
        let on = self.parse_expression()?;

        // Parse WHEN MATCHED clause
        let when_matched = if self.matches(&[TokenType::When]) {
            self.advance();
            if self.matches(&[TokenType::Matched]) {
                self.advance();
                self.expect(TokenType::Then)?;
                Some(self.parse_merge_action()?)
            } else {
                None
            }
        } else {
            None
        };

        // Parse WHEN NOT MATCHED clause
        let when_not_matched = if self.matches(&[TokenType::When]) {
            self.advance();
            if self.matches(&[TokenType::Not]) {
                self.advance();
                self.expect(TokenType::Matched)?;
                self.expect(TokenType::Then)?;
                Some(self.parse_merge_action()?)
            } else {
                None
            }
        } else {
            None
        };

        Ok(MergeStatement {
            into,
            using,
            on,
            when_matched,
            when_not_matched,
        })
    }

    /// Parse a simple FROM clause (table with optional alias) for MERGE
    fn parse_simple_table_ref(&mut self) -> Result<FromClause, ParseError> {
        let name = self.expect_identifier_or_keyword()?.value.clone();

        // Optional alias
        let alias = if self.matches(&[TokenType::As]) {
            self.advance();
            Some(self.expect_identifier_or_keyword()?.value.clone())
        } else if self.matches(&[TokenType::Identifier]) && !self.is_keyword_ahead() {
            Some(self.expect_identifier_or_keyword()?.value.clone())
        } else {
            None
        };

        Ok(FromClause::Table { name, alias })
    }

    /// Check if current position is at a keyword
    fn is_keyword_ahead(&self) -> bool {
        if let Ok(token) = self.peek() {
            matches!(
                token.token_type,
                TokenType::On
                    | TokenType::When
                    | TokenType::Where
                    | TokenType::Join
                    | TokenType::Left
                    | TokenType::Right
                    | TokenType::Inner
                    | TokenType::Full
                    | TokenType::Cross
            )
        } else {
            false
        }
    }

    /// Parse MERGE action (UPDATE SET, DELETE, or INSERT)
    fn parse_merge_action(&mut self) -> Result<MergeAction, ParseError> {
        if self.matches(&[TokenType::Update]) {
            self.advance();
            self.expect(TokenType::Set)?;
            let assignments = self.parse_update_assignments()?;
            Ok(MergeAction::Update(assignments))
        } else if self.matches(&[TokenType::Delete]) {
            self.advance();
            Ok(MergeAction::Delete)
        } else if self.matches(&[TokenType::Insert]) {
            self.advance();
            // Parse INSERT (col1, col2) VALUES (val1, val2)
            let mut values = HashMap::new();
            if self.matches(&[TokenType::LeftParen]) {
                self.advance();
                let mut columns = Vec::new();
                loop {
                    columns.push(self.expect_identifier_or_keyword()?.value.clone());
                    if !self.matches(&[TokenType::Comma]) {
                        break;
                    }
                    self.advance();
                }
                self.expect(TokenType::RightParen)?;
                self.expect(TokenType::Values)?;
                self.expect(TokenType::LeftParen)?;
                let mut idx = 0;
                loop {
                    let expr = self.parse_expression()?;
                    if idx < columns.len() {
                        values.insert(columns[idx].clone(), expr);
                    }
                    idx += 1;
                    if !self.matches(&[TokenType::Comma]) {
                        break;
                    }
                    self.advance();
                }
                self.expect(TokenType::RightParen)?;
            }
            Ok(MergeAction::Insert(values))
        } else {
            Err(ParseError::UnexpectedToken {
                expected: vec![TokenType::Update, TokenType::Delete, TokenType::Insert],
                found: self.peek()?.clone(),
            })
        }
    }

    /// Parse update assignments (col = expr, col2 = expr2, ...)
    /// Supports both simple names (col) and qualified names (table.col)
    fn parse_update_assignments(&mut self) -> Result<Vec<UpdateAssignment>, ParseError> {
        let mut assignments = Vec::new();
        loop {
            // Parse field name - may be qualified (table.column)
            let mut field = self.expect_identifier_or_keyword()?.value.clone();
            while self.matches(&[TokenType::Dot]) {
                self.advance();
                let next_part = self.expect_identifier_or_keyword()?.value.clone();
                field = format!("{}.{}", field, next_part);
            }

            self.expect(TokenType::Equal)?;
            let value = self.parse_expression()?;
            assignments.push(UpdateAssignment {
                field,
                value,
                operator: None,
            });

            if !self.matches(&[TokenType::Comma]) {
                break;
            }
            self.advance();
        }
        Ok(assignments)
    }

    /// Parse ALTER statement
    /// ALTER TABLE name ADD/DROP/RENAME/ALTER ...
    /// ALTER FIELD name ON table SET/DROP ...
    /// ALTER INDEX name ON table RENAME/REBUILD
    fn parse_alter(&mut self) -> Result<AlterStatement, ParseError> {
        self.expect(TokenType::Alter)?;

        let token = self.peek()?;
        match token.token_type {
            TokenType::Table => self.parse_alter_table(),
            TokenType::Field => self.parse_alter_field(),
            TokenType::Index => self.parse_alter_index(),
            _ => Err(ParseError::UnexpectedToken {
                expected: vec![TokenType::Table, TokenType::Field, TokenType::Index],
                found: token.clone(),
            }),
        }
    }

    /// Parse ALTER TABLE statement
    fn parse_alter_table(&mut self) -> Result<AlterStatement, ParseError> {
        self.expect(TokenType::Table)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();

        let action = if self.matches(&[TokenType::Add]) {
            self.advance();
            // ADD COLUMN or ADD CONSTRAINT
            if self.matches(&[TokenType::Column]) {
                self.advance();
            }
            if self.matches(&[TokenType::Constraint]) {
                self.advance();
                let constraint = self.parse_table_constraint()?;
                AlterTableAction::AddConstraint(constraint)
            } else {
                // ADD COLUMN
                let field = self.parse_field_definition()?;
                AlterTableAction::AddColumn(field)
            }
        } else if self.matches(&[TokenType::Drop]) {
            self.advance();
            if self.matches(&[TokenType::Column]) {
                self.advance();
                let col_name = self.expect_identifier_or_keyword()?.value.clone();
                AlterTableAction::DropColumn(col_name)
            } else if self.matches(&[TokenType::Constraint]) {
                self.advance();
                let constraint_name = self.expect_identifier_or_keyword()?.value.clone();
                AlterTableAction::DropConstraint(constraint_name)
            } else {
                let col_name = self.expect_identifier_or_keyword()?.value.clone();
                AlterTableAction::DropColumn(col_name)
            }
        } else if self.matches(&[TokenType::Rename]) {
            self.advance();
            if self.matches(&[TokenType::Column]) {
                self.advance();
                let old = self.expect_identifier_or_keyword()?.value.clone();
                self.expect(TokenType::To)?;
                let new = self.expect_identifier_or_keyword()?.value.clone();
                AlterTableAction::RenameColumn { old, new }
            } else if self.matches(&[TokenType::To]) {
                self.advance();
                let new_name = self.expect_identifier_or_keyword()?.value.clone();
                AlterTableAction::Rename(new_name)
            } else {
                let old = self.expect_identifier_or_keyword()?.value.clone();
                self.expect(TokenType::To)?;
                let new = self.expect_identifier_or_keyword()?.value.clone();
                AlterTableAction::RenameColumn { old, new }
            }
        } else if self.matches(&[TokenType::Alter]) {
            self.advance();
            if self.matches(&[TokenType::Column]) {
                self.advance();
            }
            let field = self.parse_field_definition()?;
            AlterTableAction::AlterColumn(field)
        } else {
            return Err(ParseError::UnexpectedToken {
                expected: vec![
                    TokenType::Add,
                    TokenType::Drop,
                    TokenType::Rename,
                    TokenType::Alter,
                ],
                found: self.peek()?.clone(),
            });
        };

        Ok(AlterStatement::Table { name, action })
    }

    /// Parse ALTER FIELD statement
    fn parse_alter_field(&mut self) -> Result<AlterStatement, ParseError> {
        self.expect(TokenType::Field)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();

        self.expect(TokenType::On)?;
        if self.matches(&[TokenType::Table]) {
            self.advance();
        }
        let table = self.expect_identifier_or_keyword()?.value.clone();

        let action = if self.matches(&[TokenType::Set]) {
            self.advance();
            if self.check_identifier_value("TYPE") {
                self.advance();
                let data_type = self.parse_data_type()?;
                AlterFieldAction::SetType(data_type)
            } else if self.matches(&[TokenType::Default]) {
                self.advance();
                let expr = self.parse_expression()?;
                AlterFieldAction::SetDefault(expr)
            } else if self.check_identifier_value("NOT") {
                self.advance();
                self.expect(TokenType::Null)?;
                AlterFieldAction::SetNotNull
            } else {
                return Err(ParseError::InvalidExpression {
                    message: "Expected TYPE, DEFAULT, or NOT NULL after SET".to_string(),
                    token: self.peek()?.clone(),
                });
            }
        } else if self.matches(&[TokenType::Drop]) {
            self.advance();
            if self.matches(&[TokenType::Default]) {
                self.advance();
                AlterFieldAction::DropDefault
            } else if self.check_identifier_value("NOT") {
                self.advance();
                self.expect(TokenType::Null)?;
                AlterFieldAction::DropNotNull
            } else {
                AlterFieldAction::DropDefault
            }
        } else {
            return Err(ParseError::UnexpectedToken {
                expected: vec![TokenType::Set, TokenType::Drop],
                found: self.peek()?.clone(),
            });
        };

        Ok(AlterStatement::Field {
            name,
            table,
            action,
        })
    }

    /// Parse ALTER INDEX statement
    fn parse_alter_index(&mut self) -> Result<AlterStatement, ParseError> {
        self.expect(TokenType::Index)?;
        let name = self.expect_identifier_or_keyword()?.value.clone();

        self.expect(TokenType::On)?;
        if self.matches(&[TokenType::Table]) {
            self.advance();
        }
        let table = self.expect_identifier_or_keyword()?.value.clone();

        let action = if self.matches(&[TokenType::Rename]) {
            self.advance();
            self.expect(TokenType::To)?;
            let new_name = self.expect_identifier_or_keyword()?.value.clone();
            AlterIndexAction::Rename(new_name)
        } else if self.matches(&[TokenType::Rebuild]) {
            self.advance();
            AlterIndexAction::Rebuild
        } else {
            return Err(ParseError::UnexpectedToken {
                expected: vec![TokenType::Rename, TokenType::Rebuild],
                found: self.peek()?.clone(),
            });
        };

        Ok(AlterStatement::Index {
            name,
            table,
            action,
        })
    }

    /// Parse TRUNCATE statement
    /// TRUNCATE [TABLE] name [CASCADE]
    fn parse_truncate(&mut self) -> Result<TruncateStatement, ParseError> {
        self.expect(TokenType::Truncate)?;

        // Optional TABLE keyword
        if self.matches(&[TokenType::Table]) {
            self.advance();
        }

        let table = self.expect_identifier_or_keyword()?.value.clone();

        // Optional CASCADE
        let cascade = if self.matches(&[TokenType::Cascade]) {
            self.advance();
            true
        } else {
            false
        };

        Ok(TruncateStatement { table, cascade })
    }

    /// Parse SAVEPOINT statement
    /// SAVEPOINT name | RELEASE SAVEPOINT name | ROLLBACK TO SAVEPOINT name
    fn parse_savepoint(&mut self) -> Result<SavepointStatement, ParseError> {
        let token = self.peek()?;

        match token.token_type {
            TokenType::Savepoint => {
                self.advance();
                let name = self.expect_identifier_or_keyword()?.value.clone();
                Ok(SavepointStatement::Create { name })
            }
            TokenType::Release => {
                self.advance();
                // Optional SAVEPOINT keyword
                if self.matches(&[TokenType::Savepoint]) {
                    self.advance();
                }
                let name = self.expect_identifier_or_keyword()?.value.clone();
                Ok(SavepointStatement::Release { name })
            }
            TokenType::Rollback => {
                self.advance();
                self.expect(TokenType::To)?;
                // Optional SAVEPOINT keyword
                if self.matches(&[TokenType::Savepoint]) {
                    self.advance();
                }
                let name = self.expect_identifier_or_keyword()?.value.clone();
                Ok(SavepointStatement::Rollback { name })
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: vec![
                    TokenType::Savepoint,
                    TokenType::Release,
                    TokenType::Rollback,
                ],
                found: token.clone(),
            }),
        }
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

    /// Check if current token is an identifier with a specific value (case-insensitive)
    fn check_identifier_value(&self, value: &str) -> bool {
        if let Ok(token) = self.peek() {
            token.value.eq_ignore_ascii_case(value)
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

    /// Expect an identifier or keyword as a field name after a dot
    /// This allows reserved keywords like 'from', 'to', 'select', etc. to be used as field names
    fn expect_field_name(&mut self) -> Result<&Token, ParseError> {
        let token = self.peek()?;
        // Accept regular identifiers or any keyword token as a field name after dot
        if token.token_type == TokenType::Identifier || token.is_keyword() {
            Ok(self.advance())
        } else {
            Err(ParseError::UnexpectedToken {
                expected: vec![TokenType::Identifier],
                found: token.clone(),
            })
        }
    }

    /// Expect an identifier token, allowing certain keywords to be used as identifiers
    /// This is used for table names, column names, aliases, etc.
    fn expect_identifier_or_keyword(&mut self) -> Result<&Token, ParseError> {
        let token = self.peek()?;
        match token.token_type {
            TokenType::Identifier => Ok(self.advance()),
            // Allow keywords to be used as identifiers in most contexts
            // This enables using reserved words as table/column names when needed
            _ if token.is_keyword() => {
                // Allow most keywords to be used as identifiers
                // Only restrict truly structural keywords that would break parsing
                let restricted_keywords = [
                    "SELECT",
                    "FROM",
                    "WHERE",
                    "JOIN",
                    "INNER",
                    "LEFT",
                    "RIGHT",
                    "FULL",
                    "CROSS",
                    "ON",
                    "GROUP",
                    "BY",
                    "HAVING",
                    "ORDER",
                    "LIMIT",
                    "OFFSET",
                    "INSERT",
                    "INTO",
                    "VALUES",
                    "UPDATE",
                    "SET",
                    "DELETE",
                    "CREATE",
                    "DROP",
                    "TABLE",
                    "AS",
                    "AND",
                    "OR",
                    "NOT",
                    "IN",
                    "BETWEEN",
                    "IS",
                    "LIKE",
                    "CASE",
                    "WHEN",
                    "THEN",
                    "ELSE",
                    "END",
                    "WITH",
                    "RECURSIVE",
                ];

                if restricted_keywords.contains(&token.value.to_uppercase().as_str()) {
                    Err(ParseError::UnexpectedToken {
                        expected: vec![TokenType::Identifier],
                        found: token.clone(),
                    })
                } else {
                    Ok(self.advance())
                }
            }
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

    #[test]
    fn test_parse_field_access_with_reserved_keywords() {
        // Test that reserved keywords can be used as field names after dot
        let lexer = Lexer::new();

        // Test f.from - 'from' is a reserved keyword
        let tokens = lexer
            .tokenize("SELECT f.from, f.to FROM follows f")
            .unwrap();
        let mut parser = Parser::new();
        let stmt = parser.parse(tokens).unwrap();

        if let Statement::Select(select) = stmt {
            assert_eq!(select.fields.len(), 2);
            // First field: f.from
            if let SelectField::Expression { expr, .. } = &select.fields[0] {
                if let Expression::FieldAccess { object, field } = expr {
                    if let Expression::Identifier(name) = object.as_ref() {
                        assert_eq!(name, "f");
                    } else {
                        panic!("Expected identifier 'f'");
                    }
                    assert_eq!(field, "from");
                } else {
                    panic!("Expected FieldAccess for f.from");
                }
            } else {
                panic!("Expected Expression field");
            }
            // Second field: f.to
            if let SelectField::Expression { expr, .. } = &select.fields[1] {
                if let Expression::FieldAccess { object, field } = expr {
                    if let Expression::Identifier(name) = object.as_ref() {
                        assert_eq!(name, "f");
                    } else {
                        panic!("Expected identifier 'f'");
                    }
                    assert_eq!(field, "to");
                } else {
                    panic!("Expected FieldAccess for f.to");
                }
            } else {
                panic!("Expected Expression field");
            }
        } else {
            panic!("Expected SELECT statement");
        }
    }

    #[test]
    fn test_parse_join_with_keyword_field_names() {
        // Test the exact query that was failing: JOIN with 'from' as field name
        let lexer = Lexer::new();
        let tokens = lexer
            .tokenize("SELECT u.*, f.to FROM users u JOIN follows f ON u.id = f.from")
            .unwrap();
        let mut parser = Parser::new();
        let stmt = parser.parse(tokens).unwrap();

        if let Statement::Select(select) = stmt {
            // Should have one JOIN clause
            assert_eq!(select.join_clauses.len(), 1);
            // JOIN condition should be: u.id = f.from
            let join = &select.join_clauses[0];
            if let Expression::Binary {
                left,
                operator,
                right,
            } = &join.condition
            {
                assert!(matches!(operator, BinaryOperator::Equal));
                // Left side: u.id
                if let Expression::FieldAccess { object, field } = left.as_ref() {
                    assert_eq!(field, "id");
                    if let Expression::Identifier(name) = object.as_ref() {
                        assert_eq!(name, "u");
                    }
                }
                // Right side: f.from
                if let Expression::FieldAccess { object, field } = right.as_ref() {
                    assert_eq!(field, "from");
                    if let Expression::Identifier(name) = object.as_ref() {
                        assert_eq!(name, "f");
                    }
                }
            } else {
                panic!("Expected binary expression in JOIN condition");
            }
        } else {
            panic!("Expected SELECT statement");
        }
    }

    #[test]
    fn test_parse_now_minus_interval() {
        // Test parsing NOW() - INTERVAL '30 days'
        let lexer = Lexer::new();
        let tokens = lexer
            .tokenize("SELECT * FROM posts WHERE created_at > NOW() - INTERVAL '30 days'")
            .unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse NOW() - INTERVAL: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_parse_cte() {
        // Test parsing a simple CTE
        let lexer = Lexer::new();
        let query = r#"
            WITH user_stats AS (
                SELECT user_id FROM posts WHERE id > 1
            )
            SELECT * FROM user_stats
        "#;
        let tokens = lexer.tokenize(query).unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse simple CTE: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_parse_cte_with_count_star() {
        // Test parsing a CTE with COUNT(*)
        let lexer = Lexer::new();
        let query = r#"
            WITH user_stats AS (
                SELECT user_id, COUNT(*) AS post_count FROM posts GROUP BY user_id
            )
            SELECT * FROM user_stats
        "#;
        let tokens = lexer.tokenize(query).unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse CTE with COUNT(*): {:?}",
            result.err()
        );
    }

    #[test]
    fn test_parse_cte_with_interval() {
        // Test parsing a CTE with NOW() - INTERVAL
        let lexer = Lexer::new();
        let query = r#"
            WITH user_stats AS (
                SELECT user_id, COUNT(*) AS post_count
                FROM posts
                WHERE created_at > NOW() - INTERVAL '30 days'
                GROUP BY user_id
            )
            SELECT * FROM user_stats
        "#;
        let tokens = lexer.tokenize(query).unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse CTE with interval: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_parse_count_star() {
        // Test parsing COUNT(*)
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("SELECT COUNT(*) FROM users").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse COUNT(*): {:?}",
            result.err()
        );
    }

    #[test]
    fn test_parse_create_function_simple() {
        // Test parsing simple CREATE FUNCTION
        let lexer = Lexer::new();
        let query = "CREATE FUNCTION add_numbers(a INT, b INT) RETURNS INT AS $$ SELECT a + b $$";
        let tokens = lexer.tokenize(query).unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse CREATE FUNCTION: {:?}",
            result.err()
        );

        if let Statement::Create(create) = result.unwrap() {
            assert_eq!(create.name, "add_numbers");
            assert!(matches!(create.object_type, CreateObjectType::Function));
            if let CreateDefinition::Function {
                or_replace,
                parameters,
                return_type,
                language,
                volatility,
                body,
            } = create.definition
            {
                assert!(!or_replace);
                assert_eq!(parameters.len(), 2);
                assert_eq!(parameters[0].name, "a");
                assert_eq!(parameters[1].name, "b");
                assert!(matches!(return_type, DataType::Integer));
                assert!(language.is_none());
                assert!(volatility.is_none());
                assert_eq!(body.trim(), "SELECT a + b");
            } else {
                panic!("Expected Function definition");
            }
        } else {
            panic!("Expected CREATE statement");
        }
    }

    #[test]
    fn test_parse_create_function_with_language() {
        // Test parsing CREATE FUNCTION with LANGUAGE clause
        let lexer = Lexer::new();
        let query = "CREATE FUNCTION calculate_tax(amount DECIMAL) RETURNS DECIMAL LANGUAGE SQL AS $$ SELECT amount * 0.1 $$";
        let tokens = lexer.tokenize(query).unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse CREATE FUNCTION with LANGUAGE: {:?}",
            result.err()
        );

        if let Statement::Create(create) = result.unwrap() {
            if let CreateDefinition::Function { language, .. } = create.definition {
                assert!(matches!(language, Some(FunctionLanguage::Sql)));
            } else {
                panic!("Expected Function definition");
            }
        } else {
            panic!("Expected CREATE statement");
        }
    }

    #[test]
    fn test_parse_create_function_with_volatility() {
        // Test parsing CREATE FUNCTION with IMMUTABLE
        let lexer = Lexer::new();
        let query = "CREATE FUNCTION square(x INT) RETURNS INT IMMUTABLE AS $$ SELECT x * x $$";
        let tokens = lexer.tokenize(query).unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse CREATE FUNCTION with IMMUTABLE: {:?}",
            result.err()
        );

        if let Statement::Create(create) = result.unwrap() {
            if let CreateDefinition::Function { volatility, .. } = create.definition {
                assert!(matches!(volatility, Some(FunctionVolatility::Immutable)));
            } else {
                panic!("Expected Function definition");
            }
        } else {
            panic!("Expected CREATE statement");
        }
    }

    #[test]
    fn test_parse_create_or_replace_function() {
        // Test parsing CREATE OR REPLACE FUNCTION
        let lexer = Lexer::new();
        let query = "CREATE OR REPLACE FUNCTION greet(name VARCHAR) RETURNS VARCHAR AS $$ SELECT CONCAT('Hello ', name) $$";
        let tokens = lexer.tokenize(query).unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse CREATE OR REPLACE FUNCTION: {:?}",
            result.err()
        );

        if let Statement::Create(create) = result.unwrap() {
            if let CreateDefinition::Function { or_replace, .. } = create.definition {
                assert!(or_replace);
            } else {
                panic!("Expected Function definition");
            }
        } else {
            panic!("Expected CREATE statement");
        }
    }

    #[test]
    fn test_parse_create_function_with_out_param() {
        // Test parsing CREATE FUNCTION with OUT parameter
        let lexer = Lexer::new();
        let query = "CREATE FUNCTION get_stats(IN user_id INT, OUT total INT, OUT average DECIMAL) RETURNS VOID AS $$ SELECT count(*), avg(value) INTO total, average FROM stats WHERE uid = user_id $$";
        let tokens = lexer.tokenize(query).unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse CREATE FUNCTION with OUT params: {:?}",
            result.err()
        );

        if let Statement::Create(create) = result.unwrap() {
            if let CreateDefinition::Function { parameters, .. } = create.definition {
                assert_eq!(parameters.len(), 3);
                assert!(matches!(parameters[0].mode, Some(ParameterMode::In)));
                assert!(matches!(parameters[1].mode, Some(ParameterMode::Out)));
                assert!(matches!(parameters[2].mode, Some(ParameterMode::Out)));
            } else {
                panic!("Expected Function definition");
            }
        } else {
            panic!("Expected CREATE statement");
        }
    }

    #[test]
    fn test_parse_create_procedure() {
        // Test parsing CREATE PROCEDURE
        let lexer = Lexer::new();
        let query = "CREATE PROCEDURE update_inventory(item_id INT, quantity INT) AS $$ UPDATE inventory SET qty = qty + quantity WHERE id = item_id $$";
        let tokens = lexer.tokenize(query).unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse CREATE PROCEDURE: {:?}",
            result.err()
        );

        if let Statement::Create(create) = result.unwrap() {
            assert_eq!(create.name, "update_inventory");
            assert!(matches!(create.object_type, CreateObjectType::Procedure));
            if let CreateDefinition::Procedure {
                or_replace,
                parameters,
                language,
                body,
            } = create.definition
            {
                assert!(!or_replace);
                assert_eq!(parameters.len(), 2);
                assert!(language.is_none());
                assert!(body.contains("UPDATE inventory"));
            } else {
                panic!("Expected Procedure definition");
            }
        } else {
            panic!("Expected CREATE statement");
        }
    }

    #[test]
    fn test_parse_call_statement() {
        // Test parsing CALL statement
        let lexer = Lexer::new();
        let query = "CALL update_inventory(123, 50)";
        let tokens = lexer.tokenize(query).unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse CALL statement: {:?}",
            result.err()
        );

        if let Statement::Call(call) = result.unwrap() {
            assert_eq!(call.procedure_name, "update_inventory");
            assert_eq!(call.arguments.len(), 2);
        } else {
            panic!("Expected CALL statement");
        }
    }

    #[test]
    fn test_parse_call_no_args() {
        // Test parsing CALL statement with no arguments
        let lexer = Lexer::new();
        let query = "CALL refresh_cache()";
        let tokens = lexer.tokenize(query).unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse CALL with no args: {:?}",
            result.err()
        );

        if let Statement::Call(call) = result.unwrap() {
            assert_eq!(call.procedure_name, "refresh_cache");
            assert_eq!(call.arguments.len(), 0);
        } else {
            panic!("Expected CALL statement");
        }
    }

    #[test]
    fn test_parse_call_with_expressions() {
        // Test parsing CALL statement with complex expressions
        let lexer = Lexer::new();
        let query = "CALL process_order('ORD-001', 100.50, true)";
        let tokens = lexer.tokenize(query).unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse CALL with expressions: {:?}",
            result.err()
        );

        if let Statement::Call(call) = result.unwrap() {
            assert_eq!(call.procedure_name, "process_order");
            assert_eq!(call.arguments.len(), 3);
        } else {
            panic!("Expected CALL statement");
        }
    }

    #[test]
    fn test_parse_function_language_javascript() {
        // Test parsing CREATE FUNCTION with JavaScript language
        let lexer = Lexer::new();
        let query = "CREATE FUNCTION transform_data(input VARCHAR) RETURNS VARCHAR LANGUAGE JAVASCRIPT AS $$ return input.toUpperCase(); $$";
        let tokens = lexer.tokenize(query).unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse CREATE FUNCTION with JAVASCRIPT: {:?}",
            result.err()
        );

        if let Statement::Create(create) = result.unwrap() {
            if let CreateDefinition::Function { language, body, .. } = create.definition {
                assert!(matches!(language, Some(FunctionLanguage::JavaScript)));
                assert!(body.contains("toUpperCase"));
            } else {
                panic!("Expected Function definition");
            }
        } else {
            panic!("Expected CREATE statement");
        }
    }

    // =====================================================================
    // DEFINE Statement Tests (SurrealDB-style schema definition)
    // =====================================================================

    #[test]
    fn test_parse_define_namespace() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("DEFINE NAMESPACE myapp").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse DEFINE NAMESPACE: {:?}",
            result.err()
        );

        if let Statement::Define(def) = result.unwrap() {
            if let DefineStatement::Namespace { name } = *def {
                assert_eq!(name, "myapp");
            } else {
                panic!("Expected DEFINE NAMESPACE statement");
            }
        } else {
            panic!("Expected DEFINE statement");
        }
    }

    #[test]
    fn test_parse_define_database() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("DEFINE DATABASE production").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse DEFINE DATABASE: {:?}",
            result.err()
        );

        if let Statement::Define(def) = result.unwrap() {
            if let DefineStatement::Database { name } = *def {
                assert_eq!(name, "production");
            } else {
                panic!("Expected DEFINE DATABASE statement");
            }
        } else {
            panic!("Expected DEFINE statement");
        }
    }

    #[test]
    fn test_parse_define_table_simple() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("DEFINE TABLE users").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse DEFINE TABLE: {:?}",
            result.err()
        );

        if let Statement::Define(def) = result.unwrap() {
            if let DefineStatement::Table {
                name,
                schemafull,
                drop,
                ..
            } = *def
            {
                assert_eq!(name, "users");
                assert!(!schemafull);
                assert!(!drop);
            } else {
                panic!("Expected DEFINE TABLE statement");
            }
        } else {
            panic!("Expected DEFINE statement");
        }
    }

    #[test]
    fn test_parse_define_table_schemafull() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("DEFINE TABLE users SCHEMAFULL").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse DEFINE TABLE SCHEMAFULL: {:?}",
            result.err()
        );

        if let Statement::Define(def) = result.unwrap() {
            if let DefineStatement::Table {
                name, schemafull, ..
            } = *def
            {
                assert_eq!(name, "users");
                assert!(schemafull);
            } else {
                panic!("Expected DEFINE TABLE statement");
            }
        } else {
            panic!("Expected DEFINE statement");
        }
    }

    #[test]
    fn test_parse_define_field() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("DEFINE FIELD email ON TABLE users").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse DEFINE FIELD: {:?}",
            result.err()
        );

        if let Statement::Define(def) = result.unwrap() {
            if let DefineStatement::Field { name, table, .. } = *def {
                assert_eq!(name, "email");
                assert_eq!(table, "users");
            } else {
                panic!("Expected DEFINE FIELD statement");
            }
        } else {
            panic!("Expected DEFINE statement");
        }
    }

    #[test]
    fn test_parse_define_index() {
        let lexer = Lexer::new();
        let tokens = lexer
            .tokenize("DEFINE INDEX idx_email ON TABLE users FIELDS email UNIQUE")
            .unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse DEFINE INDEX: {:?}",
            result.err()
        );

        if let Statement::Define(def) = result.unwrap() {
            if let DefineStatement::Index {
                name,
                table,
                fields,
                unique,
                ..
            } = *def
            {
                assert_eq!(name, "idx_email");
                assert_eq!(table, "users");
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "email");
                assert!(unique);
            } else {
                panic!("Expected DEFINE INDEX statement");
            }
        } else {
            panic!("Expected DEFINE statement");
        }
    }

    #[test]
    fn test_parse_define_analyzer() {
        let lexer = Lexer::new();
        let tokens = lexer
            .tokenize(
                "DEFINE ANALYZER my_analyzer TOKENIZERS class, camel FILTERS lowercase, ascii",
            )
            .unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse DEFINE ANALYZER: {:?}",
            result.err()
        );

        if let Statement::Define(def) = result.unwrap() {
            if let DefineStatement::Analyzer {
                name,
                tokenizers,
                filters,
            } = *def
            {
                assert_eq!(name, "my_analyzer");
                assert_eq!(tokenizers, vec!["class", "camel"]);
                assert_eq!(filters, vec!["lowercase", "ascii"]);
            } else {
                panic!("Expected DEFINE ANALYZER statement");
            }
        } else {
            panic!("Expected DEFINE statement");
        }
    }

    #[test]
    fn test_parse_define_user() {
        let lexer = Lexer::new();
        let tokens = lexer
            .tokenize("DEFINE USER admin ON DATABASE PASSWORD 'secret' ROLES owner, admin")
            .unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse DEFINE USER: {:?}",
            result.err()
        );

        if let Statement::Define(def) = result.unwrap() {
            if let DefineStatement::User {
                name,
                on,
                password,
                roles,
            } = *def
            {
                assert_eq!(name, "admin");
                assert_eq!(on, "DATABASE");
                assert!(password.is_some());
                assert_eq!(roles, vec!["owner", "admin"]);
            } else {
                panic!("Expected DEFINE USER statement");
            }
        } else {
            panic!("Expected DEFINE statement");
        }
    }

    #[test]
    fn test_parse_define_scope() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("DEFINE SCOPE user_auth").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse DEFINE SCOPE: {:?}",
            result.err()
        );

        if let Statement::Define(def) = result.unwrap() {
            if let DefineStatement::Scope { name, .. } = *def {
                assert_eq!(name, "user_auth");
            } else {
                panic!("Expected DEFINE SCOPE statement");
            }
        } else {
            panic!("Expected DEFINE statement");
        }
    }

    // =====================================================================
    // REMOVE Statement Tests (SurrealDB-style schema removal)
    // =====================================================================

    #[test]
    fn test_parse_remove_namespace() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("REMOVE NAMESPACE myapp").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse REMOVE NAMESPACE: {:?}",
            result.err()
        );

        if let Statement::Remove(RemoveStatement::Namespace { name }) = result.unwrap() {
            assert_eq!(name, "myapp");
        } else {
            panic!("Expected REMOVE NAMESPACE statement");
        }
    }

    #[test]
    fn test_parse_remove_database() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("REMOVE DATABASE production").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse REMOVE DATABASE: {:?}",
            result.err()
        );

        if let Statement::Remove(RemoveStatement::Database { name }) = result.unwrap() {
            assert_eq!(name, "production");
        } else {
            panic!("Expected REMOVE DATABASE statement");
        }
    }

    #[test]
    fn test_parse_remove_table() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("REMOVE TABLE users").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse REMOVE TABLE: {:?}",
            result.err()
        );

        if let Statement::Remove(RemoveStatement::Table { name }) = result.unwrap() {
            assert_eq!(name, "users");
        } else {
            panic!("Expected REMOVE TABLE statement");
        }
    }

    #[test]
    fn test_parse_remove_field() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("REMOVE FIELD email ON TABLE users").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse REMOVE FIELD: {:?}",
            result.err()
        );

        if let Statement::Remove(RemoveStatement::Field { name, table }) = result.unwrap() {
            assert_eq!(name, "email");
            assert_eq!(table, "users");
        } else {
            panic!("Expected REMOVE FIELD statement");
        }
    }

    #[test]
    fn test_parse_remove_index() {
        let lexer = Lexer::new();
        let tokens = lexer
            .tokenize("REMOVE INDEX idx_email ON TABLE users")
            .unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse REMOVE INDEX: {:?}",
            result.err()
        );

        if let Statement::Remove(RemoveStatement::Index { name, table }) = result.unwrap() {
            assert_eq!(name, "idx_email");
            assert_eq!(table, "users");
        } else {
            panic!("Expected REMOVE INDEX statement");
        }
    }

    #[test]
    fn test_parse_remove_function() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("REMOVE FUNCTION calculate_tax").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse REMOVE FUNCTION: {:?}",
            result.err()
        );

        if let Statement::Remove(RemoveStatement::Function { name }) = result.unwrap() {
            assert_eq!(name, "calculate_tax");
        } else {
            panic!("Expected REMOVE FUNCTION statement");
        }
    }

    #[test]
    fn test_parse_remove_analyzer() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("REMOVE ANALYZER my_analyzer").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse REMOVE ANALYZER: {:?}",
            result.err()
        );

        if let Statement::Remove(RemoveStatement::Analyzer { name }) = result.unwrap() {
            assert_eq!(name, "my_analyzer");
        } else {
            panic!("Expected REMOVE ANALYZER statement");
        }
    }

    #[test]
    fn test_parse_remove_user() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("REMOVE USER admin ON DATABASE").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse REMOVE USER: {:?}",
            result.err()
        );

        if let Statement::Remove(RemoveStatement::User { name, on }) = result.unwrap() {
            assert_eq!(name, "admin");
            assert_eq!(on, "DATABASE");
        } else {
            panic!("Expected REMOVE USER statement");
        }
    }

    #[test]
    fn test_parse_remove_scope() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("REMOVE SCOPE user_auth").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse REMOVE SCOPE: {:?}",
            result.err()
        );

        if let Statement::Remove(RemoveStatement::Scope { name }) = result.unwrap() {
            assert_eq!(name, "user_auth");
        } else {
            panic!("Expected REMOVE SCOPE statement");
        }
    }

    // ==================== UPSERT Statement Tests ====================

    #[test]
    fn test_parse_upsert_simple() {
        let lexer = Lexer::new();
        let tokens = lexer
            .tokenize("UPSERT INTO users (id, name) VALUES (1, 'Alice')")
            .unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(result.is_ok(), "Failed to parse UPSERT: {:?}", result.err());

        if let Statement::Upsert(upsert) = result.unwrap() {
            assert_eq!(upsert.table, "users");
            // Verify data is populated
            match &upsert.data {
                InsertValues::Values(rows) => {
                    assert!(!rows.is_empty());
                }
                _ => panic!("Expected InsertValues::Values"),
            }
        } else {
            panic!("Expected UPSERT statement");
        }
    }

    #[test]
    fn test_parse_upsert_without_into() {
        let lexer = Lexer::new();
        let tokens = lexer
            .tokenize("UPSERT users (email) VALUES ('test@example.com')")
            .unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse UPSERT without INTO: {:?}",
            result.err()
        );

        if let Statement::Upsert(upsert) = result.unwrap() {
            assert_eq!(upsert.table, "users");
        } else {
            panic!("Expected UPSERT statement");
        }
    }

    // ==================== TRUNCATE Statement Tests ====================

    #[test]
    fn test_parse_truncate_simple() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("TRUNCATE users").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse TRUNCATE: {:?}",
            result.err()
        );

        if let Statement::Truncate(truncate) = result.unwrap() {
            assert_eq!(truncate.table, "users");
            assert!(!truncate.cascade);
        } else {
            panic!("Expected TRUNCATE statement");
        }
    }

    #[test]
    fn test_parse_truncate_table_cascade() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("TRUNCATE TABLE orders CASCADE").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse TRUNCATE TABLE CASCADE: {:?}",
            result.err()
        );

        if let Statement::Truncate(truncate) = result.unwrap() {
            assert_eq!(truncate.table, "orders");
            assert!(truncate.cascade);
        } else {
            panic!("Expected TRUNCATE statement");
        }
    }

    // ==================== SAVEPOINT Statement Tests ====================

    #[test]
    fn test_parse_savepoint() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("SAVEPOINT my_savepoint").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse SAVEPOINT: {:?}",
            result.err()
        );

        if let Statement::Savepoint(SavepointStatement::Create { name }) = result.unwrap() {
            assert_eq!(name, "my_savepoint");
        } else {
            panic!("Expected SAVEPOINT Create statement");
        }
    }

    #[test]
    fn test_parse_release_savepoint() {
        let lexer = Lexer::new();
        let tokens = lexer.tokenize("RELEASE SAVEPOINT my_savepoint").unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse RELEASE SAVEPOINT: {:?}",
            result.err()
        );

        if let Statement::Savepoint(SavepointStatement::Release { name }) = result.unwrap() {
            assert_eq!(name, "my_savepoint");
        } else {
            panic!("Expected SAVEPOINT Release statement");
        }
    }

    // ==================== ALTER Statement Tests ====================

    #[test]
    fn test_parse_alter_table_add_column() {
        let lexer = Lexer::new();
        let tokens = lexer
            .tokenize("ALTER TABLE users ADD COLUMN age INT")
            .unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse ALTER TABLE ADD COLUMN: {:?}",
            result.err()
        );

        if let Statement::Alter(AlterStatement::Table { name, action }) = result.unwrap() {
            assert_eq!(name, "users");
            if let AlterTableAction::AddColumn(field) = action {
                assert_eq!(field.name, "age");
            } else {
                panic!("Expected AddColumn action");
            }
        } else {
            panic!("Expected ALTER TABLE statement");
        }
    }

    #[test]
    fn test_parse_alter_table_drop_column() {
        let lexer = Lexer::new();
        let tokens = lexer
            .tokenize("ALTER TABLE users DROP COLUMN email")
            .unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse ALTER TABLE DROP COLUMN: {:?}",
            result.err()
        );

        if let Statement::Alter(AlterStatement::Table { name, action }) = result.unwrap() {
            assert_eq!(name, "users");
            if let AlterTableAction::DropColumn(col_name) = action {
                assert_eq!(col_name, "email");
            } else {
                panic!("Expected DropColumn action");
            }
        } else {
            panic!("Expected ALTER TABLE statement");
        }
    }

    #[test]
    fn test_parse_alter_table_rename() {
        let lexer = Lexer::new();
        let tokens = lexer
            .tokenize("ALTER TABLE users RENAME TO customers")
            .unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(
            result.is_ok(),
            "Failed to parse ALTER TABLE RENAME TO: {:?}",
            result.err()
        );

        if let Statement::Alter(AlterStatement::Table { name, action }) = result.unwrap() {
            assert_eq!(name, "users");
            if let AlterTableAction::Rename(new_name) = action {
                assert_eq!(new_name, "customers");
            } else {
                panic!("Expected Rename action");
            }
        } else {
            panic!("Expected ALTER TABLE statement");
        }
    }

    // ==================== MERGE Statement Tests ====================

    #[test]
    fn test_parse_merge_simple() {
        let lexer = Lexer::new();
        let tokens = lexer
            .tokenize(
                "MERGE INTO target USING source AS s ON target.id = s.id \
                 WHEN MATCHED THEN UPDATE SET target.name = s.name \
                 WHEN NOT MATCHED THEN INSERT (id, name) VALUES (s.id, s.name)",
            )
            .unwrap();
        let mut parser = Parser::new();
        let result = parser.parse(tokens);
        assert!(result.is_ok(), "Failed to parse MERGE: {:?}", result.err());

        if let Statement::Merge(merge) = result.unwrap() {
            // Verify target table (into is just a String in current AST)
            assert_eq!(merge.into, "target");
            // Verify source table with alias
            if let FromClause::Table { name, alias } = &merge.using {
                assert_eq!(name, "source");
                assert_eq!(alias.as_deref(), Some("s"));
            } else {
                panic!("Expected Table source");
            }
            // Verify WHEN MATCHED clause
            assert!(merge.when_matched.is_some());
            // Verify WHEN NOT MATCHED clause
            assert!(merge.when_not_matched.is_some());
        } else {
            panic!("Expected MERGE statement");
        }
    }
}
