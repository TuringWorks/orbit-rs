//! TCL (Transaction Control Language) Parser Implementation
//! 
//! This module handles parsing of BEGIN, COMMIT, ROLLBACK, SAVEPOINT statements

use super::{SqlParser, ParseResult, ParseError};
use crate::postgres_wire::sql::ast::{
    Statement, BeginStatement, CommitStatement, RollbackStatement,
    SavepointStatement, ReleaseSavepointStatement,
    IsolationLevel, AccessMode
};
use crate::postgres_wire::sql::lexer::Token;

/// Parse BEGIN statement
/// BEGIN [WORK | TRANSACTION] [ISOLATION LEVEL level] [READ WRITE | READ ONLY]
pub fn parse_begin(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Begin)?;
    
    // Optional WORK or TRANSACTION
    if parser.matches(&[Token::Work, Token::Transaction]) {
        parser.advance()?;
    }
    
    let mut isolation_level = None;
    let mut access_mode = None;
    
    // Parse optional clauses
    while parser.matches(&[Token::Isolation, Token::Read]) {
        if parser.matches(&[Token::Isolation]) {
            parser.advance()?; // Skip ISOLATION
            parser.expect(Token::Level)?;
            
            isolation_level = Some(parse_isolation_level(parser)?);
        } else if parser.matches(&[Token::Read]) {
            access_mode = Some(parse_access_mode(parser)?);
        }
    }
    
    // Expect semicolon
    if parser.matches(&[Token::Semicolon]) {
        parser.advance()?;
    }
    
    Ok(Statement::Begin(BeginStatement {
        isolation_level,
        access_mode,
    }))
}

/// Parse COMMIT statement
/// COMMIT [WORK | TRANSACTION] [AND [NO] CHAIN]
pub fn parse_commit(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Commit)?;
    
    // Optional WORK or TRANSACTION
    if parser.matches(&[Token::Work, Token::Transaction]) {
        parser.advance()?;
    }
    
    // Check for AND [NO] CHAIN
    let mut chain = false;
    if parser.matches(&[Token::And]) {
        parser.advance()?;
        
        // Check for optional NO
        let no_chain = if parser.matches(&[Token::No]) {
            parser.advance()?;
            true
        } else {
            false
        };
        
        parser.expect(Token::Chain)?;
        chain = !no_chain;
    }
    
    // Expect semicolon
    if parser.matches(&[Token::Semicolon]) {
        parser.advance()?;
    }
    
    Ok(Statement::Commit(CommitStatement {
        chain,
    }))
}

/// Parse ROLLBACK statement
/// ROLLBACK [WORK | TRANSACTION] [TO [SAVEPOINT] savepoint_name] [AND [NO] CHAIN]
pub fn parse_rollback(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Rollback)?;
    
    // Optional WORK or TRANSACTION
    if parser.matches(&[Token::Work, Token::Transaction]) {
        parser.advance()?;
    }
    
    // Check for TO [SAVEPOINT] savepoint_name
    let mut to_savepoint = None;
    if parser.matches(&[Token::To]) {
        parser.advance()?;
        
        // Optional SAVEPOINT keyword
        if parser.matches(&[Token::Savepoint]) {
            parser.advance()?;
        }
        
        // Parse savepoint name
        if let Some(Token::Identifier(name)) = &parser.current_token {
            to_savepoint = Some(name.clone());
            parser.advance()?;
        } else {
            return Err(ParseError {
                message: "Expected savepoint name after TO".to_string(),
                position: parser.position,
                expected: vec!["identifier".to_string()],
                found: parser.current_token.clone(),
            });
        }
    }
    
    // Check for AND [NO] CHAIN
    let mut chain = false;
    if parser.matches(&[Token::And]) {
        parser.advance()?;
        
        // Check for optional NO
        let no_chain = if parser.matches(&[Token::No]) {
            parser.advance()?;
            true
        } else {
            false
        };
        
        parser.expect(Token::Chain)?;
        chain = !no_chain;
    }
    
    // Expect semicolon
    if parser.matches(&[Token::Semicolon]) {
        parser.advance()?;
    }
    
    Ok(Statement::Rollback(RollbackStatement {
        to_savepoint,
        chain,
    }))
}

/// Parse SAVEPOINT statement
/// SAVEPOINT savepoint_name
pub fn parse_savepoint(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Savepoint)?;
    
    // Parse savepoint name
    let name = match &parser.current_token {
        Some(Token::Identifier(name)) => {
            let name = name.clone();
            parser.advance()?;
            name
        },
        Some(token) => return Err(ParseError {
            message: format!("Expected savepoint name, found {:?}", token),
            position: parser.position,
            expected: vec!["identifier".to_string()],
            found: Some(token.clone()),
        }),
        None => return Err(ParseError {
            message: "Expected savepoint name, found EOF".to_string(),
            position: parser.position,
            expected: vec!["identifier".to_string()],
            found: None,
        }),
    };
    
    // Expect semicolon
    if parser.matches(&[Token::Semicolon]) {
        parser.advance()?;
    }
    
    Ok(Statement::Savepoint(SavepointStatement {
        name,
    }))
}

/// Parse RELEASE SAVEPOINT statement
/// RELEASE [SAVEPOINT] savepoint_name
pub fn parse_release_savepoint(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Release)?;
    
    // Optional SAVEPOINT keyword
    if parser.matches(&[Token::Savepoint]) {
        parser.advance()?;
    }
    
    // Parse savepoint name
    let name = match &parser.current_token {
        Some(Token::Identifier(name)) => {
            let name = name.clone();
            parser.advance()?;
            name
        },
        Some(token) => return Err(ParseError {
            message: format!("Expected savepoint name, found {:?}", token),
            position: parser.position,
            expected: vec!["identifier".to_string()],
            found: Some(token.clone()),
        }),
        None => return Err(ParseError {
            message: "Expected savepoint name, found EOF".to_string(),
            position: parser.position,
            expected: vec!["identifier".to_string()],
            found: None,
        }),
    };
    
    // Expect semicolon
    if parser.matches(&[Token::Semicolon]) {
        parser.advance()?;
    }
    
    Ok(Statement::ReleaseSavepoint(ReleaseSavepointStatement {
        name,
    }))
}

/// Parse isolation level
fn parse_isolation_level(parser: &mut SqlParser) -> ParseResult<IsolationLevel> {
    match &parser.current_token {
        Some(Token::Read) => {
            parser.advance()?;
            match &parser.current_token {
                Some(Token::Uncommitted) => {
                    parser.advance()?;
                    Ok(IsolationLevel::ReadUncommitted)
                },
                Some(Token::Committed) => {
                    parser.advance()?;
                    Ok(IsolationLevel::ReadCommitted)
                },
                Some(token) => Err(ParseError {
                    message: format!("Expected UNCOMMITTED or COMMITTED after READ, found {:?}", token),
                    position: parser.position,
                    expected: vec!["UNCOMMITTED".to_string(), "COMMITTED".to_string()],
                    found: Some(token.clone()),
                }),
                None => Err(ParseError {
                    message: "Expected UNCOMMITTED or COMMITTED after READ, found EOF".to_string(),
                    position: parser.position,
                    expected: vec!["UNCOMMITTED or COMMITTED".to_string()],
                    found: None,
                }),
            }
        },
        Some(Token::Repeatable) => {
            parser.advance()?;
            parser.expect(Token::Read)?;
            Ok(IsolationLevel::RepeatableRead)
        },
        Some(Token::Serializable) => {
            parser.advance()?;
            Ok(IsolationLevel::Serializable)
        },
        Some(token) => Err(ParseError {
            message: format!("Expected isolation level, found {:?}", token),
            position: parser.position,
            expected: vec![
                "READ UNCOMMITTED".to_string(), "READ COMMITTED".to_string(),
                "REPEATABLE READ".to_string(), "SERIALIZABLE".to_string(),
            ],
            found: Some(token.clone()),
        }),
        None => Err(ParseError {
            message: "Expected isolation level, found EOF".to_string(),
            position: parser.position,
            expected: vec!["isolation level".to_string()],
            found: None,
        }),
    }
}

/// Parse access mode
fn parse_access_mode(parser: &mut SqlParser) -> ParseResult<AccessMode> {
    parser.expect(Token::Read)?;
    
    match &parser.current_token {
        Some(Token::Write) => {
            parser.advance()?;
            Ok(AccessMode::ReadWrite)
        },
        Some(Token::Only) => {
            parser.advance()?;
            Ok(AccessMode::ReadOnly)
        },
        Some(token) => Err(ParseError {
            message: format!("Expected WRITE or ONLY after READ, found {:?}", token),
            position: parser.position,
            expected: vec!["WRITE".to_string(), "ONLY".to_string()],
            found: Some(token.clone()),
        }),
        None => Err(ParseError {
            message: "Expected WRITE or ONLY after READ, found EOF".to_string(),
            position: parser.position,
            expected: vec!["WRITE or ONLY".to_string()],
            found: None,
        }),
    }
}
