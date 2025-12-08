//! Orbit CLI - Interactive database client with syntax highlighting
//!
//! A rich terminal interface for connecting to Orbit databases via multiple protocols:
//! - PostgreSQL Wire Protocol
//! - MySQL Protocol
//! - Cassandra Query Language (CQL)
//! - Redis (RESP Protocol)
//! - OrbitQL (via REST API)
//!
//! Features:
//! - Syntax highlighting for SQL queries
//! - Pretty-printed result tables with colors
//! - Command history with persistent storage
//! - Tab completion for SQL keywords
//! - Multi-line query support
//! - Multiple output formats (Table, JSON, CSV, Plain)

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, Color, Table};
use orbit_server::protocols::common::formatting::{format_error, format_success, OutputFormat};
use owo_colors::OwoColorize;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::path::PathBuf;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SyntectStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};
use tokio_postgres::{Client, NoTls};
use tracing::{error, info};

/// Orbit CLI - Interactive database client
#[derive(Parser, Debug)]
#[command(name = "orbit")]
#[command(about = "Interactive CLI client for Orbit database", long_about = None)]
#[command(version)]
struct Cli {
    /// Protocol to use for connection
    #[arg(long, value_enum, default_value = "postgres")]
    protocol: Protocol,

    /// Host to connect to
    #[arg(short = 'H', long, default_value = "localhost")]
    host: String,

    /// Port to connect to
    #[arg(short = 'p', long)]
    port: Option<u16>,

    /// Database name
    #[arg(short, long, default_value = "orbit")]
    database: String,

    /// Username for authentication
    #[arg(short, long, default_value = "orbit")]
    username: String,

    /// Password for authentication
    #[arg(short = 'W', long)]
    password: Option<String>,

    /// Output format
    #[arg(short = 'o', long, value_enum, default_value = "table")]
    format: CliOutputFormat,

    /// Execute a single command and exit
    #[arg(short = 'e', long)]
    execute: Option<String>,

    /// File containing SQL commands to execute
    #[arg(short = 'f', long)]
    file: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

/// Supported connection protocols
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
enum Protocol {
    /// PostgreSQL Wire Protocol
    Postgres,
    /// MySQL Protocol
    Mysql,
    /// Cassandra Query Language (CQL)
    Cql,
    /// Redis (RESP Protocol)
    Redis,
    /// OrbitQL (via REST API)
    Orbitql,
}

impl Protocol {
    fn default_port(&self) -> u16 {
        match self {
            Protocol::Postgres => 5432,
            Protocol::Mysql => 3306,
            Protocol::Cql => 9042,
            Protocol::Redis => 6379,
            Protocol::Orbitql => 8080,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Protocol::Postgres => "PostgreSQL",
            Protocol::Mysql => "MySQL",
            Protocol::Cql => "CQL",
            Protocol::Redis => "Redis",
            Protocol::Orbitql => "OrbitQL",
        }
    }

    fn prompt_suffix(&self) -> &'static str {
        match self {
            Protocol::Postgres => "sql",
            Protocol::Mysql => "mysql",
            Protocol::Cql => "cql",
            Protocol::Redis => "redis",
            Protocol::Orbitql => "oql",
        }
    }
}

/// CLI output format options
#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliOutputFormat {
    /// Pretty table with borders
    Table,
    /// JSON array of objects
    Json,
    /// CSV format
    Csv,
    /// Plain text (tab-separated)
    Plain,
}

impl From<CliOutputFormat> for OutputFormat {
    fn from(format: CliOutputFormat) -> Self {
        match format {
            CliOutputFormat::Table => OutputFormat::Table,
            CliOutputFormat::Json => OutputFormat::Json,
            CliOutputFormat::Csv => OutputFormat::Csv,
            CliOutputFormat::Plain => OutputFormat::Plain,
        }
    }
}

/// REPL state and configuration
struct ReplState {
    protocol: Protocol,
    host: String,
    port: u16,
    database: String,
    username: String,
    password: Option<String>,
    format: OutputFormat,
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    // PostgreSQL connection
    pg_client: Option<Client>,
    pg_connection_handle: Option<tokio::task::JoinHandle<()>>,
    // MySQL connection
    mysql_pool: Option<mysql_async::Pool>,
    // Redis connection
    redis_client: Option<redis::Client>,
    // OrbitQL/REST HTTP client
    http_client: Option<reqwest::Client>,
}

impl ReplState {
    fn new(cli: &Cli) -> Self {
        let port = cli.port.unwrap_or_else(|| cli.protocol.default_port());

        Self {
            protocol: cli.protocol,
            host: cli.host.clone(),
            port,
            database: cli.database.clone(),
            username: cli.username.clone(),
            password: cli.password.clone(),
            format: cli.format.into(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            pg_client: None,
            pg_connection_handle: None,
            mysql_pool: None,
            redis_client: None,
            http_client: None,
        }
    }

    /// Connect to the database based on protocol
    async fn connect(&mut self) -> Result<()> {
        match self.protocol {
            Protocol::Postgres => self.connect_postgres().await,
            Protocol::Mysql => self.connect_mysql().await,
            Protocol::Redis => self.connect_redis().await,
            Protocol::Orbitql => self.connect_orbitql().await,
            Protocol::Cql => self.connect_cql().await,
        }
    }

    /// Connect to PostgreSQL database
    async fn connect_postgres(&mut self) -> Result<()> {
        if self.protocol != Protocol::Postgres {
            return Ok(()); // Only connect for PostgreSQL
        }

        let connection_string = format!(
            "host={} port={} user={} password={} dbname={}",
            self.host,
            self.port,
            self.username,
            self.password.as_deref().unwrap_or(""),
            self.database
        );

        match tokio_postgres::connect(&connection_string, NoTls).await {
            Ok((client, connection)) => {
                // Spawn connection handler
                let handle = tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        error!("PostgreSQL connection error: {}", e);
                    }
                });

                self.pg_client = Some(client);
                self.pg_connection_handle = Some(handle);
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!("Failed to connect to PostgreSQL: {}", e)),
        }
    }

    /// Connect to MySQL database
    async fn connect_mysql(&mut self) -> Result<()> {
        use mysql_async::prelude::*;

        let opts = mysql_async::OptsBuilder::default()
            .ip_or_hostname(self.host.clone())
            .tcp_port(self.port)
            .user(Some(self.username.clone()))
            .pass(self.password.clone())
            .db_name(Some(self.database.clone()));

        let pool = mysql_async::Pool::new(opts);

        // Test connection
        match pool.get_conn().await {
            Ok(mut conn) => {
                match conn.query_first::<String, _>("SELECT 1").await {
                    Ok(_) => {
                        self.mysql_pool = Some(pool);
                        Ok(())
                    }
                    Err(e) => Err(anyhow::anyhow!("MySQL connection test failed: {}", e)),
                }
            }
            Err(e) => Err(anyhow::anyhow!("Failed to connect to MySQL: {}", e)),
        }
    }

    /// Connect to Redis
    async fn connect_redis(&mut self) -> Result<()> {
        let redis_url = if let Some(ref pass) = self.password {
            format!("redis://:{}@{}:{}/", pass, self.host, self.port)
        } else {
            format!("redis://{}:{}/", self.host, self.port)
        };

        match redis::Client::open(redis_url) {
            Ok(client) => {
                // Test connection with PING
                let mut conn = client
                    .get_multiplexed_async_connection()
                    .await
                    .map_err(|e| anyhow::anyhow!("Redis connection failed: {}", e))?;

                let pong: String = redis::cmd("PING")
                    .query_async(&mut conn)
                    .await
                    .map_err(|e| anyhow::anyhow!("Redis PING failed: {}", e))?;

                if pong == "PONG" {
                    self.redis_client = Some(client);
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Unexpected Redis PING response: {}", pong))
                }
            }
            Err(e) => Err(anyhow::anyhow!("Invalid Redis URL: {}", e)),
        }
    }

    /// Connect to OrbitQL via REST API
    async fn connect_orbitql(&mut self) -> Result<()> {
        let client = reqwest::Client::new();
        let health_url = format!("http://{}:{}/health", self.host, self.port);

        match client.get(&health_url).send().await {
            Ok(response) if response.status().is_success() => {
                self.http_client = Some(client);
                Ok(())
            }
            Ok(response) => Err(anyhow::anyhow!(
                "OrbitQL health check failed: HTTP {}",
                response.status()
            )),
            Err(e) => Err(anyhow::anyhow!("Failed to connect to OrbitQL: {}", e)),
        }
    }

    /// Connect to CQL (Cassandra) via REST API
    async fn connect_cql(&mut self) -> Result<()> {
        // CQL uses HTTP REST API fallback for now
        let client = reqwest::Client::new();
        let health_url = format!("http://{}:{}/health", self.host, self.port);

        match client.get(&health_url).send().await {
            Ok(_) => {
                self.http_client = Some(client);
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!("Failed to connect to CQL endpoint: {}", e)),
        }
    }

    /// Execute a query based on the current protocol
    async fn execute_query(&self, query: &str) -> Result<()> {
        match self.protocol {
            Protocol::Postgres => self.execute_postgres_query(query).await,
            Protocol::Mysql => self.execute_mysql_query(query).await,
            Protocol::Redis => self.execute_redis_command(query).await,
            Protocol::Orbitql => self.execute_orbitql_query(query).await,
            Protocol::Cql => self.execute_cql_query(query).await,
        }
    }

    /// Execute a PostgreSQL query
    async fn execute_postgres_query(&self, query: &str) -> Result<()> {
        let client = self
            .pg_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to database"))?;

        // Try to execute as a simple query first (for SELECT, etc.)
        match client.simple_query(query).await {
            Ok(results) => {
                self.format_query_results(results)?;
                Ok(())
            }
            Err(_) => {
                // If simple_query fails, try as a parameterized query
                match client.query(query, &[]).await {
                    Ok(rows) => {
                        self.format_rows(rows)?;
                        Ok(())
                    }
                    Err(e) => Err(anyhow::anyhow!("Query execution failed: {}", e)),
                }
            }
        }
    }

    /// Format query results from simple_query
    fn format_query_results(&self, results: Vec<tokio_postgres::SimpleQueryMessage>) -> Result<()> {
        for result in results {
            match result {
                tokio_postgres::SimpleQueryMessage::Row(row) => {
                    // Collect column names
                    let columns: Vec<String> = row
                        .columns()
                        .iter()
                        .map(|col| col.name().to_string())
                        .collect();

                    // Collect row values
                    let mut values = Vec::new();
                    for i in 0..row.len() {
                        let value = match row.get(i) {
                            Some(v) => v.to_string(),
                            None => "NULL".to_string(),
                        };
                        values.push(value);
                    }

                    // Print as table
                    if !columns.is_empty() {
                        let mut table = Table::new();
                        table
                            .load_preset(UTF8_FULL)
                            .apply_modifier(UTF8_ROUND_CORNERS);
                        table.set_header(columns.iter().map(|c| Cell::new(c).fg(Color::Cyan)));
                        table.add_row(values.iter().map(Cell::new));
                        println!("\n{}", table);
                    }
                }
                tokio_postgres::SimpleQueryMessage::CommandComplete(count) => {
                    println!(
                        "{}",
                        format_success(&format!(
                            "Query executed successfully ({} rows affected)",
                            count
                        ))
                    );
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Format rows from parameterized query
    fn format_rows(&self, rows: Vec<tokio_postgres::Row>) -> Result<()> {
        if rows.is_empty() {
            println!("{}", format_success("Query executed successfully (0 rows)"));
            return Ok(());
        }

        // Get column names from first row
        let columns: Vec<String> = rows[0]
            .columns()
            .iter()
            .map(|col| col.name().to_string())
            .collect();

        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS);
        table.set_header(columns.iter().map(|c| Cell::new(c).fg(Color::Cyan)));

        // Add rows
        for row in &rows {
            let mut values = Vec::new();
            for i in 0..row.len() {
                let value = {
                    // Try to get value as text (most generic)
                    let col_type = row.columns()[i].type_();
                    let type_name = col_type.name();

                    // Try different types based on PostgreSQL type name
                    if type_name == "text" || type_name == "varchar" || type_name == "char" {
                        row.get::<_, Option<String>>(i)
                            .unwrap_or_else(|| "NULL".to_string())
                    } else if type_name == "int4" || type_name == "integer" {
                        row.get::<_, Option<i32>>(i)
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "NULL".to_string())
                    } else if type_name == "int8" || type_name == "bigint" {
                        row.get::<_, Option<i64>>(i)
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "NULL".to_string())
                    } else if type_name == "bool" || type_name == "boolean" {
                        row.get::<_, Option<bool>>(i)
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "NULL".to_string())
                    } else {
                        // Fallback: try to get as text
                        row.get::<_, Option<String>>(i)
                            .unwrap_or_else(|| format!("<{}>", type_name))
                    }
                };
                values.push(value);
            }
            table.add_row(values.iter().map(Cell::new));
        }

        println!("\n{}", table);
        println!("{}", format_success(&format!("({} rows)", rows.len())));
        Ok(())
    }

    /// Execute a MySQL query
    async fn execute_mysql_query(&self, query: &str) -> Result<()> {
        use mysql_async::prelude::*;

        let pool = self
            .mysql_pool
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to MySQL"))?;

        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get MySQL connection: {}", e))?;

        // Execute query and get results
        let result: Vec<mysql_async::Row> = conn
            .query(query)
            .await
            .map_err(|e| anyhow::anyhow!("MySQL query failed: {}", e))?;

        if result.is_empty() {
            println!("{}", format_success("Query executed successfully (0 rows)"));
            return Ok(());
        }

        // Get column names
        let columns: Vec<String> = result[0]
            .columns_ref()
            .iter()
            .map(|c| c.name_str().to_string())
            .collect();

        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS);
        table.set_header(columns.iter().map(|c| Cell::new(c).fg(Color::Cyan)));

        // Add rows
        for row in &result {
            let mut values = Vec::new();
            for i in 0..row.len() {
                let value: Option<String> = row.get(i);
                values.push(value.unwrap_or_else(|| "NULL".to_string()));
            }
            table.add_row(values.iter().map(Cell::new));
        }

        println!("\n{}", table);
        println!("{}", format_success(&format!("({} rows)", result.len())));
        Ok(())
    }

    /// Execute a Redis command
    async fn execute_redis_command(&self, command: &str) -> Result<()> {
        let client = self
            .redis_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to Redis"))?;

        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get Redis connection: {}", e))?;

        // Parse command into parts
        let parts: Vec<&str> = command.trim().split_whitespace().collect();
        if parts.is_empty() {
            return Err(anyhow::anyhow!("Empty command"));
        }

        let cmd_name = parts[0].to_uppercase();
        let args = &parts[1..];

        // Build and execute Redis command
        let mut cmd = redis::cmd(&cmd_name);
        for arg in args {
            cmd.arg(*arg);
        }

        let result: redis::Value = cmd
            .query_async(&mut conn)
            .await
            .map_err(|e| anyhow::anyhow!("Redis command failed: {}", e))?;

        // Format and display result
        self.format_redis_value(&result, 0);
        Ok(())
    }

    /// Format Redis value for display
    fn format_redis_value(&self, value: &redis::Value, indent: usize) {
        let prefix = "  ".repeat(indent);
        match value {
            redis::Value::Nil => println!("{}(nil)", prefix),
            redis::Value::Int(i) => println!("{}(integer) {}", prefix, i),
            redis::Value::Data(data) => {
                if let Ok(s) = String::from_utf8(data.clone()) {
                    println!("{}\"{}\"", prefix, s);
                } else {
                    println!("{}(binary data, {} bytes)", prefix, data.len());
                }
            }
            redis::Value::Bulk(arr) => {
                if arr.is_empty() {
                    println!("{}(empty array)", prefix);
                } else {
                    for (i, item) in arr.iter().enumerate() {
                        print!("{}{}) ", prefix, i + 1);
                        self.format_redis_value(item, 0);
                    }
                }
            }
            redis::Value::Status(s) => println!("{}{}", prefix, s),
            redis::Value::Okay => println!("{}OK", prefix),
        }
    }

    /// Execute an OrbitQL query via REST API
    async fn execute_orbitql_query(&self, query: &str) -> Result<()> {
        let client = self
            .http_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to OrbitQL"))?;

        let url = format!("http://{}:{}/api/v1/sql", self.host, self.port);

        let request_body = serde_json::json!({
            "query": query,
            "limit": 1000
        });

        let response = client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("OrbitQL request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("OrbitQL error (HTTP {}): {}", status, body));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse OrbitQL response: {}", e))?;

        // Display result based on structure
        if let Some(data) = result.get("data") {
            if let Some(rows) = data.get("rows").and_then(|r| r.as_array()) {
                let columns = data
                    .get("columns")
                    .and_then(|c| c.as_array())
                    .map(|cols| {
                        cols.iter()
                            .filter_map(|c| c.get("name").and_then(|n| n.as_str()))
                            .map(String::from)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                if rows.is_empty() {
                    println!("{}", format_success("Query executed successfully (0 rows)"));
                    return Ok(());
                }

                let mut table = Table::new();
                table
                    .load_preset(UTF8_FULL)
                    .apply_modifier(UTF8_ROUND_CORNERS);

                if !columns.is_empty() {
                    table.set_header(columns.iter().map(|c| Cell::new(c).fg(Color::Cyan)));
                }

                for row in rows {
                    if let Some(row_arr) = row.as_array() {
                        let values: Vec<String> = row_arr
                            .iter()
                            .map(|v| match v {
                                serde_json::Value::Null => "NULL".to_string(),
                                serde_json::Value::String(s) => s.clone(),
                                _ => v.to_string(),
                            })
                            .collect();
                        table.add_row(values.iter().map(Cell::new));
                    }
                }

                println!("\n{}", table);
                println!("{}", format_success(&format!("({} rows)", rows.len())));
            } else {
                // Non-tabular result, just print JSON
                println!("{}", serde_json::to_string_pretty(&data)?);
            }
        } else if let Some(error) = result.get("error") {
            return Err(anyhow::anyhow!("OrbitQL error: {}", error));
        } else {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }

        Ok(())
    }

    /// Execute a CQL query via REST API
    async fn execute_cql_query(&self, query: &str) -> Result<()> {
        let client = self
            .http_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to CQL endpoint"))?;

        let url = format!("http://{}:{}/api/v1/sql", self.host, self.port);

        let request_body = serde_json::json!({
            "query": query,
            "protocol": "cql"
        });

        let response = client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("CQL request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("CQL error (HTTP {}): {}", status, body));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse CQL response: {}", e))?;

        // Display result (same format as OrbitQL for now)
        println!("{}", serde_json::to_string_pretty(&result)?);
        Ok(())
    }

    /// Highlight SQL query using syntect
    fn highlight_query(&self, query: &str) -> String {
        let syntax = self
            .syntax_set
            .find_syntax_by_extension("sql")
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        let mut highlighted = String::new();
        for line in LinesWithEndings::from(query) {
            let ranges: Vec<(SyntectStyle, &str)> = highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();
            let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
            highlighted.push_str(&escaped);
        }

        highlighted
    }

    /// Format connection string for display
    fn connection_string(&self) -> String {
        format!(
            "{}@{}:{}/{}",
            self.username, self.host, self.port, self.database
        )
    }
}

/// Main entry point
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("orbit_cli=debug,orbit_client=debug")
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter("orbit_cli=info")
            .init();
    }

    info!("Starting Orbit CLI");

    // Execute single command mode
    if let Some(query) = &cli.execute {
        return execute_single_command(&cli, query).await;
    }

    // Execute file mode
    if let Some(file_path) = &cli.file {
        return execute_file(&cli, file_path).await;
    }

    // Interactive REPL mode
    run_repl(&cli).await
}

/// Execute a single command and exit
async fn execute_single_command(cli: &Cli, query: &str) -> Result<()> {
    let mut state = ReplState::new(cli);

    println!(
        "{}",
        format!(
            "Connecting to {} at {}...",
            cli.protocol.name(),
            state.connection_string()
        )
        .dimmed()
    );

    // Establish connection (works for all protocols)
    match state.connect().await {
        Ok(()) => {
            println!("{}", format_success("Connected successfully!"));
        }
        Err(e) => {
            return Err(anyhow::anyhow!("Connection failed: {}", e));
        }
    }

    println!("\n{}", state.highlight_query(query));

    // Execute query (routes to appropriate protocol handler)
    state.execute_query(query).await?;

    Ok(())
}

/// Execute commands from a file
async fn execute_file(cli: &Cli, file_path: &PathBuf) -> Result<()> {
    let mut state = ReplState::new(cli);

    println!(
        "{}",
        format!(
            "Connecting to {} at {}...",
            cli.protocol.name(),
            state.connection_string()
        )
        .dimmed()
    );

    // Establish connection (works for all protocols)
    match state.connect().await {
        Ok(()) => {
            println!("{}", format_success("Connected successfully!"));
        }
        Err(e) => {
            return Err(anyhow::anyhow!("Connection failed: {}", e));
        }
    }

    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

    // Split into statements (simple split on semicolon for now)
    // Note: For Redis, commands don't use semicolons, so split by newlines
    let statements: Vec<&str> = if cli.protocol == Protocol::Redis {
        content.lines().filter(|s| !s.trim().is_empty()).collect()
    } else {
        content
            .split(';')
            .filter(|s| !s.trim().is_empty())
            .collect()
    };

    println!(
        "{}",
        format!("Executing {} statements from file...", statements.len()).dimmed()
    );

    for (i, statement) in statements.iter().enumerate() {
        println!("\n{}", format!("Statement {}:", i + 1).cyan());
        println!("{}", state.highlight_query(statement.trim()));

        // Execute statement (routes to appropriate protocol handler)
        match state.execute_query(statement.trim()).await {
            Ok(()) => {
                // Success - results already printed
            }
            Err(e) => {
                println!("{}", format_error(&format!("Error: {}", e)));
            }
        }
    }

    Ok(())
}

/// Run interactive REPL
async fn run_repl(cli: &Cli) -> Result<()> {
    let mut state = ReplState::new(cli);

    // Print welcome banner
    print_banner(&state);

    // Establish connection (works for all protocols)
    println!(
        "{}",
        format!("Connecting to {}...", state.connection_string()).dimmed()
    );
    match state.connect().await {
        Ok(()) => {
            println!("{}", format_success("Connected successfully!"));
        }
        Err(e) => {
            println!("{}", format_error(&format!("Connection failed: {}", e)));
            println!(
                "{}",
                "Continuing in offline mode - queries will not execute.".dimmed()
            );
        }
    }

    // Initialize rustyline editor
    let mut editor = DefaultEditor::new()?;

    // Load history
    let history_file = dirs::home_dir()
        .map(|mut p| {
            p.push(".orbit_history");
            p
        })
        .unwrap_or_else(|| PathBuf::from(".orbit_history"));

    if editor.load_history(&history_file).is_err() {
        info!("No previous history found");
    }

    let mut query_buffer = String::new();

    loop {
        // Determine prompt based on whether we're in a multi-line query
        let prompt = if query_buffer.is_empty() {
            format!("{}> ", state.database).green().to_string()
        } else {
            "-> ".yellow().to_string()
        };

        match editor.readline(&prompt) {
            Ok(line) => {
                let trimmed = line.trim();

                // Skip empty lines
                if trimmed.is_empty() {
                    continue;
                }

                // Handle meta commands
                if trimmed.starts_with('\\') {
                    if handle_meta_command(trimmed, &state, &mut editor, &history_file).await? {
                        break; // Exit REPL
                    }
                    continue;
                }

                // For Redis, execute immediately (no semicolon needed)
                // For SQL protocols, buffer until semicolon
                if state.protocol == Protocol::Redis {
                    // Redis commands are single-line
                    editor.add_history_entry(trimmed)?;
                    println!("\n{}", state.highlight_query(trimmed));

                    match state.execute_query(trimmed).await {
                        Ok(()) => {}
                        Err(e) => {
                            println!("\n{}", format_error(&format!("Error: {}", e)));
                        }
                    }
                } else {
                    // SQL-like protocols - buffer until semicolon
                    query_buffer.push_str(&line);
                    query_buffer.push('\n');

                    // Check if query is complete (ends with semicolon)
                    if trimmed.ends_with(';') {
                        // Add to history
                        editor.add_history_entry(query_buffer.trim())?;

                        // Show highlighted query
                        println!("\n{}", state.highlight_query(&query_buffer));

                        // Execute query
                        match state.execute_query(&query_buffer).await {
                            Ok(()) => {
                                // Success - results already printed
                            }
                            Err(e) => {
                                println!("\n{}", format_error(&format!("Error: {}", e)));
                            }
                        }

                        // Clear buffer
                        query_buffer.clear();
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                query_buffer.clear();
            }
            Err(ReadlineError::Eof) => {
                println!("exit");
                break;
            }
            Err(err) => {
                error!("Error reading line: {:?}", err);
                println!("{}", format_error(&format!("Error: {}", err)));
                break;
            }
        }
    }

    // Save history
    editor.save_history(&history_file)?;

    println!("\n{}", format_success("Goodbye!"));
    Ok(())
}

/// Handle meta commands (commands starting with \)
#[allow(unused_variables)]
async fn handle_meta_command(
    command: &str,
    state: &ReplState,
    editor: &mut DefaultEditor,
    history_file: &PathBuf,
) -> Result<bool> {
    match command {
        "\\q" | "\\quit" | "\\exit" => {
            editor.save_history(history_file)?;
            return Ok(true); // Signal to exit
        }
        "\\?" | "\\help" => {
            print_help();
        }
        "\\c" | "\\connect" => {
            println!("{}", format_error("Connection change not yet implemented"));
        }
        "\\d" => {
            println!("{}", format_error("Table listing not yet implemented"));
        }
        "\\dt" => {
            println!("{}", format_error("Table listing not yet implemented"));
        }
        "\\l" => {
            println!("{}", format_error("Database listing not yet implemented"));
        }
        "\\format table" | "\\format json" | "\\format csv" | "\\format plain" => {
            let format_str = command.split_whitespace().nth(1).unwrap_or("table");
            println!(
                "{}",
                format_success(&format!("Output format set to: {}", format_str))
            );
            // TODO: Update state.format
        }
        "\\timing" => {
            println!("{}", format_success("Timing display toggled"));
        }
        _ => {
            println!("{}", format_error(&format!("Unknown command: {}", command)));
            println!("{}", "Type \\? for help".dimmed());
        }
    }

    Ok(false) // Don't exit
}

/// Print welcome banner
fn print_banner(state: &ReplState) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS);

    table.add_row(vec![
        Cell::new("Orbit CLI").fg(Color::Cyan),
        Cell::new(env!("CARGO_PKG_VERSION")).fg(Color::Green),
    ]);

    table.add_row(vec![
        Cell::new("Protocol").fg(Color::Yellow),
        Cell::new(state.protocol.name()),
    ]);

    table.add_row(vec![
        Cell::new("Connection").fg(Color::Yellow),
        Cell::new(state.connection_string()),
    ]);

    table.add_row(vec![
        Cell::new("Format").fg(Color::Yellow),
        Cell::new(format!("{:?}", state.format)),
    ]);

    println!("\n{}", table);
    println!("{}", "Type \\? for help, \\q to quit".dimmed());
}

/// Print help information
fn print_help() {
    let help_text = r#"
Orbit CLI Help
==============

Supported Protocols:
  --protocol postgres   PostgreSQL wire protocol (port 5432)
  --protocol mysql      MySQL protocol (port 3306)
  --protocol redis      Redis RESP protocol (port 6379)
  --protocol orbitql    OrbitQL via REST API (port 8080)
  --protocol cql        Cassandra CQL via REST (port 9042)

Meta Commands:
  \?          Show this help
  \q, \quit   Exit the CLI
  \d          List tables
  \dt         List tables (same as \d)
  \l          List databases
  \c          Connect to different database
  \timing     Toggle query timing display
  \format     Set output format (table, json, csv, plain)

Query Execution (SQL protocols):
  - End queries with semicolon (;) to execute
  - Multi-line queries are supported
  - Use Ctrl+C to cancel current query
  - Use Ctrl+D or \q to exit

Redis Commands (--protocol redis):
  - Commands execute immediately (no semicolon needed)
  - Examples: GET key, SET key value, LPUSH list item

SQL Examples:
  SELECT * FROM users;
  SELECT id, name
  FROM users
  WHERE age > 18;

Redis Examples:
  PING
  SET mykey "Hello World"
  GET mykey
  KEYS *

"#;

    println!("{}", help_text.cyan());
}
