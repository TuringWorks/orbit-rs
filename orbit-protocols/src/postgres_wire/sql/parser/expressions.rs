//! Expression Parser Implementation
//!
//! This module handles parsing of SQL expressions with proper precedence and associativity

#![allow(clippy::useless_conversion)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_map_or)]

use crate::error::ProtocolResult;
use crate::postgres_wire::sql::ast::{
    BinaryOperator, CaseExpression, ColumnRef, Expression, FrameBound, FunctionCall, FunctionName,
    NullsOrder, OrderByItem, SortDirection, UnaryOperator, WhenClause, WindowFrame,
    WindowFunctionType,
};
use crate::postgres_wire::sql::lexer::Token;
use crate::postgres_wire::sql::types::SqlType;

/// Expression parser with operator precedence handling
pub struct ExpressionParser;

impl ExpressionParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse a complete expression with proper precedence
    pub fn parse_expression(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Expression> {
        self.parse_or_expression(tokens, pos)
    }

    /// Parse OR expressions (lowest precedence)
    fn parse_or_expression(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Expression> {
        let mut left = self.parse_and_expression(tokens, pos)?;

        while *pos < tokens.len() {
            if matches!(tokens[*pos], Token::Or) {
                *pos += 1;
                let right = self.parse_and_expression(tokens, pos)?;
                left = Expression::Binary {
                    left: Box::new(left),
                    operator: BinaryOperator::Or,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }

        Ok(left)
    }

    /// Parse AND expressions
    fn parse_and_expression(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Expression> {
        let mut left = self.parse_equality_expression(tokens, pos)?;

        while *pos < tokens.len() {
            if matches!(tokens[*pos], Token::And) {
                *pos += 1;
                let right = self.parse_equality_expression(tokens, pos)?;
                left = Expression::Binary {
                    left: Box::new(left),
                    operator: BinaryOperator::And,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }

        Ok(left)
    }

    /// Parse equality expressions (=, !=, <>, IS, IS NOT)
    fn parse_equality_expression(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Expression> {
        let mut left = self.parse_comparison_expression(tokens, pos)?;

        while *pos < tokens.len() {
            let operator = match &tokens[*pos] {
                Token::Equal => BinaryOperator::Equal,
                Token::NotEqual => BinaryOperator::NotEqual,
                Token::Is => {
                    *pos += 1;
                    if *pos < tokens.len() && matches!(tokens[*pos], Token::Not) {
                        *pos += 1;
                        BinaryOperator::IsNot
                    } else {
                        BinaryOperator::Is
                    }
                }
                _ => break,
            };

            if !matches!(operator, BinaryOperator::Is | BinaryOperator::IsNot) {
                *pos += 1;
            }

            let right = self.parse_comparison_expression(tokens, pos)?;
            left = Expression::Binary {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse comparison expressions (<, <=, >, >=, LIKE, IN, BETWEEN)
    fn parse_comparison_expression(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Expression> {
        let mut left = self.parse_additive_expression(tokens, pos)?;

        while *pos < tokens.len() {
            let operator = match &tokens[*pos] {
                Token::LessThan => BinaryOperator::LessThan,
                Token::LessThanOrEqual => BinaryOperator::LessThanOrEqual,
                Token::GreaterThan => BinaryOperator::GreaterThan,
                Token::GreaterThanOrEqual => BinaryOperator::GreaterThanOrEqual,
                Token::Like => BinaryOperator::Like,
                Token::ILike => BinaryOperator::ILike,
                Token::In => BinaryOperator::In,
                Token::VectorDistance => BinaryOperator::VectorDistance,
                Token::VectorInnerProduct => BinaryOperator::VectorInnerProduct,
                Token::VectorCosineDistance => BinaryOperator::VectorCosineDistance,
                _ => break,
            };

            *pos += 1;
            let right = self.parse_additive_expression(tokens, pos)?;
            left = Expression::Binary {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse additive expressions (+, -, ||)
    fn parse_additive_expression(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Expression> {
        let mut left = self.parse_multiplicative_expression(tokens, pos)?;

        while *pos < tokens.len() {
            let operator = match &tokens[*pos] {
                Token::Plus => BinaryOperator::Plus,
                Token::Minus => BinaryOperator::Minus,
                Token::Concat => BinaryOperator::Concat,
                _ => break,
            };

            *pos += 1;
            let right = self.parse_multiplicative_expression(tokens, pos)?;
            left = Expression::Binary {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse multiplicative expressions (*, /, %)
    fn parse_multiplicative_expression(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Expression> {
        let mut left = self.parse_unary_expression(tokens, pos)?;

        while *pos < tokens.len() {
            let operator = match &tokens[*pos] {
                Token::Multiply => BinaryOperator::Multiply,
                Token::Divide => BinaryOperator::Divide,
                Token::Modulo => BinaryOperator::Modulo,
                _ => break,
            };

            *pos += 1;
            let right = self.parse_unary_expression(tokens, pos)?;
            left = Expression::Binary {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse unary expressions (NOT, -, +)
    fn parse_unary_expression(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Expression> {
        if *pos >= tokens.len() {
            return Err(crate::error::ProtocolError::ParseError(
                "Unexpected end of tokens".to_string(),
            ));
        }

        match &tokens[*pos] {
            Token::Not => {
                *pos += 1;
                let expr = self.parse_unary_expression(tokens, pos)?;
                Ok(Expression::Unary {
                    operator: UnaryOperator::Not,
                    operand: Box::new(expr),
                })
            }
            Token::Minus => {
                *pos += 1;
                let expr = self.parse_unary_expression(tokens, pos)?;
                Ok(Expression::Unary {
                    operator: UnaryOperator::Minus,
                    operand: Box::new(expr),
                })
            }
            Token::Plus => {
                *pos += 1;
                let expr = self.parse_unary_expression(tokens, pos)?;
                Ok(Expression::Unary {
                    operator: UnaryOperator::Plus,
                    operand: Box::new(expr),
                })
            }
            _ => self.parse_primary_expression(tokens, pos),
        }
    }

    /// Parse primary expressions (literals, identifiers, function calls, parenthesized expressions)
    fn parse_primary_expression(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Expression> {
        if *pos >= tokens.len() {
            return Err(crate::error::ProtocolError::ParseError(
                "Unexpected end of tokens".to_string(),
            ));
        }

        match &tokens[*pos] {
            Token::StringLiteral(s) => {
                *pos += 1;
                Ok(Expression::Literal(
                    crate::postgres_wire::sql::types::SqlValue::Text(s.clone()),
                ))
            }
            Token::NumericLiteral(n) => {
                *pos += 1;
                // Try to parse as integer first, then as decimal
                if let Ok(i) = n.parse::<i32>() {
                    Ok(Expression::Literal(
                        crate::postgres_wire::sql::types::SqlValue::Integer(i),
                    ))
                } else if let Ok(f) = n.parse::<f64>() {
                    Ok(Expression::Literal(
                        crate::postgres_wire::sql::types::SqlValue::DoublePrecision(f),
                    ))
                } else {
                    Ok(Expression::Literal(
                        crate::postgres_wire::sql::types::SqlValue::Text(n.clone()),
                    ))
                }
            }
            Token::BooleanLiteral(b) => {
                *pos += 1;
                Ok(Expression::Literal(
                    crate::postgres_wire::sql::types::SqlValue::Boolean(*b),
                ))
            }
            Token::Null => {
                *pos += 1;
                Ok(Expression::Literal(
                    crate::postgres_wire::sql::types::SqlValue::Null,
                ))
            }
            Token::Identifier(name) => {
                // Check for function call
                if *pos + 1 < tokens.len() && matches!(tokens[*pos + 1], Token::LeftParen) {
                    self.parse_function_call(tokens, pos, name.clone())
                } else {
                    *pos += 1;
                    // Regular column reference
                    Ok(Expression::Column(
                        crate::postgres_wire::sql::ast::ColumnRef {
                            table: None,
                            name: name.clone(),
                        },
                    ))
                }
            }

            // Handle aggregate function keywords
            Token::Count | Token::Sum | Token::Avg | Token::Min | Token::Max => {
                let func_name = match &tokens[*pos] {
                    Token::Count => "COUNT".to_string(),
                    Token::Sum => "SUM".to_string(),
                    Token::Avg => "AVG".to_string(),
                    Token::Min => "MIN".to_string(),
                    Token::Max => "MAX".to_string(),
                    _ => unreachable!(),
                };
                self.parse_function_call(tokens, pos, func_name)
            }

            // Handle window function keywords
            Token::RowNumber
            | Token::Rank
            | Token::DenseRank
            | Token::PercentRank
            | Token::CumeDist
            | Token::Ntile
            | Token::Lag
            | Token::Lead
            | Token::FirstValue
            | Token::LastValue
            | Token::NthValue => self.parse_window_function(tokens, pos),

            // Handle CASE expressions
            Token::Case => self.parse_case_expression(tokens, pos),

            // Handle CAST expressions
            Token::Cast => self.parse_cast_expression(tokens, pos),

            Token::LeftParen => {
                *pos += 1; // consume '('
                let expr = self.parse_expression(tokens, pos)?;

                if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RightParen) {
                    return Err(crate::error::ProtocolError::ParseError(
                        "Expected ')' after expression".to_string(),
                    ));
                }
                *pos += 1; // consume ')'

                Ok(expr)
            }
            _ => Err(crate::error::ProtocolError::ParseError(format!(
                "Unexpected token in expression: {:?}",
                tokens[*pos]
            ))),
        }
    }

    /// Parse a function call with proper DISTINCT and FILTER support
    fn parse_function_call(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
        func_name: String,
    ) -> ProtocolResult<Expression> {
        *pos += 1; // consume function name

        if *pos >= tokens.len() || !matches!(tokens[*pos], Token::LeftParen) {
            return Err(crate::error::ProtocolError::ParseError(
                "Expected '(' after function name".to_string(),
            )
            .into());
        }
        *pos += 1; // consume '('

        // Check for DISTINCT/ALL
        let distinct = if *pos < tokens.len() && matches!(tokens[*pos], Token::Distinct) {
            *pos += 1;
            true
        } else if *pos < tokens.len() && matches!(tokens[*pos], Token::All) {
            *pos += 1;
            false
        } else {
            false
        };

        let mut args = Vec::new();

        // Parse arguments
        if *pos < tokens.len() && !matches!(tokens[*pos], Token::RightParen) {
            loop {
                // Handle special case for COUNT(*)
                if func_name.to_uppercase() == "COUNT" && matches!(tokens[*pos], Token::Multiply) {
                    *pos += 1;
                    args.push(Expression::Column(ColumnRef {
                        table: None,
                        name: "*".to_string(),
                    }));
                } else {
                    args.push(self.parse_expression(tokens, pos)?);
                }

                if *pos < tokens.len() && matches!(tokens[*pos], Token::Comma) {
                    *pos += 1; // consume ','
                } else {
                    break;
                }
            }
        }

        if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RightParen) {
            return Err(crate::error::ProtocolError::ParseError(
                "Expected ')' after function arguments".to_string(),
            )
            .into());
        }
        *pos += 1; // consume ')'

        // Parse optional ORDER BY clause for aggregate functions
        let order_by = if *pos < tokens.len() && matches!(tokens[*pos], Token::Order) {
            *pos += 1;
            if *pos < tokens.len() && matches!(tokens[*pos], Token::By) {
                *pos += 1;
                Some(self.parse_order_by_list(tokens, pos)?)
            } else {
                None
            }
        } else {
            None
        };

        // Parse optional FILTER clause
        let filter = if *pos < tokens.len() {
            if let Token::Identifier(s) = &tokens[*pos] {
                if s.to_uppercase() == "FILTER" {
                    *pos += 1;
                    if *pos < tokens.len() && matches!(tokens[*pos], Token::LeftParen) {
                        *pos += 1;
                        if *pos < tokens.len() && matches!(tokens[*pos], Token::Where) {
                            *pos += 1;
                            let filter_expr = self.parse_expression(tokens, pos)?;
                            if *pos < tokens.len() && matches!(tokens[*pos], Token::RightParen) {
                                *pos += 1;
                                Some(Box::new(filter_expr))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Check if this is followed by an OVER clause (window function)
        if *pos < tokens.len() && matches!(tokens[*pos], Token::Over) {
            self.parse_window_over_clause(tokens, pos, func_name, args, distinct, order_by, filter)
        } else {
            Ok(Expression::Function(Box::new(FunctionCall {
                name: FunctionName::Simple(func_name),
                args,
                distinct,
                order_by,
                filter,
            })))
        }
    }

    /// Parse window function expressions
    fn parse_window_function(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Expression> {
        let window_func = match &tokens[*pos] {
            Token::RowNumber => {
                *pos += 1;
                self.expect_token(tokens, pos, &Token::LeftParen)?;
                self.expect_token(tokens, pos, &Token::RightParen)?;
                WindowFunctionType::RowNumber
            }
            Token::Rank => {
                *pos += 1;
                self.expect_token(tokens, pos, &Token::LeftParen)?;
                self.expect_token(tokens, pos, &Token::RightParen)?;
                WindowFunctionType::Rank
            }
            Token::DenseRank => {
                *pos += 1;
                self.expect_token(tokens, pos, &Token::LeftParen)?;
                self.expect_token(tokens, pos, &Token::RightParen)?;
                WindowFunctionType::DenseRank
            }
            Token::PercentRank => {
                *pos += 1;
                self.expect_token(tokens, pos, &Token::LeftParen)?;
                self.expect_token(tokens, pos, &Token::RightParen)?;
                WindowFunctionType::PercentRank
            }
            Token::CumeDist => {
                *pos += 1;
                self.expect_token(tokens, pos, &Token::LeftParen)?;
                self.expect_token(tokens, pos, &Token::RightParen)?;
                WindowFunctionType::CumeDist
            }
            Token::Ntile => {
                *pos += 1;
                self.expect_token(tokens, pos, &Token::LeftParen)?;
                let n = Box::new(self.parse_expression(tokens, pos)?);
                self.expect_token(tokens, pos, &Token::RightParen)?;
                WindowFunctionType::Ntile(n)
            }
            Token::Lag => {
                *pos += 1;
                self.expect_token(tokens, pos, &Token::LeftParen)?;
                let expr = Box::new(self.parse_expression(tokens, pos)?);

                let offset = if self.matches_at(tokens, *pos, &Token::Comma) {
                    *pos += 1;
                    Some(Box::new(self.parse_expression(tokens, pos)?))
                } else {
                    None
                };

                let default = if self.matches_at(tokens, *pos, &Token::Comma) {
                    *pos += 1;
                    Some(Box::new(self.parse_expression(tokens, pos)?))
                } else {
                    None
                };

                self.expect_token(tokens, pos, &Token::RightParen)?;
                WindowFunctionType::Lag {
                    expr,
                    offset,
                    default,
                }
            }
            Token::Lead => {
                *pos += 1;
                self.expect_token(tokens, pos, &Token::LeftParen)?;
                let expr = Box::new(self.parse_expression(tokens, pos)?);

                let offset = if self.matches_at(tokens, *pos, &Token::Comma) {
                    *pos += 1;
                    Some(Box::new(self.parse_expression(tokens, pos)?))
                } else {
                    None
                };

                let default = if self.matches_at(tokens, *pos, &Token::Comma) {
                    *pos += 1;
                    Some(Box::new(self.parse_expression(tokens, pos)?))
                } else {
                    None
                };

                self.expect_token(tokens, pos, &Token::RightParen)?;
                WindowFunctionType::Lead {
                    expr,
                    offset,
                    default,
                }
            }
            Token::FirstValue => {
                *pos += 1;
                self.expect_token(tokens, pos, &Token::LeftParen)?;
                let expr = Box::new(self.parse_expression(tokens, pos)?);
                self.expect_token(tokens, pos, &Token::RightParen)?;
                WindowFunctionType::FirstValue(expr)
            }
            Token::LastValue => {
                *pos += 1;
                self.expect_token(tokens, pos, &Token::LeftParen)?;
                let expr = Box::new(self.parse_expression(tokens, pos)?);
                self.expect_token(tokens, pos, &Token::RightParen)?;
                WindowFunctionType::LastValue(expr)
            }
            Token::NthValue => {
                *pos += 1;
                self.expect_token(tokens, pos, &Token::LeftParen)?;
                let expr = Box::new(self.parse_expression(tokens, pos)?);
                self.expect_token(tokens, pos, &Token::Comma)?;
                let n = Box::new(self.parse_expression(tokens, pos)?);
                self.expect_token(tokens, pos, &Token::RightParen)?;
                WindowFunctionType::NthValue { expr, n }
            }
            _ => {
                return Err(crate::error::ProtocolError::ParseError(
                    "Invalid window function".to_string(),
                )
                .into())
            }
        };

        // Parse OVER clause
        self.expect_token(tokens, pos, &Token::Over)?;
        let (partition_by, order_by, frame) = self.parse_over_clause(tokens, pos)?;

        Ok(Expression::WindowFunction {
            function: window_func,
            partition_by,
            order_by,
            frame,
        })
    }

    /// Parse window OVER clause for aggregate functions used as window functions
    fn parse_window_over_clause(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
        func_name: String,
        args: Vec<Expression>,
        distinct: bool,
        order_by: Option<Vec<OrderByItem>>,
        filter: Option<Box<Expression>>,
    ) -> ProtocolResult<Expression> {
        *pos += 1; // consume OVER
        let (partition_by, window_order_by, frame) = self.parse_over_clause(tokens, pos)?;

        let aggregate_func = FunctionCall {
            name: FunctionName::Simple(func_name),
            args,
            distinct,
            order_by,
            filter,
        };

        Ok(Expression::WindowFunction {
            function: WindowFunctionType::Aggregate(Box::new(aggregate_func)),
            partition_by,
            order_by: window_order_by,
            frame,
        })
    }

    /// Parse the contents of an OVER clause
    fn parse_over_clause(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<(Vec<Expression>, Vec<OrderByItem>, Option<WindowFrame>)> {
        self.expect_token(tokens, pos, &Token::LeftParen)?;

        // Parse PARTITION BY
        let partition_by = if self.matches_at(tokens, *pos, &Token::Partition) {
            *pos += 1;
            self.expect_token(tokens, pos, &Token::By)?;
            self.parse_expression_list(tokens, pos)?
        } else {
            Vec::new()
        };

        // Parse ORDER BY
        let order_by = if self.matches_at(tokens, *pos, &Token::Order) {
            *pos += 1;
            self.expect_token(tokens, pos, &Token::By)?;
            self.parse_order_by_list(tokens, pos)?
        } else {
            Vec::new()
        };

        // Parse optional window frame
        let frame = if self.matches_at(tokens, *pos, &Token::Rows)
            || self.matches_at(tokens, *pos, &Token::Range)
        {
            Some(self.parse_window_frame(tokens, pos)?)
        } else {
            None
        };

        self.expect_token(tokens, pos, &Token::RightParen)?;

        Ok((partition_by, order_by, frame))
    }

    /// Parse window frame specification
    fn parse_window_frame(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<WindowFrame> {
        // Skip ROWS or RANGE for now, we'll just parse the bounds
        *pos += 1;

        let start_bound = self.parse_frame_bound(tokens, pos)?;

        let end_bound = if self.matches_at(tokens, *pos, &Token::And) {
            *pos += 1;
            Some(self.parse_frame_bound(tokens, pos)?)
        } else {
            None
        };

        Ok(WindowFrame {
            start_bound,
            end_bound,
        })
    }

    /// Parse window frame bound
    fn parse_frame_bound(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<FrameBound> {
        if self.matches_at(tokens, *pos, &Token::Unbounded) {
            *pos += 1;
            if self.matches_at(tokens, *pos, &Token::Preceding) {
                *pos += 1;
                Ok(FrameBound::UnboundedPreceding)
            } else if self.matches_at(tokens, *pos, &Token::Following) {
                *pos += 1;
                Ok(FrameBound::UnboundedFollowing)
            } else {
                Err(crate::error::ProtocolError::ParseError(
                    "Expected PRECEDING or FOLLOWING after UNBOUNDED".to_string(),
                )
                .into())
            }
        } else if self.matches_at(tokens, *pos, &Token::CurrentRow) {
            *pos += 1;
            Ok(FrameBound::CurrentRow)
        } else {
            let expr = Box::new(self.parse_expression(tokens, pos)?);
            if self.matches_at(tokens, *pos, &Token::Preceding) {
                *pos += 1;
                Ok(FrameBound::Preceding(expr))
            } else if self.matches_at(tokens, *pos, &Token::Following) {
                *pos += 1;
                Ok(FrameBound::Following(expr))
            } else {
                Err(crate::error::ProtocolError::ParseError(
                    "Expected PRECEDING or FOLLOWING".to_string(),
                )
                .into())
            }
        }
    }

    // Helper methods
    fn matches_at(&self, tokens: &[Token], pos: usize, expected: &Token) -> bool {
        tokens.get(pos).map_or(false, |token| {
            std::mem::discriminant(token) == std::mem::discriminant(expected)
        })
    }

    fn expect_token(
        &self,
        tokens: &[Token],
        pos: &mut usize,
        expected: &Token,
    ) -> ProtocolResult<()> {
        if self.matches_at(tokens, *pos, expected) {
            *pos += 1;
            Ok(())
        } else {
            Err(crate::error::ProtocolError::ParseError(format!(
                "Expected {:?}, found {:?}",
                expected,
                tokens.get(*pos)
            ))
            .into())
        }
    }

    fn parse_expression_list(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Vec<Expression>> {
        let mut expressions = Vec::new();

        loop {
            expressions.push(self.parse_expression(tokens, pos)?);

            if self.matches_at(tokens, *pos, &Token::Comma) {
                *pos += 1;
            } else {
                break;
            }
        }

        Ok(expressions)
    }

    fn parse_order_by_list(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Vec<OrderByItem>> {
        let mut items = Vec::new();

        loop {
            let expression = self.parse_expression(tokens, pos)?;

            let direction = if self.matches_at(tokens, *pos, &Token::Asc) {
                *pos += 1;
                Some(SortDirection::Ascending)
            } else if self.matches_at(tokens, *pos, &Token::Desc) {
                *pos += 1;
                Some(SortDirection::Descending)
            } else {
                None
            };

            let nulls = if self.matches_at(tokens, *pos, &Token::Nulls) {
                *pos += 1;
                if self.matches_at(tokens, *pos, &Token::First) {
                    *pos += 1;
                    Some(NullsOrder::First)
                } else if self.matches_at(tokens, *pos, &Token::Last) {
                    *pos += 1;
                    Some(NullsOrder::Last)
                } else {
                    None
                }
            } else {
                None
            };

            items.push(OrderByItem {
                expression,
                direction,
                nulls,
            });

            if self.matches_at(tokens, *pos, &Token::Comma) {
                *pos += 1;
            } else {
                break;
            }
        }

        Ok(items)
    }

    /// Parse a CASE expression
    fn parse_case_expression(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Expression> {
        *pos += 1; // consume 'CASE'

        // Check for CASE WHEN vs CASE expression WHEN
        let case_expr = if !matches!(tokens.get(*pos), Some(Token::When)) {
            // CASE expression WHEN ...
            Some(Box::new(self.parse_expression(tokens, pos)?))
        } else {
            // CASE WHEN ...
            None
        };

        let mut when_clauses = Vec::new();

        // Parse WHEN clauses
        while *pos < tokens.len() && matches!(tokens[*pos], Token::When) {
            *pos += 1; // consume 'WHEN'
            let condition = Box::new(self.parse_expression(tokens, pos)?);

            if *pos >= tokens.len() || !matches!(tokens[*pos], Token::Then) {
                return Err(crate::error::ProtocolError::ParseError(
                    "Expected THEN after WHEN condition".to_string(),
                ));
            }
            *pos += 1; // consume 'THEN'

            let result = Box::new(self.parse_expression(tokens, pos)?);

            when_clauses.push(WhenClause { condition, result });
        }

        if when_clauses.is_empty() {
            return Err(crate::error::ProtocolError::ParseError(
                "CASE expression must have at least one WHEN clause".to_string(),
            ));
        }

        // Parse optional ELSE clause
        let else_clause = if *pos < tokens.len() && matches!(tokens[*pos], Token::Else) {
            *pos += 1; // consume 'ELSE'
            Some(Box::new(self.parse_expression(tokens, pos)?))
        } else {
            None
        };

        // Expect END
        if *pos >= tokens.len() || !matches!(tokens[*pos], Token::End) {
            return Err(crate::error::ProtocolError::ParseError(
                "Expected END to close CASE expression".to_string(),
            ));
        }
        *pos += 1; // consume 'END'

        Ok(Expression::Case(CaseExpression {
            operand: case_expr,
            when_clauses,
            else_clause,
        }))
    }

    /// Parse a CAST expression
    fn parse_cast_expression(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Expression> {
        *pos += 1; // consume 'CAST'

        if *pos >= tokens.len() || !matches!(tokens[*pos], Token::LeftParen) {
            return Err(crate::error::ProtocolError::ParseError(
                "Expected '(' after CAST".to_string(),
            ));
        }
        *pos += 1; // consume '('

        let expression = Box::new(self.parse_expression(tokens, pos)?);

        if *pos >= tokens.len() || !matches!(tokens[*pos], Token::As) {
            return Err(crate::error::ProtocolError::ParseError(
                "Expected AS in CAST expression".to_string(),
            ));
        }
        *pos += 1; // consume 'AS'

        let target_type = self.parse_sql_type(tokens, pos)?;

        if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RightParen) {
            return Err(crate::error::ProtocolError::ParseError(
                "Expected ')' after CAST data type".to_string(),
            ));
        }
        *pos += 1; // consume ')'

        Ok(Expression::Cast {
            expr: expression,
            target_type,
        })
    }

    /// Parse a SQL type for CAST expressions
    fn parse_sql_type(&mut self, tokens: &[Token], pos: &mut usize) -> ProtocolResult<SqlType> {
        if *pos >= tokens.len() {
            return Err(crate::error::ProtocolError::ParseError(
                "Expected data type".to_string(),
            ));
        }

        match &tokens[*pos] {
            Token::Integer => {
                *pos += 1;
                Ok(SqlType::Integer)
            }
            Token::BigInt => {
                *pos += 1;
                Ok(SqlType::BigInt)
            }
            Token::SmallInt => {
                *pos += 1;
                Ok(SqlType::SmallInt)
            }
            Token::Real => {
                *pos += 1;
                Ok(SqlType::Real)
            }
            Token::DoublePrecision => {
                *pos += 1;
                Ok(SqlType::DoublePrecision)
            }
            Token::Boolean => {
                *pos += 1;
                Ok(SqlType::Boolean)
            }
            Token::Text => {
                *pos += 1;
                Ok(SqlType::Text)
            }
            Token::Varchar => {
                *pos += 1;
                // Check for optional length specification
                if *pos < tokens.len() && matches!(tokens[*pos], Token::LeftParen) {
                    *pos += 1;
                    if let Some(Token::NumericLiteral(len_str)) = tokens.get(*pos) {
                        if let Ok(length) = len_str.parse::<u32>() {
                            *pos += 1;
                            if *pos < tokens.len() && matches!(tokens[*pos], Token::RightParen) {
                                *pos += 1;
                                return Ok(SqlType::Varchar(Some(length)));
                            }
                        }
                    }
                    return Err(crate::error::ProtocolError::ParseError(
                        "Invalid VARCHAR length specification".to_string(),
                    ));
                }
                Ok(SqlType::Varchar(None))
            }
            Token::Timestamp => {
                *pos += 1;
                Ok(SqlType::Timestamp {
                    with_timezone: false,
                })
            }
            Token::Date => {
                *pos += 1;
                Ok(SqlType::Date)
            }
            Token::Time => {
                *pos += 1;
                Ok(SqlType::Time {
                    with_timezone: false,
                })
            }
            Token::Identifier(type_name) => {
                *pos += 1;
                Ok(SqlType::Custom {
                    type_name: type_name.clone(),
                })
            }
            _ => Err(crate::error::ProtocolError::ParseError(format!(
                "Unexpected token in data type: {:?}",
                tokens[*pos]
            ))),
        }
    }
}

impl Default for ExpressionParser {
    fn default() -> Self {
        Self::new()
    }
}
