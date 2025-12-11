//! Expression Parser Implementation
//!
//! This module handles parsing of SQL expressions with proper precedence and associativity

#![allow(clippy::useless_conversion)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_map_or)]

use crate::protocols::error::ProtocolResult;
use crate::protocols::postgres_wire::sql::ast::{
    BinaryOperator, CaseExpression, ColumnRef, Expression, FrameBound, FunctionCall, FunctionName,
    InList, NullsOrder, OrderByItem, SortDirection, UnaryOperator, WhenClause, WindowFrame,
    WindowFrameExclusion, WindowFrameMode, WindowFunctionType,
};
use crate::protocols::postgres_wire::sql::lexer::Token;
use crate::protocols::postgres_wire::sql::types::SqlType;

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
            if matches!(&tokens[*pos], Token::In) {
                // Handle IN operator specially to support both value lists and subqueries
                *pos += 1; // consume IN

                // Check for NOT IN
                let negated = if *pos < tokens.len() && matches!(&tokens[*pos], Token::Not) {
                    *pos += 1;
                    true
                } else {
                    false
                };

                // Parse IN list or subquery
                if *pos >= tokens.len() {
                    return Err(crate::protocols::error::ProtocolError::ParseError(
                        "Expected '(' after IN".to_string(),
                    ));
                }

                if !matches!(&tokens[*pos], Token::LeftParen) {
                    return Err(crate::protocols::error::ProtocolError::ParseError(
                        "Expected '(' after IN".to_string(),
                    ));
                }
                *pos += 1; // consume '('

                // Check if this is a subquery
                let in_list = if *pos < tokens.len() && matches!(&tokens[*pos], Token::Select) {
                    // Parse as subquery
                    use crate::protocols::postgres_wire::sql::parser::select::SelectParser;
                    let mut select_parser = SelectParser::new();
                    let select_stmt = select_parser.parse_select(tokens, pos)?;

                    if *pos >= tokens.len() || !matches!(&tokens[*pos], Token::RightParen) {
                        return Err(crate::protocols::error::ProtocolError::ParseError(
                            "Expected ')' after subquery in IN clause".to_string(),
                        ));
                    }
                    *pos += 1; // consume ')'
                    InList::Subquery(Box::new(select_stmt))
                } else {
                    // Parse as expression list
                    let mut exprs = Vec::new();
                    if *pos < tokens.len() && !matches!(&tokens[*pos], Token::RightParen) {
                        loop {
                            exprs.push(self.parse_expression(tokens, pos)?);
                            if *pos < tokens.len() && matches!(&tokens[*pos], Token::Comma) {
                                *pos += 1; // consume ','
                            } else {
                                break;
                            }
                        }
                    }

                    if *pos >= tokens.len() || !matches!(&tokens[*pos], Token::RightParen) {
                        return Err(crate::protocols::error::ProtocolError::ParseError(
                            "Expected ')' after IN list".to_string(),
                        ));
                    }
                    *pos += 1; // consume ')'
                    InList::Expressions(exprs)
                };

                left = Expression::In {
                    expr: Box::new(left),
                    list: in_list,
                    negated,
                };
            } else {
                let operator = if matches!(&tokens[*pos], Token::Similar) {
                    if *pos + 1 < tokens.len() && matches!(&tokens[*pos + 1], Token::To) {
                        *pos += 1; // consume SIMILAR (caller consumes TO)
                        BinaryOperator::SimilarTo
                    } else {
                        break;
                    }
                } else if matches!(&tokens[*pos], Token::Not) {
                    if *pos + 2 < tokens.len()
                        && matches!(&tokens[*pos + 1], Token::Similar)
                        && matches!(&tokens[*pos + 2], Token::To)
                    {
                        *pos += 2; // consume NOT and SIMILAR (caller consumes TO)
                        BinaryOperator::NotSimilarTo
                    } else if *pos + 1 < tokens.len() && matches!(&tokens[*pos + 1], Token::Like) {
                        *pos += 1; // consume NOT (caller consumes LIKE)
                        BinaryOperator::NotLike
                    } else if *pos + 1 < tokens.len() && matches!(&tokens[*pos + 1], Token::ILike) {
                        *pos += 1; // consume NOT (caller consumes ILIKE)
                        // Assuming NotILike matches NotLike for now or creating NotILike
                        // Standard Postgres doesn't strictly have NOT ILIKE operator in AST always, 
                        // but let's check what I have. I'll use NotLike + ILike semantics if possible or just parse as NotLike? 
                        // Actually I don't have NotILike in my AST update earlier.
                        // I will skip NOT ILIKE for now or map to NotLike if acceptable (it's not).
                        // I will strictly handle SIMILAR TO for now.
                        break; 
                    } else {
                        break;
                    }
                } else {
                    match &tokens[*pos] {
                        Token::LessThan => BinaryOperator::LessThan,
                        Token::LessThanOrEqual => BinaryOperator::LessThanOrEqual,
                        Token::GreaterThan => BinaryOperator::GreaterThan,
                        Token::GreaterThanOrEqual => BinaryOperator::GreaterThanOrEqual,
                        Token::Like => BinaryOperator::Like,
                        Token::ILike => BinaryOperator::ILike,
                        Token::VectorDistance => BinaryOperator::VectorDistance,
                        Token::VectorInnerProduct => BinaryOperator::VectorInnerProduct,
                        Token::VectorCosineDistance => BinaryOperator::VectorCosineDistance,
                        // Range operators (PostgreSQL range types)
                        Token::RangeContains => BinaryOperator::RangeContains,
                        Token::RangeContainedBy => BinaryOperator::RangeContainedBy,
                        Token::RangeOverlaps => BinaryOperator::RangeOverlaps,
                        Token::RangeAdjacent => BinaryOperator::RangeAdjacent,
                        Token::RangeStrictlyLeft => BinaryOperator::RangeStrictlyLeft,
                        Token::RangeStrictlyRight => BinaryOperator::RangeStrictlyRight,
                        Token::RangeNotExtendRight => BinaryOperator::RangeNotExtendRight,
                        Token::RangeNotExtendLeft => BinaryOperator::RangeNotExtendLeft,
                        _ => break,
                    }
                };

                *pos += 1;
                let right = if *pos < tokens.len()
                    && matches!(&tokens[*pos], Token::Any | Token::Some | Token::All)
                {
                    match &tokens[*pos] {
                        Token::Any | Token::Some => {
                            *pos += 1;
                            let sub = self.parse_primary_expression(tokens, pos)?;
                            Expression::Any(Box::new(sub))
                        }
                        Token::All => {
                            *pos += 1;
                            let sub = self.parse_primary_expression(tokens, pos)?;
                            Expression::All(Box::new(sub))
                        }
                        _ => unreachable!(),
                    }
                } else {
                    self.parse_additive_expression(tokens, pos)?
                };
                left = Expression::Binary {
                    left: Box::new(left),
                    operator,
                    right: Box::new(right),
                };
            }
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
            return Err(crate::protocols::error::ProtocolError::ParseError(
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
            Token::Exists => {
                // EXISTS (SELECT ...)
                *pos += 1;
                // Parse subquery
                if *pos >= tokens.len() || !matches!(tokens[*pos], Token::LeftParen) {
                     return Err(crate::protocols::error::ProtocolError::ParseError(
                        "Expected '(' after EXISTS".to_string(),
                    ));
                }
                *pos += 1;
                
                use crate::protocols::postgres_wire::sql::parser::select::SelectParser;
                let mut select_parser = SelectParser::new();
                let subquery = select_parser.parse_select(tokens, pos)?;
                
                if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RightParen) {
                    return Err(crate::protocols::error::ProtocolError::ParseError(
                        "Expected ')' after EXISTS subquery".to_string(),
                    ));
                }
                *pos += 1;
                
                Ok(Expression::Exists(Box::new(subquery)))
            }
            _ => self.parse_postfix_expression(tokens, pos),
        }
    }

    /// Parse postfix expressions (JSONB operators, array indexing, type casting)
    /// This handles: ->, ->>, #>, #>>, [], ::
    fn parse_postfix_expression(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Expression> {
        let mut left = self.parse_primary_expression(tokens, pos)?;

        while *pos < tokens.len() {
            match &tokens[*pos] {
                Token::Arrow => {
                    // -> JSON field extraction
                    *pos += 1;
                    let right = self.parse_primary_expression(tokens, pos)?;
                    left = Expression::Binary {
                        left: Box::new(left),
                        operator: BinaryOperator::JsonExtract,
                        right: Box::new(right),
                    };
                }
                Token::JsonExtractText => {
                    // ->> JSON field extraction as text
                    *pos += 1;
                    let right = self.parse_primary_expression(tokens, pos)?;
                    left = Expression::Binary {
                        left: Box::new(left),
                        operator: BinaryOperator::JsonExtractText,
                        right: Box::new(right),
                    };
                }
                Token::JsonPathExtract => {
                    // #> JSON path extraction
                    *pos += 1;
                    let right = self.parse_primary_expression(tokens, pos)?;
                    left = Expression::Binary {
                        left: Box::new(left),
                        operator: BinaryOperator::JsonPathExtract,
                        right: Box::new(right),
                    };
                }
                Token::JsonPathExtractText => {
                    // #>> JSON path extraction as text
                    *pos += 1;
                    let right = self.parse_primary_expression(tokens, pos)?;
                    left = Expression::Binary {
                        left: Box::new(left),
                        operator: BinaryOperator::JsonPathExtractText,
                        right: Box::new(right),
                    };
                }
                Token::LeftBracket => {
                    // Array indexing
                    *pos += 1;
                    let index = self.parse_expression(tokens, pos)?;
                    if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RightBracket) {
                        return Err(crate::protocols::error::ProtocolError::ParseError(
                            "Expected ']' after array index".to_string(),
                        ));
                    }
                    *pos += 1;
                    left = Expression::ArrayIndex {
                        array: Box::new(left),
                        index: Box::new(index),
                    };
                }
                Token::Colon
                    if *pos + 1 < tokens.len() && matches!(tokens[*pos + 1], Token::Colon) =>
                {
                    // :: type cast (PostgreSQL style)
                    *pos += 2;
                    let target_type = self.parse_sql_type(tokens, pos)?;
                    left = Expression::Cast {
                        expr: Box::new(left),
                        target_type,
                    };
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse primary expressions (literals, identifiers, function calls, parenthesized expressions)
    fn parse_primary_expression(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Expression> {
        if *pos >= tokens.len() {
            return Err(crate::protocols::error::ProtocolError::ParseError(
                "Unexpected end of tokens".to_string(),
            ));
        }

        match &tokens[*pos] {
            Token::StringLiteral(s) => {
                *pos += 1;
                Ok(Expression::Literal(
                    crate::protocols::postgres_wire::sql::types::SqlValue::Text(s.clone()),
                ))
            }
            Token::NumericLiteral(n) => {
                *pos += 1;
                // Try to parse as integer first, then as decimal
                if let Ok(i) = n.parse::<i32>() {
                    Ok(Expression::Literal(
                        crate::protocols::postgres_wire::sql::types::SqlValue::Integer(i),
                    ))
                } else if let Ok(f) = n.parse::<f64>() {
                    Ok(Expression::Literal(
                        crate::protocols::postgres_wire::sql::types::SqlValue::DoublePrecision(f),
                    ))
                } else {
                    Ok(Expression::Literal(
                        crate::protocols::postgres_wire::sql::types::SqlValue::Text(n.clone()),
                    ))
                }
            }
            Token::BooleanLiteral(b) => {
                *pos += 1;
                Ok(Expression::Literal(
                    crate::protocols::postgres_wire::sql::types::SqlValue::Boolean(*b),
                ))
            }
            Token::Null => {
                *pos += 1;
                Ok(Expression::Literal(
                    crate::protocols::postgres_wire::sql::types::SqlValue::Null,
                ))
            }
            Token::Identifier(name) => {
                // Check for function call
                if *pos + 1 < tokens.len() && matches!(tokens[*pos + 1], Token::LeftParen) {
                    self.parse_function_call(tokens, pos, name.clone())
                } else {
                    *pos += 1;

                    // Check for Dot (qualified name)
                    if *pos < tokens.len() && matches!(tokens[*pos], Token::Dot) {
                        *pos += 1; // consume Dot
                                   // Check for wildcard
                        if *pos < tokens.len() && matches!(tokens[*pos], Token::Multiply) {
                            // This is table.*, which is usually handled in SELECT list, but could be an expression?
                            // Actually Expression::Column doesn't support wildcard.
                            // But wait, parse_select_list handles QualifiedWildcard separately.
                            // If we are here, we are parsing an expression.
                            // Maybe we should just return ColumnRef with name="*"?
                            // Or maybe we shouldn't handle wildcard here?
                            // Let's assume for now it's a column.
                            // But wait, if it IS table.*, parse_select_list checks for it explicitly BEFORE calling parse_expression.
                            // So we don't need to handle it here?
                            // Let's check parse_select_list in select.rs.
                            // It checks: if matches(Dot) && matches(Multiply) -> QualifiedWildcard.
                            // So we are safe.
                        }

                        if let Some(Token::Identifier(col_name)) = tokens.get(*pos) {
                            let col = col_name.clone();
                            *pos += 1;
                            Ok(Expression::Column(
                                crate::protocols::postgres_wire::sql::ast::ColumnRef {
                                    table: Some(name.clone()),
                                    name: col,
                                },
                            ))
                        } else {
                            Err(crate::protocols::error::ProtocolError::ParseError(
                                "Expected identifier after dot".to_string(),
                            ))
                        }
                    } else {
                        // Regular column reference
                        Ok(Expression::Column(
                            crate::protocols::postgres_wire::sql::ast::ColumnRef {
                                table: None,
                                name: name.clone(),
                            },
                        ))
                    }
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

            // Handle ARRAY constructor
            Token::Array => {
                *pos += 1; // consume 'ARRAY'

                if *pos >= tokens.len() || !matches!(tokens[*pos], Token::LeftBracket) {
                    return Err(crate::protocols::error::ProtocolError::ParseError(
                        "Expected '[' after ARRAY".to_string(),
                    ));
                }
                *pos += 1; // consume '['

                let mut elements = Vec::new();
                while *pos < tokens.len() && !matches!(tokens[*pos], Token::RightBracket) {
                    elements.push(self.parse_expression(tokens, pos)?);

                    if *pos < tokens.len() && matches!(tokens[*pos], Token::Comma) {
                        *pos += 1; // consume ','
                    } else {
                        break;
                    }
                }

                if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RightBracket) {
                    return Err(crate::protocols::error::ProtocolError::ParseError(
                        "Expected ']' after ARRAY elements".to_string(),
                    ));
                }
                *pos += 1; // consume ']'

                Ok(Expression::Array(elements))
            }

            // Handle CASE expressions
            Token::Case => self.parse_case_expression(tokens, pos),

            // Handle typed literals (INTERVAL '...', TIMESTAMP '...', DATE '...')
            Token::Interval => {
                *pos += 1; // consume INTERVAL
                if let Some(Token::StringLiteral(s)) = tokens.get(*pos) {
                    *pos += 1;
                    match crate::protocols::postgres_wire::sql::types::SqlValue::parse_interval(s) {
                        Ok(val) => Ok(Expression::Literal(val)),
                        Err(e) => Err(crate::protocols::error::ProtocolError::ParseError(e)),
                    }
                } else {
                    Err(crate::protocols::error::ProtocolError::ParseError(
                        "Expected string literal after INTERVAL".to_string(),
                    ))
                }
            }
            Token::Timestamp => {
                *pos += 1; // consume TIMESTAMP
                if let Some(Token::StringLiteral(s)) = tokens.get(*pos) {
                    *pos += 1;
                    // Default to no timezone for generic TIMESTAMP literal
                    match crate::protocols::postgres_wire::sql::types::SqlValue::parse_timestamp(
                        s, false,
                    ) {
                        Ok(val) => Ok(Expression::Literal(val)),
                        Err(e) => Err(crate::protocols::error::ProtocolError::ParseError(e)),
                    }
                } else {
                    Err(crate::protocols::error::ProtocolError::ParseError(
                        "Expected string literal after TIMESTAMP".to_string(),
                    ))
                }
            }
            Token::Date => {
                *pos += 1; // consume DATE
                if let Some(Token::StringLiteral(s)) = tokens.get(*pos) {
                    *pos += 1;
                    match crate::protocols::postgres_wire::sql::types::SqlValue::parse_date(s) {
                        Ok(val) => Ok(Expression::Literal(val)),
                        Err(e) => Err(crate::protocols::error::ProtocolError::ParseError(e)),
                    }
                } else {
                    Err(crate::protocols::error::ProtocolError::ParseError(
                        "Expected string literal after DATE".to_string(),
                    ))
                }
            }

            // Handle Date/Time functions
            Token::CurrentDate => {
                *pos += 1;
                Ok(Expression::CurrentDate)
            }
            Token::CurrentTime => {
                *pos += 1;
                let precision = self.parse_precision(tokens, pos);
                Ok(Expression::CurrentTime(precision))
            }
            Token::CurrentTimestamp => {
                *pos += 1;
                let precision = self.parse_precision(tokens, pos);
                Ok(Expression::CurrentTimestamp(precision))
            }
            Token::LocalTime => {
                *pos += 1;
                let precision = self.parse_precision(tokens, pos);
                Ok(Expression::LocalTime(precision))
            }
            Token::LocalTimestamp => {
                *pos += 1;
                let precision = self.parse_precision(tokens, pos);
                Ok(Expression::LocalTimestamp(precision))
            }

            // Handle CAST expressions
            Token::Cast => self.parse_cast_expression(tokens, pos),

            // Handle ANY/ALL/SOME (array comparison functions)
            Token::Any | Token::All | Token::Some => {
                let func_name = match &tokens[*pos] {
                    Token::Any => "ANY".to_string(),
                    Token::All => "ALL".to_string(),
                    Token::Some => "SOME".to_string(),
                    _ => unreachable!(),
                };
                self.parse_function_call(tokens, pos, func_name)
            }

            Token::LeftParen => {
                *pos += 1; // consume '('
                
                // Check if this is a subquery (SELECT ...)
                if *pos < tokens.len() && matches!(tokens[*pos], Token::Select) {
                    use crate::protocols::postgres_wire::sql::parser::select::SelectParser;
                    let mut select_parser = SelectParser::new();
                    let subquery = select_parser.parse_select(tokens, pos)?;
                    
                    if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RightParen) {
                        return Err(crate::protocols::error::ProtocolError::ParseError(
                            "Expected ')' after subquery".to_string(),
                        ));
                    }
                    *pos += 1; // consume ')'
                    Ok(Expression::Subquery(Box::new(subquery)))
                } else {
                    let expr = self.parse_expression(tokens, pos)?;
    
                    if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RightParen) {
                        return Err(crate::protocols::error::ProtocolError::ParseError(
                            "Expected ')' after expression".to_string(),
                        ));
                    }
                    *pos += 1; // consume ')'
    
                    Ok(expr)
                }
            }
            // MySQL/Postgres JSON functions
            Token::JsonObject => {
                 let func_name = "json_object".to_string();
                 self.parse_function_call(tokens, pos, func_name)
            }
            Token::JsonArray => {
                 let func_name = "json_array".to_string();
                 self.parse_function_call(tokens, pos, func_name)
            }
            Token::JsonQuery | Token::JsonValue | Token::JsonExists | Token::JsonTable | Token::JsonScalar | Token::JsonSerialize | Token::JsonArrayAgg | Token::JsonObjectAgg => {
                 // Map token to function name
                 if let Some(name) = self.token_to_identifier_name(&tokens[*pos]) {
                     self.parse_function_call(tokens, pos, name)
                 } else {
                     unreachable!()
                 }
            }
            Token::Select => {
                // Scalar Subquery without Parens? (Not standard, but maybe parser gets confused)
                // Or maybe test case has `SELECT (SELECT ...)`?
                // If we encounter `SELECT` here, parse as subquery
                use crate::protocols::postgres_wire::sql::parser::select::SelectParser;
                let mut select_parser = SelectParser::new();
                let subquery = select_parser.parse_select(tokens, pos)?;
                Ok(Expression::Subquery(Box::new(subquery)))
            }

            // Handle keywords that can be used as identifiers (like 'time', 'date', etc.)
            token => {
                // Try to convert keyword to identifier name
                if let Some(name) = self.token_to_identifier_name(token) {
                    // Check for function call
                    if *pos + 1 < tokens.len() && matches!(tokens[*pos + 1], Token::LeftParen) {
                        self.parse_function_call(tokens, pos, name)
                    } else {
                        *pos += 1;

                        // Check for Dot (qualified name)
                        if *pos < tokens.len() && matches!(tokens[*pos], Token::Dot) {
                            *pos += 1; // consume Dot
                            if let Some(Token::Identifier(col_name)) = tokens.get(*pos) {
                                let col = col_name.clone();
                                *pos += 1;
                                Ok(Expression::Column(
                                    crate::protocols::postgres_wire::sql::ast::ColumnRef {
                                        table: Some(name),
                                        name: col,
                                    },
                                ))
                            } else {
                                Err(crate::protocols::error::ProtocolError::ParseError(
                                    "Expected identifier after dot".to_string(),
                                ))
                            }
                        } else {
                            // Regular column reference
                            Ok(Expression::Column(
                                crate::protocols::postgres_wire::sql::ast::ColumnRef {
                                    table: None,
                                    name,
                                },
                            ))
                        }
                    }
                } else {
                    Err(crate::protocols::error::ProtocolError::ParseError(format!(
                        "Unexpected token in expression: {:?}",
                        tokens[*pos]
                    )))
                }
            }
        }
    }

    /// Extract identifier string from token (handles both Identifier and keyword tokens used as names)
    fn token_to_identifier_name(&self, token: &Token) -> Option<String> {
        match token {
            Token::Identifier(name) => Some(name.clone()),
            // Data type keywords that can be used as identifiers
            Token::Text => Some("text".to_string()),
            Token::Integer => Some("integer".to_string()),
            Token::Boolean => Some("boolean".to_string()),
            Token::Date => Some("date".to_string()),
            Token::Time => Some("time".to_string()),
            Token::Timestamp => Some("timestamp".to_string()),
            Token::Interval => Some("interval".to_string()),
            Token::Decimal => Some("decimal".to_string()),
            Token::Numeric => Some("numeric".to_string()),
            Token::Real => Some("real".to_string()),
            Token::Char => Some("char".to_string()),
            Token::Varchar => Some("varchar".to_string()),
            Token::Json => Some("json".to_string()),
            Token::Jsonb => Some("jsonb".to_string()),
            Token::Uuid => Some("uuid".to_string()),
            Token::Bytea => Some("bytea".to_string()),
            Token::Vector => Some("vector".to_string()),
            // Other keywords that can be used as identifiers
            Token::Sequence => Some("sequence".to_string()),
            Token::Key => Some("key".to_string()),
            // PostgreSQL 18 - OLD/NEW table references in RETURNING clause
            Token::Old => Some("OLD".to_string()),
            Token::New => Some("NEW".to_string()),
            // Extended DDL keywords that can be used as identifiers
            Token::Type => Some("type".to_string()),
            Token::Domain => Some("domain".to_string()),
            Token::Role => Some("role".to_string()),
            Token::User => Some("user".to_string()),
            Token::Tablespace => Some("tablespace".to_string()),
            Token::Policy => Some("policy".to_string()),
            Token::Rule => Some("rule".to_string()),
            Token::Aggregate => Some("aggregate".to_string()),
            Token::Operator => Some("operator".to_string()),
            Token::Collation => Some("collation".to_string()),
            Token::Conversion => Some("conversion".to_string()),
            Token::Statistics => Some("statistics".to_string()),
            Token::Publication => Some("publication".to_string()),
            Token::Subscription => Some("subscription".to_string()),
            // Security/Role keywords that can be used as identifiers
            Token::Login => Some("login".to_string()),
            Token::NoLogin => Some("nologin".to_string()),
            Token::SuperUser => Some("superuser".to_string()),
            Token::NoSuperUser => Some("nosuperuser".to_string()),
            Token::CreateDb => Some("createdb".to_string()),
            Token::NoCreateDb => Some("nocreatedb".to_string()),
            Token::CreateRole => Some("createrole".to_string()),
            Token::NoCreateRole => Some("nocreaterole".to_string()),
            Token::Inherit => Some("inherit".to_string()),
            Token::NoInherit => Some("noinherit".to_string()),
            Token::Replication => Some("replication".to_string()),
            Token::NoReplication => Some("noreplication".to_string()),
            Token::BypassRls => Some("bypassrls".to_string()),
            Token::NoBypassRls => Some("nobypassrls".to_string()),
            Token::ConnectionLimit => Some("connection".to_string()),
            Token::ValidUntil => Some("valid".to_string()),
            Token::Password => Some("password".to_string()),
            Token::Encrypted => Some("encrypted".to_string()),
            // Policy keywords
            Token::Permissive => Some("permissive".to_string()),
            Token::Restrictive => Some("restrictive".to_string()),
            // Type keywords
            Token::Enum => Some("enum".to_string()),
            Token::Composite => Some("composite".to_string()),
            // Window Functions
            Token::Rank => Some("rank".to_string()),
            Token::RowNumber => Some("row_number".to_string()),
            Token::DenseRank => Some("dense_rank".to_string()),
            Token::PercentRank => Some("percent_rank".to_string()),
            Token::CumeDist => Some("cume_dist".to_string()),
            Token::Ntile => Some("ntile".to_string()),
            Token::Lag => Some("lag".to_string()),
            Token::Lead => Some("lead".to_string()),
            Token::FirstValue => Some("first_value".to_string()),
            Token::LastValue => Some("last_value".to_string()),
            Token::NthValue => Some("nth_value".to_string()),
            // Other keywords
            Token::Exists => Some("exists".to_string()),
            Token::With => Some("with".to_string()),
            Token::Group => Some("group".to_string()),
            Token::Order => Some("order".to_string()),
            Token::By => Some("by".to_string()),
            Token::Window => Some("window".to_string()),
            Token::Index => Some("index".to_string()),
            // JSON tokens
            Token::JsonQuery => Some("json_query".to_string()),
            Token::JsonValue => Some("json_value".to_string()),
            Token::JsonExists => Some("json_exists".to_string()),
            Token::JsonTable => Some("json_table".to_string()),
            Token::JsonScalar => Some("json_scalar".to_string()),
            Token::JsonSerialize => Some("json_serialize".to_string()),
            Token::JsonArray => Some("json_array".to_string()),
            Token::JsonObject => Some("json_object".to_string()),
            Token::JsonArrayAgg => Some("json_arrayagg".to_string()),
            Token::JsonObjectAgg => Some("json_objectagg".to_string()),
            _ => None,
        }
    }

    /// Parse optional precision for date/time functions
    fn parse_precision(&self, tokens: &[Token], pos: &mut usize) -> Option<u32> {
        if *pos < tokens.len() && matches!(tokens[*pos], Token::LeftParen) {
            *pos += 1;
            let precision = if let Some(Token::NumericLiteral(s)) = tokens.get(*pos) {
                *pos += 1;
                s.parse::<u32>().ok()
            } else {
                None
            };
            
            if *pos < tokens.len() && matches!(tokens[*pos], Token::RightParen) {
                *pos += 1;
            }
            
            precision
        } else {
            None
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
            return Err(crate::protocols::error::ProtocolError::ParseError(
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

                // Check for ORDER BY inside function args (e.g. GROUP_CONCAT, string_agg)
                if *pos < tokens.len() && matches!(tokens[*pos], Token::Order) {
                    break; // Handled after loop
                }
                
                // Check for SEPARATOR (MySQL GROUP_CONCAT)
                if *pos < tokens.len() {
                    if let Token::Identifier(id) = &tokens[*pos] {
                        if id.eq_ignore_ascii_case("SEPARATOR") {
                            break; // Handled after loop
                        }
                    }
                }

                if *pos < tokens.len() && matches!(tokens[*pos], Token::Comma) {
                     // Lookahead for ORDER or SEPARATOR after comma (invalid but sometimes users type it?)
                     // Actually comma MUST separate args.
                     *pos += 1; // consume ','
                } else if *pos < tokens.len() && matches!(tokens[*pos], Token::RightParen) {
                    break; // End of args
                } else {
                     // Check again for ORDER/SEPARATOR as they might follow an arg without comma in some dialects? 
                     // No, usually comma separated. But MySQL GROUP_CONCAT(expr ORDER BY...) - no comma before ORDER BY.
                     // So if we are here, we check break conditions again.
                     if *pos < tokens.len() && matches!(tokens[*pos], Token::Order) {
                         break; 
                     }
                     if *pos < tokens.len() {
                        if let Token::Identifier(id) = &tokens[*pos] {
                            if id.eq_ignore_ascii_case("SEPARATOR") {
                                break;
                            }
                        }
                     }
                     // If still not matched, expecting comma or end
                     if !matches!(tokens[*pos], Token::RightParen) {
                         // Assume missing comma or special syntax, break to let outer parsing handle it?
                         // But we are in a loop collecting "args".
                         // If we break, "args" contains parsed expressions.
                         // Outer code expects RightParen.
                         break;
                     }
                }
            }
        }
        
        // Parse ORDER BY within function if present
        let mut agg_order_by = None;
        if *pos < tokens.len() && matches!(tokens[*pos], Token::Order) {
            *pos += 1; // consume ORDER
            if *pos < tokens.len() && matches!(tokens[*pos], Token::By) {
                *pos += 1; // consume BY
                agg_order_by = Some(self.parse_order_by_list(tokens, pos)?);
            } else {
                 return Err(crate::protocols::error::ProtocolError::ParseError(
                    "Expected BY after ORDER in function call".to_string(),
                )
                .into());
            }
        }
        
        // Parse SEPARATOR (MySQL) - consume but ignore for now (or store if AST supported)
        if *pos < tokens.len() {
            if let Token::Identifier(id) = &tokens[*pos] {
                if id.eq_ignore_ascii_case("SEPARATOR") {
                    *pos += 1;
                     // Expect string literal
                     if *pos < tokens.len() {
                         match &tokens[*pos] {
                             Token::StringLiteral(_) | Token::DollarQuotedString(_) => {
                                 *pos += 1;
                             }
                             _ => {
                                  return Err(crate::protocols::error::ProtocolError::ParseError(
                                    "Expected string literal after SEPARATOR".to_string(),
                                ).into());
                             }
                         }
                     }
                }
            }
        }

        if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RightParen) {
            return Err(crate::protocols::error::ProtocolError::ParseError(
                "Expected ')' after function arguments".to_string(),
            )
            .into());
        }
        *pos += 1; // consume ')'

        // Parse optional WITHIN GROUP clause
        let within_group = if *pos < tokens.len() && matches!(tokens[*pos], Token::Within) {
            *pos += 1; // consume WITHIN
            if *pos >= tokens.len() || !matches!(tokens[*pos], Token::Group) {
                return Err(crate::protocols::error::ProtocolError::ParseError(
                    "Expected GROUP after WITHIN".to_string(),
                )
                .into());
            }
            *pos += 1; // consume GROUP
            
            if *pos >= tokens.len() || !matches!(tokens[*pos], Token::LeftParen) {
                return Err(crate::protocols::error::ProtocolError::ParseError(
                    "Expected '(' after WITHIN GROUP".to_string(),
                )
                .into());
            }
            *pos += 1; // consume '('
            
            if *pos >= tokens.len() || !matches!(tokens[*pos], Token::Order) {
                return Err(crate::protocols::error::ProtocolError::ParseError(
                    "Expected ORDER BY within WITHIN GROUP".to_string(),
                )
                .into());
            }
            *pos += 1; // consume ORDER
            
            if *pos >= tokens.len() || !matches!(tokens[*pos], Token::By) {
                return Err(crate::protocols::error::ProtocolError::ParseError(
                    "Expected BY after ORDER".to_string(),
                )
                .into());
            }
            *pos += 1; // consume BY
            
            let items = self.parse_order_by_list(tokens, pos)?;
            
            if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RightParen) {
                return Err(crate::protocols::error::ProtocolError::ParseError(
                    "Expected ')' after WITHIN GROUP specification".to_string(),
                )
                .into());
            }
            *pos += 1; // consume ')'
            
            Some(items)
        } else {
            None
        };

        // Parse optional ORDER BY clause for aggregate functions (Postgres extension for some aggregates)
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
            // Ordered-set aggregates cannot be window functions with OVER clause
            if within_group.is_some() {
                 return Err(crate::protocols::error::ProtocolError::ParseError(
                    "OVER clause not allowed with WITHIN GROUP".to_string(),
                )
                .into());
            }
            self.parse_window_over_clause(tokens, pos, func_name, args, distinct, order_by.or(agg_order_by), filter)
        } else {
            Ok(Expression::Function(Box::new(FunctionCall {
                name: FunctionName::Simple(func_name),
                args,
                distinct,
                order_by: order_by.or(agg_order_by),
                filter,
                within_group,
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
                return Err(crate::protocols::error::ProtocolError::ParseError(
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
            within_group: None,
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
            || self.matches_at(tokens, *pos, &Token::Groups)
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
        // Parse frame mode: ROWS, RANGE, or GROUPS
        let mode = if self.matches_at(tokens, *pos, &Token::Rows) {
            *pos += 1;
            WindowFrameMode::Rows
        } else if self.matches_at(tokens, *pos, &Token::Range) {
            *pos += 1;
            WindowFrameMode::Range
        } else if self.matches_at(tokens, *pos, &Token::Groups) {
            *pos += 1;
            WindowFrameMode::Groups
        } else {
            WindowFrameMode::Range // default
        };

        // Check for optional BETWEEN keyword
        if self.matches_at(tokens, *pos, &Token::Between) {
            *pos += 1;
        }

        let start_bound = self.parse_frame_bound(tokens, pos)?;

        let end_bound = if self.matches_at(tokens, *pos, &Token::And) {
            *pos += 1;
            Some(self.parse_frame_bound(tokens, pos)?)
        } else {
            None
        };

        // Parse optional EXCLUDE clause
        let exclusion = if self.matches_at(tokens, *pos, &Token::Exclude) {
            *pos += 1;
            Some(self.parse_frame_exclusion(tokens, pos)?)
        } else {
            None
        };

        Ok(WindowFrame {
            mode,
            start_bound,
            end_bound,
            exclusion,
        })
    }

    /// Parse window frame exclusion
    fn parse_frame_exclusion(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<WindowFrameExclusion> {
        if self.matches_at(tokens, *pos, &Token::CurrentRow) {
            *pos += 1;
            // Skip ROW token if present (CURRENT ROW is two tokens)
            if self.matches_at(tokens, *pos, &Token::Row) {
                *pos += 1;
            }
            Ok(WindowFrameExclusion::CurrentRow)
        } else if self.matches_at(tokens, *pos, &Token::Group) {
            *pos += 1;
            Ok(WindowFrameExclusion::Group)
        } else if self.matches_at(tokens, *pos, &Token::Ties) {
            *pos += 1;
            Ok(WindowFrameExclusion::Ties)
        } else if let Some(Token::Identifier(id)) = tokens.get(*pos) {
            if id.to_uppercase() == "NO" {
                *pos += 1;
                if self.matches_at(tokens, *pos, &Token::Others) {
                    *pos += 1;
                    Ok(WindowFrameExclusion::NoOthers)
                } else {
                    Err(crate::protocols::error::ProtocolError::ParseError(
                        "Expected OTHERS after NO".to_string(),
                    )
                    .into())
                }
            } else {
                Err(crate::protocols::error::ProtocolError::ParseError(
                    "Expected CURRENT ROW, GROUP, TIES, or NO OTHERS after EXCLUDE".to_string(),
                )
                .into())
            }
        } else {
            Err(crate::protocols::error::ProtocolError::ParseError(
                "Expected CURRENT ROW, GROUP, TIES, or NO OTHERS after EXCLUDE".to_string(),
            )
            .into())
        }
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
                Err(crate::protocols::error::ProtocolError::ParseError(
                    "Expected PRECEDING or FOLLOWING after UNBOUNDED".to_string(),
                )
                .into())
            }
        } else if self.matches_at(tokens, *pos, &Token::CurrentRow) {
            // CURRENT is tokenized as CurrentRow, ROW is a separate token
            *pos += 1;
            // Skip the ROW token if present (CURRENT ROW is two tokens)
            if self.matches_at(tokens, *pos, &Token::Row) {
                *pos += 1;
            }
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
                Err(crate::protocols::error::ProtocolError::ParseError(
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
            Err(crate::protocols::error::ProtocolError::ParseError(format!(
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
                return Err(crate::protocols::error::ProtocolError::ParseError(
                    "Expected THEN after WHEN condition".to_string(),
                ));
            }
            *pos += 1; // consume 'THEN'

            let result = Box::new(self.parse_expression(tokens, pos)?);

            when_clauses.push(WhenClause { condition, result });
        }

        if when_clauses.is_empty() {
            return Err(crate::protocols::error::ProtocolError::ParseError(
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
            return Err(crate::protocols::error::ProtocolError::ParseError(
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
            return Err(crate::protocols::error::ProtocolError::ParseError(
                "Expected '(' after CAST".to_string(),
            ));
        }
        *pos += 1; // consume '('

        let expression = Box::new(self.parse_expression(tokens, pos)?);

        if *pos >= tokens.len() || !matches!(tokens[*pos], Token::As) {
            return Err(crate::protocols::error::ProtocolError::ParseError(
                "Expected AS in CAST expression".to_string(),
            ));
        }
        *pos += 1; // consume 'AS'

        let target_type = self.parse_sql_type(tokens, pos)?;

        if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RightParen) {
            return Err(crate::protocols::error::ProtocolError::ParseError(
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
    pub fn parse_sql_type(&mut self, tokens: &[Token], pos: &mut usize) -> ProtocolResult<SqlType> {
        if *pos >= tokens.len() {
            return Err(crate::protocols::error::ProtocolError::ParseError(
                "Expected data type".to_string(),
            ));
        }

        // Parse the base type first
        let base_type = self.parse_base_sql_type(tokens, pos)?;

        // Check for array suffix []
        self.check_array_suffix(tokens, pos, base_type)
    }

    /// Parse the base SQL type (without array suffix)
    fn parse_base_sql_type(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<SqlType> {
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
                    return Err(crate::protocols::error::ProtocolError::ParseError(
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
            Token::Vector => {
                *pos += 1;
                // Check for optional dimension specification
                if *pos < tokens.len() && matches!(tokens[*pos], Token::LeftParen) {
                    *pos += 1;
                    if let Some(Token::NumericLiteral(dim_str)) = tokens.get(*pos) {
                        if let Ok(dimensions) = dim_str.parse::<u32>() {
                            *pos += 1;
                            if *pos < tokens.len() && matches!(tokens[*pos], Token::RightParen) {
                                *pos += 1;
                                return Ok(SqlType::Vector {
                                    dimensions: Some(dimensions),
                                });
                            }
                        }
                    }
                    return Err(crate::protocols::error::ProtocolError::ParseError(
                        "Invalid VECTOR dimension specification".to_string(),
                    ));
                }
                Ok(SqlType::Vector { dimensions: None })
            }
            Token::HalfVec => {
                *pos += 1;
                // Check for optional dimension specification
                if *pos < tokens.len() && matches!(tokens[*pos], Token::LeftParen) {
                    *pos += 1;
                    if let Some(Token::NumericLiteral(dim_str)) = tokens.get(*pos) {
                        if let Ok(dimensions) = dim_str.parse::<u32>() {
                            *pos += 1;
                            if *pos < tokens.len() && matches!(tokens[*pos], Token::RightParen) {
                                *pos += 1;
                                return Ok(SqlType::HalfVec {
                                    dimensions: Some(dimensions),
                                });
                            }
                        }
                    }
                    return Err(crate::protocols::error::ProtocolError::ParseError(
                        "Invalid HALFVEC dimension specification".to_string(),
                    ));
                }
                Ok(SqlType::HalfVec { dimensions: None })
            }
            Token::SparseVec => {
                *pos += 1;
                // Check for optional dimension specification
                if *pos < tokens.len() && matches!(tokens[*pos], Token::LeftParen) {
                    *pos += 1;
                    if let Some(Token::NumericLiteral(dim_str)) = tokens.get(*pos) {
                        if let Ok(dimensions) = dim_str.parse::<u32>() {
                            *pos += 1;
                            if *pos < tokens.len() && matches!(tokens[*pos], Token::RightParen) {
                                *pos += 1;
                                return Ok(SqlType::SparseVec {
                                    dimensions: Some(dimensions),
                                });
                            }
                        }
                    }
                    return Err(crate::protocols::error::ProtocolError::ParseError(
                        "Invalid SPARSEVEC dimension specification".to_string(),
                    ));
                }
                Ok(SqlType::SparseVec { dimensions: None })
            }
            Token::Identifier(type_name) => {
                *pos += 1;
                Ok(SqlType::Custom {
                    type_name: type_name.clone(),
                })
            }
            _ => Err(crate::protocols::error::ProtocolError::ParseError(format!(
                "Unexpected token in data type: {:?}",
                tokens[*pos]
            ))),
        }
    }

    /// Check for array suffix [] and wrap base type in Array if present
    fn check_array_suffix(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
        base_type: SqlType,
    ) -> ProtocolResult<SqlType> {
        // Check for [] array suffix
        if *pos + 1 < tokens.len()
            && matches!(tokens[*pos], Token::LeftBracket)
            && matches!(tokens[*pos + 1], Token::RightBracket)
        {
            *pos += 2; // consume []
            Ok(SqlType::Array {
                element_type: Box::new(base_type),
                dimensions: None,
            })
        } else {
            Ok(base_type)
        }
    }
}

impl Default for ExpressionParser {
    fn default() -> Self {
        Self::new()
    }
}
