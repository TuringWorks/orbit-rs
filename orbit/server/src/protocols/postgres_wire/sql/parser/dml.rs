//! DML (Data Manipulation Language) Parser Implementation
//!
//! This module handles parsing of SELECT, INSERT, UPDATE, DELETE statements

use super::expressions::ExpressionParser;
use super::{utilities, ParseError, ParseResult, SqlParser};
use crate::protocols::postgres_wire::sql::{
    ast::{
        Assignment, AssignmentTarget, ColumnRef, ConflictAction, ConflictTarget, DeleteStatement,
        DistinctClause, Expression, FromClause, InsertSource, InsertStatement, LimitClause,
        NullsOrder, OnConflictClause, OrderByItem, SelectItem, SelectStatement, SortDirection,
        Statement, TableAlias, TraverseClause, TraverseDirection, UpdateStatement,
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
    parser.expect(Token::Select)?;

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
                    matches!(parser.tokens.get(parser.position + 2), Some(Token::Multiply))
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
                    if let Some(Token::Identifier(alias_name)) = &parser.current_token {
                        let alias = alias_name.clone();
                        parser.advance()?;
                        Some(alias)
                    } else {
                        None
                    }
                } else if let Some(Token::Identifier(alias_name)) = &parser.current_token {
                    // Check if this might be an alias (not a reserved word)
                    // Don't consume if next token is FROM, WHERE, GROUP BY, ORDER BY, or LIMIT
                    if !parser.matches(&[
                        Token::From,
                        Token::Where,
                        Token::Group,
                        Token::Order,
                        Token::Limit,
                        Token::Comma, // Also don't treat as alias if followed by comma
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

    // Create SELECT statement with ORDER BY, LIMIT, and TRAVERSE
    Ok(Statement::Select(Box::new(SelectStatement {
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
        traverse,
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
            _ => return Err(ParseError {
                message: format!("Expected OUTBOUND, INBOUND, or ANY, found {}", name),
                position: parser.position,
                expected: vec!["OUTBOUND".to_string(), "INBOUND".to_string(), "ANY".to_string()],
                found: parser.current_token.clone(),
            }),
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

/// Parse FROM clause
fn parse_from_clause(parser: &mut SqlParser) -> ParseResult<FromClause> {
    // For now, just parse simple table references
    let table_name = utilities::parse_table_name(parser)?;

    // Check for table alias
    let alias = if parser.matches(&[Token::As]) {
        parser.advance()?;
        if let Some(Token::Identifier(alias_name)) = &parser.current_token {
            let alias = TableAlias {
                name: alias_name.clone(),
                columns: None,
            };
            parser.advance()?;
            Some(alias)
        } else {
            None
        }
    } else if let Some(Token::Identifier(alias_name)) = &parser.current_token {
        // Check if this might be an alias (not a reserved word)
        if !parser.matches(&[
            Token::Where,
            Token::Group,
            Token::Order,
            Token::Limit,
            Token::Join,
        ]) {
            // Check for TRAVERSE identifier (OrbitQL extension)
            if alias_name.to_uppercase() == "TRAVERSE" {
                None
            } else {
                let alias = TableAlias {
                    name: alias_name.clone(),
                    columns: None,
                };
                parser.advance()?;
                Some(alias)
            }
        } else {
            None
        }
    } else {
        None
    };

    Ok(FromClause::Table {
        name: table_name,
        alias,
    })
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
            Some(Token::Identifier(dir)) => {
                match dir.to_uppercase().as_str() {
                    "ASC" => {
                        parser.advance()?;
                        Some(SortDirection::Ascending)
                    }
                    "DESC" => {
                        parser.advance()?;
                        Some(SortDirection::Descending)
                    }
                    _ => None,
                }
            }
            _ => None,
        };

        // Parse optional NULLS FIRST/LAST
        let nulls = if let Some(Token::Identifier(nulls_kw)) = &parser.current_token {
            if nulls_kw.to_uppercase() == "NULLS" {
                parser.advance()?;
                if let Some(Token::Identifier(order)) = &parser.current_token {
                    match order.to_uppercase().as_str() {
                        "FIRST" => {
                            parser.advance()?;
                            Some(NullsOrder::First)
                        }
                        "LAST" => {
                            parser.advance()?;
                            Some(NullsOrder::Last)
                        }
                        _ => None,
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
            if let Some(col_name) = parser.current_token.as_ref().and_then(utilities::token_to_identifier_name) {
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
    let returning = if parser.matches(&[Token::Identifier("RETURNING".to_string())]) {
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
        if let Some(Token::Identifier(action_kw)) = &parser.current_token {
            match action_kw.to_uppercase().as_str() {
                "NOTHING" => {
                    parser.advance()?;
                    ConflictAction::DoNothing
                }
                "UPDATE" => {
                    parser.advance()?;
                    parser.expect(Token::Set)?;

                    let mut set_clauses = Vec::new();
                    while let Some(Token::Identifier(col_name)) = &parser.current_token {
                        let column = col_name.clone();
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
        } else if let Some(Token::Identifier(name)) = &parser.current_token {
            let expr = Expression::Column(ColumnRef {
                table: None,
                name: name.clone(),
            });
            parser.advance()?;

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
        } else {
            let expr = utilities::parse_expression(parser)?;
            items.push(SelectItem::Expression { expr, alias: None });
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
    let returning = if parser.matches(&[Token::Identifier("RETURNING".to_string())]) {
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
    let returning = if parser.matches(&[Token::Identifier("RETURNING".to_string())]) {
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
