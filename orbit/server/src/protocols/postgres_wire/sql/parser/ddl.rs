//! DDL (Data Definition Language) Parser Implementation
//!
//! This module handles parsing of CREATE, ALTER, and DROP statements
//! with full support for tables, indexes, views, schemas, and extensions.

use super::{utilities, ParseError, ParseResult, SqlParser};
use crate::protocols::postgres_wire::sql::{
    ast::{
        AlterColumnAction, AlterDomainAction, AlterDomainStatement, AlterPolicyAction,
        AlterPolicyStatement, AlterRoleAction, AlterRoleStatement, AlterSequenceStatement,
        AlterTableAction, AlterTableStatement, AlterTypeAction, AlterTypeStatement,
        ColumnConstraint, ColumnDefinition, CommentObjectType, CommentOnStatement,
        CreateDatabaseStatement, CreateDomainStatement, CreateExtensionStatement,
        CreateFunctionStatement, CreateIndexStatement, CreatePolicyStatement, CreateRoleStatement,
        CreateRuleStatement, CreateSchemaStatement, CreateSequenceStatement, CreateTableStatement,
        CreateTriggerStatement, CreateTypeStatement, CreateViewStatement, DomainConstraint,
        DomainConstraintType, DropDatabaseStatement, DropDomainStatement, DropExtensionStatement,
        DropIndexStatement, DropPolicyStatement, DropRoleStatement, DropRuleStatement,
        DropSchemaStatement, DropSequenceStatement, DropTableStatement, DropTriggerStatement,
        DropTypeStatement, DropViewStatement, EnumValuePosition, FunctionLanguage, FunctionName,
        FunctionParameter, FunctionVolatility, GeneratedColumnStorage, IndexColumn, IndexOption,
        IndexType, NullsOrder, ParameterMode, PolicyCommand, ReferentialAction, RoleOption,
        RuleAction, RuleEvent, SequenceBound, SequenceOptions, SequenceOwner, SortDirection,
        Statement, TableConstraint, TableName, TableOption, TriggerEvent, TriggerForEach,
        TriggerTiming, TruncateIdentity, TruncateStatement, TypeAttribute, TypeDefinition,
        UndropTableStatement,
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

    let mut actions = Vec::new();
    loop {
        let action = match parser.current_token {
            Some(Token::Rename) => {
                parser.advance()?;
                if parser.matches(&[Token::To]) {
                    // RENAME TO (Table rename)
                    parser.advance()?;
                    if let Some(Token::Identifier(new_name)) = &parser.current_token {
                        let name = new_name.clone();
                        parser.advance()?;
                        AlterTableAction::RenameTable(name)
                    } else {
                        return Err(ParseError {
                            message: "Expected new table name after RENAME TO".to_string(),
                            position: parser.position,
                            expected: vec!["table name".to_string()],
                            found: parser.current_token.clone(),
                        });
                    }
                } else if parser.matches(&[Token::Column]) {
                    // RENAME COLUMN
                    parser.advance()?;
                    if let Some(Token::Identifier(old_name)) = &parser.current_token {
                        let old = old_name.clone();
                        parser.advance()?;
                        parser.expect(Token::To)?;
                        if let Some(Token::Identifier(new_name)) = &parser.current_token {
                            let new = new_name.clone();
                            parser.advance()?;
                            AlterTableAction::RenameColumn {
                                old_name: old,
                                new_name: new,
                            }
                        } else {
                            return Err(ParseError {
                                message: "Expected new column name".to_string(),
                                position: parser.position,
                                expected: vec!["column name".to_string()],
                                found: parser.current_token.clone(),
                            });
                        }
                    } else {
                        return Err(ParseError {
                            message: "Expected old column name".to_string(),
                            position: parser.position,
                            expected: vec!["column name".to_string()],
                            found: parser.current_token.clone(),
                        });
                    }
                } else if parser.matches(&[Token::Constraint]) {
                    // RENAME CONSTRAINT is not in AST yet, skipping or treating as todo
                    return Err(ParseError {
                        message: "RENAME CONSTRAINT not supported yet".to_string(),
                        position: parser.position,
                        expected: vec!["COLUMN".to_string(), "TO".to_string()],
                        found: parser.current_token.clone(),
                    });
                } else {
                    return Err(ParseError {
                        message: "Expected COLUMN or TO after RENAME".to_string(),
                        position: parser.position,
                        expected: vec!["COLUMN".to_string(), "TO".to_string()],
                        found: parser.current_token.clone(),
                    });
                }
            }
            Some(Token::Set) => {
                parser.advance()?;
                if parser.matches(&[Token::Schema]) {
                    parser.advance()?;
                    if let Some(Token::Identifier(schema)) = &parser.current_token {
                        let s = schema.clone();
                        parser.advance()?;
                        AlterTableAction::SetSchema(s)
                    } else {
                        return Err(ParseError {
                            message: "Expected schema name".to_string(),
                            position: parser.position,
                            expected: vec!["schema name".to_string()],
                            found: parser.current_token.clone(),
                        });
                    }
                } else if parser.matches(&[Token::Logged]) {
                    parser.advance()?;
                    AlterTableAction::SetLogged
                } else if parser.matches(&[Token::Unlogged]) {
                    parser.advance()?;
                    AlterTableAction::SetUnlogged
                } else if parser.matches(&[Token::Tablespace]) {
                    parser.advance()?;
                    if let Some(Token::Identifier(ts)) = &parser.current_token {
                        let t = ts.clone();
                        parser.advance()?;
                        AlterTableAction::SetTablespace(t)
                    } else {
                        return Err(ParseError {
                            message: "Expected tablespace name".to_string(),
                            position: parser.position,
                            expected: vec!["tablespace name".to_string()],
                            found: parser.current_token.clone(),
                        });
                    }
                } else if parser.matches(&[Token::Without]) {
                    parser.advance()?;
                    parser.expect(Token::Cluster)?;
                    AlterTableAction::SetWithoutCluster
                } else {
                    return Err(ParseError {
                        message: "Expected SCHEMA, LOGGED, UNLOGGED, TABLESPACE, or WITHOUT CLUSTER after SET".to_string(),
                        position: parser.position,
                        expected: vec!["SCHEMA".to_string(), "LOGGED".to_string(), "UNLOGGED".to_string()],
                        found: parser.current_token.clone(),
                    });
                }
            }
            Some(Token::Owner) => {
                parser.advance()?;
                parser.expect(Token::To)?;
                if let Some(Token::Identifier(owner)) = &parser.current_token {
                    let o = owner.clone();
                    parser.advance()?;
                    AlterTableAction::Owner(o)
                } else {
                    return Err(ParseError {
                        message: "Expected owner name".to_string(),
                        position: parser.position,
                        expected: vec!["owner name".to_string()],
                        found: parser.current_token.clone(),
                    });
                }
            }
            Some(Token::Attach) => {
                parser.advance()?;
                parser.expect(Token::Partition)?;
                let partition = utilities::parse_table_name(parser)?;
                // Parse optional FOR VALUES ... (omitted for brevity as AST doesn't support it fully yet, simplifying)
                AlterTableAction::AttachPartition { partition }
            }
            Some(Token::Detach) => {
                parser.advance()?;
                parser.expect(Token::Partition)?;
                let partition = utilities::parse_table_name(parser)?;
                let concurrently = if parser.matches(&[Token::Concurrently]) {
                    parser.advance()?;
                    true
                } else {
                    false
                };
                let finalize = if parser.matches(&[Token::Finalize]) {
                    parser.advance()?;
                    true
                } else {
                    false
                };
                AlterTableAction::DetachPartition {
                    partition,
                    concurrently,
                    finalize,
                }
            }
            Some(Token::Enable) => {
                parser.advance()?;
                if parser.matches(&[Token::Trigger]) {
                    parser.advance()?;
                    if let Some(Token::Identifier(trig)) = &parser.current_token {
                        let t = trig.clone();
                        parser.advance()?;
                        AlterTableAction::EnableTrigger(t)
                    } else if parser.matches(&[Token::All]) {
                        parser.advance()?;
                        AlterTableAction::EnableTrigger("ALL".to_string())
                    } else if parser.matches(&[Token::User]) {
                        parser.advance()?;
                        AlterTableAction::EnableTrigger("USER".to_string())
                    } else {
                        return Err(ParseError {
                            message: "Expected trigger name, ALL, or USER".to_string(),
                            position: parser.position,
                            expected: vec!["trigger name".to_string()],
                            found: parser.current_token.clone(),
                        });
                    }
                } else if parser.matches(&[Token::Always, Token::Trigger]) {
                    parser.advance()?;
                    parser.advance()?;
                    // For simplicity mapping to EnableTrigger with different value or logic,
                    // but adhering to AST. Let's assume just trigger name.
                    return Err(ParseError {
                        message: "ALWAYS TRIGGER not fully supported".to_string(),
                        position: parser.position,
                        expected: vec!["TRIGGER".to_string(), "ROW".to_string()],
                        found: parser.current_token.clone(),
                    });
                } else if parser.matches(&[Token::Row, Token::Level, Token::Security]) {
                    parser.advance()?;
                    parser.advance()?;
                    parser.advance()?;
                    AlterTableAction::EnableRowLevelSecurity
                } else {
                    return Err(ParseError {
                        message: "Expected TRIGGER or ROW LEVEL SECURITY".to_string(),
                        position: parser.position,
                        expected: vec!["TRIGGER".to_string(), "ROW".to_string()],
                        found: parser.current_token.clone(),
                    });
                }
            }
            Some(Token::Disable) => {
                parser.advance()?;
                if parser.matches(&[Token::Trigger]) {
                    parser.advance()?;
                    if let Some(Token::Identifier(trig)) = &parser.current_token {
                        let t = trig.clone();
                        parser.advance()?;
                        AlterTableAction::DisableTrigger(t)
                    } else if parser.matches(&[Token::All]) {
                        parser.advance()?;
                        AlterTableAction::DisableTrigger("ALL".to_string())
                    } else if parser.matches(&[Token::User]) {
                        parser.advance()?;
                        AlterTableAction::DisableTrigger("USER".to_string())
                    } else {
                        return Err(ParseError {
                            message: "Expected trigger name, ALL, or USER".to_string(),
                            position: parser.position,
                            expected: vec!["trigger name".to_string()],
                            found: parser.current_token.clone(),
                        });
                    }
                } else if parser.matches(&[Token::Row, Token::Level, Token::Security]) {
                    parser.advance()?;
                    parser.advance()?;
                    parser.advance()?;
                    AlterTableAction::DisableRowLevelSecurity
                } else {
                    return Err(ParseError {
                        message: "Expected TRIGGER or ROW LEVEL SECURITY".to_string(),
                        position: parser.position,
                        expected: vec!["TRIGGER".to_string(), "ROW".to_string()],
                        found: parser.current_token.clone(),
                    });
                }
            }
            Some(Token::Force) => {
                parser.advance()?;
                parser.expect(Token::Row)?;
                parser.expect(Token::Level)?;
                parser.expect(Token::Security)?;
                AlterTableAction::ForceRowLevelSecurity
            }
            Some(Token::No) => {
                parser.advance()?;
                parser.expect(Token::Force)?;
                parser.expect(Token::Row)?;
                parser.expect(Token::Level)?;
                parser.expect(Token::Security)?;
                AlterTableAction::NoForceRowLevelSecurity
            }
            Some(Token::Cluster) => {
                parser.advance()?;
                parser.expect(Token::On)?;
                if let Some(Token::Identifier(idx)) = &parser.current_token {
                    let i = idx.clone();
                    parser.advance()?;
                    AlterTableAction::ClusterOn(i)
                } else {
                    return Err(ParseError {
                        message: "Expected index name".to_string(),
                        position: parser.position,
                        expected: vec!["index name".to_string()],
                        found: parser.current_token.clone(),
                    });
                }
            }
            Some(Token::Add) => {
                parser.advance()?;
                if parser.matches(&[Token::Column]) {
                    parser.advance()?;
                    let column = parse_column_definition(parser)?;
                    AlterTableAction::AddColumn(column)
                } else if parser.matches(&[Token::Constraint]) {
                    // Constraint parsing not fully wired in AlterTableAction yet, skipping or TODO
                    return Err(ParseError {
                        message: "ADD CONSTRAINT not fully supported yet".to_string(),
                        position: parser.position,
                        expected: vec!["COLUMN".to_string()],
                        found: parser.current_token.clone(),
                    });
                } else {
                    return Err(ParseError {
                        message: "Expected COLUMN after ADD".to_string(),
                        position: parser.position,
                        expected: vec!["COLUMN".to_string()],
                        found: parser.current_token.clone(),
                    });
                }
            }
            Some(Token::Drop) => {
                parser.advance()?;
                if parser.matches(&[Token::Column]) {
                    parser.advance()?;
                    // Check for IF EXISTS
                    let if_exists = if parser.matches(&[Token::If]) {
                        parser.advance()?;
                        parser.expect(Token::Exists)?;
                        true
                    } else {
                        false
                    };
                    if let Some(Token::Identifier(col_name)) = &parser.current_token {
                        let name = col_name.clone();
                        parser.advance()?;
                        let cascade = if parser.matches(&[Token::Cascade]) {
                            parser.advance()?;
                            true
                        } else {
                            false
                        };
                        AlterTableAction::DropColumn {
                            name,
                            if_exists,
                            cascade,
                        }
                    } else {
                        return Err(ParseError {
                            message: "Expected column name".to_string(),
                            position: parser.position,
                            expected: vec!["column name".to_string()],
                            found: parser.current_token.clone(),
                        });
                    }
                } else {
                    return Err(ParseError {
                        message: "Expected COLUMN after DROP".to_string(),
                        position: parser.position,
                        expected: vec!["COLUMN".to_string()],
                        found: parser.current_token.clone(),
                    });
                }
            }
            Some(Token::Alter) => {
                parser.advance()?;
                if parser.matches(&[Token::Column]) {
                    parser.advance()?;
                    if let Some(Token::Identifier(col_name)) = &parser.current_token {
                        let name = col_name.clone();
                        parser.advance()?;

                        // Check for TYPE (Alter Column Type)
                        // MATCH TYPE or Identifier "TYPE" (case insensitive)
                        let is_type = if parser.matches(&[Token::Type]) {
                            true
                        } else if let Some(Token::Identifier(id)) = &parser.current_token {
                            id.eq_ignore_ascii_case("TYPE")
                        } else {
                            false
                        };

                        if is_type {
                            parser.advance()?;
                            let new_type = utilities::parse_data_type(parser)?;
                            // Check for USING
                            let _using = if parser.matches(&[Token::Using]) {
                                parser.advance()?;
                                Some(utilities::parse_expression(parser)?)
                            } else {
                                None
                            };
                            AlterTableAction::AlterColumn {
                                name,
                                action: crate::protocols::postgres_wire::sql::ast::AlterColumnAction::SetType(new_type),
                            }
                        } else if parser.matches(&[Token::Set]) {
                            parser.advance()?;
                            if parser.matches(&[Token::Not]) {
                                parser.advance()?;
                                if parser.matches(&[Token::Null]) {
                                    parser.advance()?;
                                    AlterTableAction::AlterColumn {
                                        name,
                                        action: crate::protocols::postgres_wire::sql::ast::AlterColumnAction::SetNotNull,
                                    }
                                } else {
                                    return Err(ParseError {
                                        message: "Expected NULL after SET NOT".to_string(),
                                        position: parser.position,
                                        expected: vec!["NULL".to_string()],
                                        found: parser.current_token.clone(),
                                    });
                                }
                            } else if parser.matches(&[Token::Default]) {
                                parser.advance()?;
                                let default_expr = utilities::parse_expression(parser)?;
                                AlterTableAction::AlterColumn {
                                    name,
                                    action: crate::protocols::postgres_wire::sql::ast::AlterColumnAction::SetDefault(default_expr),
                                }
                            } else {
                                // Handle generic SET DATA TYPE if needed, but handled above by TYPE check usually
                                return Err(ParseError {
                                    message: "Expected NOT NULL or DEFAULT after SET".to_string(),
                                    position: parser.position,
                                    expected: vec!["NOT NULL".to_string(), "DEFAULT".to_string()],
                                    found: parser.current_token.clone(),
                                });
                            }
                        } else if parser.matches(&[Token::Drop]) {
                            parser.advance()?;
                            if parser.matches(&[Token::Not]) {
                                parser.advance()?;
                                if parser.matches(&[Token::Null]) {
                                    parser.advance()?;
                                    AlterTableAction::AlterColumn {
                                        name,
                                        action: crate::protocols::postgres_wire::sql::ast::AlterColumnAction::DropNotNull,
                                    }
                                } else {
                                    return Err(ParseError {
                                        message: "Expected NULL after DROP NOT".to_string(),
                                        position: parser.position,
                                        expected: vec!["NULL".to_string()],
                                        found: parser.current_token.clone(),
                                    });
                                }
                            } else if parser.matches(&[Token::Default]) {
                                parser.advance()?;
                                AlterTableAction::AlterColumn {
                                    name,
                                    action: crate::protocols::postgres_wire::sql::ast::AlterColumnAction::DropDefault,
                                }
                            } else {
                                return Err(ParseError {
                                    message: "Expected NOT NULL or DEFAULT after DROP".to_string(),
                                    position: parser.position,
                                    expected: vec!["NOT NULL".to_string(), "DEFAULT".to_string()],
                                    found: parser.current_token.clone(),
                                });
                            }
                        } else {
                            // Assuming SET DEFAULT / DROP DEFAULT etc? For now just handle Type as per test failure implies?
                            // Test was modify column... which usually is ALTER COLUMN type?
                            // Standard SQL is ALTER COLUMN ... SET DATA TYPE ... or just type.
                            // Postgres allows ALTER COLUMN ... TYPE ...
                            // Code above handles TYPE.
                            return Err(ParseError {
                                message: "Expected TYPE, SET, or DROP after ALTER COLUMN"
                                    .to_string(),
                                position: parser.position,
                                expected: vec![
                                    "TYPE".to_string(),
                                    "SET".to_string(),
                                    "DROP".to_string(),
                                ],
                                found: parser.current_token.clone(),
                            });
                        }
                    } else {
                        return Err(ParseError {
                            message: "Expected column name".to_string(),
                            position: parser.position,
                            expected: vec!["column name".to_string()],
                            found: parser.current_token.clone(),
                        });
                    }
                } else {
                    return Err(ParseError {
                        message: "Expected COLUMN after ALTER".to_string(),
                        position: parser.position,
                        expected: vec!["COLUMN".to_string()],
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

/// Parse UNDROP TABLE statement
///
/// Syntax: `UNDROP TABLE table_name`
///
/// Restores a previously dropped table using Iceberg snapshot history.
pub fn parse_undrop_table(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Table)?;

    // Parse table name (single table only)
    let name = utilities::parse_table_name(parser)?;

    Ok(Statement::UndropTable(UndropTableStatement { name }))
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
                    "PLJAVASCRIPT" | "JAVASCRIPT" | "PLJS" | "JS" => {
                        Some(FunctionLanguage::PlJavaScript)
                    }
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
        Token::Generated, // PostgreSQL 12+ GENERATED ALWAYS AS
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
            let mut without_overlaps = None;

            while !parser.matches(&[Token::RightParen]) {
                if let Some(Token::Identifier(col_name)) = &parser.current_token {
                    let col = col_name.clone();
                    parser.advance()?;

                    // PostgreSQL 18: Check for WITHOUT OVERLAPS
                    if parser.matches(&[Token::Without]) {
                        parser.advance()?;
                        parser.expect(Token::Overlaps)?;
                        without_overlaps = Some(col.clone());
                    }

                    columns.push(col);

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

            Ok(TableConstraint::PrimaryKey {
                name: constraint_name,
                columns,
                without_overlaps,
            })
        }
        Some(Token::Unique) => {
            parser.advance()?;
            parser.expect(Token::LeftParen)?;

            let mut columns = Vec::new();
            let mut without_overlaps = None;

            while !parser.matches(&[Token::RightParen]) {
                if let Some(Token::Identifier(col_name)) = &parser.current_token {
                    let col = col_name.clone();
                    parser.advance()?;

                    // PostgreSQL 18: Check for WITHOUT OVERLAPS
                    if parser.matches(&[Token::Without]) {
                        parser.advance()?;
                        parser.expect(Token::Overlaps)?;
                        without_overlaps = Some(col.clone());
                    }

                    columns.push(col);

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

            Ok(TableConstraint::Unique {
                name: constraint_name,
                columns,
                without_overlaps,
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

            // Parse column list with PostgreSQL 18 PERIOD support
            let mut columns = Vec::new();
            let mut period_column = None;

            while !parser.matches(&[Token::RightParen]) {
                // PostgreSQL 18: Check for PERIOD keyword for temporal FK
                if parser.matches(&[Token::Period]) {
                    parser.advance()?;
                    if let Some(Token::Identifier(col_name)) = &parser.current_token {
                        period_column = Some(col_name.clone());
                        columns.push(col_name.clone());
                        parser.advance()?;
                    }
                } else if let Some(Token::Identifier(col_name)) = &parser.current_token {
                    columns.push(col_name.clone());
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
            parser.expect(Token::References)?;

            // Parse referenced table
            let references_table = utilities::parse_table_name(parser)?;

            // Parse referenced columns with PostgreSQL 18 PERIOD support
            parser.expect(Token::LeftParen)?;
            let mut references_columns = Vec::new();
            let mut references_period = None;

            while !parser.matches(&[Token::RightParen]) {
                // PostgreSQL 18: Check for PERIOD keyword in referenced columns
                if parser.matches(&[Token::Period]) {
                    parser.advance()?;
                    if let Some(Token::Identifier(col_name)) = &parser.current_token {
                        references_period = Some(col_name.clone());
                        references_columns.push(col_name.clone());
                        parser.advance()?;
                    }
                } else if let Some(Token::Identifier(col_name)) = &parser.current_token {
                    references_columns.push(col_name.clone());
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
                period_column,
                references_period,
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
#[allow(dead_code)]
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

// ===== Extended DDL: TYPE Statements =====

/// Parse CREATE TYPE statement
/// CREATE TYPE name AS ENUM ('value1', 'value2', ...)
/// CREATE TYPE name AS (attr1 type1, attr2 type2, ...)
/// CREATE TYPE name AS RANGE (SUBTYPE = ...)
/// CREATE TYPE name (INPUT = ..., OUTPUT = ...)
/// CREATE TYPE name  -- shell type
pub fn parse_create_type(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Type)?;

    // Check for IF NOT EXISTS (PostgreSQL 9.5+)
    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse type name (may be schema-qualified)
    let name = utilities::parse_table_name(parser)?;

    // Check what kind of type definition follows
    let type_definition = if parser.matches(&[Token::As]) {
        parser.advance()?;

        if parser.matches(&[Token::Enum]) {
            // ENUM type
            parser.advance()?;
            parser.expect(Token::LeftParen)?;

            let mut values = Vec::new();
            loop {
                if let Some(Token::StringLiteral(value)) = &parser.current_token {
                    values.push(value.clone());
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
            TypeDefinition::Enum { values }
        } else if parser.matches(&[Token::LeftParen]) {
            // Composite type: AS (attr1 type1, ...)
            parser.advance()?;

            let mut attributes = Vec::new();
            loop {
                if parser.matches(&[Token::RightParen]) {
                    break;
                }

                // Parse attribute name
                let attr_name = if let Some(Token::Identifier(n)) = &parser.current_token {
                    let name = n.clone();
                    parser.advance()?;
                    name
                } else {
                    return Err(ParseError {
                        message: "Expected attribute name".to_string(),
                        position: parser.position,
                        expected: vec!["attribute_name".to_string()],
                        found: parser.current_token.clone(),
                    });
                };

                // Parse data type
                let data_type = utilities::parse_data_type(parser)?;

                // Optional COLLATE
                let collation = if parser.matches(&[Token::Collation]) {
                    parser.advance()?;
                    if let Some(Token::Identifier(coll)) = &parser.current_token {
                        let c = coll.clone();
                        parser.advance()?;
                        Some(c)
                    } else if let Some(Token::StringLiteral(coll)) = &parser.current_token {
                        let c = coll.clone();
                        parser.advance()?;
                        Some(c)
                    } else {
                        None
                    }
                } else {
                    None
                };

                attributes.push(TypeAttribute {
                    name: attr_name,
                    data_type,
                    collation,
                });

                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }

            parser.expect(Token::RightParen)?;
            TypeDefinition::Composite { attributes }
        } else if parser.matches(&[Token::Range]) {
            // Range type: AS RANGE (SUBTYPE = ...)
            parser.advance()?;
            parser.expect(Token::LeftParen)?;

            // For now, just parse the subtype
            let mut subtype = None;
            while !parser.matches(&[Token::RightParen]) {
                if let Some(Token::Identifier(opt)) = &parser.current_token {
                    if opt.to_uppercase() == "SUBTYPE" {
                        parser.advance()?;
                        parser.expect(Token::Equal)?;
                        subtype = Some(utilities::parse_data_type(parser)?);
                    } else {
                        // Skip other options for now
                        parser.advance()?;
                        if parser.matches(&[Token::Equal]) {
                            parser.advance()?;
                            // Skip the value
                            parser.advance()?;
                        }
                    }
                } else {
                    parser.advance()?;
                }

                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                }
            }

            parser.expect(Token::RightParen)?;

            let subtype = subtype.ok_or_else(|| ParseError {
                message: "RANGE type requires SUBTYPE".to_string(),
                position: parser.position,
                expected: vec!["SUBTYPE".to_string()],
                found: parser.current_token.clone(),
            })?;

            TypeDefinition::Range {
                subtype,
                options: Vec::new(),
            }
        } else {
            return Err(ParseError {
                message: "Expected ENUM, composite definition, or RANGE after AS".to_string(),
                position: parser.position,
                expected: vec!["ENUM".to_string(), "(".to_string(), "RANGE".to_string()],
                found: parser.current_token.clone(),
            });
        }
    } else if parser.matches(&[Token::LeftParen]) {
        // Base type: CREATE TYPE name (INPUT = ..., OUTPUT = ...)
        parser.advance()?;

        // Skip the options for now (complex base type definitions)
        let mut depth = 1;
        while depth > 0 {
            if parser.matches(&[Token::LeftParen]) {
                depth += 1;
            } else if parser.matches(&[Token::RightParen]) {
                depth -= 1;
            }
            if depth > 0 {
                parser.advance()?;
            }
        }
        parser.expect(Token::RightParen)?;

        TypeDefinition::Base {
            options: Vec::new(),
        }
    } else {
        // Shell type: CREATE TYPE name (no definition yet)
        TypeDefinition::Shell
    };

    Ok(Statement::CreateType(CreateTypeStatement {
        if_not_exists,
        name,
        type_definition,
    }))
}

/// Parse DROP TYPE statement
pub fn parse_drop_type(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Type)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse type names (comma-separated)
    let mut names = vec![utilities::parse_table_name(parser)?];
    while parser.matches(&[Token::Comma]) {
        parser.advance()?;
        names.push(utilities::parse_table_name(parser)?);
    }

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        if parser.matches(&[Token::Restrict]) {
            parser.advance()?;
        }
        false
    };

    Ok(Statement::DropType(DropTypeStatement {
        if_exists,
        names,
        cascade,
    }))
}

/// Parse ALTER TYPE statement
pub fn parse_alter_type(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Type)?;

    let name = utilities::parse_table_name(parser)?;

    let action = if parser.matches(&[Token::Add]) {
        parser.advance()?;

        if let Some(Token::Identifier(kw)) = &parser.current_token {
            if kw.to_uppercase() == "VALUE" {
                parser.advance()?;

                let if_not_exists = if parser.matches(&[Token::If]) {
                    parser.advance()?;
                    parser.expect(Token::Not)?;
                    parser.expect(Token::Exists)?;
                    true
                } else {
                    false
                };

                let value = if let Some(Token::StringLiteral(v)) = &parser.current_token {
                    let val = v.clone();
                    parser.advance()?;
                    val
                } else {
                    return Err(ParseError {
                        message: "Expected string value".to_string(),
                        position: parser.position,
                        expected: vec!["'value'".to_string()],
                        found: parser.current_token.clone(),
                    });
                };

                let position = if parser.matches(&[Token::Before]) {
                    parser.advance()?;
                    if let Some(Token::StringLiteral(v)) = &parser.current_token {
                        let pos = v.clone();
                        parser.advance()?;
                        Some(EnumValuePosition::Before(pos))
                    } else {
                        None
                    }
                } else if parser.matches(&[Token::After]) {
                    parser.advance()?;
                    if let Some(Token::StringLiteral(v)) = &parser.current_token {
                        let pos = v.clone();
                        parser.advance()?;
                        Some(EnumValuePosition::After(pos))
                    } else {
                        None
                    }
                } else {
                    None
                };

                AlterTypeAction::AddValue {
                    if_not_exists,
                    value,
                    position,
                }
            } else if kw.to_uppercase() == "ATTRIBUTE" {
                parser.advance()?;
                let attr_name = if let Some(Token::Identifier(n)) = &parser.current_token {
                    let name = n.clone();
                    parser.advance()?;
                    name
                } else {
                    return Err(ParseError {
                        message: "Expected attribute name".to_string(),
                        position: parser.position,
                        expected: vec!["attribute_name".to_string()],
                        found: parser.current_token.clone(),
                    });
                };

                let data_type = utilities::parse_data_type(parser)?;
                AlterTypeAction::AddAttribute {
                    name: attr_name,
                    data_type,
                }
            } else {
                return Err(ParseError {
                    message: "Expected VALUE or ATTRIBUTE after ADD".to_string(),
                    position: parser.position,
                    expected: vec!["VALUE".to_string(), "ATTRIBUTE".to_string()],
                    found: parser.current_token.clone(),
                });
            }
        } else {
            return Err(ParseError {
                message: "Expected VALUE or ATTRIBUTE".to_string(),
                position: parser.position,
                expected: vec!["VALUE".to_string(), "ATTRIBUTE".to_string()],
                found: parser.current_token.clone(),
            });
        }
    } else if parser.matches(&[Token::Drop]) {
        parser.advance()?;

        if let Some(Token::Identifier(kw)) = &parser.current_token {
            if kw.to_uppercase() == "ATTRIBUTE" {
                parser.advance()?;
                let attr_name = if let Some(Token::Identifier(n)) = &parser.current_token {
                    let name = n.clone();
                    parser.advance()?;
                    name
                } else {
                    return Err(ParseError {
                        message: "Expected attribute name".to_string(),
                        position: parser.position,
                        expected: vec!["attribute_name".to_string()],
                        found: parser.current_token.clone(),
                    });
                };

                let cascade = if parser.matches(&[Token::Cascade]) {
                    parser.advance()?;
                    true
                } else {
                    false
                };

                AlterTypeAction::DropAttribute {
                    name: attr_name,
                    cascade,
                }
            } else {
                return Err(ParseError {
                    message: "Expected ATTRIBUTE after DROP".to_string(),
                    position: parser.position,
                    expected: vec!["ATTRIBUTE".to_string()],
                    found: parser.current_token.clone(),
                });
            }
        } else {
            return Err(ParseError {
                message: "Expected ATTRIBUTE".to_string(),
                position: parser.position,
                expected: vec!["ATTRIBUTE".to_string()],
                found: parser.current_token.clone(),
            });
        }
    } else if let Some(Token::Identifier(kw)) = &parser.current_token {
        if kw.to_uppercase() == "RENAME" {
            parser.advance()?;

            if let Some(Token::Identifier(kw2)) = &parser.current_token {
                if kw2.to_uppercase() == "VALUE" {
                    parser.advance()?;
                    let old_value = if let Some(Token::StringLiteral(v)) = &parser.current_token {
                        let val = v.clone();
                        parser.advance()?;
                        val
                    } else {
                        return Err(ParseError {
                            message: "Expected old value".to_string(),
                            position: parser.position,
                            expected: vec!["'old_value'".to_string()],
                            found: parser.current_token.clone(),
                        });
                    };

                    parser.expect(Token::To)?;

                    let new_value = if let Some(Token::StringLiteral(v)) = &parser.current_token {
                        let val = v.clone();
                        parser.advance()?;
                        val
                    } else {
                        return Err(ParseError {
                            message: "Expected new value".to_string(),
                            position: parser.position,
                            expected: vec!["'new_value'".to_string()],
                            found: parser.current_token.clone(),
                        });
                    };

                    AlterTypeAction::RenameValue {
                        old_value,
                        new_value,
                    }
                } else {
                    // RENAME TO new_name
                    parser.expect(Token::To)?;
                    let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
                        let name = n.clone();
                        parser.advance()?;
                        name
                    } else {
                        return Err(ParseError {
                            message: "Expected new name".to_string(),
                            position: parser.position,
                            expected: vec!["new_name".to_string()],
                            found: parser.current_token.clone(),
                        });
                    };
                    AlterTypeAction::Rename(new_name)
                }
            } else if parser.matches(&[Token::To]) {
                parser.advance()?;
                let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
                    let name = n.clone();
                    parser.advance()?;
                    name
                } else {
                    return Err(ParseError {
                        message: "Expected new name".to_string(),
                        position: parser.position,
                        expected: vec!["new_name".to_string()],
                        found: parser.current_token.clone(),
                    });
                };
                AlterTypeAction::Rename(new_name)
            } else {
                return Err(ParseError {
                    message: "Expected VALUE or TO after RENAME".to_string(),
                    position: parser.position,
                    expected: vec!["VALUE".to_string(), "TO".to_string()],
                    found: parser.current_token.clone(),
                });
            }
        } else if kw.to_uppercase() == "OWNER" {
            parser.advance()?;
            parser.expect(Token::To)?;
            let owner = if let Some(Token::Identifier(n)) = &parser.current_token {
                let name = n.clone();
                parser.advance()?;
                name
            } else {
                return Err(ParseError {
                    message: "Expected owner name".to_string(),
                    position: parser.position,
                    expected: vec!["owner_name".to_string()],
                    found: parser.current_token.clone(),
                });
            };
            AlterTypeAction::Owner(owner)
        } else {
            return Err(ParseError {
                message: "Unknown ALTER TYPE action".to_string(),
                position: parser.position,
                expected: vec![
                    "ADD".to_string(),
                    "DROP".to_string(),
                    "RENAME".to_string(),
                    "OWNER".to_string(),
                ],
                found: parser.current_token.clone(),
            });
        }
    } else if parser.matches(&[Token::Set]) {
        parser.advance()?;
        parser.expect(Token::Schema)?;
        let new_schema = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected schema name".to_string(),
                position: parser.position,
                expected: vec!["schema_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterTypeAction::SetSchema(new_schema)
    } else {
        return Err(ParseError {
            message: "Expected ALTER TYPE action".to_string(),
            position: parser.position,
            expected: vec![
                "ADD".to_string(),
                "DROP".to_string(),
                "RENAME".to_string(),
                "SET".to_string(),
                "OWNER".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterType(AlterTypeStatement { name, action }))
}

// ===== Extended DDL: DOMAIN Statements =====

/// Parse CREATE DOMAIN statement
pub fn parse_create_domain(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Domain)?;

    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = utilities::parse_table_name(parser)?;

    parser.expect(Token::As)?;

    let data_type = utilities::parse_data_type(parser)?;

    // Optional COLLATE
    let collation = if parser.matches(&[Token::Collation]) {
        parser.advance()?;
        if let Some(Token::Identifier(coll)) = &parser.current_token {
            let c = coll.clone();
            parser.advance()?;
            Some(c)
        } else if let Some(Token::StringLiteral(coll)) = &parser.current_token {
            let c = coll.clone();
            parser.advance()?;
            Some(c)
        } else {
            None
        }
    } else {
        None
    };

    // Optional DEFAULT
    let default = if parser.matches(&[Token::Default]) {
        parser.advance()?;
        Some(utilities::parse_expression(parser)?)
    } else {
        None
    };

    // Parse constraints
    let mut constraints = Vec::new();
    loop {
        if parser.matches(&[Token::Constraint]) {
            parser.advance()?;
            let constraint_name = if let Some(Token::Identifier(n)) = &parser.current_token {
                let name = n.clone();
                parser.advance()?;
                Some(name)
            } else {
                None
            };

            let constraint_type = parse_domain_constraint_type(parser)?;
            constraints.push(DomainConstraint {
                name: constraint_name,
                constraint_type,
            });
        } else if parser.matches(&[Token::Not]) {
            parser.advance()?;
            parser.expect(Token::Null)?;
            constraints.push(DomainConstraint {
                name: None,
                constraint_type: DomainConstraintType::NotNull,
            });
        } else if parser.matches(&[Token::Null]) {
            parser.advance()?;
            constraints.push(DomainConstraint {
                name: None,
                constraint_type: DomainConstraintType::Null,
            });
        } else if parser.matches(&[Token::Check]) {
            parser.advance()?;
            parser.expect(Token::LeftParen)?;
            let expr = utilities::parse_expression(parser)?;
            parser.expect(Token::RightParen)?;
            constraints.push(DomainConstraint {
                name: None,
                constraint_type: DomainConstraintType::Check(expr),
            });
        } else {
            break;
        }
    }

    Ok(Statement::CreateDomain(CreateDomainStatement {
        if_not_exists,
        name,
        data_type,
        collation,
        default,
        constraints,
    }))
}

fn parse_domain_constraint_type(parser: &mut SqlParser) -> ParseResult<DomainConstraintType> {
    if parser.matches(&[Token::Not]) {
        parser.advance()?;
        parser.expect(Token::Null)?;
        Ok(DomainConstraintType::NotNull)
    } else if parser.matches(&[Token::Null]) {
        parser.advance()?;
        Ok(DomainConstraintType::Null)
    } else if parser.matches(&[Token::Check]) {
        parser.advance()?;
        parser.expect(Token::LeftParen)?;
        let expr = utilities::parse_expression(parser)?;
        parser.expect(Token::RightParen)?;
        Ok(DomainConstraintType::Check(expr))
    } else {
        Err(ParseError {
            message: "Expected constraint type".to_string(),
            position: parser.position,
            expected: vec![
                "NOT NULL".to_string(),
                "NULL".to_string(),
                "CHECK".to_string(),
            ],
            found: parser.current_token.clone(),
        })
    }
}

/// Parse DROP DOMAIN statement
pub fn parse_drop_domain(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Domain)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let mut names = vec![utilities::parse_table_name(parser)?];
    while parser.matches(&[Token::Comma]) {
        parser.advance()?;
        names.push(utilities::parse_table_name(parser)?);
    }

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        if parser.matches(&[Token::Restrict]) {
            parser.advance()?;
        }
        false
    };

    Ok(Statement::DropDomain(DropDomainStatement {
        if_exists,
        names,
        cascade,
    }))
}

/// Parse ALTER DOMAIN statement
pub fn parse_alter_domain(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Domain)?;

    let name = utilities::parse_table_name(parser)?;

    let action = if parser.matches(&[Token::Set]) {
        parser.advance()?;

        if parser.matches(&[Token::Default]) {
            parser.advance()?;
            let expr = utilities::parse_expression(parser)?;
            AlterDomainAction::SetDefault(expr)
        } else if parser.matches(&[Token::Not]) {
            parser.advance()?;
            parser.expect(Token::Null)?;
            AlterDomainAction::SetNotNull
        } else if parser.matches(&[Token::Schema]) {
            parser.advance()?;
            let new_schema = if let Some(Token::Identifier(n)) = &parser.current_token {
                let name = n.clone();
                parser.advance()?;
                name
            } else {
                return Err(ParseError {
                    message: "Expected schema name".to_string(),
                    position: parser.position,
                    expected: vec!["schema_name".to_string()],
                    found: parser.current_token.clone(),
                });
            };
            AlterDomainAction::SetSchema(new_schema)
        } else {
            return Err(ParseError {
                message: "Expected DEFAULT, NOT NULL, or SCHEMA after SET".to_string(),
                position: parser.position,
                expected: vec![
                    "DEFAULT".to_string(),
                    "NOT NULL".to_string(),
                    "SCHEMA".to_string(),
                ],
                found: parser.current_token.clone(),
            });
        }
    } else if parser.matches(&[Token::Drop]) {
        parser.advance()?;

        if parser.matches(&[Token::Default]) {
            parser.advance()?;
            AlterDomainAction::DropDefault
        } else if parser.matches(&[Token::Not]) {
            parser.advance()?;
            parser.expect(Token::Null)?;
            AlterDomainAction::DropNotNull
        } else if parser.matches(&[Token::Constraint]) {
            parser.advance()?;
            let constraint_name = if let Some(Token::Identifier(n)) = &parser.current_token {
                let name = n.clone();
                parser.advance()?;
                name
            } else {
                return Err(ParseError {
                    message: "Expected constraint name".to_string(),
                    position: parser.position,
                    expected: vec!["constraint_name".to_string()],
                    found: parser.current_token.clone(),
                });
            };

            let cascade = if parser.matches(&[Token::Cascade]) {
                parser.advance()?;
                true
            } else {
                false
            };

            AlterDomainAction::DropConstraint {
                name: constraint_name,
                cascade,
            }
        } else {
            return Err(ParseError {
                message: "Expected DEFAULT, NOT NULL, or CONSTRAINT after DROP".to_string(),
                position: parser.position,
                expected: vec![
                    "DEFAULT".to_string(),
                    "NOT NULL".to_string(),
                    "CONSTRAINT".to_string(),
                ],
                found: parser.current_token.clone(),
            });
        }
    } else if parser.matches(&[Token::Add]) {
        parser.advance()?;

        let constraint_name = if parser.matches(&[Token::Constraint]) {
            parser.advance()?;
            if let Some(Token::Identifier(n)) = &parser.current_token {
                let name = n.clone();
                parser.advance()?;
                Some(name)
            } else {
                None
            }
        } else {
            None
        };

        let constraint_type = parse_domain_constraint_type(parser)?;
        AlterDomainAction::AddConstraint(DomainConstraint {
            name: constraint_name,
            constraint_type,
        })
    } else if let Some(Token::Identifier(kw)) = &parser.current_token {
        if kw.to_uppercase() == "RENAME" {
            parser.advance()?;

            if parser.matches(&[Token::Constraint]) {
                parser.advance()?;
                let old_name = if let Some(Token::Identifier(n)) = &parser.current_token {
                    let name = n.clone();
                    parser.advance()?;
                    name
                } else {
                    return Err(ParseError {
                        message: "Expected old constraint name".to_string(),
                        position: parser.position,
                        expected: vec!["old_constraint_name".to_string()],
                        found: parser.current_token.clone(),
                    });
                };

                parser.expect(Token::To)?;

                let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
                    let name = n.clone();
                    parser.advance()?;
                    name
                } else {
                    return Err(ParseError {
                        message: "Expected new constraint name".to_string(),
                        position: parser.position,
                        expected: vec!["new_constraint_name".to_string()],
                        found: parser.current_token.clone(),
                    });
                };

                AlterDomainAction::RenameConstraint { old_name, new_name }
            } else {
                parser.expect(Token::To)?;
                let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
                    let name = n.clone();
                    parser.advance()?;
                    name
                } else {
                    return Err(ParseError {
                        message: "Expected new domain name".to_string(),
                        position: parser.position,
                        expected: vec!["new_name".to_string()],
                        found: parser.current_token.clone(),
                    });
                };
                AlterDomainAction::Rename(new_name)
            }
        } else if kw.to_uppercase() == "OWNER" {
            parser.advance()?;
            parser.expect(Token::To)?;
            let owner = if let Some(Token::Identifier(n)) = &parser.current_token {
                let name = n.clone();
                parser.advance()?;
                name
            } else {
                return Err(ParseError {
                    message: "Expected owner name".to_string(),
                    position: parser.position,
                    expected: vec!["owner_name".to_string()],
                    found: parser.current_token.clone(),
                });
            };
            AlterDomainAction::Owner(owner)
        } else if kw.to_uppercase() == "VALIDATE" {
            parser.advance()?;
            parser.expect(Token::Constraint)?;
            let constraint_name = if let Some(Token::Identifier(n)) = &parser.current_token {
                let name = n.clone();
                parser.advance()?;
                name
            } else {
                return Err(ParseError {
                    message: "Expected constraint name".to_string(),
                    position: parser.position,
                    expected: vec!["constraint_name".to_string()],
                    found: parser.current_token.clone(),
                });
            };
            AlterDomainAction::ValidateConstraint(constraint_name)
        } else {
            return Err(ParseError {
                message: "Unknown ALTER DOMAIN action".to_string(),
                position: parser.position,
                expected: vec![
                    "SET".to_string(),
                    "DROP".to_string(),
                    "ADD".to_string(),
                    "RENAME".to_string(),
                    "OWNER".to_string(),
                    "VALIDATE".to_string(),
                ],
                found: parser.current_token.clone(),
            });
        }
    } else {
        return Err(ParseError {
            message: "Expected ALTER DOMAIN action".to_string(),
            position: parser.position,
            expected: vec![
                "SET".to_string(),
                "DROP".to_string(),
                "ADD".to_string(),
                "RENAME".to_string(),
                "OWNER".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterDomain(AlterDomainStatement {
        name,
        action,
    }))
}

// ===== Extended DDL: ROLE/USER Statements =====

/// Parse CREATE ROLE/USER statement
pub fn parse_create_role(parser: &mut SqlParser, is_user: bool) -> ParseResult<Statement> {
    if is_user {
        parser.expect(Token::User)?;
    } else {
        parser.expect(Token::Role)?;
    }

    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected role/user name".to_string(),
            position: parser.position,
            expected: vec!["role_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional WITH
    if parser.matches(&[Token::With]) {
        parser.advance()?;
    }

    // Parse role options
    let options = parse_role_options(parser)?;

    Ok(Statement::CreateRole(CreateRoleStatement {
        if_not_exists,
        name,
        is_user,
        options,
    }))
}

fn parse_role_options(parser: &mut SqlParser) -> ParseResult<Vec<RoleOption>> {
    let mut options = Vec::new();

    loop {
        if parser.matches(&[Token::SuperUser]) {
            parser.advance()?;
            options.push(RoleOption::SuperUser(true));
        } else if parser.matches(&[Token::NoSuperUser]) {
            parser.advance()?;
            options.push(RoleOption::SuperUser(false));
        } else if parser.matches(&[Token::CreateDb]) {
            parser.advance()?;
            options.push(RoleOption::CreateDb(true));
        } else if parser.matches(&[Token::NoCreateDb]) {
            parser.advance()?;
            options.push(RoleOption::CreateDb(false));
        } else if parser.matches(&[Token::CreateRole]) {
            parser.advance()?;
            options.push(RoleOption::CreateRole(true));
        } else if parser.matches(&[Token::NoCreateRole]) {
            parser.advance()?;
            options.push(RoleOption::CreateRole(false));
        } else if parser.matches(&[Token::Inherit]) {
            parser.advance()?;
            options.push(RoleOption::Inherit(true));
        } else if parser.matches(&[Token::NoInherit]) {
            parser.advance()?;
            options.push(RoleOption::Inherit(false));
        } else if parser.matches(&[Token::Login]) {
            parser.advance()?;
            options.push(RoleOption::Login(true));
        } else if parser.matches(&[Token::NoLogin]) {
            parser.advance()?;
            options.push(RoleOption::Login(false));
        } else if parser.matches(&[Token::Replication]) {
            parser.advance()?;
            options.push(RoleOption::Replication(true));
        } else if parser.matches(&[Token::NoReplication]) {
            parser.advance()?;
            options.push(RoleOption::Replication(false));
        } else if parser.matches(&[Token::BypassRls]) {
            parser.advance()?;
            options.push(RoleOption::BypassRls(true));
        } else if parser.matches(&[Token::NoBypassRls]) {
            parser.advance()?;
            options.push(RoleOption::BypassRls(false));
        } else if parser.matches(&[Token::ConnectionLimit]) {
            parser.advance()?;
            parser.expect(Token::Limit)?;
            if let Some(Token::NumericLiteral(n)) = &parser.current_token {
                let limit = n.parse::<i32>().unwrap_or(-1);
                parser.advance()?;
                options.push(RoleOption::ConnectionLimit(limit));
            }
        } else if parser.matches(&[Token::Password]) {
            parser.advance()?;
            if parser.matches(&[Token::Null]) {
                parser.advance()?;
                options.push(RoleOption::Password(None));
            } else if let Some(Token::StringLiteral(pwd)) = &parser.current_token {
                let password = pwd.clone();
                parser.advance()?;
                options.push(RoleOption::Password(Some(password)));
            }
        } else if parser.matches(&[Token::Encrypted]) {
            parser.advance()?;
            parser.expect(Token::Password)?;
            if let Some(Token::StringLiteral(pwd)) = &parser.current_token {
                let password = pwd.clone();
                parser.advance()?;
                options.push(RoleOption::EncryptedPassword(password));
            }
        } else if parser.matches(&[Token::ValidUntil]) {
            parser.advance()?;
            // Handle "VALID UNTIL" as two keywords
            if let Some(Token::Identifier(kw)) = &parser.current_token {
                if kw.to_uppercase() == "UNTIL" {
                    parser.advance()?;
                }
            }
            if let Some(Token::StringLiteral(ts)) = &parser.current_token {
                let timestamp = ts.clone();
                parser.advance()?;
                options.push(RoleOption::ValidUntil(timestamp));
            }
        } else if parser.matches(&[Token::In]) {
            parser.advance()?;
            parser.expect(Token::Role)?;
            let roles = parse_role_list(parser)?;
            options.push(RoleOption::InRole(roles));
        } else if parser.matches(&[Token::Role]) {
            parser.advance()?;
            let roles = parse_role_list(parser)?;
            options.push(RoleOption::Role(roles));
        } else if let Some(Token::Identifier(kw)) = &parser.current_token {
            if kw.to_uppercase() == "ADMIN" {
                parser.advance()?;
                let roles = parse_role_list(parser)?;
                options.push(RoleOption::Admin(roles));
            } else {
                break;
            }
        } else {
            break;
        }
    }

    Ok(options)
}

fn parse_role_list(parser: &mut SqlParser) -> ParseResult<Vec<String>> {
    let mut roles = Vec::new();

    loop {
        if let Some(Token::Identifier(name)) = &parser.current_token {
            roles.push(name.clone());
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

    Ok(roles)
}

/// Parse DROP ROLE/USER statement
pub fn parse_drop_role(parser: &mut SqlParser, is_user: bool) -> ParseResult<Statement> {
    if is_user {
        parser.expect(Token::User)?;
    } else {
        parser.expect(Token::Role)?;
    }

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let mut names = Vec::new();
    loop {
        if let Some(Token::Identifier(name)) = &parser.current_token {
            names.push(name.clone());
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

    Ok(Statement::DropRole(DropRoleStatement {
        if_exists,
        names,
        is_user,
    }))
}

/// Parse ALTER ROLE/USER statement
pub fn parse_alter_role(parser: &mut SqlParser, is_user: bool) -> ParseResult<Statement> {
    if is_user {
        parser.expect(Token::User)?;
    } else {
        parser.expect(Token::Role)?;
    }

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected role/user name".to_string(),
            position: parser.position,
            expected: vec!["role_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    let action = if let Some(Token::Identifier(kw)) = &parser.current_token {
        if kw.to_uppercase() == "RENAME" {
            parser.advance()?;
            parser.expect(Token::To)?;
            let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
                let name = n.clone();
                parser.advance()?;
                name
            } else {
                return Err(ParseError {
                    message: "Expected new name".to_string(),
                    position: parser.position,
                    expected: vec!["new_name".to_string()],
                    found: parser.current_token.clone(),
                });
            };
            AlterRoleAction::Rename(new_name)
        } else if kw.to_uppercase() == "RESET" {
            parser.advance()?;
            if parser.matches(&[Token::All]) {
                parser.advance()?;
                AlterRoleAction::ResetAllConfig
            } else if let Some(Token::Identifier(param)) = &parser.current_token {
                let parameter = param.clone();
                parser.advance()?;
                AlterRoleAction::ResetConfig(parameter)
            } else {
                return Err(ParseError {
                    message: "Expected ALL or parameter name".to_string(),
                    position: parser.position,
                    expected: vec!["ALL".to_string(), "parameter".to_string()],
                    found: parser.current_token.clone(),
                });
            }
        } else {
            // Assume it's role options
            let options = parse_role_options(parser)?;
            AlterRoleAction::SetOptions(options)
        }
    } else if parser.matches(&[Token::With]) {
        parser.advance()?;
        let options = parse_role_options(parser)?;
        AlterRoleAction::SetOptions(options)
    } else if parser.matches(&[Token::Set]) {
        parser.advance()?;
        let parameter = if let Some(Token::Identifier(param)) = &parser.current_token {
            let name = param.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected parameter name".to_string(),
                position: parser.position,
                expected: vec!["parameter".to_string()],
                found: parser.current_token.clone(),
            });
        };

        // TO or = or FROM CURRENT
        if parser.matches(&[Token::To]) || parser.matches(&[Token::Equal]) {
            parser.advance()?;
        }

        let value = utilities::parse_expression(parser)?;
        AlterRoleAction::SetConfig { parameter, value }
    } else {
        // Try parsing role options directly
        let options = parse_role_options(parser)?;
        if options.is_empty() {
            return Err(ParseError {
                message: "Expected ALTER ROLE action".to_string(),
                position: parser.position,
                expected: vec![
                    "WITH".to_string(),
                    "RENAME".to_string(),
                    "SET".to_string(),
                    "RESET".to_string(),
                    "role_option".to_string(),
                ],
                found: parser.current_token.clone(),
            });
        }
        AlterRoleAction::SetOptions(options)
    };

    Ok(Statement::AlterRole(AlterRoleStatement {
        name,
        is_user,
        action,
    }))
}

// ===== Extended DDL: POLICY Statements =====

/// Parse CREATE POLICY statement
pub fn parse_create_policy(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Policy)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected policy name".to_string(),
            position: parser.position,
            expected: vec!["policy_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    parser.expect(Token::On)?;

    let table = utilities::parse_table_name(parser)?;

    // AS PERMISSIVE | RESTRICTIVE (default PERMISSIVE)
    let permissive = if parser.matches(&[Token::As]) {
        parser.advance()?;
        if parser.matches(&[Token::Permissive]) {
            parser.advance()?;
            true
        } else if parser.matches(&[Token::Restrictive]) {
            parser.advance()?;
            false
        } else {
            true
        }
    } else {
        true
    };

    // FOR command (default ALL)
    let command = if parser.matches(&[Token::For]) {
        parser.advance()?;
        if parser.matches(&[Token::All]) {
            parser.advance()?;
            PolicyCommand::All
        } else if parser.matches(&[Token::Select]) {
            parser.advance()?;
            PolicyCommand::Select
        } else if parser.matches(&[Token::Insert]) {
            parser.advance()?;
            PolicyCommand::Insert
        } else if parser.matches(&[Token::Update]) {
            parser.advance()?;
            PolicyCommand::Update
        } else if parser.matches(&[Token::Delete]) {
            parser.advance()?;
            PolicyCommand::Delete
        } else {
            PolicyCommand::All
        }
    } else {
        PolicyCommand::All
    };

    // TO roles
    let roles = if parser.matches(&[Token::To]) {
        parser.advance()?;
        let mut roles = Vec::new();
        loop {
            if parser.matches(&[Token::Public]) {
                roles.push("PUBLIC".to_string());
                parser.advance()?;
            } else if let Some(Token::Identifier(role)) = &parser.current_token {
                roles.push(role.clone());
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
        roles
    } else {
        vec!["PUBLIC".to_string()]
    };

    // USING expression
    let using_expr = if parser.matches(&[Token::Using]) {
        parser.advance()?;
        parser.expect(Token::LeftParen)?;
        let expr = utilities::parse_expression(parser)?;
        parser.expect(Token::RightParen)?;
        Some(expr)
    } else {
        None
    };

    // WITH CHECK expression
    let check_expr = if parser.matches(&[Token::With]) {
        parser.advance()?;
        parser.expect(Token::Check)?;
        parser.expect(Token::LeftParen)?;
        let expr = utilities::parse_expression(parser)?;
        parser.expect(Token::RightParen)?;
        Some(expr)
    } else {
        None
    };

    Ok(Statement::CreatePolicy(CreatePolicyStatement {
        name,
        table,
        permissive,
        command,
        roles,
        using_expr,
        check_expr,
    }))
}

/// Parse DROP POLICY statement
pub fn parse_drop_policy(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Policy)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected policy name".to_string(),
            position: parser.position,
            expected: vec!["policy_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    parser.expect(Token::On)?;

    let table = utilities::parse_table_name(parser)?;

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropPolicy(DropPolicyStatement {
        if_exists,
        name,
        table,
        cascade,
    }))
}

/// Parse ALTER POLICY statement
pub fn parse_alter_policy(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Policy)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected policy name".to_string(),
            position: parser.position,
            expected: vec!["policy_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    parser.expect(Token::On)?;

    let table = utilities::parse_table_name(parser)?;

    let action = if let Some(Token::Identifier(kw)) = &parser.current_token {
        if kw.to_uppercase() == "RENAME" {
            parser.advance()?;
            parser.expect(Token::To)?;
            let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
                let name = n.clone();
                parser.advance()?;
                name
            } else {
                return Err(ParseError {
                    message: "Expected new policy name".to_string(),
                    position: parser.position,
                    expected: vec!["new_name".to_string()],
                    found: parser.current_token.clone(),
                });
            };
            AlterPolicyAction::Rename(new_name)
        } else {
            return Err(ParseError {
                message: "Expected RENAME, TO, USING, or WITH CHECK".to_string(),
                position: parser.position,
                expected: vec!["RENAME".to_string(), "TO".to_string(), "USING".to_string()],
                found: parser.current_token.clone(),
            });
        }
    } else if parser.matches(&[Token::To]) {
        parser.advance()?;
        let mut roles = Vec::new();
        loop {
            if parser.matches(&[Token::Public]) {
                roles.push("PUBLIC".to_string());
                parser.advance()?;
            } else if let Some(Token::Identifier(role)) = &parser.current_token {
                roles.push(role.clone());
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
        AlterPolicyAction::SetRoles(roles)
    } else if parser.matches(&[Token::Using]) {
        parser.advance()?;
        parser.expect(Token::LeftParen)?;
        let expr = utilities::parse_expression(parser)?;
        parser.expect(Token::RightParen)?;
        AlterPolicyAction::SetUsing(Some(expr))
    } else if parser.matches(&[Token::With]) {
        parser.advance()?;
        parser.expect(Token::Check)?;
        parser.expect(Token::LeftParen)?;
        let expr = utilities::parse_expression(parser)?;
        parser.expect(Token::RightParen)?;
        AlterPolicyAction::SetCheck(Some(expr))
    } else {
        return Err(ParseError {
            message: "Expected ALTER POLICY action".to_string(),
            position: parser.position,
            expected: vec![
                "RENAME".to_string(),
                "TO".to_string(),
                "USING".to_string(),
                "WITH CHECK".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterPolicy(AlterPolicyStatement {
        name,
        table,
        action,
    }))
}

// ===== Extended DDL: RULE Statements =====

/// Parse CREATE RULE statement
pub fn parse_create_rule(parser: &mut SqlParser, or_replace: bool) -> ParseResult<Statement> {
    parser.expect(Token::Rule)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected rule name".to_string(),
            position: parser.position,
            expected: vec!["rule_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    parser.expect(Token::As)?;
    parser.expect(Token::On)?;

    // Parse event (SELECT, INSERT, UPDATE, DELETE)
    let event = if parser.matches(&[Token::Select]) {
        parser.advance()?;
        RuleEvent::Select
    } else if parser.matches(&[Token::Insert]) {
        parser.advance()?;
        RuleEvent::Insert
    } else if parser.matches(&[Token::Update]) {
        parser.advance()?;
        RuleEvent::Update
    } else if parser.matches(&[Token::Delete]) {
        parser.advance()?;
        RuleEvent::Delete
    } else {
        return Err(ParseError {
            message: "Expected SELECT, INSERT, UPDATE, or DELETE".to_string(),
            position: parser.position,
            expected: vec![
                "SELECT".to_string(),
                "INSERT".to_string(),
                "UPDATE".to_string(),
                "DELETE".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    };

    parser.expect(Token::To)?;

    let table = utilities::parse_table_name(parser)?;

    // Optional WHERE clause
    let where_clause = if parser.matches(&[Token::Where]) {
        parser.advance()?;
        Some(utilities::parse_expression(parser)?)
    } else {
        None
    };

    parser.expect(Token::Do)?;

    // Parse action: ALSO | INSTEAD | NOTHING
    let action = if parser.matches(&[Token::Nothing]) {
        parser.advance()?;
        RuleAction::Nothing
    } else if parser.matches(&[Token::Instead]) {
        parser.advance()?;
        if parser.matches(&[Token::Nothing]) {
            parser.advance()?;
            RuleAction::Nothing
        } else {
            // Parse statements (simplified - just skip for now)
            RuleAction::Instead(Vec::new())
        }
    } else if let Some(Token::Identifier(kw)) = &parser.current_token {
        if kw.to_uppercase() == "ALSO" {
            parser.advance()?;
            // Parse statements (simplified - just skip for now)
            RuleAction::Also(Vec::new())
        } else {
            // Default to ALSO with single statement
            RuleAction::Also(Vec::new())
        }
    } else {
        RuleAction::Also(Vec::new())
    };

    Ok(Statement::CreateRule(CreateRuleStatement {
        or_replace,
        name,
        table,
        event,
        where_clause,
        action,
    }))
}

/// Parse DROP RULE statement
pub fn parse_drop_rule(parser: &mut SqlParser) -> ParseResult<Statement> {
    parser.expect(Token::Rule)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected rule name".to_string(),
            position: parser.position,
            expected: vec!["rule_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    parser.expect(Token::On)?;

    let table = utilities::parse_table_name(parser)?;

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropRule(DropRuleStatement {
        if_exists,
        name,
        table,
        cascade,
    }))
}

// ============================================================================
// GROUP statements
// ============================================================================

/// Parse CREATE GROUP statement
pub fn parse_create_group(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::CreateGroupStatement;

    parser.expect(Token::Group)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected group name".to_string(),
            position: parser.position,
            expected: vec!["group_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional WITH options
    let mut with_options = Vec::new();
    if parser.matches(&[Token::With]) {
        parser.advance()?;
        // Parse role options
        while let Some(Token::Identifier(opt)) = &parser.current_token {
            with_options.push(opt.clone());
            parser.advance()?;
            if !parser.matches(&[Token::Comma]) {
                break;
            }
            parser.advance()?;
        }
    }

    Ok(Statement::CreateGroup(CreateGroupStatement {
        name,
        with_options,
    }))
}

/// Parse DROP GROUP statement
pub fn parse_drop_group(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropGroupStatement;

    parser.expect(Token::Group)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let mut names = Vec::new();
    loop {
        if let Some(Token::Identifier(n)) = &parser.current_token {
            names.push(n.clone());
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

    Ok(Statement::DropGroup(DropGroupStatement {
        if_exists,
        names,
    }))
}

/// Parse ALTER GROUP statement
pub fn parse_alter_group(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{AlterGroupAction, AlterGroupStatement};

    parser.expect(Token::Group)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected group name".to_string(),
            position: parser.position,
            expected: vec!["group_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse action: ADD USER | DROP USER | RENAME TO
    let action = if parser.matches(&[Token::Add]) {
        parser.advance()?;
        parser.expect(Token::User)?;
        let mut users = Vec::new();
        loop {
            if let Some(Token::Identifier(u)) = &parser.current_token {
                users.push(u.clone());
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
        AlterGroupAction::AddUser(users)
    } else if parser.matches(&[Token::Drop]) {
        parser.advance()?;
        parser.expect(Token::User)?;
        let mut users = Vec::new();
        loop {
            if let Some(Token::Identifier(u)) = &parser.current_token {
                users.push(u.clone());
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
        AlterGroupAction::DropUser(users)
    } else if parser.matches(&[Token::Rename]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected new group name".to_string(),
                position: parser.position,
                expected: vec!["new_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterGroupAction::Rename(new_name)
    } else {
        return Err(ParseError {
            message: "Expected ADD, DROP, or RENAME".to_string(),
            position: parser.position,
            expected: vec!["ADD".to_string(), "DROP".to_string(), "RENAME".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterGroup(AlterGroupStatement { name, action }))
}

// ============================================================================
// TABLESPACE statements
// ============================================================================

/// Parse CREATE TABLESPACE statement
pub fn parse_create_tablespace(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::CreateTablespaceStatement;

    parser.expect(Token::Tablespace)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected tablespace name".to_string(),
            position: parser.position,
            expected: vec!["tablespace_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional OWNER
    let owner = if parser.matches(&[Token::Owner]) {
        parser.advance()?;
        if let Some(Token::Identifier(o)) = &parser.current_token {
            let owner = Some(o.clone());
            parser.advance()?;
            owner
        } else {
            None
        }
    } else {
        None
    };

    // Parse LOCATION
    parser.expect(Token::Location)?;
    let location = if let Some(Token::StringLiteral(loc)) = &parser.current_token {
        let location = loc.clone();
        parser.advance()?;
        location
    } else {
        return Err(ParseError {
            message: "Expected location string".to_string(),
            position: parser.position,
            expected: vec!["'location'".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional WITH options
    let mut options = Vec::new();
    if parser.matches(&[Token::With]) {
        parser.advance()?;
        parser.expect(Token::LeftParen)?;
        loop {
            if let Some(Token::Identifier(key)) = &parser.current_token {
                let key = key.clone();
                parser.advance()?;
                parser.expect(Token::Equal)?;
                let value = if let Some(Token::StringLiteral(v)) = &parser.current_token {
                    v.clone()
                } else if let Some(Token::Identifier(v)) = &parser.current_token {
                    v.clone()
                } else if let Some(Token::NumericLiteral(v)) = &parser.current_token {
                    v.clone()
                } else {
                    String::new()
                };
                parser.advance()?;
                options.push((key, value));
            }
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
    }

    Ok(Statement::CreateTablespace(CreateTablespaceStatement {
        if_not_exists: false,
        name,
        owner,
        location,
        options,
    }))
}

/// Parse DROP TABLESPACE statement
pub fn parse_drop_tablespace(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropTablespaceStatement;

    parser.expect(Token::Tablespace)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected tablespace name".to_string(),
            position: parser.position,
            expected: vec!["tablespace_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::DropTablespace(DropTablespaceStatement {
        if_exists,
        name,
    }))
}

/// Parse ALTER TABLESPACE statement
pub fn parse_alter_tablespace(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{
        AlterTablespaceAction, AlterTablespaceStatement,
    };

    parser.expect(Token::Tablespace)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected tablespace name".to_string(),
            position: parser.position,
            expected: vec!["tablespace_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    let action = if parser.matches(&[Token::Rename]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected new tablespace name".to_string(),
                position: parser.position,
                expected: vec!["new_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterTablespaceAction::Rename(new_name)
    } else if parser.matches(&[Token::Owner]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_owner = if let Some(Token::Identifier(o)) = &parser.current_token {
            let owner = o.clone();
            parser.advance()?;
            owner
        } else {
            return Err(ParseError {
                message: "Expected new owner name".to_string(),
                position: parser.position,
                expected: vec!["owner_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterTablespaceAction::Owner(new_owner)
    } else if parser.matches(&[Token::Set]) {
        parser.advance()?;
        parser.expect(Token::LeftParen)?;
        let mut options = Vec::new();
        loop {
            if let Some(Token::Identifier(key)) = &parser.current_token {
                let key = key.clone();
                parser.advance()?;
                parser.expect(Token::Equal)?;
                let value = if let Some(Token::StringLiteral(v)) = &parser.current_token {
                    v.clone()
                } else if let Some(Token::Identifier(v)) = &parser.current_token {
                    v.clone()
                } else if let Some(Token::NumericLiteral(v)) = &parser.current_token {
                    v.clone()
                } else {
                    String::new()
                };
                parser.advance()?;
                options.push((key, value));
            }
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
        AlterTablespaceAction::SetOptions(options)
    } else if parser.matches(&[Token::Reset]) {
        parser.advance()?;
        parser.expect(Token::LeftParen)?;
        let mut options = Vec::new();
        loop {
            if let Some(Token::Identifier(opt)) = &parser.current_token {
                options.push(opt.clone());
                parser.advance()?;
            }
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
        AlterTablespaceAction::ResetOptions(options)
    } else {
        return Err(ParseError {
            message: "Expected RENAME, OWNER, SET, or RESET".to_string(),
            position: parser.position,
            expected: vec![
                "RENAME".to_string(),
                "OWNER".to_string(),
                "SET".to_string(),
                "RESET".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterTablespace(AlterTablespaceStatement {
        name,
        action,
    }))
}

// ============================================================================
// AGGREGATE statements
// ============================================================================

/// Parse CREATE AGGREGATE statement
pub fn parse_create_aggregate(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{AggregateOption, CreateAggregateStatement};
    use crate::protocols::postgres_wire::sql::types::SqlType;

    parser.expect(Token::Aggregate)?;

    let name = utilities::parse_table_name(parser)?;

    // Parse input types
    parser.expect(Token::LeftParen)?;
    let mut input_types = Vec::new();
    while !parser.matches(&[Token::RightParen]) {
        let dt = utilities::parse_data_type(parser)?;
        input_types.push(dt);
        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }
    parser.expect(Token::RightParen)?;

    // Parse aggregate definition
    parser.expect(Token::LeftParen)?;
    let mut sfunc = String::new();
    let mut stype = SqlType::Integer;
    let mut options = Vec::new();

    while !parser.matches(&[Token::RightParen]) {
        if let Some(Token::Identifier(key)) = &parser.current_token {
            let key_upper = key.to_uppercase();
            parser.advance()?;
            parser.expect(Token::Equal)?;

            match key_upper.as_str() {
                "SFUNC" => {
                    if let Some(Token::Identifier(f)) = &parser.current_token {
                        sfunc = f.clone();
                        parser.advance()?;
                    }
                }
                "STYPE" => {
                    stype = utilities::parse_data_type(parser)?;
                }
                "FINALFUNC" => {
                    if let Some(Token::Identifier(f)) = &parser.current_token {
                        options.push(AggregateOption::FinalFunc(f.clone()));
                        parser.advance()?;
                    }
                }
                "INITCOND" => {
                    if let Some(Token::StringLiteral(v)) = &parser.current_token {
                        options.push(AggregateOption::InitCond(v.clone()));
                        parser.advance()?;
                    }
                }
                "COMBINEFUNC" => {
                    if let Some(Token::Identifier(f)) = &parser.current_token {
                        options.push(AggregateOption::CombineFunc(f.clone()));
                        parser.advance()?;
                    }
                }
                "SERIALFUNC" => {
                    if let Some(Token::Identifier(f)) = &parser.current_token {
                        options.push(AggregateOption::SerialFunc(f.clone()));
                        parser.advance()?;
                    }
                }
                "DESERIALFUNC" => {
                    if let Some(Token::Identifier(f)) = &parser.current_token {
                        options.push(AggregateOption::DeserialFunc(f.clone()));
                        parser.advance()?;
                    }
                }
                "MSFUNC" => {
                    if let Some(Token::Identifier(f)) = &parser.current_token {
                        options.push(AggregateOption::MSFunc(f.clone()));
                        parser.advance()?;
                    }
                }
                "MINVFUNC" => {
                    if let Some(Token::Identifier(f)) = &parser.current_token {
                        options.push(AggregateOption::MInvFunc(f.clone()));
                        parser.advance()?;
                    }
                }
                "MSTYPE" => {
                    let t = utilities::parse_data_type(parser)?;
                    options.push(AggregateOption::MSType(t));
                }
                "MSSPACE" => {
                    if let Some(Token::NumericLiteral(n)) = &parser.current_token {
                        if let Ok(size) = n.parse() {
                            options.push(AggregateOption::MSSpace(size));
                        }
                        parser.advance()?;
                    }
                }
                "MFINALFUNC" => {
                    if let Some(Token::Identifier(f)) = &parser.current_token {
                        options.push(AggregateOption::MFinalFunc(f.clone()));
                        parser.advance()?;
                    }
                }
                "SORTOP" => {
                    if let Some(Token::Identifier(op)) = &parser.current_token {
                        options.push(AggregateOption::SortOp(op.clone()));
                        parser.advance()?;
                    }
                }
                "PARALLEL" => {
                    if let Some(Token::Identifier(p)) = &parser.current_token {
                        options.push(AggregateOption::Parallel(p.clone()));
                        parser.advance()?;
                    }
                }
                _ => {
                    parser.advance()?;
                }
            }
        } else {
            parser.advance()?;
        }
        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        }
    }
    parser.expect(Token::RightParen)?;

    Ok(Statement::CreateAggregate(CreateAggregateStatement {
        name,
        args: input_types,
        or_replace: false,
        sfunc,
        stype,
        options,
    }))
}

/// Parse DROP AGGREGATE statement
pub fn parse_drop_aggregate(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropAggregateStatement;

    parser.expect(Token::Aggregate)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = utilities::parse_table_name(parser)?;

    // Parse argument types
    parser.expect(Token::LeftParen)?;
    let mut arg_types = Vec::new();
    while !parser.matches(&[Token::RightParen]) {
        let dt = utilities::parse_data_type(parser)?;
        arg_types.push(dt);
        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }
    parser.expect(Token::RightParen)?;

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropAggregate(DropAggregateStatement {
        if_exists,
        names: vec![(name, arg_types)],
        cascade,
    }))
}

/// Parse ALTER AGGREGATE statement
pub fn parse_alter_aggregate(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{
        AlterAggregateAction, AlterAggregateStatement,
    };

    parser.expect(Token::Aggregate)?;

    let name = utilities::parse_table_name(parser)?;

    // Parse argument types
    parser.expect(Token::LeftParen)?;
    let mut arg_types = Vec::new();
    while !parser.matches(&[Token::RightParen]) {
        let dt = utilities::parse_data_type(parser)?;
        arg_types.push(dt);
        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }
    parser.expect(Token::RightParen)?;

    let action = if parser.matches(&[Token::Rename]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected new aggregate name".to_string(),
                position: parser.position,
                expected: vec!["new_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterAggregateAction::Rename(new_name)
    } else if parser.matches(&[Token::Owner]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_owner = if let Some(Token::Identifier(o)) = &parser.current_token {
            let owner = o.clone();
            parser.advance()?;
            owner
        } else {
            return Err(ParseError {
                message: "Expected new owner name".to_string(),
                position: parser.position,
                expected: vec!["owner_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterAggregateAction::Owner(new_owner)
    } else if parser.matches(&[Token::Set]) {
        parser.advance()?;
        parser.expect(Token::Schema)?;
        let new_schema = if let Some(Token::Identifier(s)) = &parser.current_token {
            let schema = s.clone();
            parser.advance()?;
            schema
        } else {
            return Err(ParseError {
                message: "Expected schema name".to_string(),
                position: parser.position,
                expected: vec!["schema_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterAggregateAction::SetSchema(new_schema)
    } else {
        return Err(ParseError {
            message: "Expected RENAME, OWNER, or SET SCHEMA".to_string(),
            position: parser.position,
            expected: vec!["RENAME".to_string(), "OWNER".to_string(), "SET".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterAggregate(AlterAggregateStatement {
        name,
        args: arg_types,
        action,
    }))
}

// ============================================================================
// OPERATOR statements
// ============================================================================

/// Parse CREATE OPERATOR statement
pub fn parse_create_operator(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{CreateOperatorStatement, OperatorOption};
    use crate::protocols::postgres_wire::sql::types::SqlType;

    parser.expect(Token::Operator)?;

    // Parse operator name (can be symbols)
    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        // Could also be operator symbols
        let mut name = String::new();
        while !parser.matches(&[Token::LeftParen]) {
            if let Some(tok) = &parser.current_token {
                name.push_str(&format!("{:?}", tok));
            }
            parser.advance()?;
        }
        name
    };

    // Parse operator definition
    parser.expect(Token::LeftParen)?;
    let mut procedure = String::new();
    let mut left_type = None;
    let mut right_type = None;
    let mut options = Vec::new();

    while !parser.matches(&[Token::RightParen]) {
        if let Some(Token::Identifier(key)) = &parser.current_token {
            let key_upper = key.to_uppercase();
            parser.advance()?;
            parser.expect(Token::Equal)?;

            match key_upper.as_str() {
                "PROCEDURE" | "FUNCTION" => {
                    if let Some(Token::Identifier(f)) = &parser.current_token {
                        procedure = f.clone();
                        parser.advance()?;
                    }
                }
                "LEFTARG" => {
                    left_type = Some(utilities::parse_data_type(parser)?);
                }
                "RIGHTARG" => {
                    right_type = Some(utilities::parse_data_type(parser)?);
                }
                "COMMUTATOR" => {
                    if let Some(Token::Identifier(op)) = &parser.current_token {
                        options.push(OperatorOption::Commutator(op.clone()));
                        parser.advance()?;
                    }
                }
                "NEGATOR" => {
                    if let Some(Token::Identifier(op)) = &parser.current_token {
                        options.push(OperatorOption::Negator(op.clone()));
                        parser.advance()?;
                    }
                }
                "RESTRICT" => {
                    if let Some(Token::Identifier(f)) = &parser.current_token {
                        options.push(OperatorOption::Restrict(f.clone()));
                        parser.advance()?;
                    }
                }
                "JOIN" => {
                    if let Some(Token::Identifier(f)) = &parser.current_token {
                        options.push(OperatorOption::Join(f.clone()));
                        parser.advance()?;
                    }
                }
                "HASHES" => {
                    options.push(OperatorOption::Hashes);
                }
                "MERGES" => {
                    options.push(OperatorOption::Merges);
                }
                _ => {
                    parser.advance()?;
                }
            }
        } else {
            parser.advance()?;
        }
        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        }
    }
    parser.expect(Token::RightParen)?;

    Ok(Statement::CreateOperator(CreateOperatorStatement {
        name,
        procedure,
        left_type: left_type.or(Some(SqlType::Integer)),
        right_type: right_type.or(Some(SqlType::Integer)),
        options,
    }))
}

/// Parse DROP OPERATOR statement
pub fn parse_drop_operator(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropOperatorStatement;

    parser.expect(Token::Operator)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        String::new()
    };

    // Parse argument types
    parser.expect(Token::LeftParen)?;
    let left_type = if !parser.matches(&[Token::Comma, Token::None]) {
        Some(utilities::parse_data_type(parser)?)
    } else {
        if parser.matches(&[Token::None]) {
            parser.advance()?;
        }
        None
    };
    parser.expect(Token::Comma)?;
    let right_type = if !parser.matches(&[Token::RightParen, Token::None]) {
        Some(utilities::parse_data_type(parser)?)
    } else {
        if parser.matches(&[Token::None]) {
            parser.advance()?;
        }
        None
    };
    parser.expect(Token::RightParen)?;

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropOperator(DropOperatorStatement {
        if_exists,
        operators: vec![(name, left_type, right_type)],
        cascade,
    }))
}

/// Parse ALTER OPERATOR statement
pub fn parse_alter_operator(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{AlterOperatorAction, AlterOperatorStatement};

    parser.expect(Token::Operator)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        String::new()
    };

    // Parse argument types
    parser.expect(Token::LeftParen)?;
    let left_type = if !parser.matches(&[Token::Comma, Token::None]) {
        Some(utilities::parse_data_type(parser)?)
    } else {
        if parser.matches(&[Token::None]) {
            parser.advance()?;
        }
        None
    };
    parser.expect(Token::Comma)?;
    let right_type = if !parser.matches(&[Token::RightParen, Token::None]) {
        Some(utilities::parse_data_type(parser)?)
    } else {
        if parser.matches(&[Token::None]) {
            parser.advance()?;
        }
        None
    };
    parser.expect(Token::RightParen)?;

    let action = if parser.matches(&[Token::Owner]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_owner = if let Some(Token::Identifier(o)) = &parser.current_token {
            let owner = o.clone();
            parser.advance()?;
            owner
        } else {
            return Err(ParseError {
                message: "Expected new owner name".to_string(),
                position: parser.position,
                expected: vec!["owner_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterOperatorAction::Owner(new_owner)
    } else if parser.matches(&[Token::Set]) {
        parser.advance()?;
        if parser.matches(&[Token::Schema]) {
            parser.advance()?;
            let new_schema = if let Some(Token::Identifier(s)) = &parser.current_token {
                let schema = s.clone();
                parser.advance()?;
                schema
            } else {
                return Err(ParseError {
                    message: "Expected schema name".to_string(),
                    position: parser.position,
                    expected: vec!["schema_name".to_string()],
                    found: parser.current_token.clone(),
                });
            };
            AlterOperatorAction::SetSchema(new_schema)
        } else {
            // SET ( ... )
            parser.expect(Token::LeftParen)?;
            let mut opts = Vec::new();
            while !parser.matches(&[Token::RightParen]) {
                if let Some(Token::Identifier(key)) = &parser.current_token {
                    let key = key.clone();
                    parser.advance()?;
                    parser.expect(Token::Equal)?;
                    let value = if let Some(Token::Identifier(v)) = &parser.current_token {
                        v.clone()
                    } else {
                        String::new()
                    };
                    parser.advance()?;
                    opts.push((key, value));
                }
                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }
            parser.expect(Token::RightParen)?;
            // SetOptions doesn't exist in AlterOperatorAction, use SetRestrict as fallback
            return Err(ParseError {
                message: "Expected OWNER or SET SCHEMA for ALTER OPERATOR".to_string(),
                position: parser.position,
                expected: vec!["OWNER".to_string(), "SET SCHEMA".to_string()],
                found: parser.current_token.clone(),
            });
        }
    } else {
        return Err(ParseError {
            message: "Expected OWNER or SET".to_string(),
            position: parser.position,
            expected: vec!["OWNER".to_string(), "SET".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterOperator(AlterOperatorStatement {
        name,
        left_type,
        right_type,
        action,
    }))
}

// ============================================================================
// CAST statements
// ============================================================================

/// Parse CREATE CAST statement
pub fn parse_create_cast(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{CastContext, CreateCastStatement};

    parser.expect(Token::Cast)?;

    // Parse ( source_type AS target_type )
    parser.expect(Token::LeftParen)?;
    let source_type = utilities::parse_data_type(parser)?;
    parser.expect(Token::As)?;
    let target_type = utilities::parse_data_type(parser)?;
    parser.expect(Token::RightParen)?;

    // Parse WITH FUNCTION | WITHOUT FUNCTION | WITH INOUT
    let (function, context) = if parser.matches(&[Token::With]) {
        parser.advance()?;
        if let Some(Token::Identifier(kw)) = &parser.current_token {
            if kw.to_uppercase() == "INOUT" {
                parser.advance()?;
                (None, CastContext::Implicit)
            } else if kw.to_uppercase() == "FUNCTION" {
                parser.advance()?;
                let func_name = utilities::parse_table_name(parser)?;
                // Skip function argument types
                if parser.matches(&[Token::LeftParen]) {
                    let mut depth = 1;
                    parser.advance()?;
                    while depth > 0 {
                        if parser.matches(&[Token::LeftParen]) {
                            depth += 1;
                        } else if parser.matches(&[Token::RightParen]) {
                            depth -= 1;
                        }
                        if depth > 0 {
                            parser.advance()?;
                        }
                    }
                    parser.advance()?;
                }
                (Some(func_name), CastContext::Explicit)
            } else {
                (None, CastContext::Explicit)
            }
        } else {
            (None, CastContext::Explicit)
        }
    } else if parser.matches(&[Token::Without]) {
        parser.advance()?;
        // WITHOUT FUNCTION
        if let Some(Token::Identifier(kw)) = &parser.current_token {
            if kw.to_uppercase() == "FUNCTION" {
                parser.advance()?;
            }
        }
        (None, CastContext::Implicit)
    } else {
        (None, CastContext::Explicit)
    };

    // Parse optional AS ASSIGNMENT | AS IMPLICIT
    let context = if parser.matches(&[Token::As]) {
        parser.advance()?;
        if let Some(Token::Identifier(kw)) = &parser.current_token {
            let ctx = match kw.to_uppercase().as_str() {
                "ASSIGNMENT" => CastContext::Assignment,
                "IMPLICIT" => CastContext::Implicit,
                _ => CastContext::Explicit,
            };
            parser.advance()?;
            ctx
        } else {
            context
        }
    } else {
        context
    };

    Ok(Statement::CreateCast(CreateCastStatement {
        source_type,
        target_type,
        function: function.map(|tn| tn.name),
        context,
    }))
}

/// Parse DROP CAST statement
pub fn parse_drop_cast(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropCastStatement;

    parser.expect(Token::Cast)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse ( source_type AS target_type )
    parser.expect(Token::LeftParen)?;
    let source_type = utilities::parse_data_type(parser)?;
    parser.expect(Token::As)?;
    let target_type = utilities::parse_data_type(parser)?;
    parser.expect(Token::RightParen)?;

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropCast(DropCastStatement {
        if_exists,
        source_type,
        target_type,
        cascade,
    }))
}

// ============================================================================
// COLLATION statements
// ============================================================================

/// Parse CREATE COLLATION statement
pub fn parse_create_collation(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{CollationOptions, CreateCollationStatement};

    parser.expect(Token::Collation)?;

    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = utilities::parse_table_name(parser)?;

    // Parse collation options
    let mut from = None;
    let mut locale = None;
    let mut lc_collate = None;
    let mut lc_ctype = None;
    let mut provider = None;
    let mut deterministic = None;

    if parser.matches(&[Token::From]) {
        parser.advance()?;
        from = Some(utilities::parse_table_name(parser)?);
    } else if parser.matches(&[Token::LeftParen]) {
        parser.advance()?;
        while !parser.matches(&[Token::RightParen]) {
            if let Some(Token::Identifier(key)) = &parser.current_token {
                let key_upper = key.to_uppercase();
                parser.advance()?;
                parser.expect(Token::Equal)?;

                match key_upper.as_str() {
                    "LOCALE" => {
                        if let Some(Token::StringLiteral(v)) = &parser.current_token {
                            locale = Some(v.clone());
                            parser.advance()?;
                        }
                    }
                    "LC_COLLATE" => {
                        if let Some(Token::StringLiteral(v)) = &parser.current_token {
                            lc_collate = Some(v.clone());
                            parser.advance()?;
                        }
                    }
                    "LC_CTYPE" => {
                        if let Some(Token::StringLiteral(v)) = &parser.current_token {
                            lc_ctype = Some(v.clone());
                            parser.advance()?;
                        }
                    }
                    "PROVIDER" => {
                        if let Some(Token::Identifier(v)) = &parser.current_token {
                            provider = Some(v.clone());
                            parser.advance()?;
                        }
                    }
                    "DETERMINISTIC" => {
                        if let Some(Token::BooleanLiteral(true)) = &parser.current_token {
                            deterministic = Some(true);
                            parser.advance()?;
                        } else if let Some(Token::BooleanLiteral(false)) = &parser.current_token {
                            deterministic = Some(false);
                            parser.advance()?;
                        }
                    }
                    _ => {
                        parser.advance()?;
                    }
                }
            }
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
    }

    Ok(Statement::CreateCollation(CreateCollationStatement {
        if_not_exists,
        name,
        options: if let Some(from_name) = from {
            CollationOptions::From(from_name)
        } else {
            CollationOptions::Definition {
                locale,
                lc_collate,
                lc_ctype,
                provider,
                deterministic,
            }
        },
    }))
}

/// Parse DROP COLLATION statement
pub fn parse_drop_collation(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropCollationStatement;

    parser.expect(Token::Collation)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = utilities::parse_table_name(parser)?;

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropCollation(DropCollationStatement {
        if_exists,
        names: vec![name],
        cascade,
    }))
}

/// Parse ALTER COLLATION statement
pub fn parse_alter_collation(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{
        AlterCollationAction, AlterCollationStatement,
    };

    parser.expect(Token::Collation)?;

    let name = utilities::parse_table_name(parser)?;

    let action = if parser.matches(&[Token::Rename]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected new collation name".to_string(),
                position: parser.position,
                expected: vec!["new_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterCollationAction::Rename(new_name)
    } else if parser.matches(&[Token::Owner]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_owner = if let Some(Token::Identifier(o)) = &parser.current_token {
            let owner = o.clone();
            parser.advance()?;
            owner
        } else {
            return Err(ParseError {
                message: "Expected new owner name".to_string(),
                position: parser.position,
                expected: vec!["owner_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterCollationAction::Owner(new_owner)
    } else if parser.matches(&[Token::Set]) {
        parser.advance()?;
        parser.expect(Token::Schema)?;
        let new_schema = if let Some(Token::Identifier(s)) = &parser.current_token {
            let schema = s.clone();
            parser.advance()?;
            schema
        } else {
            return Err(ParseError {
                message: "Expected schema name".to_string(),
                position: parser.position,
                expected: vec!["schema_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterCollationAction::SetSchema(new_schema)
    } else if let Some(Token::Identifier(kw)) = &parser.current_token {
        if kw.to_uppercase() == "REFRESH" {
            parser.advance()?;
            parser.expect(Token::Version)?;
            AlterCollationAction::RefreshVersion
        } else {
            return Err(ParseError {
                message: "Expected RENAME, OWNER, SET SCHEMA, or REFRESH VERSION".to_string(),
                position: parser.position,
                expected: vec![
                    "RENAME".to_string(),
                    "OWNER".to_string(),
                    "SET".to_string(),
                    "REFRESH".to_string(),
                ],
                found: parser.current_token.clone(),
            });
        }
    } else {
        return Err(ParseError {
            message: "Expected RENAME, OWNER, SET SCHEMA, or REFRESH VERSION".to_string(),
            position: parser.position,
            expected: vec![
                "RENAME".to_string(),
                "OWNER".to_string(),
                "SET".to_string(),
                "REFRESH".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterCollation(AlterCollationStatement {
        name,
        action,
    }))
}

// ============================================================================
// CONVERSION statements
// ============================================================================

/// Parse CREATE CONVERSION statement
pub fn parse_create_conversion(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::CreateConversionStatement;

    // Check for DEFAULT keyword
    let is_default = if let Some(Token::Default) = &parser.current_token {
        parser.advance()?;
        true
    } else {
        false
    };

    parser.expect(Token::Conversion)?;

    let name = utilities::parse_table_name(parser)?;

    // Parse FOR source_encoding TO dest_encoding FROM function
    parser.expect(Token::For)?;
    let source_encoding = if let Some(Token::StringLiteral(e)) = &parser.current_token {
        let enc = e.clone();
        parser.advance()?;
        enc
    } else if let Some(Token::Identifier(e)) = &parser.current_token {
        let enc = e.clone();
        parser.advance()?;
        enc
    } else {
        return Err(ParseError {
            message: "Expected source encoding".to_string(),
            position: parser.position,
            expected: vec!["source_encoding".to_string()],
            found: parser.current_token.clone(),
        });
    };

    parser.expect(Token::To)?;
    let dest_encoding = if let Some(Token::StringLiteral(e)) = &parser.current_token {
        let enc = e.clone();
        parser.advance()?;
        enc
    } else if let Some(Token::Identifier(e)) = &parser.current_token {
        let enc = e.clone();
        parser.advance()?;
        enc
    } else {
        return Err(ParseError {
            message: "Expected destination encoding".to_string(),
            position: parser.position,
            expected: vec!["dest_encoding".to_string()],
            found: parser.current_token.clone(),
        });
    };

    parser.expect(Token::From)?;
    let function = utilities::parse_table_name(parser)?;

    Ok(Statement::CreateConversion(CreateConversionStatement {
        name,
        default: is_default,
        source_encoding,
        dest_encoding,
        function: function.name,
    }))
}

/// Parse DROP CONVERSION statement
pub fn parse_drop_conversion(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropConversionStatement;

    parser.expect(Token::Conversion)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = utilities::parse_table_name(parser)?;

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropConversion(DropConversionStatement {
        if_exists,
        names: vec![name],
        cascade,
    }))
}

/// Parse ALTER CONVERSION statement
pub fn parse_alter_conversion(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{
        AlterConversionAction, AlterConversionStatement,
    };

    parser.expect(Token::Conversion)?;

    let name = utilities::parse_table_name(parser)?;

    let action = if parser.matches(&[Token::Rename]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected new conversion name".to_string(),
                position: parser.position,
                expected: vec!["new_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterConversionAction::Rename(new_name)
    } else if parser.matches(&[Token::Owner]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_owner = if let Some(Token::Identifier(o)) = &parser.current_token {
            let owner = o.clone();
            parser.advance()?;
            owner
        } else {
            return Err(ParseError {
                message: "Expected new owner name".to_string(),
                position: parser.position,
                expected: vec!["owner_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterConversionAction::Owner(new_owner)
    } else if parser.matches(&[Token::Set]) {
        parser.advance()?;
        parser.expect(Token::Schema)?;
        let new_schema = if let Some(Token::Identifier(s)) = &parser.current_token {
            let schema = s.clone();
            parser.advance()?;
            schema
        } else {
            return Err(ParseError {
                message: "Expected schema name".to_string(),
                position: parser.position,
                expected: vec!["schema_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterConversionAction::SetSchema(new_schema)
    } else {
        return Err(ParseError {
            message: "Expected RENAME, OWNER, or SET SCHEMA".to_string(),
            position: parser.position,
            expected: vec!["RENAME".to_string(), "OWNER".to_string(), "SET".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterConversion(AlterConversionStatement {
        name,
        action,
    }))
}

// ============================================================================
// FOREIGN DATA WRAPPER statements
// ============================================================================

/// Parse CREATE FOREIGN DATA WRAPPER statement
pub fn parse_create_foreign_data_wrapper(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::CreateForeignDataWrapperStatement;

    // Already past FOREIGN
    parser.expect(Token::Data)?;
    parser.expect(Token::Wrapper)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected wrapper name".to_string(),
            position: parser.position,
            expected: vec!["wrapper_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional HANDLER, VALIDATOR, OPTIONS
    let mut handler = None;
    let mut validator = None;
    let mut options = Vec::new();

    while parser.current_token.is_some() && !parser.matches(&[Token::Semicolon]) {
        if let Some(Token::Identifier(kw)) = &parser.current_token {
            match kw.to_uppercase().as_str() {
                "HANDLER" => {
                    parser.advance()?;
                    if let Some(Token::Identifier(h)) = &parser.current_token {
                        handler = Some(h.clone());
                        parser.advance()?;
                    }
                }
                "VALIDATOR" => {
                    parser.advance()?;
                    if let Some(Token::Identifier(v)) = &parser.current_token {
                        validator = Some(v.clone());
                        parser.advance()?;
                    }
                }
                "NO" => {
                    parser.advance()?;
                    if let Some(Token::Identifier(kw2)) = &parser.current_token {
                        match kw2.to_uppercase().as_str() {
                            "HANDLER" => {
                                parser.advance()?;
                                handler = None;
                            }
                            "VALIDATOR" => {
                                parser.advance()?;
                                validator = None;
                            }
                            _ => parser.advance()?,
                        }
                    }
                }
                _ => break,
            }
        } else if parser.matches(&[Token::Options]) {
            parser.advance()?;
            parser.expect(Token::LeftParen)?;
            while !parser.matches(&[Token::RightParen]) {
                if let Some(Token::Identifier(key)) = &parser.current_token {
                    let key = key.clone();
                    parser.advance()?;
                    let value = if let Some(Token::StringLiteral(v)) = &parser.current_token {
                        let v = v.clone();
                        parser.advance()?;
                        v
                    } else {
                        String::new()
                    };
                    options.push((key, value));
                }
                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }
            parser.expect(Token::RightParen)?;
        } else {
            break;
        }
    }

    Ok(Statement::CreateForeignDataWrapper(
        CreateForeignDataWrapperStatement {
            if_not_exists: false,
            name,
            handler,
            validator,
            options,
        },
    ))
}

/// Parse DROP FOREIGN DATA WRAPPER statement
pub fn parse_drop_foreign_data_wrapper(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropForeignDataWrapperStatement;

    // Already past FOREIGN
    parser.expect(Token::Data)?;
    parser.expect(Token::Wrapper)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected wrapper name".to_string(),
            position: parser.position,
            expected: vec!["wrapper_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropForeignDataWrapper(
        DropForeignDataWrapperStatement {
            if_exists,
            names: vec![name],
            cascade,
        },
    ))
}

/// Parse ALTER FOREIGN DATA WRAPPER statement
pub fn parse_alter_foreign_data_wrapper(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{
        AlterForeignDataWrapperAction, AlterForeignDataWrapperStatement,
    };

    // Already past FOREIGN
    parser.expect(Token::Data)?;
    parser.expect(Token::Wrapper)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected wrapper name".to_string(),
            position: parser.position,
            expected: vec!["wrapper_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    let action = if parser.matches(&[Token::Rename]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected new wrapper name".to_string(),
                position: parser.position,
                expected: vec!["new_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterForeignDataWrapperAction::Rename(new_name)
    } else if parser.matches(&[Token::Owner]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_owner = if let Some(Token::Identifier(o)) = &parser.current_token {
            let owner = o.clone();
            parser.advance()?;
            owner
        } else {
            return Err(ParseError {
                message: "Expected new owner name".to_string(),
                position: parser.position,
                expected: vec!["owner_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterForeignDataWrapperAction::Owner(new_owner)
    } else if let Some(Token::Identifier(kw)) = &parser.current_token {
        match kw.to_uppercase().as_str() {
            "HANDLER" => {
                parser.advance()?;
                if let Some(Token::Identifier(h)) = &parser.current_token {
                    let handler = h.clone();
                    parser.advance()?;
                    AlterForeignDataWrapperAction::SetHandler(Some(handler))
                } else {
                    AlterForeignDataWrapperAction::SetHandler(None)
                }
            }
            "VALIDATOR" => {
                parser.advance()?;
                if let Some(Token::Identifier(v)) = &parser.current_token {
                    let validator = v.clone();
                    parser.advance()?;
                    AlterForeignDataWrapperAction::SetValidator(Some(validator))
                } else {
                    AlterForeignDataWrapperAction::SetValidator(None)
                }
            }
            "NO" => {
                parser.advance()?;
                if let Some(Token::Identifier(kw2)) = &parser.current_token {
                    match kw2.to_uppercase().as_str() {
                        "HANDLER" => {
                            parser.advance()?;
                            AlterForeignDataWrapperAction::SetHandler(None)
                        }
                        "VALIDATOR" => {
                            parser.advance()?;
                            AlterForeignDataWrapperAction::SetValidator(None)
                        }
                        _ => AlterForeignDataWrapperAction::SetHandler(None),
                    }
                } else {
                    AlterForeignDataWrapperAction::SetHandler(None)
                }
            }
            _ => {
                return Err(ParseError {
                    message: "Expected RENAME, OWNER, HANDLER, VALIDATOR, or NO".to_string(),
                    position: parser.position,
                    expected: vec![
                        "RENAME".to_string(),
                        "OWNER".to_string(),
                        "HANDLER".to_string(),
                    ],
                    found: parser.current_token.clone(),
                });
            }
        }
    } else if parser.matches(&[Token::Options]) {
        parser.advance()?;
        parser.expect(Token::LeftParen)?;
        let mut opts = Vec::new();
        while !parser.matches(&[Token::RightParen]) {
            if let Some(Token::Identifier(key)) = &parser.current_token {
                let key = key.clone();
                parser.advance()?;
                let value = if let Some(Token::StringLiteral(v)) = &parser.current_token {
                    let v = v.clone();
                    parser.advance()?;
                    v
                } else {
                    String::new()
                };
                opts.push((key, value));
            }
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
        AlterForeignDataWrapperAction::SetOptions(opts)
    } else {
        return Err(ParseError {
            message: "Expected RENAME, OWNER, HANDLER, VALIDATOR, NO, or OPTIONS".to_string(),
            position: parser.position,
            expected: vec![
                "RENAME".to_string(),
                "OWNER".to_string(),
                "HANDLER".to_string(),
                "OPTIONS".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterForeignDataWrapper(
        AlterForeignDataWrapperStatement { name, action },
    ))
}

// ============================================================================
// FOREIGN TABLE statements
// ============================================================================

/// Parse CREATE FOREIGN TABLE statement
pub fn parse_create_foreign_table(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::CreateForeignTableStatement;

    // Already past FOREIGN
    parser.expect(Token::Table)?;

    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = utilities::parse_table_name(parser)?;

    // Parse column definitions
    parser.expect(Token::LeftParen)?;
    let mut columns = Vec::new();
    while !parser.matches(&[Token::RightParen]) {
        let column = parse_column_definition(parser)?;
        columns.push(column);
        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }
    parser.expect(Token::RightParen)?;

    // Parse SERVER
    parser.expect(Token::Server)?;
    let server = if let Some(Token::Identifier(s)) = &parser.current_token {
        let server = s.clone();
        parser.advance()?;
        server
    } else {
        return Err(ParseError {
            message: "Expected server name".to_string(),
            position: parser.position,
            expected: vec!["server_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional OPTIONS
    let mut options = Vec::new();
    if parser.matches(&[Token::Options]) {
        parser.advance()?;
        parser.expect(Token::LeftParen)?;
        while !parser.matches(&[Token::RightParen]) {
            if let Some(Token::Identifier(key)) = &parser.current_token {
                let key = key.clone();
                parser.advance()?;
                let value = if let Some(Token::StringLiteral(v)) = &parser.current_token {
                    let v = v.clone();
                    parser.advance()?;
                    v
                } else {
                    String::new()
                };
                options.push((key, value));
            }
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
    }

    Ok(Statement::CreateForeignTable(CreateForeignTableStatement {
        if_not_exists,
        name,
        columns,
        server,
        options,
    }))
}

/// Parse DROP FOREIGN TABLE statement
pub fn parse_drop_foreign_table(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropForeignTableStatement;

    // Already past FOREIGN
    parser.expect(Token::Table)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

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

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropForeignTable(DropForeignTableStatement {
        if_exists,
        names,
        cascade,
    }))
}

/// Parse ALTER FOREIGN TABLE statement
pub fn parse_alter_foreign_table(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{
        AlterForeignTableAction, AlterForeignTableStatement,
    };

    // Already past FOREIGN
    parser.expect(Token::Table)?;

    let _if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = utilities::parse_table_name(parser)?;

    let action = if parser.matches(&[Token::Rename]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected new table name".to_string(),
                position: parser.position,
                expected: vec!["new_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterForeignTableAction::Rename(new_name)
    } else if parser.matches(&[Token::Owner]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_owner = if let Some(Token::Identifier(o)) = &parser.current_token {
            let owner = o.clone();
            parser.advance()?;
            owner
        } else {
            return Err(ParseError {
                message: "Expected new owner name".to_string(),
                position: parser.position,
                expected: vec!["owner_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterForeignTableAction::Owner(new_owner)
    } else if parser.matches(&[Token::Set]) {
        parser.advance()?;
        parser.expect(Token::Schema)?;
        let new_schema = if let Some(Token::Identifier(s)) = &parser.current_token {
            let schema = s.clone();
            parser.advance()?;
            schema
        } else {
            return Err(ParseError {
                message: "Expected schema name".to_string(),
                position: parser.position,
                expected: vec!["schema_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterForeignTableAction::SetSchema(new_schema)
    } else if parser.matches(&[Token::Add]) {
        parser.advance()?;
        parser.expect(Token::Column)?;
        let column = parse_column_definition(parser)?;
        AlterForeignTableAction::AddColumn(column)
    } else if parser.matches(&[Token::Drop]) {
        parser.advance()?;
        parser.expect(Token::Column)?;
        let col_name = if let Some(Token::Identifier(c)) = &parser.current_token {
            let name = c.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected column name".to_string(),
                position: parser.position,
                expected: vec!["column_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        let cascade = if parser.matches(&[Token::Cascade]) {
            parser.advance()?;
            true
        } else {
            false
        };
        AlterForeignTableAction::DropColumn {
            name: col_name,
            cascade,
        }
    } else if parser.matches(&[Token::Options]) {
        parser.advance()?;
        parser.expect(Token::LeftParen)?;
        let mut opts = Vec::new();
        while !parser.matches(&[Token::RightParen]) {
            if let Some(Token::Identifier(key)) = &parser.current_token {
                let key = key.clone();
                parser.advance()?;
                let value = if let Some(Token::StringLiteral(v)) = &parser.current_token {
                    let v = v.clone();
                    parser.advance()?;
                    v
                } else {
                    String::new()
                };
                opts.push((key, value));
            }
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
        AlterForeignTableAction::SetOptions(opts)
    } else {
        return Err(ParseError {
            message: "Expected RENAME, OWNER, SET SCHEMA, ADD COLUMN, DROP COLUMN, or OPTIONS"
                .to_string(),
            position: parser.position,
            expected: vec![
                "RENAME".to_string(),
                "OWNER".to_string(),
                "SET".to_string(),
                "ADD".to_string(),
                "DROP".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterForeignTable(AlterForeignTableStatement {
        name,
        action,
    }))
}

// ============================================================================
// SERVER statements
// ============================================================================

/// Parse CREATE SERVER statement
pub fn parse_create_server(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::CreateServerStatement;

    parser.expect(Token::Server)?;

    let _if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected server name".to_string(),
            position: parser.position,
            expected: vec!["server_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional TYPE and VERSION
    let mut server_type = None;
    let mut version = None;

    if parser.matches(&[Token::Type]) {
        parser.advance()?;
        if let Some(Token::StringLiteral(t)) = &parser.current_token {
            server_type = Some(t.clone());
            parser.advance()?;
        }
    }

    if parser.matches(&[Token::Version]) {
        parser.advance()?;
        if let Some(Token::StringLiteral(v)) = &parser.current_token {
            version = Some(v.clone());
            parser.advance()?;
        }
    }

    // Parse FOREIGN DATA WRAPPER
    parser.expect(Token::Foreign)?;
    parser.expect(Token::Data)?;
    parser.expect(Token::Wrapper)?;
    let fdw_name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected foreign data wrapper name".to_string(),
            position: parser.position,
            expected: vec!["fdw_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional OPTIONS
    let mut options = Vec::new();
    if parser.matches(&[Token::Options]) {
        parser.advance()?;
        parser.expect(Token::LeftParen)?;
        while !parser.matches(&[Token::RightParen]) {
            if let Some(Token::Identifier(key)) = &parser.current_token {
                let key = key.clone();
                parser.advance()?;
                let value = if let Some(Token::StringLiteral(v)) = &parser.current_token {
                    let v = v.clone();
                    parser.advance()?;
                    v
                } else {
                    String::new()
                };
                options.push((key, value));
            }
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
    }

    Ok(Statement::CreateServer(CreateServerStatement {
        if_not_exists: false,
        name,
        server_type,
        version,
        foreign_data_wrapper: fdw_name,
        options,
    }))
}

/// Parse DROP SERVER statement
pub fn parse_drop_server(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropServerStatement;

    parser.expect(Token::Server)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let mut names = Vec::new();
    loop {
        if let Some(Token::Identifier(n)) = &parser.current_token {
            names.push(n.clone());
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

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropServer(DropServerStatement {
        if_exists,
        names,
        cascade,
    }))
}

/// Parse ALTER SERVER statement
pub fn parse_alter_server(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{AlterServerAction, AlterServerStatement};

    parser.expect(Token::Server)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected server name".to_string(),
            position: parser.position,
            expected: vec!["server_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    let action = if parser.matches(&[Token::Rename]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected new server name".to_string(),
                position: parser.position,
                expected: vec!["new_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterServerAction::Rename(new_name)
    } else if parser.matches(&[Token::Owner]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_owner = if let Some(Token::Identifier(o)) = &parser.current_token {
            let owner = o.clone();
            parser.advance()?;
            owner
        } else {
            return Err(ParseError {
                message: "Expected new owner name".to_string(),
                position: parser.position,
                expected: vec!["owner_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterServerAction::Owner(new_owner)
    } else if parser.matches(&[Token::Version]) {
        parser.advance()?;
        let version = if let Some(Token::StringLiteral(v)) = &parser.current_token {
            let v = v.clone();
            parser.advance()?;
            Some(v)
        } else {
            None
        };
        AlterServerAction::SetVersion(version)
    } else if parser.matches(&[Token::Options]) {
        parser.advance()?;
        parser.expect(Token::LeftParen)?;
        let mut opts = Vec::new();
        while !parser.matches(&[Token::RightParen]) {
            if let Some(Token::Identifier(key)) = &parser.current_token {
                let key = key.clone();
                parser.advance()?;
                let value = if let Some(Token::StringLiteral(v)) = &parser.current_token {
                    let v = v.clone();
                    parser.advance()?;
                    v
                } else {
                    String::new()
                };
                opts.push((key, value));
            }
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
        AlterServerAction::SetOptions(opts)
    } else {
        return Err(ParseError {
            message: "Expected RENAME, OWNER, VERSION, or OPTIONS".to_string(),
            position: parser.position,
            expected: vec![
                "RENAME".to_string(),
                "OWNER".to_string(),
                "VERSION".to_string(),
                "OPTIONS".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterServer(AlterServerStatement {
        name,
        action,
    }))
}

// ============================================================================
// USER MAPPING statements
// ============================================================================

/// Parse CREATE USER MAPPING statement
pub fn parse_create_user_mapping(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::CreateUserMappingStatement;

    parser.expect(Token::User)?;
    parser.expect(Token::Mapping)?;

    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse FOR user
    parser.expect(Token::For)?;
    let user = if let Some(Token::Identifier(u)) = &parser.current_token {
        let user = u.clone();
        parser.advance()?;
        user
    } else {
        return Err(ParseError {
            message: "Expected user name".to_string(),
            position: parser.position,
            expected: vec!["user_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse SERVER
    parser.expect(Token::Server)?;
    let server = if let Some(Token::Identifier(s)) = &parser.current_token {
        let server = s.clone();
        parser.advance()?;
        server
    } else {
        return Err(ParseError {
            message: "Expected server name".to_string(),
            position: parser.position,
            expected: vec!["server_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional OPTIONS
    let mut options = Vec::new();
    if parser.matches(&[Token::Options]) {
        parser.advance()?;
        parser.expect(Token::LeftParen)?;
        while !parser.matches(&[Token::RightParen]) {
            if let Some(Token::Identifier(key)) = &parser.current_token {
                let key = key.clone();
                parser.advance()?;
                let value = if let Some(Token::StringLiteral(v)) = &parser.current_token {
                    let v = v.clone();
                    parser.advance()?;
                    v
                } else {
                    String::new()
                };
                options.push((key, value));
            }
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
    }

    Ok(Statement::CreateUserMapping(CreateUserMappingStatement {
        if_not_exists,
        user,
        server,
        options,
    }))
}

/// Parse DROP USER MAPPING statement
pub fn parse_drop_user_mapping(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropUserMappingStatement;

    parser.expect(Token::User)?;
    parser.expect(Token::Mapping)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse FOR user
    parser.expect(Token::For)?;
    let user = if let Some(Token::Identifier(u)) = &parser.current_token {
        let user = u.clone();
        parser.advance()?;
        user
    } else {
        return Err(ParseError {
            message: "Expected user name".to_string(),
            position: parser.position,
            expected: vec!["user_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse SERVER
    parser.expect(Token::Server)?;
    let server = if let Some(Token::Identifier(s)) = &parser.current_token {
        let server = s.clone();
        parser.advance()?;
        server
    } else {
        return Err(ParseError {
            message: "Expected server name".to_string(),
            position: parser.position,
            expected: vec!["server_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::DropUserMapping(DropUserMappingStatement {
        if_exists,
        user,
        server,
    }))
}

/// Parse ALTER USER MAPPING statement
pub fn parse_alter_user_mapping(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::AlterUserMappingStatement;

    parser.expect(Token::User)?;
    parser.expect(Token::Mapping)?;

    // Parse FOR user
    parser.expect(Token::For)?;
    let user = if let Some(Token::Identifier(u)) = &parser.current_token {
        let user = u.clone();
        parser.advance()?;
        user
    } else {
        return Err(ParseError {
            message: "Expected user name".to_string(),
            position: parser.position,
            expected: vec!["user_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse SERVER
    parser.expect(Token::Server)?;
    let server = if let Some(Token::Identifier(s)) = &parser.current_token {
        let server = s.clone();
        parser.advance()?;
        server
    } else {
        return Err(ParseError {
            message: "Expected server name".to_string(),
            position: parser.position,
            expected: vec!["server_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse OPTIONS
    parser.expect(Token::Options)?;
    parser.expect(Token::LeftParen)?;
    let mut options = Vec::new();
    while !parser.matches(&[Token::RightParen]) {
        if let Some(Token::Identifier(key)) = &parser.current_token {
            let key = key.clone();
            parser.advance()?;
            let value = if let Some(Token::StringLiteral(v)) = &parser.current_token {
                let v = v.clone();
                parser.advance()?;
                v
            } else {
                String::new()
            };
            options.push((key, value));
        }
        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }
    parser.expect(Token::RightParen)?;

    Ok(Statement::AlterUserMapping(AlterUserMappingStatement {
        user,
        server,
        options,
    }))
}

// ============================================================================
// PUBLICATION statements
// ============================================================================

/// Parse CREATE PUBLICATION statement
pub fn parse_create_publication(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::CreatePublicationStatement;

    parser.expect(Token::Publication)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected publication name".to_string(),
            position: parser.position,
            expected: vec!["publication_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse FOR TABLE or FOR ALL TABLES
    let mut for_all_tables = false;
    let mut tables = Vec::new();

    if parser.matches(&[Token::For]) {
        parser.advance()?;
        if parser.matches(&[Token::All]) {
            parser.advance()?;
            // TABLES is a context-sensitive keyword
            if let Some(Token::Identifier(kw)) = &parser.current_token {
                if kw.to_uppercase() == "TABLES" {
                    parser.advance()?;
                }
            }
            for_all_tables = true;
        } else {
            parser.expect(Token::Table)?;
            loop {
                let table = utilities::parse_table_name(parser)?;
                tables.push(table);
                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }
        }
    }

    // Parse optional WITH options
    let mut options = Vec::new();
    if parser.matches(&[Token::With]) {
        parser.advance()?;
        parser.expect(Token::LeftParen)?;
        while !parser.matches(&[Token::RightParen]) {
            if let Some(Token::Identifier(key)) = &parser.current_token {
                let key = key.clone();
                parser.advance()?;
                parser.expect(Token::Equal)?;
                let value = if let Some(Token::Identifier(v)) = &parser.current_token {
                    let v = v.clone();
                    parser.advance()?;
                    v
                } else {
                    String::new()
                };
                options.push((key, value));
            }
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
    }

    Ok(Statement::CreatePublication(CreatePublicationStatement {
        if_not_exists: false, // CREATE PUBLICATION doesn't support IF NOT EXISTS
        name,
        for_all_tables,
        tables,
        options,
    }))
}

/// Parse DROP PUBLICATION statement
pub fn parse_drop_publication(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropPublicationStatement;

    parser.expect(Token::Publication)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let mut names = Vec::new();
    loop {
        if let Some(Token::Identifier(n)) = &parser.current_token {
            names.push(n.clone());
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

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropPublication(DropPublicationStatement {
        if_exists,
        names,
        cascade,
    }))
}

/// Parse ALTER PUBLICATION statement
pub fn parse_alter_publication(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{
        AlterPublicationAction, AlterPublicationStatement,
    };

    parser.expect(Token::Publication)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected publication name".to_string(),
            position: parser.position,
            expected: vec!["publication_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    let action = if parser.matches(&[Token::Rename]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected new publication name".to_string(),
                position: parser.position,
                expected: vec!["new_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterPublicationAction::Rename(new_name)
    } else if parser.matches(&[Token::Owner]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_owner = if let Some(Token::Identifier(o)) = &parser.current_token {
            let owner = o.clone();
            parser.advance()?;
            owner
        } else {
            return Err(ParseError {
                message: "Expected new owner name".to_string(),
                position: parser.position,
                expected: vec!["owner_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterPublicationAction::Owner(new_owner)
    } else if parser.matches(&[Token::Add]) {
        parser.advance()?;
        parser.expect(Token::Table)?;
        let mut tables = Vec::new();
        loop {
            let table = utilities::parse_table_name(parser)?;
            tables.push(table);
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        AlterPublicationAction::AddTable(tables)
    } else if parser.matches(&[Token::Drop]) {
        parser.advance()?;
        parser.expect(Token::Table)?;
        let mut tables = Vec::new();
        loop {
            let table = utilities::parse_table_name(parser)?;
            tables.push(table);
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        AlterPublicationAction::DropTable(tables)
    } else if parser.matches(&[Token::Set]) {
        parser.advance()?;
        if parser.matches(&[Token::Table]) {
            parser.advance()?;
            let mut tables = Vec::new();
            loop {
                let table = utilities::parse_table_name(parser)?;
                tables.push(table);
                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }
            AlterPublicationAction::SetTable(tables)
        } else {
            // SET ( options )
            parser.expect(Token::LeftParen)?;
            let mut opts = Vec::new();
            while !parser.matches(&[Token::RightParen]) {
                if let Some(Token::Identifier(key)) = &parser.current_token {
                    let key = key.clone();
                    parser.advance()?;
                    parser.expect(Token::Equal)?;
                    let value = if let Some(Token::Identifier(v)) = &parser.current_token {
                        let v = v.clone();
                        parser.advance()?;
                        v
                    } else {
                        String::new()
                    };
                    opts.push((key, value));
                }
                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }
            parser.expect(Token::RightParen)?;
            AlterPublicationAction::SetOptions(opts)
        }
    } else {
        return Err(ParseError {
            message: "Expected RENAME, OWNER, ADD TABLE, DROP TABLE, or SET".to_string(),
            position: parser.position,
            expected: vec![
                "RENAME".to_string(),
                "OWNER".to_string(),
                "ADD".to_string(),
                "DROP".to_string(),
                "SET".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterPublication(AlterPublicationStatement {
        name,
        action,
    }))
}

// ============================================================================
// SUBSCRIPTION statements
// ============================================================================

/// Parse CREATE SUBSCRIPTION statement
pub fn parse_create_subscription(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::CreateSubscriptionStatement;

    parser.expect(Token::Subscription)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected subscription name".to_string(),
            position: parser.position,
            expected: vec!["subscription_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse CONNECTION (context-sensitive keyword)
    if let Some(Token::Identifier(kw)) = &parser.current_token {
        if kw.to_uppercase() != "CONNECTION" {
            return Err(ParseError {
                message: "Expected CONNECTION".to_string(),
                position: parser.position,
                expected: vec!["CONNECTION".to_string()],
                found: parser.current_token.clone(),
            });
        }
        parser.advance()?;
    }
    let connection = if let Some(Token::StringLiteral(c)) = &parser.current_token {
        let conn = c.clone();
        parser.advance()?;
        conn
    } else {
        return Err(ParseError {
            message: "Expected connection string".to_string(),
            position: parser.position,
            expected: vec!["'connection_string'".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse PUBLICATION
    parser.expect(Token::Publication)?;
    let mut publication = Vec::new();
    loop {
        if let Some(Token::Identifier(p)) = &parser.current_token {
            publication.push(p.clone());
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

    // Parse optional WITH options
    let mut options = Vec::new();
    if parser.matches(&[Token::With]) {
        parser.advance()?;
        parser.expect(Token::LeftParen)?;
        while !parser.matches(&[Token::RightParen]) {
            if let Some(Token::Identifier(key)) = &parser.current_token {
                let key = key.clone();
                parser.advance()?;
                parser.expect(Token::Equal)?;
                let value = if let Some(Token::Identifier(v)) = &parser.current_token {
                    let v = v.clone();
                    parser.advance()?;
                    v
                } else if let Some(Token::StringLiteral(v)) = &parser.current_token {
                    let v = v.clone();
                    parser.advance()?;
                    v
                } else {
                    String::new()
                };
                options.push((key, value));
            }
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
    }

    Ok(Statement::CreateSubscription(CreateSubscriptionStatement {
        if_not_exists: false,
        name,
        connection,
        publication,
        options,
    }))
}

/// Parse DROP SUBSCRIPTION statement
pub fn parse_drop_subscription(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropSubscriptionStatement;

    parser.expect(Token::Subscription)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected subscription name".to_string(),
            position: parser.position,
            expected: vec!["subscription_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropSubscription(DropSubscriptionStatement {
        if_exists,
        name,
        cascade,
    }))
}

/// Parse ALTER SUBSCRIPTION statement
pub fn parse_alter_subscription(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{
        AlterSubscriptionAction, AlterSubscriptionStatement,
    };

    parser.expect(Token::Subscription)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected subscription name".to_string(),
            position: parser.position,
            expected: vec!["subscription_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    let action = if parser.matches(&[Token::Rename]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected new subscription name".to_string(),
                position: parser.position,
                expected: vec!["new_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterSubscriptionAction::Rename(new_name)
    } else if parser.matches(&[Token::Owner]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_owner = if let Some(Token::Identifier(o)) = &parser.current_token {
            let owner = o.clone();
            parser.advance()?;
            owner
        } else {
            return Err(ParseError {
                message: "Expected new owner name".to_string(),
                position: parser.position,
                expected: vec!["owner_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterSubscriptionAction::Owner(new_owner)
    } else if let Some(Token::Identifier(kw)) = &parser.current_token {
        if kw.to_uppercase() == "CONNECTION" {
            parser.advance()?;
            let conn = if let Some(Token::StringLiteral(c)) = &parser.current_token {
                let conn = c.clone();
                parser.advance()?;
                conn
            } else {
                return Err(ParseError {
                    message: "Expected connection string".to_string(),
                    position: parser.position,
                    expected: vec!["'connection_string'".to_string()],
                    found: parser.current_token.clone(),
                });
            };
            AlterSubscriptionAction::SetConnection(conn)
        } else {
            return Err(ParseError {
                message: "Expected RENAME, OWNER, CONNECTION, SET, ENABLE, DISABLE, or REFRESH"
                    .to_string(),
                position: parser.position,
                expected: vec![
                    "RENAME".to_string(),
                    "OWNER".to_string(),
                    "CONNECTION".to_string(),
                ],
                found: parser.current_token.clone(),
            });
        }
    } else if parser.matches(&[Token::Set]) {
        parser.advance()?;
        if parser.matches(&[Token::Publication]) {
            parser.advance()?;
            let mut pubs = Vec::new();
            loop {
                if let Some(Token::Identifier(p)) = &parser.current_token {
                    pubs.push(p.clone());
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
            AlterSubscriptionAction::SetPublication(pubs)
        } else {
            // SET ( options )
            parser.expect(Token::LeftParen)?;
            let mut opts = Vec::new();
            while !parser.matches(&[Token::RightParen]) {
                if let Some(Token::Identifier(key)) = &parser.current_token {
                    let key = key.clone();
                    parser.advance()?;
                    parser.expect(Token::Equal)?;
                    let value = if let Some(Token::Identifier(v)) = &parser.current_token {
                        let v = v.clone();
                        parser.advance()?;
                        v
                    } else {
                        String::new()
                    };
                    opts.push((key, value));
                }
                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }
            parser.expect(Token::RightParen)?;
            AlterSubscriptionAction::SetOptions(opts)
        }
    } else if parser.matches(&[Token::Enable]) {
        parser.advance()?;
        AlterSubscriptionAction::Enable
    } else if parser.matches(&[Token::Disable]) {
        parser.advance()?;
        AlterSubscriptionAction::Disable
    } else if parser.matches(&[Token::Refresh]) {
        parser.advance()?;
        parser.expect(Token::Publication)?;
        AlterSubscriptionAction::Refresh
    } else {
        return Err(ParseError {
            message: "Expected RENAME, OWNER, CONNECTION, SET, ENABLE, DISABLE, or REFRESH"
                .to_string(),
            position: parser.position,
            expected: vec![
                "RENAME".to_string(),
                "OWNER".to_string(),
                "CONNECTION".to_string(),
                "SET".to_string(),
                "ENABLE".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterSubscription(AlterSubscriptionStatement {
        name,
        action,
    }))
}

// ============================================================================
// EVENT TRIGGER statements
// ============================================================================

/// Parse CREATE EVENT TRIGGER statement
pub fn parse_create_event_trigger(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::CreateEventTriggerStatement;

    // EVENT is handled as identifier in CREATE EVENT TRIGGER
    parser.expect(Token::Trigger)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected event trigger name".to_string(),
            position: parser.position,
            expected: vec!["trigger_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse ON event
    parser.expect(Token::On)?;
    let event = if let Some(Token::Identifier(e)) = &parser.current_token {
        let event = e.clone();
        parser.advance()?;
        event
    } else {
        return Err(ParseError {
            message: "Expected event name".to_string(),
            position: parser.position,
            expected: vec!["event_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional WHEN clause: WHEN tag_name IN ('tag1', 'tag2', ...)
    let mut when_clause: Option<Vec<(String, Vec<String>)>> = None;
    if parser.matches(&[Token::When]) {
        parser.advance()?;
        let mut clauses = Vec::new();
        // Parse TAG IN ('tag1', 'tag2', ...)
        if let Some(Token::Identifier(kw)) = &parser.current_token {
            let tag_name = kw.clone();
            parser.advance()?;
            parser.expect(Token::In)?;
            parser.expect(Token::LeftParen)?;
            let mut values = Vec::new();
            while !parser.matches(&[Token::RightParen]) {
                if let Some(Token::StringLiteral(t)) = &parser.current_token {
                    values.push(t.clone());
                    parser.advance()?;
                }
                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }
            parser.expect(Token::RightParen)?;
            clauses.push((tag_name, values));
        }
        when_clause = Some(clauses);
    }

    // Parse EXECUTE FUNCTION/PROCEDURE
    parser.expect(Token::Execute)?;
    if let Some(Token::Identifier(kw)) = &parser.current_token {
        if kw.to_uppercase() == "FUNCTION" || kw.to_uppercase() == "PROCEDURE" {
            parser.advance()?;
        }
    } else if parser.matches(&[Token::Function]) {
        parser.advance()?;
    }

    let function_name = utilities::parse_table_name(parser)?;
    let function = function_name.to_string();
    // Skip function arguments ()
    if parser.matches(&[Token::LeftParen]) {
        parser.advance()?;
        parser.expect(Token::RightParen)?;
    }

    Ok(Statement::CreateEventTrigger(CreateEventTriggerStatement {
        name,
        event,
        when_clause,
        function,
    }))
}

/// Parse DROP EVENT TRIGGER statement
pub fn parse_drop_event_trigger(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropEventTriggerStatement;

    // EVENT is handled as identifier in CREATE EVENT TRIGGER
    parser.expect(Token::Trigger)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected event trigger name".to_string(),
            position: parser.position,
            expected: vec!["trigger_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropEventTrigger(DropEventTriggerStatement {
        if_exists,
        name,
        cascade,
    }))
}

/// Parse ALTER EVENT TRIGGER statement
pub fn parse_alter_event_trigger(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{
        AlterEventTriggerAction, AlterEventTriggerStatement,
    };

    // EVENT is handled as identifier in CREATE EVENT TRIGGER
    parser.expect(Token::Trigger)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected event trigger name".to_string(),
            position: parser.position,
            expected: vec!["trigger_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    let action = if parser.matches(&[Token::Rename]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected new trigger name".to_string(),
                position: parser.position,
                expected: vec!["new_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterEventTriggerAction::Rename(new_name)
    } else if parser.matches(&[Token::Owner]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_owner = if let Some(Token::Identifier(o)) = &parser.current_token {
            let owner = o.clone();
            parser.advance()?;
            owner
        } else {
            return Err(ParseError {
                message: "Expected new owner name".to_string(),
                position: parser.position,
                expected: vec!["owner_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterEventTriggerAction::Owner(new_owner)
    } else if parser.matches(&[Token::Enable]) {
        parser.advance()?;
        if let Some(Token::Identifier(kw)) = &parser.current_token {
            match kw.to_uppercase().as_str() {
                "REPLICA" => {
                    parser.advance()?;
                    AlterEventTriggerAction::EnableReplica
                }
                "ALWAYS" => {
                    parser.advance()?;
                    AlterEventTriggerAction::EnableAlways
                }
                _ => AlterEventTriggerAction::Enable,
            }
        } else {
            AlterEventTriggerAction::Enable
        }
    } else if parser.matches(&[Token::Disable]) {
        parser.advance()?;
        AlterEventTriggerAction::Disable
    } else {
        return Err(ParseError {
            message: "Expected RENAME, OWNER, ENABLE, or DISABLE".to_string(),
            position: parser.position,
            expected: vec![
                "RENAME".to_string(),
                "OWNER".to_string(),
                "ENABLE".to_string(),
                "DISABLE".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterEventTrigger(AlterEventTriggerStatement {
        name,
        action,
    }))
}

// ============================================================================
// ACCESS METHOD statements
// ============================================================================

/// Parse CREATE ACCESS METHOD statement
/// Called after CREATE ACCESS METHOD tokens have been consumed
pub fn parse_create_access_method(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{
        AccessMethodType, CreateAccessMethodStatement,
    };

    // ACCESS METHOD token already consumed by dispatcher

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected access method name".to_string(),
            position: parser.position,
            expected: vec!["method_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse TYPE
    parser.expect(Token::Type)?;
    let method_type = if let Some(Token::Identifier(t)) = &parser.current_token {
        let mt = match t.to_uppercase().as_str() {
            "INDEX" => AccessMethodType::Index,
            "TABLE" => AccessMethodType::Table,
            _ => AccessMethodType::Index,
        };
        parser.advance()?;
        mt
    } else {
        AccessMethodType::Index
    };

    // Parse HANDLER
    if let Some(Token::Identifier(kw)) = &parser.current_token {
        if kw.to_uppercase() == "HANDLER" {
            parser.advance()?;
        }
    }
    let handler_name = utilities::parse_table_name(parser)?;
    let handler = handler_name.to_string();

    Ok(Statement::CreateAccessMethod(CreateAccessMethodStatement {
        name,
        method_type,
        handler,
    }))
}

/// Parse DROP ACCESS METHOD statement
/// Called after DROP ACCESS METHOD tokens have been consumed
pub fn parse_drop_access_method(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropAccessMethodStatement;

    // ACCESS METHOD token already consumed by dispatcher

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected access method name".to_string(),
            position: parser.position,
            expected: vec!["method_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropAccessMethod(DropAccessMethodStatement {
        if_exists,
        name,
        cascade,
    }))
}

// ============================================================================
// STATISTICS statements
// ============================================================================

/// Parse CREATE STATISTICS statement
pub fn parse_create_statistics(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::CreateStatisticsStatement;

    parser.expect(Token::Statistics)?;

    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = utilities::parse_table_name(parser)?;

    // Parse optional ( statistics_kind, ... )
    let mut kinds = Vec::new();
    if parser.matches(&[Token::LeftParen]) {
        parser.advance()?;
        while !parser.matches(&[Token::RightParen]) {
            if let Some(Token::Identifier(k)) = &parser.current_token {
                kinds.push(k.clone());
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

    // Parse ON columns FROM table
    parser.expect(Token::On)?;
    let mut columns = Vec::new();
    loop {
        if let Some(Token::Identifier(c)) = &parser.current_token {
            columns.push(c.clone());
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

    parser.expect(Token::From)?;
    let table = utilities::parse_table_name(parser)?;

    Ok(Statement::CreateStatistics(CreateStatisticsStatement {
        if_not_exists,
        name,
        kinds,
        columns,
        table,
    }))
}

/// Parse DROP STATISTICS statement
pub fn parse_drop_statistics(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropStatisticsStatement;

    parser.expect(Token::Statistics)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

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

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropStatistics(DropStatisticsStatement {
        if_exists,
        names,
        cascade,
    }))
}

/// Parse ALTER STATISTICS statement
pub fn parse_alter_statistics(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{
        AlterStatisticsAction, AlterStatisticsStatement,
    };

    parser.expect(Token::Statistics)?;

    let name = utilities::parse_table_name(parser)?;

    let action = if parser.matches(&[Token::Rename]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected new statistics name".to_string(),
                position: parser.position,
                expected: vec!["new_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterStatisticsAction::Rename(new_name)
    } else if parser.matches(&[Token::Owner]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_owner = if let Some(Token::Identifier(o)) = &parser.current_token {
            let owner = o.clone();
            parser.advance()?;
            owner
        } else {
            return Err(ParseError {
                message: "Expected new owner name".to_string(),
                position: parser.position,
                expected: vec!["owner_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterStatisticsAction::Owner(new_owner)
    } else if parser.matches(&[Token::Set]) {
        parser.advance()?;
        if parser.matches(&[Token::Schema]) {
            parser.advance()?;
            let new_schema = if let Some(Token::Identifier(s)) = &parser.current_token {
                let schema = s.clone();
                parser.advance()?;
                schema
            } else {
                return Err(ParseError {
                    message: "Expected schema name".to_string(),
                    position: parser.position,
                    expected: vec!["schema_name".to_string()],
                    found: parser.current_token.clone(),
                });
            };
            AlterStatisticsAction::SetSchema(new_schema)
        } else if let Some(Token::Identifier(kw)) = &parser.current_token {
            if kw.to_uppercase() == "STATISTICS" {
                parser.advance()?;
                let target = if let Some(Token::NumericLiteral(n)) = &parser.current_token {
                    let t = n.parse().unwrap_or(0);
                    parser.advance()?;
                    t
                } else if let Some(Token::Integer) = &parser.current_token {
                    parser.advance()?;
                    0
                } else {
                    0
                };
                AlterStatisticsAction::SetStatisticsTarget(target)
            } else {
                return Err(ParseError {
                    message: "Expected SCHEMA or STATISTICS".to_string(),
                    position: parser.position,
                    expected: vec!["SCHEMA".to_string(), "STATISTICS".to_string()],
                    found: parser.current_token.clone(),
                });
            }
        } else {
            return Err(ParseError {
                message: "Expected SCHEMA or STATISTICS".to_string(),
                position: parser.position,
                expected: vec!["SCHEMA".to_string(), "STATISTICS".to_string()],
                found: parser.current_token.clone(),
            });
        }
    } else {
        return Err(ParseError {
            message: "Expected RENAME, OWNER, or SET".to_string(),
            position: parser.position,
            expected: vec!["RENAME".to_string(), "OWNER".to_string(), "SET".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterStatistics(AlterStatisticsStatement {
        name,
        action,
    }))
}

// ============================================================================
// TEXT SEARCH CONFIGURATION statements
// ============================================================================

/// Parse CREATE TEXT SEARCH CONFIGURATION statement
/// Called after CREATE TEXT SEARCH tokens have been consumed
pub fn parse_create_text_search_configuration(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::CreateTextSearchConfigurationStatement;

    // TEXT SEARCH already consumed, expect CONFIGURATION
    parser.expect(Token::Configuration)?;

    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = utilities::parse_table_name(parser)?;

    // Parse ( PARSER = parser_name | COPY = source_config )
    parser.expect(Token::LeftParen)?;
    let mut parser_opt: Option<TableName> = None;
    let mut source: Option<TableName> = None;

    while !parser.matches(&[Token::RightParen]) {
        if let Some(Token::Identifier(key)) = &parser.current_token {
            let key_upper = key.to_uppercase();
            parser.advance()?;
            parser.expect(Token::Equal)?;

            match key_upper.as_str() {
                "PARSER" => {
                    parser_opt = Some(utilities::parse_table_name(parser)?);
                }
                "COPY" => {
                    source = Some(utilities::parse_table_name(parser)?);
                }
                _ => {
                    parser.advance()?;
                }
            }
        }
        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }
    parser.expect(Token::RightParen)?;

    Ok(Statement::CreateTextSearchConfiguration(
        CreateTextSearchConfigurationStatement {
            if_not_exists,
            name,
            source,
            parser: parser_opt,
        },
    ))
}

/// Parse DROP TEXT SEARCH CONFIGURATION statement
/// Called after DROP TEXT SEARCH tokens have been consumed
pub fn parse_drop_text_search_configuration(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropTextSearchConfigurationStatement;

    // TEXT SEARCH already consumed, expect CONFIGURATION
    parser.expect(Token::Configuration)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let mut names = Vec::new();
    names.push(utilities::parse_table_name(parser)?);
    while parser.matches(&[Token::Comma]) {
        parser.advance()?;
        names.push(utilities::parse_table_name(parser)?);
    }

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropTextSearchConfiguration(
        DropTextSearchConfigurationStatement {
            if_exists,
            names,
            cascade,
        },
    ))
}

/// Parse ALTER TEXT SEARCH CONFIGURATION statement
/// Called after ALTER TEXT SEARCH tokens have been consumed
pub fn parse_alter_text_search_configuration(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{
        AlterTextSearchConfigurationAction, AlterTextSearchConfigurationStatement,
    };

    // TEXT SEARCH already consumed, expect CONFIGURATION
    parser.expect(Token::Configuration)?;

    let name = utilities::parse_table_name(parser)?;

    let action = if parser.matches(&[Token::Rename]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected new configuration name".to_string(),
                position: parser.position,
                expected: vec!["new_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterTextSearchConfigurationAction::Rename(new_name)
    } else if parser.matches(&[Token::Owner]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_owner = if let Some(Token::Identifier(o)) = &parser.current_token {
            let owner = o.clone();
            parser.advance()?;
            owner
        } else {
            return Err(ParseError {
                message: "Expected new owner name".to_string(),
                position: parser.position,
                expected: vec!["owner_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterTextSearchConfigurationAction::Owner(new_owner)
    } else if parser.matches(&[Token::Set]) {
        parser.advance()?;
        parser.expect(Token::Schema)?;
        let new_schema = if let Some(Token::Identifier(s)) = &parser.current_token {
            let schema = s.clone();
            parser.advance()?;
            schema
        } else {
            return Err(ParseError {
                message: "Expected schema name".to_string(),
                position: parser.position,
                expected: vec!["schema_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterTextSearchConfigurationAction::SetSchema(new_schema)
    } else if parser.matches(&[Token::Add]) {
        parser.advance()?;
        parser.expect(Token::Mapping)?;
        // Parse FOR token_type WITH dictionary
        parser.expect(Token::For)?;
        let token_type = if let Some(Token::Identifier(t)) = &parser.current_token {
            let tt = t.clone();
            parser.advance()?;
            tt
        } else {
            return Err(ParseError {
                message: "Expected token type".to_string(),
                position: parser.position,
                expected: vec!["token_type".to_string()],
                found: parser.current_token.clone(),
            });
        };
        parser.expect(Token::With)?;
        let mut dictionaries = Vec::new();
        loop {
            let dict = utilities::parse_table_name(parser)?;
            dictionaries.push(dict);
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        AlterTextSearchConfigurationAction::AddMapping {
            token_type,
            dictionaries,
        }
    } else if parser.matches(&[Token::Drop]) {
        parser.advance()?;
        let if_exists = if parser.matches(&[Token::If]) {
            parser.advance()?;
            parser.expect(Token::Exists)?;
            true
        } else {
            false
        };
        parser.expect(Token::Mapping)?;
        parser.expect(Token::For)?;
        let token_type = if let Some(Token::Identifier(t)) = &parser.current_token {
            let tt = t.clone();
            parser.advance()?;
            tt
        } else {
            return Err(ParseError {
                message: "Expected token type".to_string(),
                position: parser.position,
                expected: vec!["token_type".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterTextSearchConfigurationAction::DropMapping {
            if_exists,
            token_type,
        }
    } else if parser.matches(&[Token::Alter]) {
        parser.advance()?;
        parser.expect(Token::Mapping)?;
        parser.expect(Token::For)?;
        let token_type = if let Some(Token::Identifier(t)) = &parser.current_token {
            let tt = t.clone();
            parser.advance()?;
            tt
        } else {
            return Err(ParseError {
                message: "Expected token type".to_string(),
                position: parser.position,
                expected: vec!["token_type".to_string()],
                found: parser.current_token.clone(),
            });
        };
        parser.expect(Token::With)?;
        let mut dictionaries = Vec::new();
        loop {
            let dict = utilities::parse_table_name(parser)?;
            dictionaries.push(dict);
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        AlterTextSearchConfigurationAction::AlterMapping {
            token_type,
            dictionaries,
        }
    } else {
        return Err(ParseError {
            message: "Expected RENAME, OWNER, SET, ADD, ALTER, or DROP".to_string(),
            position: parser.position,
            expected: vec![
                "RENAME".to_string(),
                "OWNER".to_string(),
                "SET".to_string(),
                "ADD".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterTextSearchConfiguration(
        AlterTextSearchConfigurationStatement { name, action },
    ))
}

// ============================================================================
// TEXT SEARCH DICTIONARY statements
// ============================================================================

/// Parse CREATE TEXT SEARCH DICTIONARY statement
/// Called after CREATE TEXT SEARCH tokens have been consumed
pub fn parse_create_text_search_dictionary(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::CreateTextSearchDictionaryStatement;

    // TEXT SEARCH already consumed, expect DICTIONARY
    parser.expect(Token::Dictionary)?;

    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = utilities::parse_table_name(parser)?;

    // Parse ( TEMPLATE = template_name [, option = value ...] )
    parser.expect(Token::LeftParen)?;
    let mut template = TableName {
        schema: None,
        name: String::new(),
    };
    let mut options = Vec::new();

    // First option should be TEMPLATE
    if let Some(Token::Identifier(key)) = &parser.current_token {
        if key.to_uppercase() == "TEMPLATE" {
            parser.advance()?;
            parser.expect(Token::Equal)?;
            template = utilities::parse_table_name(parser)?;
        }
    }

    while parser.matches(&[Token::Comma]) {
        parser.advance()?;
        if let Some(Token::Identifier(key)) = &parser.current_token {
            let key = key.clone();
            parser.advance()?;
            parser.expect(Token::Equal)?;
            let value = if let Some(Token::StringLiteral(v)) = &parser.current_token {
                let v = v.clone();
                parser.advance()?;
                v
            } else if let Some(Token::Identifier(v)) = &parser.current_token {
                let v = v.clone();
                parser.advance()?;
                v
            } else {
                String::new()
            };
            options.push((key, value));
        }
    }
    parser.expect(Token::RightParen)?;

    Ok(Statement::CreateTextSearchDictionary(
        CreateTextSearchDictionaryStatement {
            if_not_exists,
            name,
            template,
            options,
        },
    ))
}

/// Parse DROP TEXT SEARCH DICTIONARY statement
/// Called after DROP TEXT SEARCH tokens have been consumed
pub fn parse_drop_text_search_dictionary(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropTextSearchDictionaryStatement;

    // TEXT SEARCH already consumed, expect DICTIONARY
    parser.expect(Token::Dictionary)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let mut names = Vec::new();
    names.push(utilities::parse_table_name(parser)?);
    while parser.matches(&[Token::Comma]) {
        parser.advance()?;
        names.push(utilities::parse_table_name(parser)?);
    }

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropTextSearchDictionary(
        DropTextSearchDictionaryStatement {
            if_exists,
            names,
            cascade,
        },
    ))
}

/// Parse ALTER TEXT SEARCH DICTIONARY statement
/// Called after ALTER TEXT SEARCH tokens have been consumed
pub fn parse_alter_text_search_dictionary(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{
        AlterTextSearchDictionaryAction, AlterTextSearchDictionaryStatement,
    };

    // TEXT SEARCH already consumed, expect DICTIONARY
    parser.expect(Token::Dictionary)?;

    let name = utilities::parse_table_name(parser)?;

    let action = if parser.matches(&[Token::Rename]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected new dictionary name".to_string(),
                position: parser.position,
                expected: vec!["new_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterTextSearchDictionaryAction::Rename(new_name)
    } else if parser.matches(&[Token::Owner]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_owner = if let Some(Token::Identifier(o)) = &parser.current_token {
            let owner = o.clone();
            parser.advance()?;
            owner
        } else {
            return Err(ParseError {
                message: "Expected new owner name".to_string(),
                position: parser.position,
                expected: vec!["owner_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterTextSearchDictionaryAction::Owner(new_owner)
    } else if parser.matches(&[Token::Set]) {
        parser.advance()?;
        parser.expect(Token::Schema)?;
        let new_schema = if let Some(Token::Identifier(s)) = &parser.current_token {
            let schema = s.clone();
            parser.advance()?;
            schema
        } else {
            return Err(ParseError {
                message: "Expected schema name".to_string(),
                position: parser.position,
                expected: vec!["schema_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterTextSearchDictionaryAction::SetSchema(new_schema)
    } else if parser.matches(&[Token::LeftParen]) {
        // ( option = value, ... )
        parser.advance()?;
        let mut opts = Vec::new();
        while !parser.matches(&[Token::RightParen]) {
            if let Some(Token::Identifier(key)) = &parser.current_token {
                let key = key.clone();
                parser.advance()?;
                parser.expect(Token::Equal)?;
                let value = if let Some(Token::StringLiteral(v)) = &parser.current_token {
                    let v = v.clone();
                    parser.advance()?;
                    v
                } else if let Some(Token::Identifier(v)) = &parser.current_token {
                    let v = v.clone();
                    parser.advance()?;
                    v
                } else {
                    String::new()
                };
                opts.push((key, value));
            }
            if parser.matches(&[Token::Comma]) {
                parser.advance()?;
            } else {
                break;
            }
        }
        parser.expect(Token::RightParen)?;
        AlterTextSearchDictionaryAction::SetOptions(opts)
    } else {
        return Err(ParseError {
            message: "Expected RENAME, OWNER, SET, or (options)".to_string(),
            position: parser.position,
            expected: vec![
                "RENAME".to_string(),
                "OWNER".to_string(),
                "SET".to_string(),
                "(".to_string(),
            ],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterTextSearchDictionary(
        AlterTextSearchDictionaryStatement { name, action },
    ))
}

// ============================================================================
// TEXT SEARCH PARSER statements
// ============================================================================

/// Parse CREATE TEXT SEARCH PARSER statement
/// Called after CREATE TEXT SEARCH tokens have been consumed
pub fn parse_create_text_search_parser(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::CreateTextSearchParserStatement;

    // TEXT SEARCH already consumed, expect PARSER
    parser.expect(Token::Parser)?;

    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = utilities::parse_table_name(parser)?;

    // Parse ( START = fn, GETTOKEN = fn, END = fn, LEXTYPES = fn [, HEADLINE = fn] )
    parser.expect(Token::LeftParen)?;
    let mut start_func = String::new();
    let mut gettoken_func = String::new();
    let mut end_func = String::new();
    let mut lextypes_func = String::new();
    let mut headline_func: Option<String> = None;

    while !parser.matches(&[Token::RightParen]) {
        if let Some(Token::Identifier(key)) = &parser.current_token {
            let key_upper = key.to_uppercase();
            parser.advance()?;
            parser.expect(Token::Equal)?;

            let func_name = utilities::parse_table_name(parser)?.to_string();
            match key_upper.as_str() {
                "START" => {
                    start_func = func_name;
                }
                "GETTOKEN" => {
                    gettoken_func = func_name;
                }
                "END" => {
                    end_func = func_name;
                }
                "LEXTYPES" => {
                    lextypes_func = func_name;
                }
                "HEADLINE" => {
                    headline_func = Some(func_name);
                }
                _ => {}
            }
        }
        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }
    parser.expect(Token::RightParen)?;

    Ok(Statement::CreateTextSearchParser(
        CreateTextSearchParserStatement {
            if_not_exists,
            name,
            start_func,
            gettoken_func,
            end_func,
            lextypes_func,
            headline_func,
        },
    ))
}

/// Parse DROP TEXT SEARCH PARSER statement
/// Called after DROP TEXT SEARCH tokens have been consumed
pub fn parse_drop_text_search_parser(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropTextSearchParserStatement;

    // TEXT SEARCH already consumed, expect PARSER
    parser.expect(Token::Parser)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let mut names = Vec::new();
    names.push(utilities::parse_table_name(parser)?);
    while parser.matches(&[Token::Comma]) {
        parser.advance()?;
        names.push(utilities::parse_table_name(parser)?);
    }

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropTextSearchParser(
        DropTextSearchParserStatement {
            if_exists,
            names,
            cascade,
        },
    ))
}

/// Parse ALTER TEXT SEARCH PARSER statement
/// Called after ALTER TEXT SEARCH tokens have been consumed
pub fn parse_alter_text_search_parser(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{
        AlterTextSearchParserAction, AlterTextSearchParserStatement,
    };

    // TEXT SEARCH already consumed, expect PARSER
    parser.expect(Token::Parser)?;

    let name = utilities::parse_table_name(parser)?;

    let action = if parser.matches(&[Token::Rename]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected new parser name".to_string(),
                position: parser.position,
                expected: vec!["new_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterTextSearchParserAction::Rename(new_name)
    } else if parser.matches(&[Token::Set]) {
        parser.advance()?;
        parser.expect(Token::Schema)?;
        let new_schema = if let Some(Token::Identifier(s)) = &parser.current_token {
            let schema = s.clone();
            parser.advance()?;
            schema
        } else {
            return Err(ParseError {
                message: "Expected schema name".to_string(),
                position: parser.position,
                expected: vec!["schema_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterTextSearchParserAction::SetSchema(new_schema)
    } else {
        return Err(ParseError {
            message: "Expected RENAME or SET SCHEMA".to_string(),
            position: parser.position,
            expected: vec!["RENAME".to_string(), "SET".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterTextSearchParser(
        AlterTextSearchParserStatement { name, action },
    ))
}

// ============================================================================
// TEXT SEARCH TEMPLATE statements
// ============================================================================

/// Parse CREATE TEXT SEARCH TEMPLATE statement
/// Called after CREATE TEXT SEARCH tokens have been consumed
pub fn parse_create_text_search_template(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::CreateTextSearchTemplateStatement;

    // TEXT SEARCH already consumed, expect TEMPLATE
    parser.expect(Token::Template)?;

    let if_not_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Not)?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = utilities::parse_table_name(parser)?;

    // Parse ( INIT = fn, LEXIZE = fn )
    parser.expect(Token::LeftParen)?;
    let mut init_func: Option<String> = None;
    let mut lexize_func = String::new();

    while !parser.matches(&[Token::RightParen]) {
        if let Some(Token::Identifier(key)) = &parser.current_token {
            let key_upper = key.to_uppercase();
            parser.advance()?;
            parser.expect(Token::Equal)?;

            let func_name = utilities::parse_table_name(parser)?.to_string();
            match key_upper.as_str() {
                "INIT" => {
                    init_func = Some(func_name);
                }
                "LEXIZE" => {
                    lexize_func = func_name;
                }
                _ => {}
            }
        }
        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }
    parser.expect(Token::RightParen)?;

    Ok(Statement::CreateTextSearchTemplate(
        CreateTextSearchTemplateStatement {
            if_not_exists,
            name,
            init_func,
            lexize_func,
        },
    ))
}

/// Parse DROP TEXT SEARCH TEMPLATE statement
/// Called after DROP TEXT SEARCH tokens have been consumed
pub fn parse_drop_text_search_template(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropTextSearchTemplateStatement;

    // TEXT SEARCH already consumed, expect TEMPLATE
    parser.expect(Token::Template)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let mut names = Vec::new();
    names.push(utilities::parse_table_name(parser)?);
    while parser.matches(&[Token::Comma]) {
        parser.advance()?;
        names.push(utilities::parse_table_name(parser)?);
    }

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropTextSearchTemplate(
        DropTextSearchTemplateStatement {
            if_exists,
            names,
            cascade,
        },
    ))
}

/// Parse ALTER TEXT SEARCH TEMPLATE statement
/// Called after ALTER TEXT SEARCH tokens have been consumed
pub fn parse_alter_text_search_template(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{
        AlterTextSearchTemplateAction, AlterTextSearchTemplateStatement,
    };

    // TEXT SEARCH already consumed, expect TEMPLATE
    parser.expect(Token::Template)?;

    let name = utilities::parse_table_name(parser)?;

    let action = if parser.matches(&[Token::Rename]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected new template name".to_string(),
                position: parser.position,
                expected: vec!["new_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterTextSearchTemplateAction::Rename(new_name)
    } else if parser.matches(&[Token::Set]) {
        parser.advance()?;
        parser.expect(Token::Schema)?;
        let new_schema = if let Some(Token::Identifier(s)) = &parser.current_token {
            let schema = s.clone();
            parser.advance()?;
            schema
        } else {
            return Err(ParseError {
                message: "Expected schema name".to_string(),
                position: parser.position,
                expected: vec!["schema_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterTextSearchTemplateAction::SetSchema(new_schema)
    } else {
        return Err(ParseError {
            message: "Expected RENAME or SET SCHEMA".to_string(),
            position: parser.position,
            expected: vec!["RENAME".to_string(), "SET".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterTextSearchTemplate(
        AlterTextSearchTemplateStatement { name, action },
    ))
}

// ============================================================================
// TRANSFORM statements
// ============================================================================

/// Parse CREATE TRANSFORM statement
pub fn parse_create_transform(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::CreateTransformStatement;

    parser.expect(Token::Transform)?;

    let or_replace = false; // Would need to check OR REPLACE before CREATE

    // Parse FOR type_name LANGUAGE lang_name
    parser.expect(Token::For)?;
    let type_name = utilities::parse_data_type(parser)?;
    parser.expect(Token::Language)?;
    let language = if let Some(Token::Identifier(l)) = &parser.current_token {
        let lang = l.clone();
        parser.advance()?;
        lang
    } else {
        return Err(ParseError {
            message: "Expected language name".to_string(),
            position: parser.position,
            expected: vec!["language_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse ( FROM SQL WITH FUNCTION fn, TO SQL WITH FUNCTION fn )
    parser.expect(Token::LeftParen)?;
    let mut from_sql: Option<String> = None;
    let mut to_sql: Option<String> = None;

    while !parser.matches(&[Token::RightParen]) {
        if parser.matches(&[Token::From]) {
            parser.advance()?;
            // FROM SQL WITH FUNCTION fn
            if let Some(Token::Identifier(kw)) = &parser.current_token {
                if kw.to_uppercase() == "SQL" {
                    parser.advance()?;
                    parser.expect(Token::With)?;
                    if let Some(Token::Identifier(kw2)) = &parser.current_token {
                        if kw2.to_uppercase() == "FUNCTION" {
                            parser.advance()?;
                            let func_name = utilities::parse_table_name(parser)?;
                            from_sql = Some(func_name.to_string());
                            // Skip function arguments if present
                            if parser.matches(&[Token::LeftParen]) {
                                let mut depth = 1;
                                parser.advance()?;
                                while depth > 0 {
                                    if parser.matches(&[Token::LeftParen]) {
                                        depth += 1;
                                    } else if parser.matches(&[Token::RightParen]) {
                                        depth -= 1;
                                    }
                                    if depth > 0 {
                                        parser.advance()?;
                                    }
                                }
                                parser.advance()?;
                            }
                        }
                    }
                }
            }
        } else if parser.matches(&[Token::To]) {
            parser.advance()?;
            // TO SQL WITH FUNCTION fn
            if let Some(Token::Identifier(kw)) = &parser.current_token {
                if kw.to_uppercase() == "SQL" {
                    parser.advance()?;
                    parser.expect(Token::With)?;
                    if let Some(Token::Identifier(kw2)) = &parser.current_token {
                        if kw2.to_uppercase() == "FUNCTION" {
                            parser.advance()?;
                            let func_name = utilities::parse_table_name(parser)?;
                            to_sql = Some(func_name.to_string());
                            // Skip function arguments if present
                            if parser.matches(&[Token::LeftParen]) {
                                let mut depth = 1;
                                parser.advance()?;
                                while depth > 0 {
                                    if parser.matches(&[Token::LeftParen]) {
                                        depth += 1;
                                    } else if parser.matches(&[Token::RightParen]) {
                                        depth -= 1;
                                    }
                                    if depth > 0 {
                                        parser.advance()?;
                                    }
                                }
                                parser.advance()?;
                            }
                        }
                    }
                }
            }
        } else {
            parser.advance()?;
        }
        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        }
    }
    parser.expect(Token::RightParen)?;

    Ok(Statement::CreateTransform(CreateTransformStatement {
        or_replace,
        type_name,
        language,
        from_sql,
        to_sql,
    }))
}

/// Parse DROP TRANSFORM statement
pub fn parse_drop_transform(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropTransformStatement;

    parser.expect(Token::Transform)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    // Parse FOR type_name LANGUAGE lang_name
    parser.expect(Token::For)?;
    let type_name = utilities::parse_data_type(parser)?;
    parser.expect(Token::Language)?;
    let language = if let Some(Token::Identifier(l)) = &parser.current_token {
        let lang = l.clone();
        parser.advance()?;
        lang
    } else {
        return Err(ParseError {
            message: "Expected language name".to_string(),
            position: parser.position,
            expected: vec!["language_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropTransform(DropTransformStatement {
        if_exists,
        type_name,
        language,
        cascade,
    }))
}

// ============================================================================
// LANGUAGE statements
// ============================================================================

/// Parse CREATE LANGUAGE statement
pub fn parse_create_language(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::CreateLanguageStatement;

    // Check for TRUSTED and/or PROCEDURAL
    let mut trusted = false;
    let mut procedural = false;

    while let Some(Token::Identifier(kw)) = &parser.current_token {
        match kw.to_uppercase().as_str() {
            "TRUSTED" => {
                trusted = true;
                parser.advance()?;
            }
            "PROCEDURAL" => {
                procedural = true;
                parser.advance()?;
            }
            _ => break,
        }
    }

    parser.expect(Token::Language)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected language name".to_string(),
            position: parser.position,
            expected: vec!["language_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    // Parse optional HANDLER, INLINE, VALIDATOR
    let mut handler: Option<String> = None;
    let mut inline_handler: Option<String> = None;
    let mut validator: Option<String> = None;

    while parser.current_token.is_some() && !parser.matches(&[Token::Semicolon]) {
        if let Some(Token::Identifier(kw)) = &parser.current_token {
            match kw.to_uppercase().as_str() {
                "HANDLER" => {
                    parser.advance()?;
                    handler = Some(utilities::parse_table_name(parser)?.to_string());
                }
                "INLINE" => {
                    parser.advance()?;
                    inline_handler = Some(utilities::parse_table_name(parser)?.to_string());
                }
                "VALIDATOR" => {
                    parser.advance()?;
                    validator = Some(utilities::parse_table_name(parser)?.to_string());
                }
                "NO" => {
                    parser.advance()?;
                    if let Some(Token::Identifier(kw2)) = &parser.current_token {
                        match kw2.to_uppercase().as_str() {
                            "INLINE" => {
                                parser.advance()?;
                                inline_handler = None;
                            }
                            "VALIDATOR" => {
                                parser.advance()?;
                                validator = None;
                            }
                            _ => parser.advance()?,
                        }
                    }
                }
                _ => break,
            }
        } else {
            break;
        }
    }

    Ok(Statement::CreateLanguage(CreateLanguageStatement {
        or_replace: false,
        trusted,
        procedural,
        name,
        handler,
        inline_handler,
        validator,
    }))
}

/// Parse DROP LANGUAGE statement
pub fn parse_drop_language(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropLanguageStatement;

    // Skip PROCEDURAL if present
    if let Some(Token::Identifier(kw)) = &parser.current_token {
        if kw.to_uppercase() == "PROCEDURAL" {
            parser.advance()?;
        }
    }

    parser.expect(Token::Language)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected language name".to_string(),
            position: parser.position,
            expected: vec!["language_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropLanguage(DropLanguageStatement {
        if_exists,
        name,
        cascade,
    }))
}

/// Parse ALTER LANGUAGE statement
pub fn parse_alter_language(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::{AlterLanguageAction, AlterLanguageStatement};

    // Skip PROCEDURAL if present
    if let Some(Token::Identifier(kw)) = &parser.current_token {
        if kw.to_uppercase() == "PROCEDURAL" {
            parser.advance()?;
        }
    }

    parser.expect(Token::Language)?;

    let name = if let Some(Token::Identifier(n)) = &parser.current_token {
        let name = n.clone();
        parser.advance()?;
        name
    } else {
        return Err(ParseError {
            message: "Expected language name".to_string(),
            position: parser.position,
            expected: vec!["language_name".to_string()],
            found: parser.current_token.clone(),
        });
    };

    let action = if parser.matches(&[Token::Rename]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_name = if let Some(Token::Identifier(n)) = &parser.current_token {
            let name = n.clone();
            parser.advance()?;
            name
        } else {
            return Err(ParseError {
                message: "Expected new language name".to_string(),
                position: parser.position,
                expected: vec!["new_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterLanguageAction::Rename(new_name)
    } else if parser.matches(&[Token::Owner]) {
        parser.advance()?;
        parser.expect(Token::To)?;
        let new_owner = if let Some(Token::Identifier(o)) = &parser.current_token {
            let owner = o.clone();
            parser.advance()?;
            owner
        } else {
            return Err(ParseError {
                message: "Expected new owner name".to_string(),
                position: parser.position,
                expected: vec!["owner_name".to_string()],
                found: parser.current_token.clone(),
            });
        };
        AlterLanguageAction::Owner(new_owner)
    } else {
        return Err(ParseError {
            message: "Expected RENAME or OWNER".to_string(),
            position: parser.position,
            expected: vec!["RENAME".to_string(), "OWNER".to_string()],
            found: parser.current_token.clone(),
        });
    };

    Ok(Statement::AlterLanguage(AlterLanguageStatement {
        name,
        action,
    }))
}

// ============================================================================
// DROP FUNCTION/PROCEDURE/ROUTINE/OWNED statements
// ============================================================================

/// Parse DROP FUNCTION statement
pub fn parse_drop_function(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropFunctionStatement;

    parser.expect(Token::Function)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let mut functions = Vec::new();
    loop {
        let name = utilities::parse_table_name(parser)?;

        // Parse optional argument types
        let mut arg_types = Vec::new();
        if parser.matches(&[Token::LeftParen]) {
            parser.advance()?;
            while !parser.matches(&[Token::RightParen]) {
                let dt = utilities::parse_data_type(parser)?;
                arg_types.push(dt);
                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }
            parser.expect(Token::RightParen)?;
        }
        functions.push((name, arg_types));

        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropFunction(DropFunctionStatement {
        if_exists,
        functions,
        cascade,
    }))
}

/// Parse DROP PROCEDURE statement
pub fn parse_drop_procedure(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropProcedureStatement;

    parser.expect(Token::Procedure)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let mut procedures = Vec::new();
    loop {
        let name = utilities::parse_table_name(parser)?;

        // Parse optional argument types
        let mut arg_types = Vec::new();
        if parser.matches(&[Token::LeftParen]) {
            parser.advance()?;
            while !parser.matches(&[Token::RightParen]) {
                let dt = utilities::parse_data_type(parser)?;
                arg_types.push(dt);
                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }
            parser.expect(Token::RightParen)?;
        }
        procedures.push((name, arg_types));

        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropProcedure(DropProcedureStatement {
        if_exists,
        procedures,
        cascade,
    }))
}

/// Parse DROP ROUTINE statement
pub fn parse_drop_routine(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropRoutineStatement;

    parser.expect(Token::Routine)?;

    let if_exists = if parser.matches(&[Token::If]) {
        parser.advance()?;
        parser.expect(Token::Exists)?;
        true
    } else {
        false
    };

    let mut routines = Vec::new();
    loop {
        let name = utilities::parse_table_name(parser)?;

        // Parse optional argument types
        let mut arg_types = Vec::new();
        if parser.matches(&[Token::LeftParen]) {
            parser.advance()?;
            while !parser.matches(&[Token::RightParen]) {
                let dt = utilities::parse_data_type(parser)?;
                arg_types.push(dt);
                if parser.matches(&[Token::Comma]) {
                    parser.advance()?;
                } else {
                    break;
                }
            }
            parser.expect(Token::RightParen)?;
        }
        routines.push((name, arg_types));

        if parser.matches(&[Token::Comma]) {
            parser.advance()?;
        } else {
            break;
        }
    }

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropRoutine(DropRoutineStatement {
        if_exists,
        routines,
        cascade,
    }))
}

/// Parse DROP OWNED statement
pub fn parse_drop_owned(parser: &mut SqlParser) -> ParseResult<Statement> {
    use crate::protocols::postgres_wire::sql::ast::DropOwnedStatement;

    parser.expect(Token::Owned)?;
    parser.expect(Token::By)?;

    let mut roles = Vec::new();
    loop {
        if let Some(Token::Identifier(r)) = &parser.current_token {
            roles.push(r.clone());
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

    let cascade = if parser.matches(&[Token::Cascade]) {
        parser.advance()?;
        true
    } else {
        false
    };

    Ok(Statement::DropOwned(DropOwnedStatement { roles, cascade }))
}
