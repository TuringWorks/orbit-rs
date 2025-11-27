//! Parser Utilities
//!
//! Common parsing functions shared across different statement types

use super::{ParseError, ParseResult, SqlParser};
use crate::postgres_wire::sql::{
    ast::{BinaryOperator, ColumnRef, Expression, SelectStatement, TableName, UnaryOperator},
    lexer::Token,
    types::{SqlType, SqlValue},
};

/// Extract identifier string from token (handles both Identifier and keyword tokens used as names)
pub fn token_to_identifier_name(token: &Token) -> Option<String> {
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
        _ => None,
    }
}

/// Parse a table name (with optional schema qualification)
pub fn parse_table_name(parser: &mut SqlParser) -> ParseResult<TableName> {
    if let Some(first_name) = parser
        .current_token
        .as_ref()
        .and_then(token_to_identifier_name)
    {
        parser.advance()?;

        // Check for schema qualification
        if parser.matches(&[Token::Dot]) {
            parser.advance()?;

            if let Some(table_name) = parser
                .current_token
                .as_ref()
                .and_then(token_to_identifier_name)
            {
                parser.advance()?;
                Ok(TableName::with_schema(first_name, table_name))
            } else {
                Err(ParseError {
                    message: "Expected table name after schema qualifier".to_string(),
                    position: parser.position,
                    expected: vec!["table name".to_string()],
                    found: parser.current_token.clone(),
                })
            }
        } else {
            Ok(TableName::new(first_name))
        }
    } else {
        Err(ParseError {
            message: "Expected table name".to_string(),
            position: parser.position,
            expected: vec!["table name".to_string()],
            found: parser.current_token.clone(),
        })
    }
}

/// Parse a SQL data type
pub fn parse_data_type(parser: &mut SqlParser) -> ParseResult<SqlType> {
    match &parser.current_token {
        Some(Token::Boolean) => {
            parser.advance()?;
            Ok(SqlType::Boolean)
        }
        Some(Token::SmallInt) => {
            parser.advance()?;
            Ok(SqlType::SmallInt)
        }
        Some(Token::Integer) => {
            parser.advance()?;
            Ok(SqlType::Integer)
        }
        Some(Token::BigInt) => {
            parser.advance()?;
            Ok(SqlType::BigInt)
        }
        Some(Token::Real) => {
            parser.advance()?;
            Ok(SqlType::Real)
        }
        Some(Token::DoublePrecision) => {
            parser.advance()?;
            // Handle "DOUBLE PRECISION" as two tokens
            if parser.matches(&[Token::DoublePrecision]) {
                parser.advance()?;
            }
            Ok(SqlType::DoublePrecision)
        }
        Some(Token::Decimal) | Some(Token::Numeric) => {
            let is_numeric = matches!(parser.current_token, Some(Token::Numeric));
            parser.advance()?;

            // Parse optional precision and scale
            let (precision, scale) = if parser.matches(&[Token::LeftParen]) {
                parser.advance()?;

                let precision = if let Some(Token::NumericLiteral(num)) = &parser.current_token {
                    let p = num.parse::<u8>().map_err(|_| ParseError {
                        message: "Invalid precision value".to_string(),
                        position: parser.position,
                        expected: vec!["integer".to_string()],
                        found: parser.current_token.clone(),
                    })?;
                    parser.advance()?;
                    Some(p)
                } else {
                    return Err(ParseError {
                        message: "Expected precision value".to_string(),
                        position: parser.position,
                        expected: vec!["integer".to_string()],
                        found: parser.current_token.clone(),
                    });
                };

                let scale = if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                    if let Some(Token::NumericLiteral(num)) = &parser.current_token {
                        let s = num.parse::<u8>().map_err(|_| ParseError {
                            message: "Invalid scale value".to_string(),
                            position: parser.position,
                            expected: vec!["integer".to_string()],
                            found: parser.current_token.clone(),
                        })?;
                        parser.advance()?;
                        Some(s)
                    } else {
                        return Err(ParseError {
                            message: "Expected scale value".to_string(),
                            position: parser.position,
                            expected: vec!["integer".to_string()],
                            found: parser.current_token.clone(),
                        });
                    }
                } else {
                    None
                };

                parser.expect(Token::RightParen)?;
                (precision, scale)
            } else {
                (None, None)
            };

            if is_numeric {
                Ok(SqlType::Numeric { precision, scale })
            } else {
                Ok(SqlType::Decimal { precision, scale })
            }
        }
        Some(Token::Char) => {
            parser.advance()?;
            let length = if parser.matches(&[Token::LeftParen]) {
                parser.advance()?;
                if let Some(Token::NumericLiteral(num)) = &parser.current_token {
                    let len = num.parse::<u32>().map_err(|_| ParseError {
                        message: "Invalid character length".to_string(),
                        position: parser.position,
                        expected: vec!["integer".to_string()],
                        found: parser.current_token.clone(),
                    })?;
                    parser.advance()?;
                    parser.expect(Token::RightParen)?;
                    Some(len)
                } else {
                    return Err(ParseError {
                        message: "Expected character length".to_string(),
                        position: parser.position,
                        expected: vec!["integer".to_string()],
                        found: parser.current_token.clone(),
                    });
                }
            } else {
                None
            };
            Ok(SqlType::Char(length))
        }
        Some(Token::Varchar) => {
            parser.advance()?;
            let length = if parser.matches(&[Token::LeftParen]) {
                parser.advance()?;
                if let Some(Token::NumericLiteral(num)) = &parser.current_token {
                    let len = num.parse::<u32>().map_err(|_| ParseError {
                        message: "Invalid varchar length".to_string(),
                        position: parser.position,
                        expected: vec!["integer".to_string()],
                        found: parser.current_token.clone(),
                    })?;
                    parser.advance()?;
                    parser.expect(Token::RightParen)?;
                    Some(len)
                } else {
                    return Err(ParseError {
                        message: "Expected varchar length".to_string(),
                        position: parser.position,
                        expected: vec!["integer".to_string()],
                        found: parser.current_token.clone(),
                    });
                }
            } else {
                None
            };
            Ok(SqlType::Varchar(length))
        }
        Some(Token::Text) => {
            parser.advance()?;
            Ok(SqlType::Text)
        }
        Some(Token::Bytea) => {
            parser.advance()?;
            Ok(SqlType::Bytea)
        }
        Some(Token::Date) => {
            parser.advance()?;
            Ok(SqlType::Date)
        }
        Some(Token::Time) => {
            parser.advance()?;
            let with_timezone = if parser.matches(&[Token::With]) {
                parser.advance()?;
                parser.expect(Token::Time)?;
                parser.expect(Token::Zone)?;
                true
            } else if parser.matches(&[Token::Without]) {
                parser.advance()?;
                parser.expect(Token::Time)?;
                parser.expect(Token::Zone)?;
                false
            } else {
                false
            };
            Ok(SqlType::Time { with_timezone })
        }
        Some(Token::Timestamp) => {
            parser.advance()?;
            let with_timezone = if parser.matches(&[Token::With]) {
                parser.advance()?;
                parser.expect(Token::Time)?;
                parser.expect(Token::Zone)?;
                true
            } else if parser.matches(&[Token::Without]) {
                parser.advance()?;
                parser.expect(Token::Time)?;
                parser.expect(Token::Zone)?;
                false
            } else {
                false
            };
            Ok(SqlType::Timestamp { with_timezone })
        }
        Some(Token::Interval) => {
            parser.advance()?;
            Ok(SqlType::Interval)
        }
        Some(Token::Json) => {
            parser.advance()?;
            Ok(SqlType::Json)
        }
        Some(Token::Jsonb) => {
            parser.advance()?;
            Ok(SqlType::Jsonb)
        }
        Some(Token::Uuid) => {
            parser.advance()?;
            Ok(SqlType::Uuid)
        }
        Some(Token::Vector) => {
            parser.advance()?;
            let dimensions = if parser.matches(&[Token::LeftParen]) {
                parser.advance()?;
                if let Some(Token::NumericLiteral(num)) = &parser.current_token {
                    let dims = num.parse::<u32>().map_err(|_| ParseError {
                        message: "Invalid vector dimensions".to_string(),
                        position: parser.position,
                        expected: vec!["integer".to_string()],
                        found: parser.current_token.clone(),
                    })?;
                    parser.advance()?;
                    parser.expect(Token::RightParen)?;
                    Some(dims)
                } else {
                    return Err(ParseError {
                        message: "Expected vector dimensions".to_string(),
                        position: parser.position,
                        expected: vec!["integer".to_string()],
                        found: parser.current_token.clone(),
                    });
                }
            } else {
                None
            };
            Ok(SqlType::Vector { dimensions })
        }
        Some(Token::HalfVec) => {
            parser.advance()?;
            let dimensions = if parser.matches(&[Token::LeftParen]) {
                parser.advance()?;
                if let Some(Token::NumericLiteral(num)) = &parser.current_token {
                    let dims = num.parse::<u32>().map_err(|_| ParseError {
                        message: "Invalid halfvec dimensions".to_string(),
                        position: parser.position,
                        expected: vec!["integer".to_string()],
                        found: parser.current_token.clone(),
                    })?;
                    parser.advance()?;
                    parser.expect(Token::RightParen)?;
                    Some(dims)
                } else {
                    return Err(ParseError {
                        message: "Expected halfvec dimensions".to_string(),
                        position: parser.position,
                        expected: vec!["integer".to_string()],
                        found: parser.current_token.clone(),
                    });
                }
            } else {
                None
            };
            Ok(SqlType::HalfVec { dimensions })
        }
        Some(Token::SparseVec) => {
            parser.advance()?;
            let dimensions = if parser.matches(&[Token::LeftParen]) {
                parser.advance()?;
                if let Some(Token::NumericLiteral(num)) = &parser.current_token {
                    let dims = num.parse::<u32>().map_err(|_| ParseError {
                        message: "Invalid sparsevec dimensions".to_string(),
                        position: parser.position,
                        expected: vec!["integer".to_string()],
                        found: parser.current_token.clone(),
                    })?;
                    parser.advance()?;
                    parser.expect(Token::RightParen)?;
                    Some(dims)
                } else {
                    return Err(ParseError {
                        message: "Expected sparsevec dimensions".to_string(),
                        position: parser.position,
                        expected: vec!["integer".to_string()],
                        found: parser.current_token.clone(),
                    });
                }
            } else {
                None
            };
            Ok(SqlType::SparseVec { dimensions })
        }
        Some(Token::Identifier(type_name)) => {
            let name = type_name.clone();
            parser.advance()?;
            Ok(SqlType::Custom { type_name: name })
        }
        _ => Err(ParseError {
            message: "Expected data type".to_string(),
            position: parser.position,
            expected: vec!["data type".to_string()],
            found: parser.current_token.clone(),
        }),
    }
}

/// Parse a literal SQL value
pub fn parse_literal_value(parser: &mut SqlParser) -> ParseResult<SqlValue> {
    match &parser.current_token {
        Some(Token::StringLiteral(s)) => {
            let value = s.clone();
            parser.advance()?;
            Ok(SqlValue::Text(value))
        }
        Some(Token::NumericLiteral(n)) => {
            let value = n.clone();
            parser.advance()?;

            // Try to parse as different numeric types
            if value.contains('.') {
                if let Ok(f) = value.parse::<f64>() {
                    Ok(SqlValue::DoublePrecision(f))
                } else {
                    Err(ParseError {
                        message: "Invalid numeric literal".to_string(),
                        position: parser.position,
                        expected: vec!["valid number".to_string()],
                        found: Some(Token::NumericLiteral(value)),
                    })
                }
            } else if let Ok(i) = value.parse::<i32>() {
                Ok(SqlValue::Integer(i))
            } else if let Ok(i) = value.parse::<i64>() {
                Ok(SqlValue::BigInt(i))
            } else {
                Err(ParseError {
                    message: "Invalid numeric literal".to_string(),
                    position: parser.position,
                    expected: vec!["valid number".to_string()],
                    found: Some(Token::NumericLiteral(value)),
                })
            }
        }
        Some(Token::BooleanLiteral(b)) => {
            let value = *b;
            parser.advance()?;
            Ok(SqlValue::Boolean(value))
        }
        Some(Token::Null) => {
            parser.advance()?;
            Ok(SqlValue::Null)
        }
        _ => Err(ParseError {
            message: "Expected literal value".to_string(),
            position: parser.position,
            expected: vec!["string, number, boolean, or null".to_string()],
            found: parser.current_token.clone(),
        }),
    }
}

/// Parse an expression with proper operator precedence
pub fn parse_expression(parser: &mut SqlParser) -> ParseResult<Expression> {
    parse_or_expression(parser)
}

/// Parse OR expressions (lowest precedence)
fn parse_or_expression(parser: &mut SqlParser) -> ParseResult<Expression> {
    let mut left = parse_and_expression(parser)?;

    while parser.matches(&[Token::Or]) {
        parser.advance()?;
        let right = parse_and_expression(parser)?;
        left = Expression::Binary {
            left: Box::new(left),
            operator: BinaryOperator::Or,
            right: Box::new(right),
        };
    }

    Ok(left)
}

/// Parse AND expressions
fn parse_and_expression(parser: &mut SqlParser) -> ParseResult<Expression> {
    let mut left = parse_equality_expression(parser)?;

    while parser.matches(&[Token::And]) {
        parser.advance()?;
        let right = parse_equality_expression(parser)?;
        left = Expression::Binary {
            left: Box::new(left),
            operator: BinaryOperator::And,
            right: Box::new(right),
        };
    }

    Ok(left)
}

/// Parse equality expressions (=, !=, <>, IS, IS NOT)
fn parse_equality_expression(parser: &mut SqlParser) -> ParseResult<Expression> {
    let mut left = parse_comparison_expression(parser)?;

    while let Some(token) = &parser.current_token {
        let operator = match token {
            Token::Equal => BinaryOperator::Equal,
            Token::NotEqual => BinaryOperator::NotEqual,
            Token::Is => {
                parser.advance()?;
                if parser.matches(&[Token::Not]) {
                    parser.advance()?;
                    BinaryOperator::IsNot
                } else {
                    BinaryOperator::Is
                }
            }
            _ => break,
        };

        if !matches!(operator, BinaryOperator::Is | BinaryOperator::IsNot) {
            parser.advance()?;
        }

        let right = parse_comparison_expression(parser)?;
        left = Expression::Binary {
            left: Box::new(left),
            operator,
            right: Box::new(right),
        };
    }

    Ok(left)
}

/// Parse comparison expressions (<, <=, >, >=, LIKE, IN, BETWEEN, vector operators)
fn parse_comparison_expression(parser: &mut SqlParser) -> ParseResult<Expression> {
    let mut left = parse_additive_expression(parser)?;

    while let Some(token) = &parser.current_token {
        let operator = match token {
            Token::LessThan => BinaryOperator::LessThan,
            Token::LessThanOrEqual => BinaryOperator::LessThanOrEqual,
            Token::GreaterThan => BinaryOperator::GreaterThan,
            Token::GreaterThanOrEqual => BinaryOperator::GreaterThanOrEqual,
            Token::Like => BinaryOperator::Like,
            Token::ILike => BinaryOperator::ILike,
            Token::In => BinaryOperator::In,
            // Vector distance operators
            Token::VectorDistance => BinaryOperator::VectorDistance,
            Token::VectorInnerProduct => BinaryOperator::VectorInnerProduct,
            Token::VectorCosineDistance => BinaryOperator::VectorCosineDistance,
            _ => break,
        };

        parser.advance()?;
        let right = parse_additive_expression(parser)?;
        left = Expression::Binary {
            left: Box::new(left),
            operator,
            right: Box::new(right),
        };
    }

    Ok(left)
}

/// Parse additive expressions (+, -, ||)
fn parse_additive_expression(parser: &mut SqlParser) -> ParseResult<Expression> {
    let mut left = parse_multiplicative_expression(parser)?;

    while let Some(token) = &parser.current_token {
        let operator = match token {
            Token::Plus => BinaryOperator::Plus,
            Token::Minus => BinaryOperator::Minus,
            Token::Concat => BinaryOperator::Concat,
            _ => break,
        };

        parser.advance()?;
        let right = parse_multiplicative_expression(parser)?;
        left = Expression::Binary {
            left: Box::new(left),
            operator,
            right: Box::new(right),
        };
    }

    Ok(left)
}

/// Parse multiplicative expressions (*, /, %)
fn parse_multiplicative_expression(parser: &mut SqlParser) -> ParseResult<Expression> {
    let mut left = parse_unary_expression(parser)?;

    while let Some(token) = &parser.current_token {
        let operator = match token {
            Token::Multiply => BinaryOperator::Multiply,
            Token::Divide => BinaryOperator::Divide,
            Token::Modulo => BinaryOperator::Modulo,
            _ => break,
        };

        parser.advance()?;
        let right = parse_unary_expression(parser)?;
        left = Expression::Binary {
            left: Box::new(left),
            operator,
            right: Box::new(right),
        };
    }

    Ok(left)
}

/// Parse unary expressions (NOT, -, +)
fn parse_unary_expression(parser: &mut SqlParser) -> ParseResult<Expression> {
    match &parser.current_token {
        Some(Token::Not) => {
            parser.advance()?;
            let expr = parse_unary_expression(parser)?;
            Ok(Expression::Unary {
                operator: UnaryOperator::Not,
                operand: Box::new(expr),
            })
        }
        Some(Token::Minus) => {
            parser.advance()?;
            let expr = parse_unary_expression(parser)?;
            Ok(Expression::Unary {
                operator: UnaryOperator::Minus,
                operand: Box::new(expr),
            })
        }
        Some(Token::Plus) => {
            parser.advance()?;
            let expr = parse_unary_expression(parser)?;
            Ok(Expression::Unary {
                operator: UnaryOperator::Plus,
                operand: Box::new(expr),
            })
        }
        _ => parse_primary_expression(parser),
    }
}

/// Parse primary expressions (literals, identifiers, parenthesized expressions)
fn parse_primary_expression(parser: &mut SqlParser) -> ParseResult<Expression> {
    match &parser.current_token {
        Some(Token::StringLiteral(s)) => {
            // Check if this looks like a vector literal [1,2,3]
            if s.starts_with('[') && s.ends_with(']') {
                // Try to parse as vector literal
                let inner = &s[1..s.len() - 1];
                if let Ok(values) = parse_vector_elements(inner) {
                    let value = SqlValue::Vector(values);
                    parser.advance()?;
                    return Ok(Expression::Literal(value));
                }
            }
            // Fall back to string literal
            let value = SqlValue::Text(s.clone());
            parser.advance()?;
            Ok(Expression::Literal(value))
        }
        Some(Token::NumericLiteral(n)) => {
            let value = if n.contains('.') {
                SqlValue::DoublePrecision(n.parse().unwrap_or(0.0))
            } else {
                SqlValue::Integer(n.parse().unwrap_or(0))
            };
            parser.advance()?;
            Ok(Expression::Literal(value))
        }
        Some(Token::BooleanLiteral(b)) => {
            let value = SqlValue::Boolean(*b);
            parser.advance()?;
            Ok(Expression::Literal(value))
        }
        Some(Token::Null) => {
            parser.advance()?;
            Ok(Expression::Literal(SqlValue::Null))
        }
        Some(Token::Identifier(name)) => {
            let col_name = name.clone();
            parser.advance()?;
            Ok(Expression::Column(ColumnRef {
                table: None,
                name: col_name,
            }))
        }
        Some(Token::LeftParen) => {
            parser.advance()?;
            let expr = parse_expression(parser)?;
            parser.expect(Token::RightParen)?;
            Ok(expr)
        }
        _ => Err(ParseError {
            message: "Expected expression".to_string(),
            position: parser.position,
            expected: vec!["literal, identifier, or parenthesized expression".to_string()],
            found: parser.current_token.clone(),
        }),
    }
}

/// Parse vector elements from a comma-separated string like "1,2,3"
fn parse_vector_elements(s: &str) -> Result<Vec<f32>, ()> {
    if s.trim().is_empty() {
        return Ok(Vec::new());
    }

    s.split(',')
        .map(|element| element.trim().parse::<f32>().map_err(|_| ()))
        .collect()
}

/// Parse a SELECT statement (used in CREATE VIEW, etc.)
pub fn parse_select_statement(parser: &mut SqlParser) -> ParseResult<SelectStatement> {
    // Use the DML parser to parse SELECT, then extract the SelectStatement
    // Note: dml::parse_select expects SELECT to be the current token
    use crate::postgres_wire::sql::ast::Statement;
    use crate::postgres_wire::sql::parser::dml;

    // Ensure we have SELECT token
    if !parser.matches(&[Token::Select]) {
        return Err(ParseError {
            message: "Expected SELECT statement".to_string(),
            position: parser.position,
            expected: vec!["SELECT".to_string()],
            found: parser.current_token.clone(),
        });
    }

    match dml::parse_select(parser)? {
        Statement::Select(select_stmt) => Ok(*select_stmt),
        _ => Err(ParseError {
            message: "Expected SELECT statement".to_string(),
            position: parser.position,
            expected: vec!["SELECT".to_string()],
            found: parser.current_token.clone(),
        }),
    }
}
