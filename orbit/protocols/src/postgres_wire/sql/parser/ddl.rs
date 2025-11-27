//! DDL (Data Definition Language) Parser Implementation
//!
//! This module handles parsing of CREATE, ALTER, and DROP statements
//! with full support for tables, indexes, views, schemas, and extensions.

use super::{utilities, ParseError, ParseResult, SqlParser};
use crate::postgres_wire::sql::{
    ast::{
        AlterColumnAction, AlterTableAction, AlterTableStatement, ColumnConstraint,
        ColumnDefinition, CreateDatabaseStatement, CreateExtensionStatement, CreateIndexStatement,
        CreateSchemaStatement, CreateTableStatement, CreateViewStatement, DropDatabaseStatement,
        DropExtensionStatement, DropIndexStatement, DropSchemaStatement, DropTableStatement,
        DropViewStatement, IndexColumn, IndexOption, IndexType, NullsOrder, SortDirection,
        Statement, TableConstraint, TableOption,
    },
    lexer::Token,
    types::SqlValue,
};

/// Parse CREATE DATABASE statement
pub fn parse_create_database(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Database)?;

    // Check for IF NOT EXISTS
    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse database name
    let name = if let Some(Token::Identifier(db_name)) = &parser.current_token {
        let name = db_name.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected database name".to_string(),
            position: parser.position,
            expected: vec!["database_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional database options
    let mut owner = None;
    let mut template = None;
    let mut encoding = None;
    let locale = None;
    let connection_limit = None;

    while parser.matches(&[Token::With, Token::Owner, Token::Template, Token::Encoding]) {
        if parser.matches(&[Token::With]) {
            parser.advance()?;
            continue;
        }

        if parser.matches(&[Token::Owner]) {
            parser.advance()?;
            if let Some(Token::Identifier(owner_name)) = &parser.current_token {
                owner = Some(owner_name.clone());
                parser.advance()?;
            }
        } else if parser.matches(&[Token::Template]) {
            parser.advance()?;
            if let Some(Token::Identifier(tmpl)) = &parser.current_token {
                template = Some(tmpl.clone());
                parser.advance()?;
            }
        } else if parser.matches(&[Token::Encoding]) {
            parser.advance()?;
            if let Some(Token::StringLiteral(enc)) = &parser.current_token {
                encoding = Some(enc.clone());
                parser.advance()?;
            } else if let Some(Token::Identifier(enc)) = &parser.current_token {
                encoding = Some(enc.clone());
                parser.advance()?;
            }
        }
    }

    Ok(Statement::CreateDatabase(CreateDatabaseStatement {
        if_not_exists,
        name,
        owner,
        template,
        encoding,
        locale,
        connection_limit,
    }))
}

/// Parse DROP DATABASE statement
pub fn parse_drop_database(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Database)?;

    // Check for IF EXISTS
    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse database names
    let mut names = Vec::new();
    loop {
        if let Some(Token::Identifier(db_name)) = &parser.current_token {
            names.push(db_name.clone());
            parser.advance()?;
        } else {
            return Err(ParseError {
                message: "Expected database name".to_string(),
                position: parser.position,
                expected: vec!["database_name".to_string()],
                found: parser.current_token.clone(),
            });
        }

        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }

    // Check for FORCE option
    let force = parser.matches(&[Token::Force]);
    if force {
        parser.advance()?;
    }

    Ok(Statement::DropDatabase(DropDatabaseStatement {
        if_exists,
        names,
        force,
    }))
}

/// Parse CREATE TABLE statement
pub fn parse_create_table(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Table)?;

    // Check for IF NOT EXISTS
    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse table name
    let name = utilities::parse_table_name(parser)?;

    // Parse column definitions and constraints
    parser.expect(Token::LeftParen)?;

    let mut columns = Vec::new();
    let mut constraints = Vec::new();

    while !parser.matches(&[Token::RightParen]) {
        if parser.matches(&[
            Token::Constraint,
            Token::Primary,
            Token::Unique,
            Token::Foreign,
            Token::Check,
        ]) {
            // Parse table constraint
            let constraint = parse_table_constraint(parser)?;
            constraints.push(constraint);
        } else {
            // Parse column definition
            let column = parse_column_definition(parser)?;
            columns.push(column);
        }

        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else if !parser.matches(&[Token::RightParen]) {
            return Err(ParseError {
                message: "Expected ',' or ')' after column definition".to_string(),
                position: parser.position,
                expected: vec![",".to_string(), ")".to_string()],
                found: parser.current_token.clone(),
            });
        }
    }

    parser.expect(Token::RightParen)?;

    // Parse table options (WITH clause, etc.)
    let options = if parser.matches(&[Token::With]) {
        parse_table_options(parser)?
    } else {
        Vec::new()
    };

    Ok(Statement::CreateTable(CreateTableStatement {
        if_not_exists,
        name,
        columns,
        constraints,
        options,
    }))
}

/// Parse CREATE INDEX statement (called after UNIQUE is already consumed if present)
pub fn parse_create_index(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Index)?;
    parse_create_index_internal(parser)
}

/// Internal function to parse CREATE INDEX (without consuming INDEX token)
pub(crate) fn parse_create_index_internal(parser: &mut SqlParser) -> ParseResult<Statement> {
    // Check for IF NOT EXISTS
    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse optional index name
    let name = if let Some(Token::Identifier(index_name)) = &parser.current_token {
        let name = index_name.clone();
        parser.advance()?;
        Some(name)
    } else {
        None
    };

    parser.expect(Token::On)?;

    // Parse table name
    let table = utilities::parse_table_name(parser)?;

    // Parse index method (USING clause)
    let index_type = if parser.matches(&[Token::Using]) {
        parser.advance()?;
        parse_index_type(parser)?
    } else {
        IndexType::BTree // Default
    };

    // Parse column list
    parser.expect(Token::LeftParen)?;
    let mut columns = Vec::new();

    while !parser.matches(&[Token::RightParen]) {
        let column = parse_index_column(parser)?;
        columns.push(column);

        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }

    parser.expect(Token::RightParen)?;

    // Parse WHERE clause for partial indexes
    let where_clause = if parser.matches(&[Token::Where]) {
        parser.advance()?;
        Some(utilities::parse_expression(parser)?)
    } else {
        None
    };

    // Parse index options (WITH clause)
    let options = if parser.matches(&[Token::With]) {
        parse_index_options(parser)?
    } else {
        Vec::new()
    };

    // Extract parameters for vector indexes
    let final_index_type = match index_type {
        IndexType::IvfFlat { .. } => {
            let lists = options
                .iter()
                .find(|opt| opt.name.to_lowercase() == "lists")
                .and_then(|opt| match &opt.value {
                    SqlValue::Integer(i) => Some(*i),
                    _ => None,
                });
            IndexType::IvfFlat { lists }
        }
        IndexType::Hnsw { .. } => {
            let m = options
                .iter()
                .find(|opt| opt.name.to_lowercase() == "m")
                .and_then(|opt| match &opt.value {
                    SqlValue::Integer(i) => Some(*i),
                    _ => None,
                });
            let ef_construction = options
                .iter()
                .find(|opt| opt.name.to_lowercase() == "ef_construction")
                .and_then(|opt| match &opt.value {
                    SqlValue::Integer(i) => Some(*i),
                    _ => None,
                });
            IndexType::Hnsw { m, ef_construction }
        }
        other => other,
    };

    Ok(Statement::CreateIndex(CreateIndexStatement {
        if_not_exists,
        unique: false, // Set by caller if UNIQUE was present
        name,
        table,
        columns,
        index_type: final_index_type,
        where_clause,
        options,
    }))
}

/// Parse CREATE VIEW statement (called after OR REPLACE is already consumed if present)
pub fn parse_create_view(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::View)?;
    parse_create_view_internal(parser)
}

/// Internal function to parse CREATE VIEW
pub(crate) fn parse_create_view_internal(parser: &mut SqlParser) -> ParseResult<Statement> {
    // Check for materialized view
    let materialized = if parser.matches(&[Token::Materialized]) {
        parser.advance()?;
        true
    } else {
        false
    };

    // Check for IF NOT EXISTS
    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse view name
    let name = utilities::parse_table_name(parser)?;

    // Parse optional column list
    let columns = if parser.matches(&[Token::LeftParen]) {
        parser.advance()?;
        let mut cols = Vec::new();

        while !parser.matches(&[Token::RightParen]) {
            if let Some(Token::Identifier(col_name)) = &parser.current_token {
                cols.push(col_name.clone());
                parser.advance()?;

                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            } else {
                return Err(ParseError {
                    message: "Expected column name in view definition".to_string(),
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

    parser.expect(Token::As)?;

    // Parse the view query (SELECT statement)
    let query = Box::new(utilities::parse_select_statement(parser)?);

    Ok(Statement::CreateView(CreateViewStatement {
        if_not_exists,
        name,
        columns,
        query,
        materialized,
        replace: false, // Set by caller if OR REPLACE was present
    }))
}

/// Parse CREATE SCHEMA statement
pub fn parse_create_schema(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Schema)?;

    // Check for IF NOT EXISTS
    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse schema name
    let name = if let Some(Token::Identifier(schema_name)) = &parser.current_token {
        let name = schema_name.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected schema name".to_string(),
            position: parser.position,
            expected: vec!["schema name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional AUTHORIZATION clause
    let authorization = if parser.matches(&[Token::Authorization]) {
        parser.advance()?;
        if let Some(Token::Identifier(auth_name)) = &parser.current_token {
            let auth = auth_name.clone();
            parser.advance()?;
            Some(auth)
        } else {
            return Err(ParseError {
                message: "Expected authorization name".to_string(),
                position: parser.position,
                expected: vec!["authorization name".to_string()],
                found: parser.current_token.clone(),
            });
        }
    } else {
        None
    };

    Ok(Statement::CreateSchema(CreateSchemaStatement {
        if_not_exists,
        name,
        authorization,
    }))
}

/// Parse CREATE EXTENSION statement
pub fn parse_create_extension(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Extension)?;

    // Check for IF NOT EXISTS
    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse extension name
    let name = if let Some(Token::Identifier(ext_name)) = &parser.current_token {
        let name = ext_name.clone();
        parser.advance()?;
        name
    } else if let Some(Token::StringLiteral(ext_name)) = &parser.current_token {
        let name = ext_name.clone();
        parser.advance()?;
        name
    } else if matches!(&parser.current_token, Some(Token::Vector)) {
        parser.advance()?;
        "vector".to_string()
    } else {
        return Err(ParseError {
            message: "Expected extension name".to_string(),
            position: parser.position,
            expected: vec!["extension name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional clauses
    let mut schema = None;
    let mut version = None;
    let mut cascade = false;

    while parser.matches(&[Token::With, Token::Schema, Token::Version, Token::Cascade]) {
        match &parser.current_token {
            Some(Token::With) => {
                parser.advance()?;
                // WITH can be followed by SCHEMA or VERSION
                continue;
            }
            Some(Token::Schema) => {
                parser.advance()?;
                if let Some(Token::Identifier(schema_name)) = &parser.current_token {
                    schema = Some(schema_name.clone());
                    parser.advance()?;
                }
            }
            Some(Token::Version) => {
                parser.advance()?;
                if let Some(Token::StringLiteral(ver)) = &parser.current_token {
                    version = Some(ver.clone());
                    parser.advance()?;
                }
            }
            Some(Token::Cascade) => {
                cascade = true;
                parser.advance()?;
            }
            _ => break,
        }
    }

    Ok(Statement::CreateExtension(CreateExtensionStatement {
        if_not_exists,
        name,
        schema,
        version,
        cascade,
    }))
}

/// Parse ALTER TABLE statement
pub fn parse_alter_table(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Table)?;

    // Parse table name
    let name = utilities::parse_table_name(parser)?;

    // Parse alter actions
    let mut actions = Vec::new();

    loop {
        let action = match &parser.current_token {
            Some(Token::Add) => {
                parser.advance()?;
                if parser.matches(&[Token::Column]) {
                    parser.advance()?;
                    let column = parse_column_definition(parser)?;
                    AlterTableAction::AddColumn(column)
                } else if parser.matches(&[Token::Constraint]) {
                    let constraint = parse_table_constraint(parser)?;
                    AlterTableAction::AddConstraint(constraint)
                } else {
                    return Err(ParseError {
                        message: "Expected COLUMN or CONSTRAINT after ADD".to_string(),
                        position: parser.position,
                        expected: vec!["COLUMN".to_string(), "CONSTRAINT".to_string()],
                        found: parser.current_token.clone(),
                    });
                }
            }
            Some(Token::Drop) => {
                parser.advance()?;
                if parser.matches(&[Token::Column]) {
                    parser.advance()?;
                    if let Some(Token::Identifier(col_name)) = &parser.current_token {
                        let name = col_name.clone();
                        parser.advance()?;
                        let cascade = if parser.matches(&[Token::Cascade]) {
                            parser.advance()?;
                            true
                        } else {
                            false
                        };
                        AlterTableAction::DropColumn { name, cascade }
                    } else {
                        return Err(ParseError {
                            message: "Expected column name after DROP COLUMN".to_string(),
                            position: parser.position,
                            expected: vec!["column name".to_string()],
                            found: parser.current_token.clone(),
                        });
                    }
                } else if parser.matches(&[Token::Constraint]) {
                    parser.advance()?;
                    if let Some(Token::Identifier(constraint_name)) = &parser.current_token {
                        let name = constraint_name.clone();
                        parser.advance()?;
                        let cascade = if parser.matches(&[Token::Cascade]) {
                            parser.advance()?;
                            true
                        } else {
                            false
                        };
                        AlterTableAction::DropConstraint { name, cascade }
                    } else {
                        return Err(ParseError {
                            message: "Expected constraint name after DROP CONSTRAINT".to_string(),
                            position: parser.position,
                            expected: vec!["constraint name".to_string()],
                            found: parser.current_token.clone(),
                        });
                    }
                } else {
                    return Err(ParseError {
                        message: "Expected COLUMN or CONSTRAINT after DROP".to_string(),
                        position: parser.position,
                        expected: vec!["COLUMN".to_string(), "CONSTRAINT".to_string()],
                        found: parser.current_token.clone(),
                    });
                }
            }
            Some(Token::Alter) => {
                parser.advance()?;
                parser.expect(Token::Column)?;
                if let Some(Token::Identifier(col_name)) = &parser.current_token {
                    let name = col_name.clone();
                    parser.advance()?;
                    let action = parse_alter_column_action(parser)?;
                    AlterTableAction::AlterColumn { name, action }
                } else {
                    return Err(ParseError {
                        message: "Expected column name after ALTER COLUMN".to_string(),
                        position: parser.position,
                        expected: vec!["column name".to_string()],
                        found: parser.current_token.clone(),
                    });
                }
            }
            _ => break,
        };

        actions.push(action);

        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }

    Ok(Statement::AlterTable(AlterTableStatement { name, actions }))
}

/// Parse DROP TABLE statement
pub fn parse_drop_table(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Table)?;

    // Check for IF EXISTS
    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse table names (can be multiple)
    let mut names = Vec::new();

    loop {
        let name = utilities::parse_table_name(parser)?;
        names.push(name);

        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }

    // Check for CASCADE
    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropTable(DropTableStatement {
        if_exists,
        names,
        cascade,
    }))
}

/// Parse DROP INDEX statement
pub fn parse_drop_index(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Index)?;

    // Check for IF EXISTS
    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse index names
    let mut names = Vec::new();

    loop {
        if let Some(Token::Identifier(index_name)) = &parser.current_token {
            names.push(index_name.clone());
            parser.advance()?;

            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        } else {
            return Err(ParseError {
                message: "Expected index name".to_string(),
                position: parser.position,
                expected: vec!["index name".to_string()],
                found: parser.current_token.clone(),
            });
        }
    }

    // Check for CASCADE
    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropIndex(DropIndexStatement {
        if_exists,
        names,
        cascade,
    }))
}

/// Parse DROP VIEW statement
pub fn parse_drop_view(parser: &mut SqlParser) -> ParseResult<Statement> {
    // Check for materialized view
    let materialized = if parser.matches(&[Token::Materialized]) {
        parser.advance()?;
        true
    } else {
        false
    };

    parser.expect(Token::View)?;

    // Check for IF EXISTS
    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse view names
    let mut names = Vec::new();

    loop {
        let name = utilities::parse_table_name(parser)?;
        names.push(name);

        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }

    // Check for CASCADE
    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropView(DropViewStatement {
        if_exists,
        names,
        cascade,
        materialized,
    }))
}

/// Parse DROP SCHEMA statement
pub fn parse_drop_schema(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Schema)?;

    // Check for IF EXISTS
    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse schema names
    let mut names = Vec::new();

    loop {
        if let Some(Token::Identifier(schema_name)) = &parser.current_token {
            names.push(schema_name.clone());
            parser.advance()?;

            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        } else {
            return Err(ParseError {
                message: "Expected schema name".to_string(),
                position: parser.position,
                expected: vec!["schema name".to_string()],
                found: parser.current_token.clone(),
            });
        }
    }

    // Check for CASCADE
    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropSchema(DropSchemaStatement {
        if_exists,
        names,
        cascade,
    }))
}

/// Parse DROP EXTENSION statement
pub fn parse_drop_extension(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Extension)?;

    // Check for IF EXISTS
    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse extension names
    let mut names = Vec::new();

    loop {
        if let Some(Token::Identifier(ext_name)) = &parser.current_token {
            names.push(ext_name.clone());
            parser.advance()?;
        } else if let Some(Token::StringLiteral(ext_name)) = &parser.current_token {
            names.push(ext_name.clone());
            parser.advance()?;
        } else if matches!(&parser.current_token, Some(Token::Vector)) {
            names.push("vector".to_string());
            parser.advance()?;
        } else {
            return Err(ParseError {
                message: "Expected extension name".to_string(),
                position: parser.position,
                expected: vec!["extension name".to_string()],
                found: parser.current_token.clone(),
            });
        }

        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }

    // Check for CASCADE
    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropExtension(DropExtensionStatement {
        if_exists,
        names,
        cascade,
    }))
}

// Helper functions

/// Parse column definition
fn parse_column_definition(parser: &mut SqlParser) -> ParseResult<ColumnDefinition> {
    // Parse column name (can be identifier or keyword used as identifier)
    let name = if let Some(col_name) = parser
        .current_token
        .as_ref()
        .and_then(utilities::token_to_identifier_name)
    {
        parser.advance()?;
        col_name
    } else {
        return Err(ParseError {
            message: "Expected column name".to_string(),
            position: parser.position,
            expected: vec!["column name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse data type
    let data_type = utilities::parse_data_type(parser)?;

    // Parse column constraints
    let mut constraints = Vec::new();

    while parser.matches(&[
        Token::Not,
        Token::Null,
        Token::Default,
        Token::Primary,
        Token::Unique,
        Token::References,
        Token::Check,
    ]) {
        match &parser.current_token {
            Some(Token::Not) => {
                parser.advance()?;
                parser.expect(Token::Null)?;
                constraints.push(ColumnConstraint::NotNull);
            }
            Some(Token::Null) => {
                parser.advance()?;
                constraints.push(ColumnConstraint::Null);
            }
            Some(Token::Default) => {
                parser.advance()?;
                let expr = utilities::parse_expression(parser)?;
                constraints.push(ColumnConstraint::Default(expr));
            }
            Some(Token::Primary) => {
                parser.advance()?;
                parser.expect(Token::Key)?;
                constraints.push(ColumnConstraint::PrimaryKey);
            }
            Some(Token::Unique) => {
                parser.advance()?;
                constraints.push(ColumnConstraint::Unique);
            }
            Some(Token::References) => {
                parser.advance()?;
                let table = utilities::parse_table_name(parser)?;
                let columns = if parser.matches(&[Token::LeftParen]) {
                    parser.advance()?;
                    let mut cols = Vec::new();

                    while !parser.matches(&[Token::RightParen]) {
                        if let Some(Token::Identifier(col_name)) = &parser.current_token {
                            cols.push(col_name.clone());
                            parser.advance()?;

                            if parser.matches(&[Token::Comma]) {
                                parser.advance()?;
                            } else {
                                break;
                            }
                        }
                    }

                    parser.expect(Token::RightParen)?;
                    Some(cols)
                } else {
                    None
                };

                constraints.push(ColumnConstraint::References {
                    table,
                    columns,
                    on_delete: None, // TODO: Parse ON DELETE/UPDATE actions
                    on_update: None,
                });
            }
            Some(Token::Check) => {
                parser.advance()?;
                parser.expect(Token::LeftParen)?;
                let expr = utilities::parse_expression(parser)?;
                parser.expect(Token::RightParen)?;
                constraints.push(ColumnConstraint::Check(expr));
            }
            _ => break,
        }
    }

    Ok(ColumnDefinition {
        name,
        data_type,
        constraints,
    })
}

/// Parse table constraint
fn parse_table_constraint(parser: &mut SqlParser) -> ParseResult<TableConstraint> {
    let constraint_name = if parser.matches(&[Token::Constraint]) {
        parser.advance()?;
        if let Some(Token::Identifier(name)) = &parser.current_token {
            let name = name.clone();
            parser.advance()?;
            Some(name)
        } else {
            None
        }
    } else {
        None
    };

    match &parser.current_token {
        Some(Token::Primary) => {
            parser.advance()?;
            parser.expect(Token::Key)?;
            parser.expect(Token::LeftParen)?;

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

            Ok(TableConstraint::PrimaryKey {
                name: constraint_name,
                columns,
            })
        }
        Some(Token::Unique) => {
            parser.advance()?;
            parser.expect(Token::LeftParen)?;

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

            Ok(TableConstraint::Unique {
                name: constraint_name,
                columns,
            })
        }
        Some(Token::Check) => {
            parser.advance()?;
            parser.expect(Token::LeftParen)?;
            let expression = utilities::parse_expression(parser)?;
            parser.expect(Token::RightParen)?;

            Ok(TableConstraint::Check {
                name: constraint_name,
                expression,
            })
        }
        _ => Err(ParseError {
            message: "Expected PRIMARY KEY, UNIQUE, or CHECK constraint".to_string(),
            position: parser.position,
            expected: vec![
                "PRIMARY KEY".to_string(),
                "UNIQUE".to_string(),
                "CHECK".to_string(),
            ],
            found: parser.current_token.clone(),
        }),
    }
}

/// Parse index type (USING clause)
fn parse_index_type(parser: &mut SqlParser) -> ParseResult<IndexType> {
    match &parser.current_token {
        Some(Token::Identifier(method)) => {
            let index_type = match method.to_uppercase().as_str() {
                "BTREE" => IndexType::BTree,
                "HASH" => IndexType::Hash,
                "GIST" => IndexType::Gist,
                "GIN" => IndexType::Gin,
                "IVFFLAT" => {
                    parser.advance()?;
                    return Ok(IndexType::IvfFlat { lists: None });
                }
                "HNSW" => {
                    parser.advance()?;
                    return Ok(IndexType::Hnsw {
                        m: None,
                        ef_construction: None,
                    });
                }
                _ => {
                    return Err(ParseError {
                        message: format!("Unknown index method: {method}"),
                        position: parser.position,
                        expected: vec!["BTREE, HASH, GIST, GIN, IVFFLAT, or HNSW".to_string()],
                        found: parser.current_token.clone(),
                    })
                }
            };
            parser.advance()?;
            Ok(index_type)
        }
        Some(Token::IvfFlat) => {
            parser.advance()?;
            Ok(IndexType::IvfFlat { lists: None })
        }
        Some(Token::Hnsw) => {
            parser.advance()?;
            Ok(IndexType::Hnsw {
                m: None,
                ef_construction: None,
            })
        }
        _ => Err(ParseError {
            message: "Expected index method name".to_string(),
            position: parser.position,
            expected: vec!["BTREE, HASH, GIST, GIN, IVFFLAT, or HNSW".to_string()],
            found: parser.current_token.clone(),
        }),
    }
}

/// Parse index column specification
fn parse_index_column(parser: &mut SqlParser) -> ParseResult<IndexColumn> {
    let name = if let Some(Token::Identifier(col_name)) = &parser.current_token {
        let name = col_name.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected column name in index".to_string(),
            position: parser.position,
            expected: vec!["column name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional vector operation class (e.g., vector_l2_ops, vector_cosine_ops, etc.)
    if let Some(Token::Identifier(op_class)) = &parser.current_token {
        if op_class.starts_with("vector_") {
            // Skip vector operation class - we store it as part of the column name for now
            // This could be enhanced to have a separate field in IndexColumn
            parser.advance()?;
        }
    }

    // Parse optional ASC/DESC
    let direction = if parser.matches(&[Token::Identifier("ASC".to_string())]) {
        parser.advance()?;
        Some(SortDirection::Ascending)
    } else if parser.matches(&[Token::Identifier("DESC".to_string())]) {
        parser.advance()?;
        Some(SortDirection::Descending)
    } else {
        None
    };

    // Parse optional NULLS FIRST/LAST
    let nulls = if parser.matches(&[Token::Identifier("NULLS".to_string())]) {
        parser.advance()?;
        if parser.matches(&[Token::Identifier("FIRST".to_string())]) {
            parser.advance()?;
            Some(NullsOrder::First)
        } else if parser.matches(&[Token::Identifier("LAST".to_string())]) {
            parser.advance()?;
            Some(NullsOrder::Last)
        } else {
            None
        }
    } else {
        None
    };

    Ok(IndexColumn {
        name,
        direction,
        nulls,
    })
}

/// Parse alter column action
fn parse_alter_column_action(parser: &mut SqlParser) -> ParseResult<AlterColumnAction> {
    match &parser.current_token {
        Some(Token::Set) => {
            parser.advance()?;
            match &parser.current_token {
                Some(Token::Default) => {
                    parser.advance()?;
                    let expr = utilities::parse_expression(parser)?;
                    Ok(AlterColumnAction::SetDefault(expr))
                }
                Some(Token::Not) => {
                    parser.advance()?;
                    parser.expect(Token::Null)?;
                    Ok(AlterColumnAction::SetNotNull)
                }
                _ => Err(ParseError {
                    message: "Expected DEFAULT or NOT NULL after SET".to_string(),
                    position: parser.position,
                    expected: vec!["DEFAULT".to_string(), "NOT NULL".to_string()],
                    found: parser.current_token.clone(),
                }),
            }
        }
        Some(Token::Drop) => {
            parser.advance()?;
            match &parser.current_token {
                Some(Token::Default) => {
                    parser.advance()?;
                    Ok(AlterColumnAction::DropDefault)
                }
                Some(Token::Not) => {
                    parser.advance()?;
                    parser.expect(Token::Null)?;
                    Ok(AlterColumnAction::DropNotNull)
                }
                _ => Err(ParseError {
                    message: "Expected DEFAULT or NOT NULL after DROP".to_string(),
                    position: parser.position,
                    expected: vec!["DEFAULT".to_string(), "NOT NULL".to_string()],
                    found: parser.current_token.clone(),
                }),
            }
        }
        Some(Token::Identifier(type_keyword)) if type_keyword.to_uppercase() == "TYPE" => {
            parser.advance()?;
            let data_type = utilities::parse_data_type(parser)?;
            Ok(AlterColumnAction::SetType(data_type))
        }
        _ => Err(ParseError {
            message: "Expected SET, DROP, or TYPE in ALTER COLUMN".to_string(),
            position: parser.position,
            expected: vec!["SET".to_string(), "DROP".to_string(), "TYPE".to_string()],
            found: parser.current_token.clone(),
        }),
    }
}

/// Parse table options (WITH clause)
fn parse_table_options(parser: &mut SqlParser) -> ParseResult<Vec<TableOption>> {
    parser.expect(Token::With)?;
    parser.expect(Token::LeftParen)?;

    let mut options = Vec::new();

    while !parser.matches(&[Token::RightParen]) {
        if let Some(Token::Identifier(option_name)) = &parser.current_token {
            let name = option_name.clone();
            parser.advance()?;

            let value = if parser.matches(&[Token::Equal]) {
                parser.advance()?;
                Some(utilities::parse_literal_value(parser)?)
            } else {
                None
            };

            options.push(TableOption { name, value });

            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        } else {
            return Err(ParseError {
                message: "Expected option name in WITH clause".to_string(),
                position: parser.position,
                expected: vec!["option name".to_string()],
                found: parser.current_token.clone(),
            });
        }
    }

    parser.expect(Token::RightParen)?;
    Ok(options)
}

/// Parse index options (WITH clause for indexes)
fn parse_index_options(parser: &mut SqlParser) -> ParseResult<Vec<IndexOption>> {
    parser.expect(Token::With)?;
    parser.expect(Token::LeftParen)?;

    let mut options = Vec::new();

    while !parser.matches(&[Token::RightParen]) {
        let name = match &parser.current_token {
            Some(Token::Identifier(option_name)) => option_name.clone(),
            Some(Token::Lists) => "lists".to_string(),
            Some(Token::M) => "m".to_string(),
            Some(Token::EfConstruction) => "ef_construction".to_string(),
            _ => {
                return Err(ParseError {
                    message: "Expected option name in WITH clause".to_string(),
                    position: parser.position,
                    expected: vec!["option name".to_string()],
                    found: parser.current_token.clone(),
                });
            }
        };
        parser.advance()?;

        parser.expect(Token::Equal)?;
        let value = utilities::parse_literal_value(parser)?;

        options.push(IndexOption { name, value });

        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }

    parser.expect(Token::RightParen)?;
    Ok(options)
}
