//! PL/pgSQL: variables, conditionals, loops and `RETURN`.
//!
//! Triggers could already run a `$$BEGIN ... END$$` body, but only as a list
//! of SQL statements — there was no way to declare a variable, branch, or
//! loop. `DO $$ ... $$` and `CREATE FUNCTION ... LANGUAGE plpgsql` were worse
//! than absent: both answered "Command completed successfully" and ran
//! nothing, so a block that should have inserted a row reported success and
//! inserted nothing.
//!
//! The parser here is pure — text in, [`Block`] out — so it is tested without
//! a server. Execution is behind [`PlPgSqlHost`], which the query engine
//! implements; the interpreter never touches storage itself.
//!
//! # Expressions are evaluated by the SQL engine
//!
//! Nothing here evaluates arithmetic or comparisons. An expression is captured
//! as tokens, variable references are substituted with their values, and the
//! result is handed to the SQL engine as `SELECT <expr>`. That keeps one
//! implementation of every operator and function rather than a second one that
//! would drift from it.

use std::collections::HashMap;

use async_trait::async_trait;

use crate::protocols::error::{ProtocolError, ProtocolResult};

/// One piece of PL/pgSQL source.
///
/// Text is kept with its quotes so an expression can be rendered back exactly
/// as written, which is what makes variable substitution safe: a name inside a
/// string literal is a [`Tok::Str`] and is never mistaken for a reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    /// An identifier or keyword.
    Word(String),
    /// A quoted string, stored with its surrounding quotes.
    Str(String),
    /// A number.
    Num(String),
    /// Punctuation or an operator.
    Sym(String),
}

impl Tok {
    /// The token as it appeared in the source.
    fn text(&self) -> &str {
        match self {
            Tok::Word(t) | Tok::Str(t) | Tok::Num(t) | Tok::Sym(t) => t,
        }
    }

    /// Whether this word matches `keyword`, ignoring case.
    fn is_word(&self, keyword: &str) -> bool {
        matches!(self, Tok::Word(w) if w.eq_ignore_ascii_case(keyword))
    }
}

/// Split PL/pgSQL source into tokens.
///
/// Comments are dropped. An unterminated string is not an error here: it
/// becomes a token running to end of input, and the SQL engine rejects it with
/// a better message than this lexer could give.
#[must_use]
pub fn lex(source: &str) -> Vec<Tok> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c.is_whitespace() {
            i += 1;
        } else if c == '-' && chars.get(i + 1) == Some(&'-') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
        } else if c == '\'' || c == '"' {
            let quote = c;
            let start = i;
            i += 1;
            while i < chars.len() {
                if chars[i] == quote {
                    // A doubled quote is an escaped quote, not the end.
                    if chars.get(i + 1) == Some(&quote) {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            tokens.push(Tok::Str(chars[start..i].iter().collect()));
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                // `1..10` is a range, not a decimal point.
                if chars[i] == '.' && chars.get(i + 1) == Some(&'.') {
                    break;
                }
                i += 1;
            }
            tokens.push(Tok::Num(chars[start..i].iter().collect()));
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            tokens.push(Tok::Word(chars[start..i].iter().collect()));
        } else {
            // Two-character operators must be matched before single ones, or
            // `:=` lexes as `:` then `=` and an assignment reads as a
            // comparison.
            let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
            if matches!(two.as_str(), ":=" | ".." | "<=" | ">=" | "<>" | "!=" | "||") {
                tokens.push(Tok::Sym(two));
                i += 2;
            } else {
                tokens.push(Tok::Sym(c.to_string()));
                i += 1;
            }
        }
    }

    tokens
}

/// An expression, kept as tokens so variables can be substituted precisely.
pub type Expr = Vec<Tok>;

/// Where a declaration's type comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TypeSource {
    /// Written out: `INTEGER`, `TEXT`, `NUMERIC(10,2)`.
    Named(String),
    /// `table.column%TYPE` — whatever that column is declared as.
    LikeColumn {
        /// The table holding the column.
        table: String,
        /// The column whose type is borrowed.
        column: String,
    },
    /// `table%ROWTYPE` — a record shaped like one of the table's rows.
    LikeRow {
        /// The table whose shape is borrowed.
        table: String,
    },
    /// `CURSOR FOR <query>`.
    Cursor {
        /// The query the cursor runs when opened.
        query: Expr,
    },
}

/// A variable declared in a `DECLARE` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    /// The variable's name, folded to lower case.
    pub name: String,
    /// Where its type comes from.
    pub sql_type: TypeSource,
    /// The expression giving its initial value, if it has one.
    pub default: Option<Expr>,
}

/// One PL/pgSQL statement.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Stmt {
    /// `name := expr;`
    Assign {
        /// The variable being written.
        name: String,
        /// The value to write.
        value: Expr,
    },
    /// `IF ... THEN ... ELSIF ... ELSE ... END IF;`
    If {
        /// Each `(condition, body)` in order; the first true one runs.
        branches: Vec<(Expr, Vec<Stmt>)>,
        /// The `ELSE` body, empty when there is none.
        otherwise: Vec<Stmt>,
    },
    /// `WHILE cond LOOP ... END LOOP;`
    While {
        /// Runs while this holds.
        condition: Expr,
        /// The loop body.
        body: Vec<Stmt>,
    },
    /// `FOR v IN [REVERSE] low..high LOOP ... END LOOP;`
    ForRange {
        /// The loop variable, visible only inside the body.
        name: String,
        /// The low bound — still the low bound when `REVERSE` is given.
        from: Expr,
        /// The high bound.
        to: Expr,
        /// Whether to count down.
        reverse: bool,
        /// The loop body.
        body: Vec<Stmt>,
    },
    /// `LOOP ... END LOOP;` — exited only by `EXIT`.
    Loop {
        /// The loop body.
        body: Vec<Stmt>,
    },
    /// `EXIT [WHEN cond];`
    Exit {
        /// Leave the loop only when this holds; always when absent.
        when: Option<Expr>,
    },
    /// `CONTINUE [WHEN cond];`
    Continue {
        /// Skip to the next iteration only when this holds; always when absent.
        when: Option<Expr>,
    },
    /// `RETURN [expr];`
    Return {
        /// The value to return, if any.
        value: Option<Expr>,
    },
    /// `RAISE [level] 'message';`
    Raise {
        /// The message, already unquoted.
        message: String,
        /// Whether this aborts — `EXCEPTION` does, `NOTICE` and friends do not.
        aborts: bool,
    },
    /// `FOR rec IN SELECT ... LOOP ... END LOOP;`
    ForQuery {
        /// The record variable; its columns are read as `rec.column`.
        name: String,
        /// The query whose rows are iterated.
        query: Expr,
        /// The loop body.
        body: Vec<Stmt>,
    },
    /// `RETURN NEXT expr;` — append one value to the result.
    ReturnNext {
        /// The value to append.
        value: Expr,
    },
    /// `OPEN c;`
    OpenCursor {
        /// The cursor to open.
        name: String,
    },
    /// `FETCH [NEXT FROM] c INTO var[, var...];`
    Fetch {
        /// The cursor to read from.
        name: String,
        /// Where to put the row's columns, in order.
        targets: Vec<String>,
    },
    /// `CLOSE c;`
    CloseCursor {
        /// The cursor to close.
        name: String,
    },
    /// `FOR rec IN c LOOP ... END LOOP;` over an open-able cursor.
    ForCursor {
        /// The record variable.
        name: String,
        /// The cursor iterated.
        cursor: String,
        /// The loop body.
        body: Vec<Stmt>,
    },
    /// `RETURN QUERY SELECT ...;` — append a query's rows to the result.
    ReturnQuery {
        /// The query whose rows are returned.
        query: Expr,
    },
    /// A nested `BEGIN ... [EXCEPTION ...] END;`.
    Nested {
        /// The inner block.
        block: Box<Block>,
    },
    /// `SELECT expr INTO var;`
    SelectInto {
        /// The expression whose value is stored.
        value: Expr,
        /// The variable to store it in.
        target: String,
    },
    /// Any SQL statement, run for its effect.
    Sql(Expr),
    /// `NULL;` — does nothing, and is how an empty branch is written.
    Nothing,
}

/// An `EXCEPTION WHEN ... THEN ...` arm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handler {
    /// The conditions it catches, upper-cased. `OTHERS` catches everything.
    pub conditions: Vec<String>,
    /// What to run when it catches.
    pub body: Vec<Stmt>,
}

/// A parsed PL/pgSQL block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// Variables declared before `BEGIN`.
    pub declarations: Vec<Declaration>,
    /// The statements between `BEGIN` and `END`.
    pub body: Vec<Stmt>,
    /// `EXCEPTION` arms, empty when the block has none.
    pub handlers: Vec<Handler>,
}

/// Strip a dollar-quoted wrapper, if the body has one.
///
/// `$$ ... $$` and `$tag$ ... $tag$` both carry a body through a statement
/// splitter; neither is part of the program.
#[must_use]
pub fn strip_dollar_quotes(body: &str) -> &str {
    let trimmed = body.trim();
    let Some(rest) = trimmed.strip_prefix('$') else {
        return trimmed;
    };
    let Some(tag_end) = rest.find('$') else {
        return trimmed;
    };
    let tag = &rest[..tag_end];
    let opener_len = tag.len() + 2;
    let closer = format!("${tag}$");
    trimmed
        .get(opener_len..)
        .and_then(|inner| inner.strip_suffix(&closer))
        .map_or(trimmed, str::trim)
}

/// Whether a body needs the PL/pgSQL interpreter rather than plain SQL.
///
/// A trigger body that is only SQL statements keeps its existing path; this
/// asks whether anything in it could not be run that way.
#[must_use]
pub fn needs_interpreter(body: &str) -> bool {
    lex(strip_dollar_quotes(body)).iter().any(|token| {
        [
            "DECLARE", "IF", "LOOP", "WHILE", "RETURN", "EXIT", "CONTINUE",
        ]
        .iter()
        .any(|keyword| token.is_word(keyword))
    })
}

/// Parse a PL/pgSQL block.
///
/// The body may be dollar-quoted and may begin with `DECLARE`.
///
/// # Errors
/// Returns an error when the block is not well formed — a missing `BEGIN`,
/// an `IF` with no `END IF`, or a statement the parser does not recognise.
pub fn parse(body: &str) -> ProtocolResult<Block> {
    let tokens = lex(strip_dollar_quotes(body));
    let mut parser = Parser { tokens, pos: 0 };
    parser.block()
}

struct Parser {
    tokens: Vec<Tok>,
    pos: usize,
}

/// Keywords that end a statement list.
const BLOCK_ENDERS: [&str; 5] = ["END", "ELSIF", "ELSEIF", "ELSE", "EXCEPTION"];

fn error(message: impl Into<String>) -> ProtocolError {
    ProtocolError::PostgresError(message.into())
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn peek_is(&self, keyword: &str) -> bool {
        self.peek().is_some_and(|t| t.is_word(keyword))
    }

    fn next(&mut self) -> Option<Tok> {
        let token = self.tokens.get(self.pos).cloned();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    /// Consume `keyword` if it is next, reporting whether it was.
    fn eat(&mut self, keyword: &str) -> bool {
        if self.peek_is(keyword) {
            self.pos += 1;
            return true;
        }
        false
    }

    fn eat_sym(&mut self, symbol: &str) -> bool {
        if self
            .peek()
            .is_some_and(|t| matches!(t, Tok::Sym(s) if s == symbol))
        {
            self.pos += 1;
            return true;
        }
        false
    }

    fn expect(&mut self, keyword: &str) -> ProtocolResult<()> {
        if self.eat(keyword) {
            return Ok(());
        }
        Err(error(format!(
            "expected {keyword} in PL/pgSQL block, found {}",
            self.peek().map_or("end of block", Tok::text)
        )))
    }

    fn expect_sym(&mut self, symbol: &str) -> ProtocolResult<()> {
        if self.eat_sym(symbol) {
            return Ok(());
        }
        Err(error(format!(
            "expected {symbol} in PL/pgSQL block, found {}",
            self.peek().map_or("end of block", Tok::text)
        )))
    }

    fn identifier(&mut self) -> ProtocolResult<String> {
        match self.next() {
            Some(Tok::Word(name)) => Ok(name.to_lowercase()),
            other => Err(error(format!(
                "expected a name in PL/pgSQL block, found {}",
                other.as_ref().map_or("end of block", Tok::text)
            ))),
        }
    }

    /// Collect tokens up to — but not including — any of `stops`.
    ///
    /// Parentheses are tracked so a stop word inside a call, such as the `end`
    /// of `date_trunc('day', end)`, does not terminate the expression early.
    fn expression(&mut self, stops: &[&str]) -> Expr {
        let mut depth = 0i32;
        let mut collected = Vec::new();
        while let Some(token) = self.peek() {
            match token {
                Tok::Sym(s) if s == "(" => depth += 1,
                Tok::Sym(s) if s == ")" => depth -= 1,
                _ => {}
            }
            let stop = depth <= 0
                && stops.iter().any(|s| {
                    if s.chars().next().is_some_and(char::is_alphabetic) {
                        token.is_word(s)
                    } else {
                        matches!(token, Tok::Sym(sym) if sym == s)
                    }
                });
            if stop {
                break;
            }
            collected.push(token.clone());
            self.pos += 1;
        }
        collected
    }

    fn block(&mut self) -> ProtocolResult<Block> {
        let declarations = if self.eat("DECLARE") {
            self.declarations()?
        } else {
            Vec::new()
        };
        self.expect("BEGIN")?;
        let body = self.statements()?;
        let handlers = self.handlers()?;
        self.expect("END")?;
        Ok(Block {
            declarations,
            body,
            handlers,
        })
    }

    /// `EXCEPTION WHEN a OR b THEN ... WHEN OTHERS THEN ...`
    fn handlers(&mut self) -> ProtocolResult<Vec<Handler>> {
        if !self.eat("EXCEPTION") {
            return Ok(Vec::new());
        }
        let mut handlers = Vec::new();
        while self.eat("WHEN") {
            let mut conditions = vec![self.identifier()?.to_uppercase()];
            while self.eat("OR") {
                conditions.push(self.identifier()?.to_uppercase());
            }
            self.expect("THEN")?;
            handlers.push(Handler {
                conditions,
                body: self.statements()?,
            });
        }
        if handlers.is_empty() {
            return Err(error("EXCEPTION with no WHEN arm"));
        }
        Ok(handlers)
    }

    fn declarations(&mut self) -> ProtocolResult<Vec<Declaration>> {
        let mut declared = Vec::new();
        while !self.peek_is("BEGIN") {
            if self.peek().is_none() {
                return Err(error("PL/pgSQL block has DECLARE but no BEGIN"));
            }
            let name = self.identifier()?;

            // `c CURSOR FOR SELECT ...` declares a cursor rather than a value.
            if self.eat("CURSOR") {
                self.expect("FOR")?;
                let query = self.expression(&[";"]);
                self.expect_sym(";")?;
                declared.push(Declaration {
                    name,
                    sql_type: TypeSource::Cursor { query },
                    default: None,
                });
                continue;
            }

            // The type may be several words (`DOUBLE PRECISION`) or carry a
            // precision (`NUMERIC(10,2)`); it is kept only to decide whether a
            // value is quoted when substituted.
            let type_tokens = self.expression(&[":=", ";"]);
            let sql_type = Self::type_source(&type_tokens);
            let default = self.eat_sym(":=").then(|| self.expression(&[";"]));
            self.expect_sym(";")?;
            declared.push(Declaration {
                name,
                sql_type,
                default,
            });
        }
        Ok(declared)
    }

    /// Read a declaration's type, which may borrow one.
    ///
    /// `%TYPE` and `%ROWTYPE` are resolved when the block runs, not here: the
    /// table's schema is the server's to answer, and the parser has no server.
    fn type_source(tokens: &[Tok]) -> TypeSource {
        let percent = tokens
            .iter()
            .position(|t| matches!(t, Tok::Sym(s) if s == "%"));

        if let Some(at) = percent {
            let borrowed = tokens.get(at + 1).map(Tok::text).unwrap_or_default();
            let path: Vec<&str> = tokens[..at]
                .iter()
                .filter(|t| matches!(t, Tok::Word(_)))
                .map(Tok::text)
                .collect();

            if borrowed.eq_ignore_ascii_case("ROWTYPE") {
                if let Some(table) = path.first() {
                    return TypeSource::LikeRow {
                        table: (*table).to_lowercase(),
                    };
                }
            }
            if borrowed.eq_ignore_ascii_case("TYPE") {
                if let [table, column] = path.as_slice() {
                    return TypeSource::LikeColumn {
                        table: (*table).to_lowercase(),
                        column: (*column).to_lowercase(),
                    };
                }
            }
        }

        TypeSource::Named(
            tokens
                .iter()
                .map(Tok::text)
                .collect::<Vec<_>>()
                .join(" ")
                .to_uppercase(),
        )
    }

    fn statements(&mut self) -> ProtocolResult<Vec<Stmt>> {
        let mut body = Vec::new();
        loop {
            // A stray semicolon between statements is not an error.
            while self.eat_sym(";") {}
            match self.peek() {
                None => return Ok(body),
                Some(token) if BLOCK_ENDERS.iter().any(|k| token.is_word(k)) => return Ok(body),
                Some(_) => body.push(self.statement()?),
            }
        }
    }

    fn statement(&mut self) -> ProtocolResult<Stmt> {
        if self.eat("IF") {
            return self.if_statement();
        }
        if self.eat("WHILE") {
            let condition = self.expression(&["LOOP"]);
            self.expect("LOOP")?;
            let body = self.statements()?;
            self.end_loop()?;
            return Ok(Stmt::While { condition, body });
        }
        if self.eat("FOR") {
            return self.for_statement();
        }
        if self.eat("LOOP") {
            let body = self.statements()?;
            self.end_loop()?;
            return Ok(Stmt::Loop { body });
        }
        if self.eat("EXIT") {
            let when = self.exit_condition();
            self.eat_sym(";");
            return Ok(Stmt::Exit { when });
        }
        if self.eat("CONTINUE") {
            let when = self.exit_condition();
            self.eat_sym(";");
            return Ok(Stmt::Continue { when });
        }
        if self.peek_is("BEGIN") || (self.peek_is("DECLARE") && self.nested_declare()) {
            let block = self.block()?;
            self.eat_sym(";");
            return Ok(Stmt::Nested {
                block: Box::new(block),
            });
        }
        if self.eat("OPEN") {
            let name = self.identifier()?;
            self.eat_sym(";");
            return Ok(Stmt::OpenCursor { name });
        }
        if self.eat("CLOSE") {
            let name = self.identifier()?;
            self.eat_sym(";");
            return Ok(Stmt::CloseCursor { name });
        }
        if self.eat("FETCH") {
            // `FETCH c INTO v` and `FETCH NEXT FROM c INTO v` are the same
            // thing written two ways.
            self.eat("NEXT");
            self.eat("FROM");
            let name = self.identifier()?;
            self.expect("INTO")?;
            let mut targets = vec![self.identifier()?];
            while self.eat_sym(",") {
                targets.push(self.identifier()?);
            }
            self.eat_sym(";");
            return Ok(Stmt::Fetch { name, targets });
        }
        if self.eat("RETURN") {
            if self.eat("NEXT") {
                let value = self.expression(&[";"]);
                self.eat_sym(";");
                return Ok(Stmt::ReturnNext { value });
            }
            if self.eat("QUERY") {
                let query = self.expression(&[";"]);
                self.eat_sym(";");
                return Ok(Stmt::ReturnQuery { query });
            }
            let value = self.expression(&[";"]);
            self.eat_sym(";");
            return Ok(Stmt::Return {
                value: (!value.is_empty()).then_some(value),
            });
        }
        if self.eat("RAISE") {
            return self.raise_statement();
        }
        if self.peek_is("NULL") {
            // Only when it stands alone: `NULL` also begins `NULL::text`.
            let save = self.pos;
            self.pos += 1;
            if self.eat_sym(";") {
                return Ok(Stmt::Nothing);
            }
            self.pos = save;
        }
        if self.eat("PERFORM") {
            // `PERFORM expr` is `SELECT expr` with the result discarded.
            let mut value = vec![Tok::Word("SELECT".to_string())];
            value.extend(self.expression(&[";"]));
            self.eat_sym(";");
            return Ok(Stmt::Sql(value));
        }

        // `name := expr;` and `record.field := expr;` — an assignment is the
        // only statement whose second or fourth token is `:=`, so a little
        // lookahead tells it from a SQL statement starting with a word.
        let assigns_at = |offset: usize| matches!(self.tokens.get(self.pos + offset), Some(Tok::Sym(s)) if s == ":=");
        if matches!(self.peek(), Some(Tok::Word(_))) && assigns_at(1) {
            let name = self.identifier()?;
            self.expect_sym(":=")?;
            let value = self.expression(&[";"]);
            self.eat_sym(";");
            return Ok(Stmt::Assign { name, value });
        }
        if matches!(self.peek(), Some(Tok::Word(_)))
            && matches!(self.tokens.get(self.pos + 1), Some(Tok::Sym(s)) if s == ".")
            && matches!(self.tokens.get(self.pos + 2), Some(Tok::Word(_)))
            && assigns_at(3)
        {
            let record = self.identifier()?;
            self.expect_sym(".")?;
            let field = self.identifier()?;
            self.expect_sym(":=")?;
            let value = self.expression(&[";"]);
            self.eat_sym(";");
            // Fields live in the scope under their dotted name, which is what
            // `render` looks up when the field is read back.
            return Ok(Stmt::Assign {
                name: format!("{record}.{field}"),
                value,
            });
        }

        // Anything else is SQL. `SELECT ... INTO var` assigns rather than
        // returning, which is how a block reads a value out of a table.
        let statement = self.expression(&[";"]);
        self.eat_sym(";");
        if statement.is_empty() {
            return Ok(Stmt::Nothing);
        }
        Ok(split_select_into(statement))
    }

    /// Whether a `DECLARE` here opens a nested block rather than being a stray
    /// keyword. A nested block always reaches a `BEGIN` before any `;`.
    fn nested_declare(&self) -> bool {
        self.tokens[self.pos..]
            .iter()
            .take_while(|t| !matches!(t, Tok::Sym(s) if s == ";"))
            .any(|t| t.is_word("BEGIN"))
    }

    /// `EXIT`/`CONTINUE` take an optional `WHEN`; a bare one always fires.
    fn exit_condition(&mut self) -> Option<Expr> {
        self.eat("WHEN").then(|| self.expression(&[";"]))
    }

    fn end_loop(&mut self) -> ProtocolResult<()> {
        self.expect("END")?;
        self.expect("LOOP")?;
        self.eat_sym(";");
        Ok(())
    }

    fn if_statement(&mut self) -> ProtocolResult<Stmt> {
        let mut branches = Vec::new();
        let mut otherwise = Vec::new();

        let condition = self.expression(&["THEN"]);
        self.expect("THEN")?;
        branches.push((condition, self.statements()?));

        loop {
            if self.eat("ELSIF") || self.eat("ELSEIF") {
                let condition = self.expression(&["THEN"]);
                self.expect("THEN")?;
                branches.push((condition, self.statements()?));
                continue;
            }
            if self.eat("ELSE") {
                otherwise = self.statements()?;
            }
            break;
        }

        self.expect("END")?;
        self.expect("IF")?;
        self.eat_sym(";");
        Ok(Stmt::If {
            branches,
            otherwise,
        })
    }

    fn for_statement(&mut self) -> ProtocolResult<Stmt> {
        let name = self.identifier()?;
        self.expect("IN")?;
        let reverse = self.eat("REVERSE");
        // `FOR rec IN SELECT ...` iterates a query's rows; `FOR i IN 1..10`
        // counts. The keyword after IN says which.
        if !reverse && self.peek_is("SELECT") {
            let query = self.expression(&["LOOP"]);
            self.expect("LOOP")?;
            let body = self.statements()?;
            self.end_loop()?;
            return Ok(Stmt::ForQuery { name, query, body });
        }
        // `FOR r IN c LOOP` over a declared cursor: a bare name then LOOP.
        if !reverse
            && matches!(self.peek(), Some(Tok::Word(_)))
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|t| t.is_word("LOOP"))
        {
            let cursor = self.identifier()?;
            self.expect("LOOP")?;
            let body = self.statements()?;
            self.end_loop()?;
            return Ok(Stmt::ForCursor { name, cursor, body });
        }
        let from = self.expression(&["..", "LOOP"]);
        if !self.eat_sym("..") {
            return Err(error(
                "a FOR loop takes an integer range (FOR i IN 1..10 LOOP) or a query \
                 (FOR r IN SELECT ... LOOP)"
                    .to_string(),
            ));
        }
        let to = self.expression(&["LOOP"]);
        self.expect("LOOP")?;
        let body = self.statements()?;
        self.end_loop()?;
        Ok(Stmt::ForRange {
            name,
            from,
            to,
            reverse,
            body,
        })
    }

    fn raise_statement(&mut self) -> ProtocolResult<Stmt> {
        // `RAISE [level] 'message'`. Only EXCEPTION aborts; the rest are
        // reports, and treating them all as aborts would turn a NOTICE into a
        // failed statement.
        let mut aborts = true;
        if let Some(Tok::Word(word)) = self.peek() {
            let level = word.to_uppercase();
            if matches!(
                level.as_str(),
                "EXCEPTION" | "NOTICE" | "WARNING" | "INFO" | "LOG" | "DEBUG"
            ) {
                aborts = level == "EXCEPTION";
                self.pos += 1;
            }
        }
        let parts = self.expression(&[";"]);
        self.eat_sym(";");
        let message = parts
            .first()
            .map(|token| match token {
                Tok::Str(text) => unquote(text),
                other => other.text().to_string(),
            })
            .unwrap_or_else(|| "raised by a PL/pgSQL block".to_string());
        Ok(Stmt::Raise { message, aborts })
    }
}

/// Turn `SELECT <expr> INTO <var>` into an assignment.
///
/// Any other statement is returned unchanged.
fn split_select_into(statement: Expr) -> Stmt {
    if !statement.first().is_some_and(|t| t.is_word("SELECT")) {
        return Stmt::Sql(statement);
    }
    // The last `INTO` at paren depth zero: a subquery may contain its own.
    let mut depth = 0i32;
    let mut into_at = None;
    for (index, token) in statement.iter().enumerate() {
        match token {
            Tok::Sym(s) if s == "(" => depth += 1,
            Tok::Sym(s) if s == ")" => depth -= 1,
            _ if depth == 0 && token.is_word("INTO") => into_at = Some(index),
            _ => {}
        }
    }
    let Some(index) = into_at else {
        return Stmt::Sql(statement);
    };
    // `SELECT ... INTO t` with more than one name after INTO is a row
    // assignment, which is not supported; leave it as SQL so the engine
    // reports it rather than silently binding the first name.
    let Some([Tok::Word(target)]) = statement.get(index + 1..) else {
        return Stmt::Sql(statement);
    };
    Stmt::SelectInto {
        value: statement[1..index].to_vec(),
        target: target.to_lowercase(),
    }
}

/// Remove the quotes from a SQL string literal, undoubling escaped quotes.
fn unquote(text: &str) -> String {
    let inner = text
        .strip_prefix('\'')
        .and_then(|t| t.strip_suffix('\''))
        .unwrap_or(text);
    inner.replace("''", "'")
}

/// A variable's current value, with enough type information to render it back
/// into SQL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value {
    /// The value as text, or `None` for SQL `NULL`.
    pub text: Option<String>,
    /// Whether it must be quoted when substituted into an expression.
    pub quoted: bool,
}

impl Value {
    /// A value whose quoting is read off the value itself.
    ///
    /// Used where no declared type says: a `%TYPE` variable, or a row read
    /// from a query. Anything that parses as a number or a boolean is left
    /// bare, everything else is quoted.
    #[must_use]
    pub fn infer(text: Option<String>) -> Self {
        // `t` and `f` are not treated as booleans: unquoted they are
        // identifiers, and a text column holding "t" would be substituted as
        // one. Quoting them is wrong only for a boolean read out of a query,
        // which compares correctly either way.
        let quoted = text.as_deref().is_none_or(|text| {
            text.parse::<f64>().is_err()
                && !text.eq_ignore_ascii_case("true")
                && !text.eq_ignore_ascii_case("false")
        });
        Self { text, quoted }
    }

    /// A value of a declared type.
    #[must_use]
    pub fn typed(text: Option<String>, sql_type: &str) -> Self {
        if sql_type.trim().is_empty() {
            return Self::infer(text);
        }
        Self {
            text,
            quoted: needs_quoting(sql_type),
        }
    }

    /// A NULL of a declared type.
    fn null(sql_type: &str) -> Self {
        Self {
            text: None,
            quoted: needs_quoting(sql_type),
        }
    }

    /// Render for substitution into SQL.
    fn render(&self) -> String {
        match &self.text {
            None => "NULL".to_string(),
            Some(text) if self.quoted => format!("'{}'", text.replace('\'', "''")),
            Some(text) => text.clone(),
        }
    }
}

/// Whether a declared type's values must be quoted in SQL.
///
/// A number written as `'1'` still compares and adds correctly in PostgreSQL,
/// but quoting a number makes `v + 1` a text concatenation in some engines, so
/// the numeric types are rendered bare.
fn needs_quoting(sql_type: &str) -> bool {
    let bare = sql_type.split('(').next().unwrap_or(sql_type).trim();
    !matches!(
        bare,
        "INT"
            | "INT2"
            | "INT4"
            | "INT8"
            | "INTEGER"
            | "SMALLINT"
            | "BIGINT"
            | "NUMERIC"
            | "DECIMAL"
            | "REAL"
            | "DOUBLE"
            | "DOUBLE PRECISION"
            | "FLOAT"
            | "FLOAT4"
            | "FLOAT8"
            | "BOOL"
            | "BOOLEAN"
    )
}

/// A query's columns and rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rows {
    /// Column names, in order.
    pub columns: Vec<String>,
    /// One entry per row.
    pub rows: Vec<Vec<Option<String>>>,
}

/// What a block produced.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Returned {
    /// It ran off its end, or returned nothing.
    Nothing,
    /// `RETURN expr`.
    Scalar(Option<String>),
    /// One or more `RETURN QUERY`.
    Rows(Rows),
}

impl Returned {
    /// The single value this stands for, if it is one.
    #[must_use]
    pub fn scalar(&self) -> Option<String> {
        match self {
            Returned::Scalar(value) => value.clone(),
            Returned::Nothing | Returned::Rows(_) => None,
        }
    }
}

/// A declared cursor: the query it runs, and where it has got to.
#[derive(Debug, Clone, Default)]
pub struct Cursor {
    /// The query, rendered when the cursor is opened.
    pub query: Expr,
    /// Rows fetched when it was opened; `None` until then.
    pub rows: Option<Rows>,
    /// How many rows have been fetched.
    pub position: usize,
}

/// Everything a running block can see.
///
/// Cursors cannot live in the variable map: a cursor is not a value, and
/// substituting one into SQL would be meaningless.
#[derive(Debug, Clone, Default)]
pub struct State {
    /// Variables by name; a record's columns are keyed `record.column`.
    pub scope: HashMap<String, Value>,
    /// Cursors by name.
    pub cursors: HashMap<String, Cursor>,
}

impl State {
    /// Start from a set of bound arguments.
    #[must_use]
    pub fn with_arguments(scope: HashMap<String, Value>) -> Self {
        Self {
            scope,
            cursors: HashMap::new(),
        }
    }

    /// Record whether the last `FETCH` found a row, which `FOUND` reports.
    ///
    /// Spelled out rather than `t`/`f`: substituted bare into `EXIT WHEN NOT
    /// FOUND`, a `t` is an identifier, and the statement failed with
    /// `column "t" does not exist`.
    fn set_found(&mut self, found: bool) {
        self.scope.insert(
            "found".to_string(),
            Value {
                text: Some(if found { "TRUE" } else { "FALSE" }.to_string()),
                quoted: false,
            },
        );
    }
}

/// What the interpreter needs from the server.
///
/// Keeping this a trait is what lets the interpreter be tested without a
/// database: the tests below implement it over a `Vec`.
#[async_trait]
pub trait PlPgSqlHost: Send + Sync {
    /// Evaluate a scalar expression, already free of variable references.
    ///
    /// # Errors
    /// Returns whatever the SQL engine reports.
    async fn evaluate(&self, expression: &str) -> ProtocolResult<Option<String>>;

    /// Run a statement for its effect.
    ///
    /// # Errors
    /// Returns whatever the SQL engine reports.
    async fn run(&self, sql: &str) -> ProtocolResult<()>;

    /// Run a query and return its columns and rows.
    ///
    /// # Errors
    /// Returns whatever the SQL engine reports.
    async fn query(&self, sql: &str) -> ProtocolResult<Rows>;

    /// The declared type of a column, for `%TYPE`.
    ///
    /// Returning `None` means the column is unknown, and the variable falls
    /// back to taking its quoting from whatever it is assigned.
    ///
    /// # Errors
    /// Returns whatever the catalog lookup reports.
    async fn column_type(&self, table: &str, column: &str) -> ProtocolResult<Option<String>>;

    /// The column names of a table, for `%ROWTYPE`.
    ///
    /// # Errors
    /// Returns whatever the catalog lookup reports.
    async fn row_columns(&self, table: &str) -> ProtocolResult<Vec<String>>;

    /// Run a block so that a failure undoes only what the block wrote.
    ///
    /// This is what an `EXCEPTION` handler needs: the statements it protects
    /// must leave nothing behind when one of them fails. The default runs the
    /// block with no such protection and is only for hosts without
    /// transactions — a real one overrides it.
    ///
    /// # Errors
    /// Returns an error only when the rollback itself fails; a failure of the
    /// block is reported as the inner `Err`.
    async fn run_protected(
        &self,
        block: &Block,
        state: State,
    ) -> ProtocolResult<(Result<Returned, ProtocolError>, State)>;

    /// Report a non-aborting `RAISE`.
    fn notice(&self, message: &str) {
        tracing::info!(message, "PL/pgSQL notice");
    }
}

/// How many iterations a loop may run before the interpreter gives up.
///
/// PostgreSQL lets a loop run forever, which is the right answer for a
/// dedicated backend process. Here a statement that never finishes is a
/// statement that holds a connection and a share of the runtime, so a runaway
/// loop fails loudly instead — the same choice already made for recursive
/// CTEs.
const MAX_ITERATIONS: u64 = 10_000_000;

/// Why a statement list stopped.
enum Flow {
    /// Ran to the end.
    Normal,
    /// `EXIT` — leave the innermost loop.
    Exit,
    /// `CONTINUE` — start the innermost loop's next iteration.
    Continue,
    /// `RETURN` — leave the block with this value.
    Return(Option<String>),
}

/// Rows accumulated by `RETURN QUERY` in the block currently running.
///
/// A task-local rather than a threaded-through accumulator: `RETURN QUERY` can
/// appear inside a loop inside a nested block, and every level would otherwise
/// have to carry it.
type Collected = std::sync::Mutex<Rows>;

tokio::task_local! {
    static RETURNED_ROWS: std::sync::Arc<Collected>;
}

/// Run a parsed block.
///
/// Returns the value given to `RETURN`, or `None` if the block ran off its end.
///
/// # Errors
/// Returns an error when an expression cannot be evaluated, a statement fails,
/// a `RAISE EXCEPTION` fires, or a loop exceeds [`MAX_ITERATIONS`].
pub async fn execute(
    block: &Block,
    host: &dyn PlPgSqlHost,
    arguments: HashMap<String, Value>,
) -> ProtocolResult<Returned> {
    let collected = std::sync::Arc::new(Collected::default());
    let mut state = State::with_arguments(arguments);
    let outcome = RETURNED_ROWS
        .scope(std::sync::Arc::clone(&collected), async {
            execute_in(block, host, &mut state).await
        })
        .await?;

    // `RETURN QUERY` wins over falling off the end: a set-returning function
    // that appended rows returns those, not nothing.
    let rows = collected
        .lock()
        .map(|rows| rows.clone())
        .unwrap_or_default();
    Ok(match outcome {
        Returned::Nothing if !rows.rows.is_empty() || !rows.columns.is_empty() => {
            Returned::Rows(rows)
        }
        other => other,
    })
}

/// Run a block against an existing scope, without starting a new row
/// collector — the entry point for a nested block and for a handler.
///
/// # Errors
/// Returns whatever the block failed with, once no handler catches it.
pub async fn execute_in(
    block: &Block,
    host: &dyn PlPgSqlHost,
    state: &mut State,
) -> ProtocolResult<Returned> {
    for declaration in &block.declarations {
        declare(declaration, host, state).await?;
    }

    match run_statements(&block.body, host, state).await? {
        Flow::Return(value) => Ok(Returned::Scalar(value)),
        Flow::Normal | Flow::Exit | Flow::Continue => Ok(Returned::Nothing),
    }
}

/// Append rows to what the block is returning.
fn append_rows(fetched: &Rows) -> ProtocolResult<()> {
    RETURNED_ROWS
        .try_with(|collected| {
            if let Ok(mut collected) = collected.lock() {
                if collected.columns.is_empty() {
                    collected.columns = fetched.columns.clone();
                }
                collected.rows.extend(fetched.rows.clone());
            }
        })
        .map_err(|_| error("RETURN NEXT or RETURN QUERY outside a block"))
}

/// Bring one declaration into scope.
///
/// A `%TYPE` or `%ROWTYPE` is resolved here, against the server, because the
/// parser has no schema to ask.
async fn declare(
    declaration: &Declaration,
    host: &dyn PlPgSqlHost,
    state: &mut State,
) -> ProtocolResult<()> {
    let sql_type = match &declaration.sql_type {
        TypeSource::Named(name)
            if !crate::protocols::postgres_wire::plpgsql_function::is_builtin(name) =>
        {
            // A name that is not a built-in may be a composite type or a
            // table, either of which declares a record rather than a scalar:
            // its fields come into scope as `variable.field`.
            let fields = host.row_columns(name).await?;
            if !fields.is_empty() {
                for field in fields {
                    state.scope.insert(
                        format!("{}.{}", declaration.name, field.to_lowercase()),
                        Value::null(""),
                    );
                }
                return Ok(());
            }
            name.clone()
        }
        TypeSource::Named(name) => name.clone(),

        TypeSource::Cursor { query } => {
            state.cursors.insert(
                declaration.name.clone(),
                Cursor {
                    query: query.clone(),
                    rows: None,
                    position: 0,
                },
            );
            return Ok(());
        }

        TypeSource::LikeColumn { table, column } => {
            // An unknown column leaves the type unknown rather than guessing:
            // quoting is then read off whatever the variable is assigned.
            host.column_type(table, column).await?.unwrap_or_default()
        }

        TypeSource::LikeRow { table } => {
            // A row variable has no value of its own; its columns appear as
            // `name.column` once something assigns them.
            for column in host.row_columns(table).await? {
                state.scope.insert(
                    format!("{}.{}", declaration.name, column.to_lowercase()),
                    Value::null(""),
                );
            }
            return Ok(());
        }
    };

    let value = match &declaration.default {
        None => Value::null(&sql_type),
        Some(expression) => Value::typed(evaluate(host, expression, state).await?, &sql_type),
    };
    state.scope.insert(declaration.name.clone(), value);
    Ok(())
}

/// Whether a handler catches `error`.
///
/// A named condition matches its own SQLSTATE and no other, so
/// `WHEN unique_violation` catches a duplicate key and lets a missing table
/// through. A condition name this server does not define matches nothing
/// rather than everything.
fn catches(handler: &Handler, error: &ProtocolError) -> bool {
    let code = crate::protocols::postgres_wire::sqlstate::of(error);
    handler.conditions.iter().any(|condition| {
        crate::protocols::postgres_wire::sqlstate::condition_matches(condition, code)
    })
}

/// Substitute variables into an expression and render it as SQL.
fn render(expression: &Expr, state: &State) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(expression.len());
    let mut index = 0;

    while index < expression.len() {
        // `rec.column` is one reference, not a name followed by a field: a
        // record's columns are held in the scope under their dotted names.
        if let (Some(Tok::Word(record)), Some(Tok::Sym(dot)), Some(Tok::Word(field))) = (
            expression.get(index),
            expression.get(index + 1),
            expression.get(index + 2),
        ) {
            if dot == "." {
                let key = format!("{}.{}", record.to_lowercase(), field.to_lowercase());
                if let Some(value) = state.scope.get(&key) {
                    parts.push(value.render());
                    index += 3;
                    continue;
                }
            }
        }
        parts.push(match &expression[index] {
            Tok::Word(word) => state
                .scope
                .get(&word.to_lowercase())
                .map_or_else(|| word.clone(), Value::render),
            other => other.text().to_string(),
        });
        index += 1;
    }

    parts.join(" ")
}

async fn evaluate(
    host: &dyn PlPgSqlHost,
    expression: &Expr,
    state: &State,
) -> ProtocolResult<Option<String>> {
    host.evaluate(&render(expression, state)).await
}

/// Whether an evaluated expression counts as true.
fn is_true(value: Option<&String>) -> bool {
    value.is_some_and(|text| matches!(text.trim(), "t" | "true" | "TRUE" | "True" | "1"))
}

async fn condition_holds(
    host: &dyn PlPgSqlHost,
    expression: &Expr,
    state: &State,
) -> ProtocolResult<bool> {
    let value = evaluate(host, expression, state).await?;
    Ok(is_true(value.as_ref()))
}

/// Run statements until one diverts control.
async fn run_statements(
    body: &[Stmt],
    host: &dyn PlPgSqlHost,
    state: &mut State,
) -> ProtocolResult<Flow> {
    for statement in body {
        match Box::pin(run_statement(statement, host, state)).await? {
            Flow::Normal => {}
            diverted => return Ok(diverted),
        }
    }
    Ok(Flow::Normal)
}

/// Store a value under `name`, keeping the quoting its declaration asked for.
fn assign(state: &mut State, name: &str, text: Option<String>) {
    let quoted = state.scope.get(name).is_none_or(|existing| existing.quoted);
    state.scope.insert(name.to_string(), Value { text, quoted });
}

async fn run_statement(
    statement: &Stmt,
    host: &dyn PlPgSqlHost,
    state: &mut State,
) -> ProtocolResult<Flow> {
    match statement {
        Stmt::Nothing => Ok(Flow::Normal),

        Stmt::Assign { name, value } => {
            let evaluated = evaluate(host, value, state).await?;
            assign(state, name, evaluated);
            Ok(Flow::Normal)
        }

        Stmt::SelectInto { value, target } => {
            let evaluated = evaluate(host, value, state).await?;
            assign(state, target, evaluated);
            Ok(Flow::Normal)
        }

        Stmt::Sql(sql) => {
            host.run(&render(sql, state)).await?;
            Ok(Flow::Normal)
        }

        Stmt::Raise { message, aborts } => {
            if *aborts {
                // `P0001` is what PostgreSQL reports for an exception raised
                // by a procedure, and it is what `WHEN raise_exception`
                // matches. The message cannot say so; the code has to.
                return Err(ProtocolError::SqlState {
                    code: crate::protocols::postgres_wire::sqlstate::RAISE_EXCEPTION,
                    message: message.clone(),
                });
            }
            host.notice(message);
            Ok(Flow::Normal)
        }

        Stmt::Return { value } => match value {
            None => Ok(Flow::Return(None)),
            Some(expression) => Ok(Flow::Return(evaluate(host, expression, state).await?)),
        },

        Stmt::Exit { when } => match when {
            None => Ok(Flow::Exit),
            Some(condition) => Ok(if condition_holds(host, condition, state).await? {
                Flow::Exit
            } else {
                Flow::Normal
            }),
        },

        Stmt::Continue { when } => match when {
            None => Ok(Flow::Continue),
            Some(condition) => Ok(if condition_holds(host, condition, state).await? {
                Flow::Continue
            } else {
                Flow::Normal
            }),
        },

        Stmt::If {
            branches,
            otherwise,
        } => {
            for (condition, body) in branches {
                if condition_holds(host, condition, state).await? {
                    return run_statements(body, host, state).await;
                }
            }
            run_statements(otherwise, host, state).await
        }

        Stmt::While { condition, body } => {
            let mut iterations = 0u64;
            while condition_holds(host, condition, state).await? {
                iterations += 1;
                if iterations > MAX_ITERATIONS {
                    return Err(error(format!(
                        "PL/pgSQL loop did not finish within {MAX_ITERATIONS} iterations"
                    )));
                }
                match run_statements(body, host, state).await? {
                    Flow::Normal | Flow::Continue => {}
                    Flow::Exit => break,
                    Flow::Return(value) => return Ok(Flow::Return(value)),
                }
            }
            Ok(Flow::Normal)
        }

        Stmt::Loop { body } => {
            let mut iterations = 0u64;
            loop {
                iterations += 1;
                if iterations > MAX_ITERATIONS {
                    return Err(error(format!(
                        "PL/pgSQL loop did not finish within {MAX_ITERATIONS} iterations"
                    )));
                }
                match run_statements(body, host, state).await? {
                    Flow::Normal | Flow::Continue => {}
                    Flow::Exit => break,
                    Flow::Return(value) => return Ok(Flow::Return(value)),
                }
            }
            Ok(Flow::Normal)
        }

        Stmt::ForRange {
            name,
            from,
            to,
            reverse,
            body,
        } => run_for_range(host, state, name, from, to, *reverse, body).await,

        Stmt::ForQuery { name, query, body } => run_for_query(host, state, name, query, body).await,

        Stmt::ReturnQuery { query } => {
            let fetched = host.query(&render(query, state)).await?;
            RETURNED_ROWS
                .try_with(|collected| {
                    if let Ok(mut collected) = collected.lock() {
                        // The first query fixes the shape; later ones append.
                        if collected.columns.is_empty() {
                            collected.columns = fetched.columns.clone();
                        }
                        collected.rows.extend(fetched.rows.clone());
                    }
                })
                .map_err(|_| error("RETURN QUERY outside a block"))?;
            Ok(Flow::Normal)
        }

        Stmt::ReturnNext { value } => {
            let evaluated = evaluate(host, value, state).await?;
            append_rows(&Rows {
                columns: vec!["value".to_string()],
                rows: vec![vec![evaluated]],
            })?;
            Ok(Flow::Normal)
        }

        Stmt::OpenCursor { name } => {
            let Some(cursor) = state.cursors.get(name).cloned() else {
                return Err(error(format!("cursor \"{name}\" does not exist")));
            };
            let rows = host.query(&render(&cursor.query, state)).await?;
            if let Some(cursor) = state.cursors.get_mut(name) {
                cursor.rows = Some(rows);
                cursor.position = 0;
            }
            Ok(Flow::Normal)
        }

        Stmt::CloseCursor { name } => {
            // Closing forgets the rows but keeps the declaration, so the
            // cursor can be opened again — as PostgreSQL allows.
            if let Some(cursor) = state.cursors.get_mut(name) {
                cursor.rows = None;
                cursor.position = 0;
                return Ok(Flow::Normal);
            }
            Err(error(format!("cursor \"{name}\" does not exist")))
        }

        Stmt::Fetch { name, targets } => {
            let row = {
                let Some(cursor) = state.cursors.get_mut(name) else {
                    return Err(error(format!("cursor \"{name}\" does not exist")));
                };
                let Some(rows) = cursor.rows.as_ref() else {
                    return Err(error(format!("cursor \"{name}\" is not open")));
                };
                let row = rows.rows.get(cursor.position).cloned();
                if row.is_some() {
                    cursor.position += 1;
                }
                row
            };

            // `FOUND` is how a FETCH loop knows to stop.
            state.set_found(row.is_some());
            if let Some(row) = row {
                for (target, value) in targets.iter().zip(row) {
                    let quoted = state
                        .scope
                        .get(target)
                        .map_or_else(|| Value::infer(value.clone()).quoted, |v| v.quoted);
                    state.scope.insert(
                        target.clone(),
                        Value {
                            text: value,
                            quoted,
                        },
                    );
                }
            }
            Ok(Flow::Normal)
        }

        Stmt::ForCursor { name, cursor, body } => {
            let declared = state
                .cursors
                .get(cursor)
                .cloned()
                .ok_or_else(|| error(format!("cursor \"{cursor}\" does not exist")))?;
            // A cursor FOR loop opens it, walks it and closes it, so the body
            // reads the same rows a FETCH loop would.
            let query = declared.query.clone();
            run_for_query(host, state, name, &query, body).await
        }

        Stmt::Nested { block } => {
            if block.handlers.is_empty() {
                return match Box::pin(execute_in(block, host, state)).await? {
                    Returned::Scalar(value) => Ok(Flow::Return(value)),
                    Returned::Nothing | Returned::Rows(_) => Ok(Flow::Normal),
                };
            }
            run_protected_block(block, host, state).await
        }
    }
}

/// Run a block with `EXCEPTION` arms.
///
/// The protected statements run so that a failure undoes what they wrote —
/// otherwise a caught exception would leave a half-finished write behind,
/// which is the thing a handler exists to prevent.
async fn run_protected_block(
    block: &Block,
    host: &dyn PlPgSqlHost,
    state: &mut State,
) -> ProtocolResult<Flow> {
    let protected = Block {
        declarations: block.declarations.clone(),
        body: block.body.clone(),
        handlers: Vec::new(),
    };
    let (outcome, returned) = host.run_protected(&protected, state.clone()).await?;
    // Variable values survive a caught exception; only database writes are
    // undone. That is what PostgreSQL does.
    *state = returned;

    let failure = match outcome {
        Ok(Returned::Scalar(value)) => return Ok(Flow::Return(value)),
        Ok(Returned::Nothing | Returned::Rows(_)) => return Ok(Flow::Normal),
        Err(failure) => failure,
    };

    let Some(handler) = block
        .handlers
        .iter()
        .find(|handler| catches(handler, &failure))
    else {
        return Err(failure);
    };

    // `SQLERRM` is what a handler reads to find out what happened. It carries
    // the message the block raised, not how the message reached here — a
    // handler that logs it should not be logging our transport's name.
    let reported = failure.to_string();
    let message = reported
        .rsplit_once("error: ")
        .map_or(reported.as_str(), |(_, message)| message)
        .to_string();
    state.scope.insert(
        "sqlerrm".to_string(),
        Value {
            text: Some(message),
            quoted: true,
        },
    );
    run_statements(&handler.body, host, state).await
}

/// `FOR rec IN SELECT ... LOOP` — one iteration per row.
async fn run_for_query(
    host: &dyn PlPgSqlHost,
    state: &mut State,
    name: &str,
    query: &Expr,
    body: &[Stmt],
) -> ProtocolResult<Flow> {
    let fetched = host.query(&render(query, state)).await?;

    // The record's columns shadow anything of the same dotted name and are
    // removed afterwards, so the variable does not outlive its loop.
    let keys: Vec<String> = fetched
        .columns
        .iter()
        .map(|column| format!("{}.{}", name.to_lowercase(), column.to_lowercase()))
        .collect();

    let mut flow = Flow::Normal;
    for row in fetched.rows {
        for (key, value) in keys.iter().zip(row.iter()) {
            state.scope.insert(key.clone(), Value::infer(value.clone()));
        }
        // A single-column row is also readable as the bare name, which is how
        // `FOR id IN SELECT id FROM t` is usually written.
        if keys.len() == 1 {
            state.scope.insert(
                name.to_lowercase(),
                Value::infer(row.first().cloned().flatten()),
            );
        }
        match run_statements(body, host, state).await? {
            Flow::Normal | Flow::Continue => {}
            Flow::Exit => break,
            Flow::Return(value) => {
                flow = Flow::Return(value);
                break;
            }
        }
    }

    for key in &keys {
        state.scope.remove(key);
    }
    state.scope.remove(&name.to_lowercase());
    Ok(flow)
}

/// Parse a bound of a `FOR` range, which must be an integer.
fn bound(value: Option<String>, which: &str) -> ProtocolResult<i64> {
    value
        .as_deref()
        .and_then(|text| text.trim().parse::<i64>().ok())
        .ok_or_else(|| {
            error(format!(
                "the {which} bound of a PL/pgSQL FOR loop must be an integer, got {}",
                value.as_deref().unwrap_or("NULL")
            ))
        })
}

#[allow(clippy::too_many_arguments)]
async fn run_for_range(
    host: &dyn PlPgSqlHost,
    state: &mut State,
    name: &str,
    from: &Expr,
    to: &Expr,
    reverse: bool,
    body: &[Stmt],
) -> ProtocolResult<Flow> {
    let low = bound(evaluate(host, from, state).await?, "low")?;
    let high = bound(evaluate(host, to, state).await?, "high")?;

    // The loop variable shadows anything of the same name and is restored
    // afterwards, as PostgreSQL scopes it to the loop.
    let shadowed = state.scope.get(name).cloned();
    let counter: Box<dyn Iterator<Item = i64> + Send> = if reverse {
        Box::new((low..=high).rev())
    } else {
        Box::new(low..=high)
    };

    let mut flow = Flow::Normal;
    for index in counter {
        state.scope.insert(
            name.to_string(),
            Value {
                text: Some(index.to_string()),
                quoted: false,
            },
        );
        match run_statements(body, host, state).await? {
            Flow::Normal | Flow::Continue => {}
            Flow::Exit => break,
            Flow::Return(value) => {
                flow = Flow::Return(value);
                break;
            }
        }
    }

    match shadowed {
        Some(previous) => state.scope.insert(name.to_string(), previous),
        None => state.scope.remove(name),
    };
    Ok(flow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A host that evaluates nothing and only records what it was asked to run.
    struct Recorder {
        answers: Mutex<Vec<(String, Option<String>)>>,
        ran: Mutex<Vec<String>>,
        notices: Mutex<Vec<String>>,
    }

    impl Recorder {
        fn new(answers: &[(&str, Option<&str>)]) -> Self {
            Self {
                answers: Mutex::new(
                    answers
                        .iter()
                        .map(|(q, a)| ((*q).to_string(), a.map(str::to_string)))
                        .collect(),
                ),
                ran: Mutex::new(Vec::new()),
                notices: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl PlPgSqlHost for Recorder {
        async fn evaluate(&self, expression: &str) -> ProtocolResult<Option<String>> {
            let answers = self.answers.lock().expect("lock");
            answers
                .iter()
                .find(|(q, _)| q == expression)
                .map(|(_, a)| a.clone())
                .ok_or_else(|| error(format!("test host has no answer for {expression:?}")))
        }

        async fn run(&self, sql: &str) -> ProtocolResult<()> {
            self.ran.lock().expect("lock").push(sql.to_string());
            Ok(())
        }

        async fn query(&self, sql: &str) -> ProtocolResult<Rows> {
            self.ran.lock().expect("lock").push(sql.to_string());
            let answer = self.evaluate(sql).await?;
            Ok(Rows {
                columns: vec!["value".to_string()],
                rows: answer.into_iter().map(|v| vec![Some(v)]).collect(),
            })
        }

        /// No transactions here, so nothing is undone: these tests check which
        /// handler runs, not what a rollback reverses. The rollback itself is
        /// checked against a live server in the conformance harness.
        async fn column_type(&self, _table: &str, _column: &str) -> ProtocolResult<Option<String>> {
            Ok(Some("INTEGER".to_string()))
        }

        async fn row_columns(&self, _table: &str) -> ProtocolResult<Vec<String>> {
            Ok(vec!["id".to_string(), "name".to_string()])
        }

        async fn run_protected(
            &self,
            block: &Block,
            state: State,
        ) -> ProtocolResult<(Result<Returned, ProtocolError>, State)> {
            let mut state = state;
            let outcome = execute_in(block, self, &mut state).await;
            Ok((outcome, state))
        }

        fn notice(&self, message: &str) {
            self.notices.lock().expect("lock").push(message.to_string());
        }
    }

    fn parsed(source: &str) -> Block {
        parse(source).unwrap_or_else(|e| panic!("parse {source}: {e}"))
    }

    #[test]
    fn a_bare_block_parses() {
        let block = parsed("$$ BEGIN NULL; END $$");
        assert!(block.declarations.is_empty());
        assert_eq!(block.body, vec![Stmt::Nothing]);
    }

    #[test]
    fn declarations_carry_their_type_and_default() {
        let block = parsed("DECLARE n INTEGER := 1; s TEXT; BEGIN NULL; END");
        assert_eq!(block.declarations.len(), 2);
        assert_eq!(block.declarations[0].name, "n");
        assert_eq!(
            block.declarations[0].sql_type,
            TypeSource::Named("INTEGER".to_string())
        );
        assert!(block.declarations[0].default.is_some());
        assert_eq!(
            block.declarations[1].sql_type,
            TypeSource::Named("TEXT".to_string())
        );
        assert!(block.declarations[1].default.is_none());
    }

    #[test]
    fn a_multi_word_type_stays_intact() {
        let block = parsed("DECLARE d DOUBLE PRECISION; BEGIN NULL; END");
        assert_eq!(
            block.declarations[0].sql_type,
            TypeSource::Named("DOUBLE PRECISION".to_string())
        );
        assert!(!needs_quoting("DOUBLE PRECISION"));
    }

    #[test]
    fn an_if_keeps_every_branch_in_order() {
        let block = parsed("BEGIN IF a THEN NULL; ELSIF b THEN NULL; ELSE NULL; END IF; END");
        match &block.body[0] {
            Stmt::If {
                branches,
                otherwise,
            } => {
                assert_eq!(branches.len(), 2);
                assert_eq!(otherwise.len(), 1);
            }
            other => panic!("expected an IF, got {other:?}"),
        }
    }

    #[test]
    fn a_for_range_records_its_direction() {
        match &parsed("BEGIN FOR i IN REVERSE 1..3 LOOP NULL; END LOOP; END").body[0] {
            Stmt::ForRange { name, reverse, .. } => {
                assert_eq!(name, "i");
                assert!(*reverse);
            }
            other => panic!("expected a FOR, got {other:?}"),
        }
    }

    #[test]
    fn select_into_becomes_an_assignment() {
        match &parsed("BEGIN SELECT COUNT(*) FROM t INTO n; END").body[0] {
            Stmt::SelectInto { target, .. } => assert_eq!(target, "n"),
            other => panic!("expected a SELECT INTO, got {other:?}"),
        }
    }

    #[test]
    fn a_plain_select_is_left_as_sql() {
        assert!(matches!(
            &parsed("BEGIN SELECT 1; END").body[0],
            Stmt::Sql(_)
        ));
    }

    #[test]
    fn raise_notice_does_not_abort_but_exception_does() {
        match &parsed("BEGIN RAISE NOTICE 'hi'; END").body[0] {
            Stmt::Raise { message, aborts } => {
                assert_eq!(message, "hi");
                assert!(!aborts);
            }
            other => panic!("expected a RAISE, got {other:?}"),
        }
        match &parsed("BEGIN RAISE EXCEPTION 'no'; END").body[0] {
            Stmt::Raise { aborts, .. } => assert!(aborts),
            other => panic!("expected a RAISE, got {other:?}"),
        }
    }

    #[test]
    fn a_keyword_inside_a_string_is_not_a_keyword() {
        // 'END' here is data. If the lexer missed that, the block would end
        // early and the INSERT would be dropped.
        let block = parsed("BEGIN INSERT INTO t (a) VALUES ('END'); END");
        assert_eq!(block.body.len(), 1);
        assert!(matches!(&block.body[0], Stmt::Sql(_)));
    }

    #[test]
    fn a_missing_end_if_is_an_error_not_a_silent_truncation() {
        assert!(parse("BEGIN IF a THEN NULL; END").is_err());
    }

    #[test]
    fn a_missing_begin_is_an_error() {
        assert!(parse("DECLARE n INTEGER; NULL; END").is_err());
    }

    #[test]
    fn a_comment_is_not_a_statement() {
        let block = parsed("BEGIN -- nothing to see\n NULL; END");
        assert_eq!(block.body.len(), 1);
    }

    #[test]
    fn substitution_does_not_reach_inside_a_string() {
        let scope = HashMap::from([(
            "n".to_string(),
            Value {
                text: Some("7".to_string()),
                quoted: false,
            },
        )]);
        let rendered = render(&lex("SELECT n, 'n'"), &State::with_arguments(scope));
        assert_eq!(rendered, "SELECT 7 , 'n'");
    }

    #[test]
    fn a_text_value_is_quoted_and_escaped() {
        let value = Value {
            text: Some("it's".to_string()),
            quoted: true,
        };
        assert_eq!(value.render(), "'it''s'");
    }

    #[tokio::test]
    async fn an_if_runs_only_the_true_branch() {
        let host = Recorder::new(&[("1 < 2", Some("t"))]);
        let block = parsed("BEGIN IF 1 < 2 THEN INSERT INTO t VALUES (1); ELSE INSERT INTO t VALUES (2); END IF; END");
        execute(&block, &host, HashMap::new()).await.expect("runs");
        assert_eq!(
            *host.ran.lock().expect("lock"),
            vec!["INSERT INTO t VALUES ( 1 )"]
        );
    }

    #[tokio::test]
    async fn a_for_loop_runs_its_body_once_per_value() {
        let host = Recorder::new(&[("1", Some("1")), ("3", Some("3"))]);
        let block = parsed("BEGIN FOR i IN 1..3 LOOP INSERT INTO t VALUES (i); END LOOP; END");
        execute(&block, &host, HashMap::new()).await.expect("runs");
        assert_eq!(
            *host.ran.lock().expect("lock"),
            vec![
                "INSERT INTO t VALUES ( 1 )",
                "INSERT INTO t VALUES ( 2 )",
                "INSERT INTO t VALUES ( 3 )",
            ]
        );
    }

    #[tokio::test]
    async fn a_reverse_for_loop_counts_down() {
        let host = Recorder::new(&[("1", Some("1")), ("2", Some("2"))]);
        let block =
            parsed("BEGIN FOR i IN REVERSE 1..2 LOOP INSERT INTO t VALUES (i); END LOOP; END");
        execute(&block, &host, HashMap::new()).await.expect("runs");
        assert_eq!(
            *host.ran.lock().expect("lock"),
            vec!["INSERT INTO t VALUES ( 2 )", "INSERT INTO t VALUES ( 1 )"]
        );
    }

    #[tokio::test]
    async fn a_loop_variable_does_not_outlive_its_loop() {
        let host = Recorder::new(&[("1", Some("1")), ("2", Some("2"))]);
        let block =
            parsed("BEGIN FOR i IN 1..2 LOOP NULL; END LOOP; INSERT INTO t VALUES (i); END");
        execute(&block, &host, HashMap::new()).await.expect("runs");
        // `i` is gone, so it stays a bare name rather than its last value.
        assert_eq!(
            *host.ran.lock().expect("lock"),
            vec!["INSERT INTO t VALUES ( i )"]
        );
    }

    #[tokio::test]
    async fn return_stops_the_block() {
        let host = Recorder::new(&[("42", Some("42"))]);
        let block = parsed("BEGIN RETURN 42; INSERT INTO t VALUES (1); END");
        let returned = execute(&block, &host, HashMap::new()).await.expect("runs");
        assert_eq!(returned, Returned::Scalar(Some("42".to_string())));
        assert!(host.ran.lock().expect("lock").is_empty());
    }

    #[tokio::test]
    async fn exit_when_leaves_the_loop_early() {
        let host = Recorder::new(&[
            ("1", Some("1")),
            ("3", Some("3")),
            ("1 >= 2", Some("f")),
            ("2 >= 2", Some("t")),
        ]);
        let block = parsed(
            "BEGIN FOR i IN 1..3 LOOP INSERT INTO t VALUES (i); EXIT WHEN i >= 2; END LOOP; END",
        );
        execute(&block, &host, HashMap::new()).await.expect("runs");
        assert_eq!(host.ran.lock().expect("lock").len(), 2);
    }

    #[tokio::test]
    async fn a_while_loop_that_never_settles_fails_loudly() {
        let host = Recorder::new(&[("true", Some("t"))]);
        let block = parsed("BEGIN WHILE true LOOP NULL; END LOOP; END");
        let outcome = execute(&block, &host, HashMap::new()).await;
        // The alternative is a statement that never returns, holding a
        // connection and a share of the runtime for ever.
        assert!(outcome.is_err(), "a runaway loop must not run for ever");
    }

    #[tokio::test]
    async fn raise_exception_aborts_with_its_message() {
        let host = Recorder::new(&[]);
        let block = parsed("BEGIN RAISE EXCEPTION 'no good'; END");
        let outcome = execute(&block, &host, HashMap::new()).await;
        assert!(outcome.expect_err("aborts").to_string().contains("no good"));
    }

    #[tokio::test]
    async fn raise_notice_reports_and_carries_on() {
        let host = Recorder::new(&[]);
        let block = parsed("BEGIN RAISE NOTICE 'just saying'; INSERT INTO t VALUES (1); END");
        execute(&block, &host, HashMap::new()).await.expect("runs");
        assert_eq!(*host.notices.lock().expect("lock"), vec!["just saying"]);
        assert_eq!(host.ran.lock().expect("lock").len(), 1);
    }

    #[tokio::test]
    async fn a_declared_variable_is_substituted_into_sql() {
        let host = Recorder::new(&[("5", Some("5"))]);
        let block = parsed("DECLARE n INTEGER := 5; BEGIN INSERT INTO t VALUES (n); END");
        execute(&block, &host, HashMap::new()).await.expect("runs");
        assert_eq!(
            *host.ran.lock().expect("lock"),
            vec!["INSERT INTO t VALUES ( 5 )"]
        );
    }

    #[tokio::test]
    async fn an_undeclared_variable_is_left_for_sql_to_reject() {
        // Substituting nothing means the engine sees the bare name and reports
        // it — better than this interpreter inventing a NULL.
        let host = Recorder::new(&[]);
        let block = parsed("BEGIN INSERT INTO t VALUES (nope); END");
        execute(&block, &host, HashMap::new()).await.expect("runs");
        assert_eq!(
            *host.ran.lock().expect("lock"),
            vec!["INSERT INTO t VALUES ( nope )"]
        );
    }

    #[test]
    fn needs_interpreter_tells_a_plain_body_from_a_procedural_one() {
        assert!(!needs_interpreter(
            "$$BEGIN INSERT INTO t VALUES (1); END$$"
        ));
        assert!(needs_interpreter("$$DECLARE n INT; BEGIN NULL; END$$"));
        assert!(needs_interpreter("$$BEGIN IF a THEN NULL; END IF; END$$"));
    }

    #[test]
    fn an_exception_arm_parses_with_its_conditions() {
        let block = parsed("BEGIN NULL; EXCEPTION WHEN division_by_zero OR OTHERS THEN NULL; END");
        assert_eq!(block.handlers.len(), 1);
        assert_eq!(
            block.handlers[0].conditions,
            ["DIVISION_BY_ZERO".to_string(), "OTHERS".to_string()]
        );
    }

    #[test]
    fn a_named_condition_alone_catches_nothing() {
        // Errors here carry no SQLSTATE, so matching a named condition would
        // be a promise that could not be kept. Refusing to catch is the safe
        // direction: the error propagates rather than being swallowed.
        let handler = Handler {
            conditions: vec!["DIVISION_BY_ZERO".to_string()],
            body: Vec::new(),
        };
        assert!(!catches(&handler, &error("anything")));

        let others = Handler {
            conditions: vec!["OTHERS".to_string()],
            body: Vec::new(),
        };
        assert!(catches(&others, &error("anything")));
    }

    #[test]
    fn for_over_a_query_parses_as_a_query_loop() {
        match &parsed("BEGIN FOR r IN SELECT a FROM t LOOP NULL; END LOOP; END").body[0] {
            Stmt::ForQuery { name, .. } => assert_eq!(name, "r"),
            other => panic!("expected a query loop, got {other:?}"),
        }
    }

    #[test]
    fn a_range_loop_is_still_a_range_loop() {
        assert!(matches!(
            &parsed("BEGIN FOR i IN 1..3 LOOP NULL; END LOOP; END").body[0],
            Stmt::ForRange { .. }
        ));
    }

    #[test]
    fn return_query_is_not_return() {
        match &parsed("BEGIN RETURN QUERY SELECT a FROM t; END").body[0] {
            Stmt::ReturnQuery { .. } => {}
            other => panic!("expected RETURN QUERY, got {other:?}"),
        }
        assert!(matches!(
            &parsed("BEGIN RETURN 1; END").body[0],
            Stmt::Return { .. }
        ));
    }

    #[test]
    fn a_nested_block_parses_as_one_statement() {
        let block = parsed("BEGIN BEGIN NULL; END; NULL; END");
        assert_eq!(block.body.len(), 2);
        assert!(matches!(&block.body[0], Stmt::Nested { .. }));
    }

    #[test]
    fn a_record_field_can_be_assigned() {
        match &parsed("BEGIN a.street := 'Main'; END").body[0] {
            Stmt::Assign { name, .. } => assert_eq!(name, "a.street"),
            other => panic!("expected an assignment, got {other:?}"),
        }
    }

    #[test]
    fn a_record_field_is_one_reference() {
        let scope = HashMap::from([(
            "r.name".to_string(),
            Value {
                text: Some("ada".to_string()),
                quoted: true,
            },
        )]);
        assert_eq!(
            render(&lex("SELECT r.name"), &State::with_arguments(scope)),
            "SELECT 'ada'"
        );
    }

    #[test]
    fn an_unknown_record_field_is_left_alone() {
        // `t.col` in ordinary SQL must survive untouched.
        assert_eq!(
            render(&lex("SELECT t.col"), &State::default()),
            "SELECT t . col"
        );
    }

    #[test]
    fn quoting_is_inferred_when_no_type_says() {
        assert!(!Value::infer(Some("42".to_string())).quoted);
        assert!(!Value::infer(Some("true".to_string())).quoted);
        assert!(Value::infer(Some("ada".to_string())).quoted);
    }

    #[test]
    fn a_declared_type_decides_quoting() {
        assert!(Value::typed(Some("42".to_string()), "TEXT").quoted);
        assert!(!Value::typed(Some("42".to_string()), "INTEGER").quoted);
    }

    #[tokio::test]
    async fn a_handler_catches_and_the_block_carries_on() {
        let host = Recorder::new(&[]);
        let block = parsed(
            "BEGIN BEGIN RAISE EXCEPTION 'boom'; EXCEPTION WHEN OTHERS THEN INSERT INTO t VALUES (1); END; END",
        );
        execute(&block, &host, HashMap::new())
            .await
            .expect("the handler catches");
        assert_eq!(
            *host.ran.lock().expect("lock"),
            vec!["INSERT INTO t VALUES ( 1 )"]
        );
    }

    #[tokio::test]
    async fn an_uncaught_failure_still_propagates() {
        let host = Recorder::new(&[]);
        let block = parsed("BEGIN BEGIN RAISE EXCEPTION 'boom'; END; END");
        assert!(execute(&block, &host, HashMap::new()).await.is_err());
    }

    #[tokio::test]
    async fn a_handler_can_read_sqlerrm() {
        let host = Recorder::new(&[]);
        let block = parsed(
            "BEGIN BEGIN RAISE EXCEPTION 'boom'; EXCEPTION WHEN OTHERS THEN INSERT INTO t VALUES (SQLERRM); END; END",
        );
        execute(&block, &host, HashMap::new()).await.expect("runs");
        let ran = host.ran.lock().expect("lock");
        assert!(ran[0].contains("boom"), "SQLERRM was not bound: {ran:?}");
    }

    #[test]
    fn a_tagged_dollar_quote_is_stripped() {
        assert_eq!(strip_dollar_quotes("$body$ BEGIN END $body$"), "BEGIN END");
        assert_eq!(strip_dollar_quotes("$$ BEGIN END $$"), "BEGIN END");
        assert_eq!(strip_dollar_quotes("BEGIN END"), "BEGIN END");
    }
}
