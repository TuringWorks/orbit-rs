//! DML (Data Manipulation Language) Parser Implementation
//!
//! This module handles parsing of SELECT, INSERT, UPDATE, DELETE statements

// Parser loops use complex exit conditions that don't translate cleanly to while let
#![allow(clippy::while_let_loop)]
// Identical blocks intentional for code symmetry in operator parsing
#![allow(clippy::if_same_then_else)]

use super::expressions::ExpressionParser;
use super::{utilities, ParseError, ParseResult, SqlParser};
use crate::protocols::postgres_wire::sql::{
    ast::{
        Assignment, AssignmentTarget, ColumnRef, ConflictAction, ConflictTarget, CopyDirection,
        CopyFormat, CopyHeaderOption, CopyOnError, CopyOption, CopySource, CopyStatement,
        CopyTarget, DeleteStatement, DistinctClause, Expression, FromClause, InsertSource,
        InsertStatement, JoinCondition, JoinType, JsonTable, JsonTableColumn, LimitClause,
        MergeAction, MergeInsert, MergeInsertValues, MergeStatement, MergeUpdate, MergeWhenClause,
        NullsOrder, OnConflictClause, OrderByItem, SelectItem, SelectStatement, SetOperation,
        SetOperator, SortDirection, Statement, TableAlias, TraverseClause, TraverseDirection,
        UpdateStatement,
    },
    lexer::Token,
    types::SqlValue,
};

/// Helper function to parse expressions using the proper expression parser
fn parse_expression_with_parser(parser: &mut SqlParser) -> ParseResult<Expression> {
    let mut expr_parser = ExpressionParser::new();
    let tokens = &parser.tokens;
    let mut pos = parser.position;
    let expr = expr_parser
        .parse_expression(tokens, &mut pos)
        .map_err(|e| ParseError {
            message: e.to_string(),
            position: parser.position,
            expected: vec!["expression".to_string()],
            found: parser.current_token.clone(),
        })?;

    // Update parser position
    parser.position = pos;
    parser.current_token = parser.tokens.get(parser.position).cloned();

    Ok(expr)
}

/// Parse SELECT statement
pub fn parse_select(parser: &mut SqlParser) -> ParseResult<Statement> {
    // Parse WITH clause using SelectParser logic
    let with = if parser.matches(&[Token::With]) {
        let mut select_parser = super::select::SelectParser::new();
        let with_clause = select_parser
            .parse_with_clause(&parser.tokens, &mut parser.position)
            .map_err(|e| ParseError {
                message: e.to_string(),
                position: parser.position,
                expected: vec!["WITH clause".to_string()],
                found: parser.current_token.clone(),
            })?;
        // Update current_token as SelectParser advances position
        parser.current_token = parser.tokens.get(parser.position).cloned();
        Some(with_clause)
    } else {
        None
    };

    parser.expect(Token::Select)?;

    // Parse DISTINCT clause, including `DISTINCT ON (expr, ...)`.
    let distinct = if parser.matches(&[Token::Distinct]) {
        parser.advance()?;
        if parser.matches(&[Token::On]) {
            parser.advance()?;
            if !parser.matches(&[Token::LeftParen]) {
                return Err(ParseError {
                    message: "Expected '(' after DISTINCT ON".to_string(),
                    position: parser.position,
                    expected: vec!["(".to_string()],
                    found: parser.current_token.clone(),
                });
            }
            parser.advance()?;
            let mut keys = Vec::new();
            loop {
                keys.push(parse_expression_with_parser(parser)?);
                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }
            if !parser.matches(&[Token::RightParen]) {
                return Err(ParseError {
                    message: "Expected ')' after DISTINCT ON list".to_string(),
                    position: parser.position,
                    expected: vec![")".to_string()],
                    found: parser.current_token.clone(),
                });
            }
            parser.advance()?;
            Some(DistinctClause::DistinctOn(keys))
        } else {
            Some(DistinctClause::Distinct)
        }
    } else {
        None
    };

    // Parse select list (columns)
    let mut select_list = Vec::new();

    loop {
        let item = if parser.matches(&[Token::Multiply]) {
            parser.advance()?;
            SelectItem::Wildcard
        } else {
            // Check for table.* syntax
            let is_qualified_wildcard = if let Some(Token::Identifier(_)) = &parser.current_token {
                if let Some(Token::Dot) = parser.tokens.get(parser.position + 1) {
                    matches!(
                        parser.tokens.get(parser.position + 2),
                        Some(Token::Multiply)
                    )
                } else {
                    false
                }
            } else {
                false
            };

            if is_qualified_wildcard {
                if let Some(Token::Identifier(name)) = &parser.current_token {
                    let qualifier = name.clone();
                    parser.advance()?; // Identifier
                    parser.advance()?; // Dot
                    parser.advance()?; // Multiply
                    SelectItem::QualifiedWildcard { qualifier }
                } else {
                    unreachable!()
                }
            } else {
                // Parse as expression using proper expression parser
                // This handles columns, functions, literals, etc.
                let expr = parse_expression_with_parser(parser)?;

                // Check for alias
                let alias = if parser.matches(&[Token::As]) {
                    parser.advance()?;
                    if let Some(alias_name) = parser
                        .current_token
                        .as_ref()
                        .and_then(super::utilities::token_to_identifier_name)
                    {
                        parser.advance()?;
                        Some(alias_name)
                    } else {
                        None
                    }
                } else if let Some(alias_name) = parser
                    .current_token
                    .as_ref()
                    .and_then(super::utilities::token_to_identifier_name)
                {
                    // Check if this might be an alias (not a reserved word like FROM, WHERE, etc.)
                    // Don't consume if next token is FROM, WHERE, GROUP BY, ORDER BY, or LIMIT
                    if !parser.matches(&[
                        Token::From,
                        Token::Where,
                        Token::Group,
                        Token::Order,
                        Token::Limit,
                        Token::Comma, // Also don't treat as alias if followed by comma
                    ]) {
                        parser.advance()?;
                        Some(alias_name)
                    } else {
                        None
                    }
                } else {
                    None
                };

                SelectItem::Expression { expr, alias }
            }
        };

        select_list.push(item);

        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }

    // Parse FROM clause
    let from_clause = if parser.matches(&[Token::From]) {
        parser.advance()?;
        // `FROM a, b` is a cross join written with a comma, and
        // `FROM a, LATERAL (...)` is how a lateral subquery is usually
        // spelled. Parsing only one item left the comma as the start of a new
        // statement.
        let mut from = parse_from_clause(parser)?;
        while parser.matches(&[Token::Comma]) {
            parser.advance()?;
            let right = parse_from_clause(parser)?;
            from = FromClause::Join {
                left: Box::new(from),
                join_type: JoinType::Cross,
                right: Box::new(right),
                condition: JoinCondition::On(Expression::Literal(SqlValue::Boolean(true))),
            };
        }
        Some(from)
    } else {
        None
    };

    // Parse WHERE clause
    let where_clause = if parser.matches(&[Token::Where]) {
        parser.advance()?;
        Some(parse_where_expression(parser)?)
    } else {
        None
    };

    // Parse GROUP BY clause
    let group_by = if parser.matches(&[Token::Group]) {
        parser.advance()?;
        parser.expect(Token::By)?;

        let mut group_exprs = Vec::new();
        loop {
            group_exprs.push(parse_expression_with_parser(parser)?);

            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }

        Some(group_exprs)
    } else {
        None
    };

    // Parse HAVING clause
    let having = if parser.matches(&[Token::Having]) {
        parser.advance()?;
        Some(parse_expression_with_parser(parser)?)
    } else {
        None
    };

    // Parse ORDER BY clause
    let order_by = if parser.matches(&[Token::Order]) {
        parser.advance()?;
        parser.expect(Token::By)?;
        Some(parse_order_by_clause(parser)?)
    } else {
        None
    };

    // Parse LIMIT clause
    let (limit, offset) = if parser.matches(&[Token::Limit]) {
        parser.advance()?;
        let limit_expr = parse_expression_with_parser(parser)?;

        let offset = if parser.matches(&[Token::Offset]) {
            parser.advance()?;
            let offset_expr = parse_expression_with_parser(parser)?;
            if let Expression::Literal(SqlValue::Integer(n)) = offset_expr {
                Some(n as u64)
            } else {
                return Err(ParseError {
                    message: "OFFSET must be a non-negative integer".to_string(),
                    position: parser.position,
                    expected: vec!["integer".to_string()],
                    found: parser.current_token.clone(),
                });
            }
        } else {
            None
        };

        (
            Some(LimitClause {
                count: Some(limit_expr),
                with_ties: false,
            }),
            offset,
        )
    } else {
        (None, None)
    };

    // Parse TRAVERSE clause (OrbitQL extension)
    let traverse = if let Some(Token::Identifier(name)) = &parser.current_token {
        if name.to_uppercase() == "TRAVERSE" {
            Some(parse_traverse_clause(parser)?)
        } else {
            None
        }
    } else {
        None
    };

    // Check for set operations (UNION, INTERSECT, EXCEPT)
    let set_operation = if parser.matches(&[Token::Union, Token::Intersect, Token::Except]) {
        Some(parse_set_operation(parser)?)
    } else {
        None
    };

    // Create SELECT statement with ORDER BY, LIMIT, TRAVERSE, and set operation
    Ok(Statement::Select(Box::new(SelectStatement {
        with,
        select_list,
        distinct,
        from_clause,
        where_clause,
        group_by,
        having,
        order_by,
        limit,
        offset,
        for_clause: None,
        traverse,
        set_operation,
    })))
}

/// Parse a TRAVERSE clause: TRAVERSE <Direction> <Min>..<Max> STEPS ON <Edge> [TO <Alias>]
fn parse_traverse_clause(parser: &mut SqlParser) -> ParseResult<TraverseClause> {
    parser.advance()?; // consume TRAVERSE

    // Parse Direction
    let direction = if let Some(Token::Identifier(name)) = &parser.current_token {
        match name.to_uppercase().as_str() {
            "OUTBOUND" => TraverseDirection::Outbound,
            "INBOUND" => TraverseDirection::Inbound,
            "ANY" => TraverseDirection::Any,
            _ => {
                return Err(ParseError {
                    message: format!("Expected OUTBOUND, INBOUND, or ANY, found {}", name),
                    position: parser.position,
                    expected: vec![
                        "OUTBOUND".to_string(),
                        "INBOUND".to_string(),
                        "ANY".to_string(),
                    ],
                    found: parser.current_token.clone(),
                })
            }
        }
    } else {
        return Err(ParseError {
            message: "Expected traversal direction".to_string(),
            position: parser.position,
            expected: vec!["direction".to_string()],
            found: parser.current_token.clone(),
        });
    };
    parser.advance()?;

    // Parse Min Steps
    let min_steps = if let Some(Token::NumericLiteral(s)) = &parser.current_token {
        s.parse::<u32>().map_err(|_| ParseError {
            message: format!("Invalid integer for min steps: {}", s),
            position: parser.position,
            expected: vec!["integer".to_string()],
            found: parser.current_token.clone(),
        })?
    } else {
        return Err(ParseError {
            message: "Expected minimum steps".to_string(),
            position: parser.position,
            expected: vec!["integer".to_string()],
            found: parser.current_token.clone(),
        });
    };
    parser.advance()?;

    // Parse .. (Range)
    parser.expect(Token::RangeOperator)?;

    // Parse Max Steps
    let max_steps = if let Some(Token::NumericLiteral(s)) = &parser.current_token {
        s.parse::<u32>().map_err(|_| ParseError {
            message: format!("Invalid integer for max steps: {}", s),
            position: parser.position,
            expected: vec!["integer".to_string()],
            found: parser.current_token.clone(),
        })?
    } else {
        return Err(ParseError {
            message: "Expected maximum steps".to_string(),
            position: parser.position,
            expected: vec!["integer".to_string()],
            found: parser.current_token.clone(),
        });
    };
    parser.advance()?;

    // Parse STEPS (optional or required? Query has it)
    if let Some(Token::Identifier(name)) = &parser.current_token {
        if name.to_uppercase() == "STEPS" {
            parser.advance()?;
        }
    }

    // Parse ON
    if parser.matches(&[Token::On]) {
        parser.advance()?;
    } else {
        // Check if ON is parsed as Identifier (if not a keyword)
        if let Some(Token::Identifier(name)) = &parser.current_token {
            if name.to_uppercase() == "ON" {
                parser.advance()?;
            } else {
                return Err(ParseError {
                    message: "Expected ON".to_string(),
                    position: parser.position,
                    expected: vec!["ON".to_string()],
                    found: parser.current_token.clone(),
                });
            }
        } else {
            // ON is likely a keyword in lexer, let's assume expect(Token::On) works if it is
            // But if it's not in lexer, we need to handle it.
            // Lexer has Token::On.
            parser.expect(Token::On)?;
        }
    }

    // Parse Edge Collection
    let edge_collection = if let Some(Token::Identifier(name)) = &parser.current_token {
        name.clone()
    } else {
        return Err(ParseError {
            message: "Expected edge collection name".to_string(),
            position: parser.position,
            expected: vec!["identifier".to_string()],
            found: parser.current_token.clone(),
        });
    };
    parser.advance()?;

    // Parse TO (optional)
    let target_alias = if parser.matches(&[Token::To]) {
        parser.advance()?;
        if let Some(Token::Identifier(name)) = &parser.current_token {
            let alias = name.clone();
            parser.advance()?;
            Some(alias)
        } else {
            return Err(ParseError {
                message: "Expected target alias after TO".to_string(),
                position: parser.position,
                expected: vec!["identifier".to_string()],
                found: parser.current_token.clone(),
            });
        }
    } else {
        None
    };

    Ok(TraverseClause {
        direction,
        min_steps,
        max_steps,
        edge_collection,
        target_alias,
    })
}

/// Parse set operation (UNION, INTERSECT, EXCEPT) and the right-hand SELECT
fn parse_set_operation(parser: &mut SqlParser) -> ParseResult<SetOperation> {
    // Determine the operator
    let operator = match &parser.current_token {
        Some(Token::Union) => {
            parser.advance()?;
            // Check for ALL
            if parser.matches(&[Token::All]) {
                parser.advance()?;
                SetOperator::UnionAll
            } else if parser.matches(&[Token::Distinct]) {
                parser.advance()?;
                SetOperator::Union
            } else {
                SetOperator::Union
            }
        }
        Some(Token::Intersect) => {
            parser.advance()?;
            // Check for ALL
            if parser.matches(&[Token::All]) {
                parser.advance()?;
                SetOperator::IntersectAll
            } else if parser.matches(&[Token::Distinct]) {
                parser.advance()?;
                SetOperator::Intersect
            } else {
                SetOperator::Intersect
            }
        }
        Some(Token::Except) => {
            parser.advance()?;
            // Check for ALL
            if parser.matches(&[Token::All]) {
                parser.advance()?;
                SetOperator::ExceptAll
            } else if parser.matches(&[Token::Distinct]) {
                parser.advance()?;
                SetOperator::Except
            } else {
                SetOperator::Except
            }
        }
        _ => {
            return Err(ParseError {
                message: "Expected UNION, INTERSECT, or EXCEPT".to_string(),
                position: parser.position,
                expected: vec![
                    "UNION".to_string(),
                    "INTERSECT".to_string(),
                    "EXCEPT".to_string(),
                ],
                found: parser.current_token.clone(),
            });
        }
    };

    // Now parse the right-hand SELECT statement
    // We need to parse a SELECT statement recursively
    parser.expect(Token::Select)?;

    // Parse the inner SELECT (without calling parse_select to avoid returning Statement)
    let right_stmt = parse_select_inner(parser)?;

    Ok(SetOperation {
        operator,
        right: Box::new(right_stmt),
    })
}

/// Parse the inner SELECT statement (returns SelectStatement directly, not Statement)
fn parse_select_inner(parser: &mut SqlParser) -> ParseResult<SelectStatement> {
    // Parse DISTINCT clause
    let distinct = if parser.matches(&[Token::Distinct]) {
        parser.advance()?;
        Some(DistinctClause::Distinct)
    } else {
        None
    };

    // Parse select list (columns)
    let mut select_list = Vec::new();

    loop {
        let item = if parser.matches(&[Token::Multiply]) {
            parser.advance()?;
            SelectItem::Wildcard
        } else {
            // Check for table.* syntax
            let is_qualified_wildcard = if let Some(Token::Identifier(_)) = &parser.current_token {
                if let Some(Token::Dot) = parser.tokens.get(parser.position + 1) {
                    matches!(
                        parser.tokens.get(parser.position + 2),
                        Some(Token::Multiply)
                    )
                } else {
                    false
                }
            } else {
                false
            };

            if is_qualified_wildcard {
                if let Some(Token::Identifier(name)) = &parser.current_token {
                    let qualifier = name.clone();
                    parser.advance()?; // Identifier
                    parser.advance()?; // Dot
                    parser.advance()?; // Multiply
                    SelectItem::QualifiedWildcard { qualifier }
                } else {
                    unreachable!()
                }
            } else {
                let expr = parse_expression_with_parser(parser)?;

                // Check for alias
                let alias = if parser.matches(&[Token::As]) {
                    parser.advance()?;
                    if let Some(Token::Identifier(alias_name)) = &parser.current_token {
                        let alias = alias_name.clone();
                        parser.advance()?;
                        Some(alias)
                    } else {
                        None
                    }
                } else if let Some(Token::Identifier(alias_name)) = &parser.current_token {
                    if !parser.matches(&[
                        Token::From,
                        Token::Where,
                        Token::Group,
                        Token::Order,
                        Token::Limit,
                        Token::Comma,
                        Token::Union,
                        Token::Intersect,
                        Token::Except,
                    ]) {
                        let alias = alias_name.clone();
                        parser.advance()?;
                        Some(alias)
                    } else {
                        None
                    }
                } else {
                    None
                };

                SelectItem::Expression { expr, alias }
            }
        };

        select_list.push(item);

        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }

    // Parse FROM clause
    let from_clause = if parser.matches(&[Token::From]) {
        parser.advance()?;
        Some(parse_from_clause(parser)?)
    } else {
        None
    };

    // Parse WHERE clause
    let where_clause = if parser.matches(&[Token::Where]) {
        parser.advance()?;
        Some(parse_where_expression(parser)?)
    } else {
        None
    };

    // Parse GROUP BY clause
    let group_by = if parser.matches(&[Token::Group]) {
        parser.advance()?;
        parser.expect(Token::By)?;

        let mut group_exprs = Vec::new();
        loop {
            group_exprs.push(parse_expression_with_parser(parser)?);

            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }

        Some(group_exprs)
    } else {
        None
    };

    // Parse HAVING clause
    let having = if parser.matches(&[Token::Having]) {
        parser.advance()?;
        Some(parse_expression_with_parser(parser)?)
    } else {
        None
    };

    // Parse ORDER BY clause
    let order_by = if parser.matches(&[Token::Order]) {
        parser.advance()?;
        parser.expect(Token::By)?;
        Some(parse_order_by_clause(parser)?)
    } else {
        None
    };

    // Parse LIMIT clause
    let (limit, offset) = if parser.matches(&[Token::Limit]) {
        parser.advance()?;
        let limit_expr = parse_expression_with_parser(parser)?;

        let offset = if parser.matches(&[Token::Offset]) {
            parser.advance()?;
            let offset_expr = parse_expression_with_parser(parser)?;
            if let Expression::Literal(SqlValue::Integer(n)) = offset_expr {
                Some(n as u64)
            } else {
                return Err(ParseError {
                    message: "OFFSET must be a non-negative integer".to_string(),
                    position: parser.position,
                    expected: vec!["integer".to_string()],
                    found: parser.current_token.clone(),
                });
            }
        } else {
            None
        };

        (
            Some(LimitClause {
                count: Some(limit_expr),
                with_ties: false,
            }),
            offset,
        )
    } else {
        (None, None)
    };

    // Check for chained set operations
    let set_operation = if parser.matches(&[Token::Union, Token::Intersect, Token::Except]) {
        Some(parse_set_operation(parser)?)
    } else {
        None
    };

    Ok(SelectStatement {
        with: None,
        select_list,
        distinct,
        from_clause,
        where_clause,
        group_by,
        having,
        order_by,
        limit,
        offset,
        for_clause: None,
        traverse: None,
        set_operation,
    })
}

/// Parse FROM clause
fn parse_from_clause(parser: &mut SqlParser) -> ParseResult<FromClause> {
    // `LATERAL (SELECT ...) alias`: the keyword marks a subquery that may read
    // the rows to its left. Without it here the parser looked for a table name
    // and reported "Expected table name".
    let lateral = parser.matches(&[Token::Lateral]);
    if lateral {
        parser.advance()?;
        let mut from = parse_from_clause(parser)?;
        if let FromClause::Subquery {
            lateral: ref mut flag,
            ..
        } = from
        {
            *flag = true;
        }
        return Ok(from);
    }

    // Check for JSON_TABLE
    if let Some(Token::Identifier(name)) = &parser.current_token {
        if name.to_uppercase() == "JSON_TABLE" {
            // Check if next token is LeftParen
            if let Some(Token::LeftParen) = parser.peek() {
                return parse_json_table(parser);
            }
        }
    }

    // Check for (VALUES ...) or (SELECT ...)
    if parser.matches(&[Token::LeftParen]) {
        if let Some(Token::Values) = parser.peek() {
            parser.advance()?; // consume (
            parser.advance()?; // consume VALUES

            let mut value_lists = Vec::new();
            loop {
                parser.expect(Token::LeftParen)?;
                let mut values = Vec::new();
                while !parser.matches(&[Token::RightParen]) {
                    values.push(utilities::parse_expression(parser)?);
                    if parser.matches(&[Token::Comma]) {
                        parser.advance()?;
                    } else {
                        break;
                    }
                }
                parser.expect(Token::RightParen)?;
                value_lists.push(values);

                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }

            parser.expect(Token::RightParen)?; // consume closing )

            let alias = parse_alias_clause(parser)?;
            return Ok(FromClause::Values {
                values: value_lists,
                alias,
            });
        } else if let Some(Token::Select) = parser.peek() {
            parser.advance()?; // consume (
                               // Parse subquery
            let stmt = parse_select(parser)?;
            parser.expect(Token::RightParen)?; // consume closing )

            let alias = parse_alias_clause(parser)?;

            if let Statement::Select(select_stmt) = stmt {
                return Ok(FromClause::Subquery {
                    query: select_stmt,
                    alias: alias.unwrap_or_else(|| TableAlias {
                        name: "subquery".to_string(),
                        columns: None,
                    }),
                    lateral: false,
                });
            } else {
                return Err(ParseError {
                    message: "Expected SELECT statement in subquery".to_string(),
                    position: parser.position,
                    expected: vec!["SELECT".to_string()],
                    found: parser.current_token.clone(),
                });
            }
        }
    }

    // Parse the first table reference
    let table_name = utilities::parse_table_name(parser)?;

    // Parse time travel clause if present (comes before alias in SQL syntax)
    // Note: UPDATE/DELETE will ignore this as they're parsed before getting here
    let time_travel = {
        let mut select_parser = super::select::SelectParser::new();
        let tt = select_parser
            .parse_time_travel_clause(&parser.tokens, &mut parser.position)
            .map_err(|e| ParseError {
                message: e.to_string(),
                position: parser.position,
                expected: vec!["time travel clause".to_string()],
                found: parser.current_token.clone(),
            })?;
        // Update current_token as SelectParser advances position
        parser.current_token = parser.tokens.get(parser.position).cloned();
        tt
    };

    // Check for table alias (comes after time travel clause)
    let alias = parse_alias_clause(parser)?;

    let mut left = FromClause::Table {
        name: table_name,
        alias,
        time_travel,
    };

    // Check for JOINs and parse them recursively
    while is_join_keyword(parser) {
        left = parse_join(parser, left)?;
    }

    Ok(left)
}

/// Check if current token is a JOIN keyword
fn is_join_keyword(parser: &SqlParser) -> bool {
    matches!(
        &parser.current_token,
        Some(Token::Join)
            | Some(Token::Inner)
            | Some(Token::Left)
            | Some(Token::Right)
            | Some(Token::Full)
            | Some(Token::Cross)
            | Some(Token::Natural)
    )
}

/// Parse a JOIN clause
fn parse_join(parser: &mut SqlParser, left: FromClause) -> ParseResult<FromClause> {
    use crate::protocols::postgres_wire::sql::ast::{JoinCondition, JoinType};

    // `NATURAL JOIN` takes no ON or USING: the columns both sides share are
    // the condition. Without this the keyword ended the FROM clause and the
    // rest of the statement was read as a new one.
    let natural = parser.matches(&[Token::Natural]);
    if natural {
        parser.advance()?;
    }

    // Determine join type
    let join_type = match &parser.current_token {
        Some(Token::Join) => {
            parser.advance()?;
            JoinType::Inner
        }
        Some(Token::Inner) => {
            parser.advance()?;
            parser.expect(Token::Join)?;
            JoinType::Inner
        }
        Some(Token::Left) => {
            parser.advance()?;
            // OUTER is optional
            if parser.matches(&[Token::Outer]) {
                parser.advance()?;
            }
            parser.expect(Token::Join)?;
            JoinType::LeftOuter
        }
        Some(Token::Right) => {
            parser.advance()?;
            // OUTER is optional
            if parser.matches(&[Token::Outer]) {
                parser.advance()?;
            }
            parser.expect(Token::Join)?;
            JoinType::RightOuter
        }
        Some(Token::Full) => {
            parser.advance()?;
            // OUTER is optional
            if parser.matches(&[Token::Outer]) {
                parser.advance()?;
            }
            parser.expect(Token::Join)?;
            JoinType::FullOuter
        }
        Some(Token::Cross) => {
            parser.advance()?;
            parser.expect(Token::Join)?;
            JoinType::Cross
        }
        // `NATURAL JOIN` with no INNER/LEFT/... in front is an inner join.
        _ if natural => JoinType::Inner,
        _ => {
            return Err(ParseError {
                message: "Expected JOIN keyword".to_string(),
                position: parser.position,
                expected: vec![
                    "JOIN".to_string(),
                    "INNER JOIN".to_string(),
                    "LEFT JOIN".to_string(),
                    "RIGHT JOIN".to_string(),
                    "FULL JOIN".to_string(),
                    "CROSS JOIN".to_string(),
                ],
                found: parser.current_token.clone(),
            });
        }
    };

    // Parse the right table
    let right_table_name = utilities::parse_table_name(parser)?;
    let right_alias = parse_alias_clause(parser)?;
    let right = FromClause::Table {
        name: right_table_name,
        alias: right_alias,
        time_travel: None, // UPDATE/DELETE don't support time travel
    };

    // Parse join condition (ON or USING)
    let condition = if natural {
        JoinCondition::Natural
    } else if join_type == JoinType::Cross {
        // CROSS JOIN has no condition
        JoinCondition::Natural // Use Natural as a placeholder for no condition
    } else if parser.matches(&[Token::On]) {
        parser.advance()?;
        let expr = parse_expression_with_parser(parser)?;
        JoinCondition::On(expr)
    } else if parser.matches(&[Token::Using]) {
        parser.advance()?;
        parser.expect(Token::LeftParen)?;
        let mut columns = Vec::new();
        loop {
            if let Some(Token::Identifier(col)) = &parser.current_token {
                columns.push(col.clone());
                parser.advance()?;
            } else {
                break;
            }
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
        JoinCondition::Using(columns)
    } else {
        // Natural join
        JoinCondition::Natural
    };

    Ok(FromClause::Join {
        left: Box::new(left),
        join_type,
        right: Box::new(right),
        condition,
    })
}

fn parse_alias_clause(parser: &mut SqlParser) -> ParseResult<Option<TableAlias>> {
    if parser.matches(&[Token::As]) {
        parser.advance()?;
        if let Some(Token::Identifier(alias_name)) = &parser.current_token {
            let name = alias_name.clone();
            parser.advance()?;

            let columns = if parser.matches(&[Token::LeftParen]) {
                parser.advance()?;
                let mut cols = Vec::new();
                while !parser.matches(&[Token::RightParen]) {
                    if let Some(Token::Identifier(c)) = &parser.current_token {
                        cols.push(c.clone());
                        parser.advance()?;
                        if parser.matches(&[Token::Comma]) {
                            parser.advance()?;
                        } else {
                            break;
                        }
                    } else {
                        return Err(ParseError {
                            message: "Expected column name in alias".to_string(),
                            position: parser.position,
                            expected: vec!["column name".to_string()],
                            found: parser.current_token.clone(),
                        });
                    }
                }
                parser.expect(Token::RightParen)?;
                Some(cols)
            } else {
                None
            };

            Ok(Some(TableAlias { name, columns }))
        } else {
            Ok(None)
        }
    } else if let Some(Token::Identifier(alias_name)) = &parser.current_token {
        // Check if this might be an alias (not a reserved word)
        if !parser.matches(&[
            Token::Where,
            Token::Group,
            Token::Order,
            Token::Limit,
            Token::Join,
            Token::On,
            Token::When,
        ]) {
            // Check for TRAVERSE identifier (OrbitQL extension)
            if alias_name.to_uppercase() == "TRAVERSE" {
                Ok(None)
            } else {
                let name = alias_name.clone();
                parser.advance()?;

                let columns = if parser.matches(&[Token::LeftParen]) {
                    parser.advance()?;
                    let mut cols = Vec::new();
                    while !parser.matches(&[Token::RightParen]) {
                        if let Some(Token::Identifier(c)) = &parser.current_token {
                            cols.push(c.clone());
                            parser.advance()?;
                            if parser.matches(&[Token::Comma]) {
                                parser.advance()?;
                            } else {
                                break;
                            }
                        } else {
                            return Err(ParseError {
                                message: "Expected column name in alias".to_string(),
                                position: parser.position,
                                expected: vec!["column name".to_string()],
                                found: parser.current_token.clone(),
                            });
                        }
                    }
                    parser.expect(Token::RightParen)?;
                    Some(cols)
                } else {
                    None
                };

                Ok(Some(TableAlias { name, columns }))
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

/// Parse JSON_TABLE function
fn parse_json_table(parser: &mut SqlParser) -> ParseResult<FromClause> {
    parser.advance()?; // consume JSON_TABLE
    parser.expect(Token::LeftParen)?;

    // Parse context item (JSON document)
    let context_item = parse_expression_with_parser(parser)?;
    parser.expect(Token::Comma)?;

    // Parse path expression
    let path_expression = parse_expression_with_parser(parser)?;

    // Check for optional comma before COLUMNS
    if parser.matches(&[Token::Comma]) {
        parser.advance()?;
    }

    let mut columns = Vec::new();
    if let Some(Token::Identifier(s)) = &parser.current_token {
        if s.to_uppercase() == "COLUMNS" {
            parser.advance()?;
            parser.expect(Token::LeftParen)?;

            loop {
                // Parse column definition: name type [PATH path]
                let name = if let Some(Token::Identifier(n)) = &parser.current_token {
                    n.clone()
                } else {
                    break;
                };
                parser.advance()?;

                // Parse type using ExpressionParser directly since we need parse_sql_type
                let mut expr_parser = ExpressionParser::new();
                let mut pos = parser.position;
                let data_type = expr_parser
                    .parse_sql_type(&parser.tokens, &mut pos)
                    .map_err(|e| ParseError {
                        message: e.to_string(),
                        position: parser.position,
                        expected: vec!["data type".to_string()],
                        found: parser.current_token.clone(),
                    })?;
                parser.position = pos;
                parser.current_token = parser.tokens.get(parser.position).cloned();

                let mut path = None;
                if let Some(Token::Identifier(p)) = &parser.current_token {
                    if p.to_uppercase() == "PATH" {
                        parser.advance()?;
                        if let Some(Token::StringLiteral(path_str)) = &parser.current_token {
                            path = Some(path_str.clone());
                            parser.advance()?;
                        } else {
                            return Err(ParseError {
                                message: "Expected path string literal".to_string(),
                                position: parser.position,
                                expected: vec!["string literal".to_string()],
                                found: parser.current_token.clone(),
                            });
                        }
                    }
                }

                columns.push(JsonTableColumn {
                    name,
                    data_type,
                    path,
                });

                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }

            parser.expect(Token::RightParen)?;
        }
    }

    parser.expect(Token::RightParen)?;

    // Parse alias
    let alias = parse_alias_clause(parser)?;

    Ok(FromClause::JsonTable(JsonTable {
        context_item,
        path_expression,
        columns,
        alias,
    }))
}

/// Parse WHERE expression with basic comparison operators
fn parse_where_expression(parser: &mut SqlParser) -> ParseResult<Expression> {
    parse_expression_with_parser(parser)
}

/// Parse ORDER BY clause
fn parse_order_by_clause(parser: &mut SqlParser) -> ParseResult<Vec<OrderByItem>> {
    let mut items = Vec::new();

    loop {
        let expression = parse_expression_with_parser(parser)?;

        // Parse optional ASC/DESC
        let direction = match &parser.current_token {
            Some(Token::Asc) => {
                parser.advance()?;
                Some(SortDirection::Ascending)
            }
            Some(Token::Desc) => {
                parser.advance()?;
                Some(SortDirection::Descending)
            }
            Some(Token::Identifier(dir)) => match dir.to_uppercase().as_str() {
                "ASC" => {
                    parser.advance()?;
                    Some(SortDirection::Ascending)
                }
                "DESC" => {
                    parser.advance()?;
                    Some(SortDirection::Descending)
                }
                _ => None,
            },
            _ => None,
        };

        // Parse optional NULLS FIRST/LAST.
        //
        // The lexer emits `NULLS`, `FIRST` and `LAST` as keyword tokens, so
        // matching them as identifiers — which this did — never fired, and
        // `ORDER BY x NULLS FIRST` failed to parse at all.
        let is_nulls = matches!(&parser.current_token, Some(Token::Nulls))
            || matches!(&parser.current_token, Some(Token::Identifier(word))
                if word.eq_ignore_ascii_case("NULLS"));
        let nulls = if is_nulls {
            parser.advance()?;
            let order = match &parser.current_token {
                Some(Token::First) => Some(NullsOrder::First),
                Some(Token::Last) => Some(NullsOrder::Last),
                Some(Token::Identifier(word)) if word.eq_ignore_ascii_case("FIRST") => {
                    Some(NullsOrder::First)
                }
                Some(Token::Identifier(word)) if word.eq_ignore_ascii_case("LAST") => {
                    Some(NullsOrder::Last)
                }
                _ => None,
            };
            if order.is_some() {
                parser.advance()?;
            }
            order
        } else {
            None
        };

        items.push(OrderByItem {
            expression,
            direction,
            nulls,
        });

        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }

    Ok(items)
}

/// Parse INSERT statement with support for batch inserts and subqueries
pub fn parse_insert(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Insert)?;
    parser.expect(Token::Into)?;

    // Parse table name
    let table = utilities::parse_table_name(parser)?;

    // Parse optional column list
    let columns = if parser.matches(&[Token::LeftParen]) {
        parser.advance()?;
        let mut cols = Vec::new();

        while !parser.matches(&[Token::RightParen]) {
            if let Some(col_name) = parser
                .current_token
                .as_ref()
                .and_then(utilities::token_to_identifier_name)
            {
                cols.push(col_name);
                parser.advance()?;

                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            } else {
                return Err(ParseError {
                    message: "Expected column name in INSERT column list".to_string(),
                    position: parser.position,
                    expected: vec!["column name".to_string()],
                    found: parser.current_token.clone(),
                });
            }
        }

        parser.expect(Token::RightParen)?;
        Some(cols)
    } else {
        None
    };

    // Parse VALUES clause, SELECT statement, or DEFAULT VALUES
    let source = if parser.matches(&[Token::Values]) {
        parser.advance()?;
        let mut value_lists = Vec::new();

        loop {
            parser.expect(Token::LeftParen)?;
            let mut values = Vec::new();

            while !parser.matches(&[Token::RightParen]) {
                let expr = utilities::parse_expression(parser)?;
                values.push(expr);

                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }

            parser.expect(Token::RightParen)?;
            value_lists.push(values);

            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }

        InsertSource::Values(value_lists)
    } else if parser.matches(&[Token::Select]) {
        // INSERT ... SELECT
        let select_stmt = parse_select(parser)?;
        if let Statement::Select(select) = select_stmt {
            InsertSource::Query(select)
        } else {
            return Err(ParseError {
                message: "Expected SELECT statement after INSERT INTO table".to_string(),
                position: parser.position,
                expected: vec!["SELECT".to_string()],
                found: parser.current_token.clone(),
            });
        }
    } else if parser.matches(&[Token::Default]) {
        parser.advance()?;
        parser.expect(Token::Values)?;
        InsertSource::DefaultValues
    } else {
        return Err(ParseError {
            message: "Expected VALUES, SELECT, or DEFAULT VALUES in INSERT statement".to_string(),
            position: parser.position,
            expected: vec![
                "VALUES".to_string(),
                "SELECT".to_string(),
                "DEFAULT VALUES".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional ON CONFLICT clause (PostgreSQL extension)
    let on_conflict = if parser.matches(&[Token::On]) {
        parser.advance()?;
        if let Some(Token::Identifier(conflict_kw)) = &parser.current_token {
            if conflict_kw.to_uppercase() == "CONFLICT" {
                parser.advance()?;
                Some(parse_on_conflict_clause(parser)?)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Parse optional RETURNING clause
    let returning = if parser.matches(&[Token::Returning]) {
        parser.advance()?;
        Some(parse_returning_clause(parser)?)
    } else {
        None
    };

    Ok(Statement::Insert(InsertStatement {
        table,
        columns,
        source,
        on_conflict,
        returning,
    }))
}

/// Parse ON CONFLICT clause
fn parse_on_conflict_clause(parser: &mut SqlParser) -> ParseResult<OnConflictClause> {
    // Parse optional conflict target
    let target = if parser.matches(&[Token::LeftParen]) {
        parser.advance()?;
        let mut columns = Vec::new();

        while !parser.matches(&[Token::RightParen]) {
            if let Some(Token::Identifier(col_name)) = &parser.current_token {
                columns.push(col_name.clone());
                parser.advance()?;

                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }
        }

        parser.expect(Token::RightParen)?;
        Some(ConflictTarget::Columns(columns))
    } else if parser.matches(&[Token::On]) {
        parser.advance()?;
        if let Some(Token::Identifier(constraint_kw)) = &parser.current_token {
            if constraint_kw.to_uppercase() == "CONSTRAINT" {
                parser.advance()?;
                if let Some(Token::Identifier(constraint_name)) = &parser.current_token {
                    let name = constraint_name.clone();
                    parser.advance()?;
                    Some(ConflictTarget::Constraint(name))
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

    // Parse conflict action
    let action = if parser.matches(&[Token::Do]) {
        parser.advance()?;
        // Check for NOTHING or UPDATE (can be either keywords or identifiers)
        if parser.matches(&[Token::Nothing]) {
            parser.advance()?;
            ConflictAction::DoNothing
        } else if parser.matches(&[Token::Update]) {
            parser.advance()?;
            parser.expect(Token::Set)?;

            let mut set_clauses = Vec::new();
            loop {
                // Get column name (may be identifier or keyword used as name)
                let column = if let Some(col_name) = parser
                    .current_token
                    .as_ref()
                    .and_then(utilities::token_to_identifier_name)
                {
                    col_name
                } else {
                    break;
                };
                parser.advance()?;
                parser.expect(Token::Equal)?;
                let value = utilities::parse_expression(parser)?;

                set_clauses.push(Assignment {
                    target: AssignmentTarget::Column(column),
                    value,
                });

                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }

            let where_clause = if parser.matches(&[Token::Where]) {
                parser.advance()?;
                Some(parse_where_expression(parser)?)
            } else {
                None
            };

            ConflictAction::DoUpdate {
                set: set_clauses,
                where_clause,
            }
        } else if let Some(Token::Identifier(action_kw)) = &parser.current_token {
            // Fallback for identifiers (in case NOTHING/UPDATE aren't keywords in some contexts)
            match action_kw.to_uppercase().as_str() {
                "NOTHING" => {
                    parser.advance()?;
                    ConflictAction::DoNothing
                }
                "UPDATE" => {
                    parser.advance()?;
                    parser.expect(Token::Set)?;

                    let mut set_clauses = Vec::new();
                    loop {
                        let column = if let Some(col_name) = parser
                            .current_token
                            .as_ref()
                            .and_then(utilities::token_to_identifier_name)
                        {
                            col_name
                        } else {
                            break;
                        };
                        parser.advance()?;
                        parser.expect(Token::Equal)?;
                        let value = utilities::parse_expression(parser)?;

                        set_clauses.push(Assignment {
                            target: AssignmentTarget::Column(column),
                            value,
                        });

                        if parser.matches(&[Token::Comma]) {
                            parser.advance()?;
                        } else {
                            break;
                        }
                    }

                    let where_clause = if parser.matches(&[Token::Where]) {
                        parser.advance()?;
                        Some(parse_where_expression(parser)?)
                    } else {
                        None
                    };

                    ConflictAction::DoUpdate {
                        set: set_clauses,
                        where_clause,
                    }
                }
                _ => ConflictAction::DoNothing,
            }
        } else {
            ConflictAction::DoNothing
        }
    } else {
        ConflictAction::DoNothing
    };

    Ok(OnConflictClause { target, action })
}

/// Parse RETURNING clause
fn parse_returning_clause(parser: &mut SqlParser) -> ParseResult<Vec<SelectItem>> {
    let mut items = Vec::new();

    loop {
        if parser.matches(&[Token::Multiply]) {
            parser.advance()?;
            items.push(SelectItem::Wildcard);
        } else if parser.matches(&[Token::Old, Token::New]) {
            // PostgreSQL 18 - OLD.* and NEW.* qualified wildcards
            let qualifier = match &parser.current_token {
                Some(Token::Old) => "OLD".to_string(),
                Some(Token::New) => "NEW".to_string(),
                _ => unreachable!(),
            };
            parser.advance()?;

            // Check for qualified wildcard (OLD.* or NEW.*)
            if parser.matches(&[Token::Dot]) {
                parser.advance()?;
                if parser.matches(&[Token::Multiply]) {
                    parser.advance()?;
                    items.push(SelectItem::QualifiedWildcard { qualifier });
                } else {
                    // OLD.column or NEW.column - parse as expression
                    // Need to rewind and re-parse as a complete expression
                    // Actually, we've already consumed OLD. and the column name is next
                    if let Some(col_name) = parser
                        .current_token
                        .as_ref()
                        .and_then(utilities::token_to_identifier_name)
                    {
                        parser.advance()?;
                        let expr = Expression::Column(ColumnRef {
                            table: Some(qualifier),
                            name: col_name,
                        });

                        let alias = if parser.matches(&[Token::As]) {
                            parser.advance()?;
                            if let Some(Token::Identifier(alias_name)) = &parser.current_token {
                                let alias = alias_name.clone();
                                parser.advance()?;
                                Some(alias)
                            } else {
                                None
                            }
                        } else if let Some(Token::Identifier(alias_name)) = &parser.current_token {
                            // Allow alias without AS keyword
                            let alias = alias_name.clone();
                            parser.advance()?;
                            Some(alias)
                        } else {
                            None
                        };

                        items.push(SelectItem::Expression { expr, alias });
                    } else {
                        return Err(ParseError {
                            message: "Expected column name or * after OLD./NEW.".to_string(),
                            position: parser.position,
                            expected: vec!["column name".to_string(), "*".to_string()],
                            found: parser.current_token.clone(),
                        });
                    }
                }
            } else {
                // Just OLD or NEW without dot - treat as column reference
                let expr = Expression::Column(ColumnRef {
                    table: None,
                    name: qualifier,
                });
                items.push(SelectItem::Expression { expr, alias: None });
            }
        } else {
            let expr = utilities::parse_expression(parser)?;

            let alias = if parser.matches(&[Token::As]) {
                parser.advance()?;
                if let Some(Token::Identifier(alias_name)) = &parser.current_token {
                    let alias = alias_name.clone();
                    parser.advance()?;
                    Some(alias)
                } else {
                    None
                }
            } else {
                None
            };

            items.push(SelectItem::Expression { expr, alias });
        }

        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }

    Ok(items)
}

/// Parse UPDATE statement with JOIN support
pub fn parse_update(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Update)?;

    // Parse table name
    let table = utilities::parse_table_name(parser)?;

    // Parse optional alias
    let alias = if let Some(Token::Identifier(alias_name)) = &parser.current_token {
        if !parser.matches(&[Token::Set]) {
            let alias = alias_name.clone();
            parser.advance()?;
            Some(alias)
        } else {
            None
        }
    } else {
        None
    };

    // Parse SET clause
    parser.expect(Token::Set)?;
    let mut set_clauses = Vec::new();

    loop {
        // Parse assignment target (column or list of columns)
        if parser.matches(&[Token::LeftParen]) {
            // Multi-column assignment: (col1, col2) = (val1, val2)
            parser.advance()?;
            let mut columns = Vec::new();

            while !parser.matches(&[Token::RightParen]) {
                if let Some(Token::Identifier(col_name)) = &parser.current_token {
                    columns.push(col_name.clone());
                    parser.advance()?;

                    if parser.matches(&[Token::Comma]) {
                        parser.advance()?;
                    } else {
                        break;
                    }
                }
            }

            parser.expect(Token::RightParen)?;
            parser.expect(Token::Equal)?;

            let value = utilities::parse_expression(parser)?;

            set_clauses.push(Assignment {
                target: AssignmentTarget::Columns(columns),
                value,
            });
        } else if let Some(Token::Identifier(col_name)) = &parser.current_token {
            // Single column assignment
            let column = col_name.clone();
            parser.advance()?;

            parser.expect(Token::Equal)?;
            let value = utilities::parse_expression(parser)?;

            set_clauses.push(Assignment {
                target: AssignmentTarget::Column(column),
                value,
            });
        } else {
            return Err(ParseError {
                message: "Expected column name or column list in SET clause".to_string(),
                position: parser.position,
                expected: vec!["column name".to_string(), "(column_list)".to_string()],
                found: parser.current_token.clone(),
            });
        }

        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }

    // Parse optional FROM clause (PostgreSQL extension for JOINs in UPDATE)
    let from = if parser.matches(&[Token::From]) {
        parser.advance()?;
        let mut from_items = Vec::new();

        loop {
            from_items.push(parse_from_clause(parser)?);

            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }

        Some(from_items)
    } else {
        None
    };

    // Parse optional WHERE clause
    let where_clause = if parser.matches(&[Token::Where]) {
        parser.advance()?;
        Some(parse_where_expression(parser)?)
    } else {
        None
    };

    // Parse optional RETURNING clause
    let returning = if parser.matches(&[Token::Returning]) {
        parser.advance()?;
        Some(parse_returning_clause(parser)?)
    } else {
        None
    };

    Ok(Statement::Update(UpdateStatement {
        table,
        alias,
        set: set_clauses,
        from,
        where_clause,
        returning,
    }))
}

/// Parse DELETE statement with USING clause support
pub fn parse_delete(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Delete)?;
    parser.expect(Token::From)?;

    // Parse table name
    let table = utilities::parse_table_name(parser)?;

    // Parse optional alias
    let alias = if let Some(Token::Identifier(alias_name)) = &parser.current_token {
        if !parser.matches(&[Token::Using, Token::Where]) {
            let alias = alias_name.clone();
            parser.advance()?;
            Some(alias)
        } else {
            None
        }
    } else {
        None
    };

    // Parse optional USING clause (PostgreSQL extension)
    let using = if parser.matches(&[Token::Using]) {
        parser.advance()?;
        let mut using_items = Vec::new();

        loop {
            using_items.push(parse_from_clause(parser)?);

            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }

        Some(using_items)
    } else {
        None
    };

    // Parse optional WHERE clause
    let where_clause = if parser.matches(&[Token::Where]) {
        parser.advance()?;
        Some(parse_where_expression(parser)?)
    } else {
        None
    };

    // Parse optional RETURNING clause
    let returning = if parser.matches(&[Token::Returning]) {
        parser.advance()?;
        Some(parse_returning_clause(parser)?)
    } else {
        None
    };

    Ok(Statement::Delete(DeleteStatement {
        table,
        alias,
        using,
        where_clause,
        returning,
    }))
}

/// Parse MERGE statement
pub fn parse_merge(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Merge)?;
    parser.expect(Token::Into)?;

    let table = utilities::parse_table_name(parser)?;

    // Alias
    let alias = if parser.matches(&[Token::As]) {
        parser.advance()?;
        if let Some(Token::Identifier(a)) = &parser.current_token {
            let alias = a.clone();
            parser.advance()?;
            Some(alias)
        } else {
            None
        }
    } else if let Some(Token::Identifier(a)) = &parser.current_token {
        // Check if it's USING
        if a.to_uppercase() == "USING" {
            None
        } else {
            let alias = a.clone();
            parser.advance()?;
            Some(alias)
        }
    } else {
        None
    };

    parser.expect(Token::Using)?;

    // Source: Table or Subquery
    // We use parse_from_clause which handles simple tables and JSON_TABLE.
    // For subqueries, parse_from_clause currently doesn't support them fully (it returns FromClause::Table or JsonTable).
    // But let's use it for now. If the test uses a table as source, it works.
    let source = parse_from_clause(parser)?;

    parser.expect(Token::On)?;
    let on = parse_expression_with_parser(parser)?;

    let mut when_clauses = Vec::new();
    while parser.matches(&[Token::When]) {
        parser.advance()?;
        let matched = if parser.matches(&[Token::Not]) {
            parser.advance()?;
            parser.expect(Token::Matched)?;
            false
        } else {
            parser.expect(Token::Matched)?;
            true
        };

        let condition = if parser.matches(&[Token::And]) {
            parser.advance()?;
            Some(parse_expression_with_parser(parser)?)
        } else {
            None
        };

        parser.expect(Token::Then)?;

        let action = if parser.matches(&[Token::Update]) {
            parser.advance()?;
            parser.expect(Token::Set)?;
            // Parse assignments
            let mut assignments = Vec::new();
            loop {
                let target = if let Some(Token::Identifier(col)) = &parser.current_token {
                    AssignmentTarget::Column(col.clone())
                } else {
                    return Err(ParseError {
                        message: "Expected column name in SET clause".to_string(),
                        position: parser.position,
                        expected: vec!["column name".to_string()],
                        found: parser.current_token.clone(),
                    });
                };
                parser.advance()?;
                parser.expect(Token::Equal)?;
                let value = utilities::parse_expression(parser)?;
                assignments.push(Assignment { target, value });
                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }
            MergeAction::Update(MergeUpdate { assignments })
        } else if parser.matches(&[Token::Delete]) {
            parser.advance()?;
            MergeAction::Delete
        } else if parser.matches(&[Token::Insert]) {
            parser.advance()?;
            // Parse columns
            let columns = if parser.matches(&[Token::LeftParen]) {
                parser.advance()?;
                let mut cols = Vec::new();
                while !parser.matches(&[Token::RightParen]) {
                    if let Some(Token::Identifier(c)) = &parser.current_token {
                        cols.push(c.clone());
                        parser.advance()?;
                        if parser.matches(&[Token::Comma]) {
                            parser.advance()?;
                        } else {
                            break;
                        }
                    } else {
                        return Err(ParseError {
                            message: "Expected column name".to_string(),
                            position: parser.position,
                            expected: vec!["column name".to_string()],
                            found: parser.current_token.clone(),
                        });
                    }
                }
                parser.expect(Token::RightParen)?;
                Some(cols)
            } else {
                None
            };

            parser.expect(Token::Values)?;
            parser.expect(Token::LeftParen)?;
            let mut values = Vec::new();
            while !parser.matches(&[Token::RightParen]) {
                values.push(utilities::parse_expression(parser)?);
                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }
            parser.expect(Token::RightParen)?;
            MergeAction::Insert(MergeInsert {
                columns,
                values: MergeInsertValues::Values(values),
            })
        } else if parser.matches(&[Token::Do]) {
            parser.advance()?;
            parser.expect(Token::Nothing)?;
            MergeAction::DoNothing
        } else {
            return Err(ParseError {
                message: "Expected UPDATE, DELETE, INSERT or DO NOTHING".to_string(),
                position: parser.position,
                expected: vec![
                    "UPDATE".to_string(),
                    "DELETE".to_string(),
                    "INSERT".to_string(),
                    "DO NOTHING".to_string(),
                ],
                found: parser.current_token.clone(),
            });
        };

        when_clauses.push(MergeWhenClause {
            matched,
            condition,
            action,
        });
    }

    let returning = if parser.matches(&[Token::Returning]) {
        parser.advance()?;
        Some(parse_returning_clause(parser)?)
    } else {
        None
    };

    Ok(Statement::Merge(MergeStatement {
        table,
        alias,
        source,
        on,
        when_clauses,
        returning,
    }))
}

/// Parse COPY statement
/// COPY table_name [(column_list)] FROM/TO { 'filename' | STDIN | STDOUT | PROGRAM 'command' }
/// [ WITH ] [ ( option [, ...] ) ]
pub fn parse_copy(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Copy)?;

    // Parse target (table name or query in parentheses)
    let target = if parser.matches(&[Token::LeftParen]) {
        // COPY (query) TO ...
        parser.advance()?;
        let stmt = parse_select(parser)?;
        let select_stmt = if let Statement::Select(s) = stmt {
            s
        } else {
            return Err(ParseError {
                message: "Expected SELECT statement in COPY".to_string(),
                position: parser.position,
                expected: vec!["SELECT".to_string()],
                found: parser.current_token.clone(),
            });
        };
        parser.expect(Token::RightParen)?;
        CopyTarget::Query(select_stmt)
    } else {
        // COPY table_name ...
        let table = utilities::parse_table_name(parser)?;
        CopyTarget::Table(table)
    };

    // Parse optional column list
    let columns = if parser.matches(&[Token::LeftParen]) {
        parser.advance()?;
        let mut cols = Vec::new();
        loop {
            if let Some(Token::Identifier(name)) = &parser.current_token {
                cols.push(name.clone());
                parser.advance()?;
            } else {
                break;
            }
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
        Some(cols)
    } else {
        None
    };

    // Parse direction (FROM or TO)
    let direction = if parser.matches(&[Token::From]) {
        parser.advance()?;
        CopyDirection::From
    } else if parser.matches(&[Token::To]) {
        parser.advance()?;
        CopyDirection::To
    } else {
        return Err(ParseError {
            message: "Expected FROM or TO in COPY statement".to_string(),
            position: parser.position,
            expected: vec!["FROM".to_string(), "TO".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse source/destination
    let source = if parser.matches(&[Token::Stdin]) {
        parser.advance()?;
        CopySource::Stdio
    } else if parser.matches(&[Token::Stdout]) {
        parser.advance()?;
        CopySource::Stdio
    } else if parser.matches(&[Token::Program]) {
        parser.advance()?;
        if let Some(Token::StringLiteral(cmd)) = &parser.current_token {
            let program = cmd.clone();
            parser.advance()?;
            CopySource::Program(program)
        } else {
            return Err(ParseError {
                message: "Expected string literal for PROGRAM".to_string(),
                position: parser.position,
                expected: vec!["string literal".to_string()],
                found: parser.current_token.clone(),
            });
        }
    } else if let Some(Token::StringLiteral(path)) = &parser.current_token {
        let file_path = path.clone();
        parser.advance()?;
        CopySource::File(file_path)
    } else {
        return Err(ParseError {
            message: "Expected STDIN, STDOUT, PROGRAM, or file path".to_string(),
            position: parser.position,
            expected: vec![
                "STDIN".to_string(),
                "STDOUT".to_string(),
                "PROGRAM".to_string(),
                "file path".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional WITH clause
    if parser.matches(&[Token::With]) {
        parser.advance()?;
    }

    // Parse options in parentheses
    let mut options = Vec::new();
    if parser.matches(&[Token::LeftParen]) {
        parser.advance()?;
        loop {
            if parser.matches(&[Token::RightParen]) {
                break;
            }

            let option = parse_copy_option(parser)?;
            options.push(option);

            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
    }

    Ok(Statement::Copy(CopyStatement {
        direction,
        target,
        columns,
        source,
        options,
    }))
}

/// Parse a single COPY option
fn parse_copy_option(parser: &mut SqlParser) -> ParseResult<CopyOption> {
    match &parser.current_token {
        Some(Token::Format) => {
            parser.advance()?;
            let format = if parser.matches(&[Token::Csv]) {
                parser.advance()?;
                CopyFormat::Csv
            } else if parser.matches(&[Token::Binary]) {
                parser.advance()?;
                CopyFormat::Binary
            } else if let Some(Token::Identifier(name)) = &parser.current_token {
                let f = match name.to_uppercase().as_str() {
                    "TEXT" => CopyFormat::Text,
                    "CSV" => CopyFormat::Csv,
                    "BINARY" => CopyFormat::Binary,
                    _ => CopyFormat::Text,
                };
                parser.advance()?;
                f
            } else {
                CopyFormat::Text
            };
            Ok(CopyOption::Format(format))
        }
        Some(Token::Freeze) => {
            parser.advance()?;
            let value = parse_boolean_option(parser)?;
            Ok(CopyOption::Freeze(value))
        }
        Some(Token::Delimiter) => {
            parser.advance()?;
            if let Some(Token::StringLiteral(s)) = &parser.current_token {
                let delim = s.chars().next().unwrap_or(',');
                parser.advance()?;
                Ok(CopyOption::Delimiter(delim))
            } else {
                Ok(CopyOption::Delimiter(','))
            }
        }
        Some(Token::Null) => {
            parser.advance()?;
            if let Some(Token::StringLiteral(s)) = &parser.current_token {
                let null_str = s.clone();
                parser.advance()?;
                Ok(CopyOption::Null(null_str))
            } else {
                Ok(CopyOption::Null("\\N".to_string()))
            }
        }
        Some(Token::Header) => {
            parser.advance()?;
            let header = if parser.matches(&[Token::Match]) {
                parser.advance()?;
                CopyHeaderOption::Match
            } else {
                let value = parse_boolean_option(parser)?;
                if value {
                    CopyHeaderOption::On
                } else {
                    CopyHeaderOption::Off
                }
            };
            Ok(CopyOption::Header(header))
        }
        Some(Token::Quote) => {
            parser.advance()?;
            if let Some(Token::StringLiteral(s)) = &parser.current_token {
                let quote = s.chars().next().unwrap_or('"');
                parser.advance()?;
                Ok(CopyOption::Quote(quote))
            } else {
                Ok(CopyOption::Quote('"'))
            }
        }
        Some(Token::Escape) => {
            parser.advance()?;
            if let Some(Token::StringLiteral(s)) = &parser.current_token {
                let escape = s.chars().next().unwrap_or('"');
                parser.advance()?;
                Ok(CopyOption::Escape(escape))
            } else {
                Ok(CopyOption::Escape('"'))
            }
        }
        Some(Token::Encoding) => {
            parser.advance()?;
            if let Some(Token::StringLiteral(s)) = &parser.current_token {
                let encoding = s.clone();
                parser.advance()?;
                Ok(CopyOption::Encoding(encoding))
            } else if let Some(Token::Identifier(s)) = &parser.current_token {
                let encoding = s.clone();
                parser.advance()?;
                Ok(CopyOption::Encoding(encoding))
            } else {
                Ok(CopyOption::Encoding("UTF8".to_string()))
            }
        }
        Some(Token::OnError) => {
            parser.advance()?;
            let on_error = if parser.matches(&[Token::Ignore]) {
                parser.advance()?;
                CopyOnError::Ignore
            } else {
                CopyOnError::Stop
            };
            Ok(CopyOption::OnError(on_error))
        }
        Some(Token::Identifier(name)) => {
            let opt_name = name.to_uppercase();
            parser.advance()?;
            match opt_name.as_str() {
                "FORCE_QUOTE" => {
                    let cols = parse_column_list_option(parser)?;
                    Ok(CopyOption::ForceQuote(cols))
                }
                "FORCE_NOT_NULL" => {
                    let cols = parse_column_list_option(parser)?;
                    Ok(CopyOption::ForceNotNull(cols))
                }
                "FORCE_NULL" => {
                    let cols = parse_column_list_option(parser)?;
                    Ok(CopyOption::ForceNull(cols))
                }
                "DEFAULT" => {
                    if let Some(Token::StringLiteral(s)) = &parser.current_token {
                        let default_val = s.clone();
                        parser.advance()?;
                        Ok(CopyOption::Default(default_val))
                    } else {
                        Ok(CopyOption::Default(String::new()))
                    }
                }
                _ => Ok(CopyOption::Format(CopyFormat::Text)), // Default fallback
            }
        }
        _ => Ok(CopyOption::Format(CopyFormat::Text)),
    }
}

/// Parse a boolean option value (true, false, on, off, 1, 0)
fn parse_boolean_option(parser: &mut SqlParser) -> ParseResult<bool> {
    match &parser.current_token {
        Some(Token::BooleanLiteral(b)) => {
            let value = *b;
            parser.advance()?;
            Ok(value)
        }
        Some(Token::Identifier(name)) => {
            let value = matches!(name.to_uppercase().as_str(), "TRUE" | "ON" | "YES" | "1");
            parser.advance()?;
            Ok(value)
        }
        Some(Token::NumericLiteral(n)) => {
            let value = n != "0";
            parser.advance()?;
            Ok(value)
        }
        _ => Ok(true), // Default to true if no value specified
    }
}

/// Parse a column list option like (col1, col2, ...)
fn parse_column_list_option(parser: &mut SqlParser) -> ParseResult<Vec<String>> {
    if parser.matches(&[Token::LeftParen]) {
        parser.advance()?;
        let mut cols = Vec::new();
        loop {
            if let Some(Token::Identifier(name)) = &parser.current_token {
                cols.push(name.clone());
                parser.advance()?;
            } else {
                break;
            }
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
        Ok(cols)
    } else if parser.matches(&[Token::Multiply]) {
        parser.advance()?;
        Ok(vec!["*".to_string()])
    } else if let Some(Token::Identifier(name)) = &parser.current_token {
        let col = name.clone();
        parser.advance()?;
        Ok(vec![col])
    } else {
        Ok(vec![])
    }
}
