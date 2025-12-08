//! DDL (Data Definition Language) Parser Implementation
//!
//! This module handles parsing of CREATE, ALTER, and DROP statements
//! with full support for tables, indexes, views, schemas, and extensions.

use super::{utilities, ParseError, ParseResult, SqlParser};
use crate::protocols::postgres_wire::sql::{
    ast::{
        AlterColumnAction, AlterSequenceStatement, AlterTableAction, AlterTableStatement,
        ColumnConstraint, ColumnDefinition, CommentObjectType, CommentOnStatement,
        CreateDatabaseStatement, CreateExtensionStatement, CreateFunctionStatement,
        CreateIndexStatement, CreateSchemaStatement, CreateSequenceStatement, CreateTableStatement,
        CreateTriggerStatement, CreateViewStatement, DropDatabaseStatement, DropExtensionStatement,
        DropIndexStatement, DropSchemaStatement, DropSequenceStatement, DropTableStatement,
        DropTriggerStatement, DropViewStatement, FunctionLanguage, FunctionName, FunctionParameter,
        FunctionVolatility, GeneratedColumnStorage, IndexColumn, IndexOption, IndexType, NullsOrder,
        ParameterMode, ReferentialAction, SequenceBound, SequenceOptions, SequenceOwner,
        SortDirection, Statement, TableConstraint, TableOption, TriggerEvent, TriggerForEach,
        TriggerTiming, TruncateIdentity, TruncateStatement,
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

    // Parse extension name (can be identifier, quoted identifier, or string literal)
    let name = if let Some(Token::Identifier(ext_name)) = &parser.current_token {
        let name = ext_name.clone();
        parser.advance()?;
        name
    } else if let Some(Token::QuotedIdentifier(ext_name)) = &parser.current_token {
        // Handle double-quoted identifiers like "uuid-ossp"
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

    // Parse extension names (can be identifier, quoted identifier, or string literal)
    let mut names = Vec::new();

    loop {
        if let Some(Token::Identifier(ext_name)) = &parser.current_token {
            names.push(ext_name.clone());
            parser.advance()?;
        } else if let Some(Token::QuotedIdentifier(ext_name)) = &parser.current_token {
            // Handle double-quoted identifiers like "uuid-ossp"
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

/// Parse CREATE FUNCTION statement
pub fn parse_create_function(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Function)?;

    // Parse function name
    let name = if let Some(func_name) = parser
        .current_token
        .as_ref()
        .and_then(utilities::token_to_identifier_name)
    {
        parser.advance()?;
        FunctionName::Simple(func_name)
    } else {
        return Err(ParseError {
            message: "Expected function name".to_string(),
            position: parser.position,
            expected: vec!["function name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse parameters
    parser.expect(Token::LeftParen)?;
    let mut args = Vec::new();
    if !parser.matches(&[Token::RightParen]) {
        loop {
            // Parse parameter mode (optional)
            let mode = if parser.matches(&[Token::In]) {
                parser.advance()?;
                Some(ParameterMode::In)
            } else if parser.matches(&[Token::Out]) {
                parser.advance()?;
                Some(ParameterMode::Out)
            } else if parser.matches(&[Token::InOut]) {
                parser.advance()?;
                Some(ParameterMode::InOut)
            } else if parser.matches(&[Token::Variadic]) {
                parser.advance()?;
                Some(ParameterMode::Variadic)
            } else {
                None
            };

            // Parse parameter name (optional)
            let name = if let Some(param_name) = parser
                .current_token
                .as_ref()
                .and_then(utilities::token_to_identifier_name)
            {
                // Check if it's a type name
                if utilities::is_type_name(&parser.current_token) {
                    None
                } else {
                    parser.advance()?;
                    Some(param_name)
                }
            } else {
                None
            };

            // Parse parameter type
            let data_type = utilities::parse_data_type(parser)?;

            // Parse default value (optional)
            let default = if parser.matches(&[Token::Default]) || parser.matches(&[Token::Equal]) {
                parser.advance()?;
                Some(utilities::parse_expression(parser)?)
            } else {
                None
            };

            args.push(FunctionParameter {
                name,
                data_type,
                mode,
                default,
            });

            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
    }
    parser.expect(Token::RightParen)?;

    // Parse RETURNS clause
    let return_type = if parser.matches(&[Token::Returns]) {
        parser.advance()?;
        Some(utilities::parse_data_type(parser)?)
    } else {
        None
    };

    // Parse options (LANGUAGE, AS, etc.)
    let mut language = None;
    let mut body = String::new();
    let mut volatility = None;

    while parser.matches(&[
        Token::Language,
        Token::As,
        Token::Identifier("IMMUTABLE".to_string()),
        Token::Identifier("STABLE".to_string()),
        Token::Identifier("VOLATILE".to_string()),
    ]) {
        if parser.matches(&[Token::Language]) {
            parser.advance()?;
            if let Some(Token::Identifier(lang)) = &parser.current_token {
                language = match lang.to_uppercase().as_str() {
                    "SQL" => Some(FunctionLanguage::Sql),
                    "PLPGSQL" => Some(FunctionLanguage::PlPgSql),
                    _ => Some(FunctionLanguage::Other(lang.clone())),
                };
                parser.advance()?;
            }
        } else if parser.matches(&[Token::As]) {
            parser.advance()?;
            // Expect string literal or dollar-quoted string
            if let Some(Token::StringLiteral(s)) = &parser.current_token {
                body = s.clone();
                parser.advance()?;
            } else if let Some(Token::DollarQuotedString(s)) = &parser.current_token {
                body = s.clone();
                parser.advance()?;
            } else {
                return Err(ParseError {
                    message: "Expected function body as string literal".to_string(),
                    position: parser.position,
                    expected: vec!["string literal".to_string()],
                    found: parser.current_token.clone(),
                });
            }
        } else if let Some(Token::Identifier(v)) = &parser.current_token {
            match v.to_uppercase().as_str() {
                "IMMUTABLE" => {
                    volatility = Some(FunctionVolatility::Immutable);
                    parser.advance()?;
                }
                "STABLE" => {
                    volatility = Some(FunctionVolatility::Stable);
                    parser.advance()?;
                }
                "VOLATILE" => {
                    volatility = Some(FunctionVolatility::Volatile);
                    parser.advance()?;
                }
                _ => break,
            }
        } else {
            break;
        }
    }

    Ok(Statement::CreateFunction(CreateFunctionStatement {
        or_replace: false, // Set by caller in mod.rs
        name,
        args: Some(args),
        return_type,
        language,
        body,
        volatility,
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

                // Parse optional ON DELETE and ON UPDATE clauses
                let mut on_delete = None;
                let mut on_update = None;

                while parser.matches(&[Token::On]) {
                    parser.advance()?;
                    match &parser.current_token {
                        Some(Token::Delete) => {
                            parser.advance()?;
                            on_delete = Some(parse_referential_action(parser)?);
                        }
                        Some(Token::Update) => {
                            parser.advance()?;
                            on_update = Some(parse_referential_action(parser)?);
                        }
                        _ => break,
                    }
                }

                constraints.push(ColumnConstraint::References {
                    table,
                    columns,
                    on_delete,
                    on_update,
                });
            }
            Some(Token::Check) => {
                parser.advance()?;
                parser.expect(Token::LeftParen)?;
                let expr = utilities::parse_expression(parser)?;
                parser.expect(Token::RightParen)?;
                constraints.push(ColumnConstraint::Check(expr));
            }
            // PostgreSQL 12+ GENERATED ALWAYS AS (expression) STORED
            // PostgreSQL 18+ GENERATED ALWAYS AS (expression) VIRTUAL
            Some(Token::Generated) => {
                parser.advance()?;
                // Expect ALWAYS
                parser.expect(Token::Always)?;
                // Expect AS
                parser.expect(Token::As)?;
                // Expect (
                parser.expect(Token::LeftParen)?;
                // Parse expression
                let expr = utilities::parse_expression(parser)?;
                // Expect )
                parser.expect(Token::RightParen)?;
                // Parse STORED or VIRTUAL (default to STORED for compatibility)
                let storage = if parser.matches(&[Token::Stored]) {
                    parser.advance()?;
                    GeneratedColumnStorage::Stored
                } else if parser.matches(&[Token::Virtual]) {
                    parser.advance()?;
                    GeneratedColumnStorage::Virtual
                } else {
                    // Default to STORED if not specified (PostgreSQL 12 behavior)
                    GeneratedColumnStorage::Stored
                };
                constraints.push(ColumnConstraint::Generated {
                    expression: expr,
                    storage,
                });
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
        Some(Token::Foreign) => {
            parser.advance()?;
            parser.expect(Token::Key)?;
            parser.expect(Token::LeftParen)?;

            // Parse column list
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
                } else {
                    break;
                }
            }

            parser.expect(Token::RightParen)?;
            parser.expect(Token::References)?;

            // Parse referenced table
            let references_table = utilities::parse_table_name(parser)?;

            // Parse referenced columns
            parser.expect(Token::LeftParen)?;
            let mut references_columns = Vec::new();
            while !parser.matches(&[Token::RightParen]) {
                if let Some(Token::Identifier(col_name)) = &parser.current_token {
                    references_columns.push(col_name.clone());
                    parser.advance()?;

                    if parser.matches(&[Token::Comma]) {
                        parser.advance()?;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            parser.expect(Token::RightParen)?;

            // Parse optional ON DELETE and ON UPDATE clauses
            let mut on_delete = None;
            let mut on_update = None;

            while parser.matches(&[Token::On]) {
                parser.advance()?;
                match &parser.current_token {
                    Some(Token::Delete) => {
                        parser.advance()?;
                        on_delete = Some(parse_referential_action(parser)?);
                    }
                    Some(Token::Update) => {
                        parser.advance()?;
                        on_update = Some(parse_referential_action(parser)?);
                    }
                    _ => break,
                }
            }

            Ok(TableConstraint::ForeignKey {
                name: constraint_name,
                columns,
                references_table,
                references_columns,
                on_delete,
                on_update,
            })
        }
        _ => Err(ParseError {
            message: "Expected PRIMARY KEY, UNIQUE, CHECK, or FOREIGN KEY constraint".to_string(),
            position: parser.position,
            expected: vec![
                "PRIMARY KEY".to_string(),
                "UNIQUE".to_string(),
                "CHECK".to_string(),
                "FOREIGN KEY".to_string(),
            ],
            found: parser.current_token.clone(),
        }),
    }
}

/// Parse referential action for ON DELETE / ON UPDATE
fn parse_referential_action(parser: &mut SqlParser) -> ParseResult<ReferentialAction> {
    match &parser.current_token {
        Some(Token::Cascade) => {
            parser.advance()?;
            Ok(ReferentialAction::Cascade)
        }
        Some(Token::Set) => {
            parser.advance()?;
            match &parser.current_token {
                Some(Token::Null) => {
                    parser.advance()?;
                    Ok(ReferentialAction::SetNull)
                }
                Some(Token::Default) => {
                    parser.advance()?;
                    Ok(ReferentialAction::SetDefault)
                }
                _ => Err(ParseError {
                    message: "Expected NULL or DEFAULT after SET".to_string(),
                    position: parser.position,
                    expected: vec!["NULL".to_string(), "DEFAULT".to_string()],
                    found: parser.current_token.clone(),
                }),
            }
        }
        Some(Token::Restrict) => {
            parser.advance()?;
            Ok(ReferentialAction::Restrict)
        }
        Some(Token::No) => {
            parser.advance()?;
            // Expect ACTION keyword (as identifier since NO ACTION is two words)
            if let Some(Token::Identifier(id)) = &parser.current_token {
                if id.to_uppercase() == "ACTION" {
                    parser.advance()?;
                    return Ok(ReferentialAction::NoAction);
                }
            }
            Err(ParseError {
                message: "Expected ACTION after NO".to_string(),
                position: parser.position,
                expected: vec!["ACTION".to_string()],
                found: parser.current_token.clone(),
            })
        }
        _ => Err(ParseError {
            message:
                "Expected referential action (CASCADE, SET NULL, SET DEFAULT, RESTRICT, NO ACTION)"
                    .to_string(),
            position: parser.position,
            expected: vec![
                "CASCADE".to_string(),
                "SET NULL".to_string(),
                "SET DEFAULT".to_string(),
                "RESTRICT".to_string(),
                "NO ACTION".to_string(),
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

/// Parse CREATE TRIGGER statement
pub fn parse_create_trigger(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Trigger)?;

    // Parse trigger name
    let name = if let Some(Token::Identifier(trigger_name)) = &parser.current_token {
        let name = trigger_name.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected trigger name".to_string(),
            position: parser.position,
            expected: vec!["trigger name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse timing (BEFORE, AFTER, INSTEAD OF)
    let timing = match &parser.current_token {
        Some(Token::Before) => {
            parser.advance()?;
            TriggerTiming::Before
        }
        Some(Token::After) => {
            parser.advance()?;
            TriggerTiming::After
        }
        Some(Token::Instead) => {
            parser.advance()?;
            parser.expect(Token::Of)?;
            TriggerTiming::InsteadOf
        }
        _ => {
            return Err(ParseError {
                message: "Expected BEFORE, AFTER, or INSTEAD OF".to_string(),
                position: parser.position,
                expected: vec![
                    "BEFORE".to_string(),
                    "AFTER".to_string(),
                    "INSTEAD OF".to_string(),
                ],
                found: parser.current_token.clone(),
            });
        }
    };

    // Parse events (INSERT, UPDATE, DELETE, TRUNCATE)
    let mut events = Vec::new();
    loop {
        let event = match &parser.current_token {
            Some(Token::Insert) => {
                parser.advance()?;
                TriggerEvent::Insert
            }
            Some(Token::Update) => {
                parser.advance()?;
                // Check for OF column_list
                let columns = if parser.matches(&[Token::Of]) {
                    parser.advance()?;
                    let mut cols = Vec::new();
                    while let Some(Token::Identifier(col_name)) = &parser.current_token {
                        cols.push(col_name.clone());
                        parser.advance()?;
                        // Check for comma and next column
                        if parser.matches(&[Token::Comma]) {
                            if let Some(Token::Identifier(_)) =
                                parser.tokens.get(parser.position + 1)
                            {
                                parser.advance()?;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    if cols.is_empty() {
                        None
                    } else {
                        Some(cols)
                    }
                } else {
                    None
                };
                TriggerEvent::Update(columns)
            }
            Some(Token::Delete) => {
                parser.advance()?;
                TriggerEvent::Delete
            }
            Some(Token::Identifier(t)) if t.to_uppercase() == "TRUNCATE" => {
                parser.advance()?;
                TriggerEvent::Truncate
            }
            _ => break,
        };
        events.push(event);

        // Check for OR to continue
        if parser.matches(&[Token::Or]) {
            parser.advance()?;
        } else {
            break;
        }
    }

    if events.is_empty() {
        return Err(ParseError {
            message: "Expected at least one trigger event (INSERT, UPDATE, DELETE, TRUNCATE)"
                .to_string(),
            position: parser.position,
            expected: vec![
                "INSERT".to_string(),
                "UPDATE".to_string(),
                "DELETE".to_string(),
                "TRUNCATE".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    }

    // Parse ON table_name
    parser.expect(Token::On)?;
    let table = utilities::parse_table_name(parser)?;

    // Parse FOR EACH ROW or FOR EACH STATEMENT (optional, default is STATEMENT)
    let for_each = if parser.matches(&[Token::For]) {
        parser.advance()?;
        parser.expect(Token::Each)?;
        match &parser.current_token {
            Some(Token::Row) => {
                parser.advance()?;
                TriggerForEach::Row
            }
            Some(Token::Statement) => {
                parser.advance()?;
                TriggerForEach::Statement
            }
            _ => TriggerForEach::Statement,
        }
    } else {
        TriggerForEach::Statement
    };

    // Parse optional WHEN clause
    let when_clause = if parser.matches(&[Token::When]) {
        parser.advance()?;
        parser.expect(Token::LeftParen)?;
        let expr = utilities::parse_expression(parser)?;
        parser.expect(Token::RightParen)?;
        Some(expr)
    } else {
        None
    };

    // Parse EXECUTE FUNCTION/PROCEDURE function_name(args)
    parser.expect(Token::Execute)?;
    // Accept either FUNCTION or PROCEDURE
    if parser.matches(&[Token::Function]) || parser.matches(&[Token::Procedure]) {
        parser.advance()?;
    }

    // Parse function name
    let function = if let Some(func_name) = parser
        .current_token
        .as_ref()
        .and_then(utilities::token_to_identifier_name)
    {
        parser.advance()?;
        FunctionName::Simple(func_name)
    } else {
        return Err(ParseError {
            message: "Expected function name".to_string(),
            position: parser.position,
            expected: vec!["function name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse function arguments
    parser.expect(Token::LeftParen)?;
    let mut function_args = Vec::new();
    if !parser.matches(&[Token::RightParen]) {
        loop {
            let arg = utilities::parse_expression(parser)?;
            function_args.push(arg);
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
    }
    parser.expect(Token::RightParen)?;

    Ok(Statement::CreateTrigger(CreateTriggerStatement {
        or_replace: false, // Set by caller if OR REPLACE was present
        name,
        timing,
        events,
        table,
        for_each,
        when_clause,
        function,
        function_args,
    }))
}

/// Parse DROP TRIGGER statement
pub fn parse_drop_trigger(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Trigger)?;

    // Check for IF EXISTS
    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse trigger name
    let name = if let Some(Token::Identifier(trigger_name)) = &parser.current_token {
        let n = trigger_name.clone();
        parser.advance()?;
        n
    } else {
        return Err(ParseError {
            message: "Expected trigger name".to_string(),
            position: parser.position,
            expected: vec!["trigger name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse ON table_name
    parser.expect(Token::On)?;
    let table = utilities::parse_table_name(parser)?;

    // Check for CASCADE
    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropTrigger(DropTriggerStatement {
        if_exists,
        name,
        table,
        cascade,
    }))
}

/// Parse COMMENT ON statement
pub fn parse_comment_on(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::CommentKeyword)?;
    parser.expect(Token::On)?;

    // Parse object type
    let (object_type, object_name, column_name) = match &parser.current_token {
        Some(Token::Table) => {
            parser.advance()?;
            let name = utilities::parse_table_name(parser)?;
            (CommentObjectType::Table, name.full_name(), None)
        }
        Some(Token::Column) => {
            parser.advance()?;
            // Parse table.column format
            let table_name = utilities::parse_table_name(parser)?;
            parser.expect(Token::Dot)?;
            let col_name = if let Some(Token::Identifier(c)) = &parser.current_token {
                let n = c.clone();
                parser.advance()?;
                n
            } else {
                return Err(ParseError {
                    message: "Expected column name".to_string(),
                    position: parser.position,
                    expected: vec!["column name".to_string()],
                    found: parser.current_token.clone(),
                });
            };
            (
                CommentObjectType::Column,
                table_name.full_name(),
                Some(col_name),
            )
        }
        Some(Token::Index) => {
            parser.advance()?;
            let name = if let Some(Token::Identifier(n)) = &parser.current_token {
                let idx_name = n.clone();
                parser.advance()?;
                idx_name
            } else {
                return Err(ParseError {
                    message: "Expected index name".to_string(),
                    position: parser.position,
                    expected: vec!["index name".to_string()],
                    found: parser.current_token.clone(),
                });
            };
            (CommentObjectType::Index, name, None)
        }
        Some(Token::View) => {
            parser.advance()?;
            let name = utilities::parse_table_name(parser)?;
            (CommentObjectType::View, name.full_name(), None)
        }
        Some(Token::Schema) => {
            parser.advance()?;
            let name = if let Some(Token::Identifier(n)) = &parser.current_token {
                let schema_name = n.clone();
                parser.advance()?;
                schema_name
            } else {
                return Err(ParseError {
                    message: "Expected schema name".to_string(),
                    position: parser.position,
                    expected: vec!["schema name".to_string()],
                    found: parser.current_token.clone(),
                });
            };
            (CommentObjectType::Schema, name, None)
        }
        Some(Token::Extension) => {
            parser.advance()?;
            let name = if let Some(Token::Identifier(n)) = &parser.current_token {
                let ext_name = n.clone();
                parser.advance()?;
                ext_name
            } else if let Some(Token::QuotedIdentifier(n)) = &parser.current_token {
                let ext_name = n.clone();
                parser.advance()?;
                ext_name
            } else {
                return Err(ParseError {
                    message: "Expected extension name".to_string(),
                    position: parser.position,
                    expected: vec!["extension name".to_string()],
                    found: parser.current_token.clone(),
                });
            };
            (CommentObjectType::Extension, name, None)
        }
        Some(Token::Function) => {
            parser.advance()?;
            // Parse function name (possibly with signature)
            let name = if let Some(Token::Identifier(n)) = &parser.current_token {
                let func_name = n.clone();
                parser.advance()?;
                func_name
            } else {
                return Err(ParseError {
                    message: "Expected function name".to_string(),
                    position: parser.position,
                    expected: vec!["function name".to_string()],
                    found: parser.current_token.clone(),
                });
            };
            // Skip optional parameter list for now
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
            (CommentObjectType::Function, name, None)
        }
        Some(Token::Trigger) => {
            parser.advance()?;
            let name = if let Some(Token::Identifier(n)) = &parser.current_token {
                let trigger_name = n.clone();
                parser.advance()?;
                trigger_name
            } else {
                return Err(ParseError {
                    message: "Expected trigger name".to_string(),
                    position: parser.position,
                    expected: vec!["trigger name".to_string()],
                    found: parser.current_token.clone(),
                });
            };
            // Skip ON table_name
            if parser.matches(&[Token::On]) {
                parser.advance()?;
                let _ = utilities::parse_table_name(parser)?;
            }
            (CommentObjectType::Trigger, name, None)
        }
        Some(Token::Constraint) => {
            parser.advance()?;
            let name = if let Some(Token::Identifier(n)) = &parser.current_token {
                let constraint_name = n.clone();
                parser.advance()?;
                constraint_name
            } else {
                return Err(ParseError {
                    message: "Expected constraint name".to_string(),
                    position: parser.position,
                    expected: vec!["constraint name".to_string()],
                    found: parser.current_token.clone(),
                });
            };
            // Skip ON table_name if present
            if parser.matches(&[Token::On]) {
                parser.advance()?;
                let _ = utilities::parse_table_name(parser)?;
            }
            (CommentObjectType::Constraint, name, None)
        }
        Some(Token::Database) => {
            parser.advance()?;
            let name = if let Some(Token::Identifier(n)) = &parser.current_token {
                let db_name = n.clone();
                parser.advance()?;
                db_name
            } else {
                return Err(ParseError {
                    message: "Expected database name".to_string(),
                    position: parser.position,
                    expected: vec!["database name".to_string()],
                    found: parser.current_token.clone(),
                });
            };
            (CommentObjectType::Database, name, None)
        }
        _ => {
            return Err(ParseError {
                message: "Expected object type (TABLE, COLUMN, INDEX, etc.)".to_string(),
                position: parser.position,
                expected: vec![
                    "TABLE".to_string(),
                    "COLUMN".to_string(),
                    "INDEX".to_string(),
                    "VIEW".to_string(),
                    "SCHEMA".to_string(),
                    "FUNCTION".to_string(),
                    "TRIGGER".to_string(),
                ],
                found: parser.current_token.clone(),
            });
        }
    };

    // Parse IS 'comment' or IS NULL
    parser.expect(Token::Is)?;

    let comment = if parser.matches(&[Token::Null]) {
        parser.advance()?;
        None
    } else if let Some(Token::StringLiteral(s)) = &parser.current_token {
        let c = s.clone();
        parser.advance()?;
        Some(c)
    } else {
        return Err(ParseError {
            message: "Expected string literal or NULL after IS".to_string(),
            position: parser.position,
            expected: vec!["string literal".to_string(), "NULL".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::CommentOn(CommentOnStatement {
        object_type,
        object_name,
        column_name,
        comment,
    }))
}

// ===== SEQUENCE Statements =====

/// Parse CREATE SEQUENCE statement
/// Syntax: CREATE SEQUENCE [IF NOT EXISTS] name [options]
pub fn parse_create_sequence(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Sequence)?;

    // Check for IF NOT EXISTS
    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse sequence name
    let name = utilities::parse_table_name(parser)?;

    // Parse sequence options
    let options = parse_sequence_options(parser)?;

    Ok(Statement::CreateSequence(CreateSequenceStatement {
        if_not_exists,
        name,
        options,
    }))
}

/// Parse ALTER SEQUENCE statement
/// Syntax: ALTER SEQUENCE [IF EXISTS] name [options]
pub fn parse_alter_sequence(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Sequence)?;

    // Check for IF EXISTS
    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse sequence name
    let name = utilities::parse_table_name(parser)?;

    // Parse sequence options
    let options = parse_sequence_options(parser)?;

    Ok(Statement::AlterSequence(AlterSequenceStatement {
        if_exists,
        name,
        options,
    }))
}

/// Parse DROP SEQUENCE statement
/// Syntax: DROP SEQUENCE [IF EXISTS] name [, ...] [CASCADE | RESTRICT]
pub fn parse_drop_sequence(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Sequence)?;

    // Check for IF EXISTS
    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse sequence names (comma-separated list)
    let mut names = vec![utilities::parse_table_name(parser)?];
    while parser.matches(&[Token::Comma]) {
        parser.advance()?;
        names.push(utilities::parse_table_name(parser)?);
    }

    // Check for CASCADE or RESTRICT
    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else if parser.matches(&[Token::Restrict]) {
        parser.advance()?;
        false
    } else {
        false
    };

    Ok(Statement::DropSequence(DropSequenceStatement {
        if_exists,
        names,
        cascade,
    }))
}

/// Parse sequence options for CREATE/ALTER SEQUENCE
fn parse_sequence_options(parser: &mut SqlParser) -> ParseResult<SequenceOptions> {
    let mut options = SequenceOptions::default();

    loop {
        match &parser.current_token {
            // AS data_type
            Some(Token::As) => {
                parser.advance()?;
                options.data_type = Some(utilities::parse_data_type(parser)?);
            }
            // INCREMENT [BY] value
            Some(Token::Increment) => {
                parser.advance()?;
                if parser.matches(&[Token::By]) {
                    parser.advance()?;
                }
                options.increment = Some(parse_signed_integer(parser)?);
            }
            // MINVALUE value | NO MINVALUE
            Some(Token::MinValue) => {
                parser.advance()?;
                options.min_value = Some(SequenceBound::Value(parse_signed_integer(parser)?));
            }
            Some(Token::No) if parser.peek() == Some(&Token::MinValue) => {
                parser.advance()?; // consume NO
                parser.advance()?; // consume MINVALUE
                options.min_value = Some(SequenceBound::None);
            }
            // MAXVALUE value | NO MAXVALUE
            Some(Token::MaxValue) => {
                parser.advance()?;
                options.max_value = Some(SequenceBound::Value(parse_signed_integer(parser)?));
            }
            Some(Token::No) if parser.peek() == Some(&Token::MaxValue) => {
                parser.advance()?; // consume NO
                parser.advance()?; // consume MAXVALUE
                options.max_value = Some(SequenceBound::None);
            }
            // START [WITH] value
            Some(Token::Start) => {
                parser.advance()?;
                if parser.matches(&[Token::With]) {
                    parser.advance()?;
                }
                options.start = Some(parse_signed_integer(parser)?);
            }
            // CACHE value
            Some(Token::Cache) => {
                parser.advance()?;
                options.cache = Some(parse_signed_integer(parser)?);
            }
            // CYCLE | NO CYCLE
            Some(Token::Cycle) => {
                parser.advance()?;
                options.cycle = Some(true);
            }
            Some(Token::No) if parser.peek() == Some(&Token::Cycle) => {
                parser.advance()?; // consume NO
                parser.advance()?; // consume CYCLE
                options.cycle = Some(false);
            }
            // OWNED BY table.column | OWNED BY NONE
            Some(Token::Owned) => {
                parser.advance()?;
                parser.expect(Token::By)?;
                if parser.matches(&[Token::None]) {
                    parser.advance()?;
                    options.owned_by = Some(SequenceOwner::None);
                } else {
                    let table = utilities::parse_table_name(parser)?;
                    parser.expect(Token::Dot)?;
                    let column = if let Some(Token::Identifier(col)) = &parser.current_token {
                        let c = col.clone();
                        parser.advance()?;
                        c
                    } else {
                        return Err(ParseError {
                            message: "Expected column name after table.".to_string(),
                            position: parser.position,
                            expected: vec!["column_name".to_string()],
                            found: parser.current_token.clone(),
                        });
                    };
                    options.owned_by = Some(SequenceOwner::Column { table, column });
                }
            }
            // RESTART [WITH value] (for ALTER SEQUENCE)
            Some(Token::Restart) => {
                parser.advance()?;
                if parser.matches(&[Token::With]) {
                    parser.advance()?;
                    options.restart = Some(Some(parse_signed_integer(parser)?));
                } else if matches!(
                    &parser.current_token,
                    Some(Token::NumericLiteral(_)) | Some(Token::Minus)
                ) {
                    options.restart = Some(Some(parse_signed_integer(parser)?));
                } else {
                    options.restart = Some(None);
                }
            }
            // End of options
            _ => break,
        }
    }

    Ok(options)
}

/// Parse a signed integer value (for sequence options)
fn parse_signed_integer(parser: &mut SqlParser) -> ParseResult<i64> {
    let negative = if parser.matches(&[Token::Minus]) {
        parser.advance()?;
        true
    } else {
        false
    };

    if let Some(Token::NumericLiteral(n)) = &parser.current_token {
        let value: i64 = n.parse().map_err(|_| ParseError {
            message: format!("Invalid integer value: {}", n),
            position: parser.position,
            expected: vec!["integer".to_string()],
            found: parser.current_token.clone(),
        })?;
        parser.advance()?;
        Ok(if negative { -value } else { value })
    } else {
        Err(ParseError {
            message: "Expected integer value".to_string(),
            position: parser.position,
            expected: vec!["integer".to_string()],
            found: parser.current_token.clone(),
        })
    }
}

// ===== TRUNCATE Statement =====

/// Parse TRUNCATE statement
/// Syntax: TRUNCATE [TABLE] [ONLY] name [, ...] [RESTART IDENTITY | CONTINUE IDENTITY] [CASCADE | RESTRICT]
pub fn parse_truncate(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Truncate)?;

    // Optional TABLE keyword
    if parser.matches(&[Token::Table]) {
        parser.advance()?;
    }

    // Check for ONLY
    let only = if parser.matches(&[Token::Only]) {
        parser.advance()?;
        true
    } else {
        false
    };

    // Parse table names (comma-separated list)
    let mut tables = vec![utilities::parse_table_name(parser)?];
    while parser.matches(&[Token::Comma]) {
        parser.advance()?;
        // Handle ONLY before each table name
        if parser.matches(&[Token::Only]) {
            parser.advance()?;
        }
        tables.push(utilities::parse_table_name(parser)?);
    }

    // Parse RESTART IDENTITY | CONTINUE IDENTITY
    let identity = if parser.matches(&[Token::Restart]) {
        parser.advance()?;
        parser.expect(Token::Identity)?;
        Some(TruncateIdentity::Restart)
    } else if parser.matches(&[Token::Continue]) {
        parser.advance()?;
        parser.expect(Token::Identity)?;
        Some(TruncateIdentity::Continue)
    } else {
        None
    };

    // Parse CASCADE | RESTRICT
    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        Some(true)
    } else if parser.matches(&[Token::Restrict]) {
        parser.advance()?;
        Some(false)
    } else {
        None
    };

    Ok(Statement::Truncate(TruncateStatement {
        tables,
        identity,
        cascade,
        only,
    }))
}
