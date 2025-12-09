//! Utility Commands Parser
//!
//! This module handles parsing of PostgreSQL utility commands:
//! - RESET / DISCARD
//! - DECLARE / FETCH / MOVE / CLOSE (Cursors)
//! - LISTEN / UNLISTEN / NOTIFY (Notifications)
//! - PREPARE / EXECUTE / DEALLOCATE (Prepared Statements)
//! - VACUUM / ANALYZE / REINDEX / CLUSTER / CHECKPOINT (Maintenance)
//! - CALL / DO (Procedural)

use super::{utilities, ParseResult, SqlParser};
use crate::protocols::postgres_wire::sql::{ast::*, lexer::Token};

// ===== RESET / DISCARD =====

pub fn parse_reset(parser: &mut SqlParser) -> ParseResult<Statement> {
    // RESET already consumed

    let target = if parser.matches(&[Token::All]) {
        parser.advance()?;
        ResetTarget::All
    } else if parser.matches(&[Token::Time]) {
        parser.advance()?;
        parser.expect(Token::Zone)?;
        ResetTarget::TimeZone
    } else if parser.matches(&[Token::Role]) {
        parser.advance()?;
        ResetTarget::Role
    } else if parser.matches(&[Token::Identifier(String::new())]) {
        if let Some(Token::Identifier(name)) = &parser.current_token {
            let name = name.clone();
            if name.to_uppercase() == "SESSION" {
                parser.advance()?;
                if parser.matches(&[Token::Authorization]) {
                    parser.advance()?;
                    ResetTarget::SessionAuthorization
                } else {
                    ResetTarget::Parameter("session".to_string())
                }
            } else {
                parser.advance()?;
                ResetTarget::Parameter(name)
            }
        } else {
            ResetTarget::All
        }
    } else {
        ResetTarget::All
    };

    Ok(Statement::Reset(ResetStatement { target }))
}

pub fn parse_discard(parser: &mut SqlParser) -> ParseResult<Statement> {
    // DISCARD already consumed

    let target = if parser.matches(&[Token::All]) {
        parser.advance()?;
        DiscardTarget::All
    } else if parser.matches(&[Token::Plans]) {
        parser.advance()?;
        DiscardTarget::Plans
    } else if parser.matches(&[Token::Sequence]) {
        parser.advance()?;
        DiscardTarget::Sequences
    } else if parser.matches(&[Token::Temporary]) {
        parser.advance()?;
        DiscardTarget::Temporary
    } else if parser.matches(&[Token::Temp]) {
        parser.advance()?;
        DiscardTarget::Temp
    } else {
        DiscardTarget::All
    };

    Ok(Statement::Discard(DiscardStatement { target }))
}

// ===== Cursor Commands =====

pub fn parse_declare_cursor(parser: &mut SqlParser) -> ParseResult<Statement> {
    // DECLARE already consumed

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(super::ParseError {
            message: "Expected cursor name".to_string(),
            position: parser.position,
            expected: vec!["identifier".to_string()],
            found: parser.current_token.clone(),
        });
    };

    let mut binary = false;
    let mut insensitive = false;
    let mut scroll: Option<bool> = None;

    // Parse optional keywords before CURSOR
    loop {
        if parser.matches(&[Token::Binary]) {
            parser.advance()?;
            binary = true;
        } else if parser.matches(&[Token::Insensitive]) {
            parser.advance()?;
            insensitive = true;
        } else if parser.matches(&[Token::Scroll]) {
            parser.advance()?;
            scroll = Some(true);
        } else if parser.matches(&[Token::No]) {
            parser.advance()?;
            if parser.matches(&[Token::Scroll]) {
                parser.advance()?;
                scroll = Some(false);
            }
        } else {
            break;
        }
    }

    parser.expect(Token::Cursor)?;

    let mut hold = false;
    if parser.matches(&[Token::With]) {
        parser.advance()?;
        if parser.matches(&[Token::Hold]) {
            parser.advance()?;
            hold = true;
        }
    } else if parser.matches(&[Token::Without]) {
        parser.advance()?;
        if parser.matches(&[Token::Hold]) {
            parser.advance()?;
            hold = false;
        }
    }

    parser.expect(Token::For)?;

    // Parse the SELECT query
    let query = parser.parse_select_statement()?;
    let select_stmt = if let Statement::Select(s) = query {
        s
    } else {
        return Err(super::ParseError {
            message: "Expected SELECT statement".to_string(),
            position: parser.position,
            expected: vec!["SELECT".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::DeclareCursor(DeclareCursorStatement {
        name,
        binary,
        insensitive,
        scroll,
        hold,
        query: select_stmt,
    }))
}

pub fn parse_fetch(parser: &mut SqlParser) -> ParseResult<Statement> {
    // FETCH already consumed

    let direction = parse_fetch_direction(parser)?;

    // FROM is optional
    if parser.matches(&[Token::From]) || parser.matches(&[Token::In]) {
        parser.advance()?;
    }

    let cursor_name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(super::ParseError {
            message: "Expected cursor name".to_string(),
            position: parser.position,
            expected: vec!["identifier".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::FetchCursor(FetchCursorStatement {
        direction,
        cursor_name,
    }))
}

fn parse_fetch_direction(parser: &mut SqlParser) -> ParseResult<FetchDirection> {
    if parser.matches(&[Token::Next]) {
        parser.advance()?;
        Ok(FetchDirection::Next)
    } else if parser.matches(&[Token::Prior]) {
        parser.advance()?;
        Ok(FetchDirection::Prior)
    } else if parser.matches(&[Token::First]) {
        parser.advance()?;
        Ok(FetchDirection::First)
    } else if parser.matches(&[Token::Last]) {
        parser.advance()?;
        Ok(FetchDirection::Last)
    } else if parser.matches(&[Token::All]) {
        parser.advance()?;
        Ok(FetchDirection::All)
    } else if parser.matches(&[Token::Absolute]) {
        parser.advance()?;
        let count = parse_integer(parser)?;
        Ok(FetchDirection::Absolute(count))
    } else if parser.matches(&[Token::Relative]) {
        parser.advance()?;
        let count = parse_integer(parser)?;
        Ok(FetchDirection::Relative(count))
    } else if parser.matches(&[Token::Forward]) {
        parser.advance()?;
        if parser.matches(&[Token::All]) {
            parser.advance()?;
            Ok(FetchDirection::ForwardAll)
        } else if let Some(Token::NumericLiteral(_)) = &parser.current_token {
            let count = parse_integer(parser)?;
            Ok(FetchDirection::ForwardCount(count))
        } else {
            Ok(FetchDirection::Forward)
        }
    } else if parser.matches(&[Token::Backward]) {
        parser.advance()?;
        if parser.matches(&[Token::All]) {
            parser.advance()?;
            Ok(FetchDirection::BackwardAll)
        } else if let Some(Token::NumericLiteral(_)) = &parser.current_token {
            let count = parse_integer(parser)?;
            Ok(FetchDirection::BackwardCount(count))
        } else {
            Ok(FetchDirection::Backward)
        }
    } else if let Some(Token::NumericLiteral(_)) = &parser.current_token {
        let count = parse_integer(parser)?;
        Ok(FetchDirection::Count(count))
    } else {
        // Default is NEXT
        Ok(FetchDirection::Next)
    }
}

fn parse_integer(parser: &mut SqlParser) -> ParseResult<i64> {
    if let Some(Token::NumericLiteral(n)) = &parser.current_token {
        let val = n.parse::<i64>().unwrap_or(0);
        parser.advance()?;
        Ok(val)
    } else {
        Err(super::ParseError {
            message: "Expected integer".to_string(),
            position: parser.position,
            expected: vec!["integer".to_string()],
            found: parser.current_token.clone(),
        })
    }
}

pub fn parse_move(parser: &mut SqlParser) -> ParseResult<Statement> {
    // MOVE already consumed

    let direction = parse_fetch_direction(parser)?;

    // FROM/IN is optional
    if parser.matches(&[Token::From]) || parser.matches(&[Token::In]) {
        parser.advance()?;
    }

    let cursor_name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(super::ParseError {
            message: "Expected cursor name".to_string(),
            position: parser.position,
            expected: vec!["identifier".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::MoveCursor(MoveCursorStatement {
        direction,
        cursor_name,
    }))
}

pub fn parse_close(parser: &mut SqlParser) -> ParseResult<Statement> {
    // CLOSE already consumed

    let cursor_name = if parser.matches(&[Token::All]) {
        parser.advance()?;
        CloseCursorTarget::All
    } else if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        CloseCursorTarget::Named(name)
    } else {
        return Err(super::ParseError {
            message: "Expected cursor name or ALL".to_string(),
            position: parser.position,
            expected: vec!["identifier".to_string(), "ALL".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::CloseCursor(CloseCursorStatement { cursor_name }))
}

// ===== Notification Commands =====

pub fn parse_listen(parser: &mut SqlParser) -> ParseResult<Statement> {
    // LISTEN already consumed

    let channel = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(super::ParseError {
            message: "Expected channel name".to_string(),
            position: parser.position,
            expected: vec!["identifier".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::Listen(ListenStatement { channel }))
}

pub fn parse_unlisten(parser: &mut SqlParser) -> ParseResult<Statement> {
    // UNLISTEN already consumed

    let channel = if parser.matches(&[Token::Multiply]) {
        parser.advance()?;
        UnlistenTarget::All
    } else if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        UnlistenTarget::Channel(name)
    } else {
        return Err(super::ParseError {
            message: "Expected channel name or *".to_string(),
            position: parser.position,
            expected: vec!["identifier".to_string(), "*".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::Unlisten(UnlistenStatement { channel }))
}

pub fn parse_notify(parser: &mut SqlParser) -> ParseResult<Statement> {
    // NOTIFY already consumed

    let channel = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(super::ParseError {
            message: "Expected channel name".to_string(),
            position: parser.position,
            expected: vec!["identifier".to_string()],
            found: parser.current_token.clone(),
        });
    };

    let payload = if parser.matches(&[Token::Comma]) {
        parser.advance()?;
        if let Some(Token::StringLiteral(s)) = &parser.current_token {
            let payload = s.clone();
            parser.advance()?;
            Some(payload)
        } else {
            None
        }
    } else {
        None
    };

    Ok(Statement::Notify(NotifyStatement { channel, payload }))
}

// ===== Prepared Statement Commands =====

pub fn parse_prepare(parser: &mut SqlParser) -> ParseResult<Statement> {
    // PREPARE already consumed

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(super::ParseError {
            message: "Expected prepared statement name".to_string(),
            position: parser.position,
            expected: vec!["identifier".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Optional data types
    let mut data_types = Vec::new();
    if parser.matches(&[Token::LeftParen]) {
        parser.advance()?;
        loop {
            let data_type = utilities::parse_data_type(parser)?;
            data_types.push(data_type);

            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
    }

    parser.expect(Token::As)?;

    // Parse the statement
    let statement = parser.parse_statement()?;

    Ok(Statement::Prepare(PrepareStatement {
        name,
        data_types,
        statement: Box::new(statement),
    }))
}

pub fn parse_execute(parser: &mut SqlParser) -> ParseResult<Statement> {
    // EXECUTE already consumed

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(super::ParseError {
            message: "Expected prepared statement name".to_string(),
            position: parser.position,
            expected: vec!["identifier".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Optional parameters
    let mut parameters = Vec::new();
    if parser.matches(&[Token::LeftParen]) {
        parser.advance()?;
        loop {
            let expr = utilities::parse_expression(parser)?;
            parameters.push(expr);

            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
    }

    Ok(Statement::Execute(ExecuteStatement { name, parameters }))
}

pub fn parse_deallocate(parser: &mut SqlParser) -> ParseResult<Statement> {
    // DEALLOCATE already consumed

    // PREPARE is optional
    if parser.matches(&[Token::Prepare]) {
        parser.advance()?;
    }

    let target = if parser.matches(&[Token::All]) {
        parser.advance()?;
        DeallocateTarget::All
    } else if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        DeallocateTarget::Named(name)
    } else {
        return Err(super::ParseError {
            message: "Expected prepared statement name or ALL".to_string(),
            position: parser.position,
            expected: vec!["identifier".to_string(), "ALL".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::Deallocate(DeallocateStatement { target }))
}

// ===== Maintenance Commands =====

pub fn parse_vacuum(parser: &mut SqlParser) -> ParseResult<Statement> {
    // VACUUM already consumed

    let mut full = false;
    let mut freeze = false;
    let mut verbose = false;
    let mut analyze = false;
    let mut disable_page_skipping = false;
    let mut skip_locked = false;
    let mut index_cleanup: Option<bool> = None;
    let mut truncate: Option<bool> = None;
    let mut parallel: Option<i32> = None;

    // Parse options in parentheses or as keywords
    if parser.matches(&[Token::LeftParen]) {
        parser.advance()?;
        loop {
            if parser.matches(&[Token::Full]) {
                parser.advance()?;
                full = true;
            } else if parser.matches(&[Token::Freeze]) {
                parser.advance()?;
                freeze = true;
            } else if parser.matches(&[Token::Verbose]) {
                parser.advance()?;
                verbose = true;
            } else if parser.matches(&[Token::Analyze]) {
                parser.advance()?;
                analyze = true;
            } else if parser.matches(&[Token::Skip]) {
                parser.advance()?;
                parser.expect(Token::Locked)?;
                skip_locked = true;
            } else if parser.matches(&[Token::IndexCleanup]) {
                parser.advance()?;
                index_cleanup = Some(true);
            } else if parser.matches(&[Token::Parallel]) {
                parser.advance()?;
                if let Some(Token::NumericLiteral(n)) = &parser.current_token {
                    parallel = n.parse().ok();
                    parser.advance()?;
                }
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
    } else {
        // Parse keywords without parentheses
        loop {
            if parser.matches(&[Token::Full]) {
                parser.advance()?;
                full = true;
            } else if parser.matches(&[Token::Freeze]) {
                parser.advance()?;
                freeze = true;
            } else if parser.matches(&[Token::Verbose]) {
                parser.advance()?;
                verbose = true;
            } else if parser.matches(&[Token::Analyze]) {
                parser.advance()?;
                analyze = true;
            } else {
                break;
            }
        }
    }

    // Parse table list
    let mut tables = Vec::new();
    if !parser.current_token.is_none() && !parser.matches(&[Token::Semicolon]) {
        loop {
            let name = utilities::parse_table_name(parser)?;
            let mut columns = Vec::new();

            if parser.matches(&[Token::LeftParen]) {
                parser.advance()?;
                loop {
                    if let Some(Token::Identifier(col)) = &parser.current_token {
                        columns.push(col.clone());
                        parser.advance()?;
                    }
                    if parser.matches(&[Token::Comma]) {
                        parser.advance()?;
                    } else {
                        break;
                    }
                }
                parser.expect(Token::RightParen)?;
            }

            tables.push(VacuumTable { name, columns });

            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
    }

    Ok(Statement::Vacuum(VacuumStatement {
        full,
        freeze,
        verbose,
        analyze,
        disable_page_skipping,
        skip_locked,
        index_cleanup,
        truncate,
        parallel,
        tables,
    }))
}

pub fn parse_analyze(parser: &mut SqlParser) -> ParseResult<Statement> {
    // ANALYZE already consumed

    let mut verbose = false;
    let mut skip_locked = false;

    // Parse options
    if parser.matches(&[Token::LeftParen]) {
        parser.advance()?;
        loop {
            if parser.matches(&[Token::Verbose]) {
                parser.advance()?;
                verbose = true;
            } else if parser.matches(&[Token::Skip]) {
                parser.advance()?;
                parser.expect(Token::Locked)?;
                skip_locked = true;
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
    } else if parser.matches(&[Token::Verbose]) {
        parser.advance()?;
        verbose = true;
    }

    // Parse table list
    let mut tables = Vec::new();
    if !parser.current_token.is_none() && !parser.matches(&[Token::Semicolon]) {
        loop {
            let name = utilities::parse_table_name(parser)?;
            let mut columns = Vec::new();

            if parser.matches(&[Token::LeftParen]) {
                parser.advance()?;
                loop {
                    if let Some(Token::Identifier(col)) = &parser.current_token {
                        columns.push(col.clone());
                        parser.advance()?;
                    }
                    if parser.matches(&[Token::Comma]) {
                        parser.advance()?;
                    } else {
                        break;
                    }
                }
                parser.expect(Token::RightParen)?;
            }

            tables.push(VacuumTable { name, columns });

            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
    }

    Ok(Statement::Analyze(AnalyzeStatement {
        verbose,
        skip_locked,
        tables,
    }))
}

pub fn parse_reindex(parser: &mut SqlParser) -> ParseResult<Statement> {
    // REINDEX already consumed

    let mut concurrently = false;
    let mut verbose = false;

    // Parse options
    if parser.matches(&[Token::LeftParen]) {
        parser.advance()?;
        loop {
            if parser.matches(&[Token::Concurrently]) {
                parser.advance()?;
                concurrently = true;
            } else if parser.matches(&[Token::Verbose]) {
                parser.advance()?;
                verbose = true;
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
    }

    // Check for CONCURRENTLY without parentheses
    if parser.matches(&[Token::Concurrently]) {
        parser.advance()?;
        concurrently = true;
    }

    // Parse target type
    let target_type = if parser.matches(&[Token::Index]) {
        parser.advance()?;
        ReindexTarget::Index
    } else if parser.matches(&[Token::Table]) {
        parser.advance()?;
        ReindexTarget::Table
    } else if parser.matches(&[Token::Schema]) {
        parser.advance()?;
        ReindexTarget::Schema
    } else if parser.matches(&[Token::Database]) {
        parser.advance()?;
        ReindexTarget::Database
    } else if parser.matches(&[Token::System]) {
        parser.advance()?;
        ReindexTarget::System
    } else {
        return Err(super::ParseError {
            message: "Expected INDEX, TABLE, SCHEMA, DATABASE, or SYSTEM".to_string(),
            position: parser.position,
            expected: vec![
                "INDEX".to_string(),
                "TABLE".to_string(),
                "SCHEMA".to_string(),
                "DATABASE".to_string(),
                "SYSTEM".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional name
    let name = if !parser.current_token.is_none() && !parser.matches(&[Token::Semicolon]) {
        Some(utilities::parse_table_name(parser)?)
    } else {
        None
    };

    Ok(Statement::Reindex(ReindexStatement {
        target_type,
        concurrently,
        verbose,
        name,
    }))
}

pub fn parse_cluster(parser: &mut SqlParser) -> ParseResult<Statement> {
    // CLUSTER already consumed

    let mut verbose = false;

    // Parse VERBOSE option
    if parser.matches(&[Token::Verbose]) {
        parser.advance()?;
        verbose = true;
    }

    // Parse optional table name
    let (table_name, index_name) =
        if !parser.current_token.is_none() && !parser.matches(&[Token::Semicolon]) {
            let name = utilities::parse_table_name(parser)?;

            let idx = if parser.matches(&[Token::Using]) {
                parser.advance()?;
                if let Some(Token::Identifier(i)) = &parser.current_token {
                    let idx_name = i.clone();
                    parser.advance()?;
                    Some(idx_name)
                } else {
                    None
                }
            } else {
                None
            };

            (Some(name), idx)
        } else {
            (None, None)
        };

    Ok(Statement::Cluster(ClusterStatement {
        verbose,
        table_name,
        index_name,
    }))
}

pub fn parse_checkpoint(_parser: &mut SqlParser) -> ParseResult<Statement> {
    // CHECKPOINT already consumed - nothing more to parse
    Ok(Statement::Checkpoint)
}

// ===== Procedural Commands =====

pub fn parse_call(parser: &mut SqlParser) -> ParseResult<Statement> {
    // CALL already consumed

    let procedure_name = utilities::parse_table_name(parser)?;

    parser.expect(Token::LeftParen)?;

    let mut arguments = Vec::new();
    if !parser.matches(&[Token::RightParen]) {
        loop {
            let expr = utilities::parse_expression(parser)?;
            arguments.push(expr);

            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
    }

    parser.expect(Token::RightParen)?;

    Ok(Statement::Call(CallStatement {
        procedure_name,
        arguments,
    }))
}

pub fn parse_do(parser: &mut SqlParser) -> ParseResult<Statement> {
    // DO already consumed

    let mut language = None;

    // Check for LANGUAGE before code
    if parser.matches(&[Token::Language]) {
        parser.advance()?;
        if let Some(Token::Identifier(lang)) = &parser.current_token {
            language = Some(lang.clone());
            parser.advance()?;
        }
    }

    // Parse the code block
    let code = if let Some(Token::StringLiteral(s)) = &parser.current_token {
        let code = s.clone();
        parser.advance()?;
        code
    } else if let Some(Token::DollarQuotedString(s)) = &parser.current_token {
        let code = s.clone();
        parser.advance()?;
        code
    } else {
        return Err(super::ParseError {
            message: "Expected code block".to_string(),
            position: parser.position,
            expected: vec!["string literal".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Check for LANGUAGE after code
    if parser.matches(&[Token::Language]) {
        parser.advance()?;
        if let Some(Token::Identifier(lang)) = &parser.current_token {
            language = Some(lang.clone());
            parser.advance()?;
        }
    }

    Ok(Statement::Do(DoStatement { language, code }))
}
