//! Parser Utilities
//!
//! Common parsing functions shared across different statement types

use super::{ParseError, ParseResult, SqlParser};
use crate::postgres_wire::sql::{
    ast::*,
    lexer::Token,
    types::{SqlType, SqlValue},
};

/// Parse a table name (with optional schema qualification)
pub fn parse_table_name(parser: &mut SqlParser) -> ParseResult<TableName> {
    if let Some(Token::Identifier(name)) = &parser.current_token {
        let first_name = name.clone();
        parser.advance()?;

        // Check for schema qualification
        if parser.matches(&[Token::Dot]) {
            parser.advance()?;

            if let Some(Token::Identifier(table_name)) = &parser.current_token {
                let table_name = table_name.clone();
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

/// Parse an expression (placeholder implementation)
pub fn parse_expression(parser: &mut SqlParser) -> ParseResult<Expression> {
    // For now, this is a very basic implementation
    // TODO: Implement full expression parsing with precedence
    match &parser.current_token {
        Some(Token::StringLiteral(s)) => {
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
        _ => Err(ParseError {
            message: "Expected expression".to_string(),
            position: parser.position,
            expected: vec!["expression".to_string()],
            found: parser.current_token.clone(),
        }),
    }
}

/// Parse a SELECT statement (placeholder implementation)
pub fn parse_select_statement(parser: &mut SqlParser) -> ParseResult<SelectStatement> {
    Err(ParseError {
        message: "SELECT statement parsing not yet implemented in utilities".to_string(),
        position: parser.position,
        expected: vec![],
        found: parser.current_token.clone(),
    })
}
