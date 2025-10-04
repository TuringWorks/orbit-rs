//! DCL (Data Control Language) Parser Implementation
//! 
//! This module handles parsing of GRANT, REVOKE statements

use super::{SqlParser, ParseResult, ParseError};
use crate::postgres_wire::sql::ast::{
    Statement, GrantStatement, RevokeStatement, Privilege, ObjectType
};
use crate::postgres_wire::sql::lexer::Token;

/// Parse GRANT statement
/// GRANT privilege_list ON object_type object_name TO grantee_list [WITH GRANT OPTION]
pub fn parse_grant(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Grant)?;
    
    // Parse privileges
    let privileges = parse_privilege_list(parser)?;
    
    // Expect ON
    parser.expect(Token::On)?;
    
    // Parse object type and name
    let (object_type, object_name) = parse_object_reference(parser)?;
    
    // Expect TO
    parser.expect(Token::To)?;
    
    // Parse grantees
    let grantees = parse_grantee_list(parser)?;
    
    // Check for WITH GRANT OPTION
    let mut with_grant_option = false;
    if parser.matches(&[Token::With]) {
        parser.advance()?;
        parser.expect(Token::Grant)?;
        parser.expect(Token::Option)?;
        with_grant_option = true;
    }
    
    // Expect semicolon
    if parser.matches(&[Token::Semicolon]) {
        parser.advance()?;
    }
    
    Ok(Statement::Grant(GrantStatement {
        privileges,
        object_type,
        object_name,
        grantees,
        with_grant_option,
    }))
}

/// Parse REVOKE statement
/// REVOKE [GRANT OPTION FOR] privilege_list ON object_type object_name FROM grantee_list [CASCADE | RESTRICT]
pub fn parse_revoke(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Revoke)?;
    
    // Check for GRANT OPTION FOR
    let mut grant_option_for = false;
    if parser.matches(&[Token::Grant]) {
        parser.advance()?;
        parser.expect(Token::Option)?;
        parser.expect(Token::For)?;
        grant_option_for = true;
    }
    
    // Parse privileges
    let privileges = parse_privilege_list(parser)?;
    
    // Expect ON
    parser.expect(Token::On)?;
    
    // Parse object type and name
    let (object_type, object_name) = parse_object_reference(parser)?;
    
    // Expect FROM
    parser.expect(Token::From)?;
    
    // Parse grantees
    let grantees = parse_grantee_list(parser)?;
    
    // Check for CASCADE or RESTRICT
    let mut cascade = false;
    if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        cascade = true;
    } else if parser.matches(&[Token::Restrict]) {
        parser.advance()?;
        cascade = false;
    }
    
    // Expect semicolon
    if parser.matches(&[Token::Semicolon]) {
        parser.advance()?;
    }
    
    Ok(Statement::Revoke(RevokeStatement {
        grant_option_for,
        privileges,
        object_type,
        object_name,
        grantees,
        cascade,
    }))
}

/// Parse privilege list (e.g., SELECT, INSERT, UPDATE or ALL)
fn parse_privilege_list(parser: &mut SqlParser) -> ParseResult<Vec<Privilege>> {
    let mut privileges = Vec::new();
    
    loop {
        let privilege = match &parser.current_token {
            Some(Token::All) => {
                parser.advance()?;
                Privilege::All
            },
            Some(Token::Select) => {
                parser.advance()?;
                Privilege::Select
            },
            Some(Token::Insert) => {
                parser.advance()?;
                Privilege::Insert
            },
            Some(Token::Update) => {
                parser.advance()?;
                Privilege::Update
            },
            Some(Token::Delete) => {
                parser.advance()?;
                Privilege::Delete
            },
            Some(Token::Create) => {
                parser.advance()?;
                Privilege::Create
            },
            Some(Token::Drop) => {
                parser.advance()?;
                Privilege::Drop
            },
            Some(Token::Alter) => {
                parser.advance()?;
                Privilege::Alter
            },
            Some(Token::Execute) => {
                parser.advance()?;
                Privilege::Execute
            },
            Some(Token::Usage) => {
                parser.advance()?;
                Privilege::Usage
            },
            Some(token) => return Err(ParseError {
                message: format!("Expected privilege name, found {:?}", token),
                position: parser.position,
                expected: vec![
                    "ALL".to_string(), "SELECT".to_string(), "INSERT".to_string(),
                    "UPDATE".to_string(), "DELETE".to_string(), "CREATE".to_string(),
                    "DROP".to_string(), "ALTER".to_string(), "EXECUTE".to_string(),
                    "USAGE".to_string(),
                ],
                found: Some(token.clone()),
            }),
            None => return Err(ParseError {
                message: "Expected privilege name, found EOF".to_string(),
                position: parser.position,
                expected: vec!["privilege name".to_string()],
                found: None,
            }),
        };
        
        privileges.push(privilege);
        
        // Check for comma to continue with more privileges
        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }
    
    Ok(privileges)
}

/// Parse object type and name (e.g., TABLE users, SCHEMA public)
fn parse_object_reference(parser: &mut SqlParser) -> ParseResult<(ObjectType, String)> {
    let object_type = match &parser.current_token {
        Some(Token::Table) => {
            parser.advance()?;
            ObjectType::Table
        },
        Some(Token::View) => {
            parser.advance()?;
            ObjectType::View
        },
        Some(Token::Schema) => {
            parser.advance()?;
            ObjectType::Schema
        },
        Some(Token::Function) => {
            parser.advance()?;
            ObjectType::Function
        },
        Some(Token::Sequence) => {
            parser.advance()?;
            ObjectType::Sequence
        },
        Some(Token::Database) => {
            parser.advance()?;
            ObjectType::Database
        },
        Some(token) => return Err(ParseError {
            message: format!("Expected object type, found {:?}", token),
            position: parser.position,
            expected: vec![
                "TABLE".to_string(), "VIEW".to_string(), "SCHEMA".to_string(),
                "FUNCTION".to_string(), "SEQUENCE".to_string(), "DATABASE".to_string(),
            ],
            found: Some(token.clone()),
        }),
        None => return Err(ParseError {
            message: "Expected object type, found EOF".to_string(),
            position: parser.position,
            expected: vec!["object type".to_string()],
            found: None,
        }),
    };
    
    // Parse object name
    let object_name = match &parser.current_token {
        Some(Token::Identifier(name)) => {
            let name = name.clone();
            parser.advance()?;
            name
        },
        Some(token) => return Err(ParseError {
            message: format!("Expected object name, found {:?}", token),
            position: parser.position,
            expected: vec!["identifier".to_string()],
            found: Some(token.clone()),
        }),
        None => return Err(ParseError {
            message: "Expected object name, found EOF".to_string(),
            position: parser.position,
            expected: vec!["identifier".to_string()],
            found: None,
        }),
    };
    
    Ok((object_type, object_name))
}

/// Parse grantee list (user names or roles)
fn parse_grantee_list(parser: &mut SqlParser) -> ParseResult<Vec<String>> {
    let mut grantees = Vec::new();
    
    loop {
        let grantee = match &parser.current_token {
            Some(Token::Identifier(name)) => {
                let name = name.clone();
                parser.advance()?;
                name
            },
            Some(Token::Public) => {
                parser.advance()?;
                "PUBLIC".to_string()
            },
            Some(token) => return Err(ParseError {
                message: format!("Expected grantee name, found {:?}", token),
                position: parser.position,
                expected: vec!["identifier".to_string(), "PUBLIC".to_string()],
                found: Some(token.clone()),
            }),
            None => return Err(ParseError {
                message: "Expected grantee name, found EOF".to_string(),
                position: parser.position,
                expected: vec!["identifier".to_string()],
                found: None,
            }),
        };
        
        grantees.push(grantee);
        
        // Check for comma to continue with more grantees
        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }
    
    Ok(grantees)
}
