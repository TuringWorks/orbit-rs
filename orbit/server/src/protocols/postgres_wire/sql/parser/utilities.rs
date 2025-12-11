//! Parser Utilities
//!
//! Common parsing functions shared across different statement types

use super::{ParseError, ParseResult, SqlParser};
use crate::protocols::postgres_wire::sql::{
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
        Token::Money => Some("money".to_string()),
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
        Token::Add => Some("add".to_string()),
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
        // SQL/JSON keywords
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
        Token::Explain => Some("explain".to_string()),
        Token::Analyze => Some("analyze".to_string()),
        Token::Verbose => Some("verbose".to_string()),
        Token::Costs => Some("costs".to_string()),
        Token::Buffers => Some("buffers".to_string()),
        Token::Timing => Some("timing".to_string()),
        Token::Format => Some("format".to_string()),
        Token::Parallel => Some("parallel".to_string()),
        Token::Only => Some("only".to_string()),
        Token::Access => Some("access".to_string()),
        Token::Method => Some("method".to_string()),
        Token::Configuration => Some("configuration".to_string()),
        Token::Dictionary => Some("dictionary".to_string()),
        Token::Parser => Some("parser".to_string()),
        Token::Template => Some("template".to_string()),
        Token::Extension => Some("extension".to_string()),
        Token::Schema => Some("schema".to_string()),
        Token::Database => Some("database".to_string()),
        Token::Level => Some("level".to_string()),
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
    let base_type = match &parser.current_token {
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
        Some(Token::Money) => {
            parser.advance()?;
            Ok(SqlType::Money)
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
            // Handle common PostgreSQL type aliases
            match name.to_uppercase().as_str() {
                "INT" | "INT4" => Ok(SqlType::Integer),
                "INT2" => Ok(SqlType::SmallInt),
                "INT8" => Ok(SqlType::BigInt),
                "FLOAT4" => Ok(SqlType::Real),
                "FLOAT8" => Ok(SqlType::DoublePrecision),
                "BOOL" => Ok(SqlType::Boolean),
                "SERIAL" => Ok(SqlType::Integer), // SERIAL is INT with AUTO_INCREMENT
                "BIGSERIAL" => Ok(SqlType::BigInt),
                "SMALLSERIAL" => Ok(SqlType::SmallInt),
                "INET" => Ok(SqlType::Inet),
                "CIDR" => Ok(SqlType::Cidr),
                "MACADDR" => Ok(SqlType::Macaddr),
                // PostGIS types - handle GEOGRAPHY(POINT, 4326) and GEOMETRY(POLYGON) etc.
                "GEOGRAPHY" | "GEOMETRY" => {
                    // Consume optional type parameters
                    if parser.matches(&[Token::LeftParen]) {
                        let mut depth = 1;
                        parser.advance()?;
                        while depth > 0 {
                            match &parser.current_token {
                                Some(Token::LeftParen) => depth += 1,
                                Some(Token::RightParen) => depth -= 1,
                                None => break,
                                _ => {}
                            }
                            parser.advance()?;
                        }
                    }
                    Ok(SqlType::Custom { type_name: name })
                }
                _ => {
                    // Handle custom types with optional parameters, like VECTOR(3)
                    // but skip PostGIS-style parameters
                    Ok(SqlType::Custom { type_name: name })
                }
            }
        }
        _ => Err(ParseError {
            message: "Expected data type".to_string(),
            position: parser.position,
            expected: vec!["data type".to_string()],
            found: parser.current_token.clone(),
        }),
    }?;

    // Check for array brackets (TYPE[])
    if parser.matches(&[Token::LeftBracket]) {
        parser.advance()?;

        // Optional dimension (e.g., INTEGER[10])
        let dimensions = if let Some(Token::NumericLiteral(n)) = &parser.current_token {
            let dim = n.parse::<u32>().ok();
            parser.advance()?;
            dim
        } else {
            None
        };

        parser.expect(Token::RightBracket)?;

        Ok(SqlType::Array {
            element_type: Box::new(base_type),
            dimensions,
        })
    } else {
        Ok(base_type)
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
            // JSON operators
            Token::Question => BinaryOperator::JsonExists,
            Token::JsonbExistsAny => BinaryOperator::JsonExistsAny,
            Token::JsonbExistsAll => BinaryOperator::JsonExistsAll,
            Token::JsonPathExists => BinaryOperator::JsonPathExists,
            Token::JsonPathMatch => BinaryOperator::JsonPathMatch,
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
        _ => {
            let expr = parse_primary_expression(parser)?;
            // Check for postfix operators like :: cast
            parse_postfix_expression(parser, expr)
        }
    }
}

/// Parse postfix expressions (:: type cast, array indexing, etc.)
fn parse_postfix_expression(
    parser: &mut SqlParser,
    mut left: Expression,
) -> ParseResult<Expression> {
    loop {
        match &parser.current_token {
            // Handle :: type cast (PostgreSQL style)
            Some(Token::Colon) => {
                // Check if next token is also a colon (::)
                if let Some(Token::Colon) = parser.tokens.get(parser.position + 1) {
                    parser.advance()?; // consume first :
                    parser.advance()?; // consume second :
                    let target_type = parse_data_type(parser)?;
                    left = Expression::Cast {
                        expr: Box::new(left),
                        target_type,
                    };
                } else {
                    break;
                }
            }
            // Handle array indexing [n]
            Some(Token::LeftBracket) => {
                parser.advance()?;
                let index = parse_expression(parser)?;
                parser.expect(Token::RightBracket)?;
                left = Expression::ArrayIndex {
                    array: Box::new(left),
                    index: Box::new(index),
                };
            }
            _ => break,
        }
    }
    Ok(left)
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
        Some(Token::CurrentTimestamp) => {
            parser.advance()?;
            // Check for optional precision
            let precision = if parser.matches(&[Token::LeftParen]) {
                parser.advance()?;
                let p = if let Some(Token::NumericLiteral(s)) = &parser.current_token {
                    let val = s.parse::<u32>().ok();
                    parser.advance()?;
                    val
                } else {
                    None
                };
                parser.expect(Token::RightParen)?;
                p
            } else {
                None
            };
            Ok(Expression::CurrentTimestamp(precision))
        }
        Some(Token::LeftParen) => {
            parser.advance()?;
            let expr = parse_expression(parser)?;
            parser.expect(Token::RightParen)?;
            Ok(expr)
        }
        Some(token) => {
            // Check for typed literal (e.g. INTERVAL '1 hour')
            if matches!(
                token,
                Token::Interval | Token::Timestamp | Token::Date | Token::Time
            ) {
                let type_name = match token {
                    Token::Interval => "interval",
                    Token::Timestamp => "timestamp",
                    Token::Date => "date",
                    Token::Time => "time",
                    _ => unreachable!(),
                };

                // Check if next token is a string literal
                let literal_string =
                    if let Some(Token::StringLiteral(s)) = parser.tokens.get(parser.position + 1) {
                        Some(s.clone())
                    } else {
                        None
                    };

                if let Some(s) = literal_string {
                    parser.advance()?; // Consume type keyword
                    parser.advance()?; // Consume string literal

                    // Create a cast expression or specific literal
                    // For now, treat as text cast to type
                    return Ok(Expression::Cast {
                        expr: Box::new(Expression::Literal(SqlValue::Text(s))),
                        target_type: match type_name {
                            "interval" => SqlType::Interval,
                            "timestamp" => SqlType::Timestamp {
                                with_timezone: false,
                            },
                            "date" => SqlType::Date,
                            "time" => SqlType::Time {
                                with_timezone: false,
                            },
                            _ => SqlType::Text,
                        },
                    });
                }
            }

            // Check for ARRAY literal (ARRAY[...])
            if let Token::Identifier(name) = token {
                if name.to_uppercase() == "ARRAY" {
                    parser.advance()?;
                    parser.expect(Token::LeftBracket)?;

                    let mut elements = Vec::new();
                    if !parser.matches(&[Token::RightBracket]) {
                        loop {
                            elements.push(parse_expression(parser)?);
                            if parser.matches(&[Token::Comma]) {
                                parser.advance()?;
                            } else {
                                break;
                            }
                        }
                    }

                    parser.expect(Token::RightBracket)?;
                    return Ok(Expression::Array(elements));
                }
            }

            // Try to parse as identifier (including keywords)
            if let Some(name) = token_to_identifier_name(token) {
                parser.advance()?;

                // Check for function call
                if parser.matches(&[Token::LeftParen]) {
                    parser.advance()?;
                    let mut args = Vec::new();
                    if !parser.matches(&[Token::RightParen]) {
                        loop {
                            args.push(parse_expression(parser)?);
                            if parser.matches(&[Token::Comma]) {
                                parser.advance()?;
                            } else {
                                break;
                            }
                        }
                    }
                    parser.expect(Token::RightParen)?;

                    Ok(Expression::Function(Box::new(
                        crate::protocols::postgres_wire::sql::ast::FunctionCall {
                            name: crate::protocols::postgres_wire::sql::ast::FunctionName::Simple(
                                name,
                            ),
                            args,
                            distinct: false,
                            order_by: None,
                            filter: None,
                            within_group: None,
                        },
                    )))
                } else {
                    // Check for Dot (qualified name)
                    if parser.matches(&[Token::Dot]) {
                        parser.advance()?; // consume Dot
                        if let Some(col_name) = parser
                            .current_token
                            .as_ref()
                            .and_then(token_to_identifier_name)
                        {
                            parser.advance()?;
                            Ok(Expression::Column(ColumnRef {
                                table: Some(name),
                                name: col_name,
                            }))
                        } else {
                            Err(ParseError {
                                message: "Expected column name after dot".to_string(),
                                position: parser.position,
                                expected: vec!["identifier".to_string()],
                                found: parser.current_token.clone(),
                            })
                        }
                    } else {
                        Ok(Expression::Column(ColumnRef { table: None, name }))
                    }
                }
            } else {
                Err(ParseError {
                    message: "Expected expression".to_string(),
                    position: parser.position,
                    expected: vec!["literal, identifier, or parenthesized expression".to_string()],
                    found: parser.current_token.clone(),
                })
            }
        }
        None => Err(ParseError {
            message: "Unexpected end of input".to_string(),
            position: parser.position,
            expected: vec!["expression".to_string()],
            found: None,
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
    use crate::protocols::postgres_wire::sql::ast::Statement;
    use crate::protocols::postgres_wire::sql::parser::dml;

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

/// Check if a token represents a data type name
pub fn is_type_name(token: &Option<Token>) -> bool {
    match token {
        Some(Token::Boolean)
        | Some(Token::SmallInt)
        | Some(Token::Integer)
        | Some(Token::BigInt)
        | Some(Token::Real)
        | Some(Token::DoublePrecision)
        | Some(Token::Decimal)
        | Some(Token::Numeric)
        | Some(Token::Char)
        | Some(Token::Varchar)
        | Some(Token::Text)
        | Some(Token::Bytea)
        | Some(Token::Date)
        | Some(Token::Time)
        | Some(Token::Timestamp)
        | Some(Token::Interval)
        | Some(Token::Json)
        | Some(Token::Jsonb)
        | Some(Token::Uuid)
        | Some(Token::Vector)
        | Some(Token::HalfVec)
        | Some(Token::SparseVec) => true,
        // Check for common type aliases
        Some(Token::Identifier(name)) => {
            matches!(
                name.to_uppercase().as_str(),
                "INT"
                    | "INT2"
                    | "INT4"
                    | "INT8"
                    | "FLOAT4"
                    | "FLOAT8"
                    | "BOOL"
                    | "SERIAL"
                    | "BIGSERIAL"
                    | "SMALLSERIAL"
            )
        }
        _ => false,
    }
}
