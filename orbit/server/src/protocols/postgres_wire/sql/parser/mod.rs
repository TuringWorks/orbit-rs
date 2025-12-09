//! SQL Parser for ANSI SQL compliance
//!
//! This module provides a recursive descent parser that can handle
//! all major SQL constructs with proper error handling and recovery.

pub mod dcl;
pub mod ddl;
pub mod dml;
pub mod expressions;
pub mod select;
pub mod tcl;
pub mod utilities;
pub mod utility_commands;

use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::sql::{
    ast::Statement,
    lexer::{Lexer, Token},
};

/// Parse result type
pub type ParseResult<T> = Result<T, ParseError>;

/// Parse error information
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
    pub expected: Vec<String>,
    pub found: Option<Token>,
}

impl From<ParseError> for ProtocolError {
    fn from(err: ParseError) -> Self {
        ProtocolError::PostgresError(format!("SQL Parse Error: {}", err.message))
    }
}

/// Main SQL Parser
pub struct SqlParser {
    pub tokens: Vec<Token>,
    pub position: usize,
    pub current_token: Option<Token>,
}

impl SqlParser {
    /// Create a new SQL parser
    pub fn new() -> Self {
        Self {
            tokens: Vec::new(),
            position: 0,
            current_token: None,
        }
    }

    /// Parse a SQL statement from string input
    pub fn parse(&mut self, input: &str) -> ProtocolResult<Statement> {
        let statements = self.parse_multiple(input)?;
        statements
            .into_iter()
            .next()
            .ok_or_else(|| ProtocolError::PostgresError("Empty SQL statement".to_string()))
    }

    /// Parse multiple SQL statements from string input
    pub fn parse_multiple(&mut self, input: &str) -> ProtocolResult<Vec<Statement>> {
        // Tokenize the input
        let mut lexer = Lexer::new(input);
        self.tokens = lexer.tokenize();
        self.position = 0;
        self.current_token = self.tokens.first().cloned();

        let mut statements = Vec::new();

        while self.current_token.is_some() && self.current_token != Some(Token::Eof) {
            // Skip semicolons
            while self.matches(&[Token::Semicolon]) {
                self.advance()?;
            }

            if self.current_token.is_none() || self.current_token == Some(Token::Eof) {
                break;
            }

            // Parse the statement
            let stmt = self
                .parse_statement()
                .map_err(crate::protocols::error::ProtocolError::from)?;
            statements.push(stmt);
        }

        Ok(statements)
    }

    /// Parse a top-level SQL statement
    fn parse_statement(&mut self) -> ParseResult<Statement> {
        match &self.current_token {
            // DDL Statements
            Some(Token::Create) => self.parse_create_statement(),
            Some(Token::Alter) => self.parse_alter_statement(),
            Some(Token::Drop) => self.parse_drop_statement(),

            // DML Statements
            Some(Token::Select) => self.parse_select_statement(),
            Some(Token::Insert) => self.parse_insert_statement(),
            Some(Token::Update) => self.parse_update_statement(),
            Some(Token::Delete) => self.parse_delete_statement(),
            Some(Token::Merge) => self.parse_merge_statement(),
            Some(Token::Copy) => self.parse_copy_statement(),

            // DCL Statements
            Some(Token::Grant) => self.parse_grant_statement(),
            Some(Token::Revoke) => self.parse_revoke_statement(),

            // TCL Statements
            Some(Token::Begin) => self.parse_begin_statement(),
            Some(Token::Commit) => self.parse_commit_statement(),
            Some(Token::Rollback) => self.parse_rollback_statement(),
            Some(Token::Savepoint) => self.parse_savepoint_statement(),
            Some(Token::Abort) => {
                self.advance()?;
                Ok(Statement::Rollback(
                    crate::protocols::postgres_wire::sql::ast::RollbackStatement {
                        chain: false,
                        to_savepoint: None,
                    },
                ))
            }

            // Session Management
            Some(Token::Set) => self.parse_set_statement(),
            Some(Token::Reset) => {
                self.advance()?;
                utility_commands::parse_reset(self)
            }
            Some(Token::Discard) => {
                self.advance()?;
                utility_commands::parse_discard(self)
            }

            // Cursor Commands
            Some(Token::Declare) => {
                self.advance()?;
                utility_commands::parse_declare_cursor(self)
            }
            Some(Token::Fetch) => {
                self.advance()?;
                utility_commands::parse_fetch(self)
            }
            Some(Token::Move) => {
                self.advance()?;
                utility_commands::parse_move(self)
            }
            Some(Token::Close) => {
                self.advance()?;
                utility_commands::parse_close(self)
            }

            // Notification Commands
            Some(Token::Listen) => {
                self.advance()?;
                utility_commands::parse_listen(self)
            }
            Some(Token::Unlisten) => {
                self.advance()?;
                utility_commands::parse_unlisten(self)
            }
            Some(Token::Notify) => {
                self.advance()?;
                utility_commands::parse_notify(self)
            }

            // Prepared Statement Commands
            Some(Token::Prepare) => {
                self.advance()?;
                utility_commands::parse_prepare(self)
            }
            Some(Token::Execute) => {
                self.advance()?;
                utility_commands::parse_execute(self)
            }
            Some(Token::Deallocate) => {
                self.advance()?;
                utility_commands::parse_deallocate(self)
            }

            // Maintenance Commands
            Some(Token::Vacuum) => {
                self.advance()?;
                utility_commands::parse_vacuum(self)
            }
            Some(Token::Analyze) => {
                self.advance()?;
                utility_commands::parse_analyze(self)
            }
            Some(Token::Reindex) => {
                self.advance()?;
                utility_commands::parse_reindex(self)
            }
            Some(Token::Cluster) => {
                self.advance()?;
                utility_commands::parse_cluster(self)
            }
            Some(Token::Checkpoint) => {
                self.advance()?;
                utility_commands::parse_checkpoint(self)
            }

            // Procedural Commands
            Some(Token::Call) => {
                self.advance()?;
                utility_commands::parse_call(self)
            }
            Some(Token::Do) => {
                self.advance()?;
                utility_commands::parse_do(self)
            }

            // COMMENT ON statement
            Some(Token::CommentKeyword) => ddl::parse_comment_on(self),

            // TRUNCATE statement
            Some(Token::Truncate) => ddl::parse_truncate(self),

            // Handle EXPLAIN as identifier (not a keyword yet)
            Some(Token::Identifier(name)) if name.to_uppercase() == "EXPLAIN" => {
                self.advance()?; // consume EXPLAIN

                // Parse the statement to explain
                let statement = self.parse_statement()?;

                // Return an EXPLAIN statement
                Ok(Statement::Explain(
                    crate::protocols::postgres_wire::sql::ast::ExplainStatement {
                        analyze: false,
                        verbose: false,
                        costs: true,
                        buffers: false,
                        timing: false,
                        format: crate::protocols::postgres_wire::sql::ast::ExplainFormat::Text,
                        statement: Box::new(statement),
                    },
                ))
            }

            Some(token) => Err(ParseError {
                message: format!("Unexpected token at start of statement: {token:?}"),
                position: self.position,
                expected: vec![
                    "CREATE".to_string(),
                    "ALTER".to_string(),
                    "DROP".to_string(),
                    "SELECT".to_string(),
                    "INSERT".to_string(),
                    "UPDATE".to_string(),
                    "DELETE".to_string(),
                    "MERGE".to_string(),
                    "COPY".to_string(),
                    "GRANT".to_string(),
                    "REVOKE".to_string(),
                    "BEGIN".to_string(),
                    "COMMIT".to_string(),
                    "ROLLBACK".to_string(),
                    "SET".to_string(),
                    "COMMENT".to_string(),
                ],
                found: Some(token.clone()),
            }),

            None => Err(ParseError {
                message: "Empty SQL statement".to_string(),
                position: self.position,
                expected: vec!["SQL statement".to_string()],
                found: None,
            }),
        }
    }

    /// Advance to the next token
    fn advance(&mut self) -> ParseResult<()> {
        self.position += 1;
        self.current_token = self.tokens.get(self.position).cloned();
        Ok(())
    }

    /// Peek at the next token without advancing
    #[allow(dead_code)]
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position + 1)
    }

    /// Check if current token matches expected token and advance if so
    fn expect(&mut self, expected: Token) -> ParseResult<()> {
        if let Some(token) = &self.current_token {
            if std::mem::discriminant(token) == std::mem::discriminant(&expected) {
                self.advance()?;
                Ok(())
            } else {
                Err(ParseError {
                    message: format!("Expected {expected:?}, found {token:?}"),
                    position: self.position,
                    expected: vec![format!("{:?}", expected)],
                    found: Some(token.clone()),
                })
            }
        } else {
            Err(ParseError {
                message: format!("Expected {expected:?}, found EOF"),
                position: self.position,
                expected: vec![format!("{:?}", expected)],
                found: None,
            })
        }
    }

    /// Check if current token matches any of the expected tokens
    fn matches(&self, tokens: &[Token]) -> bool {
        if let Some(current) = &self.current_token {
            tokens
                .iter()
                .any(|token| std::mem::discriminant(current) == std::mem::discriminant(token))
        } else {
            false
        }
    }

    /// Parse CREATE statements
    fn parse_create_statement(&mut self) -> ParseResult<Statement> {
        self.expect(Token::Create)?;

        // Check for CREATE OR REPLACE VIEW/FUNCTION/TRIGGER
        let or_replace = if self.matches(&[Token::Or]) {
            self.advance()?;
            self.expect(Token::Replace)?;
            true
        } else {
            false
        };

        match &self.current_token {
            Some(Token::Database) => ddl::parse_create_database(self),
            Some(Token::Table) => ddl::parse_create_table(self),
            Some(Token::Unique) => {
                // CREATE UNIQUE INDEX
                self.advance()?; // consume UNIQUE
                self.expect(Token::Index)?;
                let mut stmt = ddl::parse_create_index_internal(self)?;
                if let Statement::CreateIndex(ref mut idx_stmt) = stmt {
                    idx_stmt.unique = true;
                }
                Ok(stmt)
            },
            Some(Token::Index) => ddl::parse_create_index(self),
            Some(Token::View) => {
                self.advance()?; // consume VIEW token
                let mut stmt = ddl::parse_create_view_internal(self)?;
                if let Statement::CreateView(ref mut view_stmt) = stmt {
                    view_stmt.replace = or_replace;
                }
                Ok(stmt)
            },
            Some(Token::Schema) => ddl::parse_create_schema(self),
            Some(Token::Extension) => ddl::parse_create_extension(self),
            Some(Token::Function) => {
                let mut stmt = ddl::parse_create_function(self)?;
                if let Statement::CreateFunction(ref mut func_stmt) = stmt {
                    func_stmt.or_replace = or_replace;
                }
                Ok(stmt)
            },
            Some(Token::Trigger) => {
                let mut stmt = ddl::parse_create_trigger(self)?;
                if let Statement::CreateTrigger(ref mut trigger_stmt) = stmt {
                    trigger_stmt.or_replace = or_replace;
                }
                Ok(stmt)
            },
            Some(Token::Sequence) => ddl::parse_create_sequence(self),
            Some(Token::Type) => ddl::parse_create_type(self),
            Some(Token::Domain) => ddl::parse_create_domain(self),
            Some(Token::Role) => ddl::parse_create_role(self, false),
            Some(Token::User) => ddl::parse_create_role(self, true),
            Some(Token::Policy) => ddl::parse_create_policy(self),
            Some(Token::Rule) => ddl::parse_create_rule(self, or_replace),
            // Extended DDL
            Some(Token::Group) => ddl::parse_create_group(self),
            Some(Token::Tablespace) => ddl::parse_create_tablespace(self),
            Some(Token::Aggregate) => ddl::parse_create_aggregate(self),
            Some(Token::Operator) => ddl::parse_create_operator(self),
            Some(Token::Cast) => ddl::parse_create_cast(self),
            Some(Token::Collation) => ddl::parse_create_collation(self),
            Some(Token::Conversion) => ddl::parse_create_conversion(self),
            Some(Token::Foreign) => {
                self.advance()?; // consume FOREIGN
                // Check for DATA WRAPPER or TABLE
                if let Some(Token::Identifier(id)) = &self.current_token {
                    if id.to_uppercase() == "DATA" {
                        self.advance()?; // consume DATA
                        // Expect WRAPPER
                        ddl::parse_create_foreign_data_wrapper(self)
                    } else {
                        ddl::parse_create_foreign_table(self)
                    }
                } else if matches!(&self.current_token, Some(Token::Table)) {
                    ddl::parse_create_foreign_table(self)
                } else {
                    Err(ParseError {
                        message: "Expected DATA WRAPPER or TABLE after FOREIGN".to_string(),
                        position: self.position,
                        expected: vec!["DATA".to_string(), "TABLE".to_string()],
                        found: self.current_token.clone(),
                    })
                }
            },
            Some(Token::Server) => ddl::parse_create_server(self),
            Some(Token::Publication) => ddl::parse_create_publication(self),
            Some(Token::Subscription) => ddl::parse_create_subscription(self),
            Some(Token::EventTrigger) => ddl::parse_create_event_trigger(self),
            Some(Token::AccessMethod) => ddl::parse_create_access_method(self),
            Some(Token::Statistics) => ddl::parse_create_statistics(self),
            Some(Token::TextSearch) => {
                self.advance()?; // consume TEXT SEARCH
                // Dispatch based on next token: CONFIGURATION, DICTIONARY, PARSER, TEMPLATE
                if matches!(&self.current_token, Some(Token::Configuration)) {
                    ddl::parse_create_text_search_configuration(self)
                } else if matches!(&self.current_token, Some(Token::Dictionary)) {
                    ddl::parse_create_text_search_dictionary(self)
                } else if matches!(&self.current_token, Some(Token::Parser)) {
                    ddl::parse_create_text_search_parser(self)
                } else if matches!(&self.current_token, Some(Token::Template)) {
                    ddl::parse_create_text_search_template(self)
                } else {
                    Err(ParseError {
                        message: "Expected CONFIGURATION, DICTIONARY, PARSER, or TEMPLATE after TEXT SEARCH".to_string(),
                        position: self.position,
                        expected: vec!["CONFIGURATION".to_string(), "DICTIONARY".to_string(), "PARSER".to_string(), "TEMPLATE".to_string()],
                        found: self.current_token.clone(),
                    })
                }
            },
            Some(Token::Transform) => ddl::parse_create_transform(self),
            Some(Token::Language) => ddl::parse_create_language(self),
            // PROCEDURE uses the same parser as FUNCTION with adjustments
            Some(Token::Procedure) => {
                let stmt = ddl::parse_create_function(self)?;
                // The function parser handles both - PROCEDURE token expected
                Ok(stmt)
            },

            Some(token) => Err(ParseError {
                message: format!("Unexpected token after CREATE: {token:?}"),
                position: self.position,
                expected: vec![
                    "DATABASE".to_string(),
                    "TABLE".to_string(),
                    "UNIQUE".to_string(),
                    "INDEX".to_string(),
                    "OR".to_string(),
                    "VIEW".to_string(),
                    "SCHEMA".to_string(),
                    "EXTENSION".to_string(),
                    "FUNCTION".to_string(),
                    "TRIGGER".to_string(),
                    "SEQUENCE".to_string(),
                    "TYPE".to_string(),
                    "DOMAIN".to_string(),
                    "ROLE".to_string(),
                    "USER".to_string(),
                    "POLICY".to_string(),
                    "RULE".to_string(),
                ],
                found: Some(token.clone()),
            }),

            None => Err(ParseError {
                message: "Expected object type after CREATE".to_string(),
                position: self.position,
                expected: vec!["DATABASE, TABLE, UNIQUE INDEX, INDEX, OR REPLACE VIEW, VIEW, SCHEMA, EXTENSION, FUNCTION, TRIGGER, or SEQUENCE".to_string()],
                found: None,
            }),
        }
    }

    /// Parse ALTER statements
    fn parse_alter_statement(&mut self) -> ParseResult<Statement> {
        self.expect(Token::Alter)?;

        match &self.current_token {
            Some(Token::Table) => ddl::parse_alter_table(self),
            Some(Token::Sequence) => ddl::parse_alter_sequence(self),
            Some(Token::Type) => ddl::parse_alter_type(self),
            Some(Token::Domain) => ddl::parse_alter_domain(self),
            Some(Token::Role) => ddl::parse_alter_role(self, false),
            Some(Token::User) => ddl::parse_alter_role(self, true),
            Some(Token::Policy) => ddl::parse_alter_policy(self),
            // Extended DDL
            Some(Token::Group) => ddl::parse_alter_group(self),
            Some(Token::Tablespace) => ddl::parse_alter_tablespace(self),
            Some(Token::Aggregate) => ddl::parse_alter_aggregate(self),
            Some(Token::Operator) => ddl::parse_alter_operator(self),
            Some(Token::Collation) => ddl::parse_alter_collation(self),
            Some(Token::Conversion) => ddl::parse_alter_conversion(self),
            Some(Token::Foreign) => {
                self.advance()?; // consume FOREIGN
                if let Some(Token::Identifier(id)) = &self.current_token {
                    if id.to_uppercase() == "DATA" {
                        self.advance()?; // consume DATA
                        ddl::parse_alter_foreign_data_wrapper(self)
                    } else {
                        ddl::parse_alter_foreign_table(self)
                    }
                } else if matches!(&self.current_token, Some(Token::Table)) {
                    ddl::parse_alter_foreign_table(self)
                } else {
                    Err(ParseError {
                        message: "Expected DATA WRAPPER or TABLE after FOREIGN".to_string(),
                        position: self.position,
                        expected: vec!["DATA".to_string(), "TABLE".to_string()],
                        found: self.current_token.clone(),
                    })
                }
            }
            Some(Token::Server) => ddl::parse_alter_server(self),
            Some(Token::Publication) => ddl::parse_alter_publication(self),
            Some(Token::Subscription) => ddl::parse_alter_subscription(self),
            Some(Token::EventTrigger) => ddl::parse_alter_event_trigger(self),
            Some(Token::Statistics) => ddl::parse_alter_statistics(self),
            Some(Token::TextSearch) => {
                self.advance()?; // consume TEXT SEARCH
                if matches!(&self.current_token, Some(Token::Configuration)) {
                    ddl::parse_alter_text_search_configuration(self)
                } else if matches!(&self.current_token, Some(Token::Dictionary)) {
                    ddl::parse_alter_text_search_dictionary(self)
                } else if matches!(&self.current_token, Some(Token::Parser)) {
                    ddl::parse_alter_text_search_parser(self)
                } else if matches!(&self.current_token, Some(Token::Template)) {
                    ddl::parse_alter_text_search_template(self)
                } else {
                    Err(ParseError {
                        message: "Expected CONFIGURATION, DICTIONARY, PARSER, or TEMPLATE after TEXT SEARCH".to_string(),
                        position: self.position,
                        expected: vec!["CONFIGURATION".to_string(), "DICTIONARY".to_string(), "PARSER".to_string(), "TEMPLATE".to_string()],
                        found: self.current_token.clone(),
                    })
                }
            }
            Some(Token::Language) => ddl::parse_alter_language(self),

            Some(token) => Err(ParseError {
                message: format!("Unexpected token after ALTER: {token:?}"),
                position: self.position,
                expected: vec![
                    "TABLE".to_string(),
                    "SEQUENCE".to_string(),
                    "TYPE".to_string(),
                    "DOMAIN".to_string(),
                    "ROLE".to_string(),
                    "USER".to_string(),
                    "POLICY".to_string(),
                ],
                found: Some(token.clone()),
            }),

            None => Err(ParseError {
                message: "Expected object type after ALTER".to_string(),
                position: self.position,
                expected: vec![
                    "TABLE".to_string(),
                    "SEQUENCE".to_string(),
                    "TYPE".to_string(),
                    "DOMAIN".to_string(),
                    "ROLE".to_string(),
                    "USER".to_string(),
                    "POLICY".to_string(),
                ],
                found: None,
            }),
        }
    }

    /// Parse DROP statements
    fn parse_drop_statement(&mut self) -> ParseResult<Statement> {
        self.expect(Token::Drop)?;

        match &self.current_token {
            Some(Token::Database) => ddl::parse_drop_database(self),
            Some(Token::Table) => ddl::parse_drop_table(self),
            Some(Token::Index) => ddl::parse_drop_index(self),
            Some(Token::View) => ddl::parse_drop_view(self),
            Some(Token::Schema) => ddl::parse_drop_schema(self),
            Some(Token::Extension) => ddl::parse_drop_extension(self),
            Some(Token::Trigger) => ddl::parse_drop_trigger(self),
            Some(Token::Sequence) => ddl::parse_drop_sequence(self),
            Some(Token::Type) => ddl::parse_drop_type(self),
            Some(Token::Domain) => ddl::parse_drop_domain(self),
            Some(Token::Role) => ddl::parse_drop_role(self, false),
            Some(Token::User) => ddl::parse_drop_role(self, true),
            Some(Token::Policy) => ddl::parse_drop_policy(self),
            Some(Token::Rule) => ddl::parse_drop_rule(self),
            // Extended DDL
            Some(Token::Group) => ddl::parse_drop_group(self),
            Some(Token::Tablespace) => ddl::parse_drop_tablespace(self),
            Some(Token::Aggregate) => ddl::parse_drop_aggregate(self),
            Some(Token::Operator) => ddl::parse_drop_operator(self),
            Some(Token::Cast) => ddl::parse_drop_cast(self),
            Some(Token::Collation) => ddl::parse_drop_collation(self),
            Some(Token::Conversion) => ddl::parse_drop_conversion(self),
            Some(Token::Foreign) => {
                self.advance()?; // consume FOREIGN
                if let Some(Token::Identifier(id)) = &self.current_token {
                    if id.to_uppercase() == "DATA" {
                        self.advance()?; // consume DATA
                        ddl::parse_drop_foreign_data_wrapper(self)
                    } else {
                        ddl::parse_drop_foreign_table(self)
                    }
                } else if matches!(&self.current_token, Some(Token::Table)) {
                    ddl::parse_drop_foreign_table(self)
                } else {
                    Err(ParseError {
                        message: "Expected DATA WRAPPER or TABLE after FOREIGN".to_string(),
                        position: self.position,
                        expected: vec!["DATA".to_string(), "TABLE".to_string()],
                        found: self.current_token.clone(),
                    })
                }
            },
            Some(Token::Server) => ddl::parse_drop_server(self),
            Some(Token::Publication) => ddl::parse_drop_publication(self),
            Some(Token::Subscription) => ddl::parse_drop_subscription(self),
            Some(Token::EventTrigger) => ddl::parse_drop_event_trigger(self),
            Some(Token::AccessMethod) => ddl::parse_drop_access_method(self),
            Some(Token::Statistics) => ddl::parse_drop_statistics(self),
            Some(Token::TextSearch) => {
                self.advance()?; // consume TEXT SEARCH
                if matches!(&self.current_token, Some(Token::Configuration)) {
                    ddl::parse_drop_text_search_configuration(self)
                } else if matches!(&self.current_token, Some(Token::Dictionary)) {
                    ddl::parse_drop_text_search_dictionary(self)
                } else if matches!(&self.current_token, Some(Token::Parser)) {
                    ddl::parse_drop_text_search_parser(self)
                } else if matches!(&self.current_token, Some(Token::Template)) {
                    ddl::parse_drop_text_search_template(self)
                } else {
                    Err(ParseError {
                        message: "Expected CONFIGURATION, DICTIONARY, PARSER, or TEMPLATE after TEXT SEARCH".to_string(),
                        position: self.position,
                        expected: vec!["CONFIGURATION".to_string(), "DICTIONARY".to_string(), "PARSER".to_string(), "TEMPLATE".to_string()],
                        found: self.current_token.clone(),
                    })
                }
            },
            Some(Token::Transform) => ddl::parse_drop_transform(self),
            Some(Token::Language) => ddl::parse_drop_language(self),
            Some(Token::Function) => ddl::parse_drop_function(self),
            Some(Token::Procedure) => ddl::parse_drop_procedure(self),
            Some(Token::Routine) => ddl::parse_drop_routine(self),
            Some(Token::Owned) => ddl::parse_drop_owned(self),

            Some(token) => Err(ParseError {
                message: format!("Unexpected token after DROP: {token:?}"),
                position: self.position,
                expected: vec![
                    "DATABASE".to_string(),
                    "TABLE".to_string(),
                    "INDEX".to_string(),
                    "VIEW".to_string(),
                    "SCHEMA".to_string(),
                    "EXTENSION".to_string(),
                    "TRIGGER".to_string(),
                    "SEQUENCE".to_string(),
                    "TYPE".to_string(),
                    "DOMAIN".to_string(),
                    "ROLE".to_string(),
                    "USER".to_string(),
                    "POLICY".to_string(),
                    "RULE".to_string(),
                ],
                found: Some(token.clone()),
            }),

            None => Err(ParseError {
                message: "Expected object type after DROP".to_string(),
                position: self.position,
                expected: vec![
                    "DATABASE, TABLE, INDEX, VIEW, SCHEMA, EXTENSION, TRIGGER, SEQUENCE, TYPE, DOMAIN, ROLE, USER, POLICY, or RULE"
                        .to_string(),
                ],
                found: None,
            }),
        }
    }

    // DML statement implementations
    fn parse_select_statement(&mut self) -> ParseResult<Statement> {
        dml::parse_select(self)
    }

    fn parse_insert_statement(&mut self) -> ParseResult<Statement> {
        dml::parse_insert(self)
    }

    fn parse_update_statement(&mut self) -> ParseResult<Statement> {
        dml::parse_update(self)
    }

    fn parse_delete_statement(&mut self) -> ParseResult<Statement> {
        dml::parse_delete(self)
    }

    fn parse_merge_statement(&mut self) -> ParseResult<Statement> {
        dml::parse_merge(self)
    }

    fn parse_copy_statement(&mut self) -> ParseResult<Statement> {
        dml::parse_copy(self)
    }

    fn parse_grant_statement(&mut self) -> ParseResult<Statement> {
        dcl::parse_grant(self)
    }

    fn parse_revoke_statement(&mut self) -> ParseResult<Statement> {
        dcl::parse_revoke(self)
    }

    fn parse_begin_statement(&mut self) -> ParseResult<Statement> {
        tcl::parse_begin(self)
    }

    fn parse_commit_statement(&mut self) -> ParseResult<Statement> {
        tcl::parse_commit(self)
    }

    fn parse_rollback_statement(&mut self) -> ParseResult<Statement> {
        tcl::parse_rollback(self)
    }

    fn parse_savepoint_statement(&mut self) -> ParseResult<Statement> {
        tcl::parse_savepoint(self)
    }

    fn parse_set_statement(&mut self) -> ParseResult<Statement> {
        self.expect(Token::Set)?;

        // Parse variable name
        let variable = if let Some(Token::Identifier(name)) = &self.current_token {
            let var_name = name.clone();
            self.advance()?;
            var_name
        } else {
            return Err(ParseError {
                message: "Expected variable name after SET".to_string(),
                position: self.position,
                expected: vec!["variable name".to_string()],
                found: self.current_token.clone(),
            });
        };

        // Optional TO or =
        if self.matches(&[Token::To, Token::Equal]) {
            self.advance()?;
        }

        // Parse value(s)
        let mut values = Vec::new();
        loop {
            let expr = utilities::parse_expression(self)?;
            values.push(expr);

            if self.matches(&[Token::Comma]) {
                self.advance()?;
            } else {
                break;
            }
        }

        Ok(Statement::Set(
            crate::protocols::postgres_wire::sql::ast::SetStatement {
                variable,
                value: values,
            },
        ))
    }
}

impl Default for SqlParser {
    fn default() -> Self {
        Self::new()
    }
}
